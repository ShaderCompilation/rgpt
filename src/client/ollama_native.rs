use std::io::{BufRead, BufReader};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::{ChatMessage, ChatRequest, LlmClient};

/// Talks to Ollama's native `/api/chat` endpoint directly, rather than
/// through its OpenAI-compatible shim, so `num_ctx`/`num_predict`/`keep_alive`
/// are reachable — the OpenAI-compat path has no way to express these.
pub struct OllamaClient {
    agent: ureq::Agent,
    base_url: String,
}

#[derive(Serialize)]
struct OllamaRequestOptions {
    temperature: f64,
    top_p: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    options: OllamaRequestOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<&'a str>,
}

#[derive(Deserialize, Default)]
struct OllamaChunkMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct OllamaChunk {
    #[serde(default)]
    message: OllamaChunkMessage,
    #[serde(default)]
    done: bool,
}

impl OllamaClient {
    pub fn new(base_url: &str, timeout_secs: u64) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs)))
            .http_status_as_error(false)
            .build();
        let agent: ureq::Agent = config.into();
        Self {
            agent,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }
}

impl LlmClient for OllamaClient {
    fn stream_chat_completion(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let wire = OllamaRequest {
            model: &request.model,
            messages: &request.messages,
            stream: true,
            options: OllamaRequestOptions {
                temperature: request.temperature,
                top_p: request.top_p,
                num_ctx: request.ollama_options.num_ctx,
                num_predict: request.ollama_options.num_predict,
            },
            keep_alive: request.ollama_options.keep_alive.as_deref(),
        };

        let response = self
            .agent
            .post(&url)
            .send_json(&wire)
            .with_context(|| format!("sending request to {url}"))?;

        let status = response.status();
        let mut reader = BufReader::new(response.into_body().into_reader());

        if !status.is_success() {
            let mut body = String::new();
            std::io::Read::read_to_string(&mut reader, &mut body).ok();
            bail!("request to {url} failed with status {status}: {body}");
        }

        // Ollama streams newline-delimited JSON objects (not SSE).
        let mut full_text = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .context("reading Ollama stream")?;
            if bytes_read == 0 {
                break;
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let chunk: OllamaChunk = serde_json::from_str(line)
                .with_context(|| format!("parsing Ollama chunk: {line}"))?;
            if !chunk.message.content.is_empty() {
                on_delta(&chunk.message.content);
                full_text.push_str(&chunk.message.content);
            }
            if chunk.done {
                break;
            }
        }

        Ok(full_text)
    }
}
