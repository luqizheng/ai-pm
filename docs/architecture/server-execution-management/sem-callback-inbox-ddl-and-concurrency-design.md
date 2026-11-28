# SEM Callback Inbox — Exact DDL and Concurrency Design

- Status: **DRAFT / REVIEW REQUIRED**
- AI-PM PR: #9, branch `docs/sem-02e-architecture-convergence`
- Scope: design only; no migration, route, application code, or runtime change is authorized.

## 1. Source audit findings

The current baseline establishes:

1. \`integration.inbound_event.id\` is already a global UUID primary key. Mapping SEM \`event_id\` directly to this column provides global uniqueness across projects and sources. A second global unique index would be redundant.
2. Existing idempotency is \`UNIQUE(project_id, source, idempotency_key)\`; it does not enforce one event per SEM execution sequence.
3. \`integration.inbound_event\` requires \`actor_id\` referencing \`core.principal(id)\`. SEM must use a service principal and must never synthesize a HumanActor.
4. Existing \`pm_application::ingestion::submit_event\` hashes \`serde_json::Value::to_string()\`, inserts through the generic project/source/idempotency key, and creates \`integration.job(job_type='ai_analysis')\` in the same transaction. It is not a valid SEM callback path.
5. \`delivery.task_run\` has global PK \`id\`, unique \`(tenant_id,id)\` and \`(project_id,id)\`, and \`run_kind\` values \`delivery|review\`. The callback must accept only the intended delivery attempt.
6. \`delivery.task_run_event\` is append-only, but its existing CHECK excludes \`sem_infrastructure_observed\`. Its existing unique key is \`(run_id,event_type,lease_generation,dedupe_tag)\`; it has no database FK to Run.

The current Inbox constraints are: PK(id), UNIQUE(project_id,id), UNIQUE(tenant_id,id), UNIQUE(project_id,source,idempotency_key), composite FK (tenant_id,project_id) to core.project, and actor_id FK to core.principal.

## 2. Recommended storage mapping

| SEM concept | AI-PM storage |
|---|---|
| event_id | integration.inbound_event.id (existing global PK) |
| idempotency_key | Canonical lowercase hyphenated UUID text of the same event_id; do not use execution sequence or payload hash |
| execution_id | delivery.task_run.id |
| execution_event_sequence | new integration.inbound_event.execution_event_sequence |
| payload | integration.inbound_event.payload JSONB |
| payload_hash | integration.inbound_event.payload_hash, SHA-256 of the payload object's RFC 8785 JCS UTF-8 bytes |
| event_envelope_hash | new integration.inbound_event.event_envelope_hash, SHA-256 of the complete authenticated event envelope's RFC 8785 JCS UTF-8 bytes |
| infrastructure observation | delivery.task_run_event, event_type sem_infrastructure_observed |
| contiguous cursor | new delivery.task_run.execution_event_sequence, initial value 0 |

Reuse the existing Inbox; do not create a parallel SEM Inbox unless live catalog evidence reveals a concrete blocker. The SEM-specific application port must not call \`submit_event()\`, create \`integration.job\`, or enqueue AI analysis.

### 2.1 Required SEM Inbox row mapping

Every first-time SEM callback insert uses these exact values:

| Column | Value |
|---|---|
| `id` | incoming `event_id` UUID |
| `project_id`, `tenant_id` | resolved from the locked delivery Run, never trusted from an unvalidated payload |
| `actor_id` | configured SEM service principal's `core.principal.id`; never a HumanActor |
| `source` | literal `sem` |
| `event_type` | authenticated envelope's `event_type` |
| `idempotency_key` | `event_id` serialized as canonical lowercase hyphenated UUID text |
| `payload` | authenticated envelope's `payload` JSON value |
| `payload_hash` | JCS SHA-256 of that payload value |
| `event_envelope_hash` | JCS SHA-256 of the complete authenticated event envelope |
| `status` | existing default unless the application contract explicitly freezes another initial value |
| `execution_id`, `execution_event_sequence` | exact authenticated envelope values |

`schema_version`, `occurred_at` and optional `correlation_id` (when present) are covered by `event_envelope_hash` even though they are not separate Inbox columns. The Run-derived tenant/project identity is authoritative for the FK scope. All hashes are computed and validated before opening the transaction; the transaction still revalidates the locked Run and binding tuple before insertion.


For SEM rows, set `integration.inbound_event.id = event_id` and `idempotency_key = event_id` serialized as the canonical lowercase hyphenated UUID string. This keeps the existing `(project_id, source, idempotency_key)` uniqueness aligned with event identity, while the global primary key remains the cross-project uniqueness authority and the new partial `(execution_id, execution_event_sequence)` index remains the sequence uniqueness authority. Do not use sequence or payload hash as the idempotency key.

## 3. Proposed DDL (illustrative only)

\`\`\`sql
ALTER TABLE delivery.task_run
    ADD COLUMN execution_event_sequence bigint NOT NULL DEFAULT 0,
    ADD CONSTRAINT task_run_execution_event_sequence_nonnegative
        CHECK (execution_event_sequence >= 0);

ALTER TABLE integration.inbound_event
    ADD COLUMN execution_id uuid,
    ADD COLUMN execution_event_sequence bigint,
    ADD COLUMN event_envelope_hash text;

ALTER TABLE integration.inbound_event
    ADD CONSTRAINT inbound_event_sem_sequence_pair
    CHECK (
        source IS NOT NULL
        AND (
            (
                source = 'sem'
                AND execution_id IS NOT NULL
                AND execution_event_sequence IS NOT NULL
                AND execution_event_sequence >= 1
                AND payload_hash IS NOT NULL
                AND payload_hash ~ '^[0-9a-f]{64}$'
                AND event_envelope_hash IS NOT NULL
                AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            )
            OR
            (
                source <> 'sem'
                AND execution_id IS NULL
                AND execution_event_sequence IS NULL
                AND event_envelope_hash IS NULL
            )
        )
    );

CREATE UNIQUE INDEX uq_inbound_event_sem_execution_sequence
    ON integration.inbound_event (execution_id, execution_event_sequence)
    WHERE source = 'sem';

ALTER TABLE delivery.task_run
    ADD CONSTRAINT uq_task_run_tenant_project_execution
    UNIQUE (tenant_id, project_id, id);

ALTER TABLE integration.inbound_event
    ADD CONSTRAINT fk_inbound_event_sem_execution
    FOREIGN KEY (tenant_id, project_id, execution_id)
    REFERENCES delivery.task_run (tenant_id, project_id, id);

ALTER TABLE delivery.task_run_event
    ADD CONSTRAINT task_run_event_sem_sequence_check
    CHECK (
        (
            event_type = 'sem_infrastructure_observed'
            AND dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
            AND detail IS NOT NULL
            AND jsonb_typeof(detail) = 'object'
            AND detail ? 'event_id'
            AND detail ? 'execution_event_sequence'
            AND detail ? 'sem_event_type'
            AND detail ? 'event_envelope_hash'
            AND detail ? 'schema_version'
            AND COALESCE(detail->>'schema_version', '') ~ '^[1-9][0-9]*$'
            AND detail ? 'occurred_at'
            AND COALESCE(detail->>'event_id', '') ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
            AND COALESCE(detail->>'sem_event_type', '') <> ''
            AND COALESCE(detail->>'execution_event_sequence', '') ~ '^[1-9][0-9]*$'
            AND detail->>'execution_event_sequence' = dedupe_tag
            AND COALESCE(detail->>'occurred_at', '') <> ''
            AND COALESCE(detail->>'event_envelope_hash', '') ~ '^[0-9a-f]{64}$'
            AND (
                (
                    COALESCE(detail->>'sem_event_type', '') = 'execution.unbound_release_applied'
                    AND COALESCE(detail->>'binding_epoch', '') = ''
                )
                OR COALESCE(detail->>'binding_epoch', '') ~ '^[1-9][0-9]*$'
            )
        )
        OR event_type <> 'sem_infrastructure_observed'
    );

CREATE UNIQUE INDEX uq_task_run_event_sem_sequence
    ON delivery.task_run_event (run_id, dedupe_tag)
    WHERE event_type = 'sem_infrastructure_observed';
\`\`\`

Important required event-type CHECK replacement (omitted from the initial illustrative block above):

The current `migrations/0013_task_run_event.sql` defines the named constraint `task_run_event_type_check` with exactly eight event types. Merely adding the separate `task_run_event_sem_sequence_check` does **not** permit the new event type; the old CHECK would continue rejecting every `sem_infrastructure_observed` insert. The migration must replace the existing allow-list in the same forward-only migration:

```sql
ALTER TABLE delivery.task_run_event
    DROP CONSTRAINT task_run_event_type_check;

ALTER TABLE delivery.task_run_event
    ADD CONSTRAINT task_run_event_type_check
    CHECK (event_type IN (
        'run_claimed',
        'run_started',
        'run_stage_changed',
        'run_finished',
        'command_issued',
        'command_delivered',
        'command_acked',
        'command_failed',
        'sem_infrastructure_observed'
    ));
```

This is a source-verified DDL dependency, not an optional hardening step. Migration acceptance must prove the original eight event types remain accepted and unknown event types remain rejected after the replacement.

DDL qualifications:

- Confirm no existing Inbox rows use reserved source value `sem` before applying the source-specific CHECK/index.
- The composite FK `(tenant_id, project_id, execution_id)` proves the exact Run scope. `run_kind='delivery'` remains a locked-row semantic check; reject review Runs before any write.
- Draft decision: no additional `source_event_id` column/FK on `task_run_event`; use atomic transaction linkage plus detail CHECK and partial unique index as described in §5/§9.
- Do not add a redundant global unique index on `inbound_event.id`; the primary key already supplies it.

## 4. Payload hash and equality

Freeze the same rule in both repositories:

1. Reject duplicate JSON object member names before application processing.
2. Canonicalize the payload object using RFC 8785 JSON Canonicalization Scheme (JCS); compute lowercase-hex SHA-256 over its exact UTF-8 JCS bytes and store it in the existing payload_hash column.
3. Canonicalize the complete authenticated event envelope using JCS; the hash input includes every schema-defined envelope field that is present: event_id, event_type, schema_version, occurred_at, execution_id, execution_event_sequence, optional correlation_id when present, and payload; absent optional fields are omitted and unknown fields are rejected before hashing. Compute lowercase-hex SHA-256 over those UTF-8 JCS bytes and store it in the new SEM-only event_envelope_hash column. Do not include HTTP headers, Signature-Input, Signature or transport metadata.
4. Store the payload semantic value in JSONB. The event_envelope_hash is an immutable replay identity, not a replacement for payload_hash.
5. Same event_id is a duplicate only when event_type, execution_id, sequence, payload_hash and event_envelope_hash all match. Because event_envelope_hash covers schema_version, occurred_at and every other envelope field, any changed authenticated envelope field is IDEMPOTENCY_CONFLICT.
6. Whitespace and object-member order do not change either JCS digest. Numeric and Unicode behavior follows RFC 8785, not language-specific serializer behavior.
7. Pin the Rust JCS implementation and version in the consumer build. The implementation must use RFC 8785's ECMAScript-compatible IEEE-754 binary64 number serialization and UTF-16 code-unit property ordering; `serde_json::Value::to_string()` is not an acceptable substitute. Reject duplicate member names before materializing a `serde_json::Value`, including escaped-equivalent names and nested objects. Publish one shared SEM callback envelope vector and verify exact JCS bytes, `payload_hash` and `event_envelope_hash` independently in Rust and Node/Python.

Never hash raw HTTP bytes on one side and canonical JSON on the other. HTTP signature canonicalization is a separate contract.

## 5. Transaction algorithm and lock order

Authenticate the SEM service principal and verify the RFC 9421 request signature, Content-Digest, key state, timestamp/expiry and nonce freshness/replay before opening the database transaction. For an in-limit request, verify the exact raw body bytes before JSON parsing; then reject duplicate decoded JSON member names, validate schema and require the signed Idempotency-Key to equal envelope event_id. The callback handler must not begin application writes before these checks.

The transaction MUST run at PostgreSQL READ COMMITTED. A serialization/snapshot conflict at a higher isolation level requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Begin the transaction and lock the Run by the authenticated envelope execution_id:

   SELECT id, tenant_id, project_id, task_id, run_kind, lease_generation, lease_client_id, execution_event_sequence
     FROM delivery.task_run
    WHERE id = :execution_id
    FOR UPDATE;

   Unknown Run or `run_kind <> 'delivery'` returns an authenticated non-enumerating signed 404. The event envelope does not supply tenant/project authority; tenant_id, project_id and task_id are derived from this locked Run row.
2. Validate the current SEM infrastructure binding tuple and event-specific identity against the exact execution binding: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, and process/launch/containment/evidence references where applicable. A mismatch is an authenticated signed conflict/validation error; no Inbox write occurs.
3. Check the global Inbox event_id before sequence ownership. If a row with this id exists:
   - If its source is not `sem`, or its execution_id differs from the incoming execution_id, return 409 IDEMPOTENCY_CONFLICT. This precedence applies even if the incoming execution/sequence is also owned by another row. Do not disclose the stored row's project/tenant.
   - If execution_id is the same but stored tenant_id/project_id disagree with the locked Run-derived scope, rollback and return a generic integrity error; never disclose the stored scope or ACK.
   - Otherwise compare source, event_type, idempotency_key, execution_event_sequence, payload_hash and event_envelope_hash. Any changed value returns 409 IDEMPOTENCY_CONFLICT.
   - If all values match, query `(execution_id, execution_event_sequence)` filtered to `source='sem'` and require it to identify the exact same Inbox row. A missing/different sequence row is a generic integrity failure.
   - Verify exactly one `task_run_event` observation for this Run, `event_type='sem_infrastructure_observed'` and decimal dedupe_tag. Its tenant_id/project_id/task_id must match the locked Run, and detail.event_id/detail.event_envelope_hash must match the Inbox row. The locked Run cursor must be >= the committed event sequence. Do not compare the observation's lease_generation to the Run's current lease_generation; it records the generation at original observation time.
   - Only after all checks return an exact duplicate ACK. Do not insert another observation or move the cursor.
4. If no global event_id row exists, check the unique `(execution_id, execution_event_sequence)` key for `source='sem'`. If another event_id owns it, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = locked Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence; a lower value without the matching committed event is conflict. Never lower the cursor.
5. Insert the Inbox row using all-constraint conflict handling:

   INSERT INTO integration.inbound_event
     (id, tenant_id, project_id, actor_id, source, event_type, idempotency_key,
      payload, payload_hash, event_envelope_hash, execution_id, execution_event_sequence)
   VALUES
     (:event_id, :run_tenant_id, :run_project_id, :sem_service_principal_id,
      'sem', :event_type, :canonical_event_id_text, :payload, :payload_hash,
      :event_envelope_hash, :execution_id, :execution_event_sequence)
   ON CONFLICT DO NOTHING
   RETURNING id;

   `id = event_id`; `idempotency_key` is the same UUID rendered as canonical lowercase hyphenated text. `actor_id` is the configured SEM service principal's `core.principal.id`, never a HumanActor. `tenant_id/project_id` come only from the locked Run. `payload` is the authenticated envelope payload; hashes are computed before the transaction.

   If no row is returned, issue new SELECT statements under READ COMMITTED and apply the same conflict precedence as step 3: (a) non-SEM or different execution_id for the global event_id → 409 IDEMPOTENCY_CONFLICT; (b) same execution_id but Run-scope mismatch → generic integrity error; (c) changed event/envelope identity → 409 IDEMPOTENCY_CONFLICT; (d) exact event_id match must have the sequence lookup resolve to the same row and the observation/cursor invariants hold before duplicate ACK; (e) no event_id row but sequence owner exists → 409 EVENT_SEQUENCE_CONFLICT; (f) neither key exists → rollback and generic integrity failure. Never query inside an aborted transaction and never disclose a foreign row's scope.
6. For a successful first-time Inbox insert, insert exactly one `task_run_event` row from the locked Run:

   INSERT INTO delivery.task_run_event
     (tenant_id, project_id, task_id, run_id, client_id, event_type,
      lease_generation, dedupe_tag, occurred_at, detail)
   SELECT tenant_id, project_id, task_id, id, lease_client_id,
          'sem_infrastructure_observed', lease_generation,
          :incoming_sequence::text, :occurred_at, :detail
     FROM delivery.task_run
    WHERE id = :execution_id
      AND tenant_id = :run_tenant_id
      AND project_id = :run_project_id
   RETURNING id;

   This statement MUST return exactly one row. `client_id` copies the Run's current `lease_client_id` (nullable; do not invent a SEM client). `lease_generation` copies the AI-PM Run lease generation at observation time; it is not SEM `binding_epoch`. `dedupe_tag` is the canonical base-10 sequence. `occurred_at` comes from the authenticated envelope; `received_at` and `created_at` use database defaults. `detail` is a JSON object containing event_id, execution_event_sequence, sem_event_type, schema_version, occurred_at, event_envelope_hash, binding_epoch, runner_id, runner_key_version, boot_id, supervisor_instance_id, launch/containment/process identity where applicable, and evidence references. Do not call generic `record_run_event()` because its default occurred_at and generation/dedupe behavior are not the SEM callback contract.
7. Advance `delivery.task_run.execution_event_sequence` using compare-and-set against the previously locked cursor:

   UPDATE delivery.task_run
      SET execution_event_sequence = :incoming_sequence
    WHERE id = :execution_id
      AND tenant_id = :run_tenant_id
      AND project_id = :run_project_id
      AND execution_event_sequence = :previous_cursor
   RETURNING execution_event_sequence;

   Exactly one row MUST be returned. Zero rows is an invariant failure and rolls back the Inbox insert and observation.
8. Commit. Only after commit return the success ACK. All callback writers use one lock order: Run row → Inbox event → Run observation. Same-execution writes serialize on the Run row; the Inbox sequence unique index and Run-observation partial unique index are database backstops.

## 6. PostgreSQL two-connection acceptance matrix

Run with two independent PostgreSQL connections and explicit synchronization barriers. Mock-only tests are insufficient.

| ID | Scenario | Required result |
|---|---|---|
| INBOX-01 | First event, sequence 1 | One Inbox row, one observation, cursor 1; ACK only after commit |
| INBOX-02 | Same event ID/body replay | Duplicate ACK; no second observation |
| INBOX-03 | Same event ID, changed payload | 409 idempotency conflict; no writes |
| INBOX-04 | Same event ID, changed execution ID or sequence | 409 idempotency conflict; no writes |
| INBOX-05 | Concurrent same sequence, different IDs | Exactly one commits; loser sequence conflict |
| INBOX-06 | Concurrent same event ID/body | Exactly one insert/observation; other returns duplicate |
| INBOX-07 | N+1 before N | 409 gap with expected N; no writes |
| INBOX-08 | Commit N, crash before ACK, replay N | Duplicate ACK; cursor remains N |
| INBOX-09 | Crash before commit | No partial Inbox/event/cursor writes; no ACK |
| INBOX-10 | Same event ID in another project/source | Global PK conflict; no cross-project disclosure |
| INBOX-11 | Wrong tenant/project or unknown execution | Non-enumerating 404; no writes |
| INBOX-12 | Review Run ID | Non-enumerating 404; no writes |
| INBOX-13 | Stale binding/key version/boot ID | Reject or quarantine; cursor unchanged |
| INBOX-14 | Process exit without exact exit evidence | Reject; cursor unchanged |
| INBOX-15 | process_exited release without evidence reference | Reject; cursor unchanged |
| INBOX-16 | SEM service principal | Accepted only on SEM service route; never HumanActor |
| INBOX-17 | Any SEM callback | Zero ai_analysis jobs |
| INBOX-18 | Exit code 0 | Infrastructure observation only; no business success transition |
| INBOX-19 | N and N+1 concurrent | Serialized; cursor N+1 and two ordered observations |
| INBOX-20 | Failure after Inbox insert | Full rollback |
| INBOX-21 | Failure after Run event insert | Full rollback |
| INBOX-22 | Retry after transient DB failure | Exact same event ID/body replayed; no sequence skip |
| INBOX-23 | Generic non-SEM ingestion after migration | Existing semantics unchanged |
| INBOX-24 | Duplicate JSON member or invalid JCS value | Rejected before persistence/cursor movement |
| INBOX-25 | Two different Run rows concurrently submit the same global event_id and identical tuple/body | One insert wins; loser uses ON CONFLICT DO NOTHING then a fresh READ COMMITTED SELECT and returns duplicate only after exact tuple/hash comparison |
| INBOX-26 | Two different Run rows concurrently submit the same global event_id with different execution_id/body | One insert wins; loser returns generic 409 IDEMPOTENCY_CONFLICT without revealing the winner's tenant/project |
| INBOX-27 | Same Run and sequence concurrently submitted with different event_ids | Run-row lock serializes writers; one commits and the other returns 409 EVENT_SEQUENCE_CONFLICT without transaction-abort recovery |
| INBOX-28 | ON CONFLICT DO NOTHING returns no row but neither event_id nor execution/sequence winner can be found | Roll back and return generic integrity failure; never ACK or create a partial observation |
| INBOX-29 | Same event_id and payload but changed event_type | 409 IDEMPOTENCY_CONFLICT; event_envelope_hash differs; no cursor movement |
| INBOX-30 | Same event_id and payload but changed schema_version or occurred_at | 409 IDEMPOTENCY_CONFLICT; event_envelope_hash differs; no cursor movement |
| INBOX-31 | Same semantic event envelope with different JSON whitespace/member order | Duplicate ACK; JCS payload_hash and event_envelope_hash remain identical |
| INBOX-32 | Rust and an independent Node/Python implementation canonicalize the same callback envelope | Exact JCS UTF-8 bytes, payload_hash and event_envelope_hash match |
| INBOX-33 | Callback payload contains RFC 8785 numeric boundary values, negative zero, and UTF-16 ordering cases | Rust and independent implementation produce identical canonical bytes and hashes; invalid non-finite/unpaired-surrogate values reject |
| INBOX-34 | Duplicate object member names, including escaped-equivalent and nested duplicates, arrive at the callback boundary | Reject before serde_json::Value materialization, persistence or cursor movement |
| INBOX-35 | Rust and Node independently verify the pinned shared SEM callback envelope fixture | Exact payload/envelope JCS bytes and both SHA-256 values match; changed event fields or payload fail the vector |
| INBOX-36 | SEM Inbox row has `payload_hash = NULL` | CHECK rejects the row; SQL NULL/UNKNOWN must not satisfy SEM hash integrity |
| INBOX-37 | SEM Inbox row has `event_envelope_hash = NULL` | CHECK rejects the row; SQL NULL/UNKNOWN must not satisfy SEM hash integrity |
| INBOX-38 | Execute the complete SEM source CHECK against SEM/non-SEM NULL and malformed-hash truth-table cases | DDL parses; only the explicitly permitted SEM/non-SEM row shapes pass; no UNKNOWN result is accepted for SEM rows |
| INBOX-39 | Optional `correlation_id` is present in one envelope and omitted in another with all other fields equal | Payload hash remains equal; full-envelope hash differs because only schema-defined fields actually present are canonicalized |
| INBOX-40 | Existing global event_id belongs to a different execution/body while the incoming execution/sequence is also owned by another event | Deterministic precedence: 409 IDEMPOTENCY_CONFLICT; no winner tenant/project disclosure and no writes |
| INBOX-41 | Existing event_id and all envelope hashes/tuple match, but the sequence lookup does not resolve to the same Inbox row | Generic integrity failure; rollback and never ACK an inconsistent duplicate |
| INBOX-42 | No event_id row exists but the incoming execution/sequence is already owned by another event_id after a concurrent conflict | 409 EVENT_SEQUENCE_CONFLICT; no cursor movement or observation insert |
| INBOX-43 | Global event_id already exists under a non-SEM source | 409 IDEMPOTENCY_CONFLICT; never classify the generic Inbox row as a SEM duplicate |
| INBOX-44 | Existing event_id row has the same execution_id but tenant_id/project_id conflict with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |
| INBOX-45 | SEM observation insert omits detail or supplies NULL/non-object detail | SEM CHECK rejects insert; whole Inbox transaction rolls back; no ACK |
| INBOX-46 | SEM observation detail omits event_id, sequence, sem_event_type, event_envelope_hash or binding_epoch, or sequence differs from dedupe_tag | SEM CHECK rejects insert; no partial Inbox/cursor state |
| INBOX-47 | SEM observation row is populated from a Run with wrong tenant/project/task/client or substitutes SEM binding_epoch for AI-PM lease_generation | Integration test fails; observation must copy scope/client/lease_generation from locked Run and store binding_epoch separately in detail |
| INBOX-48 | Exact Inbox duplicate exists but its task_run_event observation is missing or detail event_id/envelope hash differs | Generic integrity failure; no duplicate ACK, no new observation and no cursor movement |
| INBOX-49 | Exact Inbox duplicate and observation exist, but the locked Run cursor is lower than the committed event sequence | Generic integrity failure; never ACK an impossible regressed cursor |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

### 6.1 Sixth-event and projection extension cases (INBOX-50..INBOX-57)

Added 2026-11-28 for `execution.unbound_release_applied` and the canonical execution projection (§14). Numbering continues the 49-case matrix.

| ID | Scenario | Required result |
|---|---|---|
| INBOX-50 | First `execution.unbound_release_applied` for an execution with no prior binding | One Inbox row, one Run observation, projection → `released_unbound` with non-null `dispatch_fence_id` and NULL binding tuple, cursor advances; ACK only after commit |
| INBOX-51 | Same sixth event event_id/body replayed | Duplicate ACK; projection bytes and `updated_at` unchanged |
| INBOX-52 | Sixth event arrives while any binding projection state exists (`eligible` never created a binding; test `bound`/`release_pending`/`released`) | Integrity/ordering rejection; no writes; cursor unchanged |
| INBOX-53 | `execution.binding_established` (any epoch) after `released_unbound` is recorded | Rejected as contradiction; tombstone never overwritten; cursor unchanged; observation history append-only |
| INBOX-54 | Sixth event and `binding_established` concurrently delivered on two connections | Run-row lock serializes: exactly one terminal outcome. If the sixth event commits first, the binding event rejects; if the binding commits first, the sixth event rejects (UNBOUND-03 path). No partial state |
| INBOX-55 | Sixth event arrives with a sequence gap (N+1 before N) | 409 EVENT_SEQUENCE_GAP with expected_sequence; no writes |
| INBOX-56 | Projection row attempted with `released_unbound` but NULL `dispatch_fence_id` or any non-null binding-tuple column | DDL state/tuple CHECK rejects the write; whole callback transaction rolls back; no ACK |
| INBOX-57 | Sixth event payload carries a runner/binding field or `disposition <> 'released_unbound'` | Application semantic validation rejects before the transaction; cursor unchanged (mirrors SEM schema D04-F05) |

## 7. Response contract

AI-PM must match SEM D-04 exactly:
- Success/duplicate ACK is returned only after the Inbox row, `task_run_event` observation and Run cursor are committed (or exact duplicate invariants are verified).
- Every response after callback request authentication succeeds—including 200/400/404/409/422/429/500—is signed by AI-PM, bound to the exact originating method, authority, path and request Content-Digest, and carries exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- For a valid callback, the signed Idempotency-Key equals envelope event_id. If event_id is missing or mismatched, return signed 400 correlated only to the authenticated header; never echo the body value and never open the Inbox transaction.
- Unknown execution, wrong run_kind or non-enumerating scope failure is signed 404. Same event_id changed envelope is signed 409 IDEMPOTENCY_CONFLICT; sequence owner/gap conflicts are signed 409.
- Application-level 429 is authenticated, signed and includes mandatory Retry-After. Edge/WAF rate limiting before authentication is outside the API contract and must not reflect protocol IDs.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early 413 before full body digest verification), return only a generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no message_id/in_reply_to/request_id/event_id in the body.
- Verify exact raw body digest and request signature before JSON parsing; reject duplicate decoded JSON members before materializing the application value.
- If the server cannot sign an authenticated response, terminate the response/connection; never downgrade to an unsigned correlated response.

## 8. Open decisions and blockers

- Live PostgreSQL catalog proof: verify source/idempotency uniqueness, CHECKs, FKs, partial indexes and task_run_event event-type constraint against the target database.
- Empty-database forward migration replay and exact SQL acceptance for TRUE/FALSE/UNKNOWN CHECK behavior.
- Two-connection PostgreSQL tests with barriers: core semantics now have executable harness coverage (§15, 30/30 green, 2026-11-28); the remaining INBOX/BIND/UNBOUND cases beyond the harness set still require the production-adapter test suite once the migration and route exist. Crash injection proving Inbox + observation + cursor (+ projection) atomicity is harness-covered for the injected failure points.
- Pin and implement an RFC 8785 Rust canonicalizer only after independent crate/source review; `serde_json::Value::to_string()` is not JCS. Cross-language shared vectors must match exact bytes and both hashes.
- AI-PM Rust HTTP signature verifier must consume SEM's signed callback 404 and pre-auth 401/early-413 fixtures; prove middleware order and no-side-effect behavior.
- Bilateral live HTTP callback interoperability, durable nonce replay storage, key rotation/revocation, response verification and reverse-proxy behavior.
- Ratify the remaining OI decisions and this candidate DDL before any migration or callback route implementation.

**Disposition: DRAFT / CHANGES REQUIRED** — with the 2026-11-28 amendment: candidate DDL and transaction algorithm now additionally carry real-PostgreSQL harness acceptance evidence (§15); this remains design/test evidence only, not runtime completion. No migration, callback route, Rust canonicalizer, production runtime code, or implementation authorization is included.

## 9. Review corrections consolidated

- `payload_hash` is JCS SHA-256 of the payload object; `event_envelope_hash` is JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence` and `payload`. HTTP signature headers and transport metadata are excluded.
- Duplicate event_id replay compares source, Run identity/scope, event_type, idempotency_key, sequence, payload_hash and event_envelope_hash. A changed event/envelope is IDEMPOTENCY_CONFLICT; exact duplicate must resolve to the same sequence row and existing Run observation.
- Conflict precedence: non-SEM or different execution_id under same global event_id → IDEMPOTENCY_CONFLICT; same execution_id but stored tenant/project mismatch → generic integrity failure; otherwise changed tuple/hash → IDEMPOTENCY_CONFLICT; absent event_id but sequence owner → EVENT_SEQUENCE_CONFLICT; neither key after conflict → generic integrity failure.
- SEM observation detail must be an object with event_id, sequence, sem_event_type, schema_version, occurred_at, event_envelope_hash and binding_epoch. Sequence must equal decimal dedupe_tag. The SQL CHECK uses COALESCE around extracted JSON text so JSON null cannot pass as CHECK UNKNOWN.
- The callback event's `task_run_event.lease_generation` is the AI-PM lease generation at observation time; SEM `binding_epoch` is stored separately in detail. The duplicate path does not compare the stored lease_generation with the Run's current generation.
- Draft choice: no new source_event_id FK/column. Transaction atomicity, SEM detail CHECK, partial unique index and duplicate-path verification enforce Inbox↔observation linkage; retention-FK coupling is avoided.
- Shared JCS vector: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json` in `smark-sem`; Python/Node test implementations verify exact bytes and hashes. Rust canonicalizer dependency remains unselected.
- 2026-11-28 harness finding: the observation detail CHECK required a positive `binding_epoch` on every SEM observation, which rejected every valid `execution.unbound_release_applied` observation (the tombstone legitimately carries no epoch). The §3 CHECK now requires a positive epoch for the five binding-scoped event types and requires it to be absent/null for the sixth event. Fabricating epoch 0 remains forbidden on both sides. Found by the real-PostgreSQL harness, not by review.

## 10. Exact source provenance

| Source | Revision | Evidence |
|---|---|---|
| `migrations/0001_baseline.sql` | blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566` | `integration.inbound_event` and `delivery.task_run` baseline schema |
| `migrations/0013_task_run_event.sql` | blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9` | `delivery.task_run_event` columns, event-type CHECK, unique key and append-only behavior |
| `crates/pm-application/src/run_events.rs` | blob `8e7a8abcb6026682994598864a401d919bd10d8e` | Existing generic `record_run_event()` INSERT ... SELECT mapping; SEM must not call it because its occurred_at/dedupe behavior differs |

Existing `integration.inbound_event` constraints include global PK(id), unique `(project_id,id)`, `(tenant_id,id)`, `(project_id,source,idempotency_key)`, project FK and actor principal FK. `delivery.task_run` has `lease_generation`, `run_kind` and a unique `(tenant_id,project_id,id)` candidate added by this proposal. `task_run_event` has no FK to Run and is append-only; this proposal retains transaction-local linkage.


## 10. OPEN P0: current SEM binding projection and first-bind/rebind validation

The transaction algorithm in §5 step 2 requires validation against the current SEM binding tuple (`runner_id`, `runner_key_version`, `boot_id`, `supervisor_instance_id`, `binding_epoch`, and process/launch/containment/evidence identities). The proposed DDL currently adds only `task_run.execution_event_sequence`; it does not define where the current binding tuple is durably stored or how the receiver derives it. `task_run_event` is append-only, but no query/replay algorithm or allowed binding-transition table is specified.

The first `execution.binding_established` event exposes the gap: no current binding exists yet, so the generic “compare against current binding” rule cannot be applied. Before implementation, choose one explicit consumer-side representation:

- a minimal `delivery.task_run_sem_binding` projection keyed by the exact delivery attempt, atomically updated in the same transaction as Inbox + Run observation + cursor; or
- a deterministic event-history projection algorithm over accepted `sem_infrastructure_observed` events, with exact ordering, corruption detection and transition rules.

Whichever representation is chosen must define: initial bind; exact duplicate; same epoch with changed Runner tuple; lower/stale epoch; higher epoch after a positively completed prior release; rebind after SEM's separately authorized host-isolation disposition without claiming process exit; and attempted rebind while prior process/binding remains uncertain (reject and retain quarantine/reservation). SEM remains the authority that allocates binding_epoch and chooses the Runner; AI-PM stores only the last accepted binding reference for validation and must not use it to authorize dispatch or mutate Task/Run business state.

Required PostgreSQL acceptance cases: first binding inserts exactly one current-binding projection; exact duplicate does not rewrite it; same-epoch conflicting tuple is rejected; lower epoch is rejected; authorized higher-epoch rebind atomically replaces the projection while retaining append-only history; higher epoch while prior state is uncertain is rejected; transaction crash rolls back Inbox, observation, cursor and projection together. Until these are specified and tested, callback Inbox design is Changes Required and no route/migration implementation is authorized.


## 11. Proposed current-binding projection (P0-06 resolution candidate; superseded by §14)

**Recommendation: use a minimal AI-PM-owned projection, not event-history replay.** The projection is a consumer-side validation cache with transactional authority inside AI-PM only. SEM remains the sole authority for allocating binding_epoch, selecting Runner, and asserting infrastructure facts. This table MUST NOT be read by SEM, exposed as dispatch authorization, or used to mutate Task/Run business status.

Illustrative forward-only DDL (names/types must be checked against exact repository migration conventions before implementation):

~~~sql
CREATE TABLE delivery.task_run_sem_binding (
    task_run_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    binding_epoch bigint NOT NULL CHECK (binding_epoch >= 1),
    runner_id uuid NOT NULL,
    runner_key_version bigint NOT NULL CHECK (runner_key_version >= 1),
    boot_id text NOT NULL CHECK (length(boot_id) > 0),
    supervisor_instance_id uuid NOT NULL,
    delivery_id uuid NOT NULL,
    binding_state text NOT NULL CHECK (binding_state IN (
        'active', 'release_pending', 'released', 'uncertain', 'host_isolated'
    )),
    launch_id uuid,
    containment_id text,
    process_start_identity text,
    systemd_invocation_id text,
    stop_fence_id uuid,
    process_exit_evidence_id uuid,
    binding_event_id uuid NOT NULL,
    binding_event_sequence bigint NOT NULL CHECK (binding_event_sequence >= 1),
    last_event_id uuid NOT NULL,
    last_event_sequence bigint NOT NULL CHECK (last_event_sequence >= binding_event_sequence),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT fk_task_run_sem_binding_run
      FOREIGN KEY (tenant_id, project_id, task_run_id)
      REFERENCES delivery.task_run (tenant_id, project_id, id),
    CONSTRAINT ck_task_run_sem_binding_process_identity
      CHECK (
        (launch_id IS NULL AND containment_id IS NULL
          AND process_start_identity IS NULL AND systemd_invocation_id IS NULL)
        OR
        (launch_id IS NOT NULL AND containment_id IS NOT NULL
          AND length(containment_id) > 0)
      ),
    CONSTRAINT ck_task_run_sem_binding_release_evidence
      CHECK (
        binding_state NOT IN ('released')
        OR process_exit_evidence_id IS NOT NULL
        OR stop_fence_id IS NOT NULL
      )
);
~~~

The released-state check allows either positive process-exit evidence or the separately defined positive no-start stop-fence proof; application validation must verify which disposition applies. host_isolated is not process-exit evidence and must never populate either evidence column.

### 11.1 Projection update rules

The callback transaction extends §5, retaining lock order **Run row → current-binding row → Inbox event → Run observation**. The final migration must freeze one order and all writers/tests must match it.

1. **First binding:** only execution.binding_established may create the projection. Require no existing projection, binding_epoch=1 for the first SEM binding of this execution, and a valid Runner tuple. Insert state active, with binding_event_id/sequence equal to this event. No Task/Run business state changes.
2. **Exact duplicate:** resolve through Inbox event_id + envelope hash + sequence before projection mutation; return duplicate ACK without rewriting projection or updated_at.
3. **Same epoch:** immutable identity fields (runner_id, key version, boot, supervisor instance, delivery_id) must match. A changed tuple at the same epoch is a conflict; do not overwrite.
4. **Lower epoch:** reject as stale; never regress projection.
5. **Higher epoch:** do not accept merely because the integer is greater. Require the previous projection state to be released with exact release/no-start proof, or host_isolated with an exact separately authorized SEM host-isolation disposition reference. The new epoch must be strictly greater, and old binding history remains in append-only Run observations.
6. **Uncertain prior binding:** reject a higher epoch and retain the previous projection/reservation/quarantine. Heartbeat loss, WSS disconnect, host unreachable and cleanup timeout are not rebind permission.
7. **Process identity:** harness.process_observed may fill exact launch/containment/process identity only for the current epoch and exact Runner tuple. It may not replace a previously recorded non-null identity with a different identity. Exit observations attach exact evidence references; they do not change business Run outcome.
8. **Release:** requested/release_uncertain update state only according to authenticated SEM event and exact release_id. released requires the previously defined stop-fence/no-start proof or exact process-exit evidence. cleanup_failed/infrastructure_uncertain retain uncertain; do not free the binding for rebind.
9. **Host isolation:** host_isolated can be set only from an explicit SEM-authenticated event/reference proving the dedicated runner.quarantine.resolve operator disposition. An execution.infrastructure_uncertain event alone cannot set it. The current D-04 five-event contract does not yet carry this disposition reference; therefore host-isolation rebind remains a P0 contract dependency until the SEM event contract adds it.

### 11.2 Atomicity and required PostgreSQL cases

The projection update, Inbox insert, append-only Run observation and task_run.execution_event_sequence compare-and-set MUST commit or roll back together. Add two-connection tests:

- BIND-01 first epoch inserts exactly one projection and one Inbox/observation/cursor advancement.
- BIND-02 exact duplicate returns duplicate ACK and leaves projection bytes/updated_at unchanged.
- BIND-03 same epoch + changed Runner/key/boot/Supervisor/delivery tuple rejects.
- BIND-04 lower epoch rejects without cursor/projection movement.
- BIND-05 higher epoch after positive release proof atomically replaces current projection.
- BIND-06 higher epoch after authorized host-isolation reference only; no process-exit evidence is fabricated.
- BIND-07 higher epoch while prior binding uncertain rejects and retains quarantine/reservation.
- BIND-08 two concurrent first-binding events serialize; one exact sequence commits, conflicting identity/sequence rejects.
- BIND-09 crash after Inbox insert, after projection insert/update, after Run observation, or before cursor CAS rolls back all four.
- BIND-10 process identity cannot be rewritten within one binding epoch.
- BIND-11 exact duplicate old event after a newer binding exists returns duplicate without regressing the current projection.
- BIND-12 callback event from stale runner_key_version/boot_id/supervisor_instance_id rejects.
- BIND-13 two different-epoch `binding_established` events (epoch 1 and epoch 2) concurrently in flight: Run-row lock serializes; exactly one terminal state — epoch 1 first commits, epoch 2 rejects without positive release/isolation proof; two commits are impossible.
- BIND-14 host-isolation disposition commit races with a higher-epoch rebind attempt: with the frozen lock order the disposition commits first and the rebind then applies through the §14 case-9 branch; if the rebind transaction precedes it, the rebind rejects and must retry after the disposition is durable. No window admits an unauthorized rebind.

The callback transaction must not rely on task_run_event replay as an unspecified current-state query. Append-only observations remain audit history; this projection is the explicit current consumer reference.


## 12. Proposed unbound-release transaction and execution projection (P0-07)

The AI-PM consumer must persist both (a) the last accepted binding tuple and (b) whether SEM has permanently fenced dispatch for this execution. A binding-only table cannot represent an execution released before first bind. Therefore, before implementation, rename/refine the §11 candidate as one per-attempt execution projection, e.g. delivery.task_run_sem_execution, with nullable binding columns and an explicit dispatch state. Do not create a fake binding row with epoch 0.

Required projection shape (illustrative; final DDL must reconcile with §11 and migration conventions):

~~~sql
CREATE TABLE delivery.task_run_sem_execution (
    task_run_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    dispatch_state text NOT NULL CHECK (dispatch_state IN (
        'eligible', 'bound', 'release_pending', 'released_unbound',
        'released', 'uncertain', 'host_isolated'
    )),
    binding_epoch bigint,
    runner_id uuid,
    runner_key_version bigint,
    boot_id text,
    supervisor_instance_id uuid,
    delivery_id uuid,
    launch_id uuid,
    containment_id text,
    process_start_identity text,
    systemd_invocation_id text,
    stop_fence_id uuid,
    process_exit_evidence_id uuid,
    dispatch_fence_id uuid,
    last_event_id uuid,
    last_event_sequence bigint NOT NULL DEFAULT 0 CHECK (last_event_sequence >= 0),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, project_id, task_run_id)
      REFERENCES delivery.task_run (tenant_id, project_id, id),
    CHECK (
      (dispatch_state = 'eligible' AND binding_epoch IS NULL AND runner_id IS NULL)
      OR
      (dispatch_state <> 'eligible' AND dispatch_state <> 'released_unbound'
        AND binding_epoch IS NOT NULL AND runner_id IS NOT NULL)
      OR
      (dispatch_state = 'released_unbound' AND binding_epoch IS NULL
        AND runner_id IS NULL AND dispatch_fence_id IS NOT NULL)
    )
);
~~~

This is a design sketch, not executable migration SQL: the final CHECK must explicitly enumerate all states and all tuple columns, reject partial identity tuples, and define which states require release/exit evidence. released_unbound must never carry Runner, binding_epoch, stop_fence_id or process-exit evidence.

### 12.1 Callback handling of released_unbound

- Accept only the new SEM-authenticated execution.unbound_release_applied event.
- Require no prior binding projection for this execution; if a binding already exists, reject as an integrity/ordering contradiction and do not ACK.
- In one transaction insert Inbox + append-only Run observation + update execution projection to released_unbound + advance contiguous cursor.
- Once released_unbound is recorded, reject every later execution.binding_established event for this execution, even with a higher epoch. A new AI-PM attempt has a new execution_id.
- Exact duplicate event_id/body returns duplicate ACK and does not rewrite the projection.
- A gap, changed release_id/fence ID, unexpected existing binding, or non-contiguous event sequence rejects without cursor movement.

### 12.2 Additional acceptance cases

- UNBOUND-01 event accepted only when no current binding exists.
- UNBOUND-02 same event replay is duplicate and leaves projection unchanged.
- UNBOUND-03 event after binding established rejects; never discard the bound release path.
- UNBOUND-04 any later binding event after released_unbound rejects, regardless of epoch.
- UNBOUND-05 transaction crash rolls back Inbox, Run observation, projection and cursor together.
- UNBOUND-06 SEM release-wins tombstone prevents delayed dispatch/requeue; tested on SEM side under same sequencer.
- UNBOUND-07 new execution_id is independent from old released_unbound projection.
- UNBOUND-08 released_unbound never creates process-exit or Runner stop evidence.
- UNBOUND-09 release-vs-binding concurrency on two connections where the unbound release linearizes first: tombstone commits, the racing binding event rejects (INBOX-54 winner branch).
- UNBOUND-10 mid-transaction failure of the sixth-event callback (after any of Inbox insert / observation / projection update / before cursor CAS) rolls back all four writes; exact replay of the same event then succeeds exactly once and is idempotent thereafter.

The current five-event schema cannot carry this event. AI-PM must not accept a lookalike execution.release_status payload with missing Runner identity. P0-07 remains open until SEM publishes the sixth-event schema and bilateral fixtures.


## 13. Canonical projection shape and lock-order correction

Sections 11 and 12 describe one projection, not two tables. The canonical candidate is **only** delivery.task_run_sem_execution. The earlier delivery.task_run_sem_binding name in §11 is superseded and MUST NOT be created as a second table. The final projection must combine:
- dispatch_state (including eligible and released_unbound);
- nullable current binding tuple for the unbound states;
- exact binding/process/release evidence references for bound states;
- last accepted SEM event identity/sequence;
- tenant/project/Run composite identity.

Use §12's execution-projection table name and unbound semantics, and §11's binding validation/rebind rules as the lifecycle rules for its bound tuple. Before implementation, rewrite the illustrative SQL into one complete DDL block with exhaustive state/NULLability CHECK constraints. No duplicate projection or dual-write is permitted.

Freeze the callback lock order as: **delivery.task_run FOR UPDATE → delivery.task_run_sem_execution FOR UPDATE (or insert under the Run lock) → integration.inbound_event insert/lookup → delivery.task_run_event insert → task_run cursor CAS → COMMIT**. All callback writers use this order. The per-execution Run lock serializes first-row creation; a unique PK on task_run_id is the backstop. Exact duplicate handling may verify existing Inbox/observation before projection mutation, but must not acquire locks in a different order.

Add this as a required migration review gate: the projection row, Inbox, Run observation and cursor either all commit or all roll back. The projection is an AI-PM consumer reference only, never SEM dispatch authority.

## 14. Canonical single-table DDL and the ten binding/event handling rules (P0-06/P0-07 resolution)

**Single table, no dual write.** §11's `delivery.task_run_sem_binding` and §12's sketch are superseded by this section. There is exactly one projection table: `delivery.task_run_sem_execution`, one row per execution attempt (`task_run_id` PK), holding dispatch_state, the current binding tuple, host identity, launch/containment identity, release state and uncertainty/quarantine state. The illustrative SQL in §11/§12 MUST NOT be created.

### 14.1 Canonical DDL (candidate; forward-only; conventions re-checked against the repository before any migration)

~~~sql
CREATE TABLE delivery.task_run_sem_execution (
    task_run_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,

    -- dispatch / release lifecycle (P0-07) and binding lifecycle (P0-06)
    dispatch_state text NOT NULL,
    binding_epoch bigint,
    runner_id uuid,
    runner_key_version bigint,
    boot_id text,
    supervisor_instance_id uuid,
    delivery_id uuid,

    -- exact process/containment identity (filled only by harness.process_observed)
    launch_id uuid,
    containment_id text,
    process_start_identity text,
    systemd_invocation_id text,

    -- release evidence references
    stop_fence_id uuid,
    process_exit_evidence_id uuid,

    -- host-isolation operator disposition reference (D-03), never process-exit evidence
    host_isolation_disposition_id uuid,

    -- P0-07 dispatch-fence tombstone identity
    dispatch_fence_id uuid,

    -- accepted binding event identity
    binding_event_id uuid,
    binding_event_sequence bigint,

    -- last accepted SEM event (any of the six types)
    last_event_id uuid NOT NULL,
    last_event_sequence bigint NOT NULL,

    first_bound_at timestamptz,
    released_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT fk_task_run_sem_execution_run
      FOREIGN KEY (tenant_id, project_id, task_run_id)
      REFERENCES delivery.task_run (tenant_id, project_id, id),

    CONSTRAINT task_run_sem_execution_state_check CHECK (dispatch_state IN (
        'eligible', 'bound', 'release_pending', 'released',
        'released_unbound', 'uncertain', 'host_isolated'
    )),

    -- exhaustive per-state tuple constraints (no partial identity tuples)
    CONSTRAINT task_run_sem_execution_tuple_check CHECK (
        (
          dispatch_state = 'eligible'
          AND binding_epoch IS NULL AND runner_id IS NULL
          AND runner_key_version IS NULL AND boot_id IS NULL
          AND supervisor_instance_id IS NULL AND delivery_id IS NULL
          AND launch_id IS NULL AND containment_id IS NULL
          AND process_start_identity IS NULL AND systemd_invocation_id IS NULL
          AND stop_fence_id IS NULL AND process_exit_evidence_id IS NULL
          AND host_isolation_disposition_id IS NULL AND dispatch_fence_id IS NULL
          AND binding_event_id IS NULL AND binding_event_sequence IS NULL
          AND first_bound_at IS NULL AND released_at IS NULL
        ) OR (
          dispatch_state = 'released_unbound'
          AND binding_epoch IS NULL AND runner_id IS NULL
          AND runner_key_version IS NULL AND boot_id IS NULL
          AND supervisor_instance_id IS NULL AND delivery_id IS NULL
          AND launch_id IS NULL AND containment_id IS NULL
          AND process_start_identity IS NULL AND systemd_invocation_id IS NULL
          AND stop_fence_id IS NULL AND process_exit_evidence_id IS NULL
          AND host_isolation_disposition_id IS NULL
          AND dispatch_fence_id IS NOT NULL
          AND binding_event_id IS NULL AND binding_event_sequence IS NULL
          AND first_bound_at IS NULL AND released_at IS NOT NULL
        ) OR (
          dispatch_state IN ('bound', 'release_pending', 'released', 'uncertain', 'host_isolated')
          AND binding_epoch >= 1 AND runner_id IS NOT NULL
          AND runner_key_version >= 1 AND boot_id IS NOT NULL AND length(boot_id) > 0
          AND supervisor_instance_id IS NOT NULL AND delivery_id IS NOT NULL
          AND binding_event_id IS NOT NULL AND binding_event_sequence >= 1
          AND dispatch_fence_id IS NULL
        )
    ),

    -- process identity is all-or-nothing within one binding
    CONSTRAINT task_run_sem_execution_process_identity_check CHECK (
      (launch_id IS NULL AND containment_id IS NULL
        AND process_start_identity IS NULL AND systemd_invocation_id IS NULL)
      OR
      (launch_id IS NOT NULL AND containment_id IS NOT NULL
        AND length(containment_id) > 0
        AND process_start_identity IS NOT NULL
        AND systemd_invocation_id IS NOT NULL)
    ),

    -- released requires positive proof: exact process-exit evidence
    -- OR the positive no-start stop-fence proof
    CONSTRAINT task_run_sem_execution_release_evidence_check CHECK (
      dispatch_state <> 'released'
      OR process_exit_evidence_id IS NOT NULL
      OR stop_fence_id IS NOT NULL
    ),

    -- host_isolated references an explicit operator disposition and must not
    -- fabricate process-exit or stop-fence evidence
    CONSTRAINT task_run_sem_execution_host_isolation_check CHECK (
      dispatch_state <> 'host_isolated'
      OR (host_isolation_disposition_id IS NOT NULL
        AND process_exit_evidence_id IS NULL
        AND stop_fence_id IS NULL)
    ),

    -- exit evidence exists only on release-pending/released bound states
    CONSTRAINT task_run_sem_execution_exit_evidence_state_check CHECK (
      process_exit_evidence_id IS NULL
      OR dispatch_state IN ('release_pending', 'released')
    ),

    -- a stop fence exists only on bound lifecycle states
    CONSTRAINT task_run_sem_execution_stop_fence_state_check CHECK (
      stop_fence_id IS NULL
      OR dispatch_state IN ('bound', 'release_pending', 'released')
    ),

    CONSTRAINT task_run_sem_execution_sequence_check CHECK (
      last_event_sequence >= 0
      AND (binding_event_sequence IS NULL OR last_event_sequence >= binding_event_sequence)
    )
);
~~~

### 14.2 The ten binding/event handling rules (normative consumer contract)

Mirrors SEM protocol §7.2 (smark-sem `docs/sem-d05-lifecycle-crash-consistency`/`docs/d05-linux-proof-contract`, PR #3/#5). SEM guarantees the evidence; AI-PM implements exactly the handling column, inside the §13 transaction with the frozen lock order.

| # | Case | Required AI-PM handling |
|---|---|---|
| 1 | First binding (`binding_established`, no prior binding) | First-bind algorithm: under the locked Run, require no projection row or a row in `eligible`; require `binding_epoch = 1` and a complete Runner tuple (runner_id, runner_key_version ≥ 1, boot_id, supervisor_instance_id, delivery_id); INSERT the projection `bound` with binding_event_id/sequence = this event, `first_bound_at = now()`; atomically with Inbox + observation + cursor CAS. No historical binding is compared |
| 2 | Exact duplicate binding event | Resolve through Inbox event identity + envelope hash + sequence BEFORE projection mutation; return duplicate ACK; projection bytes and `updated_at` unchanged |
| 3 | Same epoch, different tuple | Integrity conflict: reject, quarantine, never overwrite the projection, cursor does not advance |
| 4 | Lower-epoch late event | Reject as stale; never regress the projection; observation history remains append-only |
| 5 | Higher-epoch rebind | Accept only when prior projection state is `released` with exact `process_exit_evidence_id`/`stop_fence_id`, or `host_isolated` with exact `host_isolation_disposition_id`; new epoch strictly greater; UPDATE replaces the tuple in-transaction; old history stays in observations |
| 6 | Rebind after normal release | Same as case 5 release branch; evidence references must resolve to the same execution/epoch/runner-key/boot/supervisor identity recorded in observations |
| 7 | Uncertain prior binding | Reject any epoch change while prior state is `uncertain`; retain quarantine + reservation; heartbeat loss/WSS disconnect/host unreachable are never rebind permission |
| 8 | Runner host permanent loss | `host_isolated` settable ONLY from the SEM-authenticated `runner.quarantine.resolve` operator disposition reference; never from an automatic event and never treated as process exit |
| 9 | Host physically isolated | Same path as case 8 with the exact affected identities + operator attestation reference; rebind afterwards follows case 5's disposition branch; no fabricated exit evidence |
| 10 | Inspection inconclusive | Prior state stays `uncertain`; reject rebind; resolution only through named-authority reconciliation evidence |

Tombstone protection (P0-07, restated): once `dispatch_state = 'released_unbound'` is committed, every later `execution.binding_established` for the same execution_id is rejected regardless of epoch (INBOX-53); the tombstone is never overwritten and a new AI-PM attempt uses a new execution_id.

SEM self-asserted host state (heartbeat, WSS session, telemetry) is never AI-PM dispatch or rebind authorization.

### 14.3 Concurrency coverage map (required two-connection scenarios)

| Required race | Covered by |
|---|---|
| 首次绑定的并发竞争 | BIND-08 |
| 两个不同 epoch 同时到达 | BIND-13 |
| 重复事件并发提交 | INBOX-06 / BIND-02 |
| 旧事件晚于新事件到达 | BIND-04/BIND-11 / INBOX-49 |
| Release 与 Binding 并发 | INBOX-54 / UNBOUND-09 |
| Host isolation 与重新绑定并发 | BIND-14 |
| 事务中途失败后重放 | BIND-09 / UNBOUND-10 / INBOX-09/22 |

## 15. Executable acceptance harness results (2026-11-28)

The candidate §3/§13/§14 contract now has a standalone real-PostgreSQL
acceptance harness on this branch: [sem-inbox-acceptance-harness](./sem-inbox-acceptance-harness/README.md).

- 30 scenarios executed on a disposable PostgreSQL 16.16 instance in an
  ephemeral database created and dropped by the harness (no ai-pm migration
  touched, no shared database touched).
- Two independent connections with explicit barriers; injected failures after
  the Inbox insert, after the Run observation and before commit all rolled
  back every write (no partial state, no ACK).
- Final result: **30 passed, 0 failed**. Full pass list and scope limits:
  [RESULTS.md](./sem-inbox-acceptance-harness/RESULTS.md).
- Coverage: baseline Inbox transaction and idempotency (INBOX-01..09/19/20/21),
  global event-id and sequence-unique backstops (INBOX-05/25/26), CHECK truth
  table (INBOX-36..38), duplicate-member rejection (INBOX-24/34), sixth event
  and tombstone (INBOX-50..54/56/57, UNBOUND-03/04/08/09/10), binding
  projection rules and races (BIND-03..09/12/13/14).
- The harness found one real contract defect (§9, binding_epoch tombstone
  CHECK) which is fixed in §3 and re-verified by the green run.

What this does NOT prove: the production migration, callback route, Rust
application port, pinned JCS canonicalizer, response signing middleware and
bilateral live HTTP interop remain unimplemented and untested. This harness is
database-contract acceptance evidence, not runtime completion.
