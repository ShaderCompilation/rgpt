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
