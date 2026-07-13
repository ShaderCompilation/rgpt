use std::io::{BufRead, BufReader};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{ChatRequest, Completion, LlmClient, ToolCall};

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
    #[serde(default)]
    tool_calls: Vec<WireToolCall>,
}

#[derive(Deserialize)]
struct WireToolCall {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: String,
    function: WireFunction,
}
#[derive(Deserialize)]
struct WireFunction {
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: String,
}

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    temperature: f64,
    top_p: f64,
    messages: Vec<OpenAiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [super::ToolDefinition]>,
}
#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}
#[derive(Serialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: OpenAiFunction,
}
#[derive(Serialize)]
struct OpenAiFunction {
    name: String,
    arguments: String,
}

fn openai_message(message: &super::ChatMessage) -> OpenAiMessage {
    OpenAiMessage {
        role: message.role.clone(),
        content: message.content.clone(),
        tool_call_id: message.tool_call_id.clone(),
        tool_calls: message.tool_calls.as_ref().map(|calls| {
            calls
                .iter()
                .map(|call| OpenAiToolCall {
                    id: call.id.clone(),
                    kind: "function",
                    function: OpenAiFunction {
                        name: call.name.clone(),
                        arguments: call.arguments.to_string(),
                    },
                })
                .collect()
        }),
    }
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
    ) -> Result<Completion> {
        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send_json(&OpenAiRequest {
                model: &request.model,
                temperature: request.temperature,
                top_p: request.top_p,
                messages: request.messages.iter().map(openai_message).collect(),
                stream: request.stream,
                tools: request.tools.as_deref(),
            })
            .with_context(|| format!("sending request to {url}"))?;

        let status = response.status();
        let mut reader = BufReader::new(response.into_body().into_reader());

        if !status.is_success() {
            let mut body = String::new();
            std::io::Read::read_to_string(&mut reader, &mut body).ok();
            bail!("request to {url} failed with status {status}: {body}");
        }

        let mut completion = Completion::default();
        let mut line = String::new();
        let mut pending_calls: BTreeMap<usize, (String, String, String)> = BTreeMap::new();
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
                completion.content.push_str(&content);
            }
            for call in choice.delta.tool_calls {
                let entry = pending_calls.entry(call.index).or_default();
                if !call.id.is_empty() {
                    entry.0 = call.id;
                }
                if !call.function.name.is_empty() {
                    entry.1 = call.function.name;
                }
                entry.2.push_str(&call.function.arguments);
            }
        }
        for (_, (id, name, arguments)) in pending_calls {
            let arguments = serde_json::from_str(&arguments)
                .with_context(|| format!("parsing arguments for tool {name}"))?;
            completion.tool_calls.push(ToolCall {
                id,
                name,
                arguments,
            });
        }

        Ok(completion)
    }
}
