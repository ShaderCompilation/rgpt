use anyhow::Result;

use super::CompletionParams;
use crate::client::{ChatMessage, LlmClient};
use crate::render::TextPrinter;
use crate::role::SystemRole;

/// Single-shot, no-persistence handler. Chat session persistence lands in a later phase.
pub struct DefaultHandler<'a> {
    client: &'a dyn LlmClient,
    printer: TextPrinter,
}

impl<'a> DefaultHandler<'a> {
    pub fn new(client: &'a dyn LlmClient, color: String) -> Self {
        Self {
            client,
            printer: TextPrinter::new(color),
        }
    }

    pub fn handle(
        &self,
        prompt: &str,
        role: &SystemRole,
        params: CompletionParams,
    ) -> Result<String> {
        let messages = vec![
            ChatMessage::system(role.role.clone()),
            ChatMessage::user(prompt.to_string()),
        ];

        Ok(super::complete_with_tools(self.client, &self.printer, messages, &params)?.0)
    }
}
