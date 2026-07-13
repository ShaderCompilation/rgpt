use std::io::{IsTerminal, Write};

/// Minimal ANSI color support, mirroring the named colors shell_gpt exposes
/// via `typer.secho(fg=...)`. Markdown/live-region rendering is a later phase.
fn ansi_code(color: &str) -> &'static str {
    match color.to_lowercase().as_str() {
        "black" => "30",
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "blue" => "34",
        "magenta" => "35",
        "cyan" => "36",
        "white" => "37",
        "bright_black" => "90",
        "bright_red" => "91",
        "bright_green" => "92",
        "bright_yellow" => "93",
        "bright_blue" => "94",
        "bright_magenta" => "95",
        "bright_cyan" => "96",
        "bright_white" => "97",
        _ => "39", // default foreground
    }
}

pub struct TextPrinter {
    color: String,
    colorize: bool,
}

impl TextPrinter {
    pub fn new(color: String) -> Self {
        let colorize = std::io::stdout().is_terminal();
        Self { color, colorize }
    }

    fn colored(&self, text: &str) -> String {
        if self.colorize {
            let code = ansi_code(&self.color);
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    /// Writes one streamed chunk immediately, colored, with no trailing newline.
    pub fn print_chunk(&self, chunk: &str) {
        print!("{}", self.colored(chunk));
        let _ = std::io::stdout().flush();
    }

    /// Writes the full completion at once (used when streaming is disabled).
    pub fn print_full(&self, text: &str) {
        println!("{}", self.colored(text));
    }

    /// Trailing newline printed once a live stream finishes.
    pub fn finish_stream(&self) {
        println!();
    }
}
