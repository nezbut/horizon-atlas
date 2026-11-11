//! The offline names, each with its skin model, in the order the player keeps them, and who
//! plays: `atlas/offline.json`. Atlas's account file is a different one, because upstream's code
//! rewrites that in its own format and drops what it doesn't know.
use super::skins::{self, Model};
use crate::i18n::trf;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Whether `name` can be an offline name: 3 to 16 letters, digits or `_`, as account names are.
pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// An offline name and the arms its skin is made for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub model: Model,
}

/// Who plays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "who", rename_all = "lowercase")]
pub enum Plays {
    /// An offline name, as it is kept in the list.
    Offline(String),
    /// A signed-in Microsoft account, by UUID. Reserved: no build of Atlas has a sign-in yet.
    Microsoft(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub names: Vec<Profile>,
    /// None: nobody was chosen yet.
    pub plays: Option<Plays>,
}

/// What a refused action says when the name isn't in the list.
pub fn missing(name: &str) -> String {
    trf("There is no name {name}.", &[("name", name)])
}

fn taken(name: &str) -> String {
    trf("The name {name} is already in the list.", &[("name", name)])
}

fn invalid(name: &str) -> Result<(), String> {
    match valid_name(name) {
        true => Ok(()),
        false => Err(trf(
            "{name}: a name is 3 to 16 letters, digits or _",
            &[("name", name)],
        )),
    }
}

impl Profiles {
    /// Atlas's `offline.json` in the data folder `data`.
    pub fn path_in(data: &Path) -> PathBuf {
        super::atlas_in(data).join("offline.json")
    }

    pub fn path() -> PathBuf {
        Self::path_in(&crate::backend::data_dir())
    }

    /// The profiles in the data folder `data`, once the move from 0.1.x has run (`migrate`). A move
    /// that fails is logged, never fatal: the old data stays where it is for the next start.
    pub fn load_in(data: &Path) -> Self {
        if let Err(e) = super::migrate::migrate(data) {
            eprintln!("Horizon: the offline names are not moved to Atlas's folder: {e}");
        }
        Self::load_from(&Self::path_in(data))
    }

    pub fn load() -> Self {
        Self::load_in(&crate::backend::data_dir())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    /// The file's profiles; a missing or unreadable file is no profiles. An unreadable one is
    /// logged and copied aside first (`offline.json.unreadable`): the file itself stays.
    pub fn load_from(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => return Self::unreadable(path, &e.to_string()),
        };
        serde_json::from_str(&text).unwrap_or_else(|e| Self::unreadable(path, &e.to_string()))
    }

    /// No profiles, for a file that is there but can't be read (a typo in a hand edit, a "who
    /// plays" a newer Atlas wrote). The next save replaces it, so it is first copied beside it as
    /// `offline.json.unreadable` (a later copy replaces an earlier one). It is never renamed or
    /// deleted: the move from 0.1.x must keep seeing it, or it would run again.
    fn unreadable(path: &Path, why: &str) -> Self {
        let mut aside = path.as_os_str().to_owned();
        aside.push(".unreadable");
        let aside = PathBuf::from(aside);
        match std::fs::copy(path, &aside) {
            Ok(_) => eprintln!(
                "Horizon: {} can't be read ({why}); a copy is kept as {}.",
                path.display(),
                aside.display()
            ),
            Err(e) => eprintln!(
                "Horizon: {} can't be read ({why}), and a copy can't be kept: {e}",
                path.display()
            ),
        }
        Self::default()
    }

    /// Replaces the file whole, its folder made if need be: a write that stops halfway never
    /// leaves a file that reads as no names.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())? + "\n";
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, text)
            .and_then(|()| std::fs::rename(&tmp, path))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("{}: {e}", path.display())
            })
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.names
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// The profile named `name`, in any letters.
    pub fn find(&self, name: &str) -> Option<&Profile> {
        self.position(name).map(|i| &self.names[i])
    }

    /// Keeps a new name (once, in any case). The first one added while nobody was chosen plays.
    pub fn add(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        invalid(name)?;
        if let Some(kept) = self.find(name) {
            return Err(taken(&kept.name));
        }
        self.names.push(Profile {
            name: name.to_string(),
            model: Model::default(),
        });
        if self.plays.is_none() {
            self.plays = Some(Plays::Offline(name.to_string()));
        }
        Ok(())
    }

    /// Renames `old` to `new`, its skin and cape in `skins` with it. A change of case alone
    /// (`steve` to `Steve`) is a rename too. The name that plays keeps playing.
    pub fn rename(&mut self, skins: &Path, old: &str, new: &str) -> Result<(), String> {
        let new = new.trim();
        let i = self.position(old).ok_or_else(|| missing(old))?;
        invalid(new)?;
        let other = self
            .names
            .iter()
            .enumerate()
            .find(|(j, p)| *j != i && p.name.eq_ignore_ascii_case(new));
        if let Some((_, other)) = other {
            return Err(taken(&other.name));
        }
        let was = self.names[i].name.clone();
        skins::rename(skins, &was, new)?;
        self.names[i].name = new.to_string();
        if let Some(Plays::Offline(playing)) = &mut self.plays {
            if playing.eq_ignore_ascii_case(&was) {
                *playing = new.to_string();
            }
        }
        Ok(())
    }

    /// Forgets `name` and drops its skin and cape from `skins`; a name that isn't there is no
    /// error. If the files can't be dropped the name stays, still playing if it played, and the
    /// error is returned. If it played, the name after it does (the one before, if it was last),
    /// or nobody when none is left.
    pub fn remove(&mut self, skins: &Path, name: &str) -> Result<(), String> {
        let Some(i) = self.position(name) else {
            return Ok(());
        };
        skins::forget(skins, &self.names[i].name)?;
        let gone = self.names.remove(i);
        if matches!(&self.plays, Some(Plays::Offline(n)) if n.eq_ignore_ascii_case(&gone.name)) {
            self.plays = self
                .names
                .get(i)
                .or(self.names.last())
                .map(|p| Plays::Offline(p.name.clone()));
        }
        Ok(())
    }

    /// Makes `name` the one that plays.
    pub fn select(&mut self, name: &str) -> Result<(), String> {
        let kept = self.find(name).ok_or_else(|| missing(name))?;
        self.plays = Some(Plays::Offline(kept.name.clone()));
        Ok(())
    }

    /// The offline name that plays.
    pub fn selected(&self) -> Option<&Profile> {
        match &self.plays {
            Some(Plays::Offline(name)) => self.find(name),
            _ => None,
        }
    }

    /// Whether the game starts offline: unless a Microsoft account was chosen to play.
    pub fn plays_offline(&self) -> bool {
        !matches!(self.plays, Some(Plays::Microsoft(_)))
    }

    pub fn set_model(&mut self, name: &str, model: Model) -> Result<(), String> {
        let i = self.position(name).ok_or_else(|| missing(name))?;
        self.names[i].model = model;
        Ok(())
    }
}
