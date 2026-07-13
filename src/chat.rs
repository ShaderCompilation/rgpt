use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::client::ChatMessage;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatSession {
    pub role_name: String,
    pub messages: Vec<ChatMessage>,
}

impl ChatSession {
    pub fn load(chat_id: &str) -> Result<Option<Self>> {
        let path = Self::path(chat_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("reading chat file {}", path.display()))?;
        let session: Self = serde_json::from_str(&contents)
            .with_context(|| format!("parsing chat file {}", path.display()))?;
        Ok(Some(session))
    }

    /// Persists the session, retaining the first message (the system role)
    /// plus only the most recent `max_length - 1` messages after it, mirroring
    /// shell_gpt's count-based truncation.
    pub fn save(&self, chat_id: &str, max_length: usize) -> Result<()> {
        let dir = Self::storage_dir()?;
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating chat directory {}", dir.display()))?;

        let messages = if self.messages.len() > max_length {
            let tail_start = self.messages.len() - max_length.saturating_sub(1);
            let mut kept = self.messages[..1].to_vec();
            kept.extend_from_slice(&self.messages[tail_start..]);
            kept
        } else {
            self.messages.clone()
        };

        let path = Self::path(chat_id)?;
        let contents = serde_json::to_string(&ChatSession {
            role_name: self.role_name.clone(),
            messages,
        })
        .context("serializing chat session")?;
        fs::write(&path, contents).with_context(|| format!("writing chat file {}", path.display()))
    }

    pub fn delete(chat_id: &str) -> Result<()> {
        let path = Self::path(chat_id)?;
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("removing chat file {}", path.display()))?;
        }
        Ok(())
    }

    /// Lists chat file paths, sorted by last-modified time ascending (oldest first).
    pub fn list() -> Result<Vec<PathBuf>> {
        let dir = Self::storage_dir()?;
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(&dir)
            .with_context(|| format!("reading chat directory {}", dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let mtime = entry.metadata().ok()?.modified().ok()?;
                Some((mtime, entry.path()))
            })
            .collect();
        entries.sort_by_key(|(mtime, _)| *mtime);
        Ok(entries.into_iter().map(|(_, path)| path).collect())
    }

    fn path(chat_id: &str) -> Result<PathBuf> {
        Ok(Self::storage_dir()?.join(chat_id))
    }

    fn storage_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().context("could not determine config directory")?;
        Ok(base.join("rgpt").join("chat_cache"))
    }
}
