# PA08-06 — Runtime Observability Inventory

**Status:** INVENTORY COMPLETE; IMPLEMENTATION IN PROGRESS (API, WORKER, RUNNER EXPORTERS AND FILESYSTEM GAUGES IMPLEMENTED; HOSTED VALIDATION PENDING).

## 1. Current-state facts

- Workspace dependencies contain no Prometheus, `metrics`, or OpenTelemetry exporter.
- `pm-api` is an Axum service with a single main listener and an authenticated `GET /metrics` Prometheus text exposition endpoint.
- `ai-pm-worker` now has an optional loopback-only Prometheus listener (`AI_PM_WORKER_METRICS_ADDR`, default `127.0.0.1:9465`) with bounded in-process counters for analysis/import claims, Run dispatch attempts/errors, Run polling errors, cleanup errors, and scrapes. The endpoint is intentionally minimal and does not add a monitoring dependency.
- `ai-pm-runner` now has an optional Prometheus HTTP exporter (`AI_PM_RUNNER_METRICS_ADDR`); it is disabled unless explicitly configured, binds loopback unless `AI_PM_RUNNER_METRICS_ALLOW_REMOTE=true`, and exposes bounded MCP operation/outcome, supervision-error, cleanup, uptime, and scrape counters.
- `pm_application::comm_overview::load_comm_overview` already aggregates shielded Runs, unconsumed interaction responses, open interaction chains, pending worker commands, and oldest pending command age. The Worker logs these fields every 500 dispatch-loop ticks.
- `delivery.task_run_cleanup` has fenced retry and `needs_human` state plus explicit requeue; the application exposes a bounded project/tenant-scoped list and the API exposes it at `GET /v1/pm/task-run-cleanups/needs-human`.
- `delivery.workspace_reclamation` has `needs_human` listing, explicit operator requeue, and logical reclaimed-byte accounting (migration `0056`). The API exports cumulative logical bytes by bounded `resource_kind` only; the cumulative total is stored separately from per-operation ledger state (migration `0057`) so the Prometheus Counter remains monotonic.
- Worker exports workspace filesystem total/available bytes from the configured workspace root. Logical reclaimed bytes are measured from regular-file sizes before deletion and do not represent physical block accounting.
- Existing operational logs include entity IDs in individual diagnostic events. Such identifiers must not become Prometheus labels.

## 2. Minimal implementation direction

Prefer a small pull-based Prometheus text exposition with no new monitoring service or event platform:

1. **API metrics endpoint:** expose database-backed global aggregates for Run counts/ages, cleanup backlog, reclamation backlog, and existing communication overview.
2. **Worker metrics endpoint:** expose bounded in-process counters and workspace filesystem capacity/available-space gauges. Implemented with a loopback exporter; filesystem gauges use the configured workspace root.
3. **Runner telemetry:** bounded counters for claim, discovery, heartbeat, command polling/acknowledgement, lease reacquisition, cleanup, and supervision errors. Implemented as an optional loopback exporter; remote binding requires an explicit override.
4. **Reclamation accounting:** record logical bytes per ledger operation and atomically increment a durable per-resource cumulative total (migration `0057`) so Counter values remain monotonic across retries and requeues.
5. **Cleanup operations:** add a bounded `needs_human` listing query for process-cleanup records, matching the reclamation ledger operations.

A plain-text exporter is acceptable only if output follows Prometheus exposition syntax, has deterministic metric names/types, escapes label values correctly, and is covered by tests. Do not hand-concatenate untrusted label values.

## 3. Required metrics and label policy

| Metric family | Type | Labels (bounded only) |
|---|---|---|
| Run count | Gauge | `run_kind`, `status` |
| Active Run age | Gauge | `run_kind`, `status` |
| Run cleanup backlog | Gauge | `status` |
| Workspace reclamation backlog | Gauge | `resource_kind`, `status` |
| Workspace filesystem bytes | Gauge | `mount`, `capacity_kind` |
| Reclaimed bytes total | Counter | `resource_kind` |
| Worker loop outcomes/errors | Counter | `loop`, `outcome` |
| Runner MCP outcomes/errors | Counter | `operation`, `outcome` |
| Pending command age/count | Gauge | no labels |
| Interaction overview | Gauge | no labels |

Never use `run_id`, `task_id`, `project_id`, `repository_id`, user identity, URL, error text, or arbitrary command names as metric labels.

## 4. Operational and security requirements

- Metrics endpoints must not return credentials, tokens, user content, prompts, repository URLs, or raw error strings.
- Worker metrics listener binds to `127.0.0.1` by default; public/network binding is an explicit deployment decision.
- Database query failures must fail the scrape clearly or expose a bounded exporter-error gauge; never silently emit stale values as current.
- Disk metrics describe only the configured workspace filesystem; do not infer remote Runner disk usage from Worker state.
- Retain structured logs for diagnostic detail; metrics are aggregate signals, not a replacement for audit history.
- Test output for bounded labels and verify no entity identifiers appear in exposition.

## 5. Implementation sequence

1. Add typed SQL aggregate snapshots and tests for Run, cleanup, reclamation, and communication metrics.
2. Add API `/metrics` exposition and response/content-type tests.
3. Add Worker metrics listener, core counters, and workspace filesystem capacity gauges (implemented; loopback by default).
4. Add reclamation byte accounting and process-cleanup `needs_human` list query. **Completed:** migration `0056`, logical reclaimed-byte metric, and project-scoped needs-human API.
5. Instrument Worker and Runner operation counters with bounded dimensions (implemented).
6. Add exporter tests, security assertions, alert-rule syntax validation, and Hosted CI steps (HTTP/unit tests added; latest Hosted validation pending).

No Kafka, external SaaS, distributed tracing backend, or separate event-sourcing system is introduced.


## 6. Implementation update — first API slice

The branch now contains `apps/pm-api/src/metrics.rs` and a `GET /metrics` route.

Implemented:
- Prometheus text exposition for Run counts and oldest active Run age, process-cleanup backlog by status, workspace reclamation backlog by resource/status, and the existing communication overview aggregates.
- The route is inside the normal authenticated router; the existing public `/health` probe remains separate.
- Scrape/database failures return HTTP 503 rather than presenting stale values as fresh.
- Labels are limited to status/kind dimensions; entity IDs, user content, repository URLs, tokens and raw errors are excluded.
- Label escaping has a unit test.

Implemented in the current PA08 branch:
- Worker loopback Prometheus listener, bounded process counters, and HTTP endpoint tests for `/metrics` and 404 behavior.
- Worker counters are wired to analysis/import claims, Run dispatch attempts/errors, Run polling errors, and cleanup errors.

Remaining validation:
- Confirm the new authenticated API `/metrics` HTTP integration tests on the latest PR head.
- Validate `infrastructure/monitoring/pa08-alerts.yml` with Prometheus `promtool` and verify alert metric names/thresholds.
- Confirm Worker/Runner exporters and filesystem gauges on Linux and Windows Hosted CI.

Implemented after the first API slice:
- Logical reclaimed-byte accounting and the `ai_pm_workspace_reclaimed_bytes_total` metric.
- Project/tenant-scoped process-cleanup `needs_human` listing API.
- Hosted verification of the workspace build, Runner suite, PostgreSQL reclamation/cleanup suites, Product lifecycle/concurrency suites, and metrics exposition unit tests.

Verification update: the API aggregate metrics code and its label-escaping unit test passed [Run #156](https://github.com/luqizheng/ai-pm/actions/runs/44976642473). Reclaimed-byte accounting, needs-human API, and the full PA08 Hosted test workflow passed [Run #44978431052](https://github.com/luqizheng/ai-pm/actions/runs/44978431052). PA08-02 delayed duplicate command and command-poll transport retry coverage also passed [Run #44979290867](https://github.com/luqizheng/ai-pm/actions/runs/44979290867). PA-07 blast-radius unit and integration jobs passed [Run #44979290858](https://github.com/luqizheng/ai-pm/actions/runs/44979290858). This does not close the remaining Runner exporter/counters, Worker filesystem gauges, alerting, or API scrape integration gates. The Worker exporter and HTTP endpoint test are now implemented but must pass the latest Hosted CI.

This is deliberately a partial slice, not PA08-06 acceptance. See the implementation plan for remaining gates.


## 7. Implementation update — Runner and filesystem metrics

- Runner now exposes an optional HTTP exporter configured by `AI_PM_RUNNER_METRICS_ADDR`; it is disabled by default, binds loopback unless `AI_PM_RUNNER_METRICS_ALLOW_REMOTE=true`, and exports only bounded operation/outcome dimensions.
- Runner counters cover claim, discovery, heartbeat, command polling, command acknowledgement, lease reacquisition, interactions, supervision errors, orphan/lease-loss cleanup, uptime, and scrapes. Entity identifiers, MCP tool names outside the fixed allow-list, URLs, credentials, and raw errors are excluded.
- Worker now exports workspace filesystem total and available bytes for the same workspace root used by Run dispatch/reclamation. Unix uses `statvfs`; Windows uses `GetDiskFreeSpaceExW`.
- API `/metrics` integration tests now cover missing bearer rejection, authenticated exposition/content type and entity-ID exclusion, and database failure returning 503. These tests are added to Hosted CI but are not accepted until the latest head passes.
- Runner startup now identity-checks and terminates orphaned child processes before recovery/claiming; this is part of the P0 single-Agent fencing work, not a metrics feature.
