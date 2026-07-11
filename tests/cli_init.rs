use std::fs;

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn init_creates_the_default_configuration_file() {
    let directory = tempdir().expect("temporary directory");

    Command::cargo_bin("rusty_repomix")
        .expect("binary should build")
        .current_dir(directory.path())
        .arg("--init")
        .assert()
        .success();

    let output = fs::read_to_string(directory.path().join("repomix.config.json"))
        .expect("config should exist");
    assert!(output.contains("\"$schema\": \"https://repomix.com/schemas/latest/schema.json\""));
    assert!(output.contains("\"style\": \"xml\""));
}
