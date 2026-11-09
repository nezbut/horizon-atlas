# Playbook: reproduce, fix, verify, hand over

`REPO` below is the owner's clone (`git rev-parse --show-toplevel`). `S` is a NEW, EMPTY directory outside it
(for example from `mktemp -d`), named after the tag, e.g. `.../sync-0.2.56`. Never work in `REPO` itself until the
owner has said yes.

## 1. Scratch clone that sees what the runner sees
```
git clone -q --no-hardlinks https://github.com/nezbut/horizon-atlas.git $S/atlas      # GitHub's main, like the runner
git -C $S/atlas remote add upstream https://github.com/horizon-rust/horizon.git
git -C $S/atlas fetch -q --no-tags upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*' '+refs/heads/main:refs/remotes/upstream/main'
uv run python $S/atlas/tools/atlas_sync.py base       # current base, e.g. launcher@0.2.54
uv run python $S/atlas/tools/atlas_sync.py new-tag    # newest upstream tag newer than the base ("" = none)
git -C $S/atlas switch -q -c sync-<tag>
```
If the user's `REPO` has commits that are not on GitHub (`git -C REPO log origin/main..main`), the issue was
produced from GitHub's `main`; say so, and base your work on GitHub's `main`.

## 2. Reproduce
```
uv run python $S/atlas/tools/atlas_sync.py rebase launcher@<new>     # exit 0 = no conflict; exit 1 + FAILED_PATCH=<subject> = conflict
```
The tool aborts and leaves the clone clean on a conflict. To resolve by hand run the same rebase directly:
```
git -C $S/atlas rebase --onto refs/upstream/tags/launcher@<new> refs/upstream/tags/launcher@<old base>
git -C $S/atlas diff --name-only --diff-filter=U      # the conflicting files of the commit it stopped at
```
For the early-warning issue use `refs/remotes/upstream/main` as the new base.

## 3. Resolve a conflict
- Markers: `grep -n '^<<<<<<<\|^=======\|^>>>>>>>' <file>`. During a rebase the part after `<<<<<<< HEAD` is
  **upstream's new code** (plus Atlas commits already replayed); the part before `>>>>>>> <sha> (<patch>)` is the
  Atlas patch being replayed.
- Keep upstream's new code and re-apply only Atlas's seam to it (`OFFLINE_PLAY`, `LAUNCHER_REPO`, one added
  condition, one added key). Read the whole block first (Read with offset and limit), then replace it with Edit:
  old string = the exact block including the three marker lines, new string = the resolution. If upstream renamed
  or restructured a function, adapt Atlas's seam to the new shape instead of restoring the old shape.
- Resolve by editing the block, not by taking a whole side with `git checkout --ours/--theirs`: a whole side
  loses either upstream's change or Atlas's seam. In `launcher/lang/*.json` conflicts upstream's side holds the
  new translations and Atlas's side holds old English placeholders plus Atlas's key: keep upstream's lines and
  add Atlas's key line. Check each JSON file still parses (`uv run python -c "import json; json.load(open(f))"`).
- Then: `git -C $S/atlas add <files>` and `GIT_EDITOR=true git -C $S/atlas rebase --continue`. It stops again at the
  next conflicting commit; repeat until it says the rebase succeeded.

## 4. Verify (all of it; paste the evidence into your report)
```
git -C $S/atlas grep -n -E '^(<<<<<<<|>>>>>>>) ' -- . ':!docs'                         # nothing
uv run python $S/atlas/tools/atlas_sync.py base                                         # prints launcher@<new>
git -C $S/atlas rev-list --count refs/upstream/tags/launcher@<new>..HEAD                # Atlas's commit count
git -C $S/atlas log --format='%s' refs/upstream/tags/launcher@<new>..HEAD | grep -c '^patch: '   # 5
uv run python -m unittest discover -s $S/atlas/tests -p 'test_atlas*.py'
cargo fmt --manifest-path $S/atlas/launcher/Cargo.toml --check
cargo clippy --manifest-path $S/atlas/launcher/Cargo.toml --release --all-targets -- -D warnings
cargo test --manifest-path $S/atlas/launcher/Cargo.toml --release --lib --bins --test owner_gate
cargo test --manifest-path $S/atlas/Cargo.toml -p horizon-release-sign
```
A cold build of the launcher takes about ten minutes. Share the build between scratch clones with
`CARGO_TARGET_DIR=<a directory outside the repository>/target-shared` and run long builds in the background. Also check `git -C $S/atlas diff refs/upstream/tags/launcher@<new>..HEAD --stat -- crates/horizon-release-sign`
shows only the `own-keys` patch and that `release-keys.txt` still lists both keys.

## 5. Show the user what changed
For each conflicting patch show the resolved hunks (`git -C $S/atlas show <rebased commit> -- <files>`) next to
what upstream changed in the same place (`git -C $S/atlas diff refs/upstream/tags/launcher@<old>..refs/upstream/tags/launcher@<new> -- <file>`),
and say in one paragraph why the conflict happened. Then ask the questions from the workflow in SKILL.md.

## 6. Apply to the user's repository (only after a yes)
```
git -C REPO fetch origin                                    # origin/main must still be what you started from
git -C REPO fetch $S/atlas sync-<tag>:sync-<tag>            # creates a local branch with the rebased stack; main is untouched
```
If `origin/main` moved since you started, stop and tell the user: the work must be redone on the new `main`.

## 7. Publish (only after a yes to pushing)
The rebased stack has new commit ids, so GitHub's `main` is not an ancestor of it and a plain push is refused;
the push that works is
```
git -C REPO push --force-with-lease origin sync-<tag>:main
```
Run it only if the owner said yes to pushing; if your environment does not let you run it, give the owner the
exact command.
Explain: `--force-with-lease` replaces `main` on GitHub only if it is still the commit the clone last fetched. A
plain `git push` is enough only when `git merge-base --is-ancestor origin/main sync-<tag>` succeeds.
Then locally: `git branch -f main sync-<tag>`, `git switch main`, `git branch -d sync-<tag>`.

## 8. After the push
- `atlas-ci` (started by the push) must go green.
- The daily sync will not release this base, because it no longer sees a newer tag. Release by hand: Actions ->
  `atlas-release` -> Run workflow with `version` = the output of `uv run python tools/atlas_sync.py next-version`,
  `upstream_tag` = the tag, `dry_run` unticked (do a dry run first if anything in the workflow changed).
- Close the issue by hand.
- After a release you may verify it independently: download the assets of the release (the API lists them:
  `https://api.github.com/repos/nezbut/horizon-atlas/releases/tags/launcher%40<version>`) into a NEW empty
  scratch directory, then run from the repository (never from inside the download directory)
  `cargo run --manifest-path REPO/Cargo.toml --locked --quiet -p horizon-release-sign -- verify --tag launcher@<version> <dir>`
  (exit 0 = signature valid and every file matches `SHA256SUMS`), and `( cd <dir> && sha256sum -c SHA256SUMS )`.
