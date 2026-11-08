# Horizon Atlas: owner checklist

What only the repository owner can do, in order. Each item says why, so you can judge whether to do it.
Nothing here is done by the workflows themselves: they only use what you set up.

## 1. Issues, Pull requests and Actions are enabled

Done. Why it matters: the daily sync reports a patch that no longer applies as an **issue**, and every
workflow is an **Action**; with either off, a failure would go unnoticed or nothing would run.

## 2. Environment `release`, restricted to `main`

Settings → Environments → New environment → `release`. Under *Deployment branches and tags* choose
*Selected branches and tags* and add `main`.

Why: the signing key will live in this environment, and only the job that signs a release
(`sign` in `atlas-release.yml`) uses it. Restricting it to `main` means a branch or a pull request
cannot make GitHub hand the key to a workflow run.

## 3. The signing key pair (on your own machine)

Every release is signed so that launchers install only what you published. Make the key pair yourself,
so the private key is never in a chat, an agent session or the repository. On your own machine:

```sh
umask 077
openssl genpkey -algorithm ed25519 -outform DER -out ~/atlas-release-key.der
```

- **Private key** (secret): put it into the `release` environment as the secret `RELEASE_SIGNING_KEY`
  (Settings → Environments → `release` → Add secret). It is the base64 of that file; with Wayland:

  ```sh
  base64 -w0 ~/atlas-release-key.der | wl-copy
  ```

  Paste the clipboard as the secret's value. Why: the release job reads it to sign `SHA256SUMS`;
  nobody else needs it.
- **Public key** (not secret): print it with

  ```sh
  openssl pkey -inform DER -in ~/atlas-release-key.der -pubout -outform DER | tail -c 32 | base64
  ```

  and send the one line it prints. It goes into `crates/horizon-release-sign/release-keys.txt` under
  upstream's key (the `patch: own-keys` commit). Why: launchers build the list of trusted keys in, and
  refuse an update that none of them signed. **Until that line is in a released launcher, a release signed
  with your key is refused by every launcher, including Atlas's own self-update.** Add the key first,
  release second.
- **Back up** `~/atlas-release-key.der` offline (an encrypted USB stick, a password manager attachment),
  then delete the copy in your home folder. Why: if the key is lost you cannot sign again, and every
  installed launcher would have to be reinstalled by hand with a new key built in.

## 4. `SYNC_TOKEN`

Settings (your account) → Developer settings → Personal access tokens → Fine-grained tokens →
Generate new token.

- Name: `atlas-sync`.
- Expiration: up to one year; put a calendar reminder a few weeks before it ends (a lapsed token stops
  the daily sync with an authentication error).
- Repository access: *Only select repositories* → `nezbut/horizon-atlas`.
- Repository permissions: **Contents**, **Workflows**, **Actions**, **Issues**, each *Read and write*.

Store it as a **repository** secret (Settings → Secrets and variables → Actions) named `SYNC_TOKEN`.

Why a token at all: the sync pushes rebased commits that can change files under `.github/workflows/`
(upstream's workflows are deleted on sync), and then starts the release workflow. The workflow's own
default token can do neither: GitHub forbids it to push workflow changes, and what it pushes or starts
does not trigger other workflows.
Why it is only read by the `publish` job: the other jobs run code that comes from upstream and from
third-party crates (cargo, build scripts, tests). `publish` runs no code from the repository, so the
token never meets it.

## 5. Repository description

Settings → General → Description (or the gear next to *About* on the main page):

```
Unofficial fork of Horizon (Minecraft Java rewritten in Rust). Offline launcher without a Microsoft account, bring your own jar, always in sync with upstream.
```

Why: it is the first thing a visitor reads, and it says "unofficial" before anything else.

## 6. First dry runs

The workflows exist on GitHub only once this work is on the repository's `main`; push it before this
step. Then, in Actions:

1. Run **atlas-sync** by hand, leaving `dry_run` ticked (the default).
2. Run **atlas-release** by hand with `version` = `0.1.0`, `upstream_tag` = `launcher@0.2.54` and
   `dry_run` left ticked.

Why: they do everything except publish, so a mistake costs nothing. Open each run's **Summary**.

A healthy `atlas-sync` run:
- says `New upstream launcher tag: none` while Atlas is already on Horizon's newest launcher tag; or,
  when there is a newer tag, `Dry run: would push main at … and release Atlas X.Y.Z; nothing was pushed
  or released`;
- ends green; no issue was opened.

A healthy `atlas-release` run:
- starts with `launcher@0.1.0 (based on launcher@0.2.54) … dry run: true`;
- builds the launcher on Linux, macOS (Apple Silicon and Intel) and Windows (x64 and ARM64) and ends green. The
  Windows ARM64 build may fail without failing the run (a warning says that file is left out);
- ends with the release title and notes in the summary (`Atlas 0.1.0 · based on Horizon launcher 0.2.54`)
  and publishes nothing. A dry run signs with a throwaway key, so it works before step 3 is done.

## 7. First real release

Run **atlas-release** from `main` with `version` = `0.1.0`, `upstream_tag` = `launcher@0.2.54` and
`dry_run` **unticked**. (Or wait: once Horizon publishes a launcher tag newer than `launcher@0.2.54`, the
daily sync does this for you.) A run from any branch other than `main` is always a dry run.

Needs steps 2 and 3 done, and the public key from step 3 already in `release-keys.txt`.

Then check by hand, because no workflow can: download `horizon-launcher-setup.exe` from the release,
install it, open the launcher **offline** (no Microsoft account), press **Choose jar…**, pick your
`26.3.jar`, and look for the **Horizon Atlas** link in the sidebar.

## 8. When something breaks

- **Issue `Sync to <tag> failed: <patch>`**: the named patch no longer applies to Horizon's new
  launcher tag. Nothing was published; the repository is left as it was. Fix the patch on top of that
  tag and push; until then Atlas stays on its previous Horizon base.
- **Issue labelled `sync-main`**: an early warning that the patches no longer apply to, or build on,
  Horizon's current `main`. Nothing is wrong yet; it will bite at the next launcher tag.
- **Issue `Upstream changed the launcher's build in <tag>: compare atlas-release.yml`**: Horizon changed
  `launcher.yml`, `packaging/` or the signing script. `atlas-release.yml` is a copy of Horizon's
  `launcher.yml`, so compare the two by hand and bring over what matters. It does not block the sync.
- **`atlas-release` failed after `atlas-sync` had already pushed `main`**: the next daily run sees no new
  tag and does not retry. Run `atlas-release` by hand with the version and tag shown in the sync run's
  summary.

## 9. Later

`docs/superpowers/ROADMAP.md` lists what comes after the fork works (landing page, offline skins and
capes check, optional Microsoft sign-in with Atlas's own app, your own features).
