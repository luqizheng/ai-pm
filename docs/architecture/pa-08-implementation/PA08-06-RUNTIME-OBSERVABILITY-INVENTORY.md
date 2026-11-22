# PA08-06 — Runtime Observability Inventory

**Status:** INVENTORY COMPLETE; PARTIAL IMPLEMENTATION LANDED (API AGGREGATE METRICS ONLY).

## 1. Current-state facts

- Workspace dependencies contain no Prometheus, `metrics`, or OpenTelemetry exporter.
- `pm-api` is an Axum service with a single main listener; no `/metrics` endpoint is mounted.
- `ai-pm-worker` has no HTTP listener or metrics dependency. It emits structured logs for claim/poll/cleanup/reclamation errors.
- `ai-pm-runner` is a client process and uses `tracing`; it does not expose an HTTP metrics endpoint.
- `pm_application::comm_overview::load_comm_overview` already aggregates shielded Runs, unconsumed interaction responses, open interaction chains, pending worker commands, and oldest pending command age. The Worker logs these fields every 500 dispatch-loop ticks.
- `delivery.task_run_cleanup` has fenced retry and `needs_human` state plus explicit requeue, but no bounded list operation.
- `delivery.workspace_reclamation` has `needs_human` listing and explicit operator requeue.
- Workspace reclamation records completion but do not record reclaimed byte counts. No filesystem capacity/available-space gauge is collected.
- Existing operational logs include entity IDs in individual diagnostic events. Such identifiers must not become Prometheus labels.

## 2. Minimal implementation direction

Prefer a small pull-based Prometheus text exposition with no new monitoring service or event platform:

1. **API metrics endpoint:** expose database-backed global aggregates for Run counts/ages, cleanup backlog, reclamation backlog, and existing communication overview.
2. **Worker metrics endpoint:** expose bounded in-process counters and workspace filesystem capacity/available-space gauges. Bind to loopback by default; require explicit operator configuration to expose beyond localhost.
3. **Runner telemetry:** add bounded counters for claim, heartbeat, command polling/acknowledgement, and supervision errors. Because Runner commonly executes on a separate host, keep its exporter optional and document how operators scrape it locally.
4. **Reclamation accounting:** record reclaimed bytes per ledger operation so cumulative bytes can be exported without rescanning historical directories.
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
4. Add reclamation byte accounting and process-cleanup `needs_human` list query.
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
- Reclaimed-byte accounting and cleanup needs-human listing API.
- Full scrape/endpoint integration tests, alerting thresholds and hosted CI verification.

This is deliberately a partial slice, not PA08-06 acceptance. See the implementation plan for remaining gates.
