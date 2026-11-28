# AI-PM ↔ SEM Callback and Inbox Design (Draft)

- Status: **DRAFT / BILATERAL REVIEW — NOT IMPLEMENTATION-AUTHORIZING**
- Source baseline audited: `master@d0e72fa6805aebeadf1b282e430deba493072fd6`
- Related SEM contract: `smark-sem/docs/protocol/PLATFORM-SEM-EXECUTION-MESSAGE-PROTOCOL-DRAFT.md`
- Related review: `smark-sem/docs/protocol/D04-AI-PM-CROSS-REPOSITORY-REVIEW.md`
- Scope: AI-PM ownership of SEM callback ingress, Inbox deduplication, per-execution event sequencing and permitted Run observations.
- No AI-PM source, migration, route, runtime configuration or database was changed by this document.

## 1. Decision summary proposed for ratification

1. AI-PM remains the business authority. SEM callback events are infrastructure observations only; they cannot set TaskStatus, approve, accept a delivery, or complete a Task.
2. The callback is a dedicated service-to-service endpoint, not an alias of `POST /api/v1/events`.
3. The existing generic `integration.inbound_event` is **not reusable through the current `submit_event()` operation unchanged**: that operation always creates an `integration.job(job_type='ai_analysis')` for a new event. SEM heartbeat/process/release telemetry must not accidentally launch ordinary AI analysis.
4. **Recommended V1 storage choice:** reuse the shared `integration.inbound_event` ledger through a dedicated application operation `receive_sem_event()` with `source='sem'`, but do **not** enqueue `ai_analysis`. Add a contiguous cursor to the existing `delivery.task_run` attempt row and append one `sem_infrastructure_observed` entry to `delivery.task_run_event`. This avoids a parallel Inbox table while preserving a distinct SEM receive/application path. Only create `integration.sem_event_inbox` if migration/source review proves the shared ledger cannot meet global event-ID uniqueness, retention or cursor semantics.
5. The protocol `execution_id` maps to one AI-PM execution attempt row: **proposed mapping: `delivery.task_run.id == execution_id`**. In the current AI-PM model, one `task_run` row is one attempt; `attempt_no` and `supersedes_run_id` express the attempt chain. This avoids a second execution-ID mapping table. SEM's own Execution lifecycle remains a separate infrastructure state machine even when the correlation UUID equals the AI-PM attempt row ID.
6. Event ACK is returned only after the Inbox/idempotency record, run-event evidence and contiguous cursor update commit atomically.

## 2. Source-verified current state

| Source at pinned master | Current behavior | Consequence |
|---|---|---|
| `apps/pm-api/src/routes.rs` | Registers `POST /api/v1/events`; handler uses `AuthedPrincipal`, project membership/visibility checks and overwrites `actor_id` from the authenticated caller | This is not a service callback route; do not weaken its human/project authorization contract |
| `crates/pm-application/src/ingestion.rs` | `submit_event()` inserts `integration.inbound_event` with `(project_id, source, idempotency_key)` idempotency and payload hash; on new insert it also creates an `integration.job(job_type='ai_analysis')` in the same transaction | Do not call `submit_event()` unchanged for SEM events |
| `docs/architecture/ai-pm-architecture-principles.md` §7.2 | Generic Inbox includes `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash`, `status` | A SEM service principal can be represented as a service actor if shared Inbox reuse is approved |
| `docs/architecture/knowledge-services/ownership-checklist.md` W-3 | Shared Outbox/Inbox writes must go through application ports; direct route-level SQL is prohibited | Route delegates to an application port; no SQL in handler |
| `docs/architecture/server-execution-management/session-manager-design.md` §1/§7 | A `task_run` row is one Attempt; a business execution chain is represented by linked rows and `attempt_no` | Proposed `execution_id = task_run.id` is a one-attempt correlation mapping |
| `migrations/0013_task_run_event.sql` | `delivery.task_run_event` is append-only, currently has eight event types and idempotency `(run_id,event_type,lease_generation,dedupe_tag)` | It can be extended as the AI-PM Run timeline, but must retain append-only and existing lease-generation semantics |

The generic inbound event ledger is not a substitute for the callback handler. The route, service authentication, SEM-specific event validation, sequence cursor and application interpretation do not currently exist in the pinned master source.

## 3. Dedicated callback route

Proposed endpoint:

```http
POST /internal/ai-pm/v1/sem-events
Content-Type: application/json
Idempotency-Key: <event_id>
Content-Digest: sha-256=:...:
Signature-Input: sig1=(...);created=...;expires=...;nonce="...";keyid="sem-service-key-v1";alg="ed25519"
Signature: sig1=:...:
```

- One event per request for V1; no unbounded batches.
- The body is the canonical SEM `SemInfrastructureEventEnvelope` from the D-04 contract.
- `Idempotency-Key` MUST equal body `event_id`.
- Signature and Content-Digest are verified before JSON parsing/DB writes; nonce replay is atomically rejected.
- The SEM signing key is a service key, separate from Runner keys and TLS certificate keys.
- HTTP is restricted to the approved private network; optional HTTPS validates enterprise CA or explicitly pinned self-signed trust. No `verify=false` path.
- Do not route this through the existing `AuthedPrincipal`/human membership middleware and do not synthesize a `HumanActor`. Use a dedicated authenticated service principal (e.g. `service=sem`) with an explicit callback permission.

The handler must not accept `project_id`, `task_id`, `run_id`, actor identity or Task status from the SEM payload. It resolves ownership from the AI-PM `execution_id` mapping.

## 4. Execution identity mapping

Proposed mapping for V1:

- `execution_id == delivery.task_run.id` (the exact attempt row).
- `task_id`, `project_id`, `tenant_id`, `attempt_no`, `run_kind`, lease generation and authorization state are loaded from that row and its owning Task/Project.
- Require `run_kind='delivery'`; review runs do not receive SEM execution callbacks.
- The business attempt chain remains represented by `supersedes_run_id` and `attempt_no`; SEM never receives authority to mutate that chain.
- Unknown execution IDs return a non-enumerating 404. Cross-tenant or wrong-run-kind IDs are treated as not found.
- This mapping must be cross-checked against the canonical SEM identity tuple before ratification. If `execution_id` must instead be a distinct AI-PM identifier, a unique mapping table/column and its lifecycle must be specified before implementation.

## 5. Inbox and sequence semantics

### 5.1 Shared Inbox reuse candidate

Preferred first candidate is to add a dedicated application port `receive_sem_event()` that writes the existing `integration.inbound_event` with:

- `source='sem'`;
- `event_type` equal to the strict SEM event type;
- `idempotency_key=event_id`;
- canonical payload hash over the exact accepted JSON representation;
- `actor_id` mapped from the authenticated SEM service principal;
- `project_id/tenant_id` resolved from the `task_run` row.

Unlike generic `submit_event()`, this operation MUST NOT create `integration.job(job_type='ai_analysis')`. It records infrastructure evidence and invokes only the SEM Run-observation path.

The generic table's current uniqueness is `(project_id, source, idempotency_key)`. For SEM, the route resolves the project from `execution_id` before insertion and uses `source='sem'`, `idempotency_key=event_id`; the event ID/body hash check is performed inside the same transaction. A partial unique index on `(source,idempotency_key) WHERE source='sem'` is recommended as defense in depth so a single event ID cannot be replayed under a different project mapping. This must be tested under concurrent duplicate delivery.

### 5.2 Per-execution contiguous cursor

The callback must maintain a contiguous `execution_event_sequence` per `execution_id`, independent of Runner journal sequence and WSS transport `message_sequence`.

Candidate minimal storage:
- Add `execution_event_sequence BIGINT NOT NULL DEFAULT 0 CHECK (execution_event_sequence >= 0)` to the AI-PM attempt row (`delivery.task_run`).
- Serialize updates by locking the exact `task_run` row `FOR UPDATE`.
- Record the SEM event in `integration.inbound_event` and append one `delivery.task_run_event(event_type='sem_infrastructure_observed')` observation in the same transaction. Extend the existing event-type CHECK by a new migration; keep the event table append-only.
- Set `task_run_event.dedupe_tag = execution_event_sequence` as canonical base-10 text; store `event_id`, `event_type`, `execution_event_sequence`, `binding_epoch`, payload hash and evidence references in `detail`. This matches the DDL CHECK (`detail.execution_event_sequence = dedupe_tag`) and partial unique index `(run_id, dedupe_tag)`, enforcing one Run observation per contiguous sequence. Event-ID idempotency remains enforced by `integration.inbound_event.id` and `idempotency_key`; do not overload `dedupe_tag` with event_id. Preserve the existing AI-PM `lease_generation` field unchanged; do not overload it with SEM `binding_epoch` because they are different fencing domains.

Transaction algorithm:

1. Verify service signature, Content-Digest, freshness and nonce.
2. Validate the strict event schema and `Idempotency-Key == event_id`.
3. Resolve `execution_id` to one delivery attempt and lock the attempt row.
4. Look up the existing Inbox row by source + event ID:
   - same canonical hash and same sequence → return duplicate ACK with current contiguous cursor;
   - same event ID but different body/sequence → `409 IDEMPOTENCY_CONFLICT`.
5. Let `expected = task_run.execution_event_sequence + 1`:
   - received sequence greater than expected → `409 EVENT_SEQUENCE_GAP`, include `expected_sequence`; do not advance or ACK success;
   - received sequence lower than expected but no matching prior event ID → `409 EVENT_SEQUENCE_CONFLICT`; do not mutate;
   - exact expected sequence → insert Inbox record, append Run observation, update cursor.
6. Commit all three writes atomically.
7. Only after commit return `200` with `event_id`, `execution_id`, `execution_event_sequence`, `contiguous_through_sequence`, `disposition` and `acknowledged_at`. The response field name MUST remain `execution_event_sequence`; the ambiguous alias `event_sequence` is not accepted because it could be confused with Runner journal `event_sequence` or WSS transport `message_sequence`.

The current generic `submit_event()` transaction is not sufficient because it creates an AI analysis job and does not enforce this cursor algorithm.

### 5.3 Alternative only if shared Inbox reuse fails review

Create `integration.sem_event_inbox` only if the shared Inbox cannot meet the SEM-specific uniqueness/retention contract. It would contain:
- primary key `event_id`;
- `tenant_id`, `project_id`, `task_run_id/execution_id`;
- `execution_event_sequence`, `event_type`, `schema_version`;
- canonical `payload` and `payload_hash`;
- `received_at`, `applied_at`, `disposition`.

Required uniqueness:
- `UNIQUE(event_id)`;
- `UNIQUE(execution_id, execution_event_sequence)`.

The same row-lock/cursor/transaction/ACK algorithm applies. This alternative avoids generic ingestion side effects but adds a dedicated table. Choose one model; do not dual-write both inboxes.

## 6. Event interpretation and business authority

SEM events are infrastructure facts. The callback may record them and update an AI-PM-owned infrastructure-observation projection, but it MUST NOT:

- change `TaskStatus`;
- mark a delivery `Submitted` merely because a process exited;
- accept a delivery or mark a Task `Completed`;
- approve a review, acceptance criterion or formal human decision;
- create a new Attempt or change `attempt_no`/supersession lineage;
- interpret exit code 0 as business success.

Permitted facts include binding established, delivery receipt, Harness start/start failure, exact process exit evidence, release status and infrastructure uncertainty. Any Run status transition must go through a separately defined AI-PM application-service transition with its own guards and audit. The existing `pm_submit_result` remains the business delivery-submission path.

## 7. Response and retry contract

| HTTP | Meaning | SEM behavior |
|---|---|---|
| 200 | Event durably accepted or exact duplicate; response includes contiguous cursor | Mark event acknowledged; continue with next sequence |
| 400 | Malformed envelope/header mismatch | Permanent failure; operator/configuration correction |
| 401/403 | Signature, key, service permission or freshness failure | Do not retry blindly; alert security/operator |
| 404 | Unknown execution / wrong tenant / non-delivery Run | Permanent until reconciled by AI-PM |
| 409 `IDEMPOTENCY_CONFLICT` | Same event ID, different canonical body | Quarantine event; security/operator investigation |
| 409 `EVENT_SEQUENCE_GAP` | Event sequence is ahead of expected | Retry/replay from expected sequence; no cursor advancement |
| 409 `EVENT_SEQUENCE_CONFLICT` | Sequence already occupied by another event ID | Quarantine and reconcile; never skip |
| 422 | Valid JSON but unsupported event/version or semantic contradiction | Permanent until compatible contract or corrected source |
| 429/5xx | Temporary capacity or service failure | Bounded exponential retry with jitter; honor Retry-After |

SEM retains the durable event until it receives a valid success ACK. A callback timeout is uncertain; retry the same `event_id` and exact body. AI-PM must be idempotent. Dead-letter recovery is an operator-visible replay, not silent event loss.

## 8. Required acceptance tests before implementation

1. Valid signed event creates one Inbox row, one Run observation and advances cursor in one transaction.
2. Crash before commit leaves no partial writes and no success ACK.
3. Crash after commit but before ACK; retry returns duplicate with the same event ID and current contiguous cursor.
4. Same event ID/same body is duplicate; same event ID/changed body is 409.
5. Same execution sequence/different event ID is 409.
6. Sequence N+1 before N returns 409 with expected N; after N commits, replay N+1 succeeds.
7. Concurrent duplicate delivery serializes and commits once.
8. Cross-project, cross-tenant, unknown and review-run execution IDs are non-enumerating 404.
9. SEM service principal is not converted to HumanActor; ordinary human-only review/approval routes remain inaccessible.
10. SEM event never creates an `ai_analysis` job.
11. Event process-exited/exit-code-zero never sets Task Submitted/Completed.
12. Old Runner binding_epoch cannot mutate a newer binding observation.
13. Duplicate/stale events never roll back or lower the contiguous cursor.
14. 429/5xx retry and dead-letter/replay preserve the exact event body and event ID.

## 9. Open decisions (must be ratified bilaterally)

- OI-01: Confirm `execution_id == delivery.task_run.id` for one Attempt.
- OI-02: Ratify reuse of generic `integration.inbound_event` through a SEM-specific application port (without `ai_analysis` job), with `task_run.execution_event_sequence` and append-only `sem_infrastructure_observed` Run events. Recommendation: reuse the existing shared Inbox and add no parallel table. Source audit confirms `integration.inbound_event.id` is already a global primary key; the missing invariant is uniqueness of `(execution_id, execution_event_sequence)`, not global event-ID uniqueness. See [Exact DDL and Concurrency Design](sem-callback-inbox-ddl-and-concurrency-design.md).
- OI-03: Ratify the exact `task_run.execution_event_sequence` migration, `sem_infrastructure_observed` event CHECK extension, and the `dedupe_tag=event_id` / `detail` payload mapping.
- OI-04: Service principal/key registry and RFC 9421 verification middleware.
- OI-05: Event retention, cursor compaction and operator replay policy.
- OI-06: Final callback response schema and retry classification must match SEM D-04 OpenAPI/JSON Schema exactly.

**Disposition: DRAFT / CHANGES REQUIRED.** This document is a bilateral design proposal only. No route, application code, migration, runtime configuration or database change is authorized by it.


## 10. DDL and concurrency design follow-up (2026-11-25)

The exact DDL candidate, source-level findings, RFC 8785 JCS hash rule, callback transaction/lock order, and 24-case two-connection PostgreSQL acceptance matrix are now documented in [sem-callback-inbox-ddl-and-concurrency-design.md](sem-callback-inbox-ddl-and-concurrency-design.md).

This is a design proposal only. OI-01..OI-06 remain open; no migration, route, application code, or runtime change is authorized.


### OI-06 — Current SEM binding projection and first-bind/rebind rules (P0, OPEN)

The exact DDL/concurrency companion identifies a blocking gap: its transaction algorithm validates against a “current SEM infrastructure binding tuple”, but the proposed storage only adds the per-attempt event cursor and does not define a current-binding projection or deterministic event-history query. The first `execution.binding_established` event also has no prior binding to compare. Before implementation, specify an atomic consumer-side binding projection or a deterministic replay algorithm, including first bind, duplicate, same-epoch conflict, stale lower epoch, authorized higher-epoch rebind, host-isolation disposition, and rejection while prior state remains uncertain. See §10 of the exact DDL/concurrency design and the SEM D-04 cross-repository review. This remains P0 / Changes Required; no implementation is authorized.


## 11. Proposed binding projection and callback validation (P0-06; superseded by the canonical §14 of the exact DDL design)

**Update 2026-11-28:** the projection design has converged on exactly ONE table, `delivery.task_run_sem_execution` (see the exact DDL/concurrency design §14). The earlier `delivery.task_run_sem_binding` name is superseded and MUST NOT be created; no duplicate binding table and no dual-write projection is permitted. The projection holds dispatch_state, the current binding tuple, binding_epoch, runner identity, host boot identity, launch/containment identity, release state and uncertainty/quarantine state in nullable columns with exhaustive per-state CHECK constraints.

The projection stores only the last SEM binding accepted by AI-PM and its lifecycle/evidence references. It is not an authority to dispatch, select a Runner, allocate epochs, or change Task/Run business state. The callback transaction locks the delivery Run first, then the projection row, then the Inbox event, then the Run observation, then advances the per-attempt cursor with a compare-and-set; commit all changes atomically. The lock order is frozen in the exact DDL design §13.

The first `execution.binding_established` creates the projection through the first-bind algorithm (epoch must be 1; complete Runner tuple required; no historical binding compared). Exact duplicates are resolved by Inbox identity and do not rewrite it. Same-epoch identity changes and lower epochs reject. A higher epoch is accepted only after a positive prior release/no-start proof or an explicit SEM-authorized host-isolation disposition. An uncertain prior binding blocks rebind. SEM self-asserted host state is never AI-PM rebind authorization. The ten normative handling cases are in the exact DDL design §14.2 and SEM protocol §7.2.

**Host-isolation dependency status:** unchanged in kind — the D-03 `runner.quarantine.resolve` operator disposition must reach AI-PM as an explicit SEM-authenticated reference (`host_isolation_disposition_id` in the canonical projection); `execution.infrastructure_uncertain` is not such proof. The projection column and its CHECK constraint are now specified; the SEM event that carries the disposition reference remains a D-03/protocol ratification item and is NOT yet part of the six-event callback schema. BIND-01..BIND-14 are in the PostgreSQL acceptance plan; BIND-01..12/13/14 core semantics have real-PostgreSQL harness coverage (see the exact DDL design §15).

### OI-07 — Unbound release and dispatch fence (P0-07; contract artifacts complete, runtime OPEN)

The SEM protocol §7.1 contract and its machine-readable artifacts now exist on `smark-sem` (`docs/d05-linux-proof-contract`, PR #5, includes PR #3): a strict tagged `expected_binding` (`unbound` forbids `binding_epoch`; `bound` requires integer ≥ 1; the legacy `expected_binding_epoch` alias is schema-rejected), one per-execution dispatch/release sequencer, a permanent SEM dispatch-fence tombstone, and the sixth callback event `execution.unbound_release_applied` with a strict schema branch whose payload forbids every Runner/binding/stop-fence/process-exit field. AI-PM records that event in `delivery.task_run_sem_execution` as `dispatch_state='released_unbound'` with a non-null `dispatch_fence_id` and NULL binding tuple; any later binding event for the same execution_id is rejected regardless of epoch; a new AI-PM attempt uses a new execution_id. Release acceptance responses carry `release_disposition` (`bound_release_accepted` / `unbound_release_fenced`); dispatch-wins against an unbound release is `409 BINDING_ALREADY_ESTABLISHED` echoing the actual epoch. Real-PostgreSQL harness coverage: INBOX-50..54/56/57 and UNBOUND-03/04/08/09/10 (exact DDL design §15). Runtime closure remains OPEN: the SEM dispatch/release sequencer service, its persistence and failure-injection tests do not exist. Do not accept a fabricated epoch 0 or a lookalike release_status event without Runner identity. No implementation is authorized.
