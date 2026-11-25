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

The detailed candidate DDL, JCS payload hash rule, transaction/lock order, and 31-case two-connection PostgreSQL acceptance matrix are recorded in [sem-callback-inbox-ddl-and-concurrency-design.md](sem-callback-inbox-ddl-and-concurrency-design.md).

Important correction: global event-ID uniqueness is already provided by the Inbox primary key. The missing database invariant is sequence uniqueness per execution, plus the cursor and observation-ledger constraints. The proposed DDL still requires live PostgreSQL catalog capture, empty-database migration replay, and bilateral review before ratification.

No implementation or migration is authorized. OI-01..OI-06 remain open.

## 7. DDL re-review — existing Run-event CHECK must be replaced (2026-11-25)

Rechecking the exact current `migrations/0013_task_run_event.sql` reveals a required DDL operation missing from the candidate block: the existing named constraint `task_run_event_type_check` allows exactly eight event types and excludes `sem_infrastructure_observed`.

Adding `task_run_event_sem_sequence_check` does not extend that allow-list. PostgreSQL will still reject the new observation event under the original CHECK. The migration must explicitly drop and recreate `task_run_event_type_check` with the original eight values plus `sem_infrastructure_observed`; the exact forward-only SQL is now included in the DDL design document.

Acceptance must prove all original eight event types remain accepted, the new SEM event type is accepted, and an unknown type remains rejected. This is a source-verified migration correctness blocker, not optional hardening.

No migration or runtime source was changed. The DDL remains Draft; live PostgreSQL replay and two-connection tests are still required.

## 8. Replay identity must cover the complete event envelope (2026-11-25)

A follow-up review found that the earlier duplicate rule compared `execution_id`, `execution_event_sequence` and the JCS hash of `payload` only. That is insufficient: an event replay could retain the same payload while changing `event_type`, `schema_version` or `occurred_at`, and be incorrectly acknowledged as a duplicate.

The candidate DDL now preserves the existing payload-level `payload_hash` and adds a SEM-only `event_envelope_hash`. The latter is SHA-256 over RFC 8785 JCS UTF-8 bytes of the complete authenticated event envelope: every schema-defined envelope field that is present: `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. Unknown fields reject; absent optional fields are omitted. It excludes HTTP signature headers and transport metadata. Duplicate detection compares the envelope hash plus the stored event type and execution/sequence tuple.

The SEM-specific CHECK requires a lowercase 64-character hex hash for `source='sem'` and NULL for non-SEM events. INBOX-29..31 cover changed event_type, changed schema_version/occurred_at, and equivalent JCS envelopes with different whitespace/member ordering.

This closes the identified design-level equality ambiguity only. It does not prove the Rust JCS implementation, cross-language hash interoperability, database migration correctness or PostgreSQL concurrency behavior. DDL remains Draft / Changes Required; no migration or runtime code was changed.


## 9. Callback JCS implementation and cross-language vector requirements

The callback hash contract is not closed merely by selecting RFC 8785. The Rust producer/consumer implementation must be pinned and proven against an independent implementation using the same complete callback envelope.

The DDL design now explicitly requires:
- A pinned Rust JCS implementation/version; generic `serde_json::Value::to_string()` is not an acceptable canonicalizer.
- RFC 8785 ECMAScript-compatible IEEE-754 number serialization and UTF-16 code-unit object-property ordering.
- Duplicate JSON object member rejection before `serde_json::Value` materialization, including escaped-equivalent names and nested objects.
- Shared callback envelope vectors proving exact JCS UTF-8 bytes, payload_hash and event_envelope_hash agreement between Rust and an independent Node/Python implementation.
- Numeric boundary, negative-zero, Unicode/UTF-16 ordering, unpaired-surrogate and malformed-input negative vectors.

Added INBOX-32..34. The expanded PostgreSQL acceptance matrix now has 39 cases. These are requirements only: no canonicalizer dependency, production parser, migration or runtime implementation was added. OI-03 and the hash interoperability gate remain open.


The same DDL update also fixes the SEM mapping for the existing Inbox idempotency key: `idempotency_key` is the canonical lowercase hyphenated UUID string of `event_id`. The global Inbox primary key remains the cross-project event identity constraint; the partial execution/sequence index remains the sequence constraint. The SEM CHECK now requires both payload_hash and event_envelope_hash to be lowercase 64-character SHA-256 hex strings.


## 10. Shared callback-envelope vector pinned

SEM now provides a shared callback fixture at `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`, pinned to `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`. It contains one valid `execution.infrastructure_uncertain` event, the payload and full-envelope RFC 8785 canonical UTF-8 bytes, and both SHA-256 hashes.

The SEM Python and Node validators independently verify exact bytes and hashes. Added INBOX-35: AI-PM's eventual Rust implementation must pass the exact same pinned vector, not a locally recreated approximation. This vector uses ASCII strings and integer values by design; the wider RFC 8785 Unicode/numeric boundary requirements in INBOX-33 remain separate.

The shared vector closes one deterministic cross-language fixture gap, but not the production Rust implementation, dependency selection, raw HTTP duplicate-member rejection, PostgreSQL DDL/concurrency tests or bilateral live callback integration. No runtime code or migration was changed.


## 11. PostgreSQL CHECK NULL semantics and SQL grouping correction

The latest review identified two defects in the illustrative SEM Inbox CHECK: regex predicates without explicit `IS NOT NULL` guards can evaluate to UNKNOWN (which PostgreSQL CHECK accepts), and the SEM/non-SEM branches were not correctly grouped in the draft expression.

The DDL design now explicitly requires non-NULL `payload_hash` and `event_envelope_hash` before validating their lowercase 64-character SHA-256 format, closes the SEM branch before the non-SEM OR branch, and retains the source NULL guard. Added INBOX-36..38 for each missing hash and the complete truth-table/parse behavior.

The corrected text is still not a validated migration. PostgreSQL must execute the expression in an empty-database replay and test TRUE/FALSE/UNKNOWN cases before this contract can be accepted. The acceptance matrix now contains 39 cases.


## 12. Optional correlation_id is part of authenticated envelope identity

A schema re-read found that Platform–SEM `EventBase` permits optional top-level `correlation_id`. The earlier full-envelope hash field list omitted it. That would have allowed a callback to be replayed with a changed correlation_id while retaining the same envelope hash.

The contract is corrected: JCS covers every schema-defined envelope field that is present, including `correlation_id` when present. Absent optional fields are omitted; unknown fields are rejected by schema validation before hashing. The shared vector now includes `correlation_id` and pins to `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`. Python and Node assert that changing correlation_id or omitting a previously present optional field changes the full-envelope hash while leaving the payload hash unchanged.

The latest design correction is documented in AI-PM PR #9. This is a deterministic fixture improvement; Rust implementation and live callback integration remain open.


INBOX-39 now explicitly covers optional-field omission: when correlation_id is present in one envelope and absent in another, the payload hash remains equal but the complete-envelope hash differs. The acceptance matrix now contains 39 cases.


## 13. Explicit PostgreSQL CHECK truth table

The DDL design now lists the required PostgreSQL execution cases for the SEM source CHECK: valid SEM hashes accept; NULL execution ID/sequence, NULL or malformed payload hash, NULL or malformed envelope hash reject; valid non-SEM rows retain NULL execution tuple/envelope hash; non-SEM rows with SEM-only fields reject; NULL source is rejected by the existing column NOT NULL constraint before this CHECK. The test must execute these rows in PostgreSQL and assert the named constraint, not merely duplicate the Boolean expression in application code.

This makes INBOX-36..38 executable acceptance requirements. The candidate expression has been textually repaired but still has no PostgreSQL replay/catalog proof.


## 14. Signed callback ACK correlation identity

The callback ACK response must carry exactly one signed `X-SEM-Event-Id` header equal to the authenticated callback `event_id`. This is distinct from the Platform→SEM response direction, where `X-SEM-Request-Id` echoes `request_id` (the operation/idempotency identity), not `message_id`. The SEM deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395` includes both `http_callback_ack` and `http_error_409`; it binds the exact shared callback envelope bytes to a signed 200 ACK, includes a signed Platform→SEM 409 structured conflict, and Python/Node reject mismatched event/request correlation. AI-PM's callback response contract has been aligned.


The signed ACK verifier must also bind the response to the exact originating request context (method, authority, path and request Content-Digest), not merely validate the response signature and event correlation header. SEM's Python/Node fixtures mutate each request-derived component and prove signature verification fails; AI-PM's future Rust/client verifier must preserve the same request context through response verification.
