use std::io::{BufRead, BufReader};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::{ChatRequest, LlmClient};

pub struct OpenAiCompatClient {
    agent: ureq::Agent,
    base_url: String,
    api_key: String,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
}

#[derive(Deserialize, Default)]
struct ChunkDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
}

impl OpenAiCompatClient {
    pub fn new(base_url: &str, api_key: &str, timeout_secs: u64) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs)))
            .http_status_as_error(false)
            .build();
        let agent: ureq::Agent = config.into();
        Self {
            agent,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
        }
    }
}

impl LlmClient for OpenAiCompatClient {
    fn stream_chat_completion(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(request)
            .with_context(|| format!("sending request to {url}"))?;

        let status = response.status();
        let mut reader = BufReader::new(response.into_body().into_reader());

        if !status.is_success() {
            let mut body = String::new();
            std::io::Read::read_to_string(&mut reader, &mut body).ok();
            bail!("request to {url} failed with status {status}: {body}");
        }

        let mut full_text = String::new();
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).context("reading SSE stream")?;
            if bytes_read == 0 {
                break;
            }
            let line = line.trim_end();
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data == "[DONE]" {
                break;
            }
            let chunk: Chunk =
                serde_json::from_str(data).with_context(|| format!("parsing SSE chunk: {data}"))?;
            let Some(choice) = chunk.choices.into_iter().next() else {
                continue;
            };
            if let Some(content) = choice.delta.content
                && !content.is_empty()
            {
                on_delta(&content);
                full_text.push_str(&content);
            }
        }

        Ok(full_text)
    }
}
