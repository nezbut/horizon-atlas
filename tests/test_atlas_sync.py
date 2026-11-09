import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools import atlas_sync


def try_git(repo, *args, check=True):
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=check,
        capture_output=True,
        text=True,
        env={
            "PATH": "/usr/bin:/bin:/usr/local/bin",
            "HOME": str(repo),
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_SYSTEM": "/dev/null",
            "GIT_EDITOR": "true",
            "LC_ALL": "C",
        },
    )


def git(repo, *args):
    return try_git(repo, *args).stdout


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

    def conflict_upstream(self, subject="c2"):
        git(self.upstream, "tag", "-d", "launcher@0.2.51")
        git(self.upstream, "reset", "-q", "--hard", "launcher@0.2.50")
        (self.upstream / "c.txt").write_text("new\n")
        (self.upstream / "a.txt").write_text("line upstream\n")
        git(self.upstream, "add", ".")
        git(self.upstream, "commit", "-q", "-m", subject)
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


WORKFLOWS = ".github/workflows"


class WorkflowFixture(SyncFixture):
    """An upstream whose two tags differ in `newer` (a text, or None to delete), and an Atlas on the first."""

    def build(self, base, newer):
        self.upstream = Path(self._tmp.name) / "upstream-workflows"
        init_repo(self.upstream)
        self.write(self.upstream, base)
        git(self.upstream, "add", "-A")
        git(self.upstream, "commit", "-q", "-m", "c1")
        git(self.upstream, "tag", "launcher@0.2.50")
        self.write(self.upstream, newer)
        git(self.upstream, "add", "-A")
        git(self.upstream, "commit", "-q", "-m", "c2")
        git(self.upstream, "tag", "launcher@0.2.51")
        init_repo(self.atlas)
        git(
            self.atlas,
            "fetch",
            "-q",
            str(self.upstream),
            "+refs/tags/launcher@*:refs/upstream/tags/launcher@*",
        )
        git(self.atlas, "reset", "-q", "--hard", "refs/upstream/tags/launcher@0.2.50")

    @staticmethod
    def write(repo, files):
        for name, text in files.items():
            path = repo / name
            if text is None:
                path.unlink()
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)

    def atlas_commit(self, subject, files):
        self.write(self.atlas, files)
        git(self.atlas, "add", "-A")
        git(self.atlas, "commit", "-q", "-m", subject)

    def assert_clean(self):
        self.assertEqual(git(self.atlas, "status", "--porcelain"), "")
        self.assertFalse((self.atlas / ".git" / "rebase-merge").exists())

    def subjects(self, count):
        return git(self.atlas, "log", "--format=%s").split("\n")[:count]


class TagParsingTests(unittest.TestCase):
    def test_parse_launcher_tag(self):
        self.assertEqual(atlas_sync.parse_launcher_tag("launcher@0.2.50"), (0, 2, 50))
        ignored = [
            "launcher-v0.16.0",
            "launcher@0.2.51-rc1",
            "nightly",
            "launcher@1.2",
            "v0.2.97",
            "nightly-2026-11-07",
        ]
        for name in ignored:
            self.assertIsNone(atlas_sync.parse_launcher_tag(name), name)

    def test_latest_launcher_tag_sorts_numerically(self):
        names = [
            "launcher@0.2.9",
            "launcher@0.2.50",
            "launcher@0.10.0",
            "launcher-v9.0.0",
            "v0.2.97",
            "v99.0.0",
            "nightly-2026-11-07",
        ]
        self.assertEqual(atlas_sync.latest_launcher_tag(names), "launcher@0.10.0")
        self.assertEqual(
            atlas_sync.latest_launcher_tag(["v0.2.97", "nightly-2026-11-07"]), None
        )
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


class UpstreamWorkflowTests(WorkflowFixture):
    ci = f"{WORKFLOWS}/ci.yml"

    def test_a_deleted_workflow_that_upstream_edited_is_not_a_conflict(self):
        self.build({"a.txt": "line\n", self.ci: "ci\n"}, {self.ci: "ci changed\n"})
        self.atlas_commit("atlas: workflows", {self.ci: None})
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})
        self.atlas_commit("patch: two", {"b.txt": "mine\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertIsNone(result.failed_patch)
        self.assertFalse((self.atlas / self.ci).exists())
        self.assertEqual((self.atlas / "a.txt").read_text(), "line patched\n")
        self.assertEqual(
            self.subjects(4), ["patch: two", "patch: one", "atlas: workflows", "c2"]
        )
        self.assert_clean()
        self.assertEqual(atlas_sync.current_base(self.atlas), "launcher@0.2.51")

    def test_every_conflicting_workflow_is_resolved_in_one_stop(self):
        other = f"{WORKFLOWS}/parity.yml"
        self.build(
            {self.ci: "ci\n", other: "parity\n"}, {self.ci: "ci 2\n", other: "parity 2\n"}
        )
        self.atlas_commit("atlas: workflows", {self.ci: None, other: None})

        self.assertTrue(atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51").ok)
        self.assertEqual(git(self.atlas, "ls-files", WORKFLOWS), "")
        self.assert_clean()

    def test_workflow_conflicts_in_several_patches_are_resolved_in_turn(self):
        other = f"{WORKFLOWS}/parity.yml"
        self.build(
            {self.ci: "ci\n", other: "parity\n", "a.txt": "x\n"},
            {self.ci: "ci 2\n", other: "parity 2\n"},
        )
        self.atlas_commit("atlas: ci", {self.ci: None})
        self.atlas_commit("patch: one", {"a.txt": "y\n"})
        self.atlas_commit("atlas: parity", {other: None})

        self.assertTrue(atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51").ok)
        self.assertEqual(git(self.atlas, "ls-files", WORKFLOWS), "")
        self.assertEqual(
            self.subjects(4), ["atlas: parity", "patch: one", "atlas: ci", "c2"]
        )

    def test_a_conflict_in_a_source_file_is_still_named_and_left_clean(self):
        self.build({"a.txt": "line\n", self.ci: "ci\n"}, {"a.txt": "line upstream\n"})
        self.atlas_commit("atlas: workflows", {self.ci: None})
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})
        head = git(self.atlas, "rev-parse", "HEAD")

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: one")
        self.assert_clean()
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_a_mixed_conflict_is_not_resolved(self):
        self.build(
            {"a.txt": "line\n", self.ci: "ci\n"},
            {"a.txt": "line upstream\n", self.ci: "ci changed\n"},
        )
        self.atlas_commit("patch: mixed", {"a.txt": "line patched\n", self.ci: None})
        head = git(self.atlas, "rev-parse", "HEAD")

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: mixed")
        self.assert_clean()
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_a_patch_that_upstream_made_pointless_is_dropped(self):
        self.build({"a.txt": "line\n", self.ci: "ci\n"}, {self.ci: None})
        self.atlas_commit("atlas: tweak ci", {self.ci: "ci tweaked\n"})
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertFalse((self.atlas / self.ci).exists())
        self.assertEqual(self.subjects(2), ["patch: one", "c2"])
        self.assert_clean()

    def test_atlas_own_workflows_are_never_resolved_by_deleting(self):
        own = f"{WORKFLOWS}/atlas-ci.yml"
        self.build({own: "own\n"}, {own: "upstream wrote this\n"})
        self.atlas_commit("patch: own", {own: "mine\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: own")
        self.assert_clean()

    def test_new_upstream_workflows_are_dropped_in_one_commit_and_atlas_ones_kept(self):
        own = f"{WORKFLOWS}/atlas-ci.yml"
        new = f"{WORKFLOWS}/new.yml"
        self.build({"a.txt": "line\n"}, {new: "new\n"})
        self.atlas_commit("atlas: own ci", {own: "own\n"})
        self.assertTrue(atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51").ok)
        self.assertTrue((self.atlas / new).exists())

        dropped = atlas_sync.drop_upstream_workflows(self.atlas)

        self.assertEqual(dropped, ["new.yml"])
        self.assertFalse((self.atlas / new).exists())
        self.assertTrue((self.atlas / own).exists())
        self.assertEqual(self.subjects(1), ["atlas: drop upstream workflows (new.yml)"])
        self.assert_clean()
        head = git(self.atlas, "rev-parse", "HEAD")
        self.assertEqual(atlas_sync.drop_upstream_workflows(self.atlas), [])
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_several_dropped_workflows_are_listed_in_the_subject(self):
        self.build(
            {"a.txt": "line\n"},
            {f"{WORKFLOWS}/b.yml": "b\n", f"{WORKFLOWS}/a.yml": "a\n"},
        )
        self.assertTrue(atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51").ok)

        self.assertEqual(atlas_sync.drop_upstream_workflows(self.atlas), ["a.yml", "b.yml"])
        self.assertEqual(self.subjects(1), ["atlas: drop upstream workflows (a.yml, b.yml)"])

    def test_upstream_deleting_a_workflow_we_already_dropped_is_fine(self):
        self.build({"a.txt": "line\n", self.ci: "ci\n"}, {self.ci: None})
        self.assertEqual(atlas_sync.drop_upstream_workflows(self.atlas), ["ci.yml"])
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertFalse((self.atlas / self.ci).exists())
        self.assertEqual((self.atlas / "a.txt").read_text(), "line patched\n")
        self.assert_clean()


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

    def test_drop_workflows_prints_what_it_removed(self):
        self.atlas_on_first_tag()
        self.assertEqual(self.cli("drop-workflows"), (0, ""))
        (self.atlas / WORKFLOWS).mkdir(parents=True)
        for name in ["x.yml", "atlas-keep.yml"]:
            (self.atlas / WORKFLOWS / name).write_text("on: push\n")
        git(self.atlas, "add", "-A")
        git(self.atlas, "commit", "-q", "-m", "upstream-like")
        self.assertEqual(self.cli("drop-workflows"), (0, "x.yml\n"))
        self.assertEqual(git(self.atlas, "ls-files", WORKFLOWS).strip(), f"{WORKFLOWS}/atlas-keep.yml")

    def test_title_and_base(self):
        self.atlas_on_first_tag()
        self.assertEqual(self.cli("base"), (0, "launcher@0.2.50\n"))
        self.assertEqual(
            self.cli("title", "0.1.0", "launcher@0.2.50"),
            (0, "Atlas 0.1.0 · based on Horizon launcher 0.2.50\n"),
        )


OLD_BASE = "launcher@0.2.50"
NEW_REF = "refs/upstream/tags/launcher@0.2.51"


class ConflictReportTests(WorkflowFixture):
    def cli(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = atlas_sync.main(list(args), repo=self.atlas)
        return code, out.getvalue()

    def stop_in_conflict(self):
        self.conflict_upstream("upstream: rewrite a.txt")
        self.atlas_on_first_tag()
        done = try_git(
            self.atlas, "-c", "merge.conflictStyle=diff3", "rebase", "--onto", NEW_REF,
            f"refs/upstream/tags/{OLD_BASE}", check=False,
        )
        self.assertNotEqual(done.returncode, 0)

    def sectioned(self, prefix, sections=4):
        parts = []
        for section in range(sections):
            parts.append(f"{prefix} {section} 0\n")
            parts += [f"filler {section} {n}\n" for n in range(10)]
        return "".join(parts)

    def test_a_conflict_report_has_what_is_needed_to_resolve_it(self):
        self.stop_in_conflict()

        report = atlas_sync.conflict_report(self.atlas, OLD_BASE, NEW_REF)

        self.assertIn("patch: one", report)
        self.assertIn(OLD_BASE, report)
        self.assertIn("launcher@0.2.51", report)
        self.assertIn("a.txt", report)
        self.assertIn("|||||||", report)
        self.assertIn("line upstream", report)
        self.assertIn("line patched", report)
        self.assertRegex(report, r"(?m)^line$")
        self.assertIn("upstream: rewrite a.txt", report)
        self.assertIn("+line patched", report)
        self.assertIn("cargo fmt --check", report)
        self.assertIn(".claude/skills/atlas-issues", report)
        self.assertNotIn("b.txt", report)

    def test_a_huge_conflict_is_cut_at_the_cap_with_a_notice(self):
        def text(prefix):
            return "".join(f"{prefix} {n} {'x' * 800}\n" for n in range(100))

        self.build({"a.txt": text("base")}, {"a.txt": text("up")})
        self.atlas_commit("patch: huge", {"a.txt": text("mine")})
        try_git(
            self.atlas, "-c", "merge.conflictStyle=diff3", "rebase", "--onto", NEW_REF,
            f"refs/upstream/tags/{OLD_BASE}", check=False,
        )

        report = atlas_sync.conflict_report(self.atlas, OLD_BASE, NEW_REF)

        self.assertLessEqual(len(report), 50_000)
        self.assertIn("(truncated)", report)
        self.assertIn("cargo test --release --lib --bins", report)
        fences = [line for line in report.splitlines() if line.startswith("```")]
        self.assertEqual(len(fences) % 2, 0)

    def test_blocks_are_limited_to_three_per_file_and_eighty_lines(self):
        self.build(
            {"a.txt": self.sectioned("base")}, {"a.txt": self.sectioned("up")}
        )
        self.atlas_commit("patch: many", {"a.txt": self.sectioned("mine")})
        try_git(
            self.atlas, "-c", "merge.conflictStyle=diff3", "rebase", "--onto", NEW_REF,
            f"refs/upstream/tags/{OLD_BASE}", check=False,
        )

        report = atlas_sync.conflict_report(self.atlas, OLD_BASE, NEW_REF)

        self.assertEqual(report.count("<<<<<<< "), 3)
        self.assertIn("(1 more blocks)", report)
        self.assertIn("up 2 0", report)
        self.assertNotIn("up 3 0", report)

    def test_a_long_block_is_cut_at_eighty_lines(self):
        def text(prefix):
            return "".join(f"{prefix} {n}\n" for n in range(200))

        self.build({"a.txt": text("base")}, {"a.txt": text("up")})
        self.atlas_commit("patch: long", {"a.txt": text("mine")})
        try_git(
            self.atlas, "-c", "merge.conflictStyle=diff3", "rebase", "--onto", NEW_REF,
            f"refs/upstream/tags/{OLD_BASE}", check=False,
        )

        report = atlas_sync.conflict_report(self.atlas, OLD_BASE, NEW_REF)

        self.assertIn("up 0", report)
        self.assertNotIn("up 199", report)
        self.assertNotIn("mine 199", report)
        self.assertRegex(report, r"\(\d+ more lines\)")

    def test_at_most_six_files_and_eight_upstream_commits(self):
        names = [f"f{n}.txt" for n in range(8)]
        self.build({name: "x\n" for name in names}, {name: "up\n" for name in names})
        for number in range(10):
            self.write(self.upstream, {"f0.txt": f"up {number}\n"})
            git(self.upstream, "commit", "-q", "-am", f"upstream change {number}")
        git(self.upstream, "tag", "-f", "launcher@0.2.51")
        git(
            self.atlas, "fetch", "-q", str(self.upstream),
            "+refs/tags/launcher@*:refs/upstream/tags/launcher@*",
        )
        self.atlas_commit("patch: wide", {name: "mine\n" for name in names})
        try_git(
            self.atlas, "-c", "merge.conflictStyle=diff3", "rebase", "--onto", NEW_REF,
            f"refs/upstream/tags/{OLD_BASE}", check=False,
        )

        report = atlas_sync.conflict_report(self.atlas, OLD_BASE, NEW_REF)

        self.assertEqual(report.count("<<<<<<< "), 6)
        self.assertEqual(report.count("upstream change"), 8)
        self.assertIn("f7.txt", report)

    def test_rebase_report_writes_the_file_and_leaves_the_repo_clean(self):
        self.conflict_upstream("upstream: rewrite a.txt")
        self.atlas_on_first_tag()
        head = git(self.atlas, "rev-parse", "HEAD")
        target = Path(self._tmp.name) / "conflict.md"

        code, out = self.cli("rebase", "launcher@0.2.51", "--report", str(target))

        self.assertEqual(code, 1)
        self.assertIn("FAILED_PATCH=patch: one", out)
        text = target.read_text()
        self.assertIn("|||||||", text)
        self.assertIn("upstream: rewrite a.txt", text)
        self.assert_clean()
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_no_report_is_written_when_the_rebase_succeeds(self):
        self.atlas_on_first_tag()
        target = Path(self._tmp.name) / "conflict.md"

        code, _ = self.cli("rebase", "launcher@0.2.51", "--report", str(target))

        self.assertEqual(code, 0)
        self.assertFalse(target.exists())

    def test_no_report_is_written_for_auto_resolved_workflow_conflicts(self):
        ci = f"{WORKFLOWS}/ci.yml"
        self.build({"a.txt": "line\n", ci: "ci\n"}, {ci: "ci changed\n"})
        self.atlas_commit("atlas: workflows", {ci: None})
        target = Path(self._tmp.name) / "conflict.md"

        code, _ = self.cli("rebase", "launcher@0.2.51", "--report", str(target))

        self.assertEqual(code, 0)
        self.assertFalse(target.exists())
        self.assert_clean()

    def test_conflict_report_command_reads_a_repo_stopped_mid_rebase(self):
        self.stop_in_conflict()

        code, out = self.cli("conflict-report", OLD_BASE, NEW_REF)

        self.assertEqual(code, 0)
        self.assertIn("patch: one", out)
        self.assertIn("|||||||", out)
        self.assertIn("upstream: rewrite a.txt", out)

    def test_conflict_report_command_fails_without_a_stopped_rebase(self):
        self.atlas_on_first_tag()

        code, out = self.cli("conflict-report", OLD_BASE, NEW_REF)

        self.assertEqual(code, 1)
        self.assertEqual(out, "")

    def test_the_repo_option_selects_the_repository(self):
        self.atlas_on_first_tag()
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = atlas_sync.main(["--repo", str(self.atlas), "base"])
        self.assertEqual((code, out.getvalue()), (0, "launcher@0.2.50\n"))


if __name__ == "__main__":
    unittest.main()
