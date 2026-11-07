#!/usr/bin/env python3
"""Find new upstream launcher tags and replay Atlas's patches onto them.

Upstream tags live in refs/upstream/tags/ (fetched with
`git fetch upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*'`),
so they never mix with this repository's own launcher@X.Y.Z release tags.
"""

import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

UPSTREAM_REFS = "refs/upstream/tags"
TAG_RE = re.compile(r"launcher@(\d+)\.(\d+)\.(\d+)")


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


def _git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=check,
        capture_output=True,
        text=True,
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


@dataclass
class RebaseResult:
    ok: bool
    failed_patch: str | None
    base: str


def rebase_patches(repo: Path, new_tag: str) -> RebaseResult:
    if parse_launcher_tag(new_tag) is None:
        raise ValueError(f"not an upstream launcher tag: {new_tag!r}")
    base = current_base(repo)
    if base is None:
        raise ValueError(f"HEAD has no upstream launcher tag in {UPSTREAM_REFS}/ as an ancestor")
    done = _git(
        repo,
        "rebase",
        "--onto",
        f"{UPSTREAM_REFS}/{new_tag}",
        f"{UPSTREAM_REFS}/{base}",
        check=False,
    )
    if done.returncode == 0:
        return RebaseResult(ok=True, failed_patch=None, base=new_tag)
    stopped = _git(repo, "rev-parse", "--verify", "-q", "REBASE_HEAD", check=False)
    failed_patch = None
    if stopped.returncode == 0:
        failed_patch = _git(repo, "log", "-1", "--format=%s", stopped.stdout.strip()).stdout.strip()
    _git(repo, "rebase", "--abort", check=False)
    if failed_patch is None:
        sys.stderr.write(done.stderr)
    return RebaseResult(ok=False, failed_patch=failed_patch, base=base)


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
    repo = repo or Path(__file__).resolve().parent.parent
    usage = "usage: atlas_sync.py new-tag | base | next-version | rebase TAG | title ATLAS UPSTREAM"
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
    if command == "rebase" and len(args) == 1:
        result = rebase_patches(repo, args[0])
        if result.ok:
            print(result.base)
            return 0
        if result.failed_patch is not None:
            print(f"FAILED_PATCH={result.failed_patch}")
        return 1
    if command == "title" and len(args) == 2:
        print(release_title(args[0], args[1]))
        return 0
    print(usage, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
