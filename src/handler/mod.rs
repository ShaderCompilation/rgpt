mod chat;
mod default;
mod repl;

pub use chat::{ChatHandler, show_chat};
pub use default::DefaultHandler;
pub use repl::ReplHandler;

use crate::cache::ResponseCache;
use crate::client::{ChatMessage, ChatRequest, LlmClient, OllamaOptions};
use crate::render::TextPrinter;
use crate::tools;
use anyhow::{Result, bail};

/// Model-invocation options shared by every handler, bundled to keep
/// `handle()` signatures from accumulating unrelated positional args.
#[derive(Clone)]
pub struct CompletionParams {
    pub model: String,
    pub temperature: f64,
    pub top_p: f64,
    pub stream: bool,
    /// Ignored unless the active client is `OllamaClient`.
    pub ollama_options: OllamaOptions,
    pub no_interaction: bool,
    pub cache_length: usize,
}

/// Runs a completion and any requested built-in tools iteratively. Unlike the
/// Python reference this never recursively re-enters completion handling.
pub(crate) fn complete_with_tools(
    client: &dyn LlmClient,
    printer: &TextPrinter,
    mut messages: Vec<ChatMessage>,
    params: &CompletionParams,
) -> Result<(String, Vec<ChatMessage>)> {
    let cache = ResponseCache::new(params.cache_length);
    let cache_key = serde_json::to_string(&(
        params.model.as_str(),
        params.temperature.to_bits(),
        params.top_p.to_bits(),
        &messages,
    ))?;
    if let Some(text) = cache.get(&cache_key)? {
        printer.print_full(&text);
        messages.push(ChatMessage::assistant(text.clone()));
        return Ok((text, messages));
    }

    let mut used_tools = false;
    for _ in 0..tools::MAX_TOOL_ROUNDS {
        let request = ChatRequest {
            model: params.model.clone(),
            temperature: params.temperature,
            top_p: params.top_p,
            messages: messages.clone(),
            stream: params.stream,
            tools: Some(tools::definitions()),
            ollama_options: params.ollama_options.clone(),
        };
        let completion = if params.stream {
            let result =
                client.stream_chat_completion(&request, &mut |chunk| printer.print_chunk(chunk))?;
            if completion_needs_newline(&result.content) {
                printer.finish_stream();
            }
            result
        } else {
            let result = client.stream_chat_completion(&request, &mut |_| {})?;
            if !result.content.is_empty() {
                printer.print_full(&result.content);
            }
            result
        };
        if completion.tool_calls.is_empty() {
            messages.push(ChatMessage::assistant(completion.content.clone()));
            // A tool invocation may have an external side effect. Never cache
            // any turn that used one, even when its final model response is text.
            if !used_tools {
                cache.put(&cache_key, &completion.content)?;
            }
            return Ok((completion.content, messages));
        }
        used_tools = true;
        messages.push(ChatMessage::assistant_tool_calls(
            completion.content,
            completion.tool_calls.clone(),
        ));
        for call in completion.tool_calls {
            let result = tools::execute(&call, params.no_interaction);
            messages.push(ChatMessage::tool(call.id, result));
        }
    }
    bail!(
        "model exceeded the {}-round tool-call limit",
        tools::MAX_TOOL_ROUNDS
    )
}

fn completion_needs_newline(content: &str) -> bool {
    !content.is_empty()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::client::{Completion, ToolCall};

    struct ScriptedClient {
        requests: Mutex<Vec<ChatRequest>>,
    }

    impl LlmClient for ScriptedClient {
        fn stream_chat_completion(
            &self,
            request: &ChatRequest,
            _on_delta: &mut dyn FnMut(&str),
        ) -> Result<Completion> {
            let mut requests = self.requests.lock().unwrap();
            let response = if requests.is_empty() {
                Completion {
                    content: String::new(),
                    tool_calls: vec![ToolCall {
                        id: "call-1".into(),
                        name: "execute_shell_command".into(),
                        arguments: json!({"command":"true"}),
                    }],
                }
            } else {
                Completion {
                    content: "done".into(),
                    tool_calls: Vec::new(),
                }
            };
            requests.push(ChatRequest {
                model: request.model.clone(),
                temperature: request.temperature,
                top_p: request.top_p,
                messages: request.messages.clone(),
                stream: request.stream,
                tools: None,
                ollama_options: request.ollama_options.clone(),
            });
            Ok(response)
        }
    }

    #[test]
    fn tool_calls_are_iterative_and_returned_to_the_model() {
        let client = ScriptedClient {
            requests: Mutex::new(Vec::new()),
        };
        let printer = TextPrinter::new("none".into());
        let params = CompletionParams {
            model: "test".into(),
            temperature: 0.0,
            top_p: 1.0,
            stream: false,
            ollama_options: OllamaOptions::default(),
            no_interaction: true,
            cache_length: 0,
        };
        let (text, _) = complete_with_tools(
            &client,
            &printer,
            vec![ChatMessage::user("go".into())],
            &params,
        )
        .unwrap();
        assert_eq!(text, "done");
        let requests = client.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[1]
                .messages
                .iter()
                .any(|message| message.role == "tool"
                    && message.tool_call_id.as_deref() == Some("call-1"))
        );
    }
}
