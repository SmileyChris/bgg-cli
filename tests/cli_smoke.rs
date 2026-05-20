use assert_cmd::Command;
use std::fs;

#[test]
fn help_lists_all_subcommands() {
    let out = Command::cargo_bin("bgg")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    for sub in ["auth", "sync", "list", "stats"] {
        assert!(
            text.contains(sub),
            "help missing subcommand: {sub}\n---\n{text}"
        );
    }
}

#[test]
fn bare_bgg_with_no_config_says_no_user() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("bgg")
        .unwrap()
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("XDG_STATE_HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("No logged-in user"), "got:\n{text}");
}

#[test]
fn stats_json_emits_overview_json() {
    let tmp = tempfile::tempdir().unwrap();
    let app_dir = tmp.path().join("bgg-cli");
    fs::create_dir_all(&app_dir).unwrap();
    fs::write(app_dir.join("config.toml"), r#"username = "tester""#).unwrap();
    fs::write(
        app_dir.join("collection-tester.json"),
        r#"{"username":"tester","last_sync":null,"items":{}}"#,
    )
    .unwrap();

    let out = Command::cargo_bin("bgg")
        .unwrap()
        .args(["stats", "--json"])
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("XDG_STATE_HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["username"], "tester");
    assert_eq!(value["items"]["total"], 0);
}
