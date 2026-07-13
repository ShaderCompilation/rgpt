mod openai_compat;

pub use openai_compat::OpenAiCompatClient;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub temperature: f64,
    pub top_p: f64,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}
