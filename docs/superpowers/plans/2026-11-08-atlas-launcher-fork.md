# Horizon Atlas Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `nezbut/horizon-atlas` into an unofficial Horizon launcher fork that plays offline without a Microsoft account, lets the player pick their own `26.3.jar`, and republishes itself, signed, whenever upstream releases a launcher.

**Architecture:** Atlas's changes are small named commits ("patches") on top of an upstream `launcher@X.Y.Z` tag. A Python tool (`tools/atlas_sync.py`) finds new upstream tags and rebases the patches onto them; GitHub Actions run it daily, test, and call a trimmed release workflow that builds, signs and publishes. Upstream tags live in `refs/upstream/tags/` so they never mix with Atlas's own `launcher@X.Y.Z` release tags.

**Tech Stack:** Rust (the GPUI launcher in `launcher/`), Python 3 stdlib + `unittest` run with `uv run python` (the sync tool), GitHub Actions, bash/OpenSSL (`tools/ci/sign_release.sh`, unchanged).

**Spec:** `docs/superpowers/specs/2026-11-08-atlas-launcher-fork-design.md`

## Global Constraints

- Atlas versions are independent semver, first release `0.1.0`; tags are `launcher@X.Y.Z` in this repo; the version and tag reach the build only through `HORIZON_LAUNCHER_VERSION` / `HORIZON_LAUNCHER_TAG` (never edit `launcher/Cargo.toml`'s version).
- No code path may fetch Minecraft's client or server jar from Mojang for a player without an owner; downloading from Mojang stays owner-gated exactly as upstream has it.
- Signature checks stay on. `release-keys.txt` lists upstream's key `MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw=` and Atlas's; never replace upstream's (game builds are verified with the same list).
- Atlas has no Microsoft sign-in UI (no client ID); online-mode servers are unsupported and the README says so.
- Visible launcher strings go through `tr("...")` / `trf(...)` from `crate::i18n`.
- Patch commit subjects are exactly `patch: offline-play`, `patch: choose-jar`, `patch: own-releases`, `patch: own-keys`, `patch: branding`; every other commit starts with `atlas:`. Commits carry no agent attribution lines.
- Python only via `uv run python ...`; edit files with the Edit tool, new files with Write. **The owner's rule: never commit on your own.** Every "Commit" step is run only after the owner authorizes commits for this plan; until then leave the work staged.
- Upstream references to reuse (read, don't copy blindly): `../horizon-free` (offline switch and release-feed fallback: `git -C ../horizon-free diff 41f822b65 HEAD -- launcher/src`) and `../Horizon-Offline-Launcher` (`from_jar`, "Choose jar…", `install.sh`; MIT).

## Deviations from the spec (confirm at review)

1. The spec's `UPSTREAM` file is dropped: a file changed by every sync would conflict on every rebase. The base is derived from git refs (`current_base`) and written into the release title and notes.
2. A `SYNC_TOKEN` repository secret (fine-grained PAT: contents, workflows, actions, issues — read/write) is required: the default `GITHUB_TOKEN` cannot push commits that change `.github/workflows/` or trigger another workflow.

## Review Focus

1. Upstream tag list holds old-style (`launcher-v0.16.0`), pre-release (`launcher@0.2.51-rc1`) or non-launcher tags: they must be ignored, and `0.10.0` must sort above `0.2.50` (Task 2).
2. A patch conflicts with the new upstream tag: nothing is published, the repo is left clean (not mid-rebase), and the issue names the patch (Task 2).
3. `settings.json` says `dev_offline: false`, is empty or corrupt: play is still offline (Task 3).
4. The picked jar is missing, is another Minecraft version's jar, or sits in a bare folder (Downloads): a clear error, never a panic or a silent success (Task 4).
5. GitHub's anonymous API is rate-limited or unreachable: the launcher falls back to the release feed, and when both fail it reports the failure instead of saying "up to date" (Task 5).

---

### Task 1: Base the repository on upstream's latest launcher tag

**Files:** Modify: git refs only (no tracked file edits).

**Interfaces:** Produces: remote `upstream`; refs `refs/upstream/tags/launcher@*`; `main` = latest upstream launcher tag (the docs already written stay untracked/uncommitted meanwhile).

- [x] **Step 1:** `git remote add upstream https://github.com/horizon-rust/horizon.git`, then `git fetch upstream '+refs/tags/launcher@*:refs/upstream/tags/launcher@*'`. Run `git for-each-ref refs/upstream/tags | wc -l`. Expected: dozens of refs, including `launcher@0.2.50` or newer.
- [x] **Step 2:** Pick the numerically newest tag (the sync tool does this in Task 2; until then `git for-each-ref --sort=-version:refname refs/upstream/tags/launcher@* --count=1`). The docs commit `atlas: design, plan and roadmap` (made before the plan starts) is the only Atlas commit on `main`; move it onto the tag: `git rebase --onto <that tag's ref> origin/main main`. Expected: `git log --oneline -2` shows the docs commit on top of the tag's commit; `git status` clean.
- [x] **Step 3:** `git diff --stat <that tag's ref> HEAD`. Expected: only files under `docs/superpowers/`.

### Task 2: The sync tool (`tools/atlas_sync.py`)

**Files:**
- Create: `tools/atlas_sync.py`
- Test: `tests/test_atlas_sync.py`

**Interfaces:** Produces (all in `tools/atlas_sync.py`):
- `parse_launcher_tag(name: str) -> tuple[int, int, int] | None` (only exactly `launcher@<n>.<n>.<n>`)
- `latest_launcher_tag(names: list[str]) -> str | None`
- `next_atlas_version(tags: list[str]) -> str` (over this repo's own tags; `"0.1.0"` if none, else newest + patch 1)
- `release_title(atlas_version: str, upstream_tag: str) -> str` → `"Atlas 0.1.0 · based on Horizon launcher 0.2.50"`
- `current_base(repo: Path) -> str | None` (newest `refs/upstream/tags/launcher@*` that is an ancestor of `HEAD`, as `launcher@X.Y.Z`)
- `new_tag(repo: Path) -> str | None` (newest upstream tag if newer than `current_base`, else `None`)
- `@dataclass RebaseResult(ok: bool, failed_patch: str | None, base: str)`
- `rebase_patches(repo: Path, new_tag: str) -> RebaseResult` (`git rebase --onto refs/upstream/tags/<new_tag> <current_base>`; on conflict `git rebase --abort`, `ok=False`, `failed_patch` = the conflicting commit's subject)
- CLI `python tools/atlas_sync.py new-tag | base | next-version | rebase TAG | title ATLAS UPSTREAM` (`base` prints `current_base`; `next-version` reads this repo's own `launcher@*` tags): prints the result; `new-tag` prints nothing and exits 0 when there is no new tag; `rebase` exits 1 and prints `FAILED_PATCH=<subject>` on conflict.

- [x] **Step 1: Write the failing tests** in `tests/test_atlas_sync.py` (`unittest`, temp git repos built with `subprocess`):
  - `test_parse_launcher_tag`: `launcher@0.2.50` → `(0, 2, 50)`; `launcher-v0.16.0`, `launcher@0.2.51-rc1`, `nightly`, `launcher@1.2` → `None`.
  - `test_latest_launcher_tag_sorts_numerically`: `["launcher@0.2.9", "launcher@0.2.50", "launcher@0.10.0", "launcher-v9.0.0"]` → `"launcher@0.10.0"`; `[]` → `None`.
  - `test_next_atlas_version`: `[]` → `"0.1.0"`; `["launcher@0.1.0", "launcher@0.1.1", "v2"]` → `"0.1.2"`; `release_title("0.1.0", "launcher@0.2.50")` == `"Atlas 0.1.0 · based on Horizon launcher 0.2.50"`.
  - `test_rebase_replays_patches_onto_the_new_tag`: upstream repo with commit c1 (`a.txt`) tagged `launcher@0.2.50`, commit c2 (`c.txt`) tagged `launcher@0.2.51`; Atlas repo fetched into `refs/upstream/tags/`, reset to the first tag, patch commits `patch: one` (edits `a.txt`) and `patch: two` (adds `b.txt`). `new_tag(repo)` == `"launcher@0.2.51"`; `rebase_patches` gives `ok=True`; `git log --format=%s` ends `patch: two`, `patch: one`, then c2's subject; `current_base(repo)` == `"launcher@0.2.51"`; `new_tag(repo)` is now `None`.
  - `test_a_conflicting_patch_is_named_and_the_repo_left_clean`: same, but c2 also edits the line `patch: one` edited. `ok=False`, `failed_patch == "patch: one"`, `git status --porcelain` empty, no `.git/rebase-merge`, `HEAD` unchanged.
- [x] **Step 2: Run to verify it fails:** `uv run python -m unittest tests.test_atlas_sync -v`. Expected: ImportError / FAIL (no module `atlas_sync`; add `tools/` to the path the way neighbouring `tests/test_*.py` import tools).
- [x] **Step 3: Implement** the interfaces above with `subprocess` calls to `git` (`-C repo`), no third-party packages. Order tags by `parse_launcher_tag`'s tuple.
- [x] **Step 4: Run to verify it passes:** same command. Expected: all five tests `ok`.
- [x] **Step 5: Commit:** `git add tools/atlas_sync.py tests/test_atlas_sync.py && git commit -m "atlas: sync tool"`.

### Task 3: Patch `offline-play`

**Files:**
- Modify: `launcher/src/backend.rs` (`pub const OFFLINE_PLAY: bool = true;`, `Settings::load`, `client_as`), `launcher/src/cli.rs` (play and account commands stop requiring an owner; messages), `launcher/src/mcp.rs` and wherever `"Sign in with a Microsoft account that owns Minecraft: Java Edition to play."` is produced (find with `grep -rn "owns Minecraft: Java Edition to play" launcher/src`), `launcher/src/main.rs`, `launcher/src/frame.rs`, `launcher/src/glass_account.rs` (hide the Microsoft sign-in plates and the Microsoft accounts section; offline-name chips stay)
- Test: `launcher/tests/owner_gate.rs`, plus a unit test beside `Settings` in `launcher/src/backend.rs`

**Interfaces:** Produces: `backend::OFFLINE_PLAY: bool`. Consumes: `account::session_for_game(game_home, offline: bool)`, `Accounts::play_as_with(store, player, offline: bool)`, `Settings::dev_offline`. Reference diff: `../horizon-free` (cli.rs, main.rs, frame.rs, glass_account.rs); keep the seam to a constant check in each place.

- [x] **Step 1: Write the failing tests.** In `launcher/tests/owner_gate.rs` replace the two refusal tests with `the_cli_plays_offline_without_an_owner` and `the_mcp_server_plays_offline_without_an_owner`: same fixtures, assert the combined output does **not** contain `REFUSED` nor `account sign-in`, and `launcher/accounts.json` still lists `Builder_1`. In `backend.rs` tests add `settings_load_is_offline_whatever_the_file_says`: with `HORIZON_LAUNCHER_HOME` pointing at a scratch folder whose `settings.json` is `{"dev_offline": false}`, then empty, then `not json`, `Settings::load().dev_offline` is `true` each time (follow the env-isolation pattern of neighbouring `Settings` tests).
- [x] **Step 2: Run to verify they fail:** `cargo test --manifest-path launcher/Cargo.toml --release --test owner_gate` and `cargo test --manifest-path launcher/Cargo.toml --release --lib settings_load_is_offline`. Expected: FAIL (the old refusal is still produced; `dev_offline` is false).
- [x] **Step 3: Implement** `OFFLINE_PLAY` and make `Settings::load()` set `dev_offline = OFFLINE_PLAY`; `client_as` passes `OFFLINE_PLAY` to `session_for_game`; the CLI/MCP owner checks are skipped when `OFFLINE_PLAY`; hide the Microsoft plates in the three UI files with an `if backend::OFFLINE_PLAY { return ... }` at each site horizon-free's diff shows (new text through `tr()`).
- [x] **Step 4: Run to verify they pass:** the two commands above, then `cargo test --manifest-path launcher/Cargo.toml --release --lib --bins`. Expected: PASS (fix any `lang/*.json` consistency test by adding the keys it names).
- [x] **Step 5: Commit:** `git add launcher && git commit -m "patch: offline-play"`.

### Task 4: Patch `choose-jar`

**Files:**
- Modify: `launcher/src/sources.rs` (`pub fn from_jar(jar: &Path, pins: &Pins, linked: &Path, others: &[PathBuf], fetch: Fetch, log: &dyn Fn(String)) -> Result<Resolved, String>`, ported from `../Horizon-Offline-Launcher/launcher/src/sources.rs`), `launcher/src/backend.rs` (`Settings::minecraft_jar: Option<PathBuf>`, its use in `minecraft_dir` and where `from_jar` is called, as in the reference), `launcher/src/cli.rs` (`settings minecraft-jar PATH`, and the settings JSON dump), `launcher/src/main.rs` (the "Choose jar…" row on Home with a clear action), `launcher/lang/*.json` (new strings)
- Test: `launcher/src/sources_tests.rs`

**Interfaces:** Consumes: `Pins` (`pins()` helper in `sources_tests.rs`), `sha1_file`. Produces: `sources::from_jar` as above; `Settings::minecraft_jar`.

- [x] **Step 1: Write the failing tests** in `sources_tests.rs`: `from_jar_refuses_a_jar_that_is_not_the_pinned_release` (a temp file of other bytes → `Err` containing `is not the Minecraft 26.3 release client jar`); `from_jar_refuses_a_missing_file` (`Err` containing `the file doesn't exist`); `from_jar_accepts_the_pinned_jar_in_a_bare_folder` (a temp file in an empty folder, `Pins { client_sha1: <sha1 of that file>, ..pins() }` → `Ok(Resolved::Folder(_))`, no panic). In `backend.rs` tests: `settings_keep_the_picked_jar` (`Settings::parse` of `{"minecraft_jar": "/x/26.3.jar"}` gives `Some("/x/26.3.jar")`).
- [x] **Step 2: Run to verify they fail:** `cargo test --manifest-path launcher/Cargo.toml --release --lib from_jar settings_keep_the_picked_jar`. Expected: FAIL (items not defined).
- [x] **Step 3: Implement** by porting the reference (function, setting, CLI key, UI row), adapting names to this tree. Do not port anything that fetches a jar from Mojang beyond what upstream's resolver already does for an owner.
- [x] **Step 4: Run to verify they pass:** the command above, then the full `--lib --bins` run. Expected: PASS.
- [x] **Step 5: Commit:** `git add launcher && git commit -m "patch: choose-jar"`.

### Task 5: Patches `own-releases` and `own-keys`

**Files:**
- Modify: `launcher/src/backend.rs` (`pub const LAUNCHER_REPO: &str = "nezbut/horizon-atlas";` launcher releases read from it; Atom-feed fallback `launcher_feed()` / `feed_releases(&str) -> Vec<Release>`; `Updates::launcher_error: Option<String>` shown instead of "up to date"; the `platform_assets_are_published` test reads only `.github/workflows/atlas-release.yml`), `launcher/src/cli.rs` (print the error), `launcher/src/main.rs` (show it in the update line); for keys: `crates/horizon-release-sign/release-keys.txt`
- Test: `launcher/src/backend.rs` tests; `crates/horizon-release-sign/src/lib.rs` tests

**Interfaces:** Consumes: `Release`, `launcher_update_from`, `releases_and_updates_with`. Produces: `LAUNCHER_REPO`, `feed_releases`, `Updates::launcher_error`. The game's repo `backend::REPO` stays `horizon-rust/horizon`; signature code (`signed_digest`) is untouched.

- [ ] **Step 1: Write the failing tests.** `the_release_feed_still_offers_the_newest_launcher`: a feed text with entries `launcher@0.1.0`, `launcher@0.1.1` (updated `2026-11-09T10:00:00Z`, title `Atlas 0.1.1 · based on Horizon launcher 0.2.50`) and a `nightly-2026-11-05` entry → `feed_releases` returns 2 releases, `launcher_update_from(&releases, "launcher@0.1.0")` is `launcher@0.1.1` with date `2026-11-09` and this platform's asset URL under `https://github.com/nezbut/horizon-atlas/releases/download/launcher%400.1.1/` decoded to `launcher@0.1.1`. `a_failed_launcher_check_is_reported_not_called_up_to_date`: `releases_and_updates_with` with a launchers closure returning `Err("boom")` yields `Updates { launcher: None, launcher_error: Some(msg containing "boom"), .. }`. In `horizon-release-sign`: `release_keys_hold_upstream_and_atlas`: `trusted_keys().len() == 2` and one key's base64 is the upstream key from Global Constraints.
- [ ] **Step 2: Run to verify they fail:** `cargo test --manifest-path launcher/Cargo.toml --release --lib release_feed failed_launcher_check`; `cargo test -p horizon-release-sign release_keys_hold`. Expected: FAIL.
- [ ] **Step 3: Implement** `own-releases` (follow horizon-free's `backend.rs` diff for the feed and error plumbing; keep the signature check on). Commit it. Then `own-keys`: **owner step first**: the owner has no `gh`, so `tools/ci/new_release_key.sh` is not used. They create the key pair on their own machine, so the private key never enters this session: private key = base64 of a PKCS#8 DER Ed25519 key (`openssl genpkey -algorithm ed25519 -outform DER`), pasted by hand into the `release` environment's secret `RELEASE_SIGNING_KEY`; public key = base64 of the last 32 bytes of the DER public key (`openssl pkey -inform DER -pubout -outform DER | tail -c 32 | base64`), which they send to you. **Before giving them the commands, rehearse the exact same commands with a throwaway key** in the scratchpad: sign a dummy file with `tools/ci/sign_release.sh` (`RELEASE_SIGNING_KEY=<throwaway>`) and check it with `cargo run -p horizon-release-sign -- verify` against the throwaway public key. Add the owner's public key as a line under upstream's in `release-keys.txt`.
- [ ] **Step 4: Run to verify they pass:** the commands above plus `cargo test --manifest-path launcher/Cargo.toml --release --lib --bins`. Expected: PASS.
- [ ] **Step 5: Commit** twice: `git add launcher && git commit -m "patch: own-releases"`; `git add crates/horizon-release-sign && git commit -m "patch: own-keys"`.

### Task 6: Workflows: remove upstream's, trim, release

**Files:**
- Delete: every `.github/workflows/*.yml` except `launcher.yml` (`git rm`)
- Create: `.github/workflows/atlas-release.yml` (from `launcher.yml`, then `git rm` `launcher.yml`), `.github/workflows/atlas-ci.yml` (names start with `atlas-` so rebases never conflict with upstream's files)

**Interfaces:** Produces: `atlas-release.yml` with `workflow_dispatch` inputs `version` (required, `X.Y.Z`), `upstream_tag` (required, `launcher@X.Y.Z`), `dry_run` (boolean, default `true`). Consumed by Task 7 via `gh workflow run atlas-release.yml -f version=... -f upstream_tag=... -f dry_run=false`.

- [x] **Step 1: Delete** the upstream workflows with `git rm`.
- [x] **Step 2: Create `atlas-release.yml`** by copying `launcher.yml` and editing: remove the `push`/`schedule` triggers and the `paths` list; replace the `decide` job with a `meta` job that outputs `tag=launcher@<version>`, `version`, `head=$GITHUB_SHA`, `dry_run` (no `tools/release_train.py`); replace `(vars.MAC_RUNNER || 'ludovics-mac-mini')` with `macos-15`; in the `release` job build notes with `python3 tools/atlas_sync.py title "$version" "$upstream_tag"` (title) plus one line "Unofficial fork; see README" (body); publish with `--latest=true` (so `releases/latest/download/...` works for `install.sh`). Keep the `sign` job, the `release` environment and the `verify` step exactly as upstream has them.
- [x] **Step 3: Create `atlas-ci.yml`:** on `push` and `pull_request`, ubuntu-22.04, install the Linux packages listed in `launcher/README.md`, `cargo fmt --manifest-path launcher/Cargo.toml --check`, `cargo test --manifest-path launcher/Cargo.toml --release --lib --bins`.
- [x] **Step 4: Verify the YAML parses:** `uv run --with pyyaml python -c "import glob,yaml; [yaml.safe_load(open(f)) for f in glob.glob('.github/workflows/*.yml')]"`. Expected: no output, exit 0. (Actions behaviour itself is verified by the owner's dry run, Task 9.)
- [x] **Step 5: Commit:** `git add .github && git commit -m "atlas: workflows"`.

### Task 7: The daily sync workflow

**Files:** Create: `.github/workflows/atlas-sync.yml`

**Interfaces:** Consumes: `tools/atlas_sync.py` CLI (Task 2), `atlas-release.yml` inputs (Task 6), secret `SYNC_TOKEN`.

- [x] **Step 1: Create `atlas-sync.yml`** (as built: see "As built" at the end) with `schedule: cron '17 4 * * *'` and `workflow_dispatch`, `concurrency: group: atlas-sync, cancel-in-progress: false`, and two jobs, both checking out with `fetch-depth: 0` and `token: ${{ secrets.SYNC_TOKEN }}`, adding `upstream` and fetching `'+refs/tags/launcher@*:refs/upstream/tags/launcher@*'`:
  - `tag`: `tag=$(python3 tools/atlas_sync.py new-tag)`; if empty, stop. Otherwise `python3 tools/atlas_sync.py rebase "$tag"`; on exit 1, open an issue titled `Sync to <tag> failed: <FAILED_PATCH>` with `gh issue create` and stop; on success run `cargo fmt --check` and `cargo test --release --lib --bins` in `launcher/`, `git push --force-with-lease origin HEAD:main`, then `gh workflow run atlas-release.yml -f version="$(python3 tools/atlas_sync.py next-version)" -f upstream_tag="$tag" -f dry_run=false`. A failed test opens an issue too and publishes nothing.
  - `main-check` (no publishing): `git fetch upstream main`; on a throwaway branch `git rebase --onto upstream/main "refs/upstream/tags/$(python3 tools/atlas_sync.py base)"`; `cargo check --manifest-path launcher/Cargo.toml --release`; on failure open an issue labelled `sync-main` only if none is open.
- [x] **Step 2: Verify:** the YAML check from Task 6 Step 4; then rehearse locally against the sibling clone as upstream: in a scratch copy run `git fetch ../horizon '+refs/tags/launcher@*:refs/upstream/tags/launcher@*'` and `uv run python tools/atlas_sync.py new-tag`. Expected: prints a tag or nothing, no traceback.
- [x] **Step 3: Commit:** `git add .github tools && git commit -m "atlas: daily sync"`.

### Task 8: Install script, README, branding, notices, owner checklist

**Files:**
- Create: `install.sh` (from `../Horizon-Offline-Launcher/install.sh`; `REPO="nezbut/horizon-atlas"`, directory and command `horizon-atlas`), `docs/superpowers/owner-checklist.md`
- Modify: `README.md` (an Atlas block above the original text, as `../horizon-free` does: unofficial and unaffiliated, goals, install, "bring your own `26.3.jar`", no online-mode servers, SmartScreen and macOS notes; the original README stays below), `AGENTS.md` (prepend a "This repository is a fork" section: Atlas changes the launcher only, commits carry no agent attribution, checks are `cargo test --release --lib --bins` in `launcher/`), `THIRD_PARTY_NOTICES.md` (credit Horizon-Offline-Launcher for ported code, MIT), `launcher/src/frame.rs` (sidebar link to `https://github.com/nezbut/horizon-atlas`, text through `tr()`)
- Test: none (docs and a link); verified by the commands below

- [ ] **Step 1: Create `install.sh` and the checklist.** The checklist lists, in order, what only the owner can do: enable Issues and Actions; create environment `release`; run `tools/ci/new_release_key.sh` and paste the public key (Task 5); create fine-grained PAT `SYNC_TOKEN` and store it as a repository secret; set the repository description to `Unofficial fork of Horizon (Minecraft Java rewritten in Rust). Offline launcher without a Microsoft account, bring your own jar, always in sync with upstream.`; back up the private key offline; push; run the Task 9 dry run; publish `0.1.0`.
- [ ] **Step 2: Write the README block, AGENTS.md preface, notices line and the sidebar link** with the Edit tool.
- [ ] **Step 3: Verify:** `sh -n install.sh` (expected: no output) and `cargo test --manifest-path launcher/Cargo.toml --release --lib --bins` (expected: PASS; add the sidebar string to `lang/*.json` if the consistency test asks).
- [ ] **Step 4: Commit** twice: `git add launcher && git commit -m "patch: branding"`; `git add install.sh README.md AGENTS.md THIRD_PARTY_NOTICES.md docs && git commit -m "atlas: install script, README, owner checklist"`.

### Task 9: End-to-end rehearsal and hand-off

**Files:** none created; produces a verification note in the reply.

- [ ] **Step 1: Full local checks:** `uv run python -m unittest discover -s tests -p 'test_atlas*.py'` (expected: all `ok`), `cargo fmt --manifest-path launcher/Cargo.toml --check` (no output), `cargo test --manifest-path launcher/Cargo.toml --release --lib --bins` (PASS), `cargo test -p horizon-release-sign` (PASS).
- [ ] **Step 2: Rebase rehearsal on the real history:** create a scratch branch at the previous upstream launcher tag, replay the patch commits onto it with `rebase_patches`, then onto the newest tag, and confirm `ok=True` for both. Report which tags were used.
- [ ] **Step 3: Hand over** the owner checklist (Task 8) and say plainly what was not verified locally: the GitHub Actions runs, macOS and Windows builds, the signing job, and the GUI (the owner opens the built launcher once, offline, picks a jar, and checks the sidebar link).

---

## As built (Tasks 6-7, after the controller's review)

- `atlas-sync.yml`: the job that runs the rebased code (`build`, `main-check`) holds no token; only `publish` holds `SYNC_TOKEN` and runs no repository code (it takes the rebased commits as a git bundle artifact and pushes with a lease). Issues are opened by separate `report` jobs with the default `github.token`.
- Upstream's workflows are deleted, never kept: `tools/atlas_sync.py` resolves modify/delete conflicts under `.github/workflows/` (files not named `atlas-*`) in favour of the deletion, and `drop-workflows` removes any new upstream workflow in its own commit.
- A non-blocking issue is opened when upstream changed `.github/workflows/launcher.yml`, `packaging/` or `tools/ci/sign_release.sh` between two tags: `atlas-release.yml` is a copy of upstream's `launcher.yml` and must be compared by hand.
- `atlas-sync.yml` has a manual `dry_run` input (default true): everything runs, nothing is pushed or released.
- If `atlas-release.yml` fails after `publish` pushed main, the next daily run finds no new tag and does not retry: re-run `atlas-release.yml` by hand.

## Self-review notes

Spec coverage: goal 1 → Tasks 2, 6, 7; goal 2 → Task 3; goal 3 → Task 4; goal 4 → Tasks 5, 6; goal 5 → Tasks 6, 8 (installers come from upstream's build jobs, plus `install.sh`); goal 6 → Task 8; non-goals → Global Constraints; roadmap → already written. Names used across tasks (`rebase_patches`, `new_tag`, `current_base`, `atlas-release.yml` inputs, `OFFLINE_PLAY`, `LAUNCHER_REPO`) are defined once in the owning task's Interfaces.
