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
            AND detail ? 'binding_epoch'
            AND detail ? 'schema_version'
            AND COALESCE(detail->>'schema_version', '') ~ '^[1-9][0-9]*$'
            AND detail ? 'occurred_at'
            AND COALESCE(detail->>'event_id', '') ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
            AND COALESCE(detail->>'sem_event_type', '') <> ''
            AND COALESCE(detail->>'execution_event_sequence', '') ~ '^[1-9][0-9]*$'
            AND detail->>'execution_event_sequence' = dedupe_tag
            AND COALESCE(detail->>'occurred_at', '') <> ''
            AND COALESCE(detail->>'event_envelope_hash', '') ~ '^[0-9a-f]{64}$'
            AND COALESCE(detail->>'binding_epoch', '') ~ '^[1-9][0-9]*$'
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
- Two-connection PostgreSQL tests with barriers for all 49 cases, plus crash injection proving Inbox + observation + cursor atomicity.
- Pin and implement an RFC 8785 Rust canonicalizer only after independent crate/source review; `serde_json::Value::to_string()` is not JCS. Cross-language shared vectors must match exact bytes and both hashes.
- AI-PM Rust HTTP signature verifier must consume SEM's signed callback 404 and pre-auth 401/early-413 fixtures; prove middleware order and no-side-effect behavior.
- Bilateral live HTTP callback interoperability, durable nonce replay storage, key rotation/revocation, response verification and reverse-proxy behavior.
- Ratify the remaining OI decisions and this candidate DDL before any migration or callback route implementation.

**Disposition: DRAFT / CHANGES REQUIRED.** This is design evidence only; no migration, callback route, Rust canonicalizer, runtime code, or implementation authorization is included.

## 9. Review corrections consolidated

- `payload_hash` is JCS SHA-256 of the payload object; `event_envelope_hash` is JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence` and `payload`. HTTP signature headers and transport metadata are excluded.
- Duplicate event_id replay compares source, Run identity/scope, event_type, idempotency_key, sequence, payload_hash and event_envelope_hash. A changed event/envelope is IDEMPOTENCY_CONFLICT; exact duplicate must resolve to the same sequence row and existing Run observation.
- Conflict precedence: non-SEM or different execution_id under same global event_id → IDEMPOTENCY_CONFLICT; same execution_id but stored tenant/project mismatch → generic integrity failure; otherwise changed tuple/hash → IDEMPOTENCY_CONFLICT; absent event_id but sequence owner → EVENT_SEQUENCE_CONFLICT; neither key after conflict → generic integrity failure.
- SEM observation detail must be an object with event_id, sequence, sem_event_type, schema_version, occurred_at, event_envelope_hash and binding_epoch. Sequence must equal decimal dedupe_tag. The SQL CHECK uses COALESCE around extracted JSON text so JSON null cannot pass as CHECK UNKNOWN.
- The callback event's `task_run_event.lease_generation` is the AI-PM lease generation at observation time; SEM `binding_epoch` is stored separately in detail. The duplicate path does not compare the stored lease_generation with the Run's current generation.
- Draft choice: no new source_event_id FK/column. Transaction atomicity, SEM detail CHECK, partial unique index and duplicate-path verification enforce Inbox↔observation linkage; retention-FK coupling is avoided.
- Shared JCS vector: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json` in `smark-sem`; Python/Node test implementations verify exact bytes and hashes. Rust canonicalizer dependency remains unselected.

## 10. Exact source provenance

| Source | Revision | Evidence |
|---|---|---|
| `migrations/0001_baseline.sql` | blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566` | `integration.inbound_event` and `delivery.task_run` baseline schema |
| `migrations/0013_task_run_event.sql` | blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9` | `delivery.task_run_event` columns, event-type CHECK, unique key and append-only behavior |
| `crates/pm-application/src/run_events.rs` | blob `8e7a8abcb6026682994598864a401d919bd10d8e` | Existing generic `record_run_event()` INSERT ... SELECT mapping; SEM must not call it because its occurred_at/dedupe behavior differs |

Existing `integration.inbound_event` constraints include global PK(id), unique `(project_id,id)`, `(tenant_id,id)`, `(project_id,source,idempotency_key)`, project FK and actor principal FK. `delivery.task_run` has `lease_generation`, `run_kind` and a unique `(tenant_id,project_id,id)` candidate added by this proposal. `task_run_event` has no FK to Run and is append-only; this proposal retains transaction-local linkage.
