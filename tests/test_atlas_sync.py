import contextlib
import io
import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

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
            path.write_text(text, encoding="utf-8")

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

    def test_release_title_names_horizon_atlas_and_the_upstream_version(self):
        self.assertEqual(
            atlas_sync.release_title("0.1.0", "launcher@0.2.50"),
            "Horizon Atlas 0.1.0 · based on Horizon launcher 0.2.50",
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

    def test_an_upstream_workflow_atlas_rewrote_and_later_renamed_keeps_atlas_content(self):
        steps = "".join(f"      - run: step {n}\n" for n in range(20))
        own = f"{WORKFLOWS}/atlas-ci.yml"
        self.build(
            {"a.txt": "line\n", self.ci: "name: ci\n" + steps},
            {self.ci: "name: CI (upstream)\n" + steps},
        )
        self.atlas_commit("atlas: workflows", {self.ci: "name: atlas-ci\n" + steps})
        self.atlas_commit(
            "atlas: unique workflow names",
            {self.ci: None, own: "name: atlas-ci\n" + steps + "      - run: atlas\n"},
        )

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok, result.failed_patch)
        self.assertFalse((self.atlas / self.ci).exists())
        self.assertEqual((self.atlas / own).read_text(), "name: atlas-ci\n" + steps + "      - run: atlas\n")
        self.assertEqual(
            self.subjects(3), ["atlas: unique workflow names", "atlas: workflows", "c2"]
        )
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


ACTIONLINT = ".github/actionlint.yaml"


class ActionlintConfigTests(WorkflowFixture):
    """`.github/actionlint.yaml` lists runner labels for the workflows in the tree, which in Atlas are its own."""

    base = "self-hosted-runner:\n  labels: [horizon-game, windows-11-vs2026-arm]\n"
    theirs = "self-hosted-runner:\n  labels: [horizon-game, windows-11-vs2026-arm, blacksmith]\n"
    mine = "self-hosted-runner:\n  labels: [windows-11-vs2026-arm]\n"

    def test_both_changed_keeps_the_atlas_commits_version(self):
        self.build({ACTIONLINT: self.base, "a.txt": "line\n"}, {ACTIONLINT: self.theirs})
        self.atlas_commit("atlas: daily sync", {ACTIONLINT: self.mine})
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertEqual((self.atlas / ACTIONLINT).read_text(), self.mine)
        self.assertEqual(self.subjects(3), ["patch: one", "atlas: daily sync", "c2"])
        self.assertEqual(
            [(subject, path) for subject, path, _ in result.resolved],
            [("atlas: daily sync", ACTIONLINT)],
        )
        self.assert_clean()

    def test_upstream_deleting_it_keeps_the_atlas_commits_version(self):
        self.build({ACTIONLINT: self.base}, {ACTIONLINT: None})
        self.atlas_commit("atlas: daily sync", {ACTIONLINT: self.mine})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertEqual((self.atlas / ACTIONLINT).read_text(), self.mine)
        self.assert_clean()

    def test_it_beside_a_source_conflict_still_names_the_commit(self):
        self.build(
            {ACTIONLINT: self.base, "a.txt": "line\n"},
            {ACTIONLINT: self.theirs, "a.txt": "line upstream\n"},
        )
        self.atlas_commit("patch: mixed", {ACTIONLINT: self.mine, "a.txt": "line patched\n"})
        head = git(self.atlas, "rev-parse", "HEAD")

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: mixed")
        self.assert_clean()
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_another_config_file_is_still_left_to_a_human(self):
        other = ".github/dependabot.yml"
        self.build({other: "version: 2\n"}, {other: "version: 2\nupdates: []\n"})
        self.atlas_commit("atlas: dependabot", {other: "version: 2\n# mine\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "atlas: dependabot")
        self.assert_clean()


class CliTests(SyncFixture):
    def cli(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
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
            (0, "Horizon Atlas 0.1.0 · based on Horizon launcher 0.2.50\n"),
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


def lang_text(pairs):
    """A launcher/lang/*.json file: one `"key": "value"` entry per line, as upstream writes them."""
    lines = [
        f"  {json.dumps(key, ensure_ascii=False)}: {json.dumps(value, ensure_ascii=False)}"
        for key, value in pairs
    ]
    return "{\n" + ",\n".join(lines) + "\n}\n"


def inserted(pairs, after, added):
    keys = [key for key, _ in pairs]
    at = keys.index(after) + 1 if after is not None else 0
    return pairs[:at] + added + pairs[at:]


INSTALLED = (
    "This version has a {installed} game build, but Settings selects {selected}. "
    "Install the selected build from Versions before launching."
)
FOUND = "Found by itself in your launcher's folder. Or pick the {version}.jar file."
JAR_KEYS = [("Minecraft jar", "Minecraft jar"), (FOUND, FOUND), ("Choose jar…", "Choose jar…")]
CS_BASE = [
    ("Play", "Hrát"),
    (INSTALLED, INSTALLED),
    ("Automatic", "Automatic"),
    ("Unknown", "Unknown"),
    ("Claim for free", "Získat zdarma"),
]
CS_UPSTREAM = [
    ("Play", "Hrát"),
    (INSTALLED, "Tato verze má sestavení hry {installed}, ale Nastavení vybírá {selected}."),
    ("Automatic", "„Automaticky“"),
    ("Unknown", "Neznámé"),
    ("Claim for free", "Získat zdarma"),
]
CS_PATCH = inserted(CS_BASE, INSTALLED, JAR_KEYS)


class MergeAddedKeysTests(unittest.TestCase):
    def merge(self, base, upstream, patch):
        return atlas_sync.merge_added_keys(lang_text(base), lang_text(upstream), lang_text(patch))

    def test_keys_added_between_lines_upstream_translated_follow_their_anchor(self):
        merged = self.merge(CS_BASE, CS_UPSTREAM, CS_PATCH)

        self.assertEqual(merged, lang_text(inserted(CS_UPSTREAM, INSTALLED, JAR_KEYS)))

    def test_keys_added_at_the_end_of_the_object_get_their_commas(self):
        base = [("A", "a"), ("B", "b")]
        upstream = [("A", "á"), ("B", "bé")]
        added = [("X", "x"), ("Y", "y")]

        merged = self.merge(base, upstream, base + added)

        self.assertEqual(merged, lang_text(upstream + added))
        self.assertEqual(list(json.loads(merged)), ["A", "B", "X", "Y"])

    def test_an_added_last_line_gets_a_comma_when_upstream_appended_after_it(self):
        base = [("A", "a"), ("B", "b")]
        upstream = [("A", "a"), ("B", "bé"), ("C", "c")]

        merged = self.merge(base, upstream, base + [("X", "x")])

        self.assertEqual(merged, lang_text(inserted(upstream, "B", [("X", "x")])))

    def test_an_anchor_deleted_upstream_falls_back_to_an_earlier_key(self):
        base = [("A", "a"), ("B", "b"), ("C", "c")]
        upstream = [("A", "á"), ("C", "cé")]
        patch = [("A", "a"), ("X", "x"), ("B", "b"), ("Y", "y"), ("C", "c")]

        merged = self.merge(base, upstream, patch)

        self.assertEqual(merged, lang_text(inserted(upstream, "A", [("X", "x"), ("Y", "y")])))

    def test_without_an_anchor_left_the_keys_go_right_after_the_brace(self):
        base = [("A", "a"), ("B", "b")]
        upstream = [("B", "bé")]

        merged = self.merge(base, upstream, [("A", "a"), ("X", "x"), ("B", "b")])

        self.assertEqual(merged, lang_text([("X", "x"), ("B", "bé")]))

    def test_non_ascii_and_escapes_are_kept_byte_for_byte(self):
        base = '{\n  "Automatic": "Automatic",\n  "Say \\"hi\\"": "Say \\"hi\\""\n}\n'
        upstream = '{\n  "Automatic": "„Automaticky“",\n  "Say \\"hi\\"": "\\u00c9\\"ahoj\\""\n}\n'
        patch = (
            '{\n  "Automatic": "Automatic",\n  "Choose jar…": "Choose jar…",\n'
            '  "Say \\"hi\\"": "Say \\"hi\\""\n}\n'
        )

        merged = atlas_sync.merge_added_keys(base, upstream, patch)

        expected = (
            '{\n  "Automatic": "„Automaticky“",\n  "Choose jar…": "Choose jar…",\n'
            '  "Say \\"hi\\"": "\\u00c9\\"ahoj\\""\n}\n'
        )
        self.assertEqual(merged.encode("utf-8"), expected.encode("utf-8"))

    def test_text_that_is_not_one_entry_per_line_json_is_left_to_a_human(self):
        good = lang_text(CS_BASE)
        bad_texts = {
            "invalid json": good.replace('"Hrát",', '"Hrát"'),
            "trailing comma": good.replace('"Získat zdarma"\n', '"Získat zdarma",\n'),
            "two entries on a line": '{\n  "Play": "Hrát", "Unknown": "Unknown"\n}\n',
            "compact": json.dumps(dict(CS_BASE), ensure_ascii=False),
            "nested value": '{\n  "Play": {\n  "x": "y"\n  }\n}\n',
            "number value": '{\n  "Play": 1\n}\n',
            "duplicate keys": '{\n  "Play": "Hrát",\n  "Play": "Play"\n}\n',
        }
        patch = lang_text(CS_PATCH)
        upstream = lang_text(CS_UPSTREAM)
        for name, bad in bad_texts.items():
            with self.subTest(name, side="base"):
                self.assertIsNone(atlas_sync.merge_added_keys(bad, upstream, patch))
            with self.subTest(name, side="upstream"):
                self.assertIsNone(atlas_sync.merge_added_keys(good, bad, patch))
            with self.subTest(name, side="patch"):
                self.assertIsNone(atlas_sync.merge_added_keys(good, upstream, bad))

    def test_a_patch_that_does_more_than_add_keys_is_left_to_a_human(self):
        patches = {
            "changes a value": inserted(
                [("Play", "Hrát"), (INSTALLED, "changed")] + CS_BASE[2:], INSTALLED, JAR_KEYS
            ),
            "removes a key": inserted(
                [pair for pair in CS_BASE if pair[0] != "Unknown"], INSTALLED, JAR_KEYS
            ),
            "reorders keys": inserted([CS_BASE[2], CS_BASE[1], CS_BASE[0]] + CS_BASE[3:], INSTALLED, JAR_KEYS),
            "adds nothing": CS_BASE,
        }
        for name, patch in patches.items():
            with self.subTest(name):
                self.assertIsNone(self.merge(CS_BASE, CS_UPSTREAM, patch))

    def test_a_key_upstream_added_too_is_left_to_a_human(self):
        upstream = CS_UPSTREAM + [("Choose jar…", "Vybrat jar…")]

        self.assertIsNone(self.merge(CS_BASE, upstream, CS_PATCH))


LANG_CS = "launcher/lang/cs.json"
CS_MERGED = lang_text(inserted(CS_UPSTREAM, INSTALLED, JAR_KEYS))


class LangConflictTests(WorkflowFixture):
    def lang(self, path=LANG_CS):
        return (self.atlas / path).read_text(encoding="utf-8")

    def test_keys_added_beside_lines_upstream_translated_are_merged(self):
        self.build({LANG_CS: lang_text(CS_BASE)}, {LANG_CS: lang_text(CS_UPSTREAM)})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH), "jar.txt": "jar\n"})
        self.atlas_commit("patch: branding", {"b.txt": "mine\n"})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertEqual(result.base, "launcher@0.2.51")
        self.assertEqual(self.subjects(3), ["patch: branding", "patch: choose-jar", "c2"])
        self.assertEqual(self.lang(), CS_MERGED)
        self.assertEqual((self.atlas / "jar.txt").read_text(), "jar\n")
        self.assert_clean()

    def test_a_patch_that_changes_a_translated_line_is_named_and_aborted(self):
        self.build({LANG_CS: lang_text(CS_BASE)}, {LANG_CS: lang_text(CS_UPSTREAM)})
        reworded = [("Play", "Hrát"), (INSTALLED, "Atlas's wording")] + CS_BASE[2:]
        self.atlas_commit("patch: wording", {LANG_CS: lang_text(inserted(reworded, INSTALLED, JAR_KEYS))})
        head = git(self.atlas, "rev-parse", "HEAD")

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: wording")
        self.assert_clean()
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_a_lang_file_upstream_deleted_is_left_to_a_human(self):
        self.build({LANG_CS: lang_text(CS_BASE), "a.txt": "line\n"}, {LANG_CS: None})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH)})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: choose-jar")
        self.assert_clean()

    def test_a_workflow_and_a_lang_conflict_in_one_stop_are_both_resolved(self):
        ci = f"{WORKFLOWS}/ci.yml"
        self.build(
            {LANG_CS: lang_text(CS_BASE), ci: "ci\n"},
            {LANG_CS: lang_text(CS_UPSTREAM), ci: "ci changed\n"},
        )
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH), ci: None})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertFalse((self.atlas / ci).exists())
        self.assertEqual(self.lang(), CS_MERGED)
        self.assertEqual(self.subjects(2), ["patch: choose-jar", "c2"])
        self.assert_clean()

    def test_the_result_names_each_resolved_file_and_its_patch(self):
        ci = f"{WORKFLOWS}/ci.yml"
        self.build(
            {LANG_CS: lang_text(CS_BASE), ci: "ci\n", "a.txt": "line\n"},
            {LANG_CS: lang_text(CS_UPSTREAM), ci: "ci changed\n"},
        )
        self.atlas_commit("atlas: workflows", {ci: None})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH)})

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51")

        self.assertTrue(result.ok)
        self.assertEqual(
            [(subject, path) for subject, path, _ in result.resolved],
            [("atlas: workflows", ci), ("patch: choose-jar", LANG_CS)],
        )

    def test_the_report_lists_only_what_a_human_must_resolve(self):
        self.build(
            {LANG_CS: lang_text(CS_BASE), "a.txt": "line\n"},
            {LANG_CS: lang_text(CS_UPSTREAM), "a.txt": "line upstream\n"},
        )
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH), "a.txt": "line patched\n"})
        target = Path(self._tmp.name) / "conflict.md"

        result = atlas_sync.rebase_patches(self.atlas, "launcher@0.2.51", target)

        self.assertFalse(result.ok)
        self.assertEqual(result.failed_patch, "patch: choose-jar")
        report = target.read_text()
        self.assertIn("Conflicting files (1):", report)
        self.assertIn("`a.txt`", report)
        self.assertNotIn("cs.json", report)
        self.assert_clean()


class CliFixture(WorkflowFixture):
    def cli(self, *args):
        code, out, _ = self.cli_streams(*args)
        return code, out

    def cli_streams(self, *args):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = atlas_sync.main(list(args), repo=self.atlas)
        return code, out.getvalue(), err.getvalue()


class RebaseOntoTests(CliFixture):
    def upstream_main(self, files, subject):
        """A commit on upstream's main after its newest tag, fetched as upstream/main."""
        self.write(self.upstream, files)
        git(self.upstream, "add", "-A")
        git(self.upstream, "commit", "-q", "-m", subject)
        git(self.atlas, "fetch", "-q", str(self.upstream), "+refs/heads/main:refs/remotes/upstream/main")

    def test_rebase_onto_a_branch_resolves_lang_conflicts(self):
        self.build({LANG_CS: lang_text(CS_BASE)}, {"c.txt": "new\n"})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH)})
        self.upstream_main({LANG_CS: lang_text(CS_UPSTREAM)}, "main: translations")

        code, out = self.cli("rebase-onto", "upstream/main")

        self.assertEqual((code, out), (0, "upstream/main\n"))
        self.assertEqual(self.subjects(3), ["patch: choose-jar", "main: translations", "c2"])
        self.assertEqual((self.atlas / LANG_CS).read_text(encoding="utf-8"), CS_MERGED)
        self.assert_clean()

    def test_a_successful_rebase_lists_what_it_resolved_on_stderr(self):
        self.build({LANG_CS: lang_text(CS_BASE)}, {"c.txt": "new\n"})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH)})
        self.upstream_main({LANG_CS: lang_text(CS_UPSTREAM)}, "main: translations")

        code, out, err = self.cli_streams("rebase-onto", "upstream/main")

        self.assertEqual((code, out), (0, "upstream/main\n"))
        lines = err.splitlines()
        self.assertEqual(len(lines), 1)
        self.assertTrue(lines[0].startswith(f'resolved: {LANG_CS} in "patch: choose-jar" ('), lines[0])

    def test_an_aborted_rebase_lists_nothing_as_resolved(self):
        self.build({LANG_CS: lang_text(CS_BASE), "a.txt": "line\n"}, {"c.txt": "new\n"})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH), "a.txt": "line patched\n"})
        self.upstream_main({LANG_CS: lang_text(CS_UPSTREAM), "a.txt": "line upstream\n"}, "main: both")

        code, out, err = self.cli_streams("rebase-onto", "upstream/main")

        self.assertEqual((code, out), (1, "FAILED_PATCH=patch: choose-jar\n"))
        self.assertNotIn("resolved:", err)

    def test_rebase_onto_names_the_conflicting_patch_and_writes_the_report(self):
        self.build({"a.txt": "line\n"}, {"c.txt": "new\n"})
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})
        self.upstream_main({"a.txt": "line upstream\n"}, "main: rewrite a.txt")
        head = git(self.atlas, "rev-parse", "HEAD")
        target = Path(self._tmp.name) / "conflict.md"

        code, out = self.cli("rebase-onto", "upstream/main", "--report", str(target))

        self.assertEqual((code, out), (1, "FAILED_PATCH=patch: one\n"))
        report = target.read_text()
        self.assertIn("the rebase onto `upstream/main` stopped", report)
        self.assertIn("main: rewrite a.txt", report)
        self.assert_clean()
        self.assertEqual(git(self.atlas, "rev-parse", "HEAD"), head)

    def test_rebase_onto_a_missing_ref_fails_without_a_patch_name(self):
        self.build({"a.txt": "line\n"}, {"c.txt": "new\n"})
        self.atlas_commit("patch: one", {"a.txt": "line patched\n"})

        self.assertEqual(self.cli("rebase-onto", "upstream/nope"), (1, ""))
        self.assert_clean()


class ResolveTests(CliFixture):
    def stop_in_conflict(self):
        done = try_git(
            self.atlas, "-c", "merge.conflictStyle=diff3", "rebase", "--onto", NEW_REF,
            f"refs/upstream/tags/{OLD_BASE}", check=False,
        )
        self.assertNotEqual(done.returncode, 0)

    def unmerged(self):
        return git(self.atlas, "diff", "--name-only", "--diff-filter=U").split()

    def test_resolve_without_a_stopped_rebase_exits_2(self):
        self.build({"a.txt": "line\n"}, {"c.txt": "new\n"})

        self.assertEqual(self.cli("resolve"), (2, ""))

    def test_resolve_settles_a_lang_conflict_and_leaves_the_rebase_stopped(self):
        self.build({LANG_CS: lang_text(CS_BASE)}, {LANG_CS: lang_text(CS_UPSTREAM)})
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH)})
        self.stop_in_conflict()

        code, out = self.cli("resolve")

        self.assertEqual((code, out), (0, f"resolved: {LANG_CS}\n"))
        self.assertEqual((self.atlas / LANG_CS).read_text(encoding="utf-8"), CS_MERGED)
        self.assertEqual(self.unmerged(), [])
        self.assertTrue((self.atlas / ".git" / "rebase-merge").exists())
        try_git(self.atlas, "rebase", "--continue")
        self.assertEqual(self.subjects(2), ["patch: choose-jar", "c2"])
        self.assert_clean()

    def test_resolve_lists_what_is_left_and_exits_1(self):
        self.build(
            {LANG_CS: lang_text(CS_BASE), "a.txt": "line\n"},
            {LANG_CS: lang_text(CS_UPSTREAM), "a.txt": "line upstream\n"},
        )
        self.atlas_commit("patch: choose-jar", {LANG_CS: lang_text(CS_PATCH), "a.txt": "line patched\n"})
        self.stop_in_conflict()

        code, out = self.cli("resolve")

        self.assertEqual((code, out), (1, f"resolved: {LANG_CS}\nunmerged: a.txt\n"))
        self.assertEqual(self.unmerged(), ["a.txt"])
        self.assertEqual((self.atlas / LANG_CS).read_text(encoding="utf-8"), CS_MERGED)


UPSTREAM_ISSUES = "https://github.com/horizon-rust/horizon/issues"
UPSTREAM_FOOTER = (
    "Your launcher updates itself. New here? On Linux take the .AppImage below, make it executable and run it; "
    "on Windows and macOS download the archive, extract it and run `horizon-launcher` (Horizon.app on macOS). "
    "Website: https://playhorizon.dev\n"
    "\n"
    "---\n"
    "Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.\n"
)
BODY_0_2_69 = (
    "### Fixed\n"
    "- Port ledger: fix 1 stale citation in horizon-port-pack (#16881)\n"
    "- Change Horizon's licence from MIT to LGPL-3.0-or-later (#17111)\n"
    "\n"
    "### Contributors\n"
    "Thanks to @MSedgeMC, @kacpherek.\n"
    "\n" + UPSTREAM_FOOTER
)
NOTES_0_2_9 = """\
Horizon Atlas 0.2.9 is Horizon launcher 0.2.69 with Atlas's changes: play without a Microsoft account (offline names \
with skins and capes), your own copy of Minecraft, and its own install beside the official launcher.

**Windows:** download `horizon-launcher-setup.exe` below and run it (`horizon-launcher-setup-arm64.exe` on ARM). \
**Linux and macOS:** see [Install](https://github.com/nezbut/horizon-atlas#install). Already installed? Atlas shows \
the new version: press **Update** in its sidebar, or turn on **Update the launcher automatically** in Settings.

## What changed in Horizon

Horizon's own notes for its official launcher, as published. What Atlas adds (above) still applies.

<details>
<summary><b>Horizon launcher 0.2.69</b>: 2 changes</summary>

[Official release notes](https://github.com/horizon-rust/horizon/releases/tag/launcher%400.2.69)

#### Fixed
- Port ledger: fix 1 stale citation in horizon-port-pack ([#16881](https://github.com/horizon-rust/horizon/issues/16881))
- Change Horizon's licence from MIT to LGPL-3.0-or-later ([#17111](https://github.com/horizon-rust/horizon/issues/17111))

#### Contributors
Thanks to `@MSedgeMC`, `@kacpherek`.

</details>

---
Licence: LGPL-3.0-or-later ([LICENSE](https://github.com/nezbut/horizon-atlas/blob/launcher%400.2.9/LICENSE)). \
Source code of this release: [launcher@0.2.9](https://github.com/nezbut/horizon-atlas/tree/launcher%400.2.9). \
Unofficial: not affiliated with Horizon's authors, Mojang or Microsoft. You need your own copy of Minecraft: Java \
Edition."""


def upstream_release(version, body="### Fixed\n- a fix\n", draft=False):
    return {
        "tag_name": f"launcher@{version}",
        "name": f"Horizon launcher {version}",
        "body": body,
        "draft": draft,
        "prerelease": False,
    }


def atlas_release(version, base, title="Horizon Atlas", draft=False):
    return {
        "tag_name": f"launcher@{version}",
        "name": f"{title} {version} · based on Horizon launcher {base}",
        "body": "",
        "draft": draft,
        "prerelease": False,
    }


def section_versions(notes):
    return re.findall(r"^<summary><b>Horizon launcher (\d+\.\d+\.\d+)</b>", notes, re.M)


def official_link(version):
    return f"[Official release notes](https://github.com/horizon-rust/horizon/releases/tag/launcher%40{version})"


class CleanUpstreamNotesTests(unittest.TestCase):
    def clean(self, body):
        return atlas_sync.clean_upstream_notes(body)

    def test_a_real_body_keeps_its_changes_and_loses_the_install_footer(self):
        self.assertEqual(
            self.clean(BODY_0_2_69),
            "#### Fixed\n"
            f"- Port ledger: fix 1 stale citation in horizon-port-pack ([#16881]({UPSTREAM_ISSUES}/16881))\n"
            f"- Change Horizon's licence from MIT to LGPL-3.0-or-later ([#17111]({UPSTREAM_ISSUES}/17111))\n"
            "\n"
            "#### Contributors\n"
            "Thanks to `@MSedgeMC`, `@kacpherek`.",
        )

    def test_a_rule_before_the_update_line_is_where_the_cut_is(self):
        self.assertEqual(self.clean("- a fix\n\n---\nfooter\n\nYour launcher updates itself.\n"), "- a fix")
        self.assertEqual(self.clean("- a fix\r\n\r\n---\r\nfooter\r\n"), "- a fix")

    def test_headings_go_one_level_down(self):
        self.assertEqual(
            self.clean("# One\n## Two\n###### Six\n#hashtag\n"),
            "## One\n### Two\n###### Six\n#hashtag",
        )

    def test_mentions_are_quoted_so_nobody_upstream_is_notified(self):
        self.assertEqual(
            self.clean("Thanks to @a-b and @Zed9 (mail me@example.com, see https://x.dev/@c, `@code`)\n"),
            "Thanks to `@a-b` and `@Zed9` (mail me@example.com, see https://x.dev/@c, `@code`)",
        )

    def test_bare_issue_numbers_link_to_upstream_and_qualified_ones_stay(self):
        self.assertEqual(
            self.clean("Fixes #12 and horizon-rust/horizon#34, [#56](u), &#35;, a/#7, x#9, `#8`, ``a `#1` @b``\n"),
            f"Fixes [#12]({UPSTREAM_ISSUES}/12) and horizon-rust/horizon#34, [#56](u), &#35;, a/#7, x#9, `#8`, "
            "``a `#1` @b``",
        )

    def test_fenced_code_is_left_as_it_is(self):
        self.assertEqual(
            self.clean("```sh\n# not a heading @me #1\n```\n@you\n"),
            "```sh\n# not a heading @me #1\n```\n`@you`",
        )

    def test_details_tags_cannot_close_or_open_atlas_blocks(self):
        self.assertEqual(
            self.clean("- a fix\n</details>\n  </DETAILS>  \n<details><summary>x</summary>\nsee `</details>`\n"),
            "- a fix\n&lt;/details>\n  &lt;/DETAILS>  \n&lt;details><summary>x</summary>\nsee `</details>`",
        )


class ReleaseNotesTests(unittest.TestCase):
    def notes(self, atlas, upstream, license_id="LGPL-3.0-or-later"):
        return atlas_sync.release_notes(
            "0.2.9", "launcher@0.2.69", atlas, upstream, license_id, "nezbut/horizon-atlas"
        )

    def test_the_whole_body_for_one_covered_release(self):
        atlas = [atlas_release("0.2.8", "0.2.68", title="Atlas")]
        upstream = [upstream_release("0.2.69", BODY_0_2_69), upstream_release("0.2.68")]

        self.assertEqual(self.notes(atlas, upstream), NOTES_0_2_9)

    def test_every_release_since_the_previous_base_newest_first_from_either_title(self):
        upstream = [upstream_release(f"0.2.{n}") for n in (65, 69, 66, 70, 67, 68)]
        for title in ("Atlas", "Horizon Atlas"):
            with self.subTest(title=title):
                notes = self.notes([atlas_release("0.2.8", "0.2.66", title=title)], upstream)
                self.assertEqual(section_versions(notes), ["0.2.69", "0.2.68", "0.2.67"])

    def test_the_previous_release_is_the_newest_published_one_below_this_version(self):
        atlas = [
            atlas_release("0.2.9", "0.2.69"),
            atlas_release("0.2.10", "0.2.70"),
            atlas_release("0.2.8", "0.2.68", draft=True),
            atlas_release("0.2.7", "0.2.66"),
            atlas_release("0.2.6", "0.2.60"),
            {"tag_name": "nightly", "name": "based on Horizon launcher 0.2.1", "draft": False},
        ]
        upstream = [upstream_release(f"0.2.{n}") for n in range(60, 71)]

        self.assertEqual(section_versions(self.notes(atlas, upstream)), ["0.2.69", "0.2.68", "0.2.67"])

    def test_an_unknown_previous_base_covers_only_the_new_tag(self):
        upstream = [upstream_release("0.2.69"), upstream_release("0.2.68")]
        for atlas in ([], [{"tag_name": "launcher@0.2.8", "name": "Atlas 0.2.8", "draft": False}]):
            with self.subTest(atlas=atlas):
                self.assertEqual(section_versions(self.notes(atlas, upstream)), ["0.2.69"])

    def test_a_new_tag_upstream_has_not_listed_still_gets_its_section(self):
        notes = self.notes([atlas_release("0.2.8", "0.2.67")], [upstream_release("0.2.68")])

        self.assertEqual(section_versions(notes), ["0.2.69", "0.2.68"])
        self.assertIn(
            "<details>\n<summary><b>Horizon launcher 0.2.69</b></summary>\n\n"
            f"{official_link('0.2.69')}\n\nNo notes yet.\n\n</details>",
            notes,
        )

    def test_a_body_with_nothing_left_says_so(self):
        for body in (UPSTREAM_FOOTER, "", None):
            with self.subTest(body=body):
                notes = self.notes([], [upstream_release("0.2.69", body)])
                self.assertIn(
                    "<summary><b>Horizon launcher 0.2.69</b></summary>\n\n"
                    f"{official_link('0.2.69')}\n\nNo notes.\n\n</details>",
                    notes,
                )

    def test_a_fence_left_open_by_the_cut_is_closed_inside_the_block(self):
        notes = self.notes([], [upstream_release("0.2.69", "```\n- not a change\n---\nfooter\n")])

        self.assertIn(
            "<summary><b>Horizon launcher 0.2.69</b></summary>\n\n"
            f"{official_link('0.2.69')}\n\n```\n- not a change\n```\n\n</details>",
            notes,
        )

    def test_the_summary_counts_top_level_changes_outside_code_and_contributors(self):
        bodies = {
            "### Fixed\n- one\n  - a detail, not a change\n\n### Contributors\n- @someone\n": ": 1 change",
            "### New\n* one\n- two\n\n```\n- code\n```\n### Fixed\n- three\n": ": 3 changes",
            "Only words.\n": "",
        }
        for body, count in bodies.items():
            with self.subTest(body=body):
                notes = self.notes([], [upstream_release("0.2.69", body)])
                self.assertIn(f"<summary><b>Horizon launcher 0.2.69</b>{count}</summary>\n\n", notes)

    def test_at_most_ten_sections_then_a_link_to_the_rest_outside_them(self):
        upstream = [upstream_release(f"0.2.{n}") for n in range(51, 70)]

        notes = self.notes([atlas_release("0.2.8", "0.2.50")], upstream)

        self.assertEqual(section_versions(notes), [f"0.2.{n}" for n in range(69, 59, -1)])
        last_block = notes.rindex("</details>")
        more = notes.index("9 older")
        link = notes.index("(https://github.com/horizon-rust/horizon/releases)")
        self.assertLess(last_block, more)
        self.assertLess(more, link)
        self.assertLess(link, notes.index("\n---\n"))

    def test_a_release_on_the_same_base_says_horizon_did_not_change(self):
        notes = self.notes([atlas_release("0.2.8", "0.2.69")], [upstream_release("0.2.69")])

        self.assertEqual(section_versions(notes), [])
        self.assertIn(
            "## What changed in Horizon\n\nNothing: Atlas 0.2.8 was already based on Horizon launcher 0.2.69.\n\n---\n",
            notes,
        )

    def test_without_a_licence_id_the_line_points_to_the_file(self):
        notes = self.notes([], [], license_id=None)

        self.assertIn(
            "\n---\nLicence: see [LICENSE](https://github.com/nezbut/horizon-atlas/blob/launcher%400.2.9/LICENSE). ",
            notes,
        )


class ReleaseNotesCliTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.repo = self.tmp / "repo"
        (self.repo / "launcher").mkdir(parents=True)
        self.manifest = self.repo / "launcher" / "Cargo.toml"
        self.manifest.write_text('[package]\nname = "horizon-launcher"\nlicense = "LGPL-3.0-or-later"\n')
        self.atlas_json = self.tmp / "atlas.json"
        self.upstream_json = self.tmp / "upstream.json"
        env = mock.patch.dict(os.environ)
        env.start()
        self.addCleanup(env.stop)
        os.environ.pop("GITHUB_REPOSITORY", None)

    def cli(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            code = atlas_sync.main(list(args), repo=self.repo)
        return code, out.getvalue()

    def release_notes(self):
        return self.cli("release-notes", "0.2.9", "launcher@0.2.69", str(self.atlas_json), str(self.upstream_json))

    def test_release_notes_reads_both_lists_the_licence_and_the_repository(self):
        self.atlas_json.write_text(json.dumps([atlas_release("0.2.8", "0.2.67", title="Atlas")]))
        self.upstream_json.write_text(json.dumps([upstream_release(f"0.2.{n}") for n in (67, 68, 69)]))
        os.environ["GITHUB_REPOSITORY"] = "someone/fork"

        code, out = self.release_notes()

        self.assertEqual(code, 0)
        self.assertEqual(section_versions(out), ["0.2.69", "0.2.68"])
        self.assertIn("[Install](https://github.com/someone/fork#install)", out)
        self.assertIn(
            "Licence: LGPL-3.0-or-later ([LICENSE](https://github.com/someone/fork/blob/launcher%400.2.9/LICENSE))",
            out,
        )

    def test_missing_empty_or_invalid_lists_count_as_no_releases(self):
        contents = [None, "", "not json", '{"message": "Bad credentials"}', '[1, "x", null, {"tag_name": 5}]']
        for text in contents:
            with self.subTest(text=text):
                for path in (self.atlas_json, self.upstream_json):
                    path.unlink(missing_ok=True)
                    if text is not None:
                        path.write_text(text)

                code, out = self.release_notes()

                self.assertEqual(code, 0)
                self.assertTrue(out.startswith("Horizon Atlas 0.2.9 is Horizon launcher 0.2.69 "), out)
                self.assertIn(f"{official_link('0.2.69')}\n\nNo notes yet.\n", out)
                self.assertIn("[launcher@0.2.9](https://github.com/nezbut/horizon-atlas/tree/launcher%400.2.9)", out)

    def test_an_unreadable_manifest_leaves_only_see_licence(self):
        for text in (None, "not = [toml", '[package]\nname = "horizon-launcher"\n'):
            with self.subTest(text=text):
                self.manifest.unlink(missing_ok=True)
                if text is not None:
                    self.manifest.write_text(text)

                code, out = self.release_notes()

                self.assertEqual(code, 0)
                self.assertIn("Licence: see [LICENSE](", out)

    def test_a_version_or_tag_that_is_not_x_y_z_is_a_usage_error(self):
        for version, tag in (("0.2", "launcher@0.2.69"), ("0.2.9", "nightly")):
            with self.subTest(version=version, tag=tag):
                code, out = self.cli("release-notes", version, tag, str(self.atlas_json), str(self.upstream_json))
                self.assertEqual((code, out), (2, ""))


if __name__ == "__main__":
    unittest.main()
