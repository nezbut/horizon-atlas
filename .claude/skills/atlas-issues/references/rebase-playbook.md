# Playbook: reproduce, fix, verify, hand over

`REPO` below is the owner's clone (`git rev-parse --show-toplevel`). The routine commands are tasks in REPO's
`Taskfile.yml` (go-task; `task --list` shows them). None of them commits, pushes or touches the owner's data;
`task clone:sync` is the only one that changes REPO's branch, so ask before running it. Never work in `REPO`
itself until the owner has said yes.

Cargo builds are slow cold (about ten minutes for the launcher). Export one shared build folder outside the
repository before any task that builds, e.g. `export CARGO_TARGET_DIR=<a folder outside REPO>/target-shared`,
and run long tasks in the background.

## 1. Rehearse in a scratch clone
```
task upstream:fetch                                        # in REPO: upstream's launcher tags and main
task rehearse -- refs/upstream/tags/launcher@<new>         # or: task rehearse -- upstream/main (early warning)
```
`rehearse` clones what is committed in REPO into a new temporary folder (honours `TMPDIR`), fetches upstream
there, runs `atlas_sync.py rebase-onto <ref>` and prints `scratch clone: <path>` and either the new base or the
failing patch (`FAILED_PATCH=<subject>`, exit 1). Call that path `C`. The conflicts the sync resolves by itself
(upstream workflow files; `launcher/lang/*.json` where a patch only adds keys) are resolved here too.

`rehearse` starts from REPO's committed `main`. If REPO's `main` differs from GitHub's (step 2 of the workflow),
the issue was produced from GitHub's `main`: bring REPO up to date first (section 9), or clone GitHub directly:
```
git clone -q --no-hardlinks https://github.com/nezbut/horizon-atlas.git $C && git -C $C remote add upstream https://github.com/horizon-rust/horizon.git
git -C $C fetch -q --no-tags upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*' '+refs/heads/main:refs/remotes/upstream/main'
```

## 2. Replay by hand (when the rehearsal named a conflict)
The tool aborted and left `C` clean. Run the same rebase directly in `C`, on a branch named after the tag. Use
REPO's copy of the sync tool (`C`'s own copy is rewritten while the rebase runs). `rehearse` gave `C` a
throwaway commit identity (`atlas-rehearsal`); this replay becomes `main`, so first give `C` the owner's (the
`user.name` and `user.email` of REPO):
```
git -C $C config user.name "$(git -C REPO config user.name)"; git -C $C config user.email "$(git -C REPO config user.email)"
git -C $C switch -q -c sync-<tag>
git -C $C -c merge.conflictStyle=diff3 rebase --onto refs/upstream/tags/launcher@<new> refs/upstream/tags/launcher@<old base>
uv run python REPO/tools/atlas_sync.py --repo $C resolve    # at every stop: resolves what the sync resolves by itself
git -C $C diff --name-only --diff-filter=U                   # what is left for you in the commit it stopped at
```
`<old base>` is what `task status` printed as the base. For the early-warning issue the new base is
`refs/remotes/upstream/main`.

## 3. Resolve a conflict
- Markers: `grep -n '^<<<<<<<\|^=======\|^>>>>>>>' <file>`. During a rebase the part after `<<<<<<< HEAD` is
  **upstream's new code** (plus Atlas commits already replayed); the part before `>>>>>>> <sha> (<patch>)` is the
  Atlas patch being replayed.
- Keep upstream's new code and re-apply only Atlas's seam to it (`LAUNCHER_REPO`, one added condition, one call
  into Atlas's own module such as `crate::offline::session()`, one added key). Read the whole block first (Read
  with offset and limit), then replace it with Edit: old string = the exact block including the three marker
  lines, new string = the resolution. If upstream renamed or restructured a function, adapt Atlas's seam to the
  new shape instead of restoring the old shape.
- Resolve by editing the block, not by taking a whole side with `git checkout --ours/--theirs`: a whole side
  loses either upstream's change or Atlas's seam. In `launcher/lang/*.json` conflicts upstream's side holds the
  new translations and Atlas's side holds old English placeholders plus Atlas's keys: keep upstream's lines and
  add Atlas's key lines. Check each JSON file still parses (`uv run python -m json.tool <file> > /dev/null`).
- Then: `git -C $C add <files>` and `GIT_EDITOR=true git -C $C rebase --continue`. It stops again at the next
  conflicting commit; repeat until it says the rebase succeeded.
- When it has succeeded, drop the workflows upstream added since the old base, as the sync does after every
  rebase (a hand-run rebase skips it, and an upstream workflow then runs in Atlas's repository):
  `uv run python REPO/tools/atlas_sync.py --repo $C drop-workflows` (prints the names it removed in one
  `atlas: drop upstream workflows (...)` commit, or nothing).

## 4. Verify (all of it; paste the evidence into your report)
```
task -d $C check                                                                     # fmt, clippy -D warnings, launcher tests + owner_gate, sync tool tests, actionlint
git -C $C grep -n -E '^(<<<<<<<|>>>>>>>) ' -- . ':!docs'                              # nothing
uv run python REPO/tools/atlas_sync.py --repo $C base                                  # prints launcher@<new>
git -C $C log --format='%s' refs/upstream/tags/launcher@<new>..HEAD | grep -c '^patch: '   # 7
ls $C/.github/workflows                                                                # only atlas-ci.yml, atlas-release.yml, atlas-sync.yml
cargo test --manifest-path $C/Cargo.toml -p horizon-release-sign
```
Also check `git -C $C diff refs/upstream/tags/launcher@<new>..HEAD --stat -- crates/horizon-release-sign` shows
only the `own-keys` patch and that `release-keys.txt` still lists both keys.

## 5. Show the user what changed
For each conflicting patch show the resolved hunks (`git -C $C show <rebased commit> -- <files>`) next to what
upstream changed in the same place (`git -C $C diff refs/upstream/tags/launcher@<old>..refs/upstream/tags/launcher@<new> -- <file>`),
and say in one paragraph why the conflict happened. Then ask the questions from the workflow in SKILL.md.

## 6. Apply to the user's repository (only after a yes)
```
git -C REPO fetch origin                                    # origin/main must still be what you started from
git -C REPO fetch --no-tags $C sync-<tag>:sync-<tag>        # creates a local branch with the rebased stack; main is untouched
```
If `origin/main` moved since you started, stop and tell the user: the work must be redone on the new `main`.
`--no-tags` matters: by default a fetch also copies the tags that point into what it fetched, and a scratch clone
may still hold upstream's old `launcher@*` tags, which would then hide Atlas's own again (section 9).

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
- `atlas-ci` (started by the push) must go green: its Linux job and its `windows` job (launcher tests, a release
  build, and the installer's end-to-end script `tools/atlas_installer_e2e.ps1`).
- The scheduled sync will not release this base, because it no longer sees a newer tag. Release by hand: Actions ->
  `atlas-release` -> Run workflow with `version` = the next version `task status` prints, `upstream_tag` = the
  tag, `dry_run` unticked (do a dry run first if anything in the workflow changed).
- Close the issue by hand.
- After a release you may verify it independently: download the assets of the release (the API lists them:
  `https://api.github.com/repos/nezbut/horizon-atlas/releases/tags/launcher%40<version>`) into a NEW empty
  scratch directory, then run from the repository (never from inside the download directory)
  `cargo run --manifest-path REPO/Cargo.toml --locked --quiet -p horizon-release-sign -- verify --tag launcher@<version> <dir>`
  (exit 0 = signature valid and every file matches `SHA256SUMS`), and `( cd <dir> && sha256sum -c SHA256SUMS )`.

## 9. Bring the owner's clone up to date after a sync
Every successful sync (and every push from step 7) replaces `main` on GitHub with the same Atlas commits replayed
onto the new upstream tag, under new ids. A clone that still has the old stack then shows `main` both ahead and
behind (`git status -sb`: `[ahead 33, behind 788]`). Nothing is lost; the clone holds the old copies. Ask the owner
before running it, then in REPO:
```
task clone:sync
```
It fetches `origin`, refuses on a dirty working tree or a branch other than `main`, and lists the commits only
this clone has (`git range-diff -s main...origin/main`, the `<` lines).
- Nothing listed (the usual case): it runs `git reset --keep origin/main` itself (`--keep` refuses rather than
  drop uncommitted changes).
- Commits listed (the owner's local work on top of the old stack): it changes nothing and prints
  `git rebase --onto origin/main <first listed>^ main`, which moves exactly those commits onto the new `main`; run
  that only after the owner says yes.

Never run a plain `git pull` here: it merges the old stack with the new one, every Atlas commit then exists twice,
and once pushed the next sync conflicts with the duplicates.

A clone that was once cloned from upstream keeps upstream's `launcher@*` tags in `refs/tags`, and they come back
through any fetch that follows tags from upstream or from a clone that still has them (a plain `git fetch upstream`,
`git fetch --all`, an editor's background fetch, a branch brought in from a scratch clone without `--no-tags`).
Atlas's own releases use the same names (`launcher@0.2.0`...), so those old tags hide them: `git fetch --tags origin`
refuses to replace them (`would clobber existing tag`), and `task status` then prints a next version below the
newest release. With the owner's yes, drop only the local tags that are exactly upstream's (the same object as
under `refs/upstream/tags/`, which keeps them), stop the `upstream` remote from bringing tags, then fetch Atlas's:
```
for t in $(git -C REPO tag -l 'launcher@*'); do [ "$(git -C REPO rev-parse "refs/tags/$t")" = "$(git -C REPO rev-parse -q --verify "refs/upstream/tags/$t")" ] && git -C REPO tag -d "$t"; done
git -C REPO config remote.upstream.tagOpt --no-tags
git -C REPO fetch --tags origin
```
