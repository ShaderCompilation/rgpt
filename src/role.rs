use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;

const SHELL_ROLE: &str = r#"Provide only {shell} commands for {os} without any description.
If there is a lack of details, provide most logical solution.
Ensure the output is a valid shell command.
If multiple steps required try to combine them together using &&.
Provide only plain text without Markdown formatting.
Do not provide markdown formatting such as ```.
"#;

const DESCRIBE_SHELL_ROLE: &str = r#"Provide a terse, single sentence description of the given shell command.
Describe each argument and option of the command.
Provide short responses in about 80 words.
APPLY MARKDOWN formatting when possible."#;

const CODE_ROLE: &str = r#"Provide only code as output without any description.
Provide only code in plain text format without Markdown formatting.
Do not include symbols such as ``` or ```python.
If there is a lack of details, provide most logical solution.
You are not allowed to ask for more details.
For example if the prompt is "Hello world Python", you should return "print('Hello world')"."#;

const DEFAULT_ROLE: &str = r#"You are programming and system administration assistant.
You are managing {os} operating system with {shell} shell.
Provide short responses in about 100 words, unless you are specifically asked for more details.
If you need to store any data, assume it will be stored in the conversation.
APPLY MARKDOWN formatting when possible."#;

pub const DEFAULT_ROLE_NAME: &str = "ShellGPT";
pub const SHELL_ROLE_NAME: &str = "Shell Command Generator";
pub const DESCRIBE_SHELL_ROLE_NAME: &str = "Shell Command Descriptor";
pub const CODE_ROLE_NAME: &str = "Code Generator";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SystemRole {
    pub name: String,
    pub role: String,
}

impl SystemRole {
    /// Builds a role from a name and its raw description, wrapping it in the
    /// same "You are {name}\n{role}" template shell_gpt persists and sends
    /// as the system message.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            role: format!("You are {name}\n{description}"),
        }
    }

    /// Creates the 4 default roles on disk if they don't already exist.
    /// Safe to call on every startup.
    pub fn ensure_defaults(config: &Config) -> Result<()> {
        let dir = Self::storage_dir()?;
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating role directory {}", dir.display()))?;

        let shell = shell_name(config);
        let os = os_name(config);
        let subst = |template: &str| template.replace("{shell}", &shell).replace("{os}", &os);

        let defaults = [
            (DEFAULT_ROLE_NAME, subst(DEFAULT_ROLE)),
            (SHELL_ROLE_NAME, subst(SHELL_ROLE)),
            (DESCRIBE_SHELL_ROLE_NAME, subst(DESCRIBE_SHELL_ROLE)),
            (CODE_ROLE_NAME, CODE_ROLE.to_string()),
        ];

        for (name, description) in defaults {
            let path = dir.join(format!("{name}.json"));
            if !path.exists() {
                Self::new(name, &description).save(false)?;
            }
        }
        Ok(())
    }

    pub fn get(name: &str) -> Result<Self> {
        let path = Self::storage_dir()?.join(format!("{name}.json"));
        if !path.exists() {
            bail!("Role \"{name}\" not found.");
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading role file {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("parsing role file {}", path.display()))
    }

    /// Interactively prompts for a role description on stdin, then saves it,
    /// asking for overwrite confirmation if a role with the same name exists.
    pub fn create_interactive(name: &str) -> Result<()> {
        print!("Enter role description: ");
        std::io::stdout().flush().ok();
        let mut description = String::new();
        std::io::stdin()
            .read_line(&mut description)
            .context("reading role description")?;

        Self::new(name, description.trim()).save(true)
    }

    /// Lists role file paths, sorted by last-modified time ascending (oldest first).
    pub fn list() -> Result<Vec<PathBuf>> {
        let dir = Self::storage_dir()?;
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(&dir)
            .with_context(|| format!("reading role directory {}", dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let mtime = entry.metadata().ok()?.modified().ok()?;
                Some((mtime, entry.path()))
            })
            .collect();
        entries.sort_by_key(|(mtime, _)| *mtime);
        Ok(entries.into_iter().map(|(_, path)| path).collect())
    }

    fn storage_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().context("could not determine config directory")?;
        Ok(base.join("rgpt").join("roles"))
    }

    fn save(&self, confirm_overwrite: bool) -> Result<()> {
        let path = Self::storage_dir()?.join(format!("{}.json", self.name));

        if confirm_overwrite && path.exists() {
            print!(
                "Role \"{}\" already exists, overwrite it? [y/N] ",
                self.name
            );
            std::io::stdout().flush().ok();
            let mut answer = String::new();
            std::io::stdin()
                .read_line(&mut answer)
                .context("reading overwrite confirmation")?;
            if !matches!(answer.trim().to_lowercase().as_str(), "y" | "yes") {
                bail!("Aborted.");
            }
        }

        let contents = serde_json::to_string(self).context("serializing role")?;
        fs::write(&path, contents).with_context(|| format!("writing role file {}", path.display()))
    }
}

fn os_name(config: &Config) -> String {
    let configured = config.get("OS_NAME").unwrap_or_else(|_| "auto".to_string());
    if configured != "auto" {
        return configured;
    }
    detect_os_name()
}

fn shell_name(config: &Config) -> String {
    let configured = config
        .get("SHELL_NAME")
        .unwrap_or_else(|_| "auto".to_string());
    if configured != "auto" {
        return configured;
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    Path::new(&shell)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or(shell)
}

#[cfg(target_os = "linux")]
fn detect_os_name() -> String {
    if let Ok(contents) = fs::read_to_string("/etc/os-release") {
        for line in contents.lines() {
            if let Some(value) = line.strip_prefix("PRETTY_NAME=") {
                return format!("Linux/{}", value.trim_matches('"'));
            }
        }
    }
    "Linux".to_string()
}

#[cfg(target_os = "macos")]
fn detect_os_name() -> String {
    let version = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    format!("Darwin/MacOS {version}")
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn detect_os_name() -> String {
    std::env::consts::OS.to_string()
}
