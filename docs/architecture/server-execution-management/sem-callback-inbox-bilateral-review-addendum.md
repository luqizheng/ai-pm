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
- a partial unique index guaranteeing global uniqueness of SEM `event_id` across projects while preserving non-SEM source semantics;
- `task_run.sem_last_event_sequence` with nonnegative constraint;
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
4. Global event_id uniqueness across two project IDs.
5. Concurrent same-event and same-sequence deliveries.
6. Hash canonicalization agreement between SEM and AI-PM.
7. Event binding_epoch / runner identity mismatch is rejected or quarantined without advancing cursor.
8. Event replay after commit-before-ACK returns the same durable disposition.
9. Callback response signature is verified by SEM before marking event acknowledged.

## 4. Decision status

- OI-01 execution_id mapping: **open**.
- OI-02 shared Inbox reuse: **recommended, open pending DDL/source proof**.
- OI-03 cursor + Run event migration details: **open**.
- OI-04 service principal and signature key registry: **open**.
- OI-05 retention/compaction/operator replay: **open**.
- OI-06 exact callback response/error contract: **open**.

No AI-PM source code, route, migration, database schema, runtime configuration or deployment was changed by this addendum.
