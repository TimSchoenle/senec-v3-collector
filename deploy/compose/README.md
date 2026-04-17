# Docker Compose Examples

All Compose examples for this repository live in this folder.

Available files:

- `stack.yml`: Full collector + Prometheus + VictoriaMetrics + Grafana stack using named volumes.
- `stack.local-bind.yml`: Full stack using only relative bind mounts (`./data/...`) for local persistence.
- `collector.local-bind.yml`: Collector-only setup using relative bind mounts.

By default, in the full stack examples only Grafana is published to the host.
Collector, Prometheus, and VictoriaMetrics stay internal to the Docker network.

Run an example from the repository root:

```powershell
docker compose -f deploy/compose/stack.local-bind.yml up -d --build
```
