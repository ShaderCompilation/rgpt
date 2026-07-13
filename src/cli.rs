use clap::{ArgAction, Parser};

/// A fast, local-model-friendly CLI for LLM chat completions.
#[derive(Parser, Debug)]
#[command(name = "rgpt", version, about)]
pub struct Cli {
    /// The prompt to generate completions for.
    pub prompt: Option<String>,

    /// Generate a shell command and offer to execute it.
    #[arg(short = 's', long, help_heading = "Assistance Options")]
    pub shell: bool,

    /// Describe a shell command.
    #[arg(short = 'd', long, help_heading = "Assistance Options")]
    pub describe_shell: bool,

    /// Generate code only.
    #[arg(short = 'c', long, help_heading = "Assistance Options")]
    pub code: bool,

    /// Do not show the shell-command confirmation prompt.
    #[arg(long, action = ArgAction::SetTrue, help_heading = "Assistance Options")]
    pub no_interaction: bool,

    /// Open $EDITOR to compose the prompt.
    #[arg(long, help_heading = "Assistance Options")]
    pub editor: bool,

    /// Remove rgpt, including its saved configuration, chats, roles, and cache.
    #[arg(long, help_heading = "Application Options")]
    pub uninstall: bool,

    /// Do not ask for confirmation when uninstalling.
    #[arg(long, requires = "uninstall", help_heading = "Application Options")]
    pub yes: bool,

    /// Large language model to use.
    #[arg(long)]
    pub model: Option<String>,

    /// Randomness of generated output (0.0-2.0).
    #[arg(long)]
    pub temperature: Option<f64>,

    /// Limits highest probable tokens (0.0-1.0).
    #[arg(long, default_value_t = 1.0)]
    pub top_p: f64,

    /// System role to use for the model.
    #[arg(long, help_heading = "Role Options")]
    pub role: Option<String>,

    /// Create a new role (prompts for a description).
    #[arg(long, value_name = "NAME", help_heading = "Role Options")]
    pub create_role: Option<String>,

    /// Show an existing role's content.
    #[arg(long, value_name = "NAME", help_heading = "Role Options")]
    pub show_role: Option<String>,

    /// List all available roles.
    #[arg(long, help_heading = "Role Options")]
    pub list_roles: bool,

    /// Follow conversation with id, use "temp" for a quick session.
    #[arg(long, value_name = "ID", help_heading = "Chat Options")]
    pub chat: Option<String>,

    /// Start a REPL (read-eval-print loop) session with id, use "temp" for a quick session.
    #[arg(long, value_name = "ID", help_heading = "Chat Options")]
    pub repl: Option<String>,

    /// Show all messages from the given chat id.
    #[arg(long, value_name = "ID", help_heading = "Chat Options")]
    pub show_chat: Option<String>,

    /// List all existing chat ids.
    #[arg(long, help_heading = "Chat Options")]
    pub list_chats: bool,

    /// Use Ollama's native /api/chat endpoint instead of the OpenAI-compatible API.
    #[arg(long, help_heading = "Ollama Options")]
    pub ollama: bool,

    /// Ollama: context window size in tokens (num_ctx).
    #[arg(long, help_heading = "Ollama Options")]
    pub num_ctx: Option<u32>,

    /// Ollama: max tokens to generate (num_predict).
    #[arg(long, help_heading = "Ollama Options")]
    pub num_predict: Option<i32>,

    /// Ollama: how long to keep the model loaded after the request, e.g. "5m", "-1" for forever.
    #[arg(long, help_heading = "Ollama Options")]
    pub keep_alive: Option<String>,

    /// Ollama: enable model thinking/reasoning output (disabled by default).
    #[arg(long, help_heading = "Ollama Options")]
    pub think: bool,
}

impl Cli {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(t) = self.temperature
            && !(0.0..=2.0).contains(&t)
        {
            anyhow::bail!("--temperature must be between 0.0 and 2.0, got {t}");
        }
        if !(0.0..=1.0).contains(&self.top_p) {
            anyhow::bail!("--top-p must be between 0.0 and 1.0, got {}", self.top_p);
        }
        if self.chat.is_some() && self.repl.is_some() {
            anyhow::bail!("--chat and --repl options cannot be used together.");
        }
        if [self.shell, self.describe_shell, self.code]
            .into_iter()
            .filter(|enabled| *enabled)
            .count()
            > 1
        {
            anyhow::bail!(
                "only one of --shell, --describe-shell, and --code can be used at a time"
            );
        }
        Ok(())
    }
}

/// If stdin is not a tty, read piped input until EOF or a line containing
/// the `__sgpt__eof__` sentinel, then combine it with the CLI prompt argument.
pub struct ResolvedPrompt {
    pub prompt: String,
    /// Lines after the sentinel are reserved for a piped REPL session.
    pub repl_input: Option<Vec<String>>,
    pub stdin_was_piped: bool,
}

pub fn resolve_prompt(cli_prompt: Option<&str>) -> anyhow::Result<ResolvedPrompt> {
    use std::io::IsTerminal;

    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(ResolvedPrompt {
            prompt: cli_prompt.unwrap_or_default().trim().to_string(),
            repl_input: None,
            stdin_was_piped: false,
        });
    }

    let mut input = String::new();
    std::io::Read::read_to_string(&mut stdin.lock(), &mut input)?;
    Ok(resolve_piped_prompt(cli_prompt, &input))
}

fn resolve_piped_prompt(cli_prompt: Option<&str>, input: &str) -> ResolvedPrompt {
    let (piped, repl_input) = match input.split_once("__sgpt__eof__") {
        Some((before, after)) => (
            before.to_string(),
            Some(
                after
                    .strip_prefix("\r\n")
                    .or_else(|| after.strip_prefix('\n'))
                    .unwrap_or(after)
                    .lines()
                    .map(|line| line.trim_end_matches('\r').to_string())
                    .collect(),
            ),
        ),
        None => (input.to_string(), None),
    };

    let combined = match cli_prompt {
        Some(p) if !p.is_empty() => format!("{piped}\n\n{p}"),
        _ => piped,
    };
    ResolvedPrompt {
        prompt: combined.trim().to_string(),
        repl_input,
        stdin_was_piped: true,
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn assistance_modes_are_exclusive() {
        let cli = Cli::try_parse_from(["rgpt", "--shell", "--code"]);
        assert!(cli.is_ok());
        assert!(cli.unwrap().validate().is_err());
    }

    #[test]
    fn sentinel_preserves_repl_input_after_initial_prompt() {
        let resolved = super::resolve_piped_prompt(
            Some("follow-up"),
            "initial context\n__sgpt__eof__\nfirst turn\nexit()\n",
        );
        assert_eq!(resolved.prompt, "initial context\n\n\nfollow-up");
        assert_eq!(resolved.repl_input.unwrap(), vec!["first turn", "exit()"]);
    }
}
