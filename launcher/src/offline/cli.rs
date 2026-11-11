//! `horizon-launcher offline ...`: the offline names from the command line. Upstream's clap
//! command is left as it is: `main` asks [`run`] first, and it answers only to `offline`.
use super::profiles::{missing, Plays, Profiles};
use super::skins::{self, Model};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

const USAGE: &str = "\
Usage: horizon-launcher offline [--json] <command>

Commands:
  list                                 the names; the one that plays is marked
  add NAME                             keep a new name (the first one added plays)
  select NAME                          play as NAME
  rename OLD NEW                       rename a name; its skin and cape go with it
  remove NAME                          forget a name with its skin and cape
  skin NAME PATH [--slim|--classic]    use a PNG (64x64 or 64x32) as NAME's skin
  cape NAME PATH                       use a PNG (64x32 or 22x17) as NAME's cape
  clear-skin NAME                      back to the default skin
  clear-cape NAME                      take the cape off

A name is 3 to 16 letters, digits or _. --json prints the names and who plays as one object.";

/// [`run`] on this process's arguments. Taken as `args_os` and read lossily: `std::env::args`
/// panics on an argument that isn't Unicode, and this runs before upstream's parser sees any.
pub fn from_env() -> Option<i32> {
    let args: Vec<String> = std::env::args_os()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    run(&args)
}

/// Runs the command when `args` (the program's, as `std::env::args`) is `offline ...` and returns
/// the exit code: 0 done, 1 refused, 2 used wrongly. None: it isn't ours.
pub fn run(args: &[String]) -> Option<i32> {
    run_in(
        args,
        &crate::backend::data_dir(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// [`run`] on the data folder `data`, writing to `out` and `err`.
pub fn run_in(
    args: &[String],
    data: &Path,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("offline") {
        return None;
    }
    let result = match parse(&args[2..]) {
        // Asking for help is an answer, not a mistake.
        Ok(Parsed::Help) => Ok(USAGE.to_string()),
        Ok(Parsed::Do(action, json)) => {
            perform(action, json, data).map_err(|refused| (format!("error: {refused}"), 1))
        }
        Err(wrong) => Err((format!("error: {wrong}\n\n{USAGE}"), 2)),
    };
    Some(match result {
        Ok(said) => {
            let _ = writeln!(out, "{said}");
            0
        }
        Err((problem, code)) => {
            let _ = writeln!(err, "{problem}");
            code
        }
    })
}

enum Action {
    List,
    Add(String),
    Select(String),
    Rename(String, String),
    Remove(String),
    Skin(String, String, Option<Model>),
    Cape(String, String),
    ClearSkin(String),
    ClearCape(String),
}

enum Parsed {
    Help,
    Do(Action, bool),
}

/// The words after `offline`: options anywhere, the command and its arguments in order.
fn parse(words: &[String]) -> Result<Parsed, String> {
    let (mut json, mut slim, mut classic) = (false, false, false);
    let mut positional = Vec::new();
    for word in words {
        match word.as_str() {
            "--json" => json = true,
            "--slim" => slim = true,
            "--classic" => classic = true,
            "--help" | "-h" => return Ok(Parsed::Help),
            option if option.starts_with("--") => return Err(format!("unknown option {option}")),
            _ => positional.push(word.clone()),
        }
    }
    let Some((command, args)) = positional.split_first() else {
        return Err("no command".into());
    };
    if (slim || classic) && command != "skin" {
        return Err("--slim and --classic go with `skin` only".into());
    }
    if slim && classic {
        return Err("--slim and --classic exclude each other".into());
    }
    let wants = |n: usize| match args.len() == n {
        true => Ok(()),
        false => Err(format!(
            "`{command}` takes {n} argument(s), not {}",
            args.len()
        )),
    };
    let action = match command.as_str() {
        "help" => return Ok(Parsed::Help),
        "list" => wants(0).map(|()| Action::List),
        "add" => wants(1).map(|()| Action::Add(args[0].clone())),
        "select" => wants(1).map(|()| Action::Select(args[0].clone())),
        "remove" => wants(1).map(|()| Action::Remove(args[0].clone())),
        "clear-skin" => wants(1).map(|()| Action::ClearSkin(args[0].clone())),
        "clear-cape" => wants(1).map(|()| Action::ClearCape(args[0].clone())),
        "rename" => wants(2).map(|()| Action::Rename(args[0].clone(), args[1].clone())),
        "cape" => wants(2).map(|()| Action::Cape(args[0].clone(), args[1].clone())),
        "skin" => wants(2).map(|()| {
            let model = match (slim, classic) {
                (true, _) => Some(Model::Slim),
                (_, true) => Some(Model::Classic),
                _ => None,
            };
            Action::Skin(args[0].clone(), args[1].clone(), model)
        }),
        other => Err(format!("unknown command {other}")),
    }?;
    Ok(Parsed::Do(action, json))
}

/// Does the action, keeps what changed, and answers with what to print.
fn perform(action: Action, json: bool, data: &Path) -> Result<String, String> {
    let mut profiles = Profiles::load_in(data);
    let before = profiles.clone();
    let dir = skins::dir_in(data);
    // The name as the list keeps it, whatever letters it was asked for in.
    let kept = |profiles: &Profiles, name: &str| {
        profiles
            .find(name.trim())
            .map(|p| p.name.clone())
            .ok_or_else(|| missing(name))
    };
    let said = match action {
        Action::List => String::new(),
        Action::Add(name) => {
            profiles.add(&name)?;
            let added = kept(&profiles, &name)?;
            match profiles.selected().is_some_and(|p| p.name == added) {
                true => format!("Added {added}. It plays."),
                false => format!("Added {added}."),
            }
        }
        Action::Select(name) => {
            let name = kept(&profiles, &name)?;
            profiles.select(&name)?;
            format!("Playing as {name}.")
        }
        Action::Rename(old, new) => {
            let old = kept(&profiles, &old)?;
            profiles.rename(&dir, &old, &new)?;
            format!("Renamed {old} to {}.", kept(&profiles, &new)?)
        }
        Action::Remove(name) => {
            let gone = kept(&profiles, &name)?;
            let was = profiles.plays.clone();
            profiles.remove(&dir, &gone)?;
            let mut said = format!("Removed {gone}.");
            if profiles.plays != was {
                said += &match profiles.selected() {
                    Some(next) => format!(" Playing as {}.", next.name),
                    None => " Nobody is selected: the game uses the name it made up.".into(),
                };
            }
            said
        }
        Action::Skin(name, path, model) => {
            let name = kept(&profiles, &name)?;
            skins::keep(&dir, &name, Path::new(&path), false)?;
            match model {
                Some(model) => {
                    profiles.set_model(&name, model)?;
                    format!("Skin kept for {name}, {} arms.", model.name())
                }
                None => format!("Skin kept for {name}."),
            }
        }
        Action::Cape(name, path) => {
            let name = kept(&profiles, &name)?;
            skins::keep(&dir, &name, Path::new(&path), true)?;
            format!("Cape kept for {name}.")
        }
        Action::ClearSkin(name) => {
            let name = kept(&profiles, &name)?;
            skins::clear(&dir, &name, false)?;
            format!("{name} has the default skin.")
        }
        Action::ClearCape(name) => {
            let name = kept(&profiles, &name)?;
            skins::clear(&dir, &name, true)?;
            format!("{name} has no cape.")
        }
    };
    if profiles != before {
        profiles.save_to(&Profiles::path_in(data))?;
    }
    Ok(match json {
        true => serde_json::to_string_pretty(&state(&profiles, &dir)).map_err(|e| e.to_string())?,
        false if said.is_empty() => listing(&profiles, &dir),
        false => said,
    })
}

/// Whether `name` has a skin and a cape kept in `dir`.
fn looks(dir: &Path, name: &str) -> (bool, bool) {
    let (skin, cape) = skins::files(dir, name);
    (skin.is_file(), cape.is_file())
}

/// The names and who plays, as `--json` prints them.
fn state(profiles: &Profiles, dir: &Path) -> Value {
    let names: Vec<Value> = profiles
        .names
        .iter()
        .map(|p| {
            let (skin, cape) = looks(dir, &p.name);
            json!({"name": p.name, "model": p.model.name(), "skin": skin, "cape": cape})
        })
        .collect();
    json!({"names": names, "plays": profiles.plays})
}

/// One line a name, the one that plays marked with `*`.
fn listing(profiles: &Profiles, dir: &Path) -> String {
    if profiles.names.is_empty() {
        return "No offline names. Add one: horizon-launcher offline add NAME".into();
    }
    let width = profiles
        .names
        .iter()
        .map(|p| p.name.len())
        .max()
        .unwrap_or(0);
    let playing = match &profiles.plays {
        Some(Plays::Offline(name)) => Some(name.as_str()),
        _ => None,
    };
    let lines: Vec<String> = profiles
        .names
        .iter()
        .map(|p| {
            let mark = if playing.is_some_and(|n| n.eq_ignore_ascii_case(&p.name)) {
                '*'
            } else {
                ' '
            };
            let (skin, cape) = looks(dir, &p.name);
            let worn = [(skin, "skin"), (cape, "cape")]
                .iter()
                .filter(|(has, _)| *has)
                .map(|(_, what)| *what)
                .collect::<Vec<_>>()
                .join(", ");
            let line = format!("{mark} {:<width$}  {:<7}  {worn}", p.name, p.model.name());
            line.trim_end().to_string()
        })
        .collect();
    lines.join("\n")
}
