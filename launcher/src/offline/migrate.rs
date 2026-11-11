//! The move from Atlas 0.1.x. It kept its offline names in the shared `accounts.json` (`offline`,
//! `offline_selected`, and each name's look in `looks`) and their pictures in the shared `skins/`;
//! the official launcher's current code drops both on its next save. The first start of 0.2 copies
//! them into Atlas's folder, once, and never writes or removes anything in the shared ones.
use super::profiles::{Plays, Profile, Profiles};
use super::skins::{self, Model};
use serde_json::Value;
use std::path::Path;

/// Moves the names, models, selection, skins and capes of 0.1.x in the data folder `data` into
/// `data/atlas`. `Ok(false)`: it was done before (`atlas/offline.json` is there), nothing happens.
/// A missing or unreadable `accounts.json` is no names. The file is written last: only a move that
/// failed to write it is tried again at the next start.
pub fn migrate(data: &Path) -> Result<bool, String> {
    let target = Profiles::path_in(data);
    if target.exists() {
        return Ok(false);
    }
    let old: Value = std::fs::read_to_string(data.join("accounts.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let looks = old.get("looks").and_then(Value::as_object);
    let (from, to) = (data.join("skins"), skins::dir_in(data));

    let mut profiles = Profiles::default();
    let listed = old.get("offline").and_then(Value::as_array);
    for name in listed.into_iter().flatten().filter_map(Value::as_str) {
        if name.trim().is_empty() || profiles.find(name).is_some() {
            continue;
        }
        // `looks` is keyed by the name in lowercase.
        let look = looks
            .and_then(|looks| looks.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)))
            .map(|(_, look)| look);
        let field = |key: &str| look.and_then(|look| look.get(key));
        let model = match field("model").and_then(Value::as_str) {
            Some("slim") => Model::Slim,
            _ => Model::Classic,
        };
        profiles.names.push(Profile {
            name: name.to_string(),
            model,
        });
        // A look's flag says it has the picture: "Default skin" cleared the flag, not the file.
        let (old_skin, old_cape) = skins::files(&from, name);
        let (new_skin, new_cape) = skins::files(&to, name);
        let has = |key: &str| field(key).and_then(Value::as_bool).unwrap_or(false);
        for (wanted, old, new) in [
            (has("skin"), old_skin, new_skin),
            (has("cape"), old_cape, new_cape),
        ] {
            if wanted && old.is_file() {
                if let Err(e) =
                    std::fs::create_dir_all(&to).and_then(|()| std::fs::copy(&old, &new))
                {
                    eprintln!(
                        "Horizon: {} is not moved to Atlas's folder: {e}",
                        old.display()
                    );
                }
            }
        }
    }

    let chosen = old
        .get("offline_selected")
        .and_then(Value::as_str)
        .and_then(|name| profiles.find(name))
        .or(profiles.names.first());
    profiles.plays = chosen.map(|p| Plays::Offline(p.name.clone()));
    profiles.save_to(&target)?;
    Ok(true)
}
