//! The Profile tab (`patch: offline`): Atlas's offline names down the left, the one that plays
//! marked, each with rename and remove, and a field to add one; on the right, that name's skin
//! and cape, choosing either from a PNG, and the arms' model. The glass look keeps upstream
//! 0.2.58's name chips (Settings > Account), both looks its skin panel (`skin_page.rs`).
use super::*;
use crate::atlas_ui::{AtlasDialog, Preview, Previews, ProfilePanel};
use crate::glass_ui::{self as ui, white};
use crate::servers_page::Field;
use horizon_launcher::offline::profiles::Profiles;
use horizon_launcher::offline::skins::{self, Model};
use horizon_theme::glass::{fill, ink, radius};
use std::path::{Path, PathBuf};

/// Points per skin texel in the previews.
const TEXEL: f32 = 6.;

/// The names column's width in both looks: with the look card's least width beside it, it fits
/// the window's default size; a narrower window puts the look card under it.
const NAMES_WIDTH: f32 = 340.;

/// A row of the blocky look's names as `slot_row` draws one (its panel, the name in the pixel
/// face, a caption under it), with `below` under the name rather than beside it: the column is
/// narrow.
fn classic_row(
    slot: Option<Div>,
    name: impl IntoElement,
    detail: Option<&str>,
    below: Div,
    t: &Theme,
) -> Div {
    plate(t)
        .flex()
        .flex_col()
        .gap(px(10.))
        .mb(px(8.))
        .px(px(16.))
        .py(px(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .children(slot)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
                        .child(
                            blocks::pixel(16., FontWeight::MEDIUM)
                                .text_color(t.ink)
                                .truncate()
                                .child(name),
                        )
                        .children(detail.map(|d| {
                            text(theme::CAPTION)
                                .text_color(t.muted)
                                .child(d.to_string())
                        })),
                ),
        )
        .child(below)
}

/// What Enter in the name field does to `profiles`: renames `editing` to what is typed (its skin
/// and cape in `skins` with it), else adds it. Whether anything changed: nothing typed changes
/// nothing.
pub(crate) fn enter_name(
    profiles: &mut Profiles,
    skins: &Path,
    editing: Option<&str>,
    typed: &str,
) -> Result<bool, String> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(false);
    }
    match editing {
        Some(old) => profiles.rename(skins, old, typed)?,
        None => profiles.add(typed)?,
    }
    Ok(true)
}

/// Keeps `file` as `name`'s skin (or cape) in `skins`, checked first, and says how it went in
/// `panel.note`, for that name. Nothing for a name forgotten meanwhile (while the file dialog was
/// open).
pub(crate) fn take_look_file(
    panel: &mut ProfilePanel,
    profiles: &Profiles,
    skins: &Path,
    name: &str,
    file: &Path,
    cape: bool,
) {
    let Some(kept) = profiles.find(name) else {
        return;
    };
    let name = kept.name.as_str();
    let done = skins::keep(skins, name, file, cape);
    if done.is_ok() {
        panel.previews = None;
    }
    let said = done.map(|()| match cape {
        false => trf("{name}'s skin is kept.", &[("name", name)]),
        true => trf("{name}'s cape is kept.", &[("name", name)]),
    });
    panel.note = Some((name.to_string(), said));
}

impl Launcher {
    pub(super) fn profile_page(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        // The name being renamed is gone (the command line removed it): no row would hold the field.
        let editing = self.atlas.panel.editing.as_deref();
        if editing.is_some_and(|e| self.atlas.profiles.find(e).is_none()) {
            self.atlas_end_rename();
        }
        let names = if t.glass {
            self.glass_names(cx)
        } else {
            self.classic_names(cx)
        };
        let look = self.look_card(cx);
        div()
            .flex()
            .flex_col()
            .when(!t.glass, |d| d.gap(px(32.)))
            .child(page_header(
                tr("Profile"),
                tr("Offline names for single player, LAN worlds and offline-mode servers, each with its own skin and cape. Online-mode servers (most big public ones) need a Microsoft account."),
                &t,
            ))
            // Side by side when the page is wide enough, else the skin under the names.
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .gap(px(if t.glass { 12. } else { 24. }))
                    .child(names)
                    .child(look),
            )
    }

    /// Enter in the name field, or its Add or Save: adds the name typed, or renames the one being
    /// edited to it. An empty field ends a rename.
    pub(crate) fn atlas_name_enter(&mut self) {
        let typed = self.servers.value(Field::AtlasName).to_string();
        if typed.trim().is_empty() {
            self.atlas_end_rename();
            return;
        }
        let editing = self.atlas.panel.editing.clone();
        let done = self.atlas_change(|profiles| {
            enter_name(profiles, &skins::dir(), editing.as_deref(), &typed).map(|_| ())
        });
        match done {
            Ok(()) => {
                self.atlas.panel.editing = None;
                self.servers.text(Field::AtlasName).set(String::new());
                self.servers.set_field_invalid(Field::AtlasName, false);
                self.atlas.panel.name_error = None;
            }
            Err(e) => {
                self.servers.set_field_invalid(Field::AtlasName, true);
                self.atlas.panel.name_error = Some(e);
            }
        }
    }

    /// Plays `name`.
    fn atlas_select(&mut self, name: &str) {
        let done = self.atlas_change(|profiles| profiles.select(name));
        self.atlas.panel.name_error = done.err();
    }

    /// Puts `name` in the name field to be renamed, in place of its row.
    fn atlas_start_rename(&mut self, name: &str) {
        self.atlas.panel.editing = Some(name.to_string());
        self.atlas.panel.name_error = None;
        let field = self.servers.text(Field::AtlasName);
        field.set(name.to_string());
        field.select_all();
        self.servers.set_field_invalid(Field::AtlasName, false);
        self.servers.focus_soon = Some(Field::AtlasName);
    }

    /// Ends a rename without one: the field adds names again.
    pub(crate) fn atlas_end_rename(&mut self) {
        if self.atlas.panel.editing.take().is_some() {
            self.servers.text(Field::AtlasName).set(String::new());
        }
        self.servers.set_field_invalid(Field::AtlasName, false);
        self.atlas.panel.name_error = None;
    }

    /// Picks a PNG for `name`'s skin (or cape).
    fn choose_look_file(&mut self, name: String, cape: bool, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr("Choose").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.pop() else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.atlas_take_look_file(&name, path, cape);
                cx.notify();
            });
        })
        .detach();
    }

    fn atlas_take_look_file(&mut self, name: &str, path: PathBuf, cape: bool) {
        // As the file is now: the name may have been removed while the file dialog was open.
        let profiles = Profiles::load();
        let panel = &mut self.atlas.panel;
        take_look_file(panel, &profiles, &skins::dir(), name, &path, cape);
    }

    /// Back to the default skin, or no cape.
    fn atlas_clear_look(&mut self, name: &str, cape: bool) {
        let done = skins::clear(&skins::dir(), name, cape);
        self.atlas.panel.previews = None;
        self.atlas.panel.note = done.err().map(|e| (name.to_string(), Err(e)));
    }

    fn atlas_set_model(&mut self, name: &str, model: Model) {
        let done = self.atlas_change(|profiles| profiles.set_model(name, model));
        self.atlas.panel.note = done.err().map(|e| (name.to_string(), Err(e)));
    }

    /// `name`'s skin front and cape for `model`, baked again when the name, the model or the
    /// window's scale changed, or a file did here; and whether it has a skin and a cape.
    fn atlas_previews(&mut self, name: &str, model: Model) -> (Preview, Preview, bool, bool) {
        let scale = self.scale;
        let key = (name.to_string(), model, scale.to_bits());
        let panel = &mut self.atlas.panel;
        if panel.previews.as_ref().is_some_and(|p| p.key != key) {
            panel.previews = None;
        }
        let p = panel.previews.get_or_insert_with(|| {
            let (skin_file, cape_file) = skins::files(&skins::dir(), name);
            let read = |f: &PathBuf| std::fs::read(f).ok();
            let skin = read(&skin_file)
                .and_then(|b| skins::read_skin(&b).ok())
                .map(|png| art::bake(&skins::front(&png, model), 16, 32, TEXEL, scale));
            let cape = read(&cape_file)
                .and_then(|b| skins::read_cape(&b).ok())
                .map(|png| art::bake(&skins::cape_front(&png), 10, 16, TEXEL, scale));
            Previews {
                key,
                skin,
                cape,
                has_skin: skin_file.is_file(),
                has_cape: cape_file.is_file(),
            }
        });
        (p.skin.clone(), p.cape.clone(), p.has_skin, p.has_cape)
    }

    /// The glass look's names: a card with each name's chip (0.2.58's `name_chip`), the one that
    /// plays ticked, then the field to add one.
    fn glass_names(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let profiles = self.atlas.profiles.clone();
        let playing = profiles.selected().map(|p| p.name.clone());
        let editing = self.atlas.panel.editing.clone();
        let mut list = div().flex().flex_col().gap(px(8.));
        for (i, profile) in profiles.names.iter().enumerate() {
            let name = profile.name.clone();
            if editing
                .as_deref()
                .is_some_and(|e| e.eq_ignore_ascii_case(&name))
            {
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            self.field(Field::AtlasName, tr("New name"), 160., cx)
                                .flex_1(),
                        )
                        .child(
                            ui::quiet("atlas-rename-save", tr("Save"), 44., &t).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.atlas_name_enter();
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(self.chip_icon(
                            "atlas-rename-cancel".into(),
                            "cancel-01",
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.atlas_end_rename();
                                cx.notify();
                            }),
                        )),
                );
                continue;
            }
            let plays = playing.as_deref() == Some(name.as_str());
            let (who, renamed, gone) = (name.clone(), name.clone(), name.clone());
            list = list.child(
                self.name_chip(format!("atlas-name-{i}"), name, plays)
                    .when(!plays, |d| {
                        d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.atlas_select(&who);
                            cx.notify();
                        }))
                    })
                    .child(self.chip_icon(
                        format!("atlas-rename-{i}").into(),
                        "pencil-edit-02",
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.atlas_start_rename(&renamed);
                            cx.notify();
                        }),
                    ))
                    .child(self.chip_icon(
                        format!("atlas-remove-{i}").into(),
                        "cancel-01",
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.atlas.dialog = Some(AtlasDialog::ConfirmRemove(gone.clone()));
                            cx.notify();
                        }),
                    )),
            );
        }
        // One field: while a name is renamed, adding waits.
        let add = editing.is_none().then(|| {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .mt(px(if profiles.names.is_empty() { 0. } else { 12. }))
                .child(
                    self.field(Field::AtlasName, tr("Add name"), 160., cx)
                        .flex_1(),
                )
                .child(
                    ui::quiet("atlas-add", tr("Add"), 44., &t).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.atlas_name_enter();
                            cx.notify();
                        },
                    )),
                )
        });
        let error = self.atlas.panel.name_error.clone().map(|e| {
            ui::text(13., FontWeight::NORMAL, ink::FULL)
                .mt(px(10.))
                .line_height(px(19.5))
                .text_color(ui::danger())
                .child(e)
        });
        crate::glass::plate(radius::PANEL)
            .flex()
            .flex_col()
            .flex_none()
            .w(px(NAMES_WIDTH))
            .p(px(20.))
            .child(caps(tr("Offline names")).mb(px(12.)))
            .child(list)
            .children(add)
            .children(error)
            .child(
                ui::card_note(tr(
                    "A Minecraft name has 3 to 16 letters, digits or underscores.",
                ))
                .mt(px(10.)),
            )
    }

    /// An offline name's chip: its head and name, a tick on the one playing; white at 14 % for
    /// that one, 5 % (8 % under the pointer) for the others.
    fn name_chip(&self, id: String, name: String, plays: bool) -> Stateful<Div> {
        let t = self.theme;
        let chip = div()
            .id(SharedString::from(id))
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .h(px(44.))
            .pl(px(6.))
            .pr(px(6.))
            .rounded(px(radius::ROW))
            .when(plays, |d| d.bg(white(0.14)))
            .when(!plays, |d| {
                d.bg(white(0.05))
                    .cursor_pointer()
                    .hover(|s| s.bg(white(0.08)))
            })
            .child(
                div()
                    .flex_none()
                    .size(px(30.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .child(self.profile_heads.draw(None, 30., self.scale, &t)),
            )
            .child(
                ui::text(14., FontWeight::MEDIUM, ink::FULL)
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(name),
            )
            .when(plays, |d| {
                d.child(ui::icon("tick-02", 16., ui::color(ui::look().accent_soft)))
            });
        keyboard(chip, &t)
    }

    /// A 28 px button in a chip (`size-7 rounded-[8px]`), white at 45 %.
    fn chip_icon(
        &self,
        id: SharedString,
        icon: &'static str,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Stateful<Div> {
        keyboard(
            div()
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(|s| s.bg(white(fill::BUTTON)))
                .child(ui::icon(icon, 14., white(0.45))),
            &self.theme,
        )
        .on_click(on_click)
    }

    /// The blocky look's names, in a column as wide as the glass card: a row each (its head, its
    /// name, "In use" on the one that plays, and Play as, Rename and Remove under it), then the row
    /// to add one.
    fn classic_names(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let profiles = self.atlas.profiles.clone();
        let playing = profiles.selected().map(|p| p.name.clone());
        let editing = self.atlas.panel.editing.clone();
        let head = |this: &Self| {
            div()
                .flex_none()
                .size(px(28.))
                .overflow_hidden()
                .child(this.profile_heads.draw(None, 28., this.scale, &t))
        };
        let mut rows = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(NAMES_WIDTH))
            .child(section_title(tr("Offline names"), "user", &t).mb(px(12.)));
        for (i, profile) in profiles.names.iter().enumerate() {
            let name = profile.name.clone();
            let plays = playing.as_deref() == Some(name.as_str());
            let actions = if editing
                .as_deref()
                .is_some_and(|e| e.eq_ignore_ascii_case(&name))
            {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        self.field(Field::AtlasName, tr("New name"), 280., cx)
                            .w_full(),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(4.))
                            .child(
                                button("atlas-rename-save", tr("Save"), Kind::Secondary, &t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.atlas_name_enter();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button("atlas-rename-cancel", tr("Cancel"), Kind::Ghost, &t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.atlas_end_rename();
                                        cx.notify();
                                    })),
                            ),
                    )
            } else {
                let (who, renamed, gone) = (name.clone(), name.clone(), name.clone());
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .when(!plays, |d| {
                        d.child(
                            button(
                                format!("atlas-play-{i}"),
                                tr("Play as"),
                                Kind::Secondary,
                                &t,
                            )
                            .on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.atlas_select(&who);
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .child(
                        button(format!("atlas-rename-{i}"), tr("Rename"), Kind::Ghost, &t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.atlas_start_rename(&renamed);
                                cx.notify();
                            })),
                    )
                    .child(
                        button(format!("atlas-remove-{i}"), tr("Remove"), Kind::Ghost, &t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.atlas.dialog = Some(AtlasDialog::ConfirmRemove(gone.clone()));
                                cx.notify();
                            })),
                    )
            };
            let detail = plays.then(|| tr("In use"));
            rows = rows.child(classic_row(Some(head(self)), name, detail, actions, &t));
        }
        if editing.is_none() {
            let add = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    self.field(Field::AtlasName, tr("Add name"), 190., cx)
                        .flex_1(),
                )
                .child(
                    button("atlas-add", tr("Add"), Kind::Secondary, &t).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.atlas_name_enter();
                            cx.notify();
                        },
                    )),
                );
            let rule = tr("A Minecraft name has 3 to 16 letters, digits or underscores.");
            rows = rows.child(classic_row(None, tr("Add name"), Some(rule), add, &t));
        }
        rows.children(self.atlas.panel.name_error.clone().map(|e| notice(e, &t)))
    }

    /// The selected name's skin and cape (0.2.58's skin panel): the previews, choosing a PNG for
    /// either or going back to none, and the arms' model.
    fn look_card(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let card = match t.glass {
            true => crate::glass::plate(radius::PANEL).p(px(20.)),
            false => plate(&t).px(px(16.)).py(px(12.)),
        }
        .flex()
        .flex_col()
        .gap(px(12.))
        .flex_1()
        .min_w(px(420.));
        let Some(profile) = self.atlas.profiles.selected().cloned() else {
            return card.child(
                text(theme::CAPTION)
                    .text_color(t.muted)
                    .child(tr("Add a name to choose its skin and cape.")),
            );
        };
        let name = profile.name.clone();
        let (skin, cape, has_skin, has_cape) = self.atlas_previews(&name, profile.model);
        let frame = |w: f32, h: f32, image: Preview, empty: &'static str| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(px(w + 24.))
                .h(px(h + 24.))
                .bg(t.well)
                .border_2()
                .border_color(t.outline)
                .when(t.glass, |d| d.rounded(px(radius::ROW)))
                .child(match image {
                    Some(i) => gpui::img(i).w(px(w)).h(px(h)).into_any_element(),
                    None => text(theme::CAPTION)
                        .text_color(t.muted)
                        .child(empty)
                        .into_any_element(),
                })
        };
        let weak = cx.entity().downgrade();
        let modelled = name.clone();
        let model = segments(
            "atlas-model",
            &[
                (Model::Classic, tr("Classic arms")),
                (Model::Slim, tr("Slim arms")),
            ],
            profile.model,
            &t,
            move |m, _, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.atlas_set_model(&modelled, m);
                    cx.notify();
                });
            },
        );
        let pick = |id: &'static str, label: &'static str, cape: bool, cx: &mut Context<Self>| {
            let name = name.clone();
            button(id, label, Kind::Secondary, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.choose_look_file(name.clone(), cape, cx);
                    cx.notify();
                },
            ))
        };
        let drop = |id: &'static str, label: &'static str, cape: bool, cx: &mut Context<Self>| {
            let name = name.clone();
            button(id, label, Kind::Ghost, &t).on_click(cx.listener(
                move |this, _: &ClickEvent, _, cx| {
                    this.atlas_clear_look(&name, cape);
                    cx.notify();
                },
            ))
        };
        let skin_buttons = div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(pick("atlas-skin-choose", tr("Choose skin…"), false, cx))
            .when(has_skin, |d| {
                d.child(drop("atlas-skin-remove", tr("Remove skin"), false, cx))
            });
        let cape_buttons = div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(pick("atlas-cape-choose", tr("Choose cape…"), true, cx))
            .when(has_cape, |d| {
                d.child(drop("atlas-cape-remove", tr("Remove cape"), true, cx))
            });
        let note = self.atlas.panel.note_for(&name).cloned().map(|n| match n {
            Ok(said) => text(theme::CAPTION).text_color(t.muted).child(said),
            Err(e) => text(theme::CAPTION).text_color(t.danger).child(e),
        });
        card.child(
            div()
                .flex()
                .gap(px(16.))
                .child(frame(16. * TEXEL, 32. * TEXEL, skin, tr("Default")))
                .child(frame(10. * TEXEL, 16. * TEXEL, cape, tr("No cape")))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .flex_1()
                        .min_w_0()
                        .gap(px(10.))
                        .child(caps(trf("{name}'s skin", &[("name", &name)])).text_color(t.muted))
                        .child(skin_buttons)
                        .child(model)
                        .child(cape_buttons)
                        .children(note),
                ),
        )
        .child(
            text(theme::CAPTION)
                .text_color(t.muted)
                .line_height(px(16.))
                .child(tr(
                    "A 64×64 (or 64×32) skin and a 64×32 (or 22×17) cape, as PNG pictures. The game shows them while this name plays.",
                )),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_ui::tests::Scratch;
    use horizon_launcher::offline::profiles::Plays;

    fn names(profiles: &Profiles) -> Vec<&str> {
        profiles.names.iter().map(|p| p.name.as_str()).collect()
    }

    /// A PNG of `w`×`h` at `path`, each texel its own colour.
    fn picture(path: &Path, w: u32, h: u32) {
        let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
        img.save_with_format(path, image::ImageFormat::Png).unwrap();
    }

    #[test]
    fn a_new_name_from_the_field_is_validated_and_added() {
        let dir = Scratch::new("add");
        let mut profiles = Profiles::default();
        assert_eq!(
            enter_name(&mut profiles, &dir.0, None, "  Steve "),
            Ok(true)
        );
        assert_eq!(names(&profiles), ["Steve"]);
        assert_eq!(profiles.plays, Some(Plays::Offline("Steve".into())));
        for refused in ["ab", "a_name_far_too_long", "no spaces", "Stéve", "steve"] {
            assert!(
                enter_name(&mut profiles, &dir.0, None, refused).is_err(),
                "{refused}"
            );
        }
        assert_eq!(names(&profiles), ["Steve"]);
        assert_eq!(enter_name(&mut profiles, &dir.0, None, "   "), Ok(false));
        assert_eq!(enter_name(&mut profiles, &dir.0, None, "Alex"), Ok(true));
        assert_eq!(names(&profiles), ["Steve", "Alex"]);
        // A second name doesn't take over who plays.
        assert_eq!(profiles.plays, Some(Plays::Offline("Steve".into())));
    }

    #[test]
    fn the_name_being_edited_is_renamed_with_its_skin() {
        let dir = Scratch::new("rename");
        let skins_dir = dir.0.join("skins");
        let mut profiles = Profiles::default();
        profiles.add("Steve").unwrap();
        profiles.add("Alex").unwrap();
        let file = dir.0.join("skin.png");
        picture(&file, 64, 64);
        skins::keep(&skins_dir, "Steve", &file, false).unwrap();
        // Taken by another name: refused, nothing moves.
        assert!(enter_name(&mut profiles, &skins_dir, Some("Steve"), "alex").is_err());
        assert!(skins::files(&skins_dir, "Steve").0.is_file());
        assert_eq!(
            enter_name(&mut profiles, &skins_dir, Some("Steve"), "Notch"),
            Ok(true)
        );
        assert_eq!(names(&profiles), ["Notch", "Alex"]);
        assert_eq!(profiles.plays, Some(Plays::Offline("Notch".into())));
        assert!(skins::files(&skins_dir, "Notch").0.is_file());
        assert!(!skins::files(&skins_dir, "Steve").0.exists());
    }

    #[test]
    fn a_rejected_file_leaves_the_old_skin() {
        let dir = Scratch::new("rejected");
        let skins_dir = dir.0.join("skins");
        let mut profiles = Profiles::default();
        profiles.add("Steve").unwrap();
        let (good, small) = (dir.0.join("good.png"), dir.0.join("small.png"));
        picture(&good, 64, 64);
        picture(&small, 32, 32);
        let mut panel = ProfilePanel::default();
        take_look_file(&mut panel, &profiles, &skins_dir, "Steve", &good, false);
        assert!(
            matches!(panel.note_for("Steve"), Some(Ok(_))),
            "{:?}",
            panel.note
        );
        let (skin, cape) = skins::files(&skins_dir, "Steve");
        let before = std::fs::read(&skin).unwrap();

        take_look_file(&mut panel, &profiles, &skins_dir, "Steve", &small, false);
        assert!(
            matches!(panel.note_for("Steve"), Some(Err(_))),
            "{:?}",
            panel.note
        );
        assert_eq!(std::fs::read(&skin).unwrap(), before);
        // A skin is no cape: refused, and no cape appears.
        take_look_file(&mut panel, &profiles, &skins_dir, "Steve", &good, true);
        assert!(
            matches!(panel.note_for("Steve"), Some(Err(_))),
            "{:?}",
            panel.note
        );
        assert!(!cape.exists());
        // A name forgotten while the file dialog was open gets nothing.
        panel.note = None;
        take_look_file(&mut panel, &profiles, &skins_dir, "Ghost", &good, false);
        assert_eq!(panel.note, None);
        assert!(!skins::files(&skins_dir, "Ghost").0.exists());
    }

    #[test]
    fn a_look_note_shows_only_under_its_own_name() {
        let dir = Scratch::new("note");
        let mut profiles = Profiles::default();
        profiles.add("Steve").unwrap();
        profiles.add("Alex").unwrap();
        let small = dir.0.join("small.png");
        picture(&small, 32, 32);
        let mut panel = ProfilePanel::default();
        take_look_file(&mut panel, &profiles, &dir.0, "steve", &small, false);
        // Kept for the name as the list spells it, found in any letters.
        assert!(matches!(panel.note_for("Steve"), Some(Err(_))));
        assert!(matches!(panel.note_for("STEVE"), Some(Err(_))));
        // Alex plays now: Steve's refusal is not shown under Alex's skin.
        assert_eq!(panel.note_for("Alex"), None);
    }
}
