# Horizon Atlas roadmap

Now: the launcher fork, see `specs/2026-11-08-atlas-launcher-fork-design.md`.

Done in 0.2.0 (`specs/2026-11-11-atlas-0.2.0-offline-module-design.md`): upstream removed offline names from its
launcher (commit `1e789800af`), so Atlas keeps offline play in its own module instead of switching upstream's on.

- `patch: offline`: `launcher/src/offline/`, offline names with their own skins and capes in `<data_dir>/atlas/`
  (copied once from 0.1.x's files), the Profile tab and the `horizon-launcher offline …` CLI; Atlas's own accounts file
  and keychain service, so the official launcher's Microsoft accounts never show in Atlas.
- `patch: no-sign-in`: a dialog instead of a Microsoft sign-in that cannot work without Atlas's own client ID.
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
  `packaging/linux/horizon-launcher.desktop` (`Name=Horizon`), and both launchers' windows have
  `StartupWMClass=horizon-launcher`, so a Linux dock groups them as one program. The data folders stay shared on
  purpose.
- A sync-tool check that upstream never contains the string `nezbut/horizon-atlas`: it is how the Windows installer
  tells an old Atlas copy from the official launcher (`launcher/src/atlas_install.rs`), so upstream gaining it would
  make an official launcher look like Atlas.
- Linting and code quality for Atlas's Python (`tools/atlas_sync.py`, `tests/test_atlas_sync.py`): ruff (lint and
  format) and ty/pyright (type check), through `uvx`, run by `atlas-ci.yml` on every push. The tool stays standard-library
  only.
- Servers tab integration with the owner's server projects, which live in their own repositories: first a
  friendly server (an easy Horizon server for a group of friends or a public one, on this machine or a VPS: Velocity
  with an auth plugin in front of `horizon-server`), later a native one in Rust without Java. Set one up and manage it
  from the launcher. Keep the patch a thin seam (new files, one entry in the Servers tab) and talk to the server
  through a stable interface (CLI with JSON output, or the panel's API).
- More of the owner's own features.
