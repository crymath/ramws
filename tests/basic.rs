use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn init_creates_config() {
    let dir = tempdir().unwrap();
    #[allow(deprecated)]
    let mut cmd = assert_cmd::Command::cargo_bin("ramws").unwrap();
    cmd.arg("--chdir").arg(dir.path()).arg("init");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("created"));
    assert!(dir.path().join(".ramws.yml").exists());
}

#[test]
fn default_config_loads() {
    let dir = tempdir().unwrap();
    let cfg_path = dir.path().join(".ramws.yml");
    let yaml = r#"workspace:
  root: /dev/shm/ramws-${USER}/${PROJECT}
"#;
    fs::write(&cfg_path, yaml).unwrap();
    let orig = dir.path().to_path_buf();
    let resolved = ramws::config::Config::load_from_file(&cfg_path, orig.clone()).unwrap();
    assert!(resolved
        .workspace_root
        .to_string_lossy()
        .contains(&ramws::util::project_slug(&orig).unwrap()));
}
