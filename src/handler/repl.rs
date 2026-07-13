use std::io::Write;

use anyhow::Result;

use super::{ChatHandler, CompletionParams, DefaultHandler};
use crate::client::LlmClient;
use crate::role::SystemRole;
use crate::shell_cmd;

/// Interactive read-eval-print loop layered on top of a ChatHandler: prompts
/// the user repeatedly, feeding each line (or `"""`-delimited block) through
/// as a chat turn until `exit()` or EOF/Ctrl+C.
pub struct ReplHandler<'a> {
    chat: ChatHandler<'a>,
    client: &'a dyn LlmClient,
    color: String,
    shell_mode: bool,
    piped_input: Option<std::vec::IntoIter<String>>,
}

impl<'a> ReplHandler<'a> {
    pub fn new(
        client: &'a dyn LlmClient,
        color: String,
        chat_id: String,
        shell_mode: bool,
        piped_input: Option<Vec<String>>,
    ) -> Result<Self> {
        Ok(Self {
            chat: ChatHandler::new(client, color.clone(), chat_id)?,
            client,
            color,
            shell_mode,
            piped_input: piped_input.map(Vec::into_iter),
        })
    }

    pub fn handle(
        &mut self,
        init_prompt: &str,
        role: &SystemRole,
        explicit_role: bool,
        params: CompletionParams,
        cache_length: usize,
        max_context_tokens: usize,
    ) -> Result<()> {
        if self.chat.exists()? {
            println!("─── Chat History ───");
            self.chat.show_history()?;
            println!("─────────────────────");
        }

        if self.shell_mode {
            println!(
                "Entering shell REPL mode; use e to execute or d to describe the last command. Press Ctrl+C to exit."
            );
        } else {
            println!("Entering REPL mode, press Ctrl+C to exit.");
        }

        let mut init_prompt = init_prompt.to_string();
        if !init_prompt.is_empty() {
            println!("─── Input ───");
            println!("{init_prompt}");
            println!("─────────────");
        }

        let mut last_completion = String::new();
        loop {
            print!(">>> ");
            std::io::stdout().flush().ok();

            let mut prompt = if let Some(input) = self.piped_input.as_mut() {
                match input.next() {
                    Some(line) => line,
                    None => break,
                }
            } else {
                let mut line = String::new();
                if std::io::stdin().read_line(&mut line)? == 0 {
                    break;
                }
                line.trim_end_matches('\n').to_string()
            };
            if prompt == "\"\"\"" {
                prompt = self.read_multiline()?;
            }
            if prompt == "exit()" {
                break;
            }
            if self.shell_mode && prompt == "e" {
                // Require a live confirmation on the controlling terminal before
                // executing. Reading from /dev/tty (not stdin) means a piped REPL
                // session cannot silently auto-execute a proposed command.
                if crate::tools::confirm_tty(&last_completion) {
                    shell_cmd::run(&last_completion)?;
                }
                continue;
            }
            if self.shell_mode && prompt == "d" {
                let descriptor = SystemRole::get(crate::role::DESCRIBE_SHELL_ROLE_NAME)?;
                DefaultHandler::new(self.client, self.color.clone()).handle(
                    &last_completion,
                    &descriptor,
                    params.clone(),
                )?;
                continue;
            }
            if !init_prompt.is_empty() {
                prompt = format!("{init_prompt}\n\n\n{prompt}");
                init_prompt.clear();
            }

            last_completion = self.chat.handle(
                &prompt,
                role,
                explicit_role,
                params.clone(),
                cache_length,
                max_context_tokens,
            )?;
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
