use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::config;

pub fn run(assume_yes: bool) -> Result<()> {
    let binary = std::env::current_exe().context("locating the rgpt executable")?;
    let data_dir = config::app_dir()?;

    if !assume_yes {
        println!("This will remove:");
        println!("  {}", binary.display());
        println!(
            "  {} (configuration, chats, roles, and cache)",
            data_dir.display()
        );
        print!("Continue? [y/N] ");
        io::stdout().flush().ok();

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .context("reading uninstall confirmation")?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            bail!("Uninstall cancelled.");
        }
    }

    remove_data_dir(&data_dir)?;
    fs::remove_file(&binary)
        .with_context(|| format!("removing executable {}", binary.display()))?;
    println!("rgpt has been uninstalled.");
    Ok(())
}

fn remove_data_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .with_context(|| format!("removing application data directory {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::remove_data_dir;

    #[test]
    fn removes_application_data_directory() {
        let dir = std::env::temp_dir().join(format!("rgpt-uninstall-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config"), "secret").unwrap();

        remove_data_dir(&dir).unwrap();

        assert!(!dir.exists());
    }
}
