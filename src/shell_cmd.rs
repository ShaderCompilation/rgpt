use std::process::Command;

use anyhow::{Context, Result, bail};

/// Executes a generated command through the user's POSIX shell.
pub fn run(command: &str) -> Result<()> {
    if command.trim().is_empty() {
        bail!("refusing to execute an empty shell command");
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let status = Command::new(&shell)
        .arg("-c")
        .arg(command)
        .status()
        .with_context(|| format!("running command with {shell}"))?;
    if !status.success() {
        bail!("command exited with status {status}");
    }
    Ok(())
}
