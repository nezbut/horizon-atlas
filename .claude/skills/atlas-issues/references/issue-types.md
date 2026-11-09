# Issue and red-run catalogue

Titles below are exactly what the workflows write (`.github/workflows/atlas-sync.yml`). Match the title first.

## Issues opened by the automation

### `Sync to launcher@X.Y.Z failed: <patch subject>`  (e.g. `... failed: patch: choose-jar`)
- **Cause:** a rebase conflict. Upstream changed the same lines that the named patch touches. The run published
  nothing and `main` is unchanged; Atlas stays on its previous base until this is fixed. (The auto-resolve in
  `atlas_sync.py` only covers upstream workflow files, so this is always a real conflict.)
- **Do:** reproduce with `references/rebase-playbook.md` (scratch clone, rebase onto the tag in the title). The named
  patch is the first to conflict; later ones may too. Resolve each by keeping upstream's new code and re-applying
  only Atlas's seam. Typical hot spots: `launcher/src/backend.rs`, `cli.rs`, `main.rs`, `frame.rs`,
  `glass_account.rs`, `sources.rs`, `launcher/lang/*.json` (upstream translates strings that Atlas's commit
  touches nearby: keep upstream's lines and add only Atlas's key), `THIRD_PARTY_NOTICES.md`, `README.md`
  (keep both sides).
- **Known resolutions:** `launcher@0.2.54` conflicted in `offline-play` (upstream turned the refusal into a
  structured error; the fix was to keep upstream's code and replace the literal `false` with `OFFLINE_PLAY`),
  `branding` (lang files) and the notices file. `launcher@0.2.56` conflicted in `choose-jar`.

### `Sync to launcher@X.Y.Z failed: the rebase did not start or finish`
- **Cause:** the rebase failed without naming a conflicting patch (a missing ref, a dirty tree, a git error).
  The body ends with the tail of git's stderr.
- **Do:** read that tail, then reproduce with the same commands in a scratch clone. If a normal rebase works
  there, the failure was in the runner (fetch of the tags, a transient network error): tell the user to re-run
  `atlas-sync`.

### `Sync to launcher@X.Y.Z failed: cargo fmt or the tests`
- **Cause:** the patches applied cleanly but `cargo fmt --check` or `cargo test --release --lib --bins` failed
  in `launcher/` on the rebased tree. Usual reasons: upstream changed an API that an Atlas patch uses (compile
  error), upstream added user-visible strings and the language-consistency test wants the key in every
  `launcher/lang/*.json`, upstream changed a test that pins behaviour Atlas inverted on purpose (the owner-gate
  tests in `launcher/tests/owner_gate.rs` and tests around `OFFLINE_PLAY`), or formatting drift.
- **Do:** rebase in the scratch clone, run the failing command, read the failure, fix it in the patch that owns
  the code. Prefer amending the owning patch commit (so the next sync conflicts, if ever, on a meaningfully
  named patch); if that is not possible in this environment, add one `atlas: ...` fix commit and say so.

### `Atlas's patches do not apply to or build on upstream's main`  (label `sync-main`)
- **Cause:** the daily early-warning job rebased the patches onto upstream's current `main` and either the
  rebase conflicted or `cargo check --release` failed. Nothing is broken today: it will bite at the next
  upstream launcher tag.
- **Do:** reproduce (rebase onto `refs/remotes/upstream/main`), tell the user what will conflict and why, and
  whether a cheap fix exists now. Usually the right move is to wait for the next tag and resolve then; do not
  push anything for this issue alone. The user closes it when a later sync succeeds.

### `Upstream changed the launcher's build in launcher@X.Y.Z: compare atlas-release.yml`
- **Cause:** a notice, not a failure: between the old base and the new tag upstream changed
  `.github/workflows/launcher.yml`, `packaging/` or `tools/ci/sign_release.sh`. `atlas-release.yml` is a copy
  of `launcher.yml`, so those changes do not reach it. The sync itself carried on.
- **Do:** `git diff refs/upstream/tags/launcher@OLD refs/upstream/tags/launcher@NEW -- .github/workflows/launcher.yml`.
  Port what matters into `.github/workflows/atlas-release.yml` and keep Atlas's differences: the `meta` job and
  inputs (`version`, `upstream_tag`, `dry_run`), no `tools/release_train.py`, `macos-15` instead of upstream's
  self-hosted Mac mini, `--latest=true`, the macOS arm64 test skip, the `release` environment on `sign`.
  `packaging/` and `sign_release.sh` are upstream files and already updated by the rebase. Lint with
  `uvx --from actionlint-py actionlint .github/workflows/atlas-release.yml`.

## Red runs that open no issue (the user pastes a screenshot or the log)

- **atlas-release, `build macos` fails in the test `an_app_bundle_updates_in_place`:** known. On GitHub's macOS 15
  arm64 runner the test's stand-in binary (a copy of `/usr/bin/true`, universal x86_64 + arm64e, re-signed ad
  hoc) is killed (`Killed: 9`, exit 137); a binary built on the runner runs, and the whole sign / zip / unzip /
  verify round trip passes. Upstream runs its macOS jobs on its own Mac mini, never on GitHub's runner. Atlas
  skips that one test on the native arm64 job only (`matrix.name == 'macos'` in `atlas-release.yml`); the Intel
  job runs it under Rosetta. If a *different* test fails on macOS, diagnose before skipping.
- **atlas-release, `build windows-arm64` fails:** allowed. The release goes out without the ARM64 files and the
  run warns.
- **atlas-release, `sign` fails:** the secret `RELEASE_SIGNING_KEY` is missing from the environment `release`, the
  environment is not restricted correctly, or the run is not on `main` (then it is a dry run with a throwaway
  key and should not fail). The `release` job fails if the signature does not verify.
- **atlas-release failed after `publish` pushed `main`:** the next scheduled run finds no newer tag and does not
  retry. The user re-runs `atlas-release` with the version and tag shown in the sync run's summary.
- **`publish` fails on the push (`stale info` / lease):** `main` on GitHub changed while the run was in
  progress. Nothing was lost; the user re-runs `atlas-sync`.
- **Authentication error in `publish` or in a checkout that uses `SYNC_TOKEN`:** the token expired or lost a
  permission. The user creates a new fine-grained token and replaces the repository secret.
- **atlas-ci red on a push:** read which step. Usually fmt or a test; reproduce in a scratch clone.
- **A sync run is green but nothing was released:** expected when there was no newer upstream tag, or on a dry run.
  The summary says which ("New upstream launcher tag: none" / "Dry run: would push ...").
