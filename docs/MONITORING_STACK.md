# Monitoring stack

Three Compose files under `deploy/compose` run the collector next to a scraper and a dashboard, so a working setup is one command rather than three services to wire together.

Every image in them is pinned by digest. Renovate bumps those pins, which is why the digests are
written out rather than left as floating tags.

## The three files

| File | Services | Published to the host |
| --- | --- | --- |
| `stack.yml` | collector, VictoriaMetrics, Grafana | Grafana on `${GRAFANA_PORT}`, default `3000` |
| `stack.local-bind.yml` | collector, VictoriaMetrics, Grafana | Grafana on `${GRAFANA_PORT}`, default `3000` |
| `collector.local-bind.yml` | collector | The collector on `${COLLECTOR_PORT}` |

The collector and VictoriaMetrics stay on the internal `senec` network in both full stacks. Nothing
reaches the metrics endpoint from outside the Compose network unless you add a port mapping.

The two full stacks differ only in where state lands. `stack.local-bind.yml` puts all of it under
`deploy/compose/data/`, which is what you want when the host is a machine you back up by copying a
directory. `stack.yml` uses named volumes for the collector state and for Grafana, and a bind mount
under `deploy/compose/data/victoriametrics` for the time series. Its `victoriametrics-data` volume
is declared and unused.

`collector.local-bind.yml` reads `COLLECTOR_PORT`, which `deploy/compose/.env` does not set. Supply
it on the command line or add it to the file before starting that one.

## Running

Compose takes its project directory from the first `-f`, so `deploy/compose/.env` is loaded and the
relative bind mounts resolve without a `cd`:

```bash
docker compose -f deploy/compose/stack.local-bind.yml up -d
```

Grafana then answers on `http://localhost:3000`, with the credentials from `GRAFANA_ADMIN_USER` and
`GRAFANA_ADMIN_PASSWORD`. Both default to `admin` in the committed `.env`. Change them before the
host is reachable by anything but you.

## What the stack is configured with

`deploy/victoriametrics/scrape.yaml` holds one scrape job, `senec-collector`, pointed at
`collector:9464`. VictoriaMetrics reads it through `-promscrape.config` and keeps
`VICTORIAMETRICS_RETENTION` of history, five years in the committed `.env`.

Grafana is provisioned from `deploy/grafana/provisioning`:

- The datasource is VictoriaMetrics at `http://victoriametrics:8428`, of Prometheus type, marked
  default and not editable in the UI. The file also deletes a datasource named `Prometheus` and
  sets `prune: true`, which removes provisioned datasources that are no longer in the file. Both
  are there to clear out an earlier Prometheus-backed version of this stack.
- Dashboards load from `deploy/grafana/dashboards` into a folder named `SENEC`, rescanned every ten
  seconds. `senec-v3-production-overview.json` is the one dashboard in it.
- `GF_USERS_ALLOW_SIGN_UP` is off.

`PROMETHEUS_PORT` and `PROMETHEUS_RETENTION_TIME` in `deploy/compose/.env` are read by nothing. No
Compose file here runs Prometheus.

## Pointing an existing scraper at the collector

The collector serves the Prometheus text format and needs no adapter. Scrape it directly:

```yaml
scrape_configs:
  - job_name: senec-collector
    static_configs:
      - targets: ['collector:9464']
```

`SENEC_SITE_ID` becomes the `site_id` label on the derived series, so several collectors can report
into one scraper without their totals colliding.
