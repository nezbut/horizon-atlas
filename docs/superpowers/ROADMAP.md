# Horizon Atlas roadmap

Now: the launcher fork, see `specs/2026-11-08-atlas-launcher-fork-design.md`.

Next, Atlas 0.2.0: upstream removed offline names from its launcher (commit `1e789800af`, "play only as an account
that owns the game"; the game itself still plays offline under `--name` / `HORIZON_PLAYER_NAME` and still reads
`HORIZON_OFFLINE_SKIN`, `HORIZON_OFFLINE_SKIN_MODEL` and `HORIZON_OFFLINE_CAPE`). Atlas stops being "upstream with
offline switched on" and keeps offline play in its own module:

- `launcher/src/offline/`: several offline names (one selected), each with its own skin and cape, kept in Atlas's own
  files under the launcher's data folder (`atlas/`); the game gets them through the session's environment. Thin seams
  in upstream files (the session, who plays, the owner gate, the account menu). Strings through `tr()` with keys in
  every `launcher/lang/*.json`, translated into each language (the sync's auto-merge handles keys a patch only adds;
  if upstream ever adds a key with the same English text, the sync opens an issue and Atlas renames its key). Room in the model for a
  Microsoft account later, no sign-in button until Atlas has its own client ID.
- The window title says "Horizon Atlas".
- Install beside the official launcher (the item below).
- Built on upstream's `main` in a branch, swapped in when the first upstream tag without offline names arrives, and
  released by hand as 0.2.0.

Later:

- Landing page on GitHub Pages: easy access to the fork's downloads and a short explanation.
- 3D preview of offline skins and capes (upstream's `horizon_cosmetics::preview` / `viewer::Stage` draw a 3D player;
  0.2.0 starts with the flat front views).
- Check offline skins and capes in the game: upstream's game reads them (`horizon-client-render/src/skins.rs`:
  `offline_skin()`, `offline_cape()`, used for the local player only, so others see the default skin). Patch the game
  only if something does not work (Horizon-Offline-Launcher noted capes were not drawn), and only then build the game
  ourselves.
- Optional Microsoft sign-in with Atlas's own Azure app (for owners who want online-mode servers). Horizon's client
  ID is Horizon's alone (`docs/forking.md`): Atlas needs its own Azure app and Mojang's approval.
- Install Atlas beside the official launcher, not over it (part of 0.2.0). Windows: `horizon-launcher-setup.exe` uses the official
  launcher's name, folder (`%LOCALAPPDATA%\Programs\Horizon Launcher`) and HKCU registry keys (`launcher/src/installer.rs`:
  `APP_NAME`, `UNINSTALL_KEY`, `OWN_KEY`), so for one Windows user each replaces the other. Linux: the applications-menu
  entry has the official launcher's file name (`launcher/src/desktop.rs`: `LAUNCHER_ENTRY = "horizon-launcher.desktop"`),
  so with both installed the menu has one entry, for whichever wrote it first (it is rewritten only when missing or
  pointing at a deleted program). Give Atlas its own names, as a small patch (constants only, so it rarely conflicts).
  Check macOS too: the `.dmg` and the zip hold a `Horizon.app` that may share upstream's name and bundle identifier.
  The data folders stay shared on purpose.
- Linting and code quality for Atlas's Python (`tools/atlas_sync.py`, `tests/test_atlas_sync.py`): ruff (lint and
  format) and ty/pyright (type check), through `uvx`, run by `atlas-ci.yml` on every push. The tool stays standard-library
  only.
- Servers tab integration with the owner's server projects, which live in their own repositories: first a
  friendly server (an easy Horizon server for a group of friends or a public one, on this machine or a VPS: Velocity
  with an auth plugin in front of `horizon-server`), later a native one in Rust without Java. Set one up and manage it
  from the launcher. Keep the patch a thin seam (new files, one entry in the Servers tab) and talk to the server
  through a stable interface (CLI with JSON output, or the panel's API).
- More of the owner's own features.
