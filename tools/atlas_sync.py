#!/usr/bin/env python3
"""Find new upstream launcher tags and replay Atlas's patches onto them.

Upstream tags live in refs/upstream/tags/ (fetched with
`git fetch upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*'`),
so they never mix with this repository's own launcher@X.Y.Z release tags.
"""

import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

UPSTREAM_REFS = "refs/upstream/tags"
TAG_RE = re.compile(r"launcher@(\d+)\.(\d+)\.(\d+)")
WORKFLOWS_DIR = ".github/workflows"
OWN_PREFIX = "atlas-"
LANG_DIR = "launcher/lang"
LANG_ENTRY_RE = re.compile(
    r'[ \t]*(?P<key>"(?:[^"\\]|\\.)*")[ \t]*:[ \t]*(?P<value>"(?:[^"\\]|\\.)*")(?P<comma>[ \t]*,)?(?P<trail>[ \t\r]*)'
)
MAX_RESOLVE_ROUNDS = 50
REPORT_CHARS = 50_000
REPORT_FILES = 6
REPORT_COMMITS = 8
REPORT_BLOCKS = 3
REPORT_BLOCK_LINES = 80
REPORT_PATCH_LINES = 120
REPORT_CONTEXT = 3
TRUNCATED = (
    f"(truncated) The report was cut at {REPORT_CHARS:,} characters; "
    "reproduce the rest in a scratch clone with the atlas-issues skill."
)
GUIDANCE = """\
### How to fix

- Keep upstream's new code and re-apply only Atlas's seam (a constant, a condition, a key); a wide patch conflicts again at the next tag.
- Verify with `cargo fmt --check` and `cargo test --release --lib --bins` in `launcher/`.
- The step-by-step handling of this issue is in `.claude/skills/atlas-issues`.
"""


def parse_launcher_tag(name: str) -> tuple[int, int, int] | None:
    match = TAG_RE.fullmatch(name)
    if match is None:
        return None
    major, minor, patch = match.groups()
    return int(major), int(minor), int(patch)


def latest_launcher_tag(names: list[str]) -> str | None:
    versions = [(parsed, name) for name in names if (parsed := parse_launcher_tag(name))]
    if not versions:
        return None
    return max(versions)[1]


def next_atlas_version(tags: list[str]) -> str:
    newest = latest_launcher_tag(tags)
    if newest is None:
        return "0.1.0"
    major, minor, patch = parse_launcher_tag(newest)
    return f"{major}.{minor}.{patch + 1}"


def release_title(atlas_version: str, upstream_tag: str) -> str:
    upstream_version = upstream_tag.removeprefix("launcher@")
    return f"Atlas {atlas_version} · based on Horizon launcher {upstream_version}"


def _git(repo: Path, *args: str, check: bool = True, text: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=check,
        capture_output=True,
        text=text,
        env={**os.environ, "LC_ALL": "C", "GIT_EDITOR": "true"},
    )


def _upstream_tag_names(repo: Path, merged_into_head: bool = False) -> list[str]:
    args = ["for-each-ref", "--format=%(refname)"]
    if merged_into_head:
        args.append("--merged=HEAD")
    args.append(f"{UPSTREAM_REFS}/launcher@*")
    refs = _git(repo, *args).stdout.split()
    return [ref.removeprefix(f"{UPSTREAM_REFS}/") for ref in refs]


def current_base(repo: Path) -> str | None:
    return latest_launcher_tag(_upstream_tag_names(repo, merged_into_head=True))


def new_tag(repo: Path) -> str | None:
    newest = latest_launcher_tag(_upstream_tag_names(repo))
    if newest is None:
        return None
    base = current_base(repo)
    if base is not None and parse_launcher_tag(newest) <= parse_launcher_tag(base):
        return None
    return newest


def _is_upstream_workflow(path: str) -> bool:
    return path.startswith(f"{WORKFLOWS_DIR}/") and not Path(path).name.startswith(OWN_PREFIX)


def _unmerged_paths(repo: Path) -> list[str]:
    listing = _git(repo, "diff", "--name-only", "--diff-filter=U", "-z", check=False).stdout
    return [path for path in listing.split("\0") if path]


def drop_upstream_workflows(repo: Path) -> list[str]:
    """Remove upstream's workflow files (Atlas's own are named atlas-*) in one commit."""
    listing = _git(repo, "ls-files", "-z", "--", WORKFLOWS_DIR).stdout
    paths = sorted(path for path in listing.split("\0") if _is_upstream_workflow(path))
    if not paths:
        return []
    names = [path.removeprefix(f"{WORKFLOWS_DIR}/") for path in paths]
    _git(repo, "rm", "-q", "--", *paths)
    _git(repo, "commit", "-q", "-m", f"atlas: drop upstream workflows ({', '.join(names)})")
    return names


def _fenced(text: str, language: str = "") -> str:
    # A fence longer than any backtick run inside, so a Markdown file in a conflict cannot close it.
    longest = max((len(run) for run in re.findall(r"`+", text)), default=0)
    fence = "`" * max(3, longest + 1)
    return f"{fence}{language}\n{text.rstrip(chr(10))}\n{fence}"


def _conflict_blocks(text: str) -> list[tuple[int, int, list[str]]]:
    """(first line number, last line number, lines) of each conflict block, with its context."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    blocks = []
    index = 0
    while index < len(lines):
        if not lines[index].startswith("<<<<<<<"):
            index += 1
            continue
        end = next((n for n in range(index + 1, len(lines)) if lines[n].startswith(">>>>>>>")), None)
        if end is None:
            break
        first = max(0, index - REPORT_CONTEXT)
        last = min(len(lines), end + 1 + REPORT_CONTEXT)
        blocks.append((first + 1, last, lines[first:last]))
        index = end + 1
    return blocks


def _capped(lines: list[str], limit: int) -> str:
    text = "\n".join(lines[:limit])
    if len(lines) > limit:
        text += f"\n... ({len(lines) - limit} more lines)"
    return text


def _file_report(repo: Path, path: str, old_ref: str, new_ref: str) -> list[str]:
    out = [f"### `{path}`", ""]
    log = _git(repo, "log", "--format=%h %s", f"{old_ref}..{new_ref}", "--", path, check=False)
    commits = log.stdout.splitlines()
    out.append(f"Upstream commits that changed it between the old base and `{_short_ref(new_ref)}`:")
    out.append("")
    if commits:
        shown = commits[:REPORT_COMMITS]
        if len(commits) > REPORT_COMMITS:
            shown.append(f"... ({len(commits) - REPORT_COMMITS} more commits)")
        out += [_fenced("\n".join(shown)), ""]
    else:
        out += ["(none: the path was not changed upstream under this name)", ""]

    try:
        text = (repo / path).read_text(errors="replace")
    except OSError:
        text = ""
    blocks = _conflict_blocks(text)
    if blocks:
        out.append(
            "The conflict as it stands in the working tree: `<<<<<<<` is upstream's new code, `|||||||` the original, "
            "`=======` to `>>>>>>>` the Atlas patch's version."
        )
        out.append("")
        for number, (first, last, lines) in enumerate(blocks[:REPORT_BLOCKS], start=1):
            out += [f"Block {number} of {len(blocks)} (lines {first}-{last}):", ""]
            out += [_fenced(_capped(lines, REPORT_BLOCK_LINES)), ""]
        if len(blocks) > REPORT_BLOCKS:
            out += [f"({len(blocks) - REPORT_BLOCKS} more blocks)", ""]
    else:
        out += ["No conflict markers in the file: it was deleted, renamed or is binary on one side.", ""]

    shown_patch = _git(repo, "show", "REBASE_HEAD", "--format=", "--", path, check=False).stdout
    out.append("What the Atlas patch changes in this file:")
    out.append("")
    if shown_patch.strip():
        out += [_fenced(_capped(shown_patch.splitlines(), REPORT_PATCH_LINES), "diff"), ""]
    else:
        out += ["(the patch does not change this file)", ""]
    return out


def _short_ref(ref: str) -> str:
    return ref.removeprefix(f"{UPSTREAM_REFS}/")


def _cut_report(body: str, tail: str, limit: int) -> str:
    if len(body) + len(tail) <= limit:
        return body + tail
    notice = f"\n\n{TRUNCATED}\n\n"
    room = limit - len(tail) - len(notice) - 200
    cut = body[:room]
    cut = cut[: cut.rfind("\n")] if "\n" in cut else cut
    opener = None
    for line in cut.splitlines():
        if opener is None:
            match = re.match(r"`{3,}", line)
            opener = match.group() if match else None
        elif line.strip() == opener:
            opener = None
    closing = f"\n{opener}" if opener else ""
    return cut + closing + notice + tail


def conflict_report(repo: Path, old_base: str, new_ref: str) -> str:
    """Markdown for an issue: where the rebase stopped, what upstream changed there and what the patch changes.

    Call it while the rebase is stopped on a conflict (the commit being replayed is REBASE_HEAD).
    """
    stopped = _git(repo, "rev-parse", "--verify", "-q", "REBASE_HEAD", check=False)
    if stopped.returncode != 0:
        raise ValueError("no rebase is stopped on a conflict in this repository")
    commit = stopped.stdout.strip()
    subject = _git(repo, "log", "-1", "--format=%s", commit).stdout.strip()
    short = _git(repo, "rev-parse", "--short", commit).stdout.strip()
    old_ref = f"{UPSTREAM_REFS}/{old_base}"
    if _git(repo, "rev-parse", "--verify", "-q", f"{old_ref}^{{commit}}", check=False).returncode != 0:
        old_ref = old_base
    paths = sorted(_unmerged_paths(repo), key=lambda path: (_is_upstream_workflow(path), path))

    body = [
        f"## Conflict in the patch `{subject}` (`{short}`)",
        "",
        f"- Atlas is based on `{old_base}`; the rebase onto `{_short_ref(new_ref)}` stopped on this patch.",
        f"- Conflicting files ({len(paths)}):",
    ]
    body += [f"  - `{path}`" for path in paths]
    body.append("")
    for path in paths[:REPORT_FILES]:
        body += _file_report(repo, path, old_ref, new_ref)
    if len(paths) > REPORT_FILES:
        body += [f"({len(paths) - REPORT_FILES} more conflicting files are not shown.)", ""]
    return _cut_report("\n".join(body), "\n" + GUIDANCE, REPORT_CHARS)


@dataclass
class _LangFile:
    lines: list[str]
    entries: list[tuple[str, str, int]]  # key, value, line index
    brace: int  # the line index of `{`


def _lang_file(text: str) -> _LangFile | None:
    """A JSON object of strings written one `"key": "value"` entry per line, or None."""
    lines = text.split("\n")
    entries = []
    braces = []
    for index, line in enumerate(lines):
        bare = line.strip(" \t\r")
        if bare in ("", "{", "}"):
            if bare == "{":
                braces.append(index)
            continue
        match = LANG_ENTRY_RE.fullmatch(line)
        if match is None:
            return None
        try:
            entries.append((json.loads(match["key"]), json.loads(match["value"]), index))
        except ValueError:
            return None
    try:
        pairs = json.loads(text, object_pairs_hook=list)
    except ValueError:
        return None
    keys = [key for key, _, _ in entries]
    if len(braces) != 1 or len(set(keys)) != len(keys):
        return None
    if pairs != [(key, value) for key, value, _ in entries]:
        return None
    return _LangFile(lines, entries, braces[0])


def _with_comma(line: str, wanted: bool) -> str:
    match = LANG_ENTRY_RE.fullmatch(line)
    if (match["comma"] is not None) == wanted:
        return line
    return line[: match.end("value")] + ("," if wanted else "") + match["trail"]


def merge_added_keys(base_text: str, upstream_text: str, patch_text: str) -> str | None:
    """Upstream's translation file plus the keys the patch adds, or None when the patch does anything else.

    The three texts are one launcher/lang/*.json file at the merge base, in upstream's new version and in
    the patch. Each added line is copied from the patch, after the key that precedes it there (or the
    nearest earlier one upstream still has, or `{`); upstream's lines stay as they are, except a comma.
    """
    base, upstream, patch = (_lang_file(text) for text in (base_text, upstream_text, patch_text))
    if base is None or upstream is None or patch is None:
        return None
    base_values = {key: value for key, value, _ in base.entries}
    patch_values = {key: value for key, value, _ in patch.entries}
    kept = [key for key, _, _ in patch.entries if key in base_values]
    if kept != list(base_values) or any(patch_values[key] != value for key, value in base_values.items()):
        return None
    upstream_lines = {key: index for key, _, index in upstream.entries}
    added = [key for key, _, _ in patch.entries if key not in base_values]
    if not added or any(key in upstream_lines for key in added):
        return None

    inserts: dict[int, list[str]] = {}
    anchor = upstream.brace
    for key, _, index in patch.entries:
        if key in base_values:
            anchor = upstream_lines.get(key, anchor)
        else:
            inserts.setdefault(anchor, []).append(patch.lines[index])
    entry_lines = set(upstream_lines.values())
    merged = []  # (line, is an entry)
    for index, line in enumerate(upstream.lines):
        merged.append((line, index in entry_lines))
        merged += [(patch_line, True) for patch_line in inserts.get(index, [])]
    last_entry = max(at for at, (_, entry) in enumerate(merged) if entry)
    text = "\n".join(
        _with_comma(line, at != last_entry) if entry else line for at, (line, entry) in enumerate(merged)
    )

    try:
        pairs = json.loads(text, object_pairs_hook=list)
    except ValueError:
        return None
    expected = {key: value for key, value, _ in upstream.entries}
    expected.update((key, patch_values[key]) for key in added)
    if len(pairs) != len(expected) or dict(pairs) != expected:
        return None
    return text


@dataclass
class RebaseResult:
    ok: bool
    failed_patch: str | None
    base: str


def _rebase(repo: Path, *args: str) -> subprocess.CompletedProcess:
    # diff3: a conflict block also shows the original code, between upstream's and the patch's.
    return _git(repo, "-c", "merge.conflictStyle=diff3", "rebase", *args, check=False)


def _is_lang_file(path: str) -> bool:
    pure = PurePosixPath(path)
    return str(pure.parent) == LANG_DIR and pure.suffix == ".json"


def _stage_bytes(repo: Path, path: str, stage: int) -> bytes | None:
    shown = _git(repo, "show", f":{stage}:{path}", check=False, text=False)
    return shown.stdout if shown.returncode == 0 else None


def _stage_text(repo: Path, path: str, stage: int) -> str | None:
    data = _stage_bytes(repo, path, stage)
    if data is None:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _resolve_workflow(repo: Path, path: str) -> bool:
    # Both changed it: the Atlas commit's version wins (a later commit renames it to atlas-*, and drop-workflows
    # removes upstream-named files anyway). Either side deleted it: the deletion wins.
    atlas = _stage_bytes(repo, path, 3)
    if atlas is None or _stage_bytes(repo, path, 2) is None:
        return _git(repo, "rm", "-q", "--", path, check=False).returncode == 0
    (repo / path).write_bytes(atlas)
    return _git(repo, "add", "--", path, check=False).returncode == 0


def _resolve_lang(repo: Path, path: str) -> bool:
    # Index stages of a stopped rebase: 1 the merge base, 2 upstream's code (HEAD), 3 the patch being replayed.
    texts = [_stage_text(repo, path, stage) for stage in (1, 2, 3)]
    if None in texts:
        return False
    merged = merge_added_keys(*texts)
    if merged is None:
        return False
    (repo / path).write_bytes(merged.encode("utf-8"))
    return _git(repo, "add", "--", path, check=False).returncode == 0


def _resolve_stop(repo: Path) -> tuple[list[str], list[str]]:
    """Resolve, in a stopped rebase, the conflicts that need no human: (paths resolved, paths still unmerged)."""
    resolved, left = [], []
    for path in _unmerged_paths(repo):
        if _is_upstream_workflow(path):
            done = _resolve_workflow(repo, path)
        elif _is_lang_file(path):
            # Keys the patch adds beside lines upstream translated: upstream's file plus those keys.
            done = _resolve_lang(repo, path)
        else:
            done = False
        (resolved if done else left).append(path)
    return resolved, left


def _auto_resolve(repo: Path) -> bool:
    resolved, left = _resolve_stop(repo)
    return bool(resolved) and not left


def rebase_onto(repo: Path, new_ref: str, report: Path | None = None) -> RebaseResult:
    """Replay Atlas's commits after its upstream base onto `new_ref` (a tag's ref, or a branch like upstream/main).

    Conflicts that need no human are resolved on the way; on any other the rebase is aborted and the
    patch it stopped on named. On success `base` is `new_ref`, otherwise the unchanged old base tag.
    """
    base = current_base(repo)
    if base is None:
        raise ValueError(f"HEAD has no upstream launcher tag in {UPSTREAM_REFS}/ as an ancestor")
    done = _rebase(repo, "--onto", new_ref, f"{UPSTREAM_REFS}/{base}")
    rounds = 0
    while done.returncode != 0 and rounds < MAX_RESOLVE_ROUNDS:
        if not _auto_resolve(repo):
            break
        done = _rebase(repo, "--continue")
        rounds += 1
    if done.returncode == 0:
        return RebaseResult(ok=True, failed_patch=None, base=new_ref)
    stopped = _git(repo, "rev-parse", "--verify", "-q", "REBASE_HEAD", check=False)
    failed_patch = None
    if stopped.returncode == 0:
        failed_patch = _git(repo, "log", "-1", "--format=%s", stopped.stdout.strip()).stdout.strip()
        if report is not None:
            try:
                report.write_text(conflict_report(repo, base, new_ref))
            except Exception as error:  # the abort and the patch's name matter more than the report
                sys.stderr.write(f"could not write the conflict report: {error}\n")
    _git(repo, "rebase", "--abort", check=False)
    if failed_patch is None:
        sys.stderr.write(done.stderr)
    return RebaseResult(ok=False, failed_patch=failed_patch, base=base)


def rebase_patches(repo: Path, new_tag: str, report: Path | None = None) -> RebaseResult:
    if parse_launcher_tag(new_tag) is None:
        raise ValueError(f"not an upstream launcher tag: {new_tag!r}")
    result = rebase_onto(repo, f"{UPSTREAM_REFS}/{new_tag}", report)
    if result.ok:
        result.base = new_tag
    return result


def _tag_objects(repo: Path, prefix: str) -> dict[str, str]:
    listing = _git(
        repo, "for-each-ref", "--format=%(refname) %(objectname)", f"{prefix}/launcher@*"
    ).stdout
    return {
        ref.removeprefix(f"{prefix}/"): obj
        for ref, obj in (line.split() for line in listing.splitlines())
    }


def _own_release_tags(repo: Path) -> list[str]:
    # A fetch can auto-follow upstream's tags into refs/tags/; those are the same
    # objects as in refs/upstream/tags/, while Atlas's own releases never are.
    upstream = _tag_objects(repo, UPSTREAM_REFS)
    return [
        name for name, obj in _tag_objects(repo, "refs/tags").items() if upstream.get(name) != obj
    ]


def main(argv: list[str], repo: Path | None = None) -> int:
    usage = (
        "usage: atlas_sync.py [--repo PATH] new-tag | base | next-version | rebase TAG [--report FILE] | "
        "rebase-onto REF [--report FILE] | resolve | conflict-report OLD_BASE_TAG NEW_REF | drop-workflows | "
        "title ATLAS UPSTREAM"
    )
    argv = list(argv)
    if "--repo" in argv:
        at = argv.index("--repo")
        if at + 1 >= len(argv):
            print(usage, file=sys.stderr)
            return 2
        repo = Path(argv[at + 1])
        del argv[at : at + 2]
    repo = repo or Path(__file__).resolve().parent.parent
    command, args = (argv[0], argv[1:]) if argv else ("", [])
    if command == "new-tag" and not args:
        tag = new_tag(repo)
        if tag is not None:
            print(tag)
        return 0
    if command == "base" and not args:
        base = current_base(repo)
        if base is None:
            print("no upstream launcher tag is an ancestor of HEAD", file=sys.stderr)
            return 1
        print(base)
        return 0
    if command == "next-version" and not args:
        print(next_atlas_version(_own_release_tags(repo)))
        return 0
    if command in ("rebase", "rebase-onto") and (len(args) == 1 or (len(args) == 3 and args[1] == "--report")):
        report = Path(args[2]) if len(args) == 3 else None
        rebase = rebase_patches if command == "rebase" else rebase_onto
        result = rebase(repo, args[0], report)
        if result.ok:
            print(result.base)
            return 0
        if result.failed_patch is not None:
            print(f"FAILED_PATCH={result.failed_patch}")
        return 1
    if command == "resolve" and not args:
        if _git(repo, "rev-parse", "--verify", "-q", "REBASE_HEAD", check=False).returncode != 0:
            print("no rebase is stopped on a conflict in this repository", file=sys.stderr)
            return 2
        resolved, left = _resolve_stop(repo)
        for path in resolved:
            print(f"resolved: {path}")
        for path in left:
            print(f"unmerged: {path}")
        return 1 if left else 0
    if command == "conflict-report" and len(args) == 2:
        try:
            print(conflict_report(repo, args[0], args[1]))
        except ValueError as error:
            print(error, file=sys.stderr)
            return 1
        return 0
    if command == "drop-workflows" and not args:
        for name in drop_upstream_workflows(repo):
            print(name)
        return 0
    if command == "title" and len(args) == 2:
        print(release_title(args[0], args[1]))
        return 0
    print(usage, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
