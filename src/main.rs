mod cache;
mod chat;
mod cli;
mod client;
mod config;
mod editor;
mod handler;
mod render;
mod role;
mod shell_cmd;
mod tools;

use anyhow::{Context, Result};
use clap::Parser;

use chat::ChatSession;
use client::{LlmClient, OllamaClient, OllamaOptions, OpenAiCompatClient};
use config::Config;
use handler::{ChatHandler, CompletionParams, DefaultHandler, ReplHandler};
use role::SystemRole;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    cli.validate()?;

    let config = Config::load(cli.ollama).context("loading config")?;
    SystemRole::ensure_defaults(&config).context("creating default roles")?;

    if let Some(name) = &cli.create_role {
        return SystemRole::create_interactive(name);
    }
    if let Some(name) = &cli.show_role {
        println!("{}", SystemRole::get(name)?.role);
        return Ok(());
    }
    if cli.list_roles {
        for path in SystemRole::list()? {
            println!("{}", path.display());
        }
        return Ok(());
    }
    if let Some(chat_id) = &cli.show_chat {
        let color = config.get("DEFAULT_COLOR")?;
        handler::show_chat(chat_id, &color)?;
        return Ok(());
    }
    if cli.list_chats {
        for path in ChatSession::list()? {
            println!("{}", path.display());
        }
        return Ok(());
    }

    let resolved_prompt = cli::resolve_prompt(cli.prompt.as_deref())?;
    if cli.editor && resolved_prompt.stdin_was_piped {
        anyhow::bail!("--editor cannot be used with stdin input");
    }
    let prompt = if cli.editor {
        editor::edited_prompt()?
    } else {
        resolved_prompt.prompt
    };
    if prompt.is_empty() && cli.repl.is_none() {
        anyhow::bail!("no prompt provided (pass an argument or pipe input via stdin)");
    }

    let explicit_role = cli.role.is_some();
    let role = match &cli.role {
        Some(name) => SystemRole::get(name)?,
        None if cli.shell => SystemRole::get(role::SHELL_ROLE_NAME)?,
        None if cli.describe_shell => SystemRole::get(role::DESCRIBE_SHELL_ROLE_NAME)?,
        None if cli.code => SystemRole::get(role::CODE_ROLE_NAME)?,
        None => SystemRole::get(role::DEFAULT_ROLE_NAME)?,
    };

    let model = cli.model.unwrap_or(config.get("DEFAULT_MODEL")?);
    let temperature: f64 = match cli.temperature {
        Some(t) => t,
        None => config
            .get("DEFAULT_TEMPERATURE")?
            .parse()
            .context("parsing DEFAULT_TEMPERATURE from config")?,
    };
    let color = config.get("DEFAULT_COLOR")?;
    let timeout: u64 = config
        .get("REQUEST_TIMEOUT")?
        .parse()
        .context("parsing REQUEST_TIMEOUT from config")?;
    let stream = config.get("DISABLE_STREAMING")? != "true";
    let cache_length: usize = config
        .get("CHAT_CACHE_LENGTH")?
        .parse()
        .context("parsing CHAT_CACHE_LENGTH from config")?;
    let max_context_tokens: usize = config
        .get("MAX_CONTEXT_TOKENS")?
        .parse()
        .context("parsing MAX_CONTEXT_TOKENS from config")?;

    let use_ollama = cli.ollama || config.get("USE_OLLAMA")? == "true";
    let ollama_options = OllamaOptions {
        num_ctx: cli.num_ctx.or_else(|| {
            config
                .get_opt("OLLAMA_NUM_CTX")
                .and_then(|v| v.parse().ok())
        }),
        num_predict: cli.num_predict.or_else(|| {
            config
                .get_opt("OLLAMA_NUM_PREDICT")
                .and_then(|v| v.parse().ok())
        }),
        keep_alive: cli
            .keep_alive
            .clone()
            .or_else(|| config.get_opt("OLLAMA_KEEP_ALIVE")),
    };

    let params = CompletionParams {
        model,
        temperature,
        top_p: cli.top_p,
        stream,
        ollama_options,
        no_interaction: cli.no_interaction,
        cache_length: config
            .get("CACHE_LENGTH")?
            .parse()
            .context("parsing CACHE_LENGTH from config")?,
        // Shell/describe-shell modes run their own confirm-and-execute flow
        // over the raw text completion; offering the execute_shell_command
        // tool on top of that let the model run the command itself before
        // the text flow ran it again.
        enable_tools: !(cli.shell || cli.describe_shell),
    };

    let client: Box<dyn LlmClient> = if use_ollama {
        let ollama_base_url = config.get("OLLAMA_BASE_URL")?;
        Box::new(OllamaClient::new(&ollama_base_url, timeout))
    } else {
        let api_key = config.get("OPENAI_API_KEY")?;
        let base_url = config.get("API_BASE_URL")?;
        let base_url = if base_url == "default" {
            "https://api.openai.com/v1".to_string()
        } else {
            base_url
        };
        Box::new(OpenAiCompatClient::new(&base_url, &api_key, timeout))
    };

    if let Some(chat_id) = cli.repl.clone() {
        let mut handler = ReplHandler::new(
            client.as_ref(),
            color,
            chat_id,
            cli.shell,
            resolved_prompt.repl_input,
        )?;
        handler.handle(
            &prompt,
            &role,
            explicit_role,
            params.clone(),
            cache_length,
            max_context_tokens,
        )?;
    } else if let Some(chat_id) = cli.chat.clone() {
        let handler = ChatHandler::new(client.as_ref(), color.clone(), chat_id)?;
        let mut completion = handler.handle(
            &prompt,
            &role,
            explicit_role,
            params.clone(),
            cache_length,
            max_context_tokens,
        )?;
        if cli.shell && !cli.no_interaction {
            shell_confirmation_loop(client.as_ref(), &color, &mut completion, &params)?;
        }
    } else {
        let handler = DefaultHandler::new(client.as_ref(), color.clone());
        let mut completion = handler.handle(&prompt, &role, params.clone())?;
        if cli.shell && !cli.no_interaction {
            shell_confirmation_loop(client.as_ref(), &color, &mut completion, &params)?;
        }
    }

    Ok(())
}

fn shell_confirmation_loop(
    client: &dyn LlmClient,
    color: &str,
    completion: &mut String,
    params: &CompletionParams,
) -> Result<()> {
    use std::io::Write;
    loop {
        print!("[E]xecute, [M]odify, [D]escribe, [A]bort: ");
        std::io::stdout().flush().ok();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer)? == 0 {
            return Ok(());
        }
        match answer.trim().to_ascii_lowercase().as_str() {
            "e" | "y" | "execute" => return shell_cmd::run(completion),
            "m" | "modify" => {
                print!("Command: ");
                std::io::stdout().flush().ok();
                let mut edited = String::new();
                if std::io::stdin().read_line(&mut edited)? == 0 {
                    return Ok(());
                }
                *completion = edited.trim().to_string();
            }
            "d" | "describe" => {
                let descriptor = SystemRole::get(role::DESCRIBE_SHELL_ROLE_NAME)?;
                DefaultHandler::new(client, color.to_string()).handle(
                    completion,
                    &descriptor,
                    params.clone(),
                )?;
            }
            "a" | "abort" | "" => return Ok(()),
            _ => eprintln!("Please choose E, M, D, or A."),
        }
    }
}
