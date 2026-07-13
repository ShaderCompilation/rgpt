use std::io::{BufRead, BufReader};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::{ChatMessage, ChatRequest, Completion, LlmClient, ToolCall};
use crate::debug::DebugLog;

/// Talks to Ollama's native `/api/chat` endpoint directly, rather than
/// through its OpenAI-compatible shim, so `num_ctx`/`num_predict`/`keep_alive`
/// are reachable — the OpenAI-compat path has no way to express these.
pub struct OllamaClient {
    agent: ureq::Agent,
    base_url: String,
    debug: Option<DebugLog>,
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
    messages: Vec<OllamaMessage>,
    stream: bool,
    options: OllamaRequestOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [super::ToolDefinition]>,
    think: bool,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCallRequest>>,
}
#[derive(Serialize)]
struct OllamaToolCallRequest {
    function: OllamaFunctionRequest,
}
#[derive(Serialize)]
struct OllamaFunctionRequest {
    name: String,
    arguments: serde_json::Value,
}

fn ollama_message(message: &ChatMessage) -> OllamaMessage {
    OllamaMessage {
        role: message.role.clone(),
        content: message.content.clone(),
        tool_calls: message.tool_calls.as_ref().map(|calls| {
            calls
                .iter()
                .map(|call| OllamaToolCallRequest {
                    function: OllamaFunctionRequest {
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                    },
                })
                .collect()
        }),
    }
}

#[derive(Deserialize, Default)]
struct OllamaChunkMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Deserialize)]
struct OllamaToolCall {
    #[serde(default)]
    id: String,
    function: OllamaFunction,
}
#[derive(Deserialize)]
struct OllamaFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Deserialize)]
struct OllamaChunk {
    #[serde(default)]
    message: OllamaChunkMessage,
    #[serde(default)]
    done: bool,
}

impl OllamaClient {
    pub fn new(base_url: &str, timeout_secs: u64, debug: Option<DebugLog>) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs)))
            .http_status_as_error(false)
            .build();
        let agent: ureq::Agent = config.into();
        Self {
            agent,
            base_url: base_url.trim_end_matches('/').to_string(),
            debug,
        }
    }
}

impl LlmClient for OllamaClient {
    fn stream_chat_completion(
        &self,
        request: &ChatRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<Completion> {
        let url = format!("{}/api/chat", self.base_url);
        let wire = OllamaRequest {
            model: &request.model,
            messages: request.messages.iter().map(ollama_message).collect(),
            stream: true,
            options: OllamaRequestOptions {
                temperature: request.temperature,
                top_p: request.top_p,
                num_ctx: request.ollama_options.num_ctx,
                num_predict: request.ollama_options.num_predict,
            },
            keep_alive: request.ollama_options.keep_alive.as_deref(),
            tools: request.tools.as_deref(),
            think: request.think,
        };

        if let Some(log) = &self.debug {
            log.section(
                "REQUEST",
                &format!(
                    "POST {url}\n{}",
                    serde_json::to_string_pretty(&wire).unwrap_or_default()
                ),
            );
        }

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
            if let Some(log) = &self.debug {
                log.section("ERROR RESPONSE", &format!("status {status}\n{body}"));
            }
            bail!("request to {url} failed with status {status}: {body}");
        }

        // Ollama streams newline-delimited JSON objects (not SSE).
        let mut completion = Completion::default();
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
            if let Some(log) = &self.debug {
                log.line("RECV", line);
            }
            let chunk: OllamaChunk = serde_json::from_str(line)
                .with_context(|| format!("parsing Ollama chunk: {line}"))?;
            if !chunk.message.content.is_empty() {
                on_delta(&chunk.message.content);
                completion.content.push_str(&chunk.message.content);
            }
            for (index, call) in chunk.message.tool_calls.into_iter().enumerate() {
                completion.tool_calls.push(ToolCall {
                    id: if call.id.is_empty() {
                        format!("ollama-{index}")
                    } else {
                        call.id
                    },
                    name: call.function.name,
                    arguments: call.function.arguments,
                });
            }
            if chunk.done {
                break;
            }
        }

        if let Some(log) = &self.debug {
            log.section("PARSED COMPLETION", &format!("{completion:#?}"));
        }

        Ok(completion)
    }
}
