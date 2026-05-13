use assert_cmd::Command;

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
    for sub in ["auth", "sync", "list", "status"] {
        assert!(text.contains(sub), "help missing subcommand: {sub}\n---\n{text}");
    }
}

#[test]
fn status_with_no_config_says_no_user() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("bgg")
        .unwrap()
        .env("XDG_CONFIG_HOME", tmp.path())
        .env("XDG_STATE_HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .arg("status")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("No logged-in user"), "got:\n{text}");
}
