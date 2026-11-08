//! Atlas's "Update the launcher automatically" (`patch: own-releases`), off by default: a player who
//! installed an older Atlas on purpose isn't updated the moment it starts. The launcher checks for
//! its own updates as before either way; off, a new version is offered (the sidebar's card and the
//! Settings card) and downloads when the player presses Update. Kept in Atlas's own
//! `<data_dir>/atlas/settings.json`: the data folder's `settings.json` is shared with the official
//! launcher, which drops a field it doesn't know when it saves.
use super::*;
use crate::glass_ui as ui;
use horizon_theme::glass::ink;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

/// Atlas's settings, `atlas/settings.json` in the data folder.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct AtlasSettings {
    #[serde(default)]
    pub(crate) auto_update_launcher: bool,
}

impl AtlasSettings {
    /// The file in the data folder `data`, in Atlas's own folder there (the one `offline::atlas_dir`
    /// names too), never the shared `settings.json`.
    pub(crate) fn path_in(data: &Path) -> PathBuf {
        data.join("atlas").join("settings.json")
    }

    /// The settings in the data folder `data`. A file that is missing or doesn't read (not a JSON
    /// object, another value than a switch, a folder) is the defaults: the switch off.
    pub(crate) fn load_in(data: &Path) -> Self {
        std::fs::read(Self::path_in(data))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            // Serde would also read a struct from an array (`[true]`): only an object is the file.
            .filter(serde_json::Value::is_object)
            .and_then(|object| serde_json::from_value(object).ok())
            .unwrap_or_default()
    }

    /// Writes them to the data folder `data`, the `atlas` folder made if need be. The file is
    /// replaced whole: a write that stops halfway never leaves one that reads as the defaults.
    pub(crate) fn save_in(&self, data: &Path) -> Result<(), String> {
        let path = Self::path_in(data);
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())? + "\n";
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        let written = std::fs::create_dir_all(data.join("atlas"))
            .and_then(|()| std::fs::write(&tmp, text))
            .and_then(|()| std::fs::rename(&tmp, &path));
        written.map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", path.display())
        })
    }
}

/// Whether a launcher release a check found goes on to `update_launcher`, today's path (it
/// downloads, then Update restarts into it or it goes in when the window closes), rather than
/// waiting as an offer: with "Update the launcher automatically" on, or once the player pressed
/// Update. Where nothing can download (`can_download` false: a development build,
/// `HORIZON_LAUNCHER_NO_SELF_UPDATE`, a launcher that can't replace itself, a package manager's)
/// it goes on as before, which downloads nothing and only says so: no Update is offered there.
pub(crate) fn goes_on(auto: bool, pressed: bool, can_download: bool) -> bool {
    auto || pressed || !can_download
}

/// The switch as saved, read from the file once.
fn switch_state() -> &'static AtomicBool {
    static AUTO: OnceLock<AtomicBool> = OnceLock::new();
    AUTO.get_or_init(|| {
        AtomicBool::new(AtlasSettings::load_in(&backend::data_dir()).auto_update_launcher)
    })
}

/// "Update the launcher automatically", as the player left it.
pub(crate) fn auto_update_launcher() -> bool {
    switch_state().load(Ordering::Relaxed)
}

/// Turns the switch on or off, saved in `atlas/settings.json`.
fn set_auto_update_launcher(on: bool) -> Result<(), String> {
    let data = backend::data_dir();
    let mut settings = AtlasSettings::load_in(&data);
    settings.auto_update_launcher = on;
    settings.save_in(&data)?;
    switch_state().store(on, Ordering::Relaxed);
    Ok(())
}

impl Launcher {
    /// Whether the launcher release a check just found goes on to `update_launcher` by itself
    /// ([`goes_on`]): this copy can download one only if it may replace itself.
    pub(crate) fn atlas_update_goes_on(&self) -> bool {
        let can_download = backend::self_update_allowed() && !backend::cannot_replace_itself();
        goes_on(auto_update_launcher(), false, can_download)
    }

    /// The release offered instead of downloaded: found while the switch is off, nothing under
    /// way (or the last try failed: Update tries again, where the switch on would at the next
    /// check), and this copy may update itself (a launcher that can't replace itself went on to
    /// `update_launcher`, which said so).
    fn atlas_offered(&self) -> Option<Release> {
        let waits = matches!(self.self_update, SelfUpdate::Idle | SelfUpdate::Failed(_))
            && !auto_update_launcher()
            && backend::self_update_allowed();
        waits.then(|| self.updates.launcher.clone()).flatten()
    }

    /// Whether an update is offered: the narrow glass rail's dot on Settings shows it too.
    pub(crate) fn atlas_offers_update(&self) -> bool {
        self.atlas_offered().is_some()
    }

    /// The glass rail's card for an offered update, where a download's progress shows: the
    /// Ready card's look (`card`), with an Update that starts the download.
    pub(crate) fn atlas_rail_offer(
        &self,
        card: impl Fn(String, Option<String>) -> Div,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let release = self.atlas_offered()?;
        let version = backend::launcher_version(&release.tag)
            .unwrap_or_default()
            .to_string();
        Some(
            card(tr("A new version is available").to_string(), None)
                .child(
                    ui::text(12.5, FontWeight::NORMAL, ink::NOTE)
                        .mt(px(4.))
                        .child(trf("Launcher {version}", &[("version", &version)])),
                )
                .child(
                    ui::solid("atlas-update-launcher", tr("Update"), 32., 13., &self.theme)
                        .mt(px(10.))
                        .w_full()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.update_launcher(release.clone());
                            cx.notify();
                        })),
                ),
        )
    }

    /// The blocky sidebar's, under the line `update` draws.
    pub(crate) fn atlas_sidebar_offer(
        &self,
        update: impl Fn(String) -> Div,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let release = self.atlas_offered()?;
        let t = self.theme;
        let version = backend::launcher_version(&release.tag)
            .unwrap_or_default()
            .to_string();
        Some(
            update(tr("A new version is available").to_string())
                .child(
                    shell::text(theme::night::UI, shell::c(t.night.ink_soft))
                        .relative()
                        .child(trf("Launcher {version}", &[("version", &version)])),
                )
                .child(div().relative().flex().child(
                    button("atlas-update-launcher", "Update", Kind::Primary, &t).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.update_launcher(release.clone());
                            cx.notify();
                        }),
                    ),
                )),
        )
    }

    /// Settings' row under "Install new versions automatically", in both looks.
    pub(crate) fn atlas_auto_update_row(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let toggle = switch("atlas-auto-update", auto_update_launcher(), &t).on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                let on = !auto_update_launcher();
                match (set_auto_update_launcher(on), this.updates.launcher.clone()) {
                    // Turned on, a version already found goes on now, as Install new versions
                    // automatically installs the Horizon version it found.
                    (Ok(()), Some(release)) if on => this.update_launcher(release),
                    (Ok(()), _) => {}
                    (Err(e), _) => this.settings_save_error = Some(e),
                }
                cx.notify();
            },
        ));
        index_row(
            0,
            tr("Update the launcher automatically"),
            Some(
                div()
                    .child(tr(
                        "Off: Atlas shows when a new version is out and updates when you press Update.",
                    ))
                    .into_any_element(),
            ),
            Some(toggle.into_any_element()),
            &t,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A data folder of its own, removed afterwards.
    struct Data(PathBuf);

    impl Data {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "atlas-updates-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Data {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn on() -> AtlasSettings {
        AtlasSettings {
            auto_update_launcher: true,
        }
    }

    #[test]
    fn the_file_is_atlas_own_not_the_shared_settings() {
        let data = Path::new("/data");
        assert_eq!(
            AtlasSettings::path_in(data),
            data.join("atlas").join("settings.json")
        );
    }

    #[test]
    fn off_by_default_without_a_file_or_an_atlas_folder() {
        let data = Data::new("default");
        assert!(!data.0.join("atlas").exists());
        assert!(!AtlasSettings::load_in(&data.0).auto_update_launcher);
        // An atlas folder without the file says the same.
        fs::create_dir_all(data.0.join("atlas")).unwrap();
        assert_eq!(AtlasSettings::load_in(&data.0), AtlasSettings::default());
    }

    #[test]
    fn the_switch_round_trips_through_atlas_folder() {
        let data = Data::new("round-trip");
        // No atlas folder yet: saving makes it.
        on().save_in(&data.0).unwrap();
        assert!(data.0.join("atlas/settings.json").is_file());
        assert!(AtlasSettings::load_in(&data.0).auto_update_launcher);
        AtlasSettings::default().save_in(&data.0).unwrap();
        assert!(!AtlasSettings::load_in(&data.0).auto_update_launcher);
        // Never the shared settings.json beside it.
        assert!(!data.0.join("settings.json").exists());
    }

    #[test]
    fn a_file_that_does_not_read_is_off() {
        let data = Data::new("garbage");
        let file = AtlasSettings::path_in(&data.0);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        for garbage in [
            "",
            "not json",
            "[true]",
            r#"{"auto_update_launcher": "yes"}"#,
            r#"{"auto_update_launcher": 1}"#,
        ] {
            fs::write(&file, garbage).unwrap();
            assert!(
                !AtlasSettings::load_in(&data.0).auto_update_launcher,
                "{garbage:?}"
            );
        }
        // A file that is a folder can't be read either.
        fs::remove_file(&file).unwrap();
        fs::create_dir_all(&file).unwrap();
        assert!(!AtlasSettings::load_in(&data.0).auto_update_launcher);
        // Saving over a garbage file works.
        fs::remove_dir_all(&file).unwrap();
        fs::write(&file, "not json").unwrap();
        on().save_in(&data.0).unwrap();
        assert!(AtlasSettings::load_in(&data.0).auto_update_launcher);
    }

    #[test]
    fn an_atlas_that_cannot_be_a_folder_refuses_to_save() {
        let data = Data::new("blocked");
        fs::write(data.0.join("atlas"), "a file where the folder goes").unwrap();
        assert!(on().save_in(&data.0).is_err());
        assert!(!AtlasSettings::load_in(&data.0).auto_update_launcher);
    }

    #[test]
    fn a_found_update_downloads_by_itself_only_with_the_switch_on() {
        // (auto, pressed) -> goes on to the download, where a download can happen.
        for (auto, pressed, goes) in [
            (false, false, false),
            (false, true, true),
            (true, false, true),
            (true, true, true),
        ] {
            assert_eq!(
                goes_on(auto, pressed, true),
                goes,
                "auto {auto}, pressed {pressed}"
            );
        }
        // Where nothing can download (a development build, HORIZON_LAUNCHER_NO_SELF_UPDATE, a
        // launcher that can't replace itself or a package manager's), today's path is kept: it
        // downloads nothing and only says so, so no Update is offered there.
        for (auto, pressed) in [(false, false), (false, true), (true, false), (true, true)] {
            assert!(
                goes_on(auto, pressed, false),
                "auto {auto}, pressed {pressed}"
            );
        }
    }
}
