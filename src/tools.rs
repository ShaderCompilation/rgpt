use std::io::Write;

use serde_json::json;

use crate::client::{ToolCall, ToolDefinition, ToolFunctionDefinition};
use crate::shell_cmd;

pub const MAX_TOOL_ROUNDS: usize = 8;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        kind: "function",
        function: ToolFunctionDefinition {
            name: "execute_shell_command",
            description: "Execute a shell command on the user's machine. Ask only when execution is necessary.",
            parameters: json!({"type":"object", "properties":{"command":{"type":"string", "description":"POSIX shell command to execute"}}, "required":["command"], "additionalProperties":false}),
        },
    }]
}

/// Executes only recognized built-ins. `no_interaction` is the explicit
/// scripting opt-out; interactive sessions otherwise default-deny.
pub fn execute(call: &ToolCall, no_interaction: bool) -> String {
    match call.name.as_str() {
        "execute_shell_command" => execute_shell(call, no_interaction),
        _ => format!("Tool error: unknown tool {:?}.", call.name),
    }
}

fn execute_shell(call: &ToolCall, no_interaction: bool) -> String {
    let Some(command) = call.arguments.get("command").and_then(|v| v.as_str()) else {
        return "Tool error: execute_shell_command requires a string `command`.".to_string();
    };
    if !no_interaction && !confirm(command) {
        return "Execution denied by user.".to_string();
    }
    match shell_cmd::run(command) {
        Ok(()) => "Command completed successfully.".to_string(),
        Err(error) => format!("Command failed: {error:#}"),
    }
}

fn confirm(command: &str) -> bool {
    print!("Model wants to execute `{command}`. Execute? [y/N] ");
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_arguments_are_not_executed() {
        let call = ToolCall {
            id: "1".into(),
            name: "execute_shell_command".into(),
            arguments: json!({}),
        };
        assert!(execute(&call, true).contains("requires a string"));
    }
    #[test]
    fn registry_exposes_the_shell_tool() {
        assert_eq!(definitions()[0].function.name, "execute_shell_command");
    }
}
