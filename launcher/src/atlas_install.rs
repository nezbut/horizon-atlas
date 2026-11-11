//! Atlas installs beside the official Horizon launcher (`patch: own-install`). Atlas 0.1.x used the
//! official launcher's names: its Linux menu entry, and on Windows its install folder, shortcuts and
//! registry keys. 0.2.0 has its own; the data folder stays shared on purpose.
//!
//! The pure parts, testable on every system: how an Atlas executable is told from the official
//! launcher's, where 0.1.x lived on Windows, and the Linux menu entry's move. The Windows install
//! itself is `installer.rs` and `installer_atlas.rs`.
use std::fs;
use std::path::{Path, PathBuf};

/// The official launcher's menu entry, which Atlas 0.1.x wrote under that same name.
const OLD_ENTRY: &str = "horizon-launcher.desktop";

/// What only an Atlas executable holds: the repository it updates from, built into every Atlas since
/// 0.1.0 (`backend::LAUNCHER_REPO`). A literal, so a renamed repository still finds the old ones.
const MARKER: &[u8] = b"nezbut/horizon-atlas";

/// The official launcher's install folder on Windows, which Atlas 0.1.x installed into too.
pub const OLD_FOLDER: &str = "Horizon Launcher";

/// The launcher's file on Windows: the same name for Atlas and the official launcher.
const WINDOWS_BINARY: &str = "horizon-launcher.exe";

/// The file in Atlas's `atlas/` folder that says the player answered "Don't show again" to the
/// dialog asking a launcher in the old place to move.
const MOVE_DISMISSED: &str = "windows-move-dismissed";

/// Whether an executable's bytes are Atlas's: they hold [`MARKER`]; the official launcher never
/// names Atlas's repository.
pub fn has_atlas_marker(exe_bytes: &[u8]) -> bool {
    exe_bytes
        .windows(MARKER.len())
        .any(|w| w[0] == MARKER[0] && w == MARKER)
}

/// Whether the launcher at `exe` runs from the official launcher's install folder, where Atlas 0.1.x
/// installed itself on Windows: a folder named "Horizon Launcher", in any letters. Either slash
/// separates folders, so a Windows path reads the same on every system.
pub fn in_old_windows_place(exe: &Path) -> bool {
    let path = exe.to_string_lossy();
    let mut parts = path.split(['/', '\\']).filter(|p| !p.is_empty()).rev();
    parts.next(); // The executable itself.
    parts
        .next()
        .is_some_and(|dir| dir.eq_ignore_ascii_case(OLD_FOLDER))
}

/// Whether `dir` holds an Atlas install in the official launcher's place: a folder named "Horizon
/// Launcher" whose `horizon-launcher.exe` is Atlas's ([`has_atlas_marker`]). An official launcher
/// there is not, and nothing that answers no here is ever removed.
pub fn is_old_atlas(dir: &Path) -> bool {
    let exe = dir.join(WINDOWS_BINARY);
    in_old_windows_place(&exe) && fs::read(&exe).is_ok_and(|bytes| has_atlas_marker(&bytes))
}

/// Whether the window opens the dialog that asks the launcher at `exe` to move into Atlas's own
/// install: it runs from the official launcher's folder ([`in_old_windows_place`]), and the player
/// hasn't answered "Don't show again" (kept in Atlas's folder `atlas_dir`).
pub fn shows_move_dialog(exe: &Path, atlas_dir: &Path) -> bool {
    in_old_windows_place(exe) && !atlas_dir.join(MOVE_DISMISSED).exists()
}

/// "Don't show again": keeps the answer in Atlas's folder `atlas_dir`.
pub fn dismiss_move_dialog(atlas_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(atlas_dir)?;
    fs::write(atlas_dir.join(MOVE_DISMISSED), b"")
}

/// The page with the newest release and its installer.
pub fn releases_page() -> String {
    format!(
        "https://github.com/{}/releases/latest",
        crate::backend::LAUNCHER_REPO
    )
}

/// Linux: removes `applications/horizon-launcher.desktop` under `data` when the program it starts is
/// `exe`, which Atlas's own entry now starts (Atlas 0.1.x wrote its entry under the official
/// launcher's name). An entry that starts anything else, the official launcher, stays. `target` reads
/// an entry's program (`desktop::entry_target`). Whether it removed one.
pub(crate) fn remove_old_entry_in(
    data: &Path,
    exe: &Path,
    target: impl Fn(&str) -> Option<String>,
) -> bool {
    let old = data.join("applications").join(OLD_ENTRY);
    let starts_atlas = fs::read_to_string(&old)
        .ok()
        .and_then(|entry| target(&entry))
        .is_some_and(|program| Path::new(&program) == exe);
    starts_atlas && fs::remove_file(&old).is_ok()
}

/// Linux: whether removing Atlas's menu entry keeps `path`, one of the files it wrote under `data`.
/// The game's entry and Horizon's icons stay while the official launcher's entry is there: it uses
/// them too (`Icon=horizon`). Atlas's own entry, `own_entry`, always goes.
pub(crate) fn keeps_shared(data: &Path, path: &Path, own_entry: &str) -> bool {
    path.file_name().is_none_or(|name| name != own_entry)
        && data.join("applications").join(OLD_ENTRY).exists()
}

/// What uninstall's "also delete" removes in the data folder `data`: Atlas's own folder (its
/// accounts and their secrets, offline names, skins and capes), never `data` itself, where the
/// official launcher keeps its builds, settings, accounts and servers. None when that folder holds
/// none of Atlas's files: a `HORIZON_LAUNCHER_HOME` pointing at a general folder may have an
/// `atlas` folder of something else's.
pub fn own_data_dir(data: &Path) -> Option<PathBuf> {
    let atlas = crate::offline::atlas_in(data);
    let own = [
        "offline.json",
        "accounts.json",
        crate::account::SECRETS_FILE,
        "skins",
        MOVE_DISMISSED,
    ];
    own.iter()
        .any(|name| atlas.join(name).exists())
        .then_some(atlas)
}

/// Upstream's installer texts that Atlas's replace. Upstream's Polish test wants every Polish key to
/// appear as a literal in the code, and the lang files keep them so the sync's merge stays additive.
const _: [&str; 8] = [
    "Installs Horizon Launcher for this account. No administrator needed.",
    "Removes Horizon Launcher from this computer. Your worlds and the game's folder stay.",
    "Also delete the launcher's data",
    "Game builds, servers made in the launcher, settings, skins.",
    "Removing the launcher's data",
    "Horizon Launcher is installed.",
    "Horizon Launcher was removed.",
    "Launch Horizon",
];

/// Of the `running` launchers (process id, full path of its program when it could be read), the
/// ones whose program is inside the folder `dir`: the only ones an uninstall of `dir` may close. An
/// official launcher elsewhere, or one whose path couldn't be read, is left alone. Either slash
/// separates folders and letters may differ in case, as Windows has them. A `dir` that names no
/// folder (empty, a drive or a root) matches nothing.
pub fn launchers_inside(dir: &str, running: &[(u32, Option<String>)]) -> Vec<u32> {
    let plain = |path: &str| path.replace('/', "\\").to_lowercase();
    // The folder's own name: the last part that is not a drive (`C:`).
    let named = plain(dir)
        .rsplit('\\')
        .find(|part| !part.is_empty())
        .is_some_and(|last| !last.ends_with(':'));
    if !named {
        return Vec::new();
    }
    let folder = format!("{}\\", plain(dir).trim_end_matches('\\'));
    running
        .iter()
        .filter(|(_, path)| {
            path.as_deref()
                .is_some_and(|p| plain(p).starts_with(&folder))
        })
        .map(|&(pid, _)| pid)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::{install_in, launcher_entry};

    /// A folder of the test's own, gone at the end.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "horizon-atlas-install-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The entry Atlas 0.1.x wrote for the binary at `exe`: the official launcher's name and text.
    fn old_entry(exe: &str) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName=Horizon\nExec=\"{exe}\"\nTryExec={exe}\n\
             Icon=horizon\nTerminal=false\nCategories=Game;\nStartupWMClass=horizon-launcher\n"
        )
    }

    #[test]
    fn the_entry_is_atlas_own() {
        let data = Scratch::new("entry");
        let exe = Path::new("/opt/atlas/horizon-launcher");
        assert!(launcher_entry(exe).contains("\nName=Horizon Atlas\n"));
        install_in(&data.0, exe).unwrap();
        let apps = data.0.join("applications");
        let entry = fs::read_to_string(apps.join("horizon-atlas.desktop")).unwrap();
        assert!(entry.contains("\nName=Horizon Atlas\n"));
        assert!(entry.contains("\nTryExec=/opt/atlas/horizon-launcher\n"));
        // The official launcher's name is not Atlas's to write.
        assert!(!apps.join(OLD_ENTRY).exists());
    }

    #[test]
    fn an_old_entry_that_starts_atlas_is_removed() {
        let data = Scratch::new("old-ours");
        let apps = data.0.join("applications");
        fs::create_dir_all(&apps).unwrap();
        fs::write(
            apps.join(OLD_ENTRY),
            old_entry("/home/a b/Atlas/horizon-launcher"),
        )
        .unwrap();
        assert!(install_in(&data.0, Path::new("/home/a b/Atlas/horizon-launcher")).unwrap());
        assert!(!apps.join(OLD_ENTRY).exists());
        assert!(apps.join("horizon-atlas.desktop").exists());
        // An entry with only an Exec line names its program there.
        fs::write(
            apps.join(OLD_ENTRY),
            "[Desktop Entry]\nName=Horizon\nExec=\"/opt/atlas/horizon-launcher\" %U\n",
        )
        .unwrap();
        install_in(&data.0, Path::new("/opt/atlas/horizon-launcher")).unwrap();
        assert!(!apps.join(OLD_ENTRY).exists());
    }

    #[test]
    fn an_old_entry_of_the_official_launcher_stays() {
        let data = Scratch::new("old-official");
        let apps = data.0.join("applications");
        fs::create_dir_all(&apps).unwrap();
        let official = old_entry("/home/a/.local/bin/horizon-launcher");
        fs::write(apps.join(OLD_ENTRY), &official).unwrap();
        install_in(&data.0, Path::new("/home/a/Atlas/horizon-launcher")).unwrap();
        assert_eq!(fs::read_to_string(apps.join(OLD_ENTRY)).unwrap(), official);
        // Nor one that names no program at all.
        fs::write(apps.join(OLD_ENTRY), "[Desktop Entry]\nName=Horizon\n").unwrap();
        install_in(&data.0, Path::new("/home/a/Atlas/horizon-launcher")).unwrap();
        assert!(apps.join(OLD_ENTRY).exists());
    }

    #[test]
    fn the_marker_is_found_only_in_atlas() {
        let atlas = b"\x7fELF...https://github.com/nezbut/horizon-atlas/releases\0...";
        assert!(has_atlas_marker(atlas));
        assert!(has_atlas_marker(b"nezbut/horizon-atlas"));
        // The official launcher's own repository and near misses are not Atlas.
        assert!(!has_atlas_marker(
            b"MZ...https://github.com/horizon-rust/horizon/releases\0horizon-launcher"
        ));
        assert!(!has_atlas_marker(b"nezbut/horizon-atla"));
        assert!(!has_atlas_marker(b"nezbut/Horizon-Atlas"));
        assert!(!has_atlas_marker(b""));
    }

    #[test]
    fn the_old_place_is_recognised() {
        for old in [
            r"C:\Users\a\AppData\Local\Programs\Horizon Launcher\horizon-launcher.exe",
            r"D:\Games\horizon launcher\horizon-launcher.exe",
            "C:/Users/a/AppData/Local/Programs/Horizon Launcher/horizon-launcher.exe",
        ] {
            assert!(in_old_windows_place(Path::new(old)), "{old}");
        }
        for new in [
            r"C:\Users\a\AppData\Local\Programs\Horizon Atlas Launcher\horizon-launcher.exe",
            r"C:\Users\a\AppData\Local\Programs\Horizon Atlas\horizon-launcher.exe",
            r"C:\Users\a\Downloads\horizon-launcher.exe",
            r"C:\Horizon Launcher\Atlas\horizon-launcher.exe",
            r"C:\Horizon Launcher Old\horizon-launcher.exe",
            "horizon-launcher.exe",
        ] {
            assert!(!in_old_windows_place(Path::new(new)), "{new}");
        }
    }

    #[test]
    fn an_official_launcher_in_the_old_place_is_not_atlas() {
        let root = Scratch::new("old-place");
        let old = root.0.join("Horizon Launcher");
        fs::create_dir_all(&old).unwrap();
        // An empty folder, or one whose launcher is gone, holds no install.
        assert!(!is_old_atlas(&old));
        let exe = old.join("horizon-launcher.exe");
        fs::write(
            &exe,
            b"MZ\x90\0official https://github.com/horizon-rust/horizon\0",
        )
        .unwrap();
        assert!(!is_old_atlas(&old));
        fs::write(
            &exe,
            b"MZ\x90\0atlas https://github.com/nezbut/horizon-atlas\0",
        )
        .unwrap();
        assert!(is_old_atlas(&old));
        // Atlas anywhere else is not in the old place: its own install, a download.
        let own = root.0.join("Horizon Atlas Launcher");
        fs::create_dir_all(&own).unwrap();
        fs::copy(&exe, own.join("horizon-launcher.exe")).unwrap();
        assert!(!is_old_atlas(&own));
    }

    #[test]
    fn the_move_dialog_shows_until_the_player_says_never() {
        let atlas = Scratch::new("move-dialog");
        let dir = atlas.0.join("atlas");
        let old =
            Path::new(r"C:\Users\a\AppData\Local\Programs\Horizon Launcher\horizon-launcher.exe");
        let own = Path::new(
            r"C:\Users\a\AppData\Local\Programs\Horizon Atlas Launcher\horizon-launcher.exe",
        );
        // Shown at every start ("Not now" keeps nothing), even before Atlas's folder exists.
        assert!(shows_move_dialog(old, &dir));
        assert!(shows_move_dialog(old, &dir));
        assert!(!shows_move_dialog(own, &dir));
        dismiss_move_dialog(&dir).unwrap();
        assert!(dir.join("windows-move-dismissed").is_file());
        assert!(!shows_move_dialog(old, &dir));
        // Saying it again changes nothing.
        dismiss_move_dialog(&dir).unwrap();
        assert!(!shows_move_dialog(old, &dir));
    }

    /// The files besides the entries: the game's entry and the icons, shared with the official
    /// launcher.
    fn shared_files(data: &Path) -> Vec<PathBuf> {
        let mut files = vec![data.join("applications/horizon.desktop")];
        for size in ["16x16", "48x48", "512x512"] {
            files.push(data.join(format!("icons/hicolor/{size}/apps/horizon.png")));
        }
        files.push(data.join("icons/hicolor/scalable/apps/horizon.svg"));
        files
    }

    #[test]
    fn removing_the_entry_keeps_what_the_official_launcher_uses() {
        let data = Scratch::new("remove-shared");
        let apps = data.0.join("applications");
        install_in(&data.0, Path::new("/opt/atlas/horizon-launcher")).unwrap();
        let official = old_entry("/home/a/.local/bin/horizon-launcher");
        fs::write(apps.join(OLD_ENTRY), &official).unwrap();
        crate::desktop::remove_in(&data.0).unwrap();
        assert!(!apps.join("horizon-atlas.desktop").exists());
        for file in shared_files(&data.0) {
            assert!(file.is_file(), "{}", file.display());
        }
        assert_eq!(fs::read_to_string(apps.join(OLD_ENTRY)).unwrap(), official);
        // Without the official launcher's entry, everything Atlas wrote goes.
        fs::remove_file(apps.join(OLD_ENTRY)).unwrap();
        install_in(&data.0, Path::new("/opt/atlas/horizon-launcher")).unwrap();
        crate::desktop::remove_in(&data.0).unwrap();
        assert!(!apps.join("horizon-atlas.desktop").exists());
        for file in shared_files(&data.0) {
            assert!(!file.exists(), "{}", file.display());
        }
    }

    #[test]
    fn deleting_atlas_data_takes_only_atlas_own_folder() {
        let data = Scratch::new("own-data");
        let atlas = data.0.join("atlas");
        // The official launcher's: its settings and accounts, never what "also delete" takes.
        fs::write(data.0.join("settings.json"), "{}").unwrap();
        fs::write(data.0.join("accounts.json"), "{}").unwrap();
        // No atlas folder: nothing to delete.
        assert_eq!(own_data_dir(&data.0), None);
        // One that holds none of Atlas's files belongs to something else.
        fs::create_dir_all(&atlas).unwrap();
        fs::write(atlas.join("notes.txt"), "not Atlas's").unwrap();
        assert_eq!(own_data_dir(&data.0), None);
        // Any one of Atlas's own files makes it Atlas's: then that folder, and only it.
        for own in [
            "offline.json",
            "accounts.json",
            "account-secrets.json",
            "windows-move-dismissed",
        ] {
            fs::write(atlas.join(own), "").unwrap();
            assert_eq!(own_data_dir(&data.0), Some(atlas.clone()), "{own}");
            fs::remove_file(atlas.join(own)).unwrap();
        }
        fs::create_dir_all(atlas.join("skins")).unwrap();
        assert_eq!(own_data_dir(&data.0), Some(atlas.clone()));
    }

    #[test]
    fn an_uninstall_closes_only_the_launchers_in_its_folder() {
        let dir = r"C:\Users\a\AppData\Local\Programs\Horizon Atlas Launcher";
        let running = [
            (10, Some(format!(r"{dir}\horizon-launcher.exe"))),
            // Windows' letters may differ in case from the folder the registry names.
            (
                11,
                Some(r"c:\users\a\appdata\local\programs\horizon atlas launcher\HORIZON-LAUNCHER.EXE".into()),
            ),
            // The official launcher, an old Atlas copy, a folder whose name starts the same.
            (20, Some(r"C:\Users\a\AppData\Local\Programs\Horizon Launcher\horizon-launcher.exe".into())),
            (21, Some(r"C:\Users\a\AppData\Local\Programs\Horizon Atlas Launcher Old\horizon-launcher.exe".into())),
            (22, Some(r"D:\Games\horizon-launcher.exe".into())),
            // One whose program couldn't be read is not closed.
            (23, None),
        ];
        assert_eq!(launchers_inside(dir, &running), [10, 11]);
        // A folder named with a closing slash, or with the other slash, is the same folder.
        assert_eq!(launchers_inside(&format!(r"{dir}\"), &running), [10, 11]);
        assert_eq!(
            launchers_inside(&dir.replace('\\', "/"), &running),
            [10, 11]
        );
        assert!(launchers_inside(dir, &[]).is_empty());
    }

    #[test]
    fn a_folder_that_names_no_folder_closes_nothing() {
        let running = [
            (
                10,
                Some(r"C:\Users\a\Programs\Horizon Atlas Launcher\horizon-launcher.exe".into()),
            ),
            (
                11,
                Some(r"\\nas\games\Horizon Atlas Launcher\horizon-launcher.exe".into()),
            ),
            (12, Some("/opt/atlas/horizon-launcher".into())),
        ];
        for dir in ["", r"C:\", "C:", "c:/", r"\", "/", r"\\", "//"] {
            assert!(launchers_inside(dir, &running).is_empty(), "{dir:?}");
        }
    }
}
