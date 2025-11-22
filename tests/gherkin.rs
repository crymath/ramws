use anyhow::Result;
use assert_cmd::Command;
use cucumber::{given, then, when, World};
use ramws::config::Config;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[derive(Debug, Default, World)]
pub struct RamwsWorld {
    project: Option<TempDir>,
    workspace_root: Option<PathBuf>,
}

impl Drop for RamwsWorld {
    fn drop(&mut self) {
        if let Some(project) = &self.project {
            #[allow(deprecated)]
            if let Ok(mut cmd) = Command::cargo_bin("ramws") {
                let _ = cmd
                    .current_dir(project.path())
                    .args(["destroy", "--force", "--noninteractive"])
                    .output();
            }
        }
    }
}

#[given("a temporary project with a tracked file")]
async fn temp_project(world: &mut RamwsWorld) -> Result<()> {
    let dir = TempDir::new()?;
    fs::create_dir_all(dir.path().join("src"))?;
    fs::write(dir.path().join("src/hello.txt"), "hello from disk")?;
    world.project = Some(dir);
    Ok(())
}

#[given("the ramws config is initialized")]
async fn init_config(world: &mut RamwsWorld) -> Result<()> {
    let project = world.project.as_ref().expect("project should exist");
    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("ramws")?;
    cmd.current_dir(project.path()).arg("init");
    cmd.assert().success();
    Ok(())
}

#[when("I start the workspace")]
async fn start_workspace(world: &mut RamwsWorld) -> Result<()> {
    let project = world.project.as_ref().expect("project should exist");
    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("ramws")?;
    cmd.current_dir(project.path()).arg("start");
    cmd.assert().success();

    let cfg_path = project.path().join(".ramws.yml");
    let resolved = Config::load_from_file(&cfg_path, project.path().to_path_buf())?;
    fs::create_dir_all(&resolved.workspace_root)?;
    world.workspace_root = Some(resolved.workspace_root.clone());

    assert!(resolved.workspace_root.join("src/hello.txt").exists());
    Ok(())
}

#[when("I edit the tracked file inside the workspace")]
async fn edit_workspace_file(world: &mut RamwsWorld) -> Result<()> {
    let ws = world
        .workspace_root
        .as_ref()
        .expect("workspace should be initialized");
    fs::write(ws.join("src/hello.txt"), "hello from ram")?;
    Ok(())
}

#[when("I sync changes back to disk")]
async fn sync_back_to_disk(world: &mut RamwsWorld) -> Result<()> {
    let project = world.project.as_ref().expect("project should exist");
    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("ramws")?;
    cmd.current_dir(project.path())
        .args(["sync", "--back", "--only", "src", "--noninteractive"]);
    cmd.assert().success();
    Ok(())
}

#[then("the original project sees the updated contents")]
async fn verify_disk_contents(world: &mut RamwsWorld) -> Result<()> {
    let project = world.project.as_ref().expect("project should exist");
    let content = fs::read_to_string(project.path().join("src/hello.txt"))?;
    assert_eq!(content, "hello from ram");
    Ok(())
}

#[tokio::test]
async fn gherkin_suite() {
    RamwsWorld::run("tests/features").await;
}
