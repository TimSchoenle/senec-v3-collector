use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use prometheus::{Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
struct GridEconomicsState {
    last_timestamp_seconds: Option<f64>,
    grid_import_energy_kwh_total: f64,
    grid_export_energy_kwh_total: f64,
    house_consumption_energy_kwh_total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GridEconomicsSnapshot {
    grid_import_energy_kwh_total: f64,
    grid_export_energy_kwh_total: f64,
    house_consumption_energy_kwh_total: f64,
}

impl GridEconomicsState {
    fn from_snapshot(snapshot: GridEconomicsSnapshot) -> Self {
        Self {
            last_timestamp_seconds: None,
            grid_import_energy_kwh_total: snapshot.grid_import_energy_kwh_total,
            grid_export_energy_kwh_total: snapshot.grid_export_energy_kwh_total,
            house_consumption_energy_kwh_total: snapshot.house_consumption_energy_kwh_total,
        }
    }

    fn snapshot(&self) -> GridEconomicsSnapshot {
        GridEconomicsSnapshot {
            grid_import_energy_kwh_total: self.grid_import_energy_kwh_total,
            grid_export_energy_kwh_total: self.grid_export_energy_kwh_total,
            house_consumption_energy_kwh_total: self.house_consumption_energy_kwh_total,
        }
    }
}

#[derive(Clone)]
pub struct PrometheusMetricsExporter {
    registry: Registry,
    values: GaugeVec,
    scrape_up: Gauge,
    poll_ok: Gauge,
    poll_timestamp_seconds: Gauge,
    grid_import_price_eur_per_kwh: f64,
    grid_export_price_eur_per_kwh: f64,
    grid_import_price_gauge: Gauge,
    grid_export_price_gauge: Gauge,
    grid_import_power_w: Gauge,
    grid_export_power_w: Gauge,
    grid_import_energy_kwh_total: Gauge,
    grid_export_energy_kwh_total: Gauge,
    house_consumption_energy_kwh_total: Gauge,
    self_supplied_energy_kwh_total: Gauge,
    self_sufficiency_percent: Gauge,
    grid_import_cost_eur_total: Gauge,
    grid_export_revenue_eur_total: Gauge,
    grid_net_balance_eur_total: Gauge,
    economics_state: Arc<Mutex<GridEconomicsState>>,
    economics_state_path: Option<PathBuf>,
    site_id: String,
}

impl PrometheusMetricsExporter {
    pub fn new(
        site_id: &str,
        grid_import_price_eur_per_kwh: f64,
        grid_export_price_eur_per_kwh: f64,
        economics_state_path: Option<PathBuf>,
    ) -> Result<Self> {
        ensure!(
            grid_import_price_eur_per_kwh >= 0.0,
            "grid import price must be >= 0"
        );
        ensure!(
            grid_export_price_eur_per_kwh >= 0.0,
            "grid export price must be >= 0"
        );

        let initial_economics_state = match economics_state_path.as_deref() {
            Some(path) => read_grid_economics_snapshot(path)?
                .map(GridEconomicsState::from_snapshot)
                .unwrap_or_default(),
            None => GridEconomicsState::default(),
        };

        let registry = Registry::new_custom(Some("senec".to_string()), None)
            .context("failed to create prometheus registry")?;

        let values = GaugeVec::new(
            Opts::new("value", "Decoded SENEC value"),
            &["site_id", "object", "key", "index"],
        )
        .context("failed to create senec value gauge")?;

        let scrape_up = Gauge::with_opts(Opts::new(
            "scrape_up",
            "Whether the metrics endpoint is operational (1=yes)",
        ))
        .context("failed to create scrape_up gauge")?;

        let poll_ok = Gauge::with_opts(Opts::new(
            "poll_ok",
            "Whether the last poll cycle succeeded (1=yes,0=no)",
        ))
        .context("failed to create poll_ok gauge")?;

        let poll_timestamp_seconds = Gauge::with_opts(Opts::new(
            "poll_timestamp_seconds",
            "Unix timestamp of the last completed poll cycle",
        ))
        .context("failed to create poll timestamp gauge")?;

        let grid_import_price_gauge = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_import_price_eur_per_kwh",
                "Configured grid import price in EUR/kWh",
            ),
            site_id,
        ))
        .context("failed to create grid import price gauge")?;

        let grid_export_price_gauge = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_export_price_eur_per_kwh",
                "Configured grid feed-in tariff in EUR/kWh",
            ),
            site_id,
        ))
        .context("failed to create grid export price gauge")?;

        let grid_import_power_w = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_import_power_w",
                "Current power imported from the grid in watts",
            ),
            site_id,
        ))
        .context("failed to create grid import power gauge")?;

        let grid_export_power_w = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_export_power_w",
                "Current power exported to the grid in watts",
            ),
            site_id,
        ))
        .context("failed to create grid export power gauge")?;

        let grid_import_energy_kwh_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_import_energy_kwh_total",
                "Estimated cumulative imported grid energy in kWh",
            ),
            site_id,
        ))
        .context("failed to create imported grid energy gauge")?;

        let grid_export_energy_kwh_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_export_energy_kwh_total",
                "Estimated cumulative exported grid energy in kWh",
            ),
            site_id,
        ))
        .context("failed to create exported grid energy gauge")?;

        let house_consumption_energy_kwh_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "house_consumption_energy_kwh_total",
                "Estimated cumulative house consumption energy in kWh",
            ),
            site_id,
        ))
        .context("failed to create house energy gauge")?;

        let self_supplied_energy_kwh_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "self_supplied_energy_kwh_total",
                "Estimated cumulative self-supplied house energy in kWh",
            ),
            site_id,
        ))
        .context("failed to create self supplied energy gauge")?;

        let self_sufficiency_percent = Gauge::with_opts(with_site_label(
            Opts::new(
                "self_sufficiency_percent",
                "Estimated self-sufficiency ratio based on cumulative energy (% of house demand not imported from grid)",
            ),
            site_id,
        ))
        .context("failed to create self-sufficiency gauge")?;

        let grid_import_cost_eur_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_import_cost_eur_total",
                "Estimated cumulative cost for imported grid energy in EUR",
            ),
            site_id,
        ))
        .context("failed to create grid import cost gauge")?;

        let grid_export_revenue_eur_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_export_revenue_eur_total",
                "Estimated cumulative revenue from exported grid energy in EUR",
            ),
            site_id,
        ))
        .context("failed to create grid export revenue gauge")?;

        let grid_net_balance_eur_total = Gauge::with_opts(with_site_label(
            Opts::new(
                "grid_net_balance_eur_total",
                "Estimated cumulative net grid balance in EUR (revenue - cost)",
            ),
            site_id,
        ))
        .context("failed to create grid net balance gauge")?;

        registry
            .register(Box::new(values.clone()))
            .context("failed to register value gauge")?;
        registry
            .register(Box::new(scrape_up.clone()))
            .context("failed to register scrape_up gauge")?;
        registry
            .register(Box::new(poll_ok.clone()))
            .context("failed to register poll_ok gauge")?;
        registry
            .register(Box::new(poll_timestamp_seconds.clone()))
            .context("failed to register poll timestamp gauge")?;
        registry
            .register(Box::new(grid_import_price_gauge.clone()))
            .context("failed to register grid import price gauge")?;
        registry
            .register(Box::new(grid_export_price_gauge.clone()))
            .context("failed to register grid export price gauge")?;
        registry
            .register(Box::new(grid_import_power_w.clone()))
            .context("failed to register grid import power gauge")?;
        registry
            .register(Box::new(grid_export_power_w.clone()))
            .context("failed to register grid export power gauge")?;
        registry
            .register(Box::new(grid_import_energy_kwh_total.clone()))
            .context("failed to register grid import energy gauge")?;
        registry
            .register(Box::new(grid_export_energy_kwh_total.clone()))
            .context("failed to register grid export energy gauge")?;
        registry
            .register(Box::new(house_consumption_energy_kwh_total.clone()))
            .context("failed to register house consumption energy gauge")?;
        registry
            .register(Box::new(self_supplied_energy_kwh_total.clone()))
            .context("failed to register self supplied energy gauge")?;
        registry
            .register(Box::new(self_sufficiency_percent.clone()))
            .context("failed to register self-sufficiency gauge")?;
        registry
            .register(Box::new(grid_import_cost_eur_total.clone()))
            .context("failed to register grid import cost gauge")?;
        registry
            .register(Box::new(grid_export_revenue_eur_total.clone()))
            .context("failed to register grid export revenue gauge")?;
        registry
            .register(Box::new(grid_net_balance_eur_total.clone()))
            .context("failed to register grid net balance gauge")?;

        scrape_up.set(1.0);
        poll_ok.set(0.0);
        grid_import_price_gauge.set(grid_import_price_eur_per_kwh);
        grid_export_price_gauge.set(grid_export_price_eur_per_kwh);

        let exporter = Self {
            registry,
            values,
            scrape_up,
            poll_ok,
            poll_timestamp_seconds,
            grid_import_price_eur_per_kwh,
            grid_export_price_eur_per_kwh,
            grid_import_price_gauge,
            grid_export_price_gauge,
            grid_import_power_w,
            grid_export_power_w,
            grid_import_energy_kwh_total,
            grid_export_energy_kwh_total,
            house_consumption_energy_kwh_total,
            self_supplied_energy_kwh_total,
            self_sufficiency_percent,
            grid_import_cost_eur_total,
            grid_export_revenue_eur_total,
            grid_net_balance_eur_total,
            economics_state: Arc::new(Mutex::new(initial_economics_state)),
            economics_state_path,
            site_id: site_id.to_string(),
        };

        exporter.publish_economics_from_state();

        Ok(exporter)
    }

    pub fn record_metric(&self, object: &str, key: &str, index: usize, value: f64) {
        let index_label = index.to_string();
        self.values
            .with_label_values(&[self.site_id.as_str(), object, key, index_label.as_str()])
            .set(value);
    }

    pub fn mark_poll_result(&self, ok: bool) {
        self.poll_ok.set(if ok { 1.0 } else { 0.0 });
        self.poll_timestamp_seconds.set(now_epoch_seconds());
    }

    pub fn update_grid_economics(&self, grid_power_w: Option<f64>, house_power_w: Option<f64>) {
        let now = now_epoch_seconds();

        let grid_import_power_w = grid_power_w.unwrap_or(0.0).max(0.0);
        let grid_export_power_w = (-grid_power_w.unwrap_or(0.0)).max(0.0);
        let house_consumption_power_w = house_power_w.unwrap_or(0.0).max(0.0);

        self.grid_import_power_w.set(grid_import_power_w);
        self.grid_export_power_w.set(grid_export_power_w);
        self.grid_import_price_gauge
            .set(self.grid_import_price_eur_per_kwh);
        self.grid_export_price_gauge
            .set(self.grid_export_price_eur_per_kwh);

        let snapshot = {
            let mut state = self
                .economics_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            if let Some(last_timestamp_seconds) = state.last_timestamp_seconds {
                let delta_seconds = (now - last_timestamp_seconds).max(0.0);
                if delta_seconds > 0.0 {
                    let delta_hours = delta_seconds / 3600.0;
                    state.grid_import_energy_kwh_total +=
                        (grid_import_power_w * delta_hours) / 1000.0;
                    state.grid_export_energy_kwh_total +=
                        (grid_export_power_w * delta_hours) / 1000.0;
                    state.house_consumption_energy_kwh_total +=
                        (house_consumption_power_w * delta_hours) / 1000.0;
                }
            }

            state.last_timestamp_seconds = Some(now);
            self.set_derived_economics_metrics(&state);
            state.snapshot()
        };

        if let Some(path) = self.economics_state_path.as_deref()
            && let Err(err) = write_grid_economics_snapshot(path, &snapshot)
        {
            eprintln!(
                "failed to persist grid economics state to {}: {err:#}",
                path.display()
            );
        }
    }

    pub fn render(&self) -> Result<String> {
        self.scrape_up.set(1.0);
        let metric_families = self.registry.gather();
        let mut output = Vec::new();
        TextEncoder::new()
            .encode(&metric_families, &mut output)
            .context("failed to encode prometheus metrics")?;
        String::from_utf8(output).context("prometheus metrics are not valid utf-8")
    }

    fn publish_economics_from_state(&self) {
        let state = self
            .economics_state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        self.set_derived_economics_metrics(&state);
    }

    fn set_derived_economics_metrics(&self, state: &GridEconomicsState) {
        let self_supplied_energy_kwh_total = (state.house_consumption_energy_kwh_total
            - state.grid_import_energy_kwh_total)
            .max(0.0);
        let self_sufficiency_percent = if state.house_consumption_energy_kwh_total > 0.0 {
            (self_supplied_energy_kwh_total / state.house_consumption_energy_kwh_total) * 100.0
        } else {
            0.0
        };

        let grid_import_cost_eur_total =
            state.grid_import_energy_kwh_total * self.grid_import_price_eur_per_kwh;
        let grid_export_revenue_eur_total =
            state.grid_export_energy_kwh_total * self.grid_export_price_eur_per_kwh;
        let grid_net_balance_eur_total = grid_export_revenue_eur_total - grid_import_cost_eur_total;

        self.grid_import_energy_kwh_total
            .set(state.grid_import_energy_kwh_total);
        self.grid_export_energy_kwh_total
            .set(state.grid_export_energy_kwh_total);
        self.house_consumption_energy_kwh_total
            .set(state.house_consumption_energy_kwh_total);
        self.self_supplied_energy_kwh_total
            .set(self_supplied_energy_kwh_total);
        self.self_sufficiency_percent.set(self_sufficiency_percent);
        self.grid_import_cost_eur_total
            .set(grid_import_cost_eur_total);
        self.grid_export_revenue_eur_total
            .set(grid_export_revenue_eur_total);
        self.grid_net_balance_eur_total
            .set(grid_net_balance_eur_total);
    }
}

fn now_epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn with_site_label(opts: Opts, site_id: &str) -> Opts {
    opts.const_label("site_id", site_id.to_string())
}

fn read_grid_economics_snapshot(path: &Path) -> Result<Option<GridEconomicsSnapshot>> {
    match fs::read_to_string(path) {
        Ok(payload) => {
            let snapshot =
                serde_json::from_str::<GridEconomicsSnapshot>(&payload).with_context(|| {
                    format!("failed to parse grid economics state at {}", path.display())
                })?;
            Ok(Some(snapshot))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err)
            .with_context(|| format!("failed to read grid economics state at {}", path.display())),
    }
}

fn write_grid_economics_snapshot(path: &Path, snapshot: &GridEconomicsSnapshot) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create grid economics state directory {}",
                parent.display()
            )
        })?;
    }

    let payload =
        serde_json::to_string(snapshot).context("failed to serialize grid economics state")?;
    let temp_path = path.with_extension("tmp");

    fs::write(&temp_path, payload).with_context(|| {
        format!(
            "failed to write temporary state file {}",
            temp_path.display()
        )
    })?;

    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("failed to replace state file {}", path.display()))?;
    }

    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "failed to rename temporary state file {} to {}",
            temp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        GridEconomicsSnapshot, PrometheusMetricsExporter, now_epoch_seconds,
        read_grid_economics_snapshot, write_grid_economics_snapshot,
    };

    #[test]
    fn renders_grid_economics_metrics_before_first_poll() {
        let exporter = PrometheusMetricsExporter::new("local", 0.35, 0.08, None)
            .expect("should initialize exporter");

        let payload = exporter.render().expect("should render metrics");

        assert!(payload.contains("senec_grid_import_energy_kwh_total"));
        assert!(payload.contains("senec_grid_export_energy_kwh_total"));
        assert!(payload.contains("senec_house_consumption_energy_kwh_total"));
        assert!(payload.contains("senec_self_supplied_energy_kwh_total"));
        assert!(payload.contains("senec_self_sufficiency_percent"));
        assert!(payload.contains("senec_grid_import_cost_eur_total"));
        assert!(payload.contains("senec_grid_export_revenue_eur_total"));
        assert!(payload.contains("senec_grid_net_balance_eur_total"));
    }

    #[test]
    fn renders_prometheus_payload() {
        let exporter = PrometheusMetricsExporter::new("local", 0.35, 0.08, None)
            .expect("should initialize exporter");
        exporter.record_metric("ENERGY", "GUI_BAT_DATA_POWER", 0, 123.45);
        exporter.update_grid_economics(Some(500.0), Some(3200.0));
        exporter.mark_poll_result(true);

        let payload = exporter.render().expect("should render metrics");

        assert!(payload.contains("senec_value"));
        assert!(payload.contains("object=\"ENERGY\""));
        assert!(payload.contains("key=\"GUI_BAT_DATA_POWER\""));
        assert!(payload.contains("site_id=\"local\""));
        assert!(payload.contains("senec_poll_ok"));
        assert!(payload.contains("senec_grid_import_price_eur_per_kwh"));
        assert!(payload.contains("senec_grid_import_cost_eur_total"));
    }

    #[test]
    fn restores_persisted_grid_economics_state() {
        let state_path = temp_state_path("restore");
        let snapshot = GridEconomicsSnapshot {
            grid_import_energy_kwh_total: 12.5,
            grid_export_energy_kwh_total: 3.0,
            house_consumption_energy_kwh_total: 20.0,
        };
        write_grid_economics_snapshot(&state_path, &snapshot).expect("snapshot should be written");

        let exporter =
            PrometheusMetricsExporter::new("local", 0.35, 0.08, Some(state_path.clone()))
                .expect("should initialize exporter with persisted state");
        let payload = exporter.render().expect("should render metrics");

        assert!(payload.contains("senec_grid_import_energy_kwh_total{site_id=\"local\"} 12.5"));
        assert!(payload.contains("senec_grid_export_energy_kwh_total{site_id=\"local\"} 3"));
        assert!(payload.contains("senec_house_consumption_energy_kwh_total{site_id=\"local\"} 20"));

        cleanup_state_file(&state_path);
    }

    #[test]
    fn persists_grid_economics_state_on_update() {
        let state_path = temp_state_path("persist");

        let exporter =
            PrometheusMetricsExporter::new("local", 0.35, 0.08, Some(state_path.clone()))
                .expect("should initialize exporter with state path");

        {
            let mut state = exporter
                .economics_state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.last_timestamp_seconds = Some(now_epoch_seconds() - 3600.0);
        }

        exporter.update_grid_economics(Some(1000.0), Some(1500.0));

        let snapshot = read_grid_economics_snapshot(&state_path)
            .expect("snapshot should be readable")
            .expect("snapshot should exist");

        assert!(snapshot.grid_import_energy_kwh_total > 0.95);
        assert!(snapshot.grid_import_energy_kwh_total < 1.05);
        assert!(snapshot.house_consumption_energy_kwh_total > 1.45);
        assert!(snapshot.house_consumption_energy_kwh_total < 1.55);

        cleanup_state_file(&state_path);
    }

    fn temp_state_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        path.push(format!("senec-grid-economics-{name}-{nanos}.json"));
        path
    }

    fn cleanup_state_file(path: &PathBuf) {
        let _ = std::fs::remove_file(path);
    }
}
