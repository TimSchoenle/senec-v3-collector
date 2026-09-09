<!--
Generated from .github/templates/README.md.hbs. Edit that file, not this one.

CI renders it on every pull request and commits the result back to the branch. A push to main
whose README.md does not match its template fails the `readme` job in
.github/workflows/docs.yml.

The payload comes from one place, TimSchoenle/actions/actions/common/readme-variables: the
repository coordinates, the release read off Cargo.toml, and the table of documents walked out of
docs/. This repository has no configuration-schema generator, so no `extra` is merged in, and the
flag tables below are written by hand against the `clap` structs in apps/*/src/main.rs. The drift
gate cannot check those two against each other.

Nothing in this comment may contain a mustache that is not a real reference.
-->

# senec-v3-collector

SENEC v3 telemetry collector that discovers device keys and exposes pull-based Prometheus metrics.

[![Release](https://img.shields.io/github/v/release/TimSchoenle/senec-v3-collector?sort=semver)](https://github.com/TimSchoenle/senec-v3-collector/releases)
[![Build](https://img.shields.io/github/actions/workflow/status/TimSchoenle/senec-v3-collector/build.yaml?branch=main)](https://github.com/TimSchoenle/senec-v3-collector/actions/workflows/build.yaml)

## What this is

A SENEC v3 home battery answers a JSON POST at `/lala.cgi` with hex-typed strings, one per key it
was asked for. This workspace turns that into a Prometheus scrape target.

`senec-v3-discover` fetches `/js/senec.min.js` from the device, pulls every object and key name the
web UI mentions out of it, asks the device for every one of them, and keeps the ones that answer
with a value rather than `VARIABLE_NOT_FOUND`, `FORBIDDEN`, `OBJECT_NOT_FOUND` or
`MALFORMED_VALUE`. What it writes is a profile file. `senec-v3-collector` polls the keys in that
file on an interval and serves them at `/metrics`.

Discovery is a separate binary because the answer is per device. Which keys exist depends on the
firmware and on what hardware is installed, so a key list compiled into the collector would be
right for one system and wrong for the next. The profile committed at
`deploy/profiles/generated/senec-profile-live.json` covers 12 objects and 159 keys, and is the
author's system rather than a specification.

Three library crates sit under `crates/`. `senec-core` holds the HTTP client, the token decoder and
the profile types; `senec-discovery` holds the candidate extraction; `senec-export` holds the
Prometheus registry and the economics accumulator.

## Quick start

```bash
docker run --rm -p 9464:9464 \
  -v "$PWD/deploy/profiles/generated:/app/profiles/generated:ro" \
  -e SENEC_BASE_URL=https://192.168.178.36 \
  -e SENEC_PROFILE_PATH=/app/profiles/generated/senec-profile-live.json \
  timschoenle/senec-v3-collector:v1.1.18
```

Then scrape `http://localhost:9464/metrics`. Point `SENEC_BASE_URL` at your own device; that
address is a default, not a discovery mechanism.

## Table of contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)
- [Operations](#operations)
- [Compatibility](#compatibility)
- [Documentation](#documentation)
- [Contributing](#contributing)
- [Security](#security)
- [License](#license)

## Features

- Values arrive as typed hex tokens such as `fl_41C80000` and `i3_FFFFFFF6`, and are decoded by
  their prefix. `fl_` is an IEEE-754 bit pattern, `i1_`, `i3_` and `i8_` are two's-complement
  integers of 16, 32 and 8 bits, and `u1_`, `u3_`, `u6_` and `u8_` are their unsigned counterparts.
  A key whose value is a list decodes to one sample per element, labelled by `index`.
- Every decoded number is published as `senec_value`, labelled with the object, the key and the
  index it came from. A key the collector has no specific opinion about is still scrapeable.
- **Grid economics are integrated, not read.** The device reports instantaneous power, so the
  collector multiplies `ENERGY.GUI_GRID_POW` and `ENERGY.GUI_HOUSE_POW` by the measured time since
  the previous cycle and accumulates kWh. Tariffs turn those into cost, revenue and a net balance.
- The accumulated totals are written to a JSON file after every cycle and read back at startup, so
  a restart does not reset them. The gap while the process was down is not counted.
- Requests are chunked at `--chunk-size` keys per POST, per object. A device that rejects or
  truncates a large request is handled by lowering that number rather than by editing the profile.
- The runtime image is `FROM scratch` and holds a statically linked musl binary and the CA
  certificate bundle. It runs as `1001:1001` and there is no shell in it.

## Installation

### Docker

```bash
docker pull timschoenle/senec-v3-collector:v1.1.18
```

Built for `linux/amd64` only, from an `x86_64-unknown-linux-musl` target. Every release is signed
with cosign, and the Compose examples pin the image by digest.

### Docker Compose

```bash
docker compose -f deploy/compose/stack.local-bind.yml up -d
```

That brings up the collector, VictoriaMetrics and Grafana, with only Grafana published to the host.
[docs/MONITORING_STACK.md](docs/MONITORING_STACK.md) covers the three Compose files and what each
one persists.

### From source

```bash
git clone https://github.com/TimSchoenle/senec-v3-collector.git
cd senec-v3-collector
cargo build --release --workspace
```

That puts both binaries in `target/release`. Without `--workspace` only `senec-v3-collector` is
built, because `default-members` in the root manifest names it alone.

## Usage

Discovery first. It talks to the device, and writes the profile the collector will read:

```bash
cargo run -p senec-v3-discover -- \
  --base-url https://192.168.178.36 \
  --output deploy/profiles/generated/senec-profile-live.json
```

Then one cycle, to check that the profile and the device agree:

```bash
cargo run -p senec-v3-collector -- --once
```

`--once` polls, exports and exits. The metrics server it started is torn down with it, so this
checks the device and the profile rather than the endpoint.

Serving continuously:

```bash
cargo run -p senec-v3-collector -- --metrics-bind 0.0.0.0:9464
```

The checks CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps
cargo test --doc --workspace --all-features
```

## Configuration

Both binaries take every option as a command-line flag or as an environment variable, and load a
`.env` from the working directory before reading either. There is no configuration file format.
`deploy/compose/.env` is the file the Compose examples read. It is not a template for a local run,
because the paths in it are the ones inside the container.

### Device connection

Read by both binaries.

| Flag | Variable | Default | Purpose |
| --- | --- | --- | --- |
| `--base-url` | `SENEC_BASE_URL` | `https://192.168.178.36` | Base URL of the SENEC system on the local network. |
| `--post-path` | `SENEC_POST_PATH` | `/lala.cgi` | Path the JSON queries are posted to. |
| `--timeout-secs` | `SENEC_TIMEOUT_SECS` | `10` | HTTP timeout per request. |
| `--insecure-tls` | `SENEC_INSECURE_TLS` | `true` | Accept the device's certificate without verifying it. |
| `--chunk-size` | `SENEC_CHUNK_SIZE` | `20` | Maximum keys per POST, per object. A value below `1` is raised to `1`. |

A SENEC v3 presents a certificate no public authority signed, which is why `--insecure-tls`
defaults to on. Turning it off needs a certificate the device can present and your trust store
accepts.

### Collector

| Flag | Variable | Default | Purpose |
| --- | --- | --- | --- |
| `--profile` | `SENEC_PROFILE_PATH` | `deploy/profiles/generated/senec-profile-live.json` | Profile file listing the objects and keys to poll. |
| `--poll-interval-secs` | `SENEC_POLL_INTERVAL_SECS` | `10` | Seconds between cycles. A missed tick delays the next one rather than bursting. |
| `--metrics-bind` | `SENEC_METRICS_BIND` | `0.0.0.0:9464` | Address the metrics server listens on. |
| `--metrics-path` | `SENEC_METRICS_PATH` | `/metrics` | Route the metrics are served at. A leading slash is added when it is missing. |
| `--site-id` | `SENEC_SITE_ID` | `local` | Value of the `site_id` label on every series. |
| `--grid-import-price-eur-per-kwh` | `SENEC_GRID_IMPORT_PRICE_EUR_PER_KWH` | `0.0` | Import tariff. A negative value fails the start. |
| `--grid-export-price-eur-per-kwh` | `SENEC_GRID_EXPORT_PRICE_EUR_PER_KWH` | `0.0` | Feed-in tariff. A negative value fails the start. |
| `--economics-state-path` | `SENEC_ECONOMICS_STATE_PATH` | `state/grid-economics-state.json` | File the accumulated kWh totals are persisted to. |
| `--once` | | `false` | Run one cycle and exit. |

`senec-v3-discover` adds one option of its own, `--output` and `SENEC_DISCOVERY_OUTPUT`, defaulting
to the same profile path the collector reads. `RUST_LOG` sets the log filter for both and defaults
to `info`.

Both path defaults are relative to the working directory. The image's working directory is `/app`,
and it declares `/app/profiles/generated` and `/app/state` as volumes, so a container needs
`SENEC_PROFILE_PATH` and `SENEC_ECONOMICS_STATE_PATH` pointed at those.

## Operations

The metrics route is the whole HTTP surface. There is no health endpoint, and no readiness signal
beyond the process having bound its port.

`senec_scrape_up` is set as the response is rendered, so it reports that the endpoint answered
rather than that the device did. `senec_poll_ok` carries the last cycle's outcome and
`senec_poll_timestamp_seconds` the time it completed. Alert on the age of that timestamp. A failed
poll leaves every `senec_value` gauge holding its previous reading, and nothing else in the scrape
tells a stale value from a fresh one.

The collector handles `SIGINT`. It installs no `SIGTERM` handler, and as PID 1 in a container it
gets no default disposition for one either, so `docker stop` waits out the full grace period before
`SIGKILL`. Nothing is lost to that: the state file is written at the end of every cycle, not at
shutdown.

The state file is the only thing the process writes, so `/app/state` is the only path that has to
be writable. `/app/profiles/generated` can be mounted read-only.

[docs/METRICS.md](docs/METRICS.md) lists every exported series.

## Compatibility

| | Supported |
| --- | --- |
| Device | SENEC v3, through the `/lala.cgi` JSON API |
| Platforms | `linux/amd64` |
| Image | `timschoenle/senec-v3-collector` |
| Exposition | Prometheus text format `0.0.4` |

## Documentation

| Document | Summary |
| --- | --- |
| [`docs/METRICS.md`](docs/METRICS.md) | Every series the collector exports, and how the derived energy and cost counters are computed from two of the device's own keys. |
| [`docs/MONITORING_STACK.md`](docs/MONITORING_STACK.md) | Three Compose files under deploy/compose run the collector next to a scraper and a dashboard, so a working setup is one command rather than three services to wire together. |

That table is walked out of `docs/` rather than maintained by hand, so a document added in a pull
request is listed by the same pull request.

## Contributing

Issues and pull requests are welcome. Commits follow Conventional Commits, and release-please reads
them to open the release pull request, so the type on a commit decides the next version.

`README.md` is generated. It says so in its first lines, and CI reverts an edit made to the output
instead of the template.

## Security

Do not open a public issue for a vulnerability. [SECURITY.md](SECURITY.md) has the reporting route
and which versions get fixes.

The collector does not authenticate the device and, by default, does not verify its certificate. It
is written for a network segment you control, and publishing its metrics port to an untrusted
network exposes every key in the profile.

## License

[LICENSE](LICENSE) has the terms.
