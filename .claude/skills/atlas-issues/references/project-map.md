# Project map

## What Atlas is
`nezbut/horizon-atlas` is an unofficial fork of the **launcher** of Horizon (`horizon-rust/horizon`, Minecraft
Java 26.3 rewritten in Rust). It changes the launcher (`launcher/`) only; the game is Horizon's, unmodified, and
the launcher installs it from Horizon's own releases (Nightly). Atlas plays offline without a Microsoft account
(several offline names, each with its own skin and cape, on a "Profile" tab), lets the player pick their own
`26.3.jar`, installs beside the official launcher instead of over it, and updates itself from this repository's
releases, signed with Atlas's own key. Microsoft sign-in is not offered (Atlas has no Azure client ID of its own;
Horizon's may not be used): its buttons open a dialog that says so.

## What `main` is
Upstream's commit for the tag `launcher@X.Y.Z` (the current *base*) plus a linear stack of Atlas commits:
- seven **patches**, by exact subject, in history order:
  - `patch: choose-jar`: `sources::from_jar`, `Settings::minecraft_jar`, the "Choose jar..." row, CLI
    `settings minecraft-jar`. Accepts only the pinned 26.3 release jar (sha1 from `research-lock.json`); the jar
    needs the `libraries` and `assets` of a launcher install next to it or in another launcher.
  - `patch: own-releases`: `backend::LAUNCHER_REPO = "nezbut/horizon-atlas"`, the Atom-feed fallback
    (`launcher_feed`, `feed_releases`), `Updates::launcher_error` (a failed check is shown, never "up to date"),
    `signed::asset_url` reads a launcher release's checksums from Atlas's repository.
  - `patch: branding`: a sidebar link to this repository (`frame.rs`, one lang key in every `launcher/lang/*.json`),
    the window title "Horizon Atlas" (`titlebar.rs`) and the macOS app menu "Horizon Atlas Launcher" (`main.rs`).
  - `patch: own-keys`: Atlas's public key in `crates/horizon-release-sign/release-keys.txt` beside upstream's.
  - `patch: offline`: Atlas's own module `launcher/src/offline/` (offline names and their skins and capes, the
    one-time migration from Atlas 0.1.x, the game's offline session, the `horizon-launcher offline ...` CLI),
    the window's `atlas_ui.rs` (who plays, the account menu's offline rows, Atlas's dialogs) and
    `atlas_profile.rs` (the Profile tab); seams in `account.rs` (Atlas's own accounts file and keychain service;
    first line of `session_for_game`), `main.rs`, `switch_page.rs`, `frame.rs`, `owner_page.rs`,
    `servers_page.rs`, `lib.rs`; Atlas's `launcher/tests/owner_gate.rs`; its keys in every lang file.
  - `patch: no-sign-in`: without a client ID the Microsoft sign-in opens Atlas's dialog (seam at the top of
    `begin_sign_in` in `main.rs`, the dialog in `atlas_ui.rs`), and the glass top bar leaves out its Sign in with
    Microsoft button (one line in `frame.rs`), so the eight tabs fit at the default window width.
  - `patch: own-install`: Windows installs as "Horizon Atlas Launcher" (`installer.rs` constants, keys
    `...\Uninstall\HorizonAtlas` and `Software\Horizon\Atlas`, folder `%LOCALAPPDATA%\Programs\Horizon Atlas Launcher`),
    the option to remove an old Atlas copy from the official launcher's folder (`installer_atlas.rs`,
    `atlas_install.rs`, `setup_page.rs`), the one-time "Atlas now installs on its own" dialog, the uninstaller's
    data switch limited to `atlas/`; Linux menu entry `horizon-atlas.desktop` ("Horizon Atlas", `desktop.rs`),
    moved from the old entry by itself; `packaging/linux/appimage.sh`; its keys in every lang file. Upstream's
    Polish test wants every Polish key used, so the upstream installer texts Atlas replaced stay as literals in a
    `const _` in `atlas_install.rs` (`pl.json` keeps them; the patch only adds lang keys).
- `atlas:` commits: `tools/atlas_sync.py` and its tests, `Taskfile.yml`, the three workflows,
  `tools/atlas_installer_e2e.ps1`, `install.sh`, README, AGENTS.md, CODEOWNERS, THIRD_PARTY_NOTICES,
  `docs/superpowers/` (specs, plans, roadmap, owner checklist), this skill.
`git log launcher@<base>..main` lists exactly how Atlas differs from upstream (the upstream tag lives in the
`refs/upstream/tags/` namespace, see below).

## Why issues exist
Upstream releases a launcher tag several times a day and changes the same files Atlas's patches touch. Replaying
the patches onto a new tag is a `git rebase --onto`; it works unless upstream changed the same lines. Then a human
has to resolve the conflict. The automation never guesses: it stops, publishes nothing, leaves `main` as it was,
and opens an issue that names the cause.

## Tools and workflows
- `Taskfile.yml` (go-task; `task --list`): `check` (fmt, clippy `-D warnings`, launcher tests with `owner_gate`,
  sync tool tests, actionlint; stops at the first failure), `test`, `test:sync`, `lint:workflows`, `build`,
  `status` (base, newer upstream tag, next version, `git status -sb`), `upstream:fetch`, `rehearse -- <ref>`
  (replays the patches onto `<ref>` in a fresh scratch clone), `clone:sync` (brings `main` up to date after a
  sync; changes nothing when the clone has commits of its own), `run:scratch` / `run:copy` (the built launcher on
  scratch folders; `run:copy` starts with a copy of the real `accounts.json` and `skins/` to try the 0.1.x
  migration). No task commits or pushes. Builds honour `CARGO_TARGET_DIR`.
- `tools/atlas_sync.py` (tests: `tests/test_atlas_sync.py`, run with `uv run python -m unittest discover -s tests
  -p 'test_atlas*.py'`): `new-tag` (newest upstream `launcher@N.N.N` newer than the base, else prints nothing),
  `base` (the newest upstream tag that is an ancestor of HEAD), `next-version` (Atlas's next version: the highest
  own `launcher@*` tag, patch +1, first `0.1.0`), `rebase TAG` (`git rebase --onto`; resolves by itself the
  conflicts that need no human: an upstream workflow file gets the Atlas commit's version when both sides
  changed it and is deleted when either side deleted it, and a `launcher/lang/*.json` where the patch only adds keys gets upstream's file plus those keys; any other
  conflict aborts cleanly and prints `FAILED_PATCH=<subject>`; on success each resolved file goes to stderr as
  `resolved: <path> in "<commit>" (<how>)`, which the sync run's summary lists), `rebase-onto REF` (the same onto any ref, e.g.
  `upstream/main`, used by `main-check`), `resolve` (that resolution for the commit a hand-run rebase stopped on),
  `drop-workflows`, `title ATLAS UPSTREAM`.
- `.github/workflows/atlas-sync.yml`: four times a day (04:17, 10:17, 16:17 and 22:17 UTC) and by hand (input `dry_run`, default true); `main-check` runs only on the 04:17 run and by hand.
  Jobs: `build` (no secrets; fetch, find tag, rebase, fmt, tests, bundle), `publish` (the only job with
  `SYNC_TOKEN`; pushes `main` with a lease and starts `atlas-release`; skipped on dry run), `report` (opens the
  issue with the default token), `main-check` (rebases onto upstream `main` and runs `cargo check`, never
  publishes), `report-main`.
- `.github/workflows/atlas-release.yml`: inputs `version`, `upstream_tag`, `dry_run`. `meta`, `build` (Linux,
  macOS arm64 and Intel, Windows x64 and ARM64; Windows ARM64 is allowed to fail), `sign` (the only job with the
  signing key, environment `release`, restricted to `main`; a dry run signs with a throwaway key), `release`
  (verifies the signature as launchers do, then `gh release create`, tag `launcher@<version>`, marked latest).
  It is a copy of upstream's `launcher.yml`, so upstream changes to that file do not reach it by themselves.
  Atlas's own step after `package.sh` renames the macOS app to `Horizon Atlas.app` (bundle id
  `io.github.nezbut.horizon-atlas`), signs it again and rebuilds the zip and `.dmg`; upstream's `packaging/macos/`
  is not patched.
- `.github/workflows/atlas-ci.yml`, for every push and pull request: `launcher` (Linux: fmt, launcher tests,
  `owner_gate`) and `windows` (launcher tests, a release build, then `tools/atlas_installer_e2e.ps1`, which
  installs, removes and uninstalls in a scratch profile and checks that the official launcher's install, keys
  and the shared data folder are never touched).
- Atlas's versions are its own (`0.1.0`, `0.1.1`...). The version and tag reach the build as
  `HORIZON_LAUNCHER_VERSION` / `HORIZON_LAUNCHER_TAG`; `launcher/Cargo.toml` is never edited.

## Git facts that matter
- Upstream's tags are copied to the **local** namespace `refs/upstream/tags/launcher@*` by
  `git fetch --no-tags upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*'`. They are never pushed
  and never mixed with Atlas's own `launcher@0.1.x` release tags. Every runner and every clone fetches them again.
- Nothing stores the "current base": it is computed from the commit graph (the highest upstream tag that is an
  ancestor of `main`). After a rebase and a push the next run knows the new base by itself.
- Rebasing rewrites the commits of the stack, so pushing the result to `main` needs `--force-with-lease`.
- For the same reason, after every successful sync a clone's `main` is both ahead of and behind `origin/main`.
  That is expected; `references/rebase-playbook.md` section 9 brings the clone up to date (never a plain `git pull`).

## Secrets and who reads them
- `RELEASE_SIGNING_KEY`: Ed25519 private key (base64 PKCS#8), in the environment `release`, read only by the
  `sign` job. The public key (32 raw bytes, base64) is in `release-keys.txt`.
- `SYNC_TOKEN`: a fine-grained personal access token (Contents, Workflows, Actions, Issues: read and write, only
  this repository), a repository secret, read only by `publish`. It expires (at most a year): an authentication
  error in `publish` or in a checkout that uses it means it lapsed; the user makes a new one (steps are in
  `docs/superpowers/owner-checklist.md`, section 4).

## On the player's machine
- Linux (`install.sh`): the launcher is one file, `~/.local/share/horizon-atlas/horizon-atlas` (a symlink in
  `~/.local/bin`), with its own menu entry `horizon-atlas.desktop`. Self-update downloads the release archive into
  `~/.local/share/horizon-launcher/.launcher-update/<pid>-<nonce>/`, checks the signed SHA-256 (keys: upstream's
  and Atlas's), unpacks, checks again, copies the new binary next to the old one and renames it over it. Game
  builds update separately from Horizon's releases.
- Windows: `%LOCALAPPDATA%\Programs\Horizon Atlas Launcher\horizon-launcher.exe`, Start menu and Apps entry
  "Horizon Atlas Launcher". Atlas 0.1.x installed into the official launcher's folder
  (`...\Programs\Horizon Launcher`); a copy still running there shows a one-time dialog pointing to the new
  installer. macOS: `Horizon Atlas.app` (`install.sh` puts it in `~/Applications/Horizon Atlas/`); a self-update
  keeps the bundle where it is, so a 0.1.x `Horizon.app` keeps its name until reinstalled.
- Data: the data folder (`~/.local/share/horizon-launcher`, `%APPDATA%\horizon-launcher`, ...: settings,
  installed game builds, servers) is shared with the official launcher on purpose. Atlas's own data is its
  subfolder `atlas/`: `accounts.json` and `account-secrets.json` (Atlas's own accounts, none in 0.2.0),
  `offline.json` (offline names, their skin models, who plays), `skins/<name>.png` and `<name>.cape.png`,
  `windows-move-dismissed`. On the first start of 0.2.0 the names, selection and skins of Atlas 0.1.x are copied
  from the shared `accounts.json` and `skins/` (read only, once: `offline.json`'s existence marks it). Saves live in
  `~/.local/share/horizon` (the game's folder).
- The launcher asks `api.github.com/repos/nezbut/horizon-atlas/releases`, and the repository's `releases.atom` if
  the API refuses; a failed check is shown in the window.

## Where the reasoning is written down
`docs/superpowers/specs/2026-11-08-atlas-launcher-fork-design.md` (design and non-goals),
`docs/superpowers/plans/2026-11-08-atlas-launcher-fork.md` (what was built, with an "As built" section),
`docs/superpowers/specs/2026-11-11-atlas-0.2.0-offline-module-design.md` and
`docs/superpowers/plans/2026-11-11-atlas-0.2.0-offline-module.md` (0.2.0: the offline module, the separate install),
`docs/superpowers/owner-checklist.md` (what only the owner can do, and why), `docs/superpowers/ROADMAP.md`.
