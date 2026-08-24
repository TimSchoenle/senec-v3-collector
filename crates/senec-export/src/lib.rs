//! Publishing decoded readings as Prometheus series, and the totals integrated from them.
//!
//! The accumulated kWh totals inside [`PrometheusMetricsExporter`] are the only state here that
//! outlives the process. Everything else is rebuilt at startup. `docs/METRICS.md` lists every
//! series this crate publishes.

mod telemetry;

pub use telemetry::PrometheusMetricsExporter;
