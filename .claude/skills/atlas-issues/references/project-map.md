# Project map

## What Atlas is
`nezbut/horizon-atlas` is an unofficial fork of the **launcher** of Horizon (`horizon-rust/horizon`, Minecraft
Java 26.3 rewritten in Rust). It changes the launcher (`launcher/`) only; the game is Horizon's, unmodified, and
the launcher installs it from Horizon's own releases (Nightly). Atlas plays offline without a Microsoft account,
lets the player pick their own `26.3.jar`, and updates itself from this repository's releases, signed with Atlas's
own key.

## What `main` is
Upstream's commit for the tag `launcher@X.Y.Z` (the current *base*) plus a linear stack of Atlas commits:
- five **patches**, by exact subject:
  - `patch: offline-play`: `backend::OFFLINE_PLAY = true` (`Settings::load` forces `dev_offline`, `client_as`
    passes it to `session_for_game`), the CLI and MCP stop requiring an owner, Microsoft sign-in plates hidden.
  - `patch: choose-jar`: `sources::from_jar`, `Settings::minecraft_jar`, the "Choose jar..." row, CLI
    `settings minecraft-jar`. Accepts only the pinned 26.3 release jar (sha1 from `research-lock.json`); the jar
    needs the `libraries` and `assets` of a launcher install next to it or in another launcher.
  - `patch: own-releases`: `backend::LAUNCHER_REPO = "nezbut/horizon-atlas"`, the Atom-feed fallback
    (`launcher_feed`, `feed_releases`), `Updates::launcher_error` (a failed check is shown, never "up to date"),
    `signed::asset_url` reads a launcher release's checksums from Atlas's repository.
  - `patch: own-keys`: Atlas's public key in `crates/horizon-release-sign/release-keys.txt` beside upstream's.
  - `patch: branding`: a sidebar link to this repository (`frame.rs`, one lang key in every `launcher/lang/*.json`).
- `atlas:` commits: `tools/atlas_sync.py` and its tests, the three workflows, `install.sh`, README, AGENTS.md,
  CODEOWNERS, THIRD_PARTY_NOTICES, `docs/superpowers/` (spec, plan, roadmap, owner checklist), this skill.
`git log launcher@<base>..main` lists exactly how Atlas differs from upstream (the upstream tag lives in the
`refs/upstream/tags/` namespace, see below).

## Why issues exist
Upstream releases a launcher tag several times a week and changes the same files Atlas's patches touch. Replaying
the patches onto a new tag is a `git rebase --onto`; it works unless upstream changed the same lines. Then a human
has to resolve the conflict. The automation never guesses: it stops, publishes nothing, leaves `main` as it was,
and opens an issue that names the cause.

## Tools and workflows
- `tools/atlas_sync.py` (tests: `tests/test_atlas_sync.py`, run with `uv run python -m unittest discover -s tests
  -p 'test_atlas*.py'`): `new-tag` (newest upstream `launcher@N.N.N` newer than the base, else prints nothing),
  `base` (the newest upstream tag that is an ancestor of HEAD), `next-version` (Atlas's next version: the highest
  own `launcher@*` tag, patch +1, first `0.1.0`), `rebase TAG` (`git rebase --onto`; resolves by itself the
  conflicts that need no human: an upstream workflow file gets the Atlas commit's version when both sides
  changed it and is deleted when either side deleted it, and a `launcher/lang/*.json` where the patch only adds keys gets upstream's file plus those keys; any other
  conflict aborts cleanly and prints `FAILED_PATCH=<subject>`), `rebase-onto REF` (the same onto any ref, e.g.
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
- `.github/workflows/atlas-ci.yml`: fmt and launcher tests on Linux for every push and pull request.
- Atlas's versions are its own (`0.1.0`, `0.1.1`...). The version and tag reach the build as
  `HORIZON_LAUNCHER_VERSION` / `HORIZON_LAUNCHER_TAG`; `launcher/Cargo.toml` is never edited.

## Git facts that matter
- Upstream's tags are copied to the **local** namespace `refs/upstream/tags/launcher@*` by
  `git fetch --no-tags upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*'`. They are never pushed
  and never mixed with Atlas's own `launcher@0.1.x` release tags. Every runner and every clone fetches them again.
- Nothing stores the "current base": it is computed from the commit graph (the highest upstream tag that is an
  ancestor of `main`). After a rebase and a push the next run knows the new base by itself.
- Rebasing rewrites the commits of the stack, so pushing the result to `main` needs `--force-with-lease`.

## Secrets and who reads them
- `RELEASE_SIGNING_KEY`: Ed25519 private key (base64 PKCS#8), in the environment `release`, read only by the
  `sign` job. The public key (32 raw bytes, base64) is in `release-keys.txt`.
- `SYNC_TOKEN`: a fine-grained personal access token (Contents, Workflows, Actions, Issues: read and write, only
  this repository), a repository secret, read only by `publish`. It expires (at most a year): an authentication
  error in `publish` or in a checkout that uses it means it lapsed; the user makes a new one (steps are in
  `docs/superpowers/owner-checklist.md`, section 4).

## On the player's machine (Linux install via `install.sh`)
- The launcher is one file, `~/.local/share/horizon-atlas/horizon-atlas` (a symlink in `~/.local/bin`).
  Self-update downloads the release archive into `~/.local/share/horizon-launcher/.launcher-update/<pid>-<nonce>/`,
  checks the signed SHA-256 (keys: upstream's and Atlas's), unpacks, checks again, copies the new binary next to
  the old one and renames it over it. Data (`~/.local/share/horizon`, `~/.local/share/horizon-launcher`: saves,
  settings, accounts, installed game builds) is not touched. Game builds update separately from Horizon's releases.
- The launcher asks `api.github.com/repos/nezbut/horizon-atlas/releases`, and the repository's `releases.atom` if
  the API refuses; a failed check is shown in the window.

## Where the reasoning is written down
`docs/superpowers/specs/2026-11-08-atlas-launcher-fork-design.md` (design and non-goals),
`docs/superpowers/plans/2026-11-08-atlas-launcher-fork.md` (what was built, with an "As built" section),
`docs/superpowers/owner-checklist.md` (what only the owner can do, and why), `docs/superpowers/ROADMAP.md`.
