//! Atlas's owner gate from the outside: a player with an offline name needs no account that owns
//! Minecraft: Java Edition for `horizon-launcher play` or the MCP server's play. They go on to
//! start the game, and stop here only where the version isn't prepared. Nothing is downloaded:
//! without an owner, Mojang's files stay refused.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// What upstream's launcher says to a player with no owning account; Atlas never does.
const REFUSED: &str = "Sign in with a Microsoft account that owns Minecraft: Java Edition to play.";
const SIGN_IN: &str = "account sign-in";
/// Where the run stops instead: past the owner gate, with nothing prepared to start.
const NOT_PREPARED: &str = "is not prepared yet";
/// The shared accounts.json of an Atlas 0.1.x player, with an offline name and no account.
const OLD_ACCOUNTS: &str =
    r#"{"accounts":[],"selected":null,"offline":["Builder_1"],"offline_selected":"Builder_1"}"#;

/// Fresh homes with one installed (not prepared) version and an offline name from Atlas 0.1.x,
/// as a player who played offline before has.
fn home(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("horizon-owner-gate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let version = root.join("launcher/versions/v2026.10.27.01");
    std::fs::create_dir_all(&version).unwrap();
    std::fs::write(version.join(".installed"), "").unwrap();
    std::fs::write(root.join("launcher/accounts.json"), OLD_ACCOUNTS).unwrap();
    root
}

fn launcher(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_horizon-launcher"));
    cmd.args(args)
        .env("HORIZON_HOME", root.join("game"))
        .env("HORIZON_LAUNCHER_HOME", root.join("launcher"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("HORIZON_LAUNCHER_NO_SELF_UPDATE", "1")
        .env("HORIZON_NO_AUDIO", "1")
        .env("HORIZON_TELEMETRY_URL", "off")
        // Memory only: the test never reads the player's keychain.
        .env("HORIZON_NO_KEYCHAIN", "1")
        .env("HORIZON_MSA_CLIENT_ID", "test-client");
    cmd
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

/// What a start leaves for the checks below: the names Atlas keeps, the shared file as it was,
/// and no Minecraft download.
fn assert_played_offline_as_builder(root: &Path) {
    let kept = std::fs::read_to_string(root.join("launcher/atlas/offline.json")).unwrap();
    assert!(kept.contains("Builder_1"), "{kept}");
    let shared = std::fs::read_to_string(root.join("launcher/accounts.json")).unwrap();
    assert_eq!(
        shared, OLD_ACCOUNTS,
        "the shared accounts file is not ours to change"
    );
    let versions: Vec<_> = std::fs::read_dir(root.join("launcher/versions"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(versions, ["v2026.10.27.01"], "nothing was downloaded");
}

/// The names of the accounts upstream's `account list` shows.
fn signed_in(root: &Path) -> Vec<String> {
    let out = launcher(root, &["account", "list", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", text(&out));
    let listed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let accounts = listed["accounts"].as_array().unwrap();
    accounts
        .iter()
        .map(|a| a["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn atlas_has_accounts_of_its_own_and_never_the_shared_ones() {
    // The official launcher's sign-in sits in the shared accounts.json, beside the old names.
    let shared = r#"{"accounts":[{"uuid":"069a79f444e94726a5befca90e38aaf5","name":"Notch"}],"selected":"069a79f444e94726a5befca90e38aaf5","offline":["Builder_1"]}"#;
    let root = home("accounts");
    std::fs::write(root.join("launcher/accounts.json"), shared).unwrap();
    assert_eq!(signed_in(&root), Vec::<String>::new());

    // Atlas reads its own file: the same command shows what is in it, and only that.
    std::fs::create_dir_all(root.join("launcher/atlas")).unwrap();
    std::fs::write(
        root.join("launcher/atlas/accounts.json"),
        r#"{"accounts":[{"uuid":"4d1f4a1ab3a94e5e8f1a2b3c4d5e6f70","name":"Alex"}],"selected":null}"#,
    )
    .unwrap();
    assert_eq!(signed_in(&root), ["Alex"]);

    let kept = std::fs::read_to_string(root.join("launcher/accounts.json")).unwrap();
    assert_eq!(
        kept, shared,
        "the shared accounts file is not ours to change"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_cli_plays_offline_without_an_owner() {
    let root = home("cli");
    let out = launcher(&root, &["play"]).output().unwrap();
    let said = text(&out);
    assert!(!said.contains(REFUSED), "{said}");
    assert!(!said.contains(SIGN_IN), "{said}");
    // It stops later, where the version isn't prepared, and nothing of the game ran.
    assert!(!out.status.success(), "{said}");
    assert!(said.contains(NOT_PREPARED), "{said}");
    assert!(!root.join("game/sessions").exists());
    assert_played_offline_as_builder(&root);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_name_added_with_the_offline_command_plays() {
    // A fresh install, no accounts file at all: the command runs before upstream's parser and
    // before any window, and the name it keeps is the one `play` starts with.
    let root = home("added");
    std::fs::remove_file(root.join("launcher/accounts.json")).unwrap();
    let added = launcher(&root, &["offline", "add", "Steve"])
        .output()
        .unwrap();
    assert!(added.status.success(), "{}", text(&added));
    let listed = launcher(&root, &["offline", "list", "--json"])
        .output()
        .unwrap();
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["names"][0]["name"], "Steve", "{listed}");
    assert_eq!(listed["plays"]["who"], "Steve", "{listed}");

    let said = text(&launcher(&root, &["play"]).output().unwrap());
    assert!(!said.contains(REFUSED), "{said}");
    assert!(said.contains(NOT_PREPARED), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_mcp_server_plays_offline_without_an_owner() {
    let root = home("mcp");
    let mut child = launcher(&root, &["mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"play","arguments":{}}}"#,
    ];
    let mut stdin = child.stdin.take().unwrap();
    for r in requests {
        writeln!(stdin, "{r}").unwrap();
    }
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let reply = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .find(|v| v["id"] == 2)
        .expect("a reply to play");
    let said = reply["result"]["content"][0]["text"].as_str().unwrap();
    assert!(!said.contains(REFUSED), "{said}");
    assert!(!said.contains(SIGN_IN), "{said}");
    assert!(said.contains(NOT_PREPARED), "{said}");
    assert_played_offline_as_builder(&root);
    let _ = std::fs::remove_dir_all(&root);
}
