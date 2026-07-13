mod chat;
mod cli;
mod client;
mod config;
mod handler;
mod render;
mod role;

use anyhow::{Context, Result};
use clap::Parser;

use chat::ChatSession;
use client::OpenAiCompatClient;
use config::Config;
use handler::{ChatHandler, CompletionParams, DefaultHandler, ReplHandler};
use role::SystemRole;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    cli.validate()?;

    let config = Config::load().context("loading config")?;
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

    let prompt = cli::resolve_prompt(cli.prompt.as_deref())?;
    if prompt.is_empty() && cli.repl.is_none() {
        anyhow::bail!("no prompt provided (pass an argument or pipe input via stdin)");
    }

    let explicit_role = cli.role.is_some();
    let role = match &cli.role {
        Some(name) => SystemRole::get(name)?,
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
    let api_key = config.get("OPENAI_API_KEY")?;
    let timeout: u64 = config
        .get("REQUEST_TIMEOUT")?
        .parse()
        .context("parsing REQUEST_TIMEOUT from config")?;
    let base_url = config.get("API_BASE_URL")?;
    let base_url = if base_url == "default" {
        "https://api.openai.com/v1".to_string()
    } else {
        base_url
    };
    let stream = config.get("DISABLE_STREAMING")? != "true";
    let cache_length: usize = config
        .get("CHAT_CACHE_LENGTH")?
        .parse()
        .context("parsing CHAT_CACHE_LENGTH from config")?;

    let params = CompletionParams {
        model,
        temperature,
        top_p: cli.top_p,
        stream,
    };
    let client = OpenAiCompatClient::new(&base_url, &api_key, timeout);

    if let Some(chat_id) = cli.repl.clone() {
        let handler = ReplHandler::new(&client, color, chat_id)?;
        handler.handle(&prompt, &role, explicit_role, params, cache_length)?;
    } else if let Some(chat_id) = cli.chat.clone() {
        let handler = ChatHandler::new(&client, color, chat_id)?;
        handler.handle(&prompt, &role, explicit_role, params, cache_length)?;
    } else {
        let handler = DefaultHandler::new(&client, color);
        handler.handle(&prompt, &role, params)?;
    }

    Ok(())
}
