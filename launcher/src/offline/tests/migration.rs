//! The move from 0.1.x: its names, skins and selection, out of the shared `accounts.json` and
//! `skins/` that the official launcher's code may delete from.
use super::*;
use crate::offline::migrate::migrate;
use crate::offline::profiles::Profile;

/// What a 0.1.x launcher kept: two names, nezbut's look and who plays.
const OLD: &str = r#"{
    "accounts": [],
    "selected": null,
    "offline": ["nezbut", "Player"],
    "offline_selected": "Player",
    "looks": {"nezbut": {"model": "slim", "skin": true, "cape": true}}
}"#;

/// A 0.1.x data folder: `accounts` as its accounts.json, and nezbut's skin and cape in `skins/`.
fn old_launcher(name: &str, accounts: &str) -> Scratch {
    let data = Scratch::new(name);
    std::fs::create_dir_all(data.join("skins")).unwrap();
    std::fs::write(data.join("accounts.json"), accounts).unwrap();
    std::fs::write(data.join("skins/nezbut.png"), png(&mapped(64))).unwrap();
    std::fs::write(data.join("skins/nezbut.cape.png"), png(&mapped(32))).unwrap();
    data
}

/// The shared files a move must leave alone, with their bytes.
fn shared(data: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = vec![data.join("accounts.json")];
    if let Ok(skins) = std::fs::read_dir(data.join("skins")) {
        files.extend(skins.map(|e| e.unwrap().path()));
    }
    files.sort();
    files
        .into_iter()
        .map(|f| {
            let bytes = std::fs::read(&f).unwrap_or_default();
            (f, bytes)
        })
        .collect()
}

fn migrated(data: &Path) -> Profiles {
    Profiles::load_from(&Profiles::path_in(data))
}

/// Where `name`'s skin and cape are kept in Atlas's folder.
fn kept(data: &Path, name: &str) -> (PathBuf, PathBuf) {
    skins::files(&skins::dir_in(data), name)
}

fn profile(name: &str, model: Model) -> Profile {
    Profile {
        name: name.to_string(),
        model,
    }
}

#[test]
fn old_names_skins_and_selection_move_over() {
    let data = old_launcher("move-over", OLD);
    let before = shared(&data);

    assert_eq!(migrate(&data), Ok(true));
    let profiles = migrated(&data);
    assert_eq!(
        profiles.names,
        [
            profile("nezbut", Model::Slim),
            profile("Player", Model::Classic)
        ]
    );
    assert_eq!(profiles.plays, playing("Player"));
    let (skin, cape) = kept(&data, "nezbut");
    assert_eq!(std::fs::read(skin).unwrap(), png(&mapped(64)));
    assert_eq!(std::fs::read(cape).unwrap(), png(&mapped(32)));
    let player = kept(&data, "Player");
    assert!(!player.0.exists() && !player.1.exists());

    assert_eq!(shared(&data), before, "the shared files are as they were");
}

#[test]
fn a_microsoft_selection_still_moves_the_offline_choice() {
    let accounts = r#"{
        "accounts": [{"uuid": "069a79f444e94726a5befca90e38aaf5", "name": "Notch"}],
        "selected": "069a79f444e94726a5befca90e38aaf5",
        "offline": ["nezbut", "Player"],
        "offline_selected": "Player"
    }"#;
    let data = old_launcher("move-microsoft", accounts);
    assert_eq!(migrate(&data), Ok(true));
    assert_eq!(migrated(&data).plays, playing("Player"));
}

#[test]
fn without_a_valid_choice_the_first_name_plays() {
    for chosen in [
        r#""offline_selected": null,"#,
        r#""offline_selected": "Gone_1","#,
        "",
    ] {
        let accounts = format!(r#"{{ {chosen} "offline": ["nezbut", "Player"] }}"#);
        let data = old_launcher("move-first", &accounts);
        assert_eq!(migrate(&data), Ok(true));
        assert_eq!(migrated(&data).plays, playing("nezbut"), "{chosen}");
    }
}

#[test]
fn missing_skin_files_are_skipped() {
    // Every look claims a skin and a cape; only nezbut's skin is there, and Player has neither.
    let accounts = r#"{
        "offline": ["nezbut", "Player", "Ghost_1"],
        "offline_selected": "Ghost_1",
        "looks": {
            "nezbut": {"model": "slim", "skin": true, "cape": true},
            "player": {"skin": true, "cape": true},
            "ghost_1": {"model": "slim", "skin": true}
        }
    }"#;
    let data = old_launcher("move-missing", accounts);
    std::fs::remove_file(data.join("skins/nezbut.cape.png")).unwrap();

    assert_eq!(migrate(&data), Ok(true));
    let profiles = migrated(&data);
    let list: Vec<_> = profiles.names.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(list, ["nezbut", "Player", "Ghost_1"], "every name is kept");
    assert_eq!(profiles.plays, playing("Ghost_1"));
    assert_eq!(profiles.names[2].model, Model::Slim);
    let (skin, cape) = kept(&data, "nezbut");
    assert!(skin.is_file() && !cape.exists());
    for name in ["Player", "Ghost_1"] {
        let (skin, cape) = kept(&data, name);
        assert!(!skin.exists() && !cape.exists(), "{name}");
    }
}

#[test]
fn a_look_belongs_to_its_name_in_any_letters() {
    let accounts = r#"{
        "offline": ["NezBut", "Player"],
        "offline_selected": "player",
        "looks": {"NEZBUT": {"model": "slim", "skin": true, "cape": true}}
    }"#;
    let data = old_launcher("move-letters", accounts);
    assert_eq!(migrate(&data), Ok(true));
    let profiles = migrated(&data);
    assert_eq!(profiles.names[0], profile("NezBut", Model::Slim));
    assert_eq!(profiles.plays, playing("Player"), "as the list has it");
    let (skin, cape) = kept(&data, "nezbut");
    assert!(skin.is_file() && cape.is_file());
}

#[test]
fn a_skin_the_player_took_off_does_not_come_back() {
    // "Default skin" in 0.1.x cleared the look's flag and left the picture where it was.
    let accounts = r#"{
        "offline": ["nezbut"],
        "looks": {"nezbut": {"model": "slim", "cape": true}}
    }"#;
    let data = old_launcher("move-cleared", accounts);
    assert_eq!(migrate(&data), Ok(true));
    let (skin, cape) = kept(&data, "nezbut");
    assert!(!skin.exists(), "no skin on the look: no skin");
    assert!(cape.is_file());
}

#[test]
fn it_runs_once() {
    let data = old_launcher("move-once", OLD);
    assert_eq!(migrate(&data), Ok(true));

    // Edits after the move stay, whatever the old files say or lose later.
    let mut profiles = migrated(&data);
    profiles.add("Extra_1").unwrap();
    profiles.select("Extra_1").unwrap();
    profiles.remove(&skins::dir_in(&data), "nezbut").unwrap();
    profiles.save_to(&Profiles::path_in(&data)).unwrap();
    std::fs::write(data.join("accounts.json"), r#"{"offline": ["Other_1"]}"#).unwrap();

    assert_eq!(migrate(&data), Ok(false));
    assert_eq!(migrated(&data), profiles);
    assert!(!kept(&data, "nezbut").0.exists(), "no copy again");
    assert_eq!(Profiles::load_in(&data), profiles);
}

#[test]
fn no_old_file_makes_an_empty_profile_file() {
    let data = Scratch::new("move-none");
    assert_eq!(migrate(&data), Ok(true));
    assert!(Profiles::path_in(&data).is_file());
    assert_eq!(migrated(&data), Profiles::default());
    assert_eq!(migrate(&data), Ok(false));
    assert!(
        !data.join("accounts.json").exists(),
        "it never writes the shared file"
    );
    assert!(!data.join("skins").exists());
}

#[test]
fn a_broken_old_file_is_not_fatal() {
    let wrecks = [
        "{ not json",
        "",
        "[]",
        r#"{"offline": "nezbut"}"#,
        r#"{"offline": {"nezbut": true}, "looks": []}"#,
        r#"{"offline": [], "looks": 5}"#,
    ];
    for old in wrecks {
        let data = old_launcher("move-broken", old);
        let before = shared(&data);
        assert_eq!(migrate(&data), Ok(true), "{old}");
        assert_eq!(migrated(&data), Profiles::default(), "{old}");
        assert_eq!(shared(&data), before, "{old}");
    }

    // What can be read is kept: a name that is no text goes, the rest stays.
    let data = old_launcher(
        "move-partly",
        r#"{"offline": [1, null, "Ann", {}], "looks": 3}"#,
    );
    assert_eq!(migrate(&data), Ok(true));
    assert_eq!(migrated(&data).names, [profile("Ann", Model::Classic)]);
}

#[test]
fn odd_names_are_kept_once_and_stay_in_atlas_s_folder() {
    // A hand-edited list: a repeat in other letters, a blank, a name that is no file name.
    let accounts = r#"{
        "offline": ["Ann", "ANN", "  ", "../evil", "Bob"],
        "offline_selected": "../evil",
        "looks": {"../evil": {"skin": true, "cape": true}}
    }"#;
    let data = old_launcher("move-odd", accounts);
    // What `skins/../evil.png` would be, were the name taken for a path.
    std::fs::write(data.join("evil.png"), png(&mapped(64))).unwrap();
    assert_eq!(migrate(&data), Ok(true));
    let profiles = migrated(&data);
    let list: Vec<_> = profiles.names.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(list, ["Ann", "../evil", "Bob"]);
    assert_eq!(profiles.plays, playing("../evil"));

    let entries = |dir: &Path| {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        entries(&data),
        ["accounts.json", "atlas", "evil.png", "skins"]
    );
    assert_eq!(entries(&data.join("atlas")), ["offline.json"]);
}

#[test]
fn loading_moves_first_and_a_failed_move_is_only_logged() {
    let data = old_launcher("move-load", OLD);
    assert_eq!(Profiles::load_in(&data).names.len(), 2);
    assert!(Profiles::path_in(&data).is_file());

    // Atlas's folder can't be made: nothing is moved, and the start goes on with no names.
    let blocked = old_launcher("move-blocked", OLD);
    std::fs::write(blocked.join("atlas"), "in the way").unwrap();
    assert!(migrate(&blocked).is_err());
    assert_eq!(Profiles::load_in(&blocked), Profiles::default());
    assert_eq!(
        std::fs::read_to_string(blocked.join("accounts.json")).unwrap(),
        OLD
    );
}
