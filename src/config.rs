use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Config keys with their default values. Mirrors shell_gpt's DEFAULT_CONFIG,
/// trimmed to what Phase 1 (single-shot streaming query) actually uses.
/// Later phases add entries here as they need them, no redesign required.
const DEFAULTS: &[(&str, &str)] = &[
    ("DEFAULT_MODEL", "gpt-5.4-mini"),
    ("DEFAULT_TEMPERATURE", "0.0"),
    ("DEFAULT_COLOR", "magenta"),
    ("REQUEST_TIMEOUT", "60"),
    ("API_BASE_URL", "default"),
    ("DISABLE_STREAMING", "false"),
    ("OS_NAME", "auto"),
    ("SHELL_NAME", "auto"),
    ("CHAT_CACHE_LENGTH", "100"),
    ("USE_OLLAMA", "false"),
    ("OLLAMA_BASE_URL", "http://localhost:11434"),
    ("OLLAMA_NUM_CTX", ""),
    ("OLLAMA_NUM_PREDICT", ""),
    ("OLLAMA_KEEP_ALIVE", ""),
    ("MAX_CONTEXT_TOKENS", "0"),
];

pub struct Config {
    values: HashMap<String, String>,
    path: PathBuf,
}

impl Config {
    /// `skip_api_key_prompt` avoids asking for an OpenAI API key on first run
    /// when the user has already indicated (e.g. via `--ollama`) that they
    /// only intend to talk to a local model.
    pub fn load(skip_api_key_prompt: bool) -> Result<Config> {
        let path = config_path()?;
        if path.exists() {
            Self::load_existing(path)
        } else {
            Self::bootstrap(path, skip_api_key_prompt)
        }
    }

    fn load_existing(path: PathBuf) -> Result<Config> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let mut values = HashMap::new();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.to_string(), value.to_string());
            }
        }

        let mut added = false;
        for (key, default) in DEFAULTS {
            if !values.contains_key(*key) {
                values.insert(key.to_string(), default.to_string());
                added = true;
            }
        }

        let config = Config { values, path };
        if added {
            config.write()?;
        }
        Ok(config)
    }

    fn bootstrap(path: PathBuf, skip_api_key_prompt: bool) -> Result<Config> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating config directory {}", parent.display()))?;
        }

        let mut values: HashMap<String, String> = DEFAULTS
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        if !skip_api_key_prompt && std::env::var("OPENAI_API_KEY").is_err() {
            let key = rpassword::prompt_password("Please enter your OpenAI API key: ")
                .context("reading API key from prompt")?;
            values.insert("OPENAI_API_KEY".to_string(), key);
        }

        let config = Config { values, path };
        config.write()?;
        Ok(config)
    }

    fn write(&self) -> Result<()> {
        let mut contents = String::new();
        for (key, value) in &self.values {
            contents.push_str(key);
            contents.push('=');
            contents.push_str(value);
            contents.push('\n');
        }
        fs::write(&self.path, contents)
            .with_context(|| format!("writing config file {}", self.path.display()))
    }

    /// Env vars take priority over the config file, matching shell_gpt.
    pub fn get(&self, key: &str) -> Result<String> {
        if let Ok(v) = std::env::var(key)
            && !v.is_empty()
        {
            return Ok(v);
        }
        self.values
            .get(key)
            .filter(|v| !v.is_empty())
            .cloned()
            .with_context(|| format!("missing config key: {key}"))
    }

    /// Like `get`, but returns `None` instead of erroring when the key is
    /// unset or blank — for genuinely optional settings (Ollama tuning knobs
    /// most users never touch) where absence isn't a misconfiguration.
    pub fn get_opt(&self, key: &str) -> Option<String> {
        if let Ok(v) = std::env::var(key)
            && !v.is_empty()
        {
            return Some(v);
        }
        self.values.get(key).filter(|v| !v.is_empty()).cloned()
    }
}

fn config_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not determine config directory")?;
    Ok(base.join("rgpt").join(".rgptrc"))
}
