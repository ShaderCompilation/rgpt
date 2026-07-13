mod ollama_native;
mod openai_compat;

pub use ollama_native::OllamaClient;
pub use openai_compat::OpenAiCompatClient;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
        }
    }

    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
        }
    }
}

/// Ollama-specific generation knobs. Ignored by the OpenAI-compatible client;
/// the native Ollama client maps these onto its `options`/`keep_alive` fields.
#[derive(Clone, Debug, Default)]
pub struct OllamaOptions {
    pub num_ctx: Option<u32>,
    pub num_predict: Option<i32>,
    pub keep_alive: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub temperature: f64,
    pub top_p: f64,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    #[serde(skip)]
    pub ollama_options: OllamaOptions,
}

/// Common interface both backends implement, so handlers don't need to know
/// whether they're talking to an OpenAI-compatible endpoint or Ollama's
/// native `/api/chat`.
pub trait LlmClient {
    /// Streams a chat completion, invoking `on_delta` for each content chunk
    /// as it arrives. Returns the full concatenated completion text.
    fn stream_chat_completion(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String>;
}
