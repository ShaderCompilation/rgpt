mod openai_compat;

pub use openai_compat::OpenAiCompatClient;

use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub temperature: f64,
    pub top_p: f64,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}
