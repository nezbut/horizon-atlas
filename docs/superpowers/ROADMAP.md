# Horizon Atlas roadmap

Now: the launcher fork, see `specs/2026-11-08-atlas-launcher-fork-design.md`.

Later:

- Landing page on GitHub Pages: easy access to the fork's downloads and a short explanation.
- Check offline skins and capes: upstream already has them (`launcher/src/skins.rs`, `horizon-client-render/src/skins.rs`: `offline_skin()`, `offline_cape()`). Patch the game only if something does not work (Horizon-Offline-Launcher noted capes were not drawn), and only then build the game ourselves.
- Optional Microsoft sign-in with Atlas's own Azure app (for owners who want online-mode servers).
- More of the owner's own features.
