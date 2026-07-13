use std::fs::{self, OpenOptions};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Runs the user's configured editor against a temporary prompt file.
pub fn edited_prompt() -> Result<String> {
    let path = std::env::temp_dir().join(format!(
        "rgpt-prompt-{}-{}.txt",
        std::process::id(),
        unique_suffix()
    ));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("creating temporary editor file {}", path.display()))?;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let status = Command::new(shell)
        .arg("-c")
        .arg("${EDITOR:-vi} \"$1\"")
        .arg("rgpt-editor")
        .arg(&path)
        .status()
        .context("starting $EDITOR")?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        bail!("$EDITOR exited with status {status}");
    }

    let result = fs::read_to_string(&path).context("reading prompt from $EDITOR")?;
    fs::remove_file(&path).context("removing temporary editor file")?;
    if result.trim().is_empty() {
        bail!("$EDITOR did not provide a prompt");
    }
    Ok(result)
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}
