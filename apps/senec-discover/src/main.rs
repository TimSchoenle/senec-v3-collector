use std::{fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use clap::Parser;
use senec_core::client::SenecClient;
use senec_discovery::discover;
use url::Url;

#[derive(Debug, Parser, Clone)]
#[command(author, version, about = "SENEC v3 discovery tool", long_about = None)]
struct Cli {
    #[arg(
        long,
        env = "SENEC_BASE_URL",
        default_value = "https://192.168.178.36",
        help = "SENEC base URL"
    )]
    base_url: Url,

    #[arg(
        long,
        env = "SENEC_POST_PATH",
        default_value = "/lala.cgi",
        help = "POST path for SENEC JSON API"
    )]
    post_path: String,

    #[arg(
        long,
        env = "SENEC_TIMEOUT_SECS",
        default_value_t = 10,
        help = "HTTP timeout for SENEC requests"
    )]
    timeout_secs: u64,

    #[arg(
        long,
        env = "SENEC_INSECURE_TLS",
        default_value_t = true,
        help = "Allow self-signed certificate for local SENEC"
    )]
    insecure_tls: bool,

    #[arg(
        long,
        env = "SENEC_CHUNK_SIZE",
        default_value_t = 20,
        help = "Max keys per POST chunk to lala.cgi"
    )]
    chunk_size: usize,

    #[arg(
        long,
        env = "SENEC_DISCOVERY_OUTPUT",
        default_value = "deploy/profiles/generated/senec-profile-live.json",
        help = "Output path for generated metric profile"
    )]
    output: PathBuf,
}

impl Cli {
    fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    let client = SenecClient::new(
        cli.base_url.clone(),
        &cli.post_path,
        cli.timeout(),
        cli.insecure_tls,
        cli.chunk_size,
    )?;

    let profile = discover(&client).await?;
    let json = serde_json::to_string(&profile).context("failed to serialize metric profile")?;

    if let Some(parent) = cli.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create profile output directory {}",
                parent.display()
            )
        })?;
    }

    fs::write(&cli.output, json).with_context(|| {
        format!(
            "failed to write generated profile to {}",
            cli.output.display()
        )
    })?;

    let key_count: usize = profile.objects.values().map(std::vec::Vec::len).sum();
    println!("Profile written to {}", cli.output.display());
    println!("Objects: {}, keys: {}", profile.objects.len(), key_count);

    Ok(())
}
