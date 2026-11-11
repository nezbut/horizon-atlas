//! `horizon-launcher offline ...`, called the way `main` does, with the data folder given.
use super::*;
use crate::offline::cli::run_in;
use serde_json::{json, Value};

/// What a call said: its exit code (none: not ours), stdout and stderr.
struct Said {
    code: Option<i32>,
    out: String,
    err: String,
}

/// `horizon-launcher <args>` in `data`.
fn call(data: &Path, args: &[&str]) -> Said {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_in(&args, data, &mut out, &mut err);
    Said {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// `horizon-launcher offline <args>` in `data`.
fn offline(data: &Path, args: &[&str]) -> Said {
    let all: Vec<&str> = ["horizon-launcher", "offline"]
        .into_iter()
        .chain(args.iter().copied())
        .collect();
    call(data, &all)
}

/// The call must have worked and said nothing on stderr.
fn works(said: Said) -> String {
    assert_eq!(
        (said.code, said.err.as_str()),
        (Some(0), ""),
        "{}",
        said.out
    );
    said.out
}

fn listed(data: &Path) -> Value {
    serde_json::from_str(&works(offline(data, &["list", "--json"]))).unwrap()
}

fn entry(name: &str, model: &str, skin: bool, cape: bool) -> Value {
    json!({"name": name, "model": model, "skin": skin, "cape": cape})
}

#[test]
fn not_our_command_returns_none() {
    let data = Scratch::new("cli-none");
    for args in [
        &[][..],
        &["horizon-launcher"],
        &["horizon-launcher", "play"],
        &["horizon-launcher", "offlines", "list"],
        &["horizon-launcher", "account", "offline"],
    ] {
        let said = call(&data, args);
        assert_eq!(
            (said.code, said.out, said.err),
            (None, "".into(), "".into())
        );
    }
    assert!(!data.join("atlas").exists(), "nothing was touched");
}

#[test]
fn add_select_list() {
    let data = Scratch::new("cli-list");
    assert!(works(offline(&data, &["list"])).contains("No offline names"));
    assert_eq!(listed(&data), json!({"names": [], "plays": null}));

    assert!(works(offline(&data, &["add", "Steve"])).contains("Added Steve"));
    works(offline(&data, &["add", "Alex"]));
    assert_eq!(
        listed(&data),
        json!({
            "names": [entry("Steve", "classic", false, false), entry("Alex", "classic", false, false)],
            "plays": {"kind": "offline", "who": "Steve"},
        }),
        "the first name plays"
    );

    assert!(works(offline(&data, &["select", "alex"])).contains("Alex"));
    let text = works(offline(&data, &["list"]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].starts_with("  Steve"), "{text}");
    assert!(lines[1].starts_with("* Alex"), "{text}");

    // `--json` goes anywhere after `offline`, and every command answers with the state after it.
    let after = works(offline(&data, &["--json", "select", "Steve"]));
    let state: Value = serde_json::from_str(&after).unwrap();
    assert_eq!(state["plays"], json!({"kind": "offline", "who": "Steve"}));
    assert_eq!(state, listed(&data));
    assert_eq!(
        Profiles::load_from(&Profiles::path_in(&data)).plays,
        playing("Steve"),
        "kept in the file"
    );
}

#[test]
fn a_bad_name_exits_1() {
    let data = Scratch::new("cli-refused");
    works(offline(&data, &["add", "Steve"]));
    let (skin_png, _) = pictures(&data);
    let big = data.join("big.png");
    std::fs::write(&big, png(&image::RgbaImage::new(10, 10))).unwrap();
    let before = listed(&data);

    let refused: &[&[&str]] = &[
        &["add", "a-b"],
        &["add", "ab"],
        &["add", "STEVE"],
        &["select", "Nobody"],
        &["rename", "Nobody", "Herobrine"],
        &["rename", "Steve", "a-b"],
        &["remove", "Nobody"],
        &["skin", "Nobody", skin_png.to_str().unwrap()],
        &["skin", "Steve", big.to_str().unwrap()],
        &["skin", "Steve", "/no/such/file.png"],
        &["cape", "Steve", skin_png.to_str().unwrap()],
        &["clear-skin", "Nobody"],
        &["clear-cape", "Nobody"],
    ];
    for args in refused {
        let said = offline(&data, args);
        assert_eq!(said.code, Some(1), "{args:?}: {}", said.err);
        assert!(said.err.starts_with("error: "), "{args:?}: {}", said.err);
        assert_eq!(said.out, "", "{args:?}");
    }
    assert_eq!(listed(&data), before, "a refusal changes nothing");
}

#[test]
fn skin_with_slim_sets_the_model() {
    let data = Scratch::new("cli-skin");
    let (skin_png, cape_png) = pictures(&data);
    let (skin_png, cape_png) = (skin_png.to_str().unwrap(), cape_png.to_str().unwrap());
    works(offline(&data, &["add", "Alex"]));
    let worn = |model: &str, skin: bool, cape: bool| json!({"names": [entry("Alex", model, skin, cape)], "plays": {"kind": "offline", "who": "Alex"}});

    works(offline(&data, &["skin", "Alex", skin_png, "--slim"]));
    assert_eq!(listed(&data), worn("slim", true, false));
    let (kept, _) = skins::files(&skins::dir_in(&data), "Alex");
    assert_eq!(std::fs::read(kept).unwrap(), png(&mapped(64)));

    // No flag: the model stays; the other flag changes it back, wherever it stands.
    works(offline(&data, &["skin", "alex", skin_png]));
    assert_eq!(listed(&data), worn("slim", true, false));
    works(offline(&data, &["skin", "--classic", "Alex", skin_png]));
    assert_eq!(listed(&data), worn("classic", true, false));

    works(offline(&data, &["cape", "Alex", cape_png]));
    assert_eq!(listed(&data), worn("classic", true, true));
    works(offline(&data, &["clear-skin", "Alex"]));
    assert_eq!(listed(&data), worn("classic", false, true));
    works(offline(&data, &["clear-cape", "Alex"]));
    assert_eq!(listed(&data), worn("classic", false, false));
    works(offline(&data, &["clear-cape", "Alex"]));
}

#[test]
fn renaming_and_removing_take_the_pictures_along() {
    let data = Scratch::new("cli-files");
    let (skin_png, _) = pictures(&data);
    let dir = skins::dir_in(&data);
    works(offline(&data, &["add", "Steve"]));
    works(offline(&data, &["add", "Alex"]));
    works(offline(
        &data,
        &["skin", "Steve", skin_png.to_str().unwrap()],
    ));

    works(offline(&data, &["rename", "Steve", "Herobrine"]));
    assert!(skins::files(&dir, "Herobrine").0.is_file());
    assert!(!skins::files(&dir, "Steve").0.exists());
    assert_eq!(
        listed(&data)["plays"],
        json!({"kind": "offline", "who": "Herobrine"})
    );

    let said = works(offline(&data, &["remove", "herobrine"]));
    assert!(said.contains("Alex"), "it says who plays now: {said}");
    assert!(!skins::files(&dir, "Herobrine").0.exists());
    assert_eq!(
        listed(&data)["plays"],
        json!({"kind": "offline", "who": "Alex"})
    );
}

#[test]
fn a_name_with_spaces_around_it_is_found_like_any_other() {
    let data = Scratch::new("cli-padded");
    let (skin_png, _) = pictures(&data);
    for name in ["Steve", "Alex", "Cat"] {
        works(offline(&data, &["add", name]));
    }
    let names = |data: &Path| -> Vec<String> {
        let state = listed(data);
        let all = state["names"].as_array().unwrap();
        all.iter()
            .map(|n| n["name"].as_str().unwrap().to_string())
            .collect()
    };

    works(offline(&data, &["select", " alex "]));
    assert_eq!(
        listed(&data)["plays"],
        json!({"kind": "offline", "who": "Alex"})
    );
    works(offline(&data, &["rename", " Steve ", " Herobrine "]));
    assert_eq!(names(&data), ["Herobrine", "Alex", "Cat"]);
    works(offline(
        &data,
        &["skin", " Cat", skin_png.to_str().unwrap()],
    ));

    // "Removed" means gone, files too.
    let said = works(offline(&data, &["remove", " Alex"]));
    assert!(said.starts_with("Removed Alex."), "{said}");
    assert_eq!(names(&data), ["Herobrine", "Cat"]);
    works(offline(&data, &["remove", " cat "]));
    assert_eq!(names(&data), ["Herobrine"]);
    assert!(!skins::files(&skins::dir_in(&data), "Cat").0.exists());
}

#[test]
fn a_clear_or_a_removal_that_fails_is_refused_and_changes_nothing() {
    let data = Scratch::new("cli-stuck");
    let dir = skins::dir_in(&data);
    works(offline(&data, &["add", "Steve"]));
    works(offline(&data, &["add", "Alex"]));
    // Steve's skin can't be deleted: its path is a folder with something in it.
    let skin = stuck_skin(&dir, "Steve");
    let before = listed(&data);

    for args in [&["clear-skin", "Steve"][..], &["remove", "steve"]] {
        let said = offline(&data, args);
        assert_eq!(said.code, Some(1), "{args:?}: {}", said.out);
        assert!(said.err.starts_with("error: "), "{args:?}: {}", said.err);
        assert!(said.err.contains("steve.png"), "{args:?}: {}", said.err);
        assert_eq!(said.out, "", "{args:?}");
    }
    assert_eq!(listed(&data), before, "Steve is still there, and plays");
    assert!(skin.exists());
    // Another name's pictures are not in the way, and a cape that isn't there is no failure.
    works(offline(&data, &["clear-cape", "Steve"]));
    works(offline(&data, &["clear-skin", "Alex"]));
}

#[test]
fn unknown_subcommand_exits_2() {
    let data = Scratch::new("cli-usage");
    let (skin_png, _) = pictures(&data);
    let skin_png = skin_png.to_str().unwrap();
    let wrong: &[&[&str]] = &[
        &[],
        &["frobnicate"],
        &["frobnicate", "Steve"],
        &["add"],
        &["add", "Steve", "Alex"],
        &["list", "Steve"],
        &["rename", "Steve"],
        &["skin", "Steve"],
        &["skin", "Steve", skin_png, "--slim", "--classic"],
        &["cape", "Steve", skin_png, "--slim"],
        &["add", "Steve", "--slim"],
        &["list", "--frobnicate"],
    ];
    for args in wrong {
        let said = offline(&data, args);
        assert_eq!(said.code, Some(2), "{args:?}");
        assert!(said.err.starts_with("error: "), "{args:?}: {}", said.err);
        assert!(
            said.err.contains("Usage: horizon-launcher offline"),
            "{args:?}"
        );
        assert_eq!(said.out, "", "{args:?}");
    }
    assert!(!data.join("atlas").exists(), "bad usage touches nothing");

    // Asking for help is no mistake.
    for flag in ["--help", "-h", "help"] {
        let help = works(offline(&data, &[flag]));
        assert!(help.contains("Usage: horizon-launcher offline"), "{flag}");
    }
}

#[test]
fn the_names_of_0_1_are_there_for_the_first_command() {
    let data = Scratch::new("cli-moved");
    std::fs::write(
        data.join("accounts.json"),
        r#"{"offline": ["nezbut", "Player"], "offline_selected": "Player"}"#,
    )
    .unwrap();
    assert_eq!(
        listed(&data),
        json!({
            "names": [entry("nezbut", "classic", false, false), entry("Player", "classic", false, false)],
            "plays": {"kind": "offline", "who": "Player"},
        })
    );
}
