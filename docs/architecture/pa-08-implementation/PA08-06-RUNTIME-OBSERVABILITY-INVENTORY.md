# PA08-06 — Runtime Observability Inventory

**Status:** IMPLEMENTATION COMPLETE; HOSTED VERIFIED on code SHA `643231111624aa170d90f5bf4a467720302cb07a` (PA-08 Run #292).

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
| Workspace filesystem bytes | Gauge | no labels (one configured workspace root per exporter) |
| Reclaimed bytes total | Counter | `resource_kind` |
| Worker loop outcomes/errors | Counter | no labels (fixed metric family per operation) |
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


## 6. Implementation and Hosted acceptance

The implementation slice covers the API, Worker, Runner, filesystem capacity, reclamation accounting, and alert rules. The complete PA-08 Hosted workflow passed on code SHA `643231111624aa170d90f5bf4a467720302cb07a`; PA08-06 is accepted against that verified code SHA.

### Implemented

- **API:** authenticated `GET /metrics` exports Run counts/oldest active age, process-cleanup backlog, workspace-reclamation backlog, cumulative logical reclaimed bytes, and communication-overview aggregates. Database scrape failures return HTTP 503 with a fixed, non-diagnostic response.
- **Worker:** optional loopback Prometheus listener (`AI_PM_WORKER_METRICS_ADDR`, default `127.0.0.1:9465`) exposes bounded claim/dispatch/poll/cleanup counters and workspace filesystem total/available bytes for the configured workspace root.
- **Runner:** optional Prometheus exporter (`AI_PM_RUNNER_METRICS_ADDR`); disabled unless configured, loopback by default, and remote binding requires the explicit `AI_PM_RUNNER_METRICS_ALLOW_REMOTE=true` override. It exports bounded MCP operation/outcome, supervision-error, cleanup, uptime, and scrape counters.
- **Reclamation accounting:** `ai_pm_workspace_reclaimed_bytes_total` is a monotonic Counter backed by migration `0057`; its only label is `resource_kind`. Byte values are logical regular-file sizes, not physical block accounting.
- **HTTP/security tests:** authenticated and unauthenticated API scrape paths, Prometheus content type, HELP/TYPE exposition, real newline output, database-failure 503, and absence of project/user identifiers or raw diagnostics are covered. Worker and Runner exporters have endpoint/exposition tests.
- **Alerts:** `infrastructure/monitoring/pa08-alerts.yml` includes pending Worker command age, process-cleanup and reclamation needs-human state, Worker workspace filesystem pressure, Runner supervision errors, and Runner cleanup errors.

### Hosted validation

The PA08 workflow now checks locked dependency metadata and public-registry resolution, workspace build, workspace Clippy, Runner integration tests, Worker/Runner metrics tests, PostgreSQL lifecycle/cleanup/reclamation tests, Product lifecycle/concurrency tests, authenticated API metrics tests, Prometheus rule syntax via `promtool`, and Windows mirror-lock plus Runner/Worker platform compilation.

Verified code SHA: `643231111624aa170d90f5bf4a467720302cb07a`.

- PA-08 Run [#45044216787](https://github.com/luqizheng/ai-pm/actions/runs/45044216787): 8/8 jobs successful, including all four Linux Nextest shards (2,000 passed, 0 failed), Linux doctests, public-registry build/Clippy/format/migration/alert validation, and Windows mirror-lock plus Runner/Worker compilation.
- PA-07 Run [#45044216776](https://github.com/luqizheng/ai-pm/actions/runs/45044216776): 2/2 jobs successful; the `batch_create_tasks` PostgreSQL integration target executed (2 passed, 0 failed; not skipped).
- API HTTP tests cover unauthenticated rejection, authenticated Prometheus exposition/content type, bounded output without identifiers, and database failure 503. Worker and Runner exporter tests are part of the full workspace suite.

No PA08-06 implementation or Hosted acceptance gate remains open. Archive size/performance is tracked separately as a non-blocking CI optimization follow-up. The later branch commit `b57768c7af8492fb283b7928e3b2d8b0850489c3` changes this documentation only; Hosted test evidence above is intentionally attributed to the exact code SHA.

