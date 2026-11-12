# Horizon Atlas roadmap

Done in 0.1.x: the launcher fork itself, see `specs/2026-11-08-atlas-launcher-fork-design.md`.

Done in 0.2.0 (`specs/2026-11-11-atlas-0.2.0-offline-module-design.md`): upstream removed offline names from its
launcher (commit `1e789800af`), so Atlas keeps offline play in its own module instead of switching upstream's on.

- `patch: offline`: `launcher/src/offline/`, offline names with their own skins and capes in `<data_dir>/atlas/`
  (copied once from 0.1.x's files), the Profile tab and the `horizon-launcher offline …` CLI; Atlas's own accounts file
  and keychain service, so the official launcher's Microsoft accounts never show in Atlas.
- `patch: no-sign-in`: a dialog instead of a Microsoft sign-in that cannot work without Atlas's own client ID; the
  glass top bar hides its sign-in button until then.
- `patch: own-install`: Atlas installs beside the official launcher. Windows: "Horizon Atlas Launcher" in its own
  folder and registry keys, with an option to remove an old Atlas copy from the official launcher's folder. Linux: its
  own menu entry, `horizon-atlas.desktop`.
- `patch: branding`: the window title says "Horizon Atlas".
- macOS (`atlas-release.yml`): the app is `Horizon Atlas.app`, with its own bundle identifier.

Later:

- Landing page on GitHub Pages: easy access to the fork's downloads and a short explanation.
- 3D preview of offline skins and capes (upstream's `horizon_cosmetics::preview` / `viewer::Stage` draw a 3D player;
  0.2.0 starts with the flat front views).
- Check offline skins and capes in the game: upstream's game reads them (`horizon-client-render/src/skins.rs`:
  `offline_skin()`, `offline_cape()`, used for the local player only, so others see the default skin). Patch the game
  only if something does not work, and only then build the game ourselves.
- Optional Microsoft sign-in with Atlas's own Azure app (for owners who want online-mode servers). Horizon's client
  ID is Horizon's alone (`docs/forking.md`): Atlas needs its own Azure app and Mojang's approval.
- Names Atlas still shares with the official launcher (0.2.0 gave it its own install, menu entry and app): instance
  shortcuts are `<name> (Horizon)` (`launcher/src/shortcut.rs`; `.lnk` on Windows), the AppImage carries
  `packaging/linux/horizon-launcher.desktop` (`Name=Horizon`), and both launchers' desktop entries have
  `StartupWMClass=horizon-launcher`, the window class both launchers' windows share (`app_id` in `main.rs`), so a
  Linux dock groups them as one program. The data folders stay shared on purpose.
- A sync-tool check that upstream never contains the string `nezbut/horizon-atlas`: it is how the Windows installer
  tells an old Atlas copy from the official launcher (`launcher/src/atlas_install.rs`), so upstream gaining it would
  make an official launcher look like Atlas.
- Release file names of Atlas's own: the assets are still upstream's (`horizon-launcher-setup.exe`,
  `horizon-launcher-linux-x86_64.tar.gz`, `...-macos-arm64.zip`, the AppImage, the `.dmg`).
  - The self-updater finds its asset by these exact names (`launcher/src/backend.rs`, the platform list and
    `LAUNCHER_APPIMAGE_ASSET`): renaming the `.zip`, `.tar.gz` and AppImage needs a seam there and a transition of a
    few releases that publish both names, or every installed Atlas stops updating.
  - The setup `.exe` knows it is the setup by its file name starting with `horizon-launcher-setup`
    (`installer::is_setup_exe`): renaming it needs a one-line seam there, plus `atlas-release.yml`, the Windows CI
    check, `install.sh`'s messages and the README links. The `.dmg` is only downloaded by hand.
  - Do it once the Windows CI job has run green on the current names.
- Remove the one-time migration from Atlas 0.1.x (`launcher/src/offline/migrate.rs` and its tests) in a later
  significant release, once no 0.1.x installs are likely left. It costs little meanwhile: it runs once per data
  folder and lives in Atlas's own files.
- Polish left from 0.2.0:
  - The window doesn't reload `atlas/offline.json`: after `horizon-launcher offline select` from a terminal the
    account chip shows the old name until the next change in the window, while Play already starts the new one.
  - The MCP server's `account_list` reports no player and `owner: false` while its Play works offline; report the
    offline name that plays.
  - The macOS `.dmg` background is upstream's picture ("DRAG HORIZON TO APPLICATIONS"); give Atlas its own.
  - `task check` and `atlas-ci.yml` don't run `--test desktop`, which the sync now runs; `tools/atlas_sync.py`'s
    issue text names the older test command.
- Linting and code quality for Atlas's Python (`tools/atlas_sync.py`, `tests/test_atlas_sync.py`): ruff (lint and
  format) and ty/pyright (type check), through `uvx`, run by `atlas-ci.yml` on every push. The tool stays standard-library
  only.
- Servers tab integration with the owner's server projects, which live in their own repositories: first a
  friendly server (an easy Horizon server for a group of friends or a public one, on this machine or a VPS: Velocity
  with an auth plugin in front of `horizon-server`), later a native one in Rust without Java. Set one up and manage it
  from the launcher. Keep the patch a thin seam (new files, one entry in the Servers tab) and talk to the server
  through a stable interface (CLI with JSON output, or the panel's API).
- More of the owner's own features.
