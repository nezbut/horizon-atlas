use super::profiles::{valid_name, Plays, Profiles};
use super::skins::{self, cape_front, front, read_cape, read_skin, Model};
use crate::account::{Accounts, SERVICE};
use std::path::{Path, PathBuf};

mod cli;
mod migration;
mod session;

// That Atlas's accounts file is its own (`Accounts::path`) is checked from outside, on the real
// binary with its own environment: `launcher/tests/owner_gate.rs`. No test here sets the
// process's environment.

#[test]
fn atlas_keeps_its_secrets_under_its_own_keychain_service() {
    assert_eq!(SERVICE, "io.github.nezbut.horizon-atlas");
}

/// A folder of the test's own, empty at the start and gone at the end; no environment involved.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("horizon-atlas-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ----- Skins and capes -----------------------------------------------------------------------------

/// A skin whose every texel says where it is: red = x, green = y.
fn mapped(height: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(64, height, |x, y| image::Rgba([x as u8, y as u8, 0, 255]))
}

fn png(img: &image::RgbaImage) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A skin picture and a cape picture on disk in `dir`, for `skins::keep`.
fn pictures(dir: &Path) -> (PathBuf, PathBuf) {
    let (skin, cape) = (dir.join("a-skin.png"), dir.join("a-cape.png"));
    std::fs::write(&skin, png(&mapped(64))).unwrap();
    std::fs::write(&cape, png(&mapped(32))).unwrap();
    (skin, cape)
}

#[test]
fn the_front_view_takes_each_part_from_its_place() {
    let mut skin = mapped(64);
    // No outer layer, except one hat texel.
    for (x, y) in (0..64).flat_map(|x| (0..64).map(move |y| (x, y))) {
        let outer = (40..48).contains(&x) && y < 16
            || (16..56).contains(&x) && (32..48).contains(&y)
            || (0..16).contains(&x) && (48..64).contains(&y)
            || (48..64).contains(&x) && (48..64).contains(&y);
        if outer {
            skin.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        }
    }
    skin.put_pixel(41, 9, image::Rgba([200, 0, 9, 255]));
    let f = front(&skin, Model::Classic);
    let at = |x: usize, y: usize| f[y * 16 + x];
    assert_eq!(at(4, 0), [8, 8, 0, 255], "head front starts at (8, 8)");
    assert_eq!(at(5, 1), [200, 0, 9, 255], "the hat covers it");
    assert_eq!(at(4, 8), [20, 20, 0, 255], "body");
    assert_eq!(at(0, 8), [44, 20, 0, 255], "right arm");
    assert_eq!(
        at(12, 8),
        [36, 52, 0, 255],
        "left arm, its own on a 64×64 skin"
    );
    assert_eq!(at(8, 20), [20, 52, 0, 255], "left leg");
    let slim = front(&skin, Model::Slim);
    assert_eq!(slim[8 * 16], [0, 0, 0, 0], "slim arms leave a column");
    assert_eq!(slim[8 * 16 + 1], [44, 20, 0, 255]);
}

#[test]
fn an_old_skin_mirrors_its_right_limbs() {
    let f = front(&mapped(32), Model::Classic);
    let at = |x: usize, y: usize| f[y * 16 + x];
    assert_eq!(
        at(12, 8),
        [47, 20, 0, 255],
        "the left arm is the right one mirrored"
    );
    assert_eq!(at(15, 8), [44, 20, 0, 255]);
    assert_eq!(at(8, 20), [7, 20, 0, 255]);
}

#[test]
fn only_skin_and_cape_sizes_are_kept() {
    assert!(read_skin(&png(&mapped(64))).is_ok());
    assert!(read_skin(&png(&mapped(32))).is_ok());
    let big = image::RgbaImage::new(128, 128);
    assert_eq!(
        read_skin(&png(&big)).unwrap_err(),
        "A skin is 64×64 (or the older 64×32) pixels; this one is 128×128."
    );
    assert!(read_skin(b"not a png").unwrap_err().contains("isn't a PNG"));
    assert!(read_cape(&png(&mapped(32))).is_ok());
    assert!(read_cape(&png(&image::RgbaImage::new(22, 17))).is_ok());
    assert!(read_cape(&png(&mapped(64))).is_err());
    assert_eq!(cape_front(&mapped(32))[0], [1, 1, 0, 255]);
}

#[test]
fn a_picture_is_checked_before_it_is_kept_and_can_be_cleared() {
    let root = Scratch::new("keep");
    let (skin_png, cape_png) = pictures(&root);
    let big = root.join("big.png");
    std::fs::write(&big, png(&image::RgbaImage::new(10, 10))).unwrap();
    let dir = root.join("skins");
    let (skin, cape) = skins::files(&dir, "Alex");
    assert_eq!(
        (skin.file_name().unwrap(), cape.file_name().unwrap()),
        ("alex.png".as_ref(), "alex.cape.png".as_ref())
    );

    assert!(skins::keep(&dir, "Alex", &big, false).is_err());
    assert!(
        skins::keep(&dir, "Alex", &skin_png, true).is_err(),
        "a skin is no cape"
    );
    assert!(
        !skin.exists() && !cape.exists(),
        "a refused picture leaves nothing"
    );

    skins::keep(&dir, "Alex", &skin_png, false).unwrap();
    skins::keep(&dir, "ALEX", &cape_png, true).unwrap();
    assert!(skin.is_file() && cape.is_file());
    skins::clear(&dir, "alex", true).unwrap();
    assert!(skin.is_file() && !cape.exists(), "only the cape goes");
    skins::clear(&dir, "alex", true).unwrap();
    skins::clear(&dir, "alex", false).unwrap();
    assert!(!skin.exists());
}

/// `dir`'s skin path for `name` made a folder with something in it: removing it as a file fails,
/// and with something other than "not found".
fn stuck_skin(dir: &Path, name: &str) -> PathBuf {
    let (skin, _) = skins::files(dir, name);
    std::fs::create_dir_all(&skin).unwrap();
    std::fs::write(skin.join("inside"), "x").unwrap();
    skin
}

#[test]
fn a_file_that_cannot_be_removed_is_an_error_not_a_success() {
    let root = Scratch::new("clear-fails");
    let dir = root.join("skins");
    let skin = stuck_skin(&dir, "Alex");
    let (_, cape) = skins::files(&dir, "Alex");
    std::fs::write(&cape, "a cape").unwrap();

    let refused = skins::clear(&dir, "ALEX", false).unwrap_err();
    assert!(refused.contains("alex.png"), "it names the file: {refused}");
    assert!(skin.exists());
    // A file that is not there is already gone.
    assert_eq!(skins::clear(&dir, "Nobody_1", false), Ok(()));
    assert_eq!(skins::clear(&dir, "Nobody_1", true), Ok(()));
    assert_eq!(skins::forget(&dir, "Nobody_1"), Ok(()));

    // `forget` tries both, and still says what it could not do.
    assert!(skins::forget(&dir, "Alex")
        .unwrap_err()
        .contains("alex.png"));
    assert!(!cape.exists(), "the cape went all the same");
}

#[test]
fn the_game_env_has_only_what_exists() {
    let root = Scratch::new("env");
    let (skin_png, cape_png) = pictures(&root);
    let dir = root.join("skins");
    let (skin, cape) = skins::files(&dir, "Alex");
    let var = |name: &'static str, value: &Path| (name, value.to_string_lossy().into_owned());

    assert!(skins::env(&dir, "Alex", Model::Slim).is_empty(), "no files");
    skins::keep(&dir, "Alex", &skin_png, false).unwrap();
    assert_eq!(
        skins::env(&dir, "ALEX", Model::Slim),
        [
            var("HORIZON_OFFLINE_SKIN", &skin),
            ("HORIZON_OFFLINE_SKIN_MODEL", "slim".to_string())
        ]
    );
    assert_eq!(
        skins::env(&dir, "alex", Model::Classic)[1],
        ("HORIZON_OFFLINE_SKIN_MODEL", "classic".to_string())
    );
    skins::clear(&dir, "Alex", false).unwrap();
    skins::keep(&dir, "Alex", &cape_png, true).unwrap();
    assert_eq!(
        skins::env(&dir, "Alex", Model::Slim),
        [var("HORIZON_OFFLINE_CAPE", &cape)],
        "a cape alone is no skin"
    );
    skins::keep(&dir, "Alex", &skin_png, false).unwrap();
    assert_eq!(skins::env(&dir, "Alex", Model::Classic).len(), 3);
}

#[test]
fn a_name_cannot_make_its_files_leave_the_skins_folder() {
    let dir = Path::new("skins");
    let (skin, cape) = skins::files(dir, "../../Evil/x");
    assert_eq!(skin.parent(), Some(dir));
    assert_eq!(cape.parent(), Some(dir));
}

#[test]
fn saving_atlas_accounts_keeps_the_skins_beside_them() {
    // Upstream's `Accounts::save_to` cleans the `skins` folder beside the accounts file, which for
    // Atlas is `atlas/skins/`: a wider cleanup upstream must fail here, not delete players' skins.
    let data = Scratch::new("accounts-keep-skins");
    let (skin_png, cape_png) = pictures(&data);
    let path = data.join("atlas").join("accounts.json");
    let dir = data.join("atlas").join("skins");
    skins::keep(&dir, "Steve", &skin_png, false).unwrap();
    skins::keep(&dir, "Steve", &cape_png, true).unwrap();
    let (skin, cape) = skins::files(&dir, "Steve");
    let before = (std::fs::read(&skin).unwrap(), std::fs::read(&cape).unwrap());

    Accounts::default().save_to(&path).unwrap();
    // As the launcher saves: what the file holds, read back and saved again.
    Accounts::load_from(&path).save_to(&path).unwrap();

    assert!(path.is_file());
    assert!(
        skin.is_file() && cape.is_file(),
        "saving the accounts deleted Atlas's skin or cape"
    );
    let after = (std::fs::read(&skin).unwrap(), std::fs::read(&cape).unwrap());
    assert!(after == before, "saving the accounts changed Atlas's skins");
}

// ----- Names ---------------------------------------------------------------------------------------

fn names(profiles: &Profiles) -> Vec<&str> {
    profiles.names.iter().map(|p| p.name.as_str()).collect()
}

fn playing(name: &str) -> Option<Plays> {
    Some(Plays::Offline(name.to_string()))
}

/// Profiles with these names, the first playing.
fn listed(list: &[&str]) -> Profiles {
    let mut profiles = Profiles::default();
    for name in list {
        profiles.add(name).unwrap();
    }
    profiles
}

#[test]
fn names_follow_the_rules() {
    assert!(!valid_name("ab"));
    assert!(valid_name("abc"));
    assert!(valid_name(&"a".repeat(16)));
    assert!(!valid_name(&"a".repeat(17)));
    assert!(valid_name("Builder_1"));
    assert!(!valid_name("a-b"));
    assert!(!valid_name("a b"));
    assert!(!valid_name("Ünï"));
    assert!(!valid_name(""));
}

#[test]
fn adding_a_name_twice_ignoring_case_is_refused() {
    let mut profiles = listed(&["Steve"]);
    assert!(profiles.add("steve").is_err());
    assert!(profiles.add("STEVE").is_err());
    assert!(profiles.add("ab").is_err(), "a bad name is refused too");
    assert!(profiles.add("a-b").is_err());
    assert_eq!(names(&profiles), ["Steve"]);
    profiles.add("  Alex ").unwrap();
    assert_eq!(
        names(&profiles),
        ["Steve", "Alex"],
        "spaces around are dropped"
    );
}

#[test]
fn the_first_name_added_plays() {
    let mut profiles = Profiles::default();
    assert!(profiles.plays_offline(), "nobody yet still plays offline");
    assert!(profiles.selected().is_none());
    profiles.add("Steve").unwrap();
    assert_eq!(profiles.plays, playing("Steve"));
    assert_eq!(profiles.selected().unwrap().name, "Steve");
    profiles.add("Alex").unwrap();
    assert_eq!(profiles.plays, playing("Steve"), "the next ones wait");

    // A Microsoft choice is nobody's to take.
    let mut profiles = Profiles {
        plays: Some(Plays::Microsoft("069a79f444e94726a5befca90e38aaf5".into())),
        ..Profiles::default()
    };
    profiles.add("Steve").unwrap();
    assert!(!profiles.plays_offline());
    assert!(profiles.selected().is_none());
    assert!(matches!(profiles.plays, Some(Plays::Microsoft(_))));
}

#[test]
fn selecting_and_the_model_go_by_name_in_any_case() {
    let mut profiles = listed(&["Steve", "Alex"]);
    profiles.select("alex").unwrap();
    assert_eq!(profiles.plays, playing("Alex"), "the name as it was added");
    assert!(profiles.select("Nobody").is_err());
    assert_eq!(profiles.plays, playing("Alex"));

    assert_eq!(profiles.selected().unwrap().model, Model::Classic);
    profiles.set_model("ALEX", Model::Slim).unwrap();
    assert_eq!(profiles.selected().unwrap().model, Model::Slim);
    assert_eq!(profiles.names[0].model, Model::Classic);
    assert!(profiles.set_model("Nobody", Model::Slim).is_err());
}

#[test]
fn renaming_moves_skin_and_cape() {
    let root = Scratch::new("rename");
    let (skin_png, cape_png) = pictures(&root);
    let dir = root.join("skins");
    let mut profiles = listed(&["Steve", "Alex"]);
    profiles.set_model("Steve", Model::Slim).unwrap();
    skins::keep(&dir, "Steve", &skin_png, false).unwrap();
    skins::keep(&dir, "Steve", &cape_png, true).unwrap();
    let skin_bytes = std::fs::read(skins::files(&dir, "steve").0).unwrap();

    profiles.rename(&dir, "steve", "Herobrine").unwrap();
    assert_eq!(names(&profiles), ["Herobrine", "Alex"]);
    assert_eq!(profiles.names[0].model, Model::Slim, "the model goes along");
    assert_eq!(
        profiles.plays,
        playing("Herobrine"),
        "the one playing keeps playing"
    );
    let (old, new) = (skins::files(&dir, "Steve"), skins::files(&dir, "Herobrine"));
    assert!(!old.0.exists() && !old.1.exists());
    assert_eq!(std::fs::read(&new.0).unwrap(), skin_bytes);
    assert!(new.1.is_file());

    // The same name in other letters is the same files.
    profiles.rename(&dir, "Herobrine", "herobrine").unwrap();
    assert_eq!(names(&profiles), ["herobrine", "Alex"]);
    assert!(
        new.0.is_file() && new.1.is_file(),
        "a case-only rename loses nothing"
    );
    profiles.rename(&dir, "Alex", "ALEX").unwrap();
    assert_eq!(names(&profiles), ["herobrine", "ALEX"]);
}

#[test]
fn a_name_cannot_be_renamed_to_another_one() {
    let root = Scratch::new("rename-refused");
    let (skin_png, _) = pictures(&root);
    let dir = root.join("skins");
    let mut profiles = listed(&["Steve", "Alex"]);
    skins::keep(&dir, "Steve", &skin_png, false).unwrap();
    let before = profiles.clone();

    assert!(profiles.rename(&dir, "Steve", "alex").is_err(), "taken");
    assert!(profiles.rename(&dir, "Steve", "no").is_err(), "too short");
    assert!(
        profiles.rename(&dir, "Nobody", "Herobrine").is_err(),
        "not a name"
    );
    assert_eq!(profiles, before);
    assert!(skins::files(&dir, "Steve").0.is_file());
}

#[test]
fn removing_the_selected_name_selects_the_next_or_nobody() {
    let root = Scratch::new("remove-selected");
    let dir = root.join("skins");
    let mut profiles = listed(&["Ann", "Bob", "Cat", "Dan"]);

    // Not the one playing: it plays on.
    profiles.remove(&dir, "cat").unwrap();
    assert_eq!(names(&profiles), ["Ann", "Bob", "Dan"]);
    assert_eq!(profiles.plays, playing("Ann"));

    // In the middle: the one after it.
    profiles.select("Bob").unwrap();
    profiles.remove(&dir, "Bob").unwrap();
    assert_eq!(profiles.plays, playing("Dan"));
    // The last: the one before it.
    profiles.remove(&dir, "Dan").unwrap();
    assert_eq!(profiles.plays, playing("Ann"));
    // The only one: nobody.
    profiles.remove(&dir, "Ann").unwrap();
    assert!(names(&profiles).is_empty());
    assert_eq!(profiles.plays, None);
    assert!(profiles.selected().is_none());

    // A name that isn't there changes nothing; a Microsoft choice is never moved.
    profiles.remove(&dir, "Ann").unwrap();
    let mut profiles = Profiles {
        plays: Some(Plays::Microsoft("abc".into())),
        ..listed(&["Ann"])
    };
    profiles.remove(&dir, "Ann").unwrap();
    assert!(matches!(profiles.plays, Some(Plays::Microsoft(_))));
}

#[test]
fn removing_a_name_forgets_its_files() {
    let root = Scratch::new("remove-files");
    let (skin_png, cape_png) = pictures(&root);
    let dir = root.join("skins");
    let mut profiles = listed(&["Ann", "Bob"]);
    for name in ["Ann", "Bob"] {
        skins::keep(&dir, name, &skin_png, false).unwrap();
        skins::keep(&dir, name, &cape_png, true).unwrap();
    }
    profiles.remove(&dir, "ANN").unwrap();
    let (ann, bob) = (skins::files(&dir, "Ann"), skins::files(&dir, "Bob"));
    assert!(!ann.0.exists() && !ann.1.exists());
    assert!(bob.0.is_file() && bob.1.is_file(), "the others keep theirs");
}

#[test]
fn a_name_whose_files_cannot_be_forgotten_stays() {
    let root = Scratch::new("remove-fails");
    let dir = root.join("skins");
    let mut profiles = listed(&["Ann", "Bob"]);
    let skin = stuck_skin(&dir, "Ann");
    let before = profiles.clone();

    let refused = profiles.remove(&dir, "ann").unwrap_err();
    assert!(refused.contains("ann.png"), "{refused}");
    assert_eq!(
        profiles, before,
        "nothing half-done: Ann is there and plays"
    );

    std::fs::remove_dir_all(&skin).unwrap();
    profiles.remove(&dir, "ann").unwrap();
    assert_eq!(names(&profiles), ["Bob"]);
    assert_eq!(profiles.plays, playing("Bob"));
}

#[test]
fn profiles_round_trip_through_their_file() {
    let root = Scratch::new("round-trip");
    let path = root.join("atlas").join("offline.json");
    assert_eq!(Profiles::load_from(&path), Profiles::default(), "no file");

    let mut profiles = listed(&["Steve", "Alex"]);
    profiles.set_model("Alex", Model::Slim).unwrap();
    profiles.select("Alex").unwrap();
    profiles.save_to(&path).unwrap();
    assert_eq!(Profiles::load_from(&path), profiles);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&path).unwrap())
            .unwrap(),
        serde_json::json!({
            "names": [{"name": "Steve", "model": "classic"}, {"name": "Alex", "model": "slim"}],
            "plays": {"kind": "offline", "who": "Alex"},
        })
    );
    let files: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
    assert_eq!(files.len(), 1, "no temporary file stays");

    // Another kind of "who plays" reads back too, and a name without a model has the default.
    std::fs::write(
        &path,
        r#"{"names":[{"name":"Steve"}],"plays":{"kind":"microsoft","who":"abc"}}"#,
    )
    .unwrap();
    let read = Profiles::load_from(&path);
    assert_eq!(read.plays, Some(Plays::Microsoft("abc".into())));
    assert_eq!(read.names[0].model, Model::Classic);

    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(
        Profiles::load_from(&path),
        Profiles::default(),
        "unreadable"
    );
}

#[test]
fn an_unreadable_file_is_kept_aside_not_lost() {
    use crate::offline::migrate::migrate;
    let data = Scratch::new("unreadable");
    let path = Profiles::path_in(&data);
    let aside = path.with_file_name("offline.json.unreadable");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    // Bytes that are no text, text that is no JSON, and a "who plays" a newer Atlas may write.
    let garbage: [&[u8]; 3] = [
        &[0xff, 0xfe, 0x00, b'{'],
        b"{ \"names\": [",
        br#"{"names":[{"name":"Steve"}],"plays":{"kind":"from-the-future","who":"x"}}"#,
    ];
    for bytes in garbage {
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(Profiles::load_from(&path), Profiles::default());
        assert_eq!(
            std::fs::read(&aside).unwrap(),
            bytes,
            "a copy to recover from"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "the file itself stays"
        );
        assert_eq!(migrate(&data), Ok(false), "so the move doesn't run again");
        assert_eq!(Profiles::load_in(&data), Profiles::default());
    }

    // The next save replaces the file, and the copy is still the last unreadable one.
    let mut profiles = Profiles::default();
    profiles.add("Steve").unwrap();
    profiles.save_to(&path).unwrap();
    assert_eq!(std::fs::read(&aside).unwrap(), garbage[2]);

    // A file that reads leaves no copy, and neither does a missing one.
    std::fs::remove_file(&aside).unwrap();
    assert_eq!(Profiles::load_from(&path), profiles);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(Profiles::load_from(&path), Profiles::default());
    assert!(!aside.exists());
}
