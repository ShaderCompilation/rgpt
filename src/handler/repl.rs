use std::io::Write;

use anyhow::Result;

use super::{ChatHandler, CompletionParams};
use crate::client::OpenAiCompatClient;
use crate::role::SystemRole;

/// Interactive read-eval-print loop layered on top of a ChatHandler: prompts
/// the user repeatedly, feeding each line (or `"""`-delimited block) through
/// as a chat turn until `exit()` or EOF/Ctrl+C.
pub struct ReplHandler<'a> {
    chat: ChatHandler<'a>,
}

impl<'a> ReplHandler<'a> {
    pub fn new(client: &'a OpenAiCompatClient, color: String, chat_id: String) -> Result<Self> {
        Ok(Self {
            chat: ChatHandler::new(client, color, chat_id)?,
        })
    }

    pub fn handle(
        &self,
        init_prompt: &str,
        role: &SystemRole,
        explicit_role: bool,
        params: CompletionParams,
        cache_length: usize,
    ) -> Result<()> {
        if self.chat.exists()? {
            println!("─── Chat History ───");
            self.chat.show_history()?;
            println!("─────────────────────");
        }

        println!("Entering REPL mode, press Ctrl+C to exit.");

        let mut init_prompt = init_prompt.to_string();
        if !init_prompt.is_empty() {
            println!("─── Input ───");
            println!("{init_prompt}");
            println!("─────────────");
        }

        loop {
            print!(">>> ");
            std::io::stdout().flush().ok();

            let mut line = String::new();
            if std::io::stdin().read_line(&mut line)? == 0 {
                break;
            }
            let mut prompt = line.trim_end_matches('\n').to_string();
            if prompt == "\"\"\"" {
                prompt = self.read_multiline()?;
            }
            if prompt == "exit()" {
                break;
            }
            if !init_prompt.is_empty() {
                prompt = format!("{init_prompt}\n\n\n{prompt}");
                init_prompt.clear();
            }

            self.chat
                .handle(&prompt, role, explicit_role, params.clone(), cache_length)?;
            println!();
        }
        Ok(())
    }

    fn read_multiline(&self) -> Result<String> {
        let mut buf = String::new();
        loop {
            print!("... ");
            std::io::stdout().flush().ok();
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line)? == 0 {
                break;
            }
            if line.trim_end_matches('\n') == "\"\"\"" {
                break;
            }
            buf.push_str(&line);
        }
        Ok(buf)
    }
}
