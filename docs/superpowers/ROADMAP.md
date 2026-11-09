# Horizon Atlas roadmap

Now: the launcher fork, see `specs/2026-11-08-atlas-launcher-fork-design.md`.

Later:

- Landing page on GitHub Pages: easy access to the fork's downloads and a short explanation.
- Check offline skins and capes: upstream already has them (`launcher/src/skins.rs`, `horizon-client-render/src/skins.rs`: `offline_skin()`, `offline_cape()`). Patch the game only if something does not work (Horizon-Offline-Launcher noted capes were not drawn), and only then build the game ourselves.
- Optional Microsoft sign-in with Atlas's own Azure app (for owners who want online-mode servers).
- Install Atlas beside the official launcher, not over it. Windows: `horizon-launcher-setup.exe` uses the official
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
- More of the owner's own features.
