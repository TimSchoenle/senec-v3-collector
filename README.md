# SENEC v3 Collector

SENEC v3 Collector is a Rust workspace that discovers available SENEC keys, polls a local SENEC v3 system, and exposes pull-based Prometheus metrics.

## Repository Structure

- `apps/senec-collect` (`senec-v3-collector`): Polling service and `/metrics` HTTP endpoint.
- `apps/senec-discover` (`senec-v3-discover`): Profile discovery tool.
- `crates/senec-core`: SENEC client, decoding, shared models, profile loading.
- `crates/senec-discovery`: Discovery pipeline used by the discover app.
- `crates/senec-export`: Prometheus exporter and derived economics metrics.

## Requirements

- Rust toolchain with Edition 2024 support.
- Access to your SENEC device on the local network.
- Docker and Docker Compose (optional, for container deployment).

## Quick Start

### 1. Build and test

```powershell
cargo check --workspace
cargo test --workspace
```

### 2. Discover a live profile

```powershell
cargo run -p senec-v3-discover -- --output deploy/profiles/generated/senec-profile-live.json
```

### 3. Run the collector

Single cycle:

```powershell
cargo run -p senec-v3-collector -- --once --metrics-bind 127.0.0.1:9464
```

Continuous mode:

```powershell
cargo run -p senec-v3-collector -- --metrics-bind 0.0.0.0:9464 --metrics-path /metrics
```

Metrics endpoint: `http://localhost:9464/metrics`

## Configuration

The apps accept CLI flags and environment variables (`.env` is loaded automatically when present).

Create a local config file:

```powershell
Copy-Item .env.example .env
```

Common variables:

| Variable | Default | Purpose |
|---|---|---|
| `SENEC_BASE_URL` | `https://192.168.178.36` | SENEC base URL |
| `SENEC_POST_PATH` | `/lala.cgi` | SENEC JSON POST path |
| `SENEC_TIMEOUT_SECS` | `10` | HTTP timeout |
| `SENEC_INSECURE_TLS` | `true` | Allow self-signed certificates |
| `SENEC_CHUNK_SIZE` | `20` | Max keys per request chunk |
| `SENEC_DISCOVERY_OUTPUT` | `deploy/profiles/generated/senec-profile-live.json` | Output path for discovery |
| `SENEC_PROFILE_PATH` | `deploy/profiles/generated/senec-profile-live.json` | Profile used by collector |
| `SENEC_POLL_INTERVAL_SECS` | `10` | Collector poll interval |
| `SENEC_METRICS_BIND` | `0.0.0.0:9464` | Collector bind address |
| `SENEC_METRICS_PATH` | `/metrics` | Collector metrics route |
| `SENEC_SITE_ID` | `local` | `site_id` label in metrics |
| `SENEC_GRID_IMPORT_PRICE_EUR_PER_KWH` | `0.0` | Import tariff for cost metrics |
| `SENEC_GRID_EXPORT_PRICE_EUR_PER_KWH` | `0.0` | Export tariff for revenue metrics |
| `SENEC_ECONOMICS_STATE_PATH` | `state/grid-economics-state.json` | Persistent state for cumulative economics |
| `RUST_LOG` | `info` | Log level |

Note: `.env.example` sets `SENEC_ECONOMICS_STATE_PATH=/app/state/grid-economics-state.json` for containers. For local runs, use `state/grid-economics-state.json`.

## Generated Files

- `deploy/profiles/generated/senec-profile-live.json`: Generated profile from discovery.
- `state/grid-economics-state.json`: Persistent cumulative economics state (path configurable).

## Docker

Build collector image:

```powershell
docker build -t senec-v3-collector:dev .
```

Run collector container:

```powershell
docker run --rm `
  -p 9464:9464 `
  -v ${PWD}/deploy/profiles/generated:/app/profiles/generated `
  -v ${PWD}/state:/app/state `
  -e SENEC_BASE_URL=https://192.168.178.36 `
  -e SENEC_PROFILE_PATH=/app/profiles/generated/senec-profile-live.json `
  -e SENEC_ECONOMICS_STATE_PATH=/app/state/grid-economics-state.json `
  senec-v3-collector:dev
```

## Docker Compose Examples

Compose files are in `deploy/compose`:

- `deploy/compose/stack.yml`: Full monitoring stack using named volumes.
- `deploy/compose/stack.local-bind.yml`: Full monitoring stack with only relative local bind mounts (`deploy/compose/data/...`).
- `deploy/compose/collector.local-bind.yml`: Collector-only setup with relative local bind mounts.

Start a stack example:

```powershell
Copy-Item .env.example .env
docker compose -f deploy/compose/stack.yml up -d --build
```

Fully local disk-backed stack (relative bind mounts):

```powershell
Copy-Item .env.example .env
docker compose -f deploy/compose/stack.local-bind.yml up -d --build
```

Default URLs:

- Grafana: `http://localhost:3000`
- Prometheus: internal-only by default (`http://prometheus:9090` on the `senec` Docker network)
- VictoriaMetrics: internal-only by default (`http://victoriametrics:8428` on the `senec` Docker network)

Default Grafana credentials:

- User: `admin`
- Password: `admin`

## Development Checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

## License

Distributed under the GPL-3.0 License. See `LICENSE` for more information.
