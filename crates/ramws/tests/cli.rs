use std::process::Command;

use assert_cmd::prelude::*;
use tempfile::TempDir;

#[test]
fn init_creates_project_config() {
    let tmp = TempDir::new().expect("tmp");
    let bin = assert_cmd::cargo::cargo_bin!("ramws");

    let mut cmd = Command::new(bin);
    cmd.current_dir(tmp.path()).arg("init");
    cmd.assert().success();

    assert!(tmp.path().join(".ramws.toml").exists());
}

#[test]
fn init_refuses_without_force_when_config_exists() {
    let tmp = TempDir::new().expect("tmp");
    let bin = assert_cmd::cargo::cargo_bin!("ramws");

    let mut first = Command::new(bin);
    first.current_dir(tmp.path()).arg("init");
    first.assert().success();

    let mut second = Command::new(bin);
    second.current_dir(tmp.path()).arg("init");
    second.assert().failure();

    let mut forced = Command::new(bin);
    forced.current_dir(tmp.path()).args(["init", "--force"]);
    forced.assert().success();
}
