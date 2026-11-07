import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools import atlas_sync


def git(repo, *args):
    done = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        capture_output=True,
        text=True,
        env={
            "PATH": "/usr/bin:/bin:/usr/local/bin",
            "HOME": str(repo),
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_SYSTEM": "/dev/null",
            "LC_ALL": "C",
        },
    )
    return done.stdout


def init_repo(path):
    path.mkdir(parents=True, exist_ok=True)
    git(path, "init", "-q", "-b", "main")
    git(path, "config", "user.name", "Test")
    git(path, "config", "user.email", "test@example.com")
    git(path, "config", "commit.gpgsign", "false")


def commit_file(repo, name, text, subject):
    (repo / name).write_text(text)
    git(repo, "add", name)
    git(repo, "commit", "-q", "-m", subject)


class SyncFixture(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        root = Path(self._tmp.name)
        self.upstream = root / "upstream"
        self.atlas = root / "atlas"
        init_repo(self.upstream)
        commit_file(self.upstream, "a.txt", "line\n", "c1")
        git(self.upstream, "tag", "launcher@0.2.50")
        commit_file(self.upstream, "c.txt", "new\n", "c2")
        git(self.upstream, "tag", "launcher@0.2.51")

    def conflict_upstream(self):
        git(self.upstream, "tag", "-d", "launcher@0.2.51")
        git(self.upstream, "reset", "-q", "--hard", "launcher@0.2.50")
        (self.upstream / "c.txt").write_text("new\n")
        (self.upstream / "a.txt").write_text("line upstream\n")
        git(self.upstream, "add", ".")
        git(self.upstream, "commit", "-q", "-m", "c2")
        git(self.upstream, "tag", "launcher@0.2.51")

    def atlas_on_first_tag(self):
        init_repo(self.atlas)
        git(
            self.atlas,
            "fetch",
            "-q",
            str(self.upstream),
            "+refs/tags/launcher@*:refs/upstream/tags/launcher@*",
        )
        git(self.atlas, "reset", "-q", "--hard", "refs/upstream/tags/launcher@0.2.50")
        commit_file(self.atlas, "a.txt", "line patched\n", "patch: one")
        commit_file(self.atlas, "b.txt", "mine\n", "patch: two")


class TagParsingTests(unittest.TestCase):
    def test_parse_launcher_tag(self):
        self.assertEqual(atlas_sync.parse_launcher_tag("launcher@0.2.50"), (0, 2, 50))
        for name in ["launcher-v0.16.0", "launcher@0.2.51-rc1", "nightly", "launcher@1.2"]:
            self.assertIsNone(atlas_sync.parse_launcher_tag(name), name)

    def test_latest_launcher_tag_sorts_numerically(self):
        names = ["launcher@0.2.9", "launcher@0.2.50", "launcher@0.10.0", "launcher-v9.0.0"]
        self.assertEqual(atlas_sync.latest_launcher_tag(names), "launcher@0.10.0")
        self.assertIsNone(atlas_sync.latest_launcher_tag([]))

    def test_next_atlas_version(self):
        self.assertEqual(atlas_sync.next_atlas_version([]), "0.1.0")
        self.assertEqual(
            atlas_sync.next_atlas_version(["launcher@0.1.0", "launcher@0.1.1", "v2"]), "0.1.2"
        )
        self.assertEqual(
            atlas_sync.release_title("0.1.0", "launcher@0.2.50"),
            "Atlas 0.1.0 · based on Horizon launcher 0.2.50",
        )


class RebaseTests(SyncFixture):
    def test_rebase_replays_patches_onto_the_new_tag(self):
        self.atlas_on_first_tag()
        self.assertEqual(atlas_sync.current_base(self.atlas), "launcher@0.2.50")
        self.assertEqual(atlas_sync.new_tag(self.atlas), "launcher@0.2.51")

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertIsNone(result.failed_patch)
        self.assertEqual(result.base, "launcher@0.2.51")
        subjects = git(self.atlas, "log", "--format=%s").split("\n")[:3]
        self.assertEqual(subjects, ["patch: two", "patch: one", "c2"])
        self.assertEqual(atlas_sync.current_base(self.atlas), "launcher@0.2.51")
        self.assertIsNone(atlas_sync.new_tag(self.atlas))

    def test_a_conflicting_patch_is_named_and_the_repo_left_clean(self):
        self.conflict_upstream()
        self.atlas_on_first_tag()
        head = git(self.atlas, "rev-parse", "HEAD")

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: one")
        self.assertEqual(git(self.atlas, "status", "--porcelain"), "")
        self.assertFalse((self.atlas / ".git" / "rebase-merge").exists())
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_own_release_tags_are_not_upstream_tags(self):
        self.atlas_on_first_tag()
        git(self.atlas, "tag", "launcher@9.9.9")
        self.assertEqual(atlas_sync.current_base(self.atlas), "launcher@0.2.50")
        self.assertEqual(atlas_sync.new_tag(self.atlas), "launcher@0.2.51")


class CliTests(SyncFixture):
    def cli(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = atlas_sync.main(list(args), repo=self.atlas)
        return code, out.getvalue()

    def test_new_tag_is_silent_when_up_to_date(self):
        self.atlas_on_first_tag()
        git(self.atlas, "reset", "-q", "--hard", "refs/upstream/tags/launcher@0.2.51")
        self.assertEqual(self.cli("new-tag"), (0, ""))

    def test_new_tag_prints_the_newer_tag(self):
        self.atlas_on_first_tag()
        self.assertEqual(self.cli("new-tag"), (0, "launcher@0.2.51\n"))

    def test_rebase_conflict_prints_failed_patch_and_exits_1(self):
        self.conflict_upstream()
        self.atlas_on_first_tag()
        code, out = self.cli("rebase", "launcher@0.2.51")
        self.assertEqual(code, 1)
        self.assertIn("FAILED_PATCH=patch: one", out)

    def test_next_version_reads_this_repos_release_tags(self):
        self.atlas_on_first_tag()
        git(self.atlas, "tag", "launcher@0.1.0")
        git(self.atlas, "tag", "launcher@0.1.1")
        self.assertEqual(self.cli("next-version"), (0, "0.1.2\n"))

    def test_title_and_base(self):
        self.atlas_on_first_tag()
        self.assertEqual(self.cli("base"), (0, "launcher@0.2.50\n"))
        self.assertEqual(
            self.cli("title", "0.1.0", "launcher@0.2.50"),
            (0, "Atlas 0.1.0 · based on Horizon launcher 0.2.50\n"),
        )


if __name__ == "__main__":
    unittest.main()
