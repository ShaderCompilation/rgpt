use anyhow::Result;

use super::CompletionParams;
use crate::client::{ChatMessage, ChatRequest, LlmClient};
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

    pub fn handle(&self, prompt: &str, role: &SystemRole, params: CompletionParams) -> Result<()> {
        let messages = vec![
            ChatMessage::system(role.role.clone()),
            ChatMessage::user(prompt.to_string()),
        ];

        let request = ChatRequest {
            model: params.model,
            temperature: params.temperature,
            top_p: params.top_p,
            messages,
            stream: true,
            ollama_options: params.ollama_options,
        };

        if params.stream {
            self.client
                .stream_chat_completion(&request, &mut |chunk| self.printer.print_chunk(chunk))?;
            self.printer.finish_stream();
        } else {
            let full_text = self.client.stream_chat_completion(&request, &mut |_| {})?;
            self.printer.print_full(&full_text);
        }

        Ok(())
    }
}
