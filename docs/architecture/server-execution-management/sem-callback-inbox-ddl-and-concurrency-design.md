# SEM Callback Inbox — Exact DDL and Concurrency Design

- Status: **DRAFT / REVIEW REQUIRED**
- AI-PM PR: #9, branch \`docs/sem-02e-architecture-convergence\`
- Audited base: \`master\` at \`d0e72fa6805aebeadf1b282e430deba493072fd6\`
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.

            AND COALESCE(detail->>'binding_epoch', '') ~ '^[1-9][0-9]*
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful Inbox insert, insert exactly one task_run_event row: event_type='sem_infrastructure_observed', run_id=execution_id, dedupe_tag=decimal(sequence), and detail containing event ID, sequence, type, binding identity, and evidence references. The explicit NOT NULL + length CHECK rejects a missing/NULL dedupe tag.
7. Advance task_run.execution_event_sequence to the incoming value, guarded by the old cursor. Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.

        )
        OR (
            event_type <> 'sem_infrastructure_observed'
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful Inbox insert, insert exactly one task_run_event row: event_type='sem_infrastructure_observed', run_id=execution_id, dedupe_tag=decimal(sequence), and detail containing event ID, sequence, type, binding identity, and evidence references. The explicit NOT NULL + length CHECK rejects a missing/NULL dedupe tag.
7. Advance task_run.execution_event_sequence to the incoming value, guarded by the old cursor. Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.


## 12. Run-observation row contract and Inbox linkage

The existing `delivery.task_run_event` table has required denormalized columns `tenant_id`, `project_id`, `task_id`, `run_id`, `client_id`, `event_type`, `lease_generation`, `dedupe_tag`, and nullable `detail`. A SEM callback cannot insert only `run_id/event_type/dedupe_tag/detail`; it must populate the full row.

The SEM observation mapping is now frozen at Draft level in transaction step 6. The key distinction is:

- `task_run_event.lease_generation` records the AI-PM Run lease generation observed when the callback was accepted.
- `detail.binding_epoch` records the SEM infrastructure binding epoch.
- They are different authorities and are never interchangeable.

The SEM-only CHECK now requires an object detail with event_id, execution_event_sequence, sem_event_type, event_envelope_hash and binding_epoch; the sequence must equal the decimal dedupe_tag and the envelope hash must be lowercase 64-character hex. The unique partial index on (run_id, dedupe_tag) prevents duplicate observations across lease generations.

The Draft deliberately retains transaction-local Inbox↔observation linkage rather than adding a source_event_id FK/column. On first insert, Inbox, observation and Run cursor update commit atomically. On duplicate, the receiver verifies the existing observation's event_id and event_envelope_hash before ACK. INBOX-45..48 cover missing/invalid detail, denormalized Run mapping, and broken duplicate linkage. This choice avoids additional retention coupling but requires real PostgreSQL two-connection and crash-atomicity proof.

            AND COALESCE(detail->>'sem_event_type', '') <> ''
            AND COALESCE(detail->>'execution_event_sequence', '') ~ '^[1-9][0-9]*
            AND detail->>'event_envelope_hash' ~ '^[0-9a-f]{64}
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- **Draft decision:** retain the existing append-only task_run_event ledger shape without a new source_event_id column/FK. Enforce Inbox↔observation linkage in the same locked transaction, add the SEM detail CHECK and partial uniqueness backstop, and verify the observation row on duplicate replay. This avoids introducing a new retention FK coupling; two-connection tests must prove the transaction-local invariant.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful first-time Inbox insert, insert exactly one task_run_event row using the locked Run as the source of denormalized scope/provenance:
   - tenant_id, project_id, task_id, run_id: copied from the locked delivery Run;
   - client_id: the Run's current lease_client_id (nullable; do not invent a SEM client);
   - event_type: literal sem_infrastructure_observed;
   - lease_generation: the Run's current AI-PM lease_generation at observation time. This is NOT SEM binding_epoch and MUST NOT be substituted for it;
   - dedupe_tag: canonical base-10 execution_event_sequence string;
   - occurred_at: authenticated envelope occurred_at; received_at and created_at use database defaults;
   - detail: JSON object containing event_id, execution_event_sequence, sem_event_type, schema_version, occurred_at, event_envelope_hash, binding_epoch, runner_id, runner_key_version, boot_id, supervisor_instance_id, launch/containment/process identity where applicable, and evidence references.
   
   Use a direct INSERT ... SELECT from the locked Run row (or an equivalent statement that copies exactly those fields). Do not call generic record_run_event() because its default occurred_at and generation/dedupe behavior are not the SEM callback contract. The SEM CHECK requires non-null object detail, event_id/sequence/type/hash/binding_epoch fields, sequence equality with dedupe_tag and a valid envelope-hash format. The partial unique index prevents a second observation for the same Run/sequence across lease generations.
7. Advance task_run.execution_event_sequence to the incoming value with a compare-and-set guard against the previously locked cursor. The UPDATE MUST affect exactly one row; zero rows is an invariant failure and rolls back Inbox + observation:
   
   UPDATE delivery.task_run
      SET execution_event_sequence = :incoming_sequence
    WHERE id = :execution_id
      AND tenant_id = :tenant_id
      AND project_id = :project_id
      AND execution_event_sequence = :previous_cursor
   RETURNING execution_event_sequence;

   Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK. For an exact duplicate event_id, do not insert another observation or move the cursor; verify the existing task_run_event row by run_id + sem event type + dedupe_tag and require detail.event_id and detail.event_envelope_hash to match the Inbox row. Missing/mismatched observation or a cursor lower than an already committed event sequence is a generic integrity failure and MUST NOT be ACKed.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |
| INBOX-45 | SEM observation insert omits detail or supplies NULL/non-object detail | SEM CHECK rejects insert; whole Inbox transaction rolls back; no ACK |
| INBOX-46 | SEM observation detail omits event_id, sequence, sem_event_type, event_envelope_hash or binding_epoch, or sequence differs from dedupe_tag | SEM CHECK rejects insert; no partial Inbox/cursor state |
| INBOX-47 | SEM observation row is populated from a Run with wrong tenant/project/task/client or substitutes SEM binding_epoch for AI-PM lease_generation | Integration test fails; observation must copy scope/client/lease_generation from locked Run and store binding_epoch separately in detail |
| INBOX-48 | Exact Inbox duplicate exists but its task_run_event observation is missing or detail event_id/envelope hash differs | Generic integrity failure; no duplicate ACK, no new observation and no cursor movement |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.

            AND (detail->>'binding_epoch') ~ '^[1-9][0-9]*
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful Inbox insert, insert exactly one task_run_event row: event_type='sem_infrastructure_observed', run_id=execution_id, dedupe_tag=decimal(sequence), and detail containing event ID, sequence, type, binding identity, and evidence references. The explicit NOT NULL + length CHECK rejects a missing/NULL dedupe tag.
7. Advance task_run.execution_event_sequence to the incoming value, guarded by the old cursor. Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.

        )
        OR (
            event_type <> 'sem_infrastructure_observed'
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful Inbox insert, insert exactly one task_run_event row: event_type='sem_infrastructure_observed', run_id=execution_id, dedupe_tag=decimal(sequence), and detail containing event ID, sequence, type, binding identity, and evidence references. The explicit NOT NULL + length CHECK rejects a missing/NULL dedupe tag.
7. Advance task_run.execution_event_sequence to the incoming value, guarded by the old cursor. Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.


## 12. Run-observation row contract and Inbox linkage

The existing `delivery.task_run_event` table has required denormalized columns `tenant_id`, `project_id`, `task_id`, `run_id`, `client_id`, `event_type`, `lease_generation`, `dedupe_tag`, and nullable `detail`. A SEM callback cannot insert only `run_id/event_type/dedupe_tag/detail`; it must populate the full row.

The SEM observation mapping is now frozen at Draft level in transaction step 6. The key distinction is:

- `task_run_event.lease_generation` records the AI-PM Run lease generation observed when the callback was accepted.
- `detail.binding_epoch` records the SEM infrastructure binding epoch.
- They are different authorities and are never interchangeable.

The SEM-only CHECK now requires an object detail with event_id, execution_event_sequence, sem_event_type, event_envelope_hash and binding_epoch; the sequence must equal the decimal dedupe_tag and the envelope hash must be lowercase 64-character hex. The unique partial index on (run_id, dedupe_tag) prevents duplicate observations across lease generations.

The Draft deliberately retains transaction-local Inbox↔observation linkage rather than adding a source_event_id FK/column. On first insert, Inbox, observation and Run cursor update commit atomically. On duplicate, the receiver verifies the existing observation's event_id and event_envelope_hash before ACK. INBOX-45..48 cover missing/invalid detail, denormalized Run mapping, and broken duplicate linkage. This choice avoids additional retention coupling but requires real PostgreSQL two-connection and crash-atomicity proof.

            AND detail->>'execution_event_sequence' = dedupe_tag
            AND COALESCE(detail->>'occurred_at', '') <> ''
            AND detail->>'event_envelope_hash' ~ '^[0-9a-f]{64}
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- **Draft decision:** retain the existing append-only task_run_event ledger shape without a new source_event_id column/FK. Enforce Inbox↔observation linkage in the same locked transaction, add the SEM detail CHECK and partial uniqueness backstop, and verify the observation row on duplicate replay. This avoids introducing a new retention FK coupling; two-connection tests must prove the transaction-local invariant.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful first-time Inbox insert, insert exactly one task_run_event row using the locked Run as the source of denormalized scope/provenance:
   - tenant_id, project_id, task_id, run_id: copied from the locked delivery Run;
   - client_id: the Run's current lease_client_id (nullable; do not invent a SEM client);
   - event_type: literal sem_infrastructure_observed;
   - lease_generation: the Run's current AI-PM lease_generation at observation time. This is NOT SEM binding_epoch and MUST NOT be substituted for it;
   - dedupe_tag: canonical base-10 execution_event_sequence string;
   - occurred_at: authenticated envelope occurred_at; received_at and created_at use database defaults;
   - detail: JSON object containing event_id, execution_event_sequence, sem_event_type, schema_version, occurred_at, event_envelope_hash, binding_epoch, runner_id, runner_key_version, boot_id, supervisor_instance_id, launch/containment/process identity where applicable, and evidence references.
   
   Use a direct INSERT ... SELECT from the locked Run row (or an equivalent statement that copies exactly those fields). Do not call generic record_run_event() because its default occurred_at and generation/dedupe behavior are not the SEM callback contract. The SEM CHECK requires non-null object detail, event_id/sequence/type/hash/binding_epoch fields, sequence equality with dedupe_tag and a valid envelope-hash format. The partial unique index prevents a second observation for the same Run/sequence across lease generations.
7. Advance task_run.execution_event_sequence to the incoming value with a compare-and-set guard against the previously locked cursor. The UPDATE MUST affect exactly one row; zero rows is an invariant failure and rolls back Inbox + observation:
   
   UPDATE delivery.task_run
      SET execution_event_sequence = :incoming_sequence
    WHERE id = :execution_id
      AND tenant_id = :tenant_id
      AND project_id = :project_id
      AND execution_event_sequence = :previous_cursor
   RETURNING execution_event_sequence;

   Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK. For an exact duplicate event_id, do not insert another observation or move the cursor; verify the existing task_run_event row by run_id + sem event type + dedupe_tag and require detail.event_id and detail.event_envelope_hash to match the Inbox row. Missing/mismatched observation is a generic integrity failure and MUST NOT be ACKed.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |
| INBOX-45 | SEM observation insert omits detail or supplies NULL/non-object detail | SEM CHECK rejects insert; whole Inbox transaction rolls back; no ACK |
| INBOX-46 | SEM observation detail omits event_id, sequence, sem_event_type, event_envelope_hash or binding_epoch, or sequence differs from dedupe_tag | SEM CHECK rejects insert; no partial Inbox/cursor state |
| INBOX-47 | SEM observation row is populated from a Run with wrong tenant/project/task/client or substitutes SEM binding_epoch for AI-PM lease_generation | Integration test fails; observation must copy scope/client/lease_generation from locked Run and store binding_epoch separately in detail |
| INBOX-48 | Exact Inbox duplicate exists but its task_run_event observation is missing or detail event_id/envelope hash differs | Generic integrity failure; no duplicate ACK, no new observation and no cursor movement |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.

            AND (detail->>'binding_epoch') ~ '^[1-9][0-9]*
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful Inbox insert, insert exactly one task_run_event row: event_type='sem_infrastructure_observed', run_id=execution_id, dedupe_tag=decimal(sequence), and detail containing event ID, sequence, type, binding identity, and evidence references. The explicit NOT NULL + length CHECK rejects a missing/NULL dedupe tag.
7. Advance task_run.execution_event_sequence to the incoming value, guarded by the old cursor. Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.

        )
        OR (
            event_type <> 'sem_infrastructure_observed'
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.

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

Authenticate the SEM service principal and verify signature/freshness before opening the database transaction. Then:

The SEM callback port MUST run this transaction at PostgreSQL READ COMMITTED isolation. If the surrounding transaction is configured at REPEATABLE READ or SERIALIZABLE, a serialization/snapshot conflict requires retrying the entire transaction with the exact same event_id and body; never continue querying inside an aborted transaction.

1. Resolve the candidate Run by tenant_id + project_id + execution_id. Unknown, wrong-tenant, wrong-project, and review-run IDs return non-enumerating 404.
2. Lock the exact Run row with SELECT FOR UPDATE. Validate delivery run kind and the current infrastructure binding tuple: runner_id, runner_key_version, boot_id, supervisor_instance_id, binding_epoch, plus event-specific process/evidence identity.
3. Check the global event_id before sequence ownership. If an event_id row already exists, compare source, tenant_id, project_id, event_type, idempotency_key, execution_id, execution_event_sequence, payload_hash and event_envelope_hash against the authenticated request and the locked Run-derived tenant/project:
   - First, a stored tenant_id/project_id that disagrees with the locked Run-derived scope is an integrity invariant failure: rollback and return a generic server error, never ACK or disclose the stored scope.
   - Otherwise, source other than sem, a different idempotency_key, or any changed event/envelope identity value is 409 IDEMPOTENCY_CONFLICT. This classification has precedence even if the incoming execution/sequence pair is currently owned by another event.

   - If all values match, query the unique (execution_id, execution_event_sequence) key and require it to resolve to the exact same Inbox row. If it does not, rollback and return a generic integrity error; never ACK a partially inconsistent duplicate.
   - Only when the event_id row and the matching sequence row are the same row is the request an exact duplicate.
4. If no event_id row exists, check the (execution_id, execution_event_sequence) key. If another event_id already owns the sequence, return 409 EVENT_SEQUENCE_CONFLICT. Then require incoming sequence = Run cursor + 1. A greater value is 409 EVENT_SEQUENCE_GAP with expected_sequence. A lower value without the matching committed event is conflict; never lower the cursor.
5. Insert the Inbox row using an all-constraint conflict-safe statement, not a plain INSERT:

   INSERT INTO integration.inbound_event (...)
   VALUES (...)
   ON CONFLICT DO NOTHING
   RETURNING id;

   This catches the global event_id primary key and the SEM partial unique execution-sequence index without aborting the transaction. If no row is returned, issue new SELECT statements under READ COMMITTED (each obtains a fresh statement snapshot), first by global event_id and then, as applicable, by (execution_id, execution_event_sequence):
   - If the event_id row exists and source, tenant_id, project_id, event_type, idempotency_key, execution_id, sequence, payload_hash or event_envelope_hash differs, return 409 IDEMPOTENCY_CONFLICT (except a tenant/project mismatch against the locked Run, which is a generic integrity failure). This takes precedence over any simultaneous sequence-owner conflict.
   - If the event_id row exists and all values match, require the sequence lookup (filtered to source='sem') to identify that exact same row; otherwise rollback and return a generic integrity error, never ACK.
   - If no event_id row exists but the SEM sequence key is owned by another event_id, return 409 EVENT_SEQUENCE_CONFLICT.
   - If neither row exists after ON CONFLICT DO NOTHING, rollback and return a generic integrity error, never ACK.
   The two lookups may return different rows only when the event_id already conflicts with a changed tuple; in that case IDEMPOTENCY_CONFLICT takes precedence and no winner details are disclosed. A matching event_id row and a different sequence row is an invariant violation.
   This post-conflict read uses new SQL statements; it never queries inside an aborted transaction.
6. For a successful Inbox insert, insert exactly one task_run_event row: event_type='sem_infrastructure_observed', run_id=execution_id, dedupe_tag=decimal(sequence), and detail containing event ID, sequence, type, binding identity, and evidence references. The explicit NOT NULL + length CHECK rejects a missing/NULL dedupe tag.
7. Advance task_run.execution_event_sequence to the incoming value, guarded by the old cursor. Because all events for one execution lock the same Run row first, same-execution sequence writes serialize; the unique index remains a database backstop.
8. Commit. Only after commit return the success ACK.

All callback writers use one lock order: Run row → Inbox event → Run observation. No callback path may lock Inbox first and then Run. A cross-Run global event_id collision is resolved by the conflict-safe insert and the follow-up lookup; it must not attempt to lock the foreign Run or disclose its project/tenant. Same sequence/different event ID is EVENT_SEQUENCE_CONFLICT; same event ID/different body is IDEMPOTENCY_CONFLICT.

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
| INBOX-44 | Existing event_id row's tenant_id/project_id conflicts with the locked Run-derived scope | Generic integrity failure; rollback and never disclose the stored scope or ACK |

Every test asserts exact row counts and values in all three storage locations, not only HTTP status.

## 7. Response contract

AI-PM must match SEM D-04 exactly:

- Success/duplicate: event_id, execution_id, execution_event_sequence, contiguous_through_sequence, disposition, acknowledged_at.

- Every response after successful callback request authentication is signed by AI-PM, including authenticated 400/404/409/422/429/500. It binds the exact originating method, authority, path and request Content-Digest and includes exactly one `X-SEM-Event-Id` equal to the authenticated `Idempotency-Key` header.
- Before authentication succeeds (invalid/missing signature, unknown/revoked key, stale/replayed nonce, or early body-size rejection), return only the generic unsigned pre-authentication error: no response signature headers, no `X-SEM-Event-Id`, and no request/event identifiers in the body.
- Verify signature and exact raw-body digest before JSON parsing. After authentication, reject duplicate JSON members, validate schema, and require the signed `Idempotency-Key` to equal envelope `event_id` before opening the database transaction. If they differ, return signed 400 correlated only to the authenticated header value; do not echo the body event_id and do not write.
- Application-level 429 is authenticated, signed and includes mandatory `Retry-After`; early edge/WAF throttling is outside the API protocol and must not reflect protocol identifiers.
- Unknown/wrong tenant/project/execution and review Run IDs are authenticated signed non-enumerating 404 responses.
- SEM publishes deterministic signed 404 response vectors for both Platform GET and SEM callback directions, plus unsigned pre-authentication 401/early-413 fixtures, in `tests/protocol/fixtures/crypto-vectors-v1.json` (reviewed SEM branch head `d4b31a0f8d1c344f858870fd9afbeb4e2b42f362`). AI-PM must independently verify the callback 404 vector's Ed25519 signature, request-context association and `X-SEM-Event-Id` correlation, and assert the pre-auth fixtures contain no trusted protocol IDs. These are fixtures, not live middleware proof.
- The signed callback ACK MUST include exactly one `X-SEM-Event-Id` correlation header equal to the authenticated callback `event_id`; it must not be derived from an untrusted body/header after verification. The Platform→SEM response direction uses `X-SEM-Request-Id = request_id` (the operation/idempotency identity), not `message_id`.
- SEM's deterministic response fixture `tests/protocol/fixtures/crypto-vectors-v1.json` (pinned at `smark-sem@3df2d35cd6e8066e2aff831506ab1e0b1ddc6395`) includes both `http_callback_ack` and `http_error_409`; AI-PM's eventual response verifier must validate the exact signed callback 200 ACK and its originating method/authority/path/request-digest context, the signed Platform→SEM 409, and reject mismatched direction-specific correlation headers or a response replayed against another request context.
- Same event ID with changed identity/body: HTTP 409 IDEMPOTENCY_CONFLICT.
- Same sequence with another event ID: HTTP 409 EVENT_SEQUENCE_CONFLICT.
- Sequence gap: HTTP 409 EVENT_SEQUENCE_GAP with expected_sequence.
- Unknown/wrong tenant/project/run kind: non-enumerating 404.
- Authentication/signature/freshness failures: exact status classification to be frozen in shared OpenAPI.
- Transient rate/database/service failure: retryable 429/5xx; no ACK and no partial commit.

Exact response JSON Schema and OpenAPI error components remain a separate bilateral closure item.

## 8. Open decisions and blockers

- OI-01: Ratify execution_id == delivery.task_run.id for one AI-PM attempt.
- OI-02: Ratify shared integration.inbound_event reuse after live catalog proof.
- OI-03: Ratify cursor, observation event type, and migration constraints.
- OI-04: Ratify service principal/key registry and RFC 9421 middleware.
- OI-05: Ratify retention, compaction, and operator replay.
- OI-06: Freeze exact success/error schemas and retry classification.
- Complete empty-database migration replay and capture live PostgreSQL catalog constraints/indexes.
- Add bilateral executable fixtures and two-connection PostgreSQL tests before implementation approval.

**Disposition: DRAFT / CHANGES REQUIRED.** Design artifact only; it does not authorize implementation, migration creation, route changes, or PR merge.

### 1.1 Exact migration-source provenance

Rechecked the actual squashed schema baseline at `migrations/0001_baseline.sql` (blob `6f1e9e59a4a539a26bf83d1826c48ddaf9fea566`) and the current Run-event migration `migrations/0013_task_run_event.sql` (blob `bec35e8e03f9dc36db3f8484a05fad19f6694ef9`) on the audited `ai-pm/master` source revision.

Exact existing `integration.inbound_event` columns are all NOT NULL for `id`, `project_id`, `actor_id`, `source`, `event_type`, `idempotency_key`, `payload`, `payload_hash` and `tenant_id`; `status` and `received_at` have defaults. The baseline defines global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)`, `UNIQUE (project_id,source,idempotency_key)`, and `UNIQUE (tenant_id,id)`, plus `(tenant_id,project_id) → core.project(tenant_id,id)` and `actor_id → core.principal(id)` foreign keys. Therefore `source IS NOT NULL` in the proposed SEM CHECK is an explicit defensive assertion, not a currently nullable-column repair.

Exact existing `delivery.task_run` has global `PRIMARY KEY (id)`, `UNIQUE (project_id,id)` and `UNIQUE (tenant_id,id)`, but no current `UNIQUE (tenant_id,project_id,id)`. The candidate triple key is therefore a new constraint required only to support the proposed three-column composite FK; it is not an existing source invariant. The row also has `run_kind` CHECK values `delivery|review`, so the callback still must lock and semantically reject review Runs because a foreign key cannot express the required delivery-only rule.

Exact `migrations/0013_task_run_event.sql` defines `dedupe_tag text DEFAULT '' NOT NULL`, `lease_generation integer DEFAULT 0 NOT NULL`, `task_run_event_type_check` with eight allowed values, `task_run_event_dedupe_tag_len CHECK (length(dedupe_tag) <= 200)`, and the existing unique index `(run_id,event_type,lease_generation,dedupe_tag)`. The new SEM event type therefore requires replacing the existing named type CHECK; a separate SEM sequence CHECK alone is insufficient. The existing `dedupe_tag` NOT NULL declaration already prevents SQL NULL, while the explicit `IS NOT NULL` in the new SEM-specific CHECK is retained for local clarity.

This is static migration-source verification, not a live PostgreSQL catalog capture. The exact catalog state of a deployed database, applied migration set and any local drift remain unverified until read-only catalog queries are run against the target database.)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 9. Independent replay-identity review correction (2026-11-25)

A further review found an idempotency gap in the original candidate rule: comparing only `execution_id`, `execution_event_sequence` and a JCS hash of `payload` does not prove that the complete authenticated event envelope is unchanged. The same `event_id` and payload could otherwise be replayed with a changed `event_type`, `schema_version`, `occurred_at` or `correlation_id` and be incorrectly classified as a duplicate.

The candidate now keeps two hashes with distinct meanings:

- `payload_hash`: JCS SHA-256 of the payload object, preserving the existing Inbox field's payload-level meaning.
- `event_envelope_hash`: new SEM-only column, JCS SHA-256 of the complete authenticated envelope fields `event_id`, `event_type`, `schema_version`, `occurred_at`, `execution_id`, `execution_event_sequence`, optional `correlation_id` when present, and `payload`. HTTP signature headers and transport metadata are excluded.

The SEM CHECK requires a lowercase 64-character hex event_envelope_hash for source='sem' and NULL for non-SEM rows. Duplicate detection compares event_type, execution_id, sequence, payload_hash and event_envelope_hash. Added INBOX-29..31 for changed event_type, changed schema_version/occurred_at, and semantically identical JSON with different whitespace/member order.

This is a design correction only. The candidate DDL, migration replay, two-connection tests and bilateral hash vectors remain unratified; no migration or runtime code was changed.

             AND event_envelope_hash IS NOT NULL
             AND event_envelope_hash ~ '^[0-9a-f]{64}$'
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 10. Shared callback-envelope JCS vector (2026-11-25)

The SEM protocol repository now carries a deterministic cross-language callback vector:

- File: `tests/protocol/fixtures/platform-sem-callback-envelope-jcs-v1.json`
- Pinned fixture commit: `smark-sem@ad09195d44d557b5a5d174d6bdfdef687dcef4b5`
- Event type: `execution.infrastructure_uncertain`
- The fixture includes the complete authenticated envelope, including optional `correlation_id`, payload canonical bytes, full-envelope canonical bytes, payload SHA-256 and event-envelope SHA-256.

SEM's Python and Node validators independently canonicalize the same fixture and compare exact UTF-8 bytes and both hashes. AI-PM's Rust implementation must consume this exact fixture (pinned to the commit above until a ratified SEM protocol tag exists) and match all expected bytes/hashes. The vector is intentionally ASCII-only with integer values; it proves cross-language agreement for the complete-envelope shape, not the full Unicode/numeric edge matrix. INBOX-33 continues to require the broader RFC 8785 cases.

This is test-contract evidence only. No AI-PM canonicalizer dependency or production parser has been selected or added. Acceptance matrix now has 42 cases.
)
            OR
            (source <> 'sem'
             AND execution_id IS NULL
             AND execution_event_sequence IS NULL
             AND event_envelope_hash IS NULL)
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
        event_type <> 'sem_infrastructure_observed'
        OR (
            dedupe_tag IS NOT NULL
            AND length(dedupe_tag) BETWEEN 1 AND 200
        )
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

DDL qualifications before ratification:

- Confirm no existing Inbox rows use reserved source value \`sem\`.
- The proposed FK proves tenant + Run identity, but not project identity or \`run_kind='delivery'\`. Before freezing DDL, either add a reviewed composite key/FK proving tenant + project + execution together, or explicitly retain locked-row semantic validation as the enforcement point and prove it with integration tests.
- The callback must lock the Run and reject review runs before any write; a CHECK cannot enforce the referenced Run's run_kind.
- Decide whether to add a composite FK for SEM-only Run observations or retain the current ledger pattern with strict transaction-local validation.
- Do not add a redundant global unique index on \`inbound_event.id\`; the primary key already supplies it.



## 11. SEM hash CHECK three-valued-logic correction (2026-11-25)

A fresh SQL review found that the candidate CHECK used regular-expression predicates for `payload_hash` and `event_envelope_hash` without explicit `IS NOT NULL` predicates. PostgreSQL CHECK constraints accept both TRUE and UNKNOWN, so a NULL hash could pass when the surrounding expression evaluates to UNKNOWN. The original illustrative expression also omitted the closing parenthesis for the SEM branch before `OR`.

The candidate SQL is corrected to:
- Require `payload_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Require `event_envelope_hash IS NOT NULL` before its lowercase-hex predicate for `source='sem'`.
- Close the SEM branch before the non-SEM `OR` branch.
- Preserve the explicit `source IS NOT NULL` guard.

Added INBOX-36..38 for each NULL hash and a complete three-valued-logic/DDL-parse truth table. This correction is documentation-only. The SQL still requires actual PostgreSQL parse/replay and catalog verification; it is not a migration.

The required truth-table cases are:

| source | execution_id | sequence | payload_hash | event_envelope_hash | Expected |
|---|---|---:|---|---|---|
| `sem` | non-NULL | positive | valid 64-hex | valid 64-hex | Accept |
| `sem` | NULL | positive | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | NULL / zero | valid 64-hex | valid 64-hex | Reject |
| `sem` | non-NULL | positive | NULL | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | NULL | Reject |
| `sem` | non-NULL | positive | malformed | valid 64-hex | Reject |
| `sem` | non-NULL | positive | valid 64-hex | malformed | Reject |
| non-`sem` | NULL | NULL | existing generic value | NULL | Accept |
| non-`sem` | non-NULL | NULL | existing generic value | NULL | Reject |
| non-`sem` | NULL | NULL | existing generic value | non-NULL | Reject |
| NULL | any | any | any | any | Reject at the existing column NOT NULL constraint before the SEM CHECK |

The test must execute these rows against PostgreSQL itself and assert insert success/failure plus the expected constraint for each rejected SEM-specific row. The NULL-source row is rejected by the existing column NOT NULL constraint before this CHECK. A reimplementation of the Boolean expression in application code is not a substitute.



The shared vector also exercises optional-field identity: its `correlation_id` is present. Independent validators prove that changing its value or omitting it changes the full-envelope hash while leaving the payload hash unchanged. This ensures optional schema fields are included when present and absent fields are not synthesized.


## 10. Canonicalizer dependency selection is not yet ratified

The separate evaluation document `sem-jcs-canonicalizer-dependency-evaluation.md` identifies `canonprims-json 0.1.4` as the leading technical candidate for an isolated AI-PM-only spike. It accepts raw bytes, rejects duplicate members and invalid I-JSON values before producing canonical bytes, and provides explicit limits and a shared conformance corpus. It requires Rust 1.88.0; AI-PM's current Docker builder floats on `rust:1-slim` and the workspace does not declare an MSRV. SEM's candidate workspace remains at Rust 1.85 and need not change if the JCS dependency stays AI-PM-only.

No dependency or toolchain change is approved. Before implementation, the spike must prove corpus parity, cross-language callback envelope hashes, refusal/no-side-effect behavior, and dependency provenance. The Rust 1.88 AI-PM build floor must be explicitly ratified before adding the crate.

## 11. Conflict classification precedence correction (2026-11-25)

The prior post-conflict text was ambiguous: it said a one-sided lookup result could be an integrity failure, while also requiring an existing global event_id with a changed execution tuple to return IDEMPOTENCY_CONFLICT. A cross-execution collision can legitimately produce exactly that one-sided event_id lookup.

The algorithm now has explicit precedence:

1. If the global event_id exists and any authenticated tuple/hash differs, return 409 IDEMPOTENCY_CONFLICT. This wins even if the incoming execution/sequence is also owned by another row.
2. If the global event_id exists and the tuple/hash matches exactly, the execution/sequence lookup MUST resolve to the same Inbox row; otherwise this is a database invariant failure and no ACK is allowed.
3. If no global event_id exists but the execution/sequence key is owned, return 409 EVENT_SEQUENCE_CONFLICT.
4. If ON CONFLICT reports no insert but neither key can be found, rollback and return generic integrity failure.

Added INBOX-40..44 for cross-key conflict precedence, exact-duplicate row consistency, sequence-only conflict, non-SEM global event_id collisions and Run-derived tenant/project integrity. This is a design correction; no migration or runtime code changed.


## 12. Run-observation row contract and Inbox linkage

The existing `delivery.task_run_event` table has required denormalized columns `tenant_id`, `project_id`, `task_id`, `run_id`, `client_id`, `event_type`, `lease_generation`, `dedupe_tag`, and nullable `detail`. A SEM callback cannot insert only `run_id/event_type/dedupe_tag/detail`; it must populate the full row.

The SEM observation mapping is now frozen at Draft level in transaction step 6. The key distinction is:

- `task_run_event.lease_generation` records the AI-PM Run lease generation observed when the callback was accepted.
- `detail.binding_epoch` records the SEM infrastructure binding epoch.
- They are different authorities and are never interchangeable.

The SEM-only CHECK now requires an object detail with event_id, execution_event_sequence, sem_event_type, event_envelope_hash and binding_epoch; the sequence must equal the decimal dedupe_tag and the envelope hash must be lowercase 64-character hex. The unique partial index on (run_id, dedupe_tag) prevents duplicate observations across lease generations.

The Draft deliberately retains transaction-local Inbox↔observation linkage rather than adding a source_event_id FK/column. On first insert, Inbox, observation and Run cursor update commit atomically. On duplicate, the receiver verifies the existing observation's event_id and event_envelope_hash before ACK. INBOX-45..48 cover missing/invalid detail, denormalized Run mapping, and broken duplicate linkage. This choice avoids additional retention coupling but requires real PostgreSQL two-connection and crash-atomicity proof.
