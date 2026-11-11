//! Atlas's part of the window (`patch: offline`): who plays as the account chip and its menu show
//! it (the offline names after the Microsoft accounts), Atlas's own dialogs, and the Profile tab's
//! state (`atlas_profile.rs` draws it). Upstream's files call in here through one-line seams.
use super::*;
use crate::glass_ui::{self as ui, white};
use gpui::RenderImage;
use horizon_launcher::offline::profiles::Profiles;
use horizon_launcher::offline::skins::{self, Model};
use horizon_theme::glass::{ink, radius};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

/// What the account menu's keys for offline names start with; the rest are Microsoft UUIDs.
const OFFLINE_KEY: &str = "offline:";

/// The account menu's key for the offline name `name`.
pub(crate) fn offline_key(name: &str) -> String {
    format!("{OFFLINE_KEY}{name}")
}

/// The offline names as the account menu lists them, in the order the player keeps them: each
/// one's key and name.
pub(crate) fn offline_rows(profiles: &Profiles) -> Vec<(String, String)> {
    profiles
        .names
        .iter()
        .map(|p| (offline_key(&p.name), p.name.clone()))
        .collect()
}

/// Plays the offline name `key` stands for. None: the key isn't an offline name's, it is left to
/// upstream (a Microsoft account).
pub(crate) fn play_key(profiles: &mut Profiles, key: &str) -> Option<Result<(), String>> {
    let name = key.strip_prefix(OFFLINE_KEY)?;
    Some(profiles.select(name))
}

/// Changes the offline names in the data folder `data` as its `offline.json` has them now, and
/// keeps the result there. The profiles to show come back either way: the changed ones, or the
/// file's as they are when the change was refused or not kept (the file is the truth).
pub(crate) fn change_in(
    data: &Path,
    change: impl FnOnce(&mut Profiles) -> Result<(), String>,
) -> (Profiles, Result<(), String>) {
    let mut profiles = Profiles::load_in(data);
    let done = change(&mut profiles).and_then(|()| profiles.save_to(&Profiles::path_in(data)));
    if done.is_err() {
        profiles = Profiles::load_in(data);
    }
    (profiles, done)
}

/// Atlas's state in the window.
#[derive(Default)]
pub(crate) struct AtlasUi {
    /// The offline names and who plays, as `atlas/offline.json` had them at the last change.
    pub profiles: Profiles,
    /// The dialog over the window, while one is open.
    pub dialog: Option<AtlasDialog>,
    pub panel: ProfilePanel,
}

impl AtlasUi {
    /// Atlas's state at the window's start. Reading the profiles moves a 0.1.x player's offline
    /// names into Atlas's folder first (`Profiles::load`), before anything else can run.
    pub(crate) fn load() -> Self {
        Self {
            profiles: Profiles::load(),
            ..Self::default()
        }
    }
}

/// The Profile tab's state (`atlas_profile.rs`).
#[derive(Default)]
pub(crate) struct ProfilePanel {
    /// The name being renamed: the name field holds its new name. None: the field adds a name.
    pub editing: Option<String>,
    /// What the last change of a skin or cape did, or why not, and whose it was ([`Self::note_for`]).
    pub note: Option<(String, Result<String, String>)>,
    /// Why the last change of the names didn't happen, shown under the name field.
    pub name_error: Option<String>,
    /// The selected name's skin and cape as drawn, made again when they change.
    pub previews: Option<Previews>,
}

impl ProfilePanel {
    /// The last note about `name`'s skin or cape: none when it was about another name.
    pub(crate) fn note_for(&self, name: &str) -> Option<&Result<String, String>> {
        let (about, note) = self.note.as_ref()?;
        about.eq_ignore_ascii_case(name).then_some(note)
    }
}

/// A baked preview, or None without one.
pub(crate) type Preview = Option<Arc<RenderImage>>;

/// A name's skin front and cape, baked at the window's scale.
pub(crate) struct Previews {
    /// Whose, in which model, at which scale (its bits).
    pub key: (String, Model, u32),
    pub skin: Preview,
    pub cape: Preview,
    /// Whether the name has a skin file and a cape file, drawn or not.
    pub has_skin: bool,
    pub has_cape: bool,
}

/// A dialog of Atlas's own over the window.
pub(crate) enum AtlasDialog {
    /// Removing an offline name, asked first: its skin and cape go with it.
    ConfirmRemove(String),
}

/// What a button at a dialog's foot does to the window, once the dialog has closed.
type Act = Rc<dyn Fn(&mut Launcher, &mut Context<Launcher>)>;

/// How a dialog's button looks: a destructive answer, or a way out.
#[derive(Clone, Copy, PartialEq)]
enum Weight {
    Danger,
    Quiet,
}

/// A dialog's button.
struct Answer {
    id: &'static str,
    label: String,
    weight: Weight,
    act: Act,
}

impl Answer {
    fn new(
        id: &'static str,
        label: impl Into<String>,
        weight: Weight,
        act: impl Fn(&mut Launcher, &mut Context<Launcher>) + 'static,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            weight,
            act: Rc::new(act),
        }
    }
}

impl Launcher {
    /// The chip's two lines while Atlas plays offline: the offline name that plays, or "Offline"
    /// when nobody does (the game then keeps its own name). None: upstream's (a Microsoft account).
    pub(crate) fn atlas_who_plays(&self) -> Option<(String, &'static str)> {
        let profiles = &self.atlas.profiles;
        if !profiles.plays_offline() {
            return None;
        }
        Some(match profiles.selected() {
            Some(p) => (p.name.clone(), tr("Offline")),
            None => (tr("Offline").to_string(), tr("The game's own name")),
        })
    }

    /// The account menu picked `key`: plays that offline name, kept in `atlas/offline.json`, and
    /// closes the menu. Whether the key was Atlas's: upstream's `play_as` handles the others.
    pub(crate) fn atlas_play_as(&mut self, key: &str) -> bool {
        if !key.starts_with(OFFLINE_KEY) {
            return false;
        }
        let done = self.atlas_change(|profiles| play_key(profiles, key).unwrap_or(Ok(())));
        // Said where the player is (the menu closes); the menu already shows the file's names.
        if let Err(e) = done {
            glass_toast::say(e);
        }
        self.account_menu = false;
        true
    }

    /// Changes the offline names as `atlas/offline.json` has them now (the command line may have
    /// changed them since the window read them), and keeps the result there. The window then
    /// shows what the file holds, even when the change was refused ([`change_in`]).
    pub(crate) fn atlas_change(
        &mut self,
        change: impl FnOnce(&mut Profiles) -> Result<(), String>,
    ) -> Result<(), String> {
        let (profiles, done) = change_in(&backend::data_dir(), change);
        self.atlas.profiles = profiles;
        done
    }

    /// Escape closes Atlas's dialog. Whether one was open.
    pub(crate) fn atlas_escape(&mut self) -> bool {
        self.atlas.dialog.take().is_some()
    }

    /// The account menu's rows for the offline names, after the Microsoft accounts: key, name and
    /// how each plays.
    pub(crate) fn atlas_players(&self) -> Vec<(String, String, &'static str)> {
        offline_rows(&self.atlas.profiles)
            .into_iter()
            .map(|(key, name)| (key, name, tr("Offline")))
            .collect()
    }

    /// The account menu's key of who plays, for its tick: the offline name's while Atlas plays
    /// offline, else the Microsoft account's.
    pub(crate) fn atlas_chosen(&self) -> Option<String> {
        if self.atlas.profiles.plays_offline() {
            return (self.atlas.profiles.selected()).map(|p| offline_key(&p.name));
        }
        self.accounts.selected().map(|a| a.uuid.clone())
    }

    /// `owner` as the Play page's texts read it: none while Atlas plays offline. Those texts
    /// promise a download from Mojang "for your account", which an offline name doesn't get (the
    /// download itself asks for an owner again, `backend::download_for_owner`).
    pub(crate) fn atlas_owner(&self) -> Option<bool> {
        match self.atlas.profiles.plays_offline() {
            true => None,
            false => self.owner,
        }
    }

    /// Forgets the offline name `name` with its skin and cape (the confirmed Remove).
    pub(crate) fn atlas_remove(&mut self, name: &str) {
        let done = self.atlas_change(|profiles| profiles.remove(&skins::dir(), name));
        if done.is_ok() {
            self.atlas.panel.previews = None;
            let editing = self.atlas.panel.editing.as_deref();
            if editing.is_some_and(|e| e.eq_ignore_ascii_case(name)) {
                self.atlas_end_rename();
            }
        }
        self.atlas.panel.name_error = done.err();
    }

    /// Atlas's dialog, while one is open: over everything, closed by a press outside it or by
    /// any of its buttons.
    pub(crate) fn atlas_dialog(&mut self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let (title, text, answers) = match self.atlas.dialog.as_ref()? {
            AtlasDialog::ConfirmRemove(name) => {
                let gone = name.clone();
                (
                    trf("Remove {name}?", &[("name", name)]),
                    tr("Its skin and cape are removed with it.").to_string(),
                    vec![
                        Answer::new(
                            "atlas-dialog-cancel",
                            tr("Cancel"),
                            Weight::Quiet,
                            |_, _| {},
                        ),
                        Answer::new("atlas-dialog-remove", tr("Remove"), Weight::Danger, {
                            move |this, _| this.atlas_remove(&gone)
                        }),
                    ],
                )
            }
        };
        let t = self.theme;
        let buttons: Vec<Stateful<Div>> = answers
            .into_iter()
            .map(|answer| {
                let act = answer.act.clone();
                dialog_button(&answer, &t).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.atlas.dialog = None;
                        act(this, cx);
                        cx.notify();
                    },
                ))
            })
            .collect();
        let close = cx.listener(|this, _: &MouseDownEvent, _, cx| {
            this.atlas.dialog = None;
            cx.notify();
        });
        let n = t.night;
        // The glass look's dialog plate over its scrim; the blocky look's card, as its own dialogs.
        let (card, scrim) = if t.glass {
            let card = ui::dialog_plate(radius::MODAL)
                .flex()
                .flex_col()
                .p(px(28.))
                .child(
                    div()
                        .font_family(horizon_theme::glass::DISPLAY_FONT)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(22.))
                        .line_height(px(27.5))
                        .text_color(white(ink::FULL))
                        .child(title),
                )
                .child(
                    ui::text(14., FontWeight::NORMAL, ink::BODY)
                        .mt(px(10.))
                        .line_height(px(21.))
                        .child(text),
                )
                .child(
                    div()
                        .mt(px(24.))
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .children(buttons),
                );
            (card, ui::scrim())
        } else {
            let card = plate::cut(
                theme::night::PIXEL_CORNER,
                2,
                Some(plate::alpha(n.card, 0.98)),
                Some((plate::alpha(n.stroke, 2. * n.stroke_alpha), 1.)),
            )
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(28.))
                    .child(shell::text(theme::night::TITLE, shell::c(n.ink)).child(title))
                    .child(shell::text(theme::night::UI, shell::c(n.ink_soft)).child(text)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(28.))
                    .py(px(16.))
                    .border_t_1()
                    .border_color(plate::alpha(n.stroke, n.rule_alpha))
                    .children(buttons),
            );
            (card, gpui::Hsla::from(plate::alpha(n.page, 0.8)))
        };
        Some(
            div()
                .id("atlas-dialog")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(24.))
                .occlude()
                .on_scroll(|_, _, cx| cx.stop_propagation())
                .when(t.glass, |d| d.child(ui::scrim_backdrop()))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(scrim)
                        .on_mouse_down(gpui::MouseButton::Left, close),
                )
                .child(
                    card.id("atlas-dialog-card")
                        .occlude()
                        .w_full()
                        .max_w(px(460.)),
                ),
        )
    }
}

/// A dialog's button in the window's look.
fn dialog_button(answer: &Answer, t: &Theme) -> Stateful<Div> {
    let label = answer.label.clone();
    if t.glass {
        let kind = match answer.weight {
            Weight::Danger => Kind::Secondary,
            Weight::Quiet => Kind::Ghost,
        };
        return ui::button(answer.id.into(), label.into(), kind, t)
            .when(answer.weight == Weight::Danger, |d| {
                d.text_color(ui::danger())
            });
    }
    let action = match answer.weight {
        Weight::Danger => shell::Action::Raised,
        Weight::Quiet => shell::Action::Quiet,
    };
    shell::action(answer.id, action, t)
        .when(answer.weight == Weight::Danger, |d| d.text_color(t.danger))
        .child(shell::label(label))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use horizon_launcher::offline::profiles::Plays;
    use std::path::PathBuf;

    /// A folder of the test's own, gone at the end.
    pub(crate) struct Scratch(pub(crate) PathBuf);

    impl Scratch {
        pub(crate) fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("horizon-atlas-ui-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn kept(names: &[&str]) -> Profiles {
        let mut profiles = Profiles::default();
        for name in names {
            profiles.add(name).unwrap();
        }
        profiles
    }

    #[test]
    fn offline_rows_follow_the_profile_order() {
        let profiles = kept(&["Steve", "alex", "Zed_9"]);
        let rows: Vec<(String, String)> = offline_rows(&profiles);
        assert_eq!(
            rows,
            [
                ("offline:Steve", "Steve"),
                ("offline:alex", "alex"),
                ("offline:Zed_9", "Zed_9"),
            ]
            .map(|(k, n)| (k.to_string(), n.to_string()))
        );
        assert!(offline_rows(&Profiles::default()).is_empty());
    }

    #[test]
    fn an_offline_key_selects_that_name() {
        let mut profiles = kept(&["Steve", "Alex"]);
        assert_eq!(play_key(&mut profiles, "offline:Alex"), Some(Ok(())));
        assert_eq!(profiles.plays, Some(Plays::Offline("Alex".into())));
        // Any letters find the name; it plays as the list keeps it.
        assert_eq!(play_key(&mut profiles, "offline:steve"), Some(Ok(())));
        assert_eq!(profiles.plays, Some(Plays::Offline("Steve".into())));
        // A name forgotten meanwhile (the CLI) is refused, and who plays stays.
        assert!(matches!(
            play_key(&mut profiles, "offline:Ghost"),
            Some(Err(_))
        ));
        assert_eq!(profiles.plays, Some(Plays::Offline("Steve".into())));
    }

    #[test]
    fn a_key_that_is_not_ours_is_left_to_upstream() {
        let mut profiles = kept(&["Steve"]);
        let before = profiles.clone();
        assert_eq!(
            play_key(&mut profiles, "069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            None
        );
        assert_eq!(play_key(&mut profiles, ""), None);
        assert_eq!(profiles, before);
    }

    #[test]
    fn a_refused_change_shows_the_file_as_it_is() {
        let data = Scratch::new("change");
        let file = kept(&["Steve", "Alex"]);
        file.save_to(&Profiles::path_in(&data.0)).unwrap();
        // The window still lists Ghost, which the command line removed: picking it is refused,
        // and the window gets the file's names, without Ghost.
        let (shown, done) = change_in(&data.0, |p| p.select("Ghost"));
        assert!(done.is_err());
        assert_eq!(shown, file);
        let (shown, done) = change_in(&data.0, |p| p.select("alex"));
        assert_eq!(done, Ok(()));
        assert_eq!(shown.plays, Some(Plays::Offline("Alex".into())));
        assert_eq!(Profiles::load_in(&data.0), shown);
    }
}
