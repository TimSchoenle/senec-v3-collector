//! The Prometheus side of the collector: the registry, the gauges, and the energy accumulator.
//!
//! The accumulated kWh totals inside [`PrometheusMetricsExporter`] are the only state here that
//! outlives the process. Everything else is rebuilt at startup. `docs/METRICS.md` lists every
//! series this crate publishes.

mod telemetry;

pub use telemetry::PrometheusMetricsExporter;
