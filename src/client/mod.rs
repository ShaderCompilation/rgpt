mod ollama_native;
mod openai_compat;

pub use ollama_native::OllamaClient;
pub use openai_compat::OpenAiCompatClient;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: String) -> Self {
        Self {
            role: "system".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(content: String, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool(tool_call_id: String, content: String) -> Self {
        Self {
            role: "tool".to_string(),
            content,
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
        }
    }
}

/// A model-requested function invocation. Arguments remain JSON until the
/// registry validates them, so malformed model output never reaches a tool.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Serialize, Clone, Debug)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub function: ToolFunctionDefinition,
}

#[derive(Serialize, Clone, Debug)]
pub struct ToolFunctionDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Clone, Default)]
pub struct Completion {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// Whether the model should emit thinking/reasoning output. Off by
    /// default; mapped onto whichever convention the active backend uses.
    #[serde(skip)]
    pub think: bool,
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
    ) -> Result<Completion>;
}
