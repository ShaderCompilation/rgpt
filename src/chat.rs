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

/// Rough token estimate for text, used in place of a real tokenizer since
/// the target model (and thus its tokenizer) varies per request. ~4 chars/token
/// is a common approximation for English text; good enough for a truncation
/// safety margin, not exact accounting.
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4).max(1)
}

/// Trims `messages` to fit within `budget` estimated tokens for the outgoing
/// request, always keeping a leading system message and otherwise dropping
/// the oldest messages first. A budget of 0 disables truncation (returns
/// `messages` unchanged) — the default, since most remote models have context
/// windows large enough that this never matters, but small local models do.
pub fn truncate_by_token_budget(messages: Vec<ChatMessage>, budget: usize) -> Vec<ChatMessage> {
    if budget == 0 || messages.is_empty() {
        return messages;
    }

    let has_system = messages[0].role == "system";
    let head: Vec<ChatMessage> = if has_system {
        vec![messages[0].clone()]
    } else {
        Vec::new()
    };
    let rest = &messages[head.len()..];

    let head_tokens: usize = head.iter().map(|m| estimate_tokens(&m.content)).sum();
    if head_tokens >= budget {
        return head;
    }

    let mut kept: Vec<ChatMessage> = Vec::new();
    let mut used = head_tokens;
    for message in rest.iter().rev() {
        let tokens = estimate_tokens(&message.content);
        if used + tokens > budget && !kept.is_empty() {
            break;
        }
        used += tokens;
        kept.push(message.clone());
    }
    kept.reverse();

    let mut result = head;
    result.extend(kept);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn zero_budget_disables_truncation() {
        let messages = vec![msg("system", "sys"), msg("user", "hello")];
        assert_eq!(truncate_by_token_budget(messages.clone(), 0), messages);
    }

    #[test]
    fn keeps_system_and_drops_oldest_first() {
        let messages = vec![
            msg("system", "s"),                   // ~1 token
            msg("user", "a".repeat(40).as_str()), // ~10 tokens, oldest turn
            msg("assistant", "b".repeat(40).as_str()),
            msg("user", "c".repeat(40).as_str()), // most recent turn
        ];
        // Budget fits system + only the last message.
        let result = truncate_by_token_budget(messages.clone(), 12);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "system");
        assert_eq!(result[1].content, messages[3].content);
    }

    #[test]
    fn always_keeps_at_least_one_trailing_message() {
        let messages = vec![msg("system", "s"), msg("user", &"x".repeat(1000))];
        // Budget smaller than the single trailing message: still kept, not dropped to empty.
        let result = truncate_by_token_budget(messages.clone(), 5);
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].content, messages[1].content);
    }

    #[test]
    fn no_system_message_still_works() {
        let messages = vec![msg("user", "a"), msg("assistant", "b")];
        let result = truncate_by_token_budget(messages.clone(), 100);
        assert_eq!(result, messages);
    }
}
