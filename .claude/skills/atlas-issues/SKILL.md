---
name: atlas-issues
description: >-
  Horizon Atlas (nezbut/horizon-atlas, an unofficial fork of the Horizon launcher kept as a patch queue on upstream `launcher@X.Y.Z` tags) - explains how the project and its automation work, and how to handle anything that goes red or opens an issue. Use whenever someone pastes the text or a screenshot of a GitHub issue or an Actions run of this repository (titles like "Sync to launcher@0.2.56 failed: patch: choose-jar", the label sync-main, "Upstream changed the launcher's build in ..."), mentions atlas-sync / atlas-release / atlas-ci failing or a failed release, a new upstream launcher tag, a rebase conflict in one of Atlas's patches, an expired SYNC_TOKEN, or asks what happened with the daily sync - even if they never say "issue" or "skill".
---

# Atlas issues

Horizon Atlas keeps its changes to the Horizon launcher as a stack of small commits ("patches") on top of an
upstream release tag. A scheduled workflow (four times a day) replays that stack onto each new upstream tag. When that cannot be done
automatically it opens a GitHub issue. This skill is how to deal with such an issue, or with any other red run,
in a session that knows nothing about the project.

## The rule that governs everything

**Never commit and never push without the owner's explicit permission.** Diagnose, edit files, run the checks,
show the result and explain it; then ask whether to commit, and ask separately whether to push (a yes to one is
not a yes to the other). A rebase creates commits, so run it in a scratch clone outside the repository and offer
the result afterwards; commits inside your own scratch clone are your workspace and are fine.

## Workflow

1. **Read the whole issue, then classify.** The input is the issue's title and body (or a screenshot of them). The
   body states the reason in one sentence, links the failed run, and ends with a log tail; read all of it before
   anything else. The title has a fixed shape per kind, which is a quick key into `references/issue-types.md`.
   A red run with no issue (atlas-release, atlas-ci, `publish`) is also in that file. Actions logs cannot be downloaded without a login: if a screenshot is not enough, ask for
   the log text.
2. **Get oriented** (read `references/project-map.md` once if you do not know the project): `git status`,
   `git fetch origin`, compare `main` with `origin/main`, and check the newest upstream tag with
   `git ls-remote --tags https://github.com/horizon-rust/horizon 'refs/tags/launcher@*'`. If the owner's `main`
   differs from `origin/main`, say so before doing anything.
3. **Reproduce in a scratch clone** with the commands in `references/rebase-playbook.md`. Reproduce the failure
   first; a fix you have not seen fail and then pass is a guess.
4. **Fix with the smallest change that keeps Atlas's behaviour.** Take upstream's new code as the base and
   re-apply only Atlas's seam (a constant, a repository name, one condition). A wide patch conflicts again at the
   next tag. If a hypothesis about the cause is not proven, find the cause (a throwaway diagnostic workflow is a
   legitimate way) rather than skipping the failing test.
5. **Verify** with the full set in the playbook (sync-tool tests, `cargo fmt --check`, clippy `-D warnings`,
   launcher tests, release-sign tests, no conflict markers left, `atlas_sync.py base` prints the new tag).
6. **Report, then ask.** Say what the problem was (the cause chain, one paragraph), what you changed (the exact
   lines of the resolution, not just file names), what you verified and what you could not (Actions cannot be run
   from a scratch clone; Windows and macOS builds and the launcher window cannot be tested there), and what
   happens next. Then ask as separate questions: may I bring the result into the repository as a local branch
   `sync-<tag>`, and may it be committed? Pushing a rebased history needs `--force-with-lease`: give the exact command
   and explain what it does (it overwrites `main` only if GitHub's `main` is still what the clone last saw), and
   run it only if the owner said yes to pushing.
7. **After the push**, say what is left: `atlas-ci` should go green; if the sync no longer sees a newer tag, the
   Atlas release for this base must be started by hand (`atlas-release` with the version from
   `uv run python tools/atlas_sync.py next-version` and the upstream tag); close the issue by hand (nothing closes
   it); and after a release, the signature check in the playbook.

## Hard limits (never trade these for a green run)

- Atlas never downloads Minecraft's client or server jar, libraries or assets from Mojang for a player without an
  account that owns the game, in the launcher or in a side tool. Downloading stays gated on an owner exactly as in
  upstream. If a conflict resolution would drop that gate, stop and tell the owner.
- Signature checks stay on. `crates/horizon-release-sign/release-keys.txt` keeps both keys: upstream's (game
  builds are verified with the same list) and Atlas's. Do not remove, replace or "fix" either, and never ask for
  or print a private key.
- `SYNC_TOKEN` is read only by the `publish` job of `atlas-sync.yml`, which runs no code from the repository. Do not
  add it to any other job, step or environment: the other jobs run upstream's code and third-party crates.
- Do not skip, delete or weaken tests wholesale. One skipped test needs a diagnosed cause written next to it.
- Patch commit subjects are exact: `patch: offline-play`, `patch: choose-jar`, `patch: own-releases`,
  `patch: own-keys`, `patch: branding`; everything else starts with `atlas:`. The sync names the failing patch by
  its subject, so never rename them. Commits carry no `Co-Authored-By` or "Generated with" lines.
- No new code path that talks to Mojang for non-owners, and no telemetry.

## Reference files

- `references/project-map.md` - what Atlas is, how `main` is built, the files, workflows, secrets and trust
  boundaries, what runs on the player's machine. Read it first in a new session.
- `references/issue-types.md` - every issue the automation opens and every known red run: title, cause, what to do.
- `references/rebase-playbook.md` - the exact commands: scratch clone, rebase onto a new tag, resolving
  conflicts, verification, applying to the repository, publishing, releasing by hand, checking a release.
