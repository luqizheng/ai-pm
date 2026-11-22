# PA08-06 — Runtime Observability Inventory

**Status:** INVENTORY COMPLETE; PARTIAL IMPLEMENTATION LANDED (API AGGREGATE METRICS, RECLAMATION ACCOUNTING, CLEANUP NEEDS-HUMAN API).

## 1. Current-state facts

- Workspace dependencies contain no Prometheus, `metrics`, or OpenTelemetry exporter.
- `pm-api` is an Axum service with a single main listener and an authenticated `GET /metrics` Prometheus text exposition endpoint.
- `ai-pm-worker` has no HTTP listener or metrics dependency. It emits structured logs for claim/poll/cleanup/reclamation errors.
- `ai-pm-runner` is a client process and uses `tracing`; it does not expose an HTTP metrics endpoint.
- `pm_application::comm_overview::load_comm_overview` already aggregates shielded Runs, unconsumed interaction responses, open interaction chains, pending worker commands, and oldest pending command age. The Worker logs these fields every 500 dispatch-loop ticks.
- `delivery.task_run_cleanup` has fenced retry and `needs_human` state plus explicit requeue; the application exposes a bounded project/tenant-scoped list and the API exposes it at `GET /v1/pm/task-run-cleanups/needs-human`.
- `delivery.workspace_reclamation` has `needs_human` listing, explicit operator requeue, and logical reclaimed-byte accounting (migration `0056`). The API exports cumulative logical bytes by bounded `resource_kind` only; the cumulative total is stored separately from per-operation ledger state (migration `0057`) so the Prometheus Counter remains monotonic.
- No filesystem capacity/available-space gauge is collected yet. Logical reclaimed bytes are measured from regular-file sizes before deletion and do not represent physical block accounting.
- Existing operational logs include entity IDs in individual diagnostic events. Such identifiers must not become Prometheus labels.

## 2. Minimal implementation direction

Prefer a small pull-based Prometheus text exposition with no new monitoring service or event platform:

1. **API metrics endpoint:** expose database-backed global aggregates for Run counts/ages, cleanup backlog, reclamation backlog, and existing communication overview.
2. **Worker metrics endpoint:** expose bounded in-process counters and workspace filesystem capacity/available-space gauges. Bind to loopback by default; require explicit operator configuration to expose beyond localhost.
3. **Runner telemetry:** add bounded counters for claim, heartbeat, command polling/acknowledgement, and supervision errors. Because Runner commonly executes on a separate host, keep its exporter optional and document how operators scrape it locally.
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
3. Add Worker metrics listener and filesystem capacity gauges, defaulting to loopback.
4. Add reclamation byte accounting and process-cleanup `needs_human` list query. **Completed:** migration `0056`, logical reclaimed-byte metric, and project-scoped needs-human API.
5. Instrument Worker and Runner operation counters with bounded dimensions.
6. Add exporter tests, security assertions, and Hosted CI steps.

No Kafka, external SaaS, distributed tracing backend, or separate event-sourcing system is introduced.


## 6. Implementation update — first API slice

The branch now contains `apps/pm-api/src/metrics.rs` and a `GET /metrics` route.

Implemented:
- Prometheus text exposition for Run counts and oldest active Run age, process-cleanup backlog by status, workspace reclamation backlog by resource/status, and the existing communication overview aggregates.
- The route is inside the normal authenticated router; the existing public `/health` probe remains separate.
- Scrape/database failures return HTTP 503 rather than presenting stale values as fresh.
- Labels are limited to status/kind dimensions; entity IDs, user content, repository URLs, tokens and raw errors are excluded.
- Label escaping has a unit test.

Not yet implemented:
- Worker and Runner operation counters / exporters.
- Worker loopback listener and filesystem capacity/available-space gauges.
- Full scrape/endpoint integration tests and alerting thresholds.

Implemented after the first API slice:
- Logical reclaimed-byte accounting and the `ai_pm_workspace_reclaimed_bytes_total` metric.
- Project/tenant-scoped process-cleanup `needs_human` listing API.
- Hosted verification of the workspace build, Runner suite, PostgreSQL reclamation/cleanup suites, Product lifecycle/concurrency suites, and metrics exposition unit tests.

Verification update: the API aggregate metrics code and its label-escaping unit test passed [Run #156](https://github.com/luqizheng/ai-pm/actions/runs/44976642473). Reclaimed-byte accounting, needs-human API, and the full PA08 Hosted test workflow passed [Run #44978431052](https://github.com/luqizheng/ai-pm/actions/runs/44978431052). PA08-02 delayed duplicate command and command-poll transport retry coverage also passed [Run #44979290867](https://github.com/luqizheng/ai-pm/actions/runs/44979290867). PA-07 blast-radius unit and integration jobs passed [Run #44979290858](https://github.com/luqizheng/ai-pm/actions/runs/44979290858). This does not close the remaining Worker/Runner exporter, filesystem gauge, alerting, or scrape integration gates.

This is deliberately a partial slice, not PA08-06 acceptance. See the implementation plan for remaining gates.
