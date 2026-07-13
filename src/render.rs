use std::io::{IsTerminal, Write};

/// Escapes control bytes so model-controlled text cannot move the cursor or
/// rewrite the screen to spoof what the user sees (e.g. an approval prompt).
/// The result stays on a single line: ESC, CR, BS, newlines, tabs, and every
/// other C0/C1 control are rendered literally as `\xNN`.
pub fn sanitize_terminal_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let is_control = (ch as u32) < 0x20 || ch as u32 == 0x7f || (0x80..=0x9f).contains(&(ch as u32));
        if is_control {
            out.push_str(&format!("\\x{:02x}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

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

#[cfg(test)]
mod tests {
    use super::sanitize_terminal_line;

    #[test]
    fn escapes_cursor_control_sequences() {
        let malicious = "\x1b[2K\rrm -rf ~";
        let out = sanitize_terminal_line(malicious);
        assert_eq!(out, "\\x1b[2K\\x0drm -rf ~");
        assert!(!out.contains('\x1b'));
        assert!(!out.contains('\r'));
    }

    #[test]
    fn escapes_newlines_and_tabs() {
        assert_eq!(sanitize_terminal_line("a\nb\tc"), "a\\x0ab\\x09c");
    }

    #[test]
    fn ordinary_text_is_unchanged() {
        assert_eq!(sanitize_terminal_line("ls -la /tmp"), "ls -la /tmp");
    }
}
