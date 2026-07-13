use anyhow::Result;

use crate::client::{ChatMessage, ChatRequest, OpenAiCompatClient};
use crate::render::TextPrinter;
use crate::role::SystemRole;

/// Single-shot, no-persistence handler. Chat session persistence lands in a later phase.
pub struct DefaultHandler<'a> {
    client: &'a OpenAiCompatClient,
    printer: TextPrinter,
}

impl<'a> DefaultHandler<'a> {
    pub fn new(client: &'a OpenAiCompatClient, color: String) -> Self {
        Self {
            client,
            printer: TextPrinter::new(color),
        }
    }

    pub fn handle(
        &self,
        prompt: &str,
        role: &SystemRole,
        model: String,
        temperature: f64,
        top_p: f64,
        stream: bool,
    ) -> Result<()> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: role.role.clone(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ];

        let request = ChatRequest {
            model,
            temperature,
            top_p,
            messages,
            stream: true,
        };

        if stream {
            self.client
                .stream_chat_completion(&request, |chunk| self.printer.print_chunk(chunk))?;
            self.printer.finish_stream();
        } else {
            let full_text = self.client.stream_chat_completion(&request, |_| {})?;
            self.printer.print_full(&full_text);
        }

        Ok(())
    }
}
