use assert_cmd::Command;

#[test]
fn binary_starts_successfully() {
    Command::cargo_bin("rusty_repomix")
        .expect("binary should build")
        .assert()
        .success()
        .stdout("");
}
