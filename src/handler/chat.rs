use anyhow::{Result, bail};

use super::CompletionParams;
use crate::chat::{ChatSession, truncate_by_token_budget};
use crate::client::{ChatMessage, LlmClient};
use crate::render::TextPrinter;
use crate::role::SystemRole;

/// Chat handler: like DefaultHandler, but loads/persists a named message
/// history around each turn instead of sending a single isolated exchange.
pub struct ChatHandler<'a> {
    client: &'a dyn LlmClient,
    printer: TextPrinter,
    color: String,
    chat_id: String,
}

impl<'a> ChatHandler<'a> {
    pub fn new(client: &'a dyn LlmClient, color: String, chat_id: String) -> Result<Self> {
        if chat_id == "temp" {
            // "temp" is a quick, throwaway session: wipe any leftover history
            // from a previous run before this one starts.
            ChatSession::delete(&chat_id)?;
        }
        Ok(Self {
            client,
            printer: TextPrinter::new(color.clone()),
            color,
            chat_id,
        })
    }

    pub fn exists(&self) -> Result<bool> {
        Ok(ChatSession::load(&self.chat_id)?.is_some_and(|s| !s.messages.is_empty()))
    }

    pub fn show_history(&self) -> Result<()> {
        show_chat(&self.chat_id, &self.color)
    }

    /// Resolves which role to use for this turn and the prior messages, if any.
    /// If the chat already exists and the caller passed an explicit `--role`
    /// that differs from the one it was initiated with, that's a conflict.
    /// Otherwise, an unspecified role falls back to whatever the chat was
    /// started with, rather than the ambient default role.
    fn load_role(
        &self,
        role: &SystemRole,
        explicit_role: bool,
    ) -> Result<(SystemRole, Vec<ChatMessage>)> {
        match ChatSession::load(&self.chat_id)? {
            Some(session) => {
                if explicit_role && role.name != session.role_name {
                    bail!(
                        "Can't change chat role to \"{}\" since it was initiated as \"{}\" chat.",
                        role.name,
                        session.role_name
                    );
                }
                let effective_role = if explicit_role {
                    role.clone()
                } else {
                    SystemRole::get(&session.role_name)?
                };
                Ok((effective_role, session.messages))
            }
            None => Ok((role.clone(), Vec::new())),
        }
    }

    pub fn handle(
        &self,
        prompt: &str,
        role: &SystemRole,
        explicit_role: bool,
        params: CompletionParams,
        cache_length: usize,
        max_context_tokens: usize,
    ) -> Result<String> {
        let (effective_role, mut messages) = self.load_role(role, explicit_role)?;
        if messages.is_empty() {
            messages.push(ChatMessage::system(effective_role.role.clone()));
        }
        messages.push(ChatMessage::user(prompt.to_string()));

        // Truncated only for the outgoing request — the persisted history
        // below keeps everything (subject to `cache_length`), so a tight
        // token budget for a small local model doesn't discard saved turns.
        let request_messages = truncate_by_token_budget(messages.clone(), max_context_tokens);
        let (full_text, request_history) =
            super::complete_with_tools(self.client, &self.printer, request_messages, &params)?;
        // Preserve the exact history the model saw, including tool calls and
        // their results, so follow-up turns have the necessary context.
        messages = request_history;
        ChatSession {
            role_name: effective_role.name.clone(),
            messages,
        }
        .save(&self.chat_id, cache_length)?;

        Ok(full_text)
    }
}

/// Prints a chat's message history, alternating colors by index the same
/// way shell_gpt's plain-text (non-markdown) chat display does.
pub fn show_chat(chat_id: &str, color: &str) -> Result<()> {
    let Some(session) = ChatSession::load(chat_id)? else {
        bail!("Chat \"{chat_id}\" not found.");
    };

    let primary = TextPrinter::new(color.to_string());
    let secondary = TextPrinter::new("green".to_string());
    for (i, message) in session.messages.iter().enumerate() {
        let line = format!("{}: {}", message.role, message.content);
        if i % 2 == 0 {
            primary.print_full(&line);
        } else {
            secondary.print_full(&line);
        }
    }
    Ok(())
}
