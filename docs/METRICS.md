# Metrics

Every series the collector exports, and how the derived energy and cost counters are computed from two of the device's own keys.

All series carry the `senec` registry prefix, which the Prometheus client library joins to the name
each metric was registered under. Names below are the full ones as they appear in a scrape.

## Device values

| Series | Labels | Meaning |
| --- | --- | --- |
| `senec_value` | `site_id`, `object`, `key`, `index` | One decoded number from the device. `object` and `key` are the SENEC names, `index` the position within a key whose value is a list. |

Everything in the loaded profile lands here, whether or not the collector understands it. A key
that decodes to three numbers becomes three series at `index` `0`, `1` and `2`.

## Collector health

| Series | Meaning |
| --- | --- |
| `senec_scrape_up` | Set to `1` while the payload is being rendered. It reports that the endpoint answered, not that the device did. |
| `senec_poll_ok` | `1` after a poll cycle that completed, `0` after one that failed. |
| `senec_poll_timestamp_seconds` | Unix time of the last completed cycle, successful or not. |

None of the three carries a label. A failed cycle leaves every `senec_value` gauge holding its last
reading, because a Prometheus gauge is not cleared by the absence of an update, so
`senec_poll_timestamp_seconds` is the series to alert on. Compare it against `time()` and the poll
interval.

## Grid economics

All of these carry a single `site_id` label, taken from `SENEC_SITE_ID`.

| Series | Meaning |
| --- | --- |
| `senec_grid_import_price_eur_per_kwh` | The configured import tariff, republished so a dashboard can read it. |
| `senec_grid_export_price_eur_per_kwh` | The configured feed-in tariff. |
| `senec_grid_import_power_w` | `ENERGY.GUI_GRID_POW` when positive, else `0`. |
| `senec_grid_export_power_w` | `ENERGY.GUI_GRID_POW` negated when negative, else `0`. |
| `senec_grid_import_energy_kwh_total` | Cumulative imported energy. |
| `senec_grid_export_energy_kwh_total` | Cumulative exported energy. |
| `senec_house_consumption_energy_kwh_total` | Cumulative house demand, integrated from `ENERGY.GUI_HOUSE_POW`. |
| `senec_self_supplied_energy_kwh_total` | House demand minus imported energy, floored at zero. |
| `senec_self_sufficiency_percent` | Self-supplied energy as a percentage of house demand. Zero while house demand is still zero. |
| `senec_grid_import_cost_eur_total` | Imported energy times the import tariff. |
| `senec_grid_export_revenue_eur_total` | Exported energy times the feed-in tariff. |
| `senec_grid_net_balance_eur_total` | Revenue minus cost. |

The `_total` suffix is conventional here and nothing more. Every one of these is a Prometheus
gauge, so `rate()` and `increase()` do not apply to them; the collector does the accumulation
itself and publishes the running total.

### How the accumulation works

`ENERGY.GUI_GRID_POW` is signed: positive while the house draws from the grid, negative while the
battery or the array feeds into it. The collector splits it into the two power gauges, then
multiplies each by the wall-clock seconds since the previous cycle to get an energy increment. The
interval is measured, not assumed, so a missed tick or a slow device is accounted for rather than
counted as one poll interval.

Cost and revenue are derived from those cumulative kWh figures on every publish, not accumulated
separately. Changing a tariff therefore reprices the whole history rather than the period after the
change.

### Persistence

`SENEC_ECONOMICS_STATE_PATH` names a JSON file holding the three accumulated kWh figures. It is
written after every cycle through a temporary file and a rename, and read back at startup.

The timestamp is deliberately not part of that file. A restart therefore starts a fresh interval,
and the energy that flowed while the process was down is not counted. Counting it would mean
assuming the last observed power held for the whole outage, which is the assumption most likely to
be wrong exactly when the process was down.

Deleting the state file resets every cumulative series to zero.
