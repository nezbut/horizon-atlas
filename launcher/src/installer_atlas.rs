//! Atlas installs beside the official launcher on Windows (`patch: own-install`), as `installer`'s
//! child module. Atlas 0.1.x installed under the official launcher's names: its folder
//! (`%LOCALAPPDATA%\Programs\Horizon Launcher`), shortcuts and registry keys. 0.2.0 has its own
//! (`installer.rs`: `APP_NAME`, `UNINSTALL_KEY`, `OWN_KEY`). This finds a 0.1.x install in the
//! official launcher's place and removes it: after Atlas is installed in its own folder, when the
//! player asks (the setup window's switch, or `setup --quiet --remove-old-atlas`), or when the old
//! copy's own uninstall runs (its Apps entry, still under the official launcher's name).
//!
//! A folder is an old Atlas install only when its `horizon-launcher.exe` is Atlas's
//! ([`is_old_atlas`]): an official launcher there, its shortcuts and its keys are never touched, nor
//! are the official launcher's keys when they name another folder. Uninstalling closes only the
//! launchers running from the folder it removes ([`launchers_in`]), never the official one. The data
//! folder is shared by both launchers: only Atlas's own `atlas/` folder in it is ever deleted, and
//! by an old copy's uninstall only when no new install is registered ([`offers_data_removal`]).
use super::*;
use crate::atlas_install::{is_old_atlas, launchers_inside, own_data_dir, OLD_FOLDER};

/// The official launcher's keys, which Atlas 0.1.x wrote too.
const OLD_UNINSTALL_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HorizonLauncher";
const OLD_OWN_KEY: &str = r"Software\Horizon\Launcher";

/// The folder the official launcher's key names (`InstallDir`), if there is one.
fn old_registered_dir() -> Option<PathBuf> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(OLD_OWN_KEY, KEY_READ)
        .ok()?;
    let dir: String = key.get_value("InstallDir").ok()?;
    Some(PathBuf::from(dir))
}

/// `%LOCALAPPDATA%\Programs\Horizon Launcher`, where both launchers install by default.
fn old_default_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::data_local_dir)
        .map(|dir| dir.join("Programs").join(OLD_FOLDER))
}

/// An Atlas 0.1.x install in the official launcher's place: the folder the official launcher's key
/// names, else the default one. None when neither holds Atlas (an official launcher there is not).
pub fn old_atlas_install() -> Option<PathBuf> {
    [old_registered_dir(), old_default_dir()]
        .into_iter()
        .flatten()
        .find(|dir| is_old_atlas(dir))
}

/// The old Atlas install this launcher runs from, if it does: its uninstall (the old Apps entry
/// "Horizon Launcher") removes that copy, not the install Atlas's own keys name.
pub(super) fn running_from_old_install() -> Option<PathBuf> {
    backend::exe_path()
        .parent()
        .filter(|dir| is_old_atlas(dir))
        .map(Path::to_path_buf)
}

/// Whether this uninstall may also delete Atlas's own data: not for an old copy beside a new
/// install, whose data it now is. The window shows its switch only then.
pub fn offers_data_removal() -> bool {
    running_from_old_install().is_none() || registered().is_none()
}

/// The uninstall of the old copy in `old` ([`running_from_old_install`]): removes that copy
/// ([`remove_old_atlas`]), and Atlas's own data when asked and no new install is registered (then it
/// is the player's last Atlas).
pub(super) fn uninstall_old(old: &Path, remove_data: bool) -> Result<Vec<String>, String> {
    let last = registered().is_none();
    let mut left = remove_old_atlas(old)?;
    if remove_data && last {
        remove_own_data(&mut left);
    }
    Ok(left)
}

/// Removes the old Atlas install in `dir`: its launcher and what its self-updates left beside it
/// (a launcher still running from there steps aside, as the uninstaller's own copy does); then,
/// once the launcher is gone, the Start menu's and the desktop's shortcuts that start it, the
/// official launcher's keys only where they name `dir`, and the folder if nothing else is in it.
/// A launcher that could not be removed keeps its shortcuts and its Apps entry, to try again.
/// Refused, touching nothing, when `dir` is not an old Atlas install ([`is_old_atlas`]). Returns what
/// could not be removed.
pub fn remove_old_atlas(dir: &Path) -> Result<Vec<String>, String> {
    if !is_old_atlas(dir) {
        return Err(format!(
            "{} holds no old Horizon Atlas install.",
            dir.display()
        ));
    }
    let exe = dir.join(BINARY);
    let mut left = Vec::new();

    // Only what setup and self-updates put there: the launcher and what they moved aside.
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name != BINARY && !name.starts_with("horizon-launcher.exe.") {
            continue;
        }
        let path = entry.path();
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            // A launcher still open from there: its file can't be deleted, but can be moved out.
            Err(_) if name == BINARY && step_aside(&path, dir).is_ok() => {}
            Err(e) => left.push(format!("{}: {e}", path.display())),
        }
    }
    if exe.exists() {
        // Still there, whatever hid why (a folder that couldn't be read): never "removed".
        let named = exe.display().to_string().to_lowercase();
        if !left
            .iter()
            .any(|item| item.to_lowercase().starts_with(&named))
        {
            left.push(exe.display().to_string());
        }
        return Ok(left);
    }

    let start_menu = start_menu_link().and_then(|link| link.parent().map(Path::to_path_buf));
    for folder in [start_menu, shortcut::desktop_dir()].into_iter().flatten() {
        for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
                && points_to(&path, &exe)
            {
                remove_file(&path, &mut left);
            }
        }
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for (key, location) in [
        (OLD_UNINSTALL_KEY, "InstallLocation"),
        (OLD_OWN_KEY, "InstallDir"),
    ] {
        let names_dir = hkcu
            .open_subkey_with_flags(key, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(location))
            .is_ok_and(|named| same_path(Path::new(&named), dir));
        if names_dir {
            if let Err(e) = hkcu.delete_subkey(key) {
                left.push(format!(r"HKCU\{key}: {e}"));
            }
        }
    }

    if let Err(e) = fs::remove_dir(dir) {
        // Something else lives there (a file Atlas didn't make): say so, unless listed above.
        if dir.exists() && left.is_empty() {
            left.push(format!("{}: {e}", dir.display()));
        }
    }
    Ok(left)
}

/// Installs as [`install`] does, then removes the old Atlas install ([`old_atlas_install`]) when
/// `options.remove_old` asks for it. Returns the installed `.exe` and what the removal could not take.
pub fn install_atlas(
    options: &Options,
    progress: impl Fn(Step, f32),
) -> Result<(PathBuf, Vec<String>), String> {
    let exe = install(options, &progress)?;
    let mut left = Vec::new();
    if let Some(old) = old_atlas_install().filter(|_| options.remove_old) {
        progress(Step::Removing, 1.0);
        left = remove_old_atlas(&old).unwrap_or_else(|e| vec![e]);
    }
    Ok((exe, left))
}

/// Of the launchers `pids`, those running from inside `dir`: the only ones its uninstall may close.
/// The official launcher has the same file name, so the name alone would close it too.
pub(super) fn launchers_in(dir: &Path, pids: Vec<u32>) -> Vec<u32> {
    let running: Vec<(u32, Option<String>)> =
        pids.into_iter().map(|pid| (pid, image_path(pid))).collect();
    launchers_inside(&dir.to_string_lossy(), &running)
}

/// The full path of process `pid`'s program. None when it can't be read (a process of another
/// user or an administrator's, or one that has ended).
fn image_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: the handle is checked before use and closed once; `buf` and `len` outlive the call,
    // and `len` holds `buf`'s size in characters, as the call expects.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let read =
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        (read != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

/// Uninstall's "also delete": Atlas's own folder in the data folder, when it is Atlas's
/// ([`own_data_dir`]); never the data folder itself or the build folder, which the official
/// launcher uses too.
pub(super) fn remove_own_data(left: &mut Vec<String>) {
    if let Some(atlas) = own_data_dir(&backend::data_dir()) {
        remove_all(&atlas, left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_official_launcher_in_the_old_place_is_not_removed() {
        let root = std::env::temp_dir().join(format!("horizon-atlas-old-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dir = root.join(OLD_FOLDER);
        fs::create_dir_all(&dir).unwrap();
        let official = b"MZ\x90\0official https://github.com/horizon-rust/horizon\0";
        fs::write(dir.join(BINARY), official).unwrap();
        fs::write(dir.join("horizon-launcher.exe.old-1-0"), b"MZ").unwrap();
        assert!(remove_old_atlas(&dir).is_err());
        // Refused before anything was touched.
        assert_eq!(fs::read(dir.join(BINARY)).unwrap(), official);
        assert!(dir.join("horizon-launcher.exe.old-1-0").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn this_test_process_is_found_in_its_own_folder_only() {
        let own = std::process::id();
        let exe = std::env::current_exe().unwrap();
        let dir = exe.parent().unwrap();
        // The path Windows gives may be spelled differently (8.3 names): read it the same way.
        let image = image_path(own).expect("the image path of this process");
        assert!(image.to_lowercase().ends_with(".exe"), "{image}");
        let image_dir = Path::new(&image).parent().unwrap();
        assert_eq!(launchers_in(image_dir, vec![own]), [own]);
        assert!(launchers_in(&dir.join("elsewhere"), vec![own]).is_empty());
        // A process that isn't there has no path and is never closed.
        assert!(launchers_in(image_dir, vec![u32::MAX - 1]).is_empty());
    }
}
