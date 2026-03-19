# ADR-0012: Grafana Alloy + Grafana Cloud for Monitoring

**Date:** 2026-03-18
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse needs monitoring for the API: request latency, error rates, throughput per endpoint, and alerting when things break. The platform runs on a single 4GB k3s droplet, so the monitoring stack must be lightweight.

The API exposes a Prometheus-compatible `/metrics` endpoint via `axum-prometheus` middleware (request count, latency histograms, status codes per route). Something needs to scrape that endpoint and store the data somewhere dashboards and alerts can consume it.

---

## Decision

Use **Grafana Alloy** (Grafana's open-source telemetry collector) to scrape the API's `/metrics` endpoint and push metrics to **Grafana Cloud** (free tier) via `remote_write`.

- **No on-cluster TSDB** — Grafana Cloud stores the metrics (10k series, 14-day retention on free tier)
- **No on-cluster Grafana** — dashboards and alerting live in Grafana Cloud
- **Alloy is the only monitoring component on the droplet** — ~40MB RAM, single container

---

## Alternatives Considered

### VictoriaMetrics single-node + vmagent + Grafana Cloud

Run VictoriaMetrics as a local TSDB with 30-day retention, and vmagent as a sidecar to remote-write to Grafana Cloud.

Rejected because:
- Two components instead of one (VM + vmagent), ~130MB RAM combined.
- Local storage (PVC) needed even though Grafana Cloud is the primary dashboard.
- The 30-day local retention doesn't add value over Grafana Cloud's 14 days at this scale.
- More moving parts to fail on a single-node cluster.

### Full kube-prometheus-stack (Prometheus Operator + Grafana)

The standard Kubernetes monitoring stack. Prometheus Operator, Alertmanager, Grafana, node-exporter, kube-state-metrics.

Rejected because:
- Uses 1-2GB RAM — half the droplet's capacity.
- Massive operational complexity for a single-node cluster with one API.
- Designed for multi-team, multi-cluster environments.

### Prometheus single-binary + Grafana

Lighter than the operator stack, but still requires two services (Prometheus + Grafana) on-cluster, with local storage for both.

Rejected because Grafana Cloud eliminates the need for on-cluster Grafana, and Alloy is lighter than Prometheus for the scrape-and-forward use case.

### No monitoring (structured logs only)

Rely on `kubectl logs` and `tracing` JSON output. Add monitoring later.

Rejected because latency percentiles and error rates aren't derivable from logs without a log aggregation pipeline, which would be heavier than Alloy.

---

## Consequences

**Positive:**
- Single lightweight container (~40MB RAM) handles all metrics collection.
- Grafana Cloud provides dashboards, alerting, and 14-day retention with zero on-cluster infrastructure.
- Alloy is Grafana's supported collector — it's what Grafana Cloud expects and integrates with natively.
- If we outgrow the free tier, we can add a local TSDB (VictoriaMetrics) behind Alloy without changing the scrape config.

**Negative / tradeoffs:**
- Depends on Grafana Cloud (external service). If Grafana Cloud is down, we lose dashboards and alerting until it recovers. Mitigated: Alloy buffers locally and replays when the endpoint is back.
- Free tier has 14-day retention and 10k series limit. Sufficient for one API with a handful of endpoints; will need paid tier or local TSDB if series count grows significantly.
- Grafana Cloud credentials must be managed as a Kubernetes Secret (SealedSecret in infra repo).

---

## References

- Grafana Alloy: https://grafana.com/docs/alloy/latest/
- Grafana Cloud free tier: https://grafana.com/pricing/
- axum-prometheus: https://github.com/Ptrber/axum-prometheus
