mod cli;
mod client;
mod config;
mod handler;
mod render;

use anyhow::{Context, Result};
use clap::Parser;

use client::OpenAiCompatClient;
use config::Config;
use handler::DefaultHandler;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    cli.validate()?;

    let prompt = cli::resolve_prompt(cli.prompt.as_deref())?;
    if prompt.is_empty() {
        anyhow::bail!("no prompt provided (pass an argument or pipe input via stdin)");
    }

    let config = Config::load().context("loading config")?;

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

    let client = OpenAiCompatClient::new(&base_url, &api_key, timeout);
    let handler = DefaultHandler::new(&client, color);
    handler.handle(&prompt, model, temperature, cli.top_p, stream)?;

    Ok(())
}
