use std::{collections::BTreeMap, net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use senec_core::{
    client::SenecClient, decode::decode_numeric_values, model::MetricProfile, profile::load_profile,
};
use senec_export::PrometheusMetricsExporter;
use tokio::{task::JoinHandle, time::MissedTickBehavior};
use url::Url;

#[derive(Debug, Parser, Clone)]
#[command(author, version, about = "SENEC v3 collector", long_about = None)]
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
        env = "SENEC_POLL_INTERVAL_SECS",
        default_value_t = 10,
        help = "Polling interval"
    )]
    poll_interval_secs: u64,

    #[arg(
        long,
        env = "SENEC_METRICS_BIND",
        default_value = "0.0.0.0:9464",
        help = "Address to bind pull metrics HTTP server"
    )]
    metrics_bind: SocketAddr,

    #[arg(
        long,
        env = "SENEC_METRICS_PATH",
        default_value = "/metrics",
        help = "HTTP path served for pull metrics"
    )]
    metrics_path: String,

    #[arg(
        long,
        env = "SENEC_PROFILE_PATH",
        default_value = "profiles/generated/senec-profile-live.json",
        help = "JSON file path to metric profile"
    )]
    profile: PathBuf,

    #[arg(
        long,
        env = "SENEC_SITE_ID",
        default_value = "local",
        help = "Site label for metric attributes"
    )]
    site_id: String,

    #[arg(
        long,
        env = "SENEC_GRID_IMPORT_PRICE_EUR_PER_KWH",
        default_value_t = 0.0,
        help = "Grid import price in EUR per kWh (used for cost metrics)"
    )]
    grid_import_price_eur_per_kwh: f64,

    #[arg(
        long,
        env = "SENEC_GRID_EXPORT_PRICE_EUR_PER_KWH",
        default_value_t = 0.0,
        help = "Grid feed-in tariff in EUR per kWh (used for revenue metrics)"
    )]
    grid_export_price_eur_per_kwh: f64,

    #[arg(
        long,
        env = "SENEC_ECONOMICS_STATE_PATH",
        default_value = "state/grid-economics-state.json",
        help = "Path to persisted state for cumulative grid economics metrics"
    )]
    economics_state_path: PathBuf,

    #[arg(
        long,
        default_value_t = false,
        help = "Run a single poll/export cycle and exit"
    )]
    once: bool,
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

    tracing::info!(path = %cli.profile.display(), "loading metric profile");
    let profile = normalize_profile(load_profile(&cli.profile)?);
    if profile.objects.is_empty() {
        anyhow::bail!("selected profile has no objects/keys to poll");
    }

    let metrics_path = normalize_metrics_path(&cli.metrics_path);
    let exporter = PrometheusMetricsExporter::new(
        &cli.site_id,
        cli.grid_import_price_eur_per_kwh,
        cli.grid_export_price_eur_per_kwh,
        Some(cli.economics_state_path.clone()),
    )?;
    let metrics_server =
        start_metrics_server(exporter.clone(), cli.metrics_bind, &metrics_path).await?;

    tracing::info!(
        endpoint = %client.post_endpoint(),
        poll_interval_secs = cli.poll_interval_secs,
        objects = profile.objects.len(),
        metrics_bind = %cli.metrics_bind,
        metrics_path = %metrics_path,
        grid_import_price_eur_per_kwh = cli.grid_import_price_eur_per_kwh,
        grid_export_price_eur_per_kwh = cli.grid_export_price_eur_per_kwh,
        economics_state_path = %cli.economics_state_path.display(),
        "collector started"
    );

    if cli.once {
        let result = poll_once(&client, &profile, &exporter).await;
        if result.is_err() {
            exporter.mark_poll_result(false);
        }
        metrics_server.abort();
        return result;
    }

    let mut ticker = tokio::time::interval(Duration::from_secs(cli.poll_interval_secs.max(1)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("ctrl-c received, shutting down collector");
                break;
            }
            _ = ticker.tick() => {
                if let Err(err) = poll_once(&client, &profile, &exporter).await {
                    exporter.mark_poll_result(false);
                    tracing::error!(%err, "poll cycle failed");
                }
            }
        }
    }

    metrics_server.abort();
    Ok(())
}

async fn start_metrics_server(
    exporter: PrometheusMetricsExporter,
    bind: SocketAddr,
    path: &str,
) -> Result<JoinHandle<()>> {
    let router = Router::new()
        .route(path, get(metrics_handler))
        .with_state(exporter);

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind metrics server to {bind}"))?;

    Ok(tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!(%err, "metrics server failed");
        }
    }))
}

async fn metrics_handler(State(exporter): State<PrometheusMetricsExporter>) -> impl IntoResponse {
    match exporter.render() {
        Ok(payload) => {
            let headers = [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
            )];
            (headers, payload).into_response()
        }
        Err(err) => {
            tracing::error!(%err, "failed to render metrics payload");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to render metrics",
            )
                .into_response()
        }
    }
}

async fn poll_once(
    client: &SenecClient,
    profile: &MetricProfile,
    exporter: &PrometheusMetricsExporter,
) -> Result<()> {
    let response = client.query_strings(&profile.objects).await?;

    let mut exported = 0usize;
    let mut grid_power_w: Option<f64> = None;
    let mut house_power_w: Option<f64> = None;

    for (object, values) in response {
        for (key, raw) in values {
            let parsed = decode_numeric_values(&raw);
            for (index, value) in parsed.into_iter().enumerate() {
                if object == "ENERGY" && index == 0 {
                    match key.as_str() {
                        "GUI_GRID_POW" => grid_power_w = Some(value),
                        "GUI_HOUSE_POW" => house_power_w = Some(value),
                        _ => {}
                    }
                }
                exporter.record_metric(&object, &key, index, value);
                exported += 1;
            }
        }
    }

    exporter.update_grid_economics(grid_power_w, house_power_w);
    exporter.mark_poll_result(true);
    tracing::debug!(metrics = exported, "poll cycle completed");
    Ok(())
}

fn normalize_metrics_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn normalize_profile(mut profile: MetricProfile) -> MetricProfile {
    let objects = profile
        .objects
        .into_iter()
        .filter_map(|(object, keys)| {
            let mut unique = BTreeMap::<String, ()>::new();
            for key in keys {
                if !key.trim().is_empty() {
                    unique.insert(key, ());
                }
            }

            if unique.is_empty() {
                None
            } else {
                Some((object, unique.into_keys().collect::<Vec<_>>()))
            }
        })
        .collect();

    profile.objects = objects;
    profile
}
