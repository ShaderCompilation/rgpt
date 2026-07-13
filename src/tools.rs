use std::io::Write;

use serde_json::json;

use crate::client::{ToolCall, ToolDefinition, ToolFunctionDefinition};
use crate::render::sanitize_terminal_line;
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
    // The command is model-controlled; sanitize before display so control bytes
    // cannot rewrite the prompt and spoof what is being approved.
    print!(
        "Model wants to execute `{}`. Execute? [y/N] ",
        sanitize_terminal_line(command)
    );
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok();
    is_yes(&answer)
}

/// True only for an explicit yes; anything else (incl. empty/EOF) denies.
pub fn is_yes(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Prompts on the controlling terminal (`/dev/tty`) — not stdin, which may be a
/// pipe — to confirm executing a model-proposed `command`. Returns false,
/// denying execution, when there is no controlling terminal to prompt on. The
/// command is sanitized before display to prevent approval-prompt spoofing.
pub fn confirm_tty(command: &str) -> bool {
    use std::io::{BufRead, BufReader};

    let tty = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    {
        Ok(tty) => tty,
        Err(_) => {
            eprintln!("Refusing to execute: no controlling terminal available to confirm.");
            return false;
        }
    };
    let mut writer = &tty;
    let _ = write!(
        writer,
        "Execute `{}`? [y/N] ",
        sanitize_terminal_line(command)
    );
    let _ = writer.flush();
    let mut answer = String::new();
    if BufReader::new(&tty).read_line(&mut answer).is_err() {
        return false;
    }
    is_yes(&answer)
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
    #[test]
    fn confirmation_denies_by_default() {
        for yes in ["y", "yes", "Y", "YES", " yes \n"] {
            assert!(is_yes(yes), "{yes:?} should confirm");
        }
        for no in ["", "\n", "n", "no", "sure", "yep", "yolo"] {
            assert!(!is_yes(no), "{no:?} should deny");
        }
    }
}
