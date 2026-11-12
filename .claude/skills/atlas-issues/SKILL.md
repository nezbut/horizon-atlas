---
name: atlas-issues
description: >-
  Horizon Atlas (nezbut/horizon-atlas, an unofficial fork of the Horizon launcher kept as a patch queue on upstream `launcher@X.Y.Z` tags) - explains how the project and its automation work, and how to handle anything that goes red or opens an issue. Use whenever someone pastes the text or a screenshot of a GitHub issue or an Actions run of this repository (titles like "Sync to launcher@0.2.56 failed: patch: choose-jar", the label sync-main, "Upstream changed the launcher's build in ..."), mentions atlas-sync / atlas-release / atlas-ci (its Linux or windows job, the installer end-to-end script) failing or a failed release, a new upstream launcher tag, a rebase conflict in one of Atlas's patches (offline, no-sign-in, own-install, choose-jar, own-releases, own-keys, branding), an expired SYNC_TOKEN, a clone that is ahead and behind origin after a sync, or asks what happened with the scheduled sync - even if they never say "issue" or "skill".
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
2. **Get oriented** (read `references/project-map.md` once if you do not know the project). The repository has a
   `Taskfile.yml` (go-task): `task --list` shows its commands, and they never commit or push. Run
   `task upstream:fetch` (upstream's launcher tags and `main`), `git fetch --tags origin` (Atlas's release tags
   too: a plain fetch skips a tag whose commit the clone already has), then `task status` (the current base, the
   newest upstream tag newer than it, Atlas's next version, `git status -sb`). If the next version is not the
   newest release on GitHub plus one, the clone holds upstream's old `launcher@*` tags in `refs/tags` (from
   cloning upstream once); they hide Atlas's own tags of the same name, so the release's version comes from
   GitHub's releases page instead (playbook section 9 says how to clean such a clone). If the owner's `main`
   differs from `origin/main`, say so before doing anything. Ahead and behind at once is normal after a
   successful sync (it rewrote `main` on GitHub): playbook section 9 (`task clone:sync`).
3. **Reproduce in a scratch clone**: `task rehearse -- refs/upstream/tags/launcher@<new>` (or `-- upstream/main`
   for the early-warning issue) clones the repository into a new temporary folder, replays the patches there and
   prints the scratch path and the result; the owner's clone is not touched. Resolving by hand then follows
   `references/rebase-playbook.md` in that scratch clone. Reproduce the failure first; a fix you have not seen
   fail and then pass is a guess.
4. **Fix with the smallest change that keeps Atlas's behaviour.** Take upstream's new code as the base and
   re-apply only Atlas's seam (a constant, a repository name, one condition, one call into Atlas's own module).
   Atlas's own code lives in its own files (`launcher/src/offline/`, `atlas_*.rs`, `installer_atlas.rs`), which
   upstream never touches; conflicts are in the seams. A wide patch conflicts again at the next tag. If a
   hypothesis about the cause is not proven, find the cause (a throwaway diagnostic workflow is a legitimate way)
   rather than skipping the failing test.
5. **Verify** with `task check` in the scratch clone (fmt, clippy `-D warnings`, the launcher's tests with
   `owner_gate`, the sync tool's tests, actionlint) plus the few checks it does not cover (playbook section 4: no
   conflict markers, the base is the new tag, seven patches, release-sign tests).
6. **Report, then ask.** Say what the problem was (the cause chain, one paragraph), what you changed (the exact
   lines of the resolution, not just file names), what you verified and what you could not (Actions cannot be run
   from a scratch clone; Windows and macOS builds and the launcher window cannot be tested there), and what
   happens next. Then ask as separate questions: may I bring the result into the repository as a local branch
   `sync-<tag>`, and may it be committed? Pushing a rebased history needs `--force-with-lease`: give the exact command
   and explain what it does (it overwrites `main` only if GitHub's `main` is still what the clone last saw), and
   run it only if the owner said yes to pushing.
7. **After the push**, say what is left: `atlas-ci` should go green; if the sync no longer sees a newer tag, the
   Atlas release for this base must be started by hand (`atlas-release` with the version `task status` shows as
   next and the upstream tag); close the issue by hand (nothing closes it); after a release, the signature check
   in the playbook; and the owner's clone needs `task clone:sync` (playbook section 9).

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
- Patch commit subjects are exact, seven of them, in this order: `patch: choose-jar`, `patch: own-releases`,
  `patch: branding`, `patch: own-keys`, `patch: offline`, `patch: no-sign-in`, `patch: own-install`; everything
  else starts with `atlas:`. The sync names the failing patch by its subject, so never rename them. Commits carry
  no `Co-Authored-By` or "Generated with" lines.
- Atlas's own data is `<data_dir>/atlas/` (its accounts, offline names, skins, capes). The data folder around it
  is shared with the official launcher on purpose: no resolution may make Atlas write the shared
  `accounts.json` or `skins/`, or let its uninstaller delete more than `atlas/`.
- The installer touches only Atlas's own install. An old install in the official launcher's folder counts as
  Atlas only when its exe contains `nezbut/horizon-atlas`; the official launcher's exe, shortcuts, registry keys
  and running processes are never removed or closed.
- No new code path that talks to Mojang for non-owners, and no telemetry.

## Reference files

- `references/project-map.md` - what Atlas is, how `main` is built, the files, workflows, secrets and trust
  boundaries, what runs on the player's machine. Read it first in a new session.
- `references/issue-types.md` - every issue the automation opens and every known red run: title, cause, what to do.
- `references/rebase-playbook.md` - the exact steps: rehearsal in a scratch clone, resolving conflicts by hand,
  verification, applying to the repository, publishing, releasing by hand, checking a release, bringing the
  owner's clone up to date after a sync.
