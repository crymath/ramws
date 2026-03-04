use std::process::{Command, Output};

use anyhow::{Context, Result, bail};
use plist::Value;

#[derive(Debug)]
pub struct CmdOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl CmdOutput {
    pub fn stdout_string(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }

    pub fn stderr_string(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }
}

pub fn try_run(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn command: {} {:?}", program, args))
}

pub fn try_run_owned(program: &str, args: &[String]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn command: {} {:?}", program, args))
}

pub fn run_checked(program: &str, args: &[&str]) -> Result<CmdOutput> {
    let output = run(program, args)?;
    Ok(CmdOutput { stdout: output.stdout, stderr: output.stderr })
}

pub fn run(program: &str, args: &[&str]) -> Result<Output> {
    let output = try_run(program, args)?;
    ensure_success(program, args, &output)?;
    Ok(output)
}

pub fn run_checked_owned(program: &str, args: &[String]) -> Result<CmdOutput> {
    let output = run_owned(program, args)?;
    Ok(CmdOutput { stdout: output.stdout, stderr: output.stderr })
}

pub fn run_owned(program: &str, args: &[String]) -> Result<Output> {
    let output = try_run_owned(program, args)?;
    ensure_success(program, args, &output)?;
    Ok(output)
}

fn ensure_success<T>(program: &str, args: &[T], output: &Output) -> Result<()>
where
    T: std::fmt::Debug,
{
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    bail!(
        "command failed: {} {:?} (status={:?})\nstdout:\n{}\nstderr:\n{}",
        program,
        args,
        output.status.code(),
        stdout,
        stderr
    );
}

pub fn run_plist(program: &str, args: &[String]) -> Result<Value> {
    let output = run_owned(program, args)?;
    Value::from_reader_xml(output.stdout.as_slice())
        .with_context(|| format!("failed to parse plist output from {} {:?}", program, args))
}
