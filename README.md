# Horizon Atlas

**Play Horizon offline.** Horizon Atlas is a launcher for [Horizon](https://github.com/horizon-rust/horizon), Minecraft:
Java Edition 26.3 rewritten in Rust. It plays without a Microsoft account sign-in, prepares the game from your own copy
of Minecraft, and keeps up with every Horizon launcher release.

[![Latest release](https://img.shields.io/github/v/release/nezbut/horizon-atlas?label=latest)](https://github.com/nezbut/horizon-atlas/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nezbut/horizon-atlas/total)](https://github.com/nezbut/horizon-atlas/releases)
[![Windows · Linux · macOS](https://img.shields.io/badge/Windows%20%C2%B7%20Linux%20%C2%B7%20macOS-555)](#install)

**Download:** [Windows](https://github.com/nezbut/horizon-atlas/releases/latest/download/horizon-launcher-setup.exe) ·
[Windows ARM](https://github.com/nezbut/horizon-atlas/releases/latest/download/horizon-launcher-setup-arm64.exe) ·
[Linux (AppImage)](https://github.com/nezbut/horizon-atlas/releases/latest/download/horizon-launcher-linux-x86_64.AppImage) ·
[macOS (Apple Silicon)](https://github.com/nezbut/horizon-atlas/releases/latest/download/horizon-launcher-macos-arm64.dmg) ·
[macOS (Intel)](https://github.com/nezbut/horizon-atlas/releases/latest/download/horizon-launcher-macos-x64.dmg) ·
[all files](https://github.com/nezbut/horizon-atlas/releases/latest)

<p align="center">
  <img src="docs/media/atlas/launcher.png" alt="Horizon Atlas: the Home page with Play, an offline player name, the Nightly game build and two resource packs" width="100%">
</p>

> **Unofficial.** Made by a player. Not affiliated with, endorsed by or supported by Horizon's authors, Mojang or
> Microsoft: ask for help in this repository's [issues](https://github.com/nezbut/horizon-atlas/issues), not on
> Horizon's channels. You need your own copy of Minecraft: Java Edition.

## Features

- **Offline profiles, no Microsoft account.** In the **Profile** tab, add your player names, give each its own skin and cape, and pick who plays. From a terminal it is `horizon-launcher offline list`, `add`, `select`, `rename`, `remove`, `skin`, `cape` (`horizon-atlas offline …` after `install.sh`).
- **Beside the official launcher.** Atlas installs as its own program, so the official Horizon launcher can stay installed next to it.
- **Your own Minecraft, found or chosen.** Atlas finds Minecraft 26.3 in the launchers on your computer, or you pick its `26.3.jar` with **Choose jar…**.
- **All of Horizon.** The game is Horizon's, unmodified: your worlds, mods, resource and shader packs, the fast native renderer ([Horizon's README below](#horizon)).
- **Always current.** Each Horizon launcher release becomes an Atlas release, usually within hours, and the launcher updates itself.
- **Signed updates.** Every update is checked against Atlas's signing key before it is installed.

## Install

- **Windows:** run `horizon-launcher-setup.exe` (`horizon-launcher-setup-arm64.exe` on ARM). It installs **Horizon Atlas Launcher** for your user, in `%LOCALAPPDATA%\Programs\Horizon Atlas Launcher`. It is not code-signed, so SmartScreen may say "Windows protected your PC": choose **More info**, then **Run anyway**.
- **Linux (x86-64) and macOS (Apple Silicon)**, from a terminal:

  ```sh
  curl -fsSL https://raw.githubusercontent.com/nezbut/horizon-atlas/main/install.sh | sh
  ```

  It installs for your user only and puts `horizon-atlas` in `~/.local/bin`. Or take the AppImage (Linux) or the `.dmg` (macOS, Intel too) from the links above.
- **macOS:** the app is **Horizon Atlas**. It is not notarized, so macOS may refuse to open it the first time. Right-click it and choose **Open**, or run `xattr -dr com.apple.quarantine "<path to the app>"` (`install.sh` already does this).

### Coming from Atlas 0.1.x

Atlas updates itself to 0.2.0 as usual, and your worlds, settings, offline names and skins come along. Atlas 0.1.x took the official launcher's place, though; to move it to its own:

- **Windows:** run the new `horizon-launcher-setup.exe`. It installs Horizon Atlas Launcher in its own folder, and its option **Remove the old Horizon Atlas install** (ticked) removes the old copy from the official launcher's folder. Your worlds and settings stay. Remove the old copy this way, not with the old "Horizon Launcher" entry in Windows' installed apps. Until you move, Atlas reminds you when it starts (or choose **Don't show again**).
- **Linux:** nothing to do: the menu entry becomes **Horizon Atlas** by itself.
- **macOS:** an update keeps the old app's name (`Horizon`). To get **Horizon Atlas**, install again: `install.sh` replaces the old app in `~/Applications/Horizon Atlas` by itself; with the new `.dmg` or zip, move the old **Horizon** app to the Trash afterwards (unless it is the official launcher, which has the same name).

## First run

1. Have Minecraft: Java Edition 26.3 installed once, in any launcher (the official one, Prism, Modrinth App and others work).
2. Open Atlas and press **Prepare**. It finds your copy of 26.3 and prepares the game from it. If it cannot, press **Choose jar…** and pick `versions/26.3/26.3.jar` in the folder of the launcher that downloaded 26.3, so that its `libraries` and `assets` are found too.
3. In the **Profile** tab, add a player name (and a skin and a cape, if you like), then press **Play**.

## FAQ

**Do I need to own Minecraft?** Yes. Atlas prepares the game from your own copy of Minecraft: Java Edition 26.3 and never downloads Minecraft for you. The jar must be the 26.3 release, with the `libraries` and `assets` its launcher downloaded next to it; a jar on its own fails with a "Missing …/libraries/…" error.

**Can I play on servers?** In single player, in LAN worlds and on servers that run in offline mode, yes. Online-mode servers (most public ones) check a Microsoft account, and Atlas doesn't support Microsoft sign-in yet, so they turn you away.

**Can I sign in with Microsoft?** Not yet. Atlas keeps its own accounts, apart from the official launcher's, and shows a short note where a Microsoft sign-in would start.

**Will my worlds work?** Horizon opens vanilla saves as they are and upgrades worlds from 1.8 to 26.2.

**Is it safe?** The source is open, every release is built by GitHub Actions from this repository, and the launcher installs an update only if its signature checks out. The Windows warning only means the installer has no paid code-signing certificate.

**Mods and shader packs?** As in Horizon: install them from the launcher's Mods page.

**Can I keep the official Horizon launcher too?** Yes, side by side. Atlas is its own program (Horizon Atlas Launcher on Windows, Horizon Atlas in the Linux menu and on macOS), and both launchers share your worlds and settings. Their accounts stay apart.

## For developers: how Atlas stays in sync

Atlas is a short list of small commits (patches: `choose-jar`, `own-releases`, `own-keys`, `branding`, `offline`, `no-sign-in`, `own-install`) on top of one of Horizon's `launcher@X.Y.Z` tags.
Four times a day a workflow checks for a newer Horizon launcher tag, replays the patches onto it, tests, and publishes a release. If a patch no longer applies, nothing is published and an issue names the patch.
Atlas's versions are its own (`0.x`); each release title says which Horizon launcher it is based on.

The everyday commands are in `Taskfile.yml` ([go-task](https://taskfile.dev)):

- `task check`: every check (formatting, clippy, the launcher's tests, the sync tool's tests, actionlint).
- `task status`: the Horizon launcher tag Atlas is on, a newer one if there is, and Atlas's next version.
- `task rehearse -- <ref>`: replay the patches onto an upstream ref in a scratch clone, leaving yours alone.
- `task clone:sync`: bring your clone's `main` up to date after a sync.
- `task run:scratch`: start the built launcher on scratch folders, away from your real data.

## License

MIT, like Horizon (see [LICENSE](LICENSE)). Horizon is by Ludovic Marie and the Horizon contributors; Atlas is built on their work. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for other credits.

---

*Below is Horizon's own README, unchanged.*

<p align="center">
  <img src="docs/media/banner.webp" alt="Horizon: the game you know, rebuilt in Rust" width="100%">
</p>

<p align="center">
  <b>Minecraft: Java Edition 26.3, rebuilt from scratch in Rust.</b><br>
  The same rules and the same terrain, your own worlds, in one native program without Java.
</p>

<p align="center">
  <a href="https://playhorizon.dev"><img src="docs/media/readme/download-button.png" alt="Download the Horizon launcher for Windows, macOS and Linux" width="400"></a>
</p>

<p align="center">
  <a href="https://github.com/horizon-rust/horizon/stargazers"><img src="https://img.shields.io/github/stars/horizon-rust/horizon?style=flat&color=F0795A" alt="GitHub stars"></a>
  <a href="https://discord.gg/playhorizon"><img src="https://img.shields.io/discord/1559227979002028052?label=Discord&logo=discord&logoColor=white&color=5865F2" alt="Discord"></a>
  <a href="https://github.com/horizon-rust/horizon/graphs/contributors"><img src="https://img.shields.io/github/contributors/horizon-rust/horizon?color=2EA44F" alt="Contributors"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/licence-MIT-blue" alt="MIT licence"></a>
</p>

<p align="center">
  <a href="https://playhorizon.dev"><b>Website</b></a> ·
  <a href="https://playhorizon.dev/docs/"><b>Docs</b></a> ·
  <a href="https://playhorizon.dev/docs/install"><b>Install guide</b></a> ·
  <a href="https://discord.gg/playhorizon"><b>Discord</b></a> ·
  <a href="ROADMAP.md"><b>Roadmap</b></a>
</p>

<p align="center">
  <img src="docs/media/readme/valley.webp" alt="A river valley with a plains village, savanna hills and a cherry grove, seen from above with a long view distance" width="100%">
</p>

<p align="center">
  <img src="docs/media/readme/panel-worlds.webp" alt="Your worlds, same rules: survival, creative and hardcore, and your saves open as they are" width="49%">
  <img src="docs/media/readme/panel-together.webp" alt="Play together: vanilla and Horizon servers, LAN, or a code from a friend" width="49%">
</p>

<p align="center">
  <img src="docs/media/readme/panel-farther.webp" alt="See farther: terrain up to 1,024 chunks away, HDR and controllers" width="49%">
  <img src="docs/media/readme/panel-dimensions.webp" alt="Every dimension: vanilla world generation, mobs, villagers and redstone" width="49%">
</p>

<p align="center">
  <img src="docs/media/readme/panel-terrain.webp" alt="Same terrain: vanilla world generation, here a savanna village from above" width="49%">
  <img src="docs/media/readme/panel-mobs.webp" alt="Same mobs: attributes, spawn rules and mob AI match vanilla, here a baby piglin" width="49%">
</p>

<p align="center">
  <b>On the roadmap:</b> play in your browser · Android and iOS with mods · splitscreen · <a href="ROADMAP.md">see the roadmap</a>
</p>

<p align="center">
  <img src="docs/media/readme/signs-row.webp" alt="A voxel dragon in welding goggles points at three wooden signs: 3.5x faster start than modded Java, 38 percent more FPS than Java with Sodium, and a mod API to port your mods to Rust" width="100%">
</p>

<p align="center">
  <sub>Against Java Edition with Sodium, Lithium, FerriteCore and ImmediatelyFast on a Ryzen 7 9800X3D with an RTX 5080: the title screen in about 3 seconds instead of 11, and 31 to 38 percent more frames per second across render distances.</sub>
</p>

<h2 align="center">Made by the Horizon Team</h2>

<table align="center">
  <tr>
    <td align="center" width="140"><a href="https://github.com/kacpherek"><img src="https://github.com/kacpherek.png?size=160" width="80" alt="Kacper"><br><b>Kacper</b></a></td>
    <td align="center" width="140"><a href="https://github.com/Online5277"><img src="https://github.com/Online5277.png?size=160" width="80" alt="Cosmic"><br><b>Cosmic</b></a></td>
    <td align="center" width="140"><a href="https://github.com/levzzz5154"><img src="https://github.com/levzzz5154.png?size=160" width="80" alt="levzzz"><br><b>levzzz</b></a></td>
    <td align="center" width="140"><a href="https://github.com/rdbtCVS"><img src="https://github.com/rdbtCVS.png?size=160" width="80" alt="rdbt"><br><b>rdbt</b></a></td>
    <td align="center" width="140"><a href="https://github.com/dezer-x"><img src="https://github.com/dezer-x.png?size=160" width="80" alt="Anthony S"><br><b>Anthony S</b></a></td>
    <td align="center" width="140"><a href="https://github.com/ludovic111"><img src="https://github.com/ludovic111.png?size=160" width="80" alt="Ludovic Marie"><br><b>Ludovic Marie</b></a><br><sub>Founder</sub></td>
  </tr>
</table>

<p align="center">
  <sub>and everyone who has contributed</sub><br><br>
  <a href="https://github.com/horizon-rust/horizon/graphs/contributors"><img src="https://contrib.rocks/image?repo=horizon-rust/horizon&max=72&columns=12" alt="Everyone who has contributed to Horizon"></a>
</p>

<p align="center">
  <a href="docs/native-client.md#run-from-a-checkout"><b>Build guide</b></a> ·
  <a href="CONTRIBUTING.md"><b>Contributing</b></a> ·
  <a href="GOVERNANCE.md"><b>Governance</b></a> ·
  <a href="https://discord.gg/playhorizon"><b>Discord</b></a>
</p>

<sub>Horizon is released under the [MIT licence](LICENSE) for the project's original work; dependencies keep their own licences, see the [notices](THIRD_PARTY_NOTICES.md). Founded by [Ludovic Marie](https://lu4ovic.com) and built by the Horizon Team and [its contributors](https://github.com/horizon-rust/horizon/graphs/contributors). You can support it on [GitHub Sponsors](https://github.com/sponsors/ludovic111).</sub>

<sub>NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT. Minecraft is a trademark of Mojang Synergies AB; its code and assets belong to Mojang and Microsoft under the [Minecraft EULA](https://www.minecraft.net/en-us/eula).</sub>
