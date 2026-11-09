# Horizon Atlas: design

Status: draft for approval. Date: 2026-11-08. Repository: `nezbut/horizon-atlas`, a GitHub fork of
`horizon-rust/horizon` (main only, public, MIT).

## Intent

An unofficial fork of the Horizon **launcher** that the owner controls. It always tracks upstream's
latest launcher release, plays offline without a Microsoft account, and lets the player pick their own
`26.3.jar`. The game itself is not forked: the launcher installs Horizon's game builds from upstream
releases, unchanged.

## Goals

1. Follow upstream automatically: every new upstream launcher release becomes an Atlas release, with
   no hand work when the patches still apply.
2. Offline play with no Microsoft account, in the window and on the CLI.
3. "Choose jar…": the player points the launcher at their own `26.3.jar` (checked against the sha1 in
   `research-lock.json`), from anywhere on disk, besides the folders it searches by itself.
4. Our own releases and our own update chain: signed with our Ed25519 key, self-update reads only
   our repository.
5. Installers on every platform upstream supports (Windows x64 and ARM64, Linux x86-64, macOS arm64
   and x64), plus an `install.sh` for Linux/macOS.
6. A public README that says what the fork is and what it is for.

## Non-goals

- Forking the game: no game patches for now (offline skins and capes already exist in upstream; see ROADMAP).
- Any feature that fetches Minecraft's client or server jar from Mojang for a player who has no
  license, in the launcher or in a companion tool. Downloading the game from Mojang stays gated on an
  owner exactly as upstream has it.
- Other Minecraft versions: Horizon reproduces 26.3 only.
- Telemetry, anticheat, mod registry hosting or anything else upstream runs as a service.

## Anti-goals (a failure even if it works)

- Unsigned self-updates (horizon-free disables the signature check; Atlas must not).
- A patch set so wide that most upstream releases need manual merging.
- Presenting the fork as official, or implying affiliation with Mojang, Microsoft or the Horizon authors.

## Constraints

- Upstream tags launcher releases `launcher@X.Y.Z` (currently 0.2.50) and moves `main` many commits
  a day. The fork was created main-only, so it has no upstream tags until an `upstream` remote is added.
- Upstream already has: offline names (`account::add_offline`, `valid_offline_name`), the window's
  offline-name chips with per-name skins (`glass_account.rs`), and `Settings::dev_offline` which lets
  an offline name play without an owner. Atlas switches this on instead of rewriting it.
- Upstream builds all platform installers in `.github/workflows/launcher.yml` (the Windows
  `horizon-launcher-setup.exe` is the launcher itself run with `setup`; Linux AppImage and tar.gz;
  macOS `.dmg`). Atlas reuses that, no separate Inno Setup script.
- Signature trust: `crates/horizon-release-sign` embeds public keys from `release-keys.txt`, and the
  same list (`trusted_keys()` in `signed_digest`) verifies both the launcher's own updates and the
  game builds, which stay signed by upstream. Replacing the key would break every game install, so
  Atlas's list holds upstream's key and Atlas's.
- The launcher's version and tag are injected at build time (`HORIZON_LAUNCHER_VERSION`,
  `HORIZON_LAUNCHER_TAG`), not read from `Cargo.toml`.
- Microsoft sign-in needs an Azure app client ID (`HORIZON_MSA_CLIENT_ID`), which is Horizon's own and
  not Atlas's; builds without it say they have no sign-in. Upstream's macOS jobs run on its own
  self-hosted runner.
- The user's rules: no auto-commit, `bun`/`uv` for JS/Python, edits through the Edit tool.

## Approaches considered

1. **Merge upstream on a schedule.** Simple, but history fills with merges and conflicts pile up in
   `account.rs` and `main.rs`, which upstream changes constantly.
2. **Patch queue.** Atlas's changes are small, named commits on top of upstream; CI rebases them onto
   the new upstream release. A conflict names the exact patch.
3. **Narrow seams on top of 2.** Where possible a patch changes one line in upstream code (a constant
   such as `backend::OFFLINE_PLAY`) and puts new logic in new files.

**Chosen: 2 + 3** (the owner's pick). Reference: `0CYBER/horizon-free` does the same by hand with
`fork-sync.md`; Atlas automates it and keeps signing.

## Design

### Patch set (on top of upstream's `launcher@X.Y.Z`)

Each is one commit with one concern:

1. `offline-play`: a constant (as in horizon-free, `OFFLINE_PLAY = true`) makes `Settings::load()` and
   `client_as()` use offline sessions; the CLI's `play`, `account offline NAME`, `account select NAME`
   and the MCP server stop requiring an owner; the Microsoft sign-in plates are hidden. Atlas has no
   Microsoft sign-in at all (my call): it has no client ID, so a card that cannot work would only
   mislead. Consequence: no online-mode servers; the README says so. Registering Atlas's own Azure
   app is a roadmap item.
2. `choose-jar`: `Settings::minecraft_jar`, `sources::from_jar`, a "Choose jar…" row on Home,
   `horizon-launcher settings minecraft-jar PATH`. Ported from `Horizon-Offline-Launcher` (its
   `sources.rs` differs from upstream's by about 85 lines).
3. `own-releases`: `backend::LAUNCHER_REPO` points at the Atlas repository; the release-feed fallback
   (Atom) for rate-limited addresses; failed update checks are reported, not shown as "up to date".
   The game's own releases still come from `horizon-rust/horizon` (`backend::REPO`).
4. `own-keys`: `release-keys.txt` lists upstream's key and Atlas's (my call, see Constraints); Atlas's
   launcher assets are signed with Atlas's key and the self-update check stays on.
5. `branding`: names, links and the README warning block.

Patches live as commits on the Atlas `main`; a `UPSTREAM` file records the upstream tag they sit on.
The first step resets Atlas `main` to upstream's latest `launcher@*` tag and applies the patches:
the fork has nothing of its own yet to lose (my call).

### Sync pipeline (GitHub Actions)

- **Four times a day, on a new upstream tag `launcher@*`:** fetch the tag, rebase the patches onto it, run
  `cargo fmt`, `cargo check` and `cargo test --lib --bins` in `launcher/`, bump the Atlas version,
  build all platforms, sign, publish. If a patch does not apply or tests fail, publish nothing and
  open an issue naming the patch.
- **Once a day, on upstream `main`:** same rebase and `cargo check` only, no publishing; opens an issue on
  failure, as an early warning.
- Upstream's own workflows are removed; Atlas keeps its own: launcher build/release, sync, and a
  trimmed CI for `launcher/` (fmt, clippy, tests).
- Publishing the rebased `main` (force-push or a separate release branch) is left to the builder,
  see "Left to the builder".

### Versioning

Atlas has its own semver, independent of upstream's: patch = a new upstream sync or a fix, minor = a
new Atlas feature, major = a breaking change. Start at `0.1.0` (the owner's call: the first
releases are for testing; `1.0.0` comes when the pipeline has proven itself). The upstream base is in the release
title ("Atlas 0.1.0 · based on Horizon launcher 0.2.50"), in `UPSTREAM`, and optionally in About.
The version is injected at build time, so `Cargo.toml` is never touched and rebases do not conflict
on it. Tags keep the form `launcher@X.Y.Z` (for example `launcher@0.1.0`), which the launcher's tag
parsing expects; they live in the Atlas repository, so they cannot clash with upstream's (my call).

### Signing

One Ed25519 key pair for Atlas. The private key is the repository secret `RELEASE_SIGNING_KEY` (in a
`release` environment); the public key is in `release-keys.txt`. `tools/ci/sign_release.sh` signs
`SHA256SUMS`; the launcher refuses an update whose signature does not verify. Loss of the key means
releasing a new launcher by hand that carries the new public key; the key is backed up offline by the
owner.

### Distribution

GitHub Releases, `install.sh` (Linux/macOS, adapted from Offline-Launcher's) and the platform
installers upstream builds. Windows shows SmartScreen's warning (no code-signing certificate); the
README says so. macOS ships unsigned (upstream signs only when the `MACOS_SIGN_IDENTITY` variable
is set); the README explains how to open it. Atlas builds macOS on GitHub-hosted runners, not
upstream's self-hosted one.

## Scope of the plan

The plan ends with the patches and the pipeline built and verified locally, plus a checklist of what
only the owner can do in GitHub: create the key pair, add `RELEASE_SIGNING_KEY` to a `release`
environment, enable Issues and Actions, push, and publish the first release. Atlas's attribution of
ported code (from `Horizon-Offline-Launcher`, MIT) goes into `THIRD_PARTY_NOTICES.md` (my call).

## Left to the builder

- Which Mojang-derived inputs "Prepare" needs besides the client jar (upstream's install flow
  handles a server jar too), and how `from_jar` satisfies them in `Horizon-Offline-Launcher`. Any
  input the player cannot supply themselves stays gated on an owner; Atlas adds no download path.
- Whether upstream guarantees a launcher tag works with newer game builds; if not, a compatibility
  check in the sync job.

- Exact tag/branch layout for rebased `main` and what users who cloned see (force-push vs a release
  branch).
- The CI details: runner types, caching, which upstream jobs to keep in trimmed form.
- Whether the Windows ARM64 job is optional like in horizon-free.
- Wording of README and of the "Choose jar…" row, and its i18n keys (upstream requires `tr()`).
- Checking how Horizon derives an offline player's UUID: if it comes from the name, say in the
  interface that changing the name starts a new player in saves.

## Roadmap (docs/superpowers/ROADMAP.md)

- Landing page on GitHub Pages for easy access to the fork.
- Check offline skins and capes, which upstream already has; patch the game only if something does
  not work.
- More of the owner's own features.
