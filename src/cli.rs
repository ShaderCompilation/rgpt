use clap::Parser;

/// A fast, local-model-friendly CLI for LLM chat completions.
#[derive(Parser, Debug)]
#[command(name = "rgpt", version, about)]
pub struct Cli {
    /// The prompt to generate completions for.
    pub prompt: Option<String>,

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
        Ok(())
    }
}

/// If stdin is not a tty, read piped input until EOF or a line containing
/// the `__sgpt__eof__` sentinel, then combine it with the CLI prompt argument.
pub fn resolve_prompt(cli_prompt: Option<&str>) -> anyhow::Result<String> {
    use std::io::IsTerminal;

    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Ok(cli_prompt.unwrap_or_default().trim().to_string());
    }

    let mut piped = String::new();
    for line in std::io::BufRead::lines(stdin.lock()) {
        let line = line?;
        if line.contains("__sgpt__eof__") {
            break;
        }
        piped.push_str(&line);
        piped.push('\n');
    }

    let combined = match cli_prompt {
        Some(p) if !p.is_empty() => format!("{piped}\n\n{p}"),
        _ => piped,
    };
    Ok(combined.trim().to_string())
}
