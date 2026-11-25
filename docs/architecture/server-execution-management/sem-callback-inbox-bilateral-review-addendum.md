# SEM Callback Inbox — Bilateral Review Addendum

- Status: **DRAFT / REVIEW REQUIRED**
- Related AI-PM PR: #9, branch `docs/sem-02e-architecture-convergence`
- Related SEM PR: #3, branch `docs/sem-d05-lifecycle-crash-consistency`
- Review date: 2026-11-25
- No implementation or migration is authorized.

## 1. Review result

The proposed direction is accepted for continued design: a dedicated service-authenticated callback route, a SEM-specific application port, reuse of `integration.inbound_event` only if its constraints are extended safely, a per-attempt contiguous event cursor, and one transaction for Inbox deduplication + Run observation + cursor advance.

The callback must not use the existing human-authenticated `POST /api/v1/events`, must not invoke generic `submit_event()` unchanged, must not enqueue `ai_analysis`, and must not synthesize a HumanActor.

The proposal remains Draft. OI-01..OI-06 are not ratified.

## 2. Cross-repository contract alignment items

### 2.1 ACK sequence name

The SEM incoming event field is `execution_event_sequence`; the AI-PM cursor is also named `execution_event_sequence`. The current SEM ACK schema instead calls the echoed event value `event_sequence`.

Preferred correction: use `execution_event_sequence` in the ACK as well. If the shorter field is retained, define it normatively as the exact echo of the incoming `execution_event_sequence`, and explicitly distinguish it from Runner journal `event_sequence` and WSS `message_sequence`.

The ACK must also return `contiguous_through_sequence`, `disposition` and `acknowledged_at`, and must only be sent after the transaction commits.

### 2.2 Exact storage contract is not yet frozen

Before approving shared Inbox reuse, provide exact DDL for:
- confirm the existing `integration.inbound_event.id` global primary key is used directly as SEM `event_id` (no additional global unique index is needed);
- `delivery.task_run.execution_event_sequence` cursor with nonnegative constraint;
- any Run-event CHECK/constraint extension and event detail representation;
- FK/tenant/project/service-principal integrity;
- any uniqueness needed to prevent two event IDs occupying one `(execution_id, execution_event_sequence)`.

Specify whether payload hash is over exact received bytes or canonicalized JSON. Both sides must use one exact rule.

### 2.3 Transaction and replay proof

Add deterministic PostgreSQL two-connection tests for concurrent duplicates and sequence collisions. The transaction must atomically:
1. insert/deduplicate the SEM Inbox event;
2. append one `sem_infrastructure_observed` Run event;
3. advance the attempt cursor.

Crash before commit must leave no partial writes and no success ACK. Crash after commit before ACK must be recoverable by replaying the exact event_id/body and returning a duplicate ACK.

### 2.4 Semantic evidence validation

JSON Schema shape validation is not enough to validate D-05 identity. Before accepting a callback, the application service must validate the event against the current attempt and known infrastructure binding, including runner_id, runner key version, boot_id, supervisor instance, binding_epoch, launch/containment identity and evidence references as applicable.

In particular:
- process exit evidence must be tied to the exact process/containment identity;
- release state `process_exited` must carry verifiable process-exit evidence;
- stale binding epochs must not overwrite newer observations;
- exit_code 0 is infrastructure evidence only and never means business success.

### 2.5 Error/ACK alignment

Freeze exact response bodies and retry classes for:
- `IDEMPOTENCY_CONFLICT`;
- `EVENT_SEQUENCE_GAP` (including `expected_sequence`);
- `EVENT_SEQUENCE_CONFLICT`;
- malformed/signature/freshness failures;
- unknown execution / wrong tenant / wrong run kind;
- transient 429/5xx.

The OpenAPI, SEM JSON Schema, AI-PM application port and SEM retry logic must use the same field names and status-code semantics.

## 3. Required acceptance cases

In addition to the 14 cases in the main draft, add explicit tests for:
1. ACK echoes the exact incoming execution_event_sequence.
2. Same event_id with changed sequence is a conflict.
3. Same sequence with different event_id is a conflict.
4. Global `event_id` primary-key collision across two project IDs, with non-enumerating conflict handling.
5. Concurrent same-event and same-sequence deliveries.
6. Hash canonicalization agreement between SEM and AI-PM.
7. Event binding_epoch / runner identity mismatch is rejected or quarantined without advancing cursor.
8. Event replay after commit-before-ACK returns the same durable disposition.
9. Callback response signature is verified by SEM before marking event acknowledged.

## 4. Decision status

- OI-01 execution_id mapping: **open**.
- OI-02 shared Inbox reuse: **recommended; source audit confirms global PK and identifies required sequence metadata/constraints; live catalog proof remains open**.
- OI-03 cursor + Run event migration details: **open**.
- OI-04 service principal and signature key registry: **open**.
- OI-05 retention/compaction/operator replay: **open**.
- OI-06 exact callback response/error contract: **open**.

No AI-PM source code, route, migration, database schema, runtime configuration or deployment was changed by this addendum.


## 5. SEM-side contract update — proposed bilateral alignment

The current SEM contract draft has been updated to use `execution_event_sequence` in the callback success acknowledgement, matching the incoming event envelope and the AI-PM per-attempt cursor. The legacy ambiguous ACK field `event_sequence` is rejected by the SEM positive/negative fixture validator.

The expected ACK shape is:

```json
{
  "event_id": "uuid",
  "execution_id": "uuid",
  "execution_event_sequence": 12,
  "contiguous_through_sequence": 12,
  "disposition": "accepted",
  "acknowledged_at": "RFC3339 UTC"
}
```

AI-PM's response contract should echo the accepted event's exact `execution_event_sequence`; this field is not the Runner journal sequence or WSS connection-scoped message sequence. A duplicate ACK echoes the same sequence and uses `disposition=duplicate`. The response remains forbidden until the Inbox + Run observation + cursor transaction commits.

SEM has also strengthened the event JSON Schema to require runner key version, boot ID and Supervisor instance identity on delivery/process/release/uncertainty observations. The AI-PM receiver must still compare these against the currently bound attempt; JSON Schema validity does not replace semantic binding validation. A release observation of `process_exited` requires a process-exit evidence reference, and an exited process observation requires exact process identity and exactly one exit status discriminator (`exit_code` or `exit_signal`).

These are changes to the SEM Draft contract, not ratified bilateral decisions. AI-PM PR #9 must independently adopt the same field names and semantics before interoperability can be claimed.


## 6. AI-PM source/DDL audit — 2026-11-25

A source-level audit was performed against PR #9 base commit `d0e72fa6805aebeadf1b282e430deba493072fd6`, including the squashed baseline DDL, `pm-application/src/ingestion.rs`, and `migrations/0013_task_run_event.sql`.

Confirmed findings:

1. `integration.inbound_event.id` is already a global UUID primary key. Map SEM `event_id` directly to it; a second global unique index is redundant.
2. Existing `UNIQUE(project_id, source, idempotency_key)` does not enforce uniqueness of `(execution_id, execution_event_sequence)`.
3. Existing generic `submit_event()` inserts Inbox + `integration.job(job_type='ai_analysis')` atomically and hashes a Rust JSON serialization. It must not be reused for SEM.
4. The current Inbox requires `actor_id` and has a principal FK. SEM must use a service principal through a dedicated service-authenticated port; it must not synthesize HumanActor.
5. `delivery.task_run` has `UNIQUE(tenant_id,id)`, `UNIQUE(project_id,id)`, and `run_kind` distinguishes delivery/review.
6. `delivery.task_run_event` is append-only but its current event-type CHECK does not include `sem_infrastructure_observed`; its current unique index is not a direct execution-sequence uniqueness constraint.

The detailed candidate DDL, JCS payload hash rule, transaction/lock order, and 24-case two-connection PostgreSQL acceptance matrix are recorded in [sem-callback-inbox-ddl-and-concurrency-design.md](sem-callback-inbox-ddl-and-concurrency-design.md).

Important correction: global event-ID uniqueness is already provided by the Inbox primary key. The missing database invariant is sequence uniqueness per execution, plus the cursor and observation-ledger constraints. The proposed DDL still requires live PostgreSQL catalog capture, empty-database migration replay, and bilateral review before ratification.

No implementation or migration is authorized. OI-01..OI-06 remain open.
