//! What the game is started with: the selected offline name, its skin and its cape.
use super::*;
use crate::offline::session::session_in;

/// Atlas's profiles in `data`: these names, `playing` of them selected, `Alex` slim.
fn profiles_in(data: &Path, list: &[&str], playing: Option<&str>) -> Profiles {
    let mut profiles = Profiles::default();
    for name in list {
        profiles.add(name).unwrap();
    }
    if let Some(name) = playing {
        profiles.select(name).unwrap();
    }
    if profiles.find("Alex").is_some() {
        profiles.set_model("Alex", Model::Slim).unwrap();
    }
    profiles.save_to(&Profiles::path_in(data)).unwrap();
    profiles
}

fn var(name: &'static str, value: &Path) -> (&'static str, String) {
    (name, value.to_string_lossy().into_owned())
}

#[test]
fn an_offline_name_starts_the_game_with_its_name_skin_and_cape() {
    let data = Scratch::new("session-look");
    let (skin_png, cape_png) = pictures(&data);
    profiles_in(&data, &["Steve", "Alex"], Some("Alex"));
    let dir = skins::dir_in(&data);
    skins::keep(&dir, "Alex", &skin_png, false).unwrap();
    skins::keep(&dir, "Alex", &cape_png, true).unwrap();
    // The other name's picture isn't the game's.
    skins::keep(&dir, "Steve", &cape_png, true).unwrap();
    let (skin, cape) = skins::files(&dir, "Alex");

    let session = session_in(&data).expect("an offline name plays");
    assert_eq!(session.file, None, "no account session");
    assert_eq!(session.offline, None, "nothing to explain");
    assert_eq!(
        session.env,
        [
            ("HORIZON_PLAYER_NAME", "Alex".to_string()),
            var("HORIZON_OFFLINE_SKIN", &skin),
            ("HORIZON_OFFLINE_SKIN_MODEL", "slim".to_string()),
            var("HORIZON_OFFLINE_CAPE", &cape),
        ]
    );
}

#[test]
fn a_name_without_pictures_starts_the_game_with_its_name_only() {
    let data = Scratch::new("session-plain");
    profiles_in(&data, &["Steve"], None);
    let session = session_in(&data).expect("an offline name plays");
    assert_eq!(session.env, [("HORIZON_PLAYER_NAME", "Steve".to_string())]);
}

#[test]
fn nobody_selected_starts_the_game_without_a_name() {
    // The game then keeps the name it made up (`horizon-player.txt`): nothing to set.
    let data = Scratch::new("session-nobody");
    profiles_in(&data, &[], None);
    let session = session_in(&data).expect("offline play needs no name");
    assert_eq!((session.file, session.offline), (None, None));
    assert!(session.env.is_empty());

    // A fresh install has no file at all: the same.
    let fresh = Scratch::new("session-fresh");
    assert!(session_in(&fresh).unwrap().env.is_empty());
}

#[test]
fn a_microsoft_choice_leaves_the_session_to_upstream() {
    let data = Scratch::new("session-microsoft");
    let mut profiles = profiles_in(&data, &["Steve"], None);
    profiles.plays = Some(Plays::Microsoft("069a79f444e94726a5befca90e38aaf5".into()));
    profiles.save_to(&Profiles::path_in(&data)).unwrap();
    assert!(session_in(&data).is_none());
}

#[test]
fn the_first_start_plays_with_the_names_of_0_1() {
    // A 0.1.x player's first start is the CLI or the game, not the window: the move runs there too.
    let data = Scratch::new("session-moved");
    std::fs::write(
        data.join("accounts.json"),
        r#"{"offline": ["nezbut", "Player"], "offline_selected": "nezbut",
            "looks": {"nezbut": {"model": "slim", "skin": true}}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(data.join("skins")).unwrap();
    std::fs::write(data.join("skins/nezbut.png"), png(&mapped(64))).unwrap();

    let session = session_in(&data).unwrap();
    let (skin, _) = skins::files(&skins::dir_in(&data), "nezbut");
    assert_eq!(
        session.env,
        [
            ("HORIZON_PLAYER_NAME", "nezbut".to_string()),
            var("HORIZON_OFFLINE_SKIN", &skin),
            ("HORIZON_OFFLINE_SKIN_MODEL", "slim".to_string()),
        ]
    );
}
