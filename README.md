# Horizon Atlas

> **Unofficial.** Horizon Atlas is a fork made by a player. It is not affiliated with, endorsed by or supported by
> Horizon's authors, Mojang or Microsoft. Do not ask them for help with it; open an issue here.

Horizon Atlas is a fork of the [Horizon](https://github.com/horizon-rust/horizon) **launcher**. The game is Horizon's, unmodified;
Atlas changes the launcher only. Minecraft: Java Edition is a product of Mojang and Microsoft, and you need your own copy of it.

## What it is for

- **Offline play without a Microsoft account.** Play starts the game under an offline name of your choice. There is no Microsoft sign-in in Atlas.
- **Choose your own `26.3.jar`.** Point the launcher at the client jar from your own copy of Minecraft: Java Edition. The jar needs the `libraries` and `assets` folders of that copy: they must be next to it (as in a launcher's own folder) or in another launcher on this computer that has 26.3.
- **Releases that follow Horizon's launcher releases.** A check four times a day turns each new Horizon launcher release into an Atlas release, with Atlas's changes replayed on top.
- **Signed with Atlas's own key.** The launcher updates itself from this repository and checks the signature before it installs anything. Game builds are still Horizon's, signed by Horizon's key.

## Install

Download from the [releases page](https://github.com/nezbut/horizon-atlas/releases/latest).

- **Windows:** run `horizon-launcher-setup.exe` (`horizon-launcher-setup-arm64.exe` on ARM). It is not code-signed, so SmartScreen may say "Windows protected your PC": choose **More info**, then **Run anyway**.
- **Linux (x86-64) and macOS (Apple Silicon):**

  ```sh
  curl -fsSL https://raw.githubusercontent.com/nezbut/horizon-atlas/main/install.sh | sh
  ```

  It installs for your user only and puts `horizon-atlas` in `~/.local/bin`. Linux also has an AppImage on the releases page, and Intel Macs have `horizon-launcher-macos-x64.zip`.
- **macOS:** the app is not notarized, so macOS may refuse to open it the first time. Right-click it and choose **Open**, or run `xattr -dr com.apple.quarantine "<path to the app>"` (`install.sh` already does this).

## First run

1. Have Minecraft: Java Edition 26.3 installed once, in any launcher (the official one, Prism, Modrinth App and others work).
2. Open Atlas and press **Prepare**. It finds your copy of 26.3 and prepares the game from it. If it cannot find it, press **Choose jar…** and pick the jar yourself, from the folder of the launcher that downloaded 26.3 (`versions/26.3/26.3.jar` there), so that its `libraries` and `assets` are found too.
3. Press **Play**.

The jar must be the **26.3 release** jar: Horizon reproduces 26.3 only, and the launcher refuses any other jar. A jar on its own is not enough: preparing also needs the `libraries` and `assets` that a launcher downloads next to it, and a folder holding only the jar fails with a "Missing …/libraries/…" error. The only way Atlas gets Minecraft is from your own copy of Minecraft: Java Edition.

## Limits

- **Online-mode servers are not supported.** They need a Microsoft sign-in, and Atlas has none. Atlas is for offline play.
- The game itself is Horizon's, unmodified. Atlas changes the launcher only.
- Atlas builds the launcher for Windows (x64, ARM64), Linux (x86-64) and macOS (Apple Silicon and Intel).

## How it stays in sync

Atlas is a short list of small commits (patches: `offline-play`, `choose-jar`, `own-releases`, `own-keys`, `branding`) on top of one of Horizon's `launcher@X.Y.Z` tags.
Four times a day a workflow checks for a newer Horizon launcher tag, replays the patches onto it, tests, and publishes a release. If a patch no longer applies, nothing is published and an issue names the patch.
Atlas's versions are its own (`0.x`); each release title says which Horizon launcher it is based on.

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
