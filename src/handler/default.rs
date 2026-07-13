use anyhow::Result;

use crate::client::{ChatMessage, ChatRequest, OpenAiCompatClient};
use crate::render::TextPrinter;

/// Single-shot, no-persistence handler. Chat sessions and system roles
/// (system-message construction) land in later phases.
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
        model: String,
        temperature: f64,
        top_p: f64,
        stream: bool,
    ) -> Result<()> {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];

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
