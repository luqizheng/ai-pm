# ART-04B — Visualization ↔ Generic Artifact Mapping and Cutover Design

> **Status:** Architecture proposal / Draft; no implementation approval.
> **Date:** 2026-11-25
> **Repository baseline:** `master` at `d0e72fa6805aebeadf1b282e430deba493072fd6`
> **Related:** ART-01 §9, ART-04 / ART-04A in `smark-artifact-service`, `docs/visualization/ai-pm-viz-27-artifact-service-integration-v1.md`
> **Scope:** AI-PM-side identity mapping, durable orchestration, and migration/cutover boundaries. This document does not change schema or code.

## 1. Executive decision

The existing `viz.artifact` is a **Visualization business-version record**, not the generic logical Artifact resource.

The target mapping is:

- One AI-PM Visualization slot (`tenant_id + project_id + slot_key`) maps to one generic Artifact Service `artifact_id`.
- Each existing AI-PM `viz.artifact.id` maps to exactly one generic Artifact Service `version_id` under that logical Artifact.
- AI-PM retains the Visualization business record, VizSpec, source/provenance semantics, approval state machine, slot policy, and Live Graph.
- Artifact Service owns the generic Artifact, immutable Version, Output metadata, GenerationJob, and technical READY state.
- AI-PM persists the exact cross-service identity mapping. An ArtifactReference is only an Artifact-level relationship and is not a substitute for the version mapping.

This is consistent with ART-01 §9, but adds the missing AI-PM-side persistence and recovery detail.

## 2. Repository facts (Observed)

Static inspection of AI-PM `master` confirms:

### 2.1 Existing schema

Migration `migrations/0020_visualization_artifact.sql` creates four tables:

| Table | Current meaning |
|---|---|
| `viz.artifact` | One Visualization business record per slot version; stores tenant/project, slot_key, version_no, business state, nature, full VizSpec JSONB, spec_hash, source_hash, provenance, renderer metadata |
| `viz.approval` | Separate business approval state machine; FK to `viz.artifact.id`; anchors target_hash + expected_version_no |
| `viz.render_output` | Output rows keyed by business artifact and format; stores HTML/SVG text or PNG bytes and content hash |
| `viz.slot_config` | Visualization-specific slot policy; require_approval, auto_stale, allowed_nature, max_versions, retention_days |

Important constraints:
- `UNIQUE(project_id, slot_key, version_no)` makes `viz.artifact` a per-slot version record.
- `viz.approval.artifact_id` and `viz.render_output.artifact_id` reference the business record, not a generic Artifact Service resource.
- At most one pending approval per project/slot is enforced by a partial unique index.
- Published VizSpec is immutable; published may become stale; stale is terminal.
- `viz.render_output` has `UNIQUE(artifact_id, format)`; the current renderer uses `ON CONFLICT DO UPDATE` for the same business record/format.

The 2026-11-24 ART-02E schema audit reports zero rows in all four tables at that audit time. This is historical evidence only; row counts must be rechecked before any migration or cutover.

### 2.2 Existing code paths

Observed in the current repository:
- `crates/pm-application/src/visualization/artifact_service.rs` creates business draft records and allocates `version_no` per project/slot.
- `apps/pm-api/src/viz_handlers.rs` exposes create/list/detail/approval/refresh APIs over the legacy business record.
- `crates/pm-application/src/visualization/render_job.rs` calls a RenderAdapter, performs SVG safety inspection, and writes `viz.render_output`; it updates the same artifact+format row on conflict.
- `crates/pm-infrastructure/src/viz_seecode_adapter.rs` contains the existing AI-PM-side SeeCode adapter and sandbox integration.
- `viz.slot_config`, VizSpec, Relation Registry, source_hash, and Live Graph remain Visualization business concerns.

These paths are the legacy implementation to be replaced at the controlled cutover; they must not run in parallel as an indefinite second generation authority.

## 3. Identity model and mapping

### 3.1 Identities must remain distinct

| Identity | Owner | Meaning |
|---|---|---|
| `viz.artifact.id` | AI-PM | Business Visualization version record; approval and Viewer identity |
| `artifact_id` | Artifact Service | Logical artifact container for one Visualization slot |
| `version_id` | Artifact Service | Immutable technical/file version produced for one business Visualization record |
| `generation_job_id` | Artifact Service | One technical generation request |
| `viz.approval.id` | AI-PM | Business approval record; never becomes a GenerationJob or ArtifactReference |

Do not rename, repurpose, or replace `viz.artifact.id` with `artifact_id`. Do not use Artifact.current_version_id as a substitute for a specific business-record-to-Version mapping.

### 3.2 Required AI-PM persistence

The following are proposed new AI-PM tables. Names are provisional; the constraints and semantics are normative.

#### A. `viz.artifact_service_slot_binding`

One row per Visualization slot:

- `tenant_id`, `project_id`, `slot_key`;
- `artifact_service_artifact_id`;
- `created_at`, `updated_at`.

Required constraints:
- UNIQUE (`tenant_id`, `project_id`, `slot_key`);
- UNIQUE (`artifact_service_artifact_id`);
- all scope columns immutable after binding;
- no cross-database FK to Artifact Service.

This table is the durable local lookup from a Visualization slot to its generic Artifact. It prevents each new Visualization version from creating a new generic Artifact.

#### B. `viz.artifact_service_version_binding`

One row per AI-PM business Visualization version:

- `viz_artifact_id` (PK/FK to `viz.artifact.id`);
- `tenant_id`, `project_id`, `slot_key`, `legacy_version_no`;
- `artifact_service_artifact_id`, `artifact_service_version_id`;
- `generation_job_id` (nullable until a Job is known);
- `created_at`, `bound_at`.

Required constraints:
- UNIQUE (`artifact_service_version_id`);
- UNIQUE (`tenant_id`, `project_id`, `slot_key`, `legacy_version_no`);
- UNIQUE (`generation_job_id`) WHERE `generation_job_id` IS NOT NULL, because one GenerationJob result may bind to only one AI-PM business-version record;
- the bound artifact_id must equal the slot binding's artifact_id (enforced by AI-PM transaction/constraint design);
- once a non-null version_id is bound, it is immutable;
- no cross-service FK; the SDK/API reconciliation path verifies remote existence and scope.

This is the authoritative business-version ↔ generic-Version mapping. It preserves the legacy business ID for existing approval and UI references.

#### C. `viz.artifact_service_operation`

One row per outbound generation operation (not merely one per Task or slot):

- local `operation_id` UUID;
- `viz_artifact_id`;
- stable `caller_request_id` (same identity sent to Artifact Service);
- `idempotency_key`;
- canonical request hash / immutable request reference;
- `artifact_service_artifact_id`, `reference_id`, `generation_job_id`, `target_version_id` (nullable as steps progress);
- operation state, last error code, retry/reconciliation timestamps;
- created/updated timestamps.

Required constraints:
- UNIQUE (`caller_request_id`);
- unique active operation policy per business record must be explicit; terminal failed/cancelled operations may be followed by a new operation with a new caller_request_id;
- persist the operation before the first remote call;
- update each remote identifier and operation state in a local transaction;
- operation state is an AI-PM recovery record, not a mirror that can write Artifact Service Job state.

This table is the AI-PM saga ledger required by ART-04 §4/AR04-05. It allows restart recovery without distributed transactions.

#### D. Output mapping (conditional)

If legacy `viz.render_output` contains rows at migration time, add `viz.artifact_service_output_binding` mapping each legacy output ID to the new ArtifactOutput ID and file-service object ID, with the original format/hash/size. If the table is empty at cutover, no historical output mapping rows are needed; future output identity is read from the Artifact Service Version manifest/Output API.

Do not store file bytes in the new mapping tables.

## 4. Artifact key and uncertain logical Artifact creation

The logical Artifact is per slot, not per Visualization version.

Preferred stable key:
- `artifact_key = "viz-slot:" + lowercase_hex_sha256(UTF8(slot_key))`;
- the key is scoped by tenant/project under the Artifact Service uniqueness contract;
- a changed slot_key creates a different logical Artifact; old Artifact is archived only under an explicit retention policy.

ART-04A now proposes an exact-match artifact_key filter and same-key create convergence; this remains unimplemented and requires independent approval. The recovery contract must also handle the current create implementation's reserve → insert → complete sequence: a crash after Idempotency-Key reservation but before Artifact insertion can leave a stale in-progress reservation while lookup returns no Artifact. ART-04A now requires stable artifact_key recovery to bypass that stale response-cache reservation. The stable artifact_key is the logical-slot correctness boundary; GenerationJob caller_request_id remains the per-generation-operation identity.

## 5. Reference semantics

ART-04B does **not** create a per-`viz.artifact` ArtifactReference in V1. The proposed owner_type=`viz_artifact` is invalid: Artifact Service currently accepts only `requirement`, `design`, `adr`, `task`, `knowledge`, and `project`. AI-PM must not invent a generic owner_type value or mislabel a Visualization business record as another business entity.

The exact version-level relation is exclusively the AI-PM `viz.artifact_service_version_binding` row. Business approval remains `viz.approval` → `viz.artifact.id`.

If a project-level generic Reference is later judged useful, it may use the already-supported `owner_type=project` with the stable project ID and an ART-01-approved relation. It is optional metadata only and is not a per-version association or an approval record.

Additionally, current OpenAPI text says duplicate active Reference creation returns the existing record, but current application code performs a plain INSERT and the API handler returns 201 only for a successful insert; a duplicate active tuple raises a uniqueness error. Until Artifact Service implements create-or-return-existing semantics or an exact reconciliation endpoint, AI-PM must not assume Reference creation is idempotent. Reference is therefore omitted from the mandatory generation saga.

## 6. State and authority mapping

| Legacy AI-PM state | Artifact Service state | Rule |
|---|---|---|
| `draft` | Version may be `READY` | Technical generation can be ready before business approval; no automatic publication |
| `published` | Version remains `READY` | Human business approval remains AI-PM-only |
| `stale` | Version remains `READY` | Stale is business freshness, not file corruption or technical failure |
| `rejected` | Version may remain `READY` | Rejection is a business decision; do not delete/quarantine an otherwise valid file |
| no business record / generation failed | Job terminal failure; Version may be absent or non-ready | Never fabricate a successful business version |

Artifact Service `current_version_id` is a technical convenience pointer to a READY Version. It is not the published Visualization pointer and must not drive business approval or Viewer visibility. AI-PM selects the business-visible record through its existing slot/approval semantics and resolves its exact Version through the mapping table.

Consequences:
- A draft Version may be technically READY while remaining invisible to published-only business surfaces.
- A stale business record may point to a technically READY Version.
- A rejected business record does not change the technical Version state.
- Artifact Service never receives `published`, `stale`, `rejected`, `nature`, or approval status as its own resource lifecycle state.

## 7. Generation orchestration saga

For a new business Visualization record:

1. AI-PM validates VizSpec, business references, project authorization, source hashes, nature and slot policy.
2. AI-PM creates the `viz.artifact` business record (or uses an existing record created by the approved product flow).
3. AI-PM creates and commits `viz.artifact_service_operation` with a new caller_request_id, Idempotency-Key, request hash and immutable input reference **before** calling Artifact Service.
4. Resolve the slot binding. If absent, create/find the generic Artifact by stable artifact_key and persist the returned ID. If the response is uncertain, use artifact_key lookup; never mint a second logical Artifact.
5. Do not create a per-business-record ArtifactReference in V1. If a project-level Reference is explicitly required, treat it as optional metadata and do not block generation on it; its replay semantics must be resolved before use.
6. Submit GenerationJob using the operation's caller_request_id and Idempotency-Key. If response is uncertain, use ART-04A lookup by caller_request_id; never create a replacement operation.
7. Persist `generation_job_id`; poll asynchronously and reconcile after AI-PM process restart.
8. On Job success, require non-null exact `target_version_id`, fetch that Version, verify same artifact/scope and READY, then persist the version binding.
9. AI-PM continues its own business workflow: create/request/decide `viz.approval` as required; Artifact Service success never auto-publishes the Visualization.
10. If AI-PM crashes between any two calls, resume from the operation row and repeat only a step with a proven idempotent/reconciliation contract. Artifact create uses stable artifact_key recovery (including stale Idempotency-Key handling); GenerationJob create uses caller_request_id. Do not retry Reference create until its replay/reconciliation contract is fixed.

Failure handling:
- Artifact created, optional project-level Reference failed: retain Artifact; generation may proceed because Reference is not the exact version mapping and is not a required saga step.
- Job create uncertain: lookup by caller_request_id; do not create a new caller_request_id.
- Job failed/timed_out/cancelled: close the operation according to AI-PM policy; a deliberate new generation is a new operation with a new caller_request_id.
- Cancel requested while running: AI-PM remains waiting until Artifact Service reports terminal state; `cancel_requested` is not `cancelled`.
- Job succeeded but business record was later rejected/cancelled: retain the immutable Version and record the business decision in AI-PM; never rewrite Job history.

No distributed transaction, cross-database write, shared SQL lock, RabbitMQ dependency, or indefinite dual-write is introduced.

## 8. Legacy output and generation path cutover

Before cutover:
- AI-PM still contains the legacy SeeCode Adapter, sandbox, SVG inspection and `render_job::render_artifact` code path. Static repository review has not established a production caller for `render_artifact`; do not describe this path as an actively used production executor without fresh call-site/runtime evidence.
- The legacy renderer upserts `viz.render_output` by (business artifact, format); that behavior is incompatible with immutable ArtifactVersion Output once an external Version has been committed.
- At the approved authority switch, the Artifact Service Worker becomes the sole generation executor. AI-PM must not locally render and externally generate the same business operation.

Cutover prerequisites:
1. Artifact Service API/SDK contract and ART-04A are approved and implemented.
2. File Service upload/finalize, LMMS delegation, SeeCode Adapter and Worker/runtime gates are closed.
3. AI-PM mapping tables, operation ledger, client adapter and consumer contract tests are implemented.
4. Viewer reads use AI-PM business state + exact version binding; output bytes/URLs are resolved through Artifact Service/file-service.
5. New generation requests are frozen during the authority switch; no concurrent old/new writers.

After cutover:
- AI-PM remains the sole writer of `viz.artifact`, `viz.approval`, and `viz.slot_config` business semantics until a separate retirement decision.
- Artifact Service is the sole writer of generic Artifact/Version/Output/GenerationJob.
- `viz.render_output` writes and local render execution are disabled; old rows remain read-only during the rollback/retention window.
- Live Graph and business source projection remain unchanged.

## 9. No historical data backfill and controlled cutover

The product-owner decision recorded in ART-02F is authoritative: AI-PM has no real users and no real business data; historical `viz.*` data migration/backfill, old-ID mapping, compatibility mode and dual-write are not part of this product cutover. The earlier zero-row audit is supporting historical evidence, not a substitute for a fresh pre-cutover check.

Before cutover:
- Re-check the target database schema and row counts for all legacy `viz.*` tables immediately before the authority switch.
- Expected state is no real business rows. If unexpected non-test/business rows are found, stop the cutover and investigate their origin and ownership. Do not silently discard them, and do not automatically introduce a backfill project that contradicts ART-02F; obtain a new explicit product decision first.
- Test/fixture data is handled by the test-environment reset/cleanup policy, not migrated into Artifact Service.
- Verify all old handlers, queries, SQL, workers, tests and other code dependencies have been removed or deliberately retired before dropping legacy tables in a later AI-PM migration.
- Freeze new generation requests during the authority switch; no concurrent old/new writers.

Rollback is not a second writer mode:
- Freeze new external generation and wait/reconcile all in-flight Jobs.
- Keep successful Artifact Service Versions and AI-PM mapping/operation records; do not delete them to imitate a rollback.
- Restore an old AI-PM read path only if its underlying legacy data is still complete and authoritative; this is not permission to resume the legacy renderer for operations already owned by Artifact Service.
- Reconcile any completed external Job whose AI-PM business operation was not finalized before reopening writes.
- If the new path must be disabled after cutover, keep the external technical records intact and require an explicit recovery decision before any generation authority is switched again.

## 10. Open decisions / implementation gates

1. **Artifact lookup by artifact_key** — ART-04A proposes an exact-match filter and same-key create convergence; implementation and SDK support must be reviewed, including stale Idempotency-Key reservation recovery, before the AI-PM slot-binding saga is implementable.
2. **VizSpec compiler boundary** — keep business-aware VizSpec validation/compilation in AI-PM and freeze the exact versioned SeeCode input envelope consumed by Artifact Service; the existing compiler currently returns a bare SeeCode spec, so the integration must add the smark.seecode.input envelope wrapper and contract tests. No VizSpec or business relation semantics may leak into the generic service.
3. **Reference semantics** — per-viz-artifact Reference is excluded because owner_type is not supported; only consider optional project-level Reference with owner_type=project, and only after create replay/reconciliation semantics are implemented and tested.
4. **Output compatibility** — define how legacy HTML/PNG rows, if any, are copied and surfaced when the new SeeCode path is SVG-only.
5. **Business version numbering** — preserve legacy version_no in AI-PM; do not assume Artifact Service version_number will equal it.
6. **Mapping retention** — keep legacy ID mappings for at least the lifetime of AI-PM approvals, audit history, and external references.
7. **Current pointer semantics** — generic current_version_id is not the business-published Visualization pointer.
8. **Local atomicity** — business draft/operation-ledger persistence must be atomic before remote calls; current helper transaction boundaries need adjustment.
9. **Legacy call-path inventory** — trace all real production and conversation-triggered callers of the compiler/adapter/render helper; static search found no confirmed production caller of render_artifact, and migration 0020 has no durable legacy render-job table.

## 11. Non-goals

This document does not:
- alter existing `viz.*` tables or create migrations;
- implement Artifact Service SDK/consumer adapter;
- migrate existing rows or copy files;
- change Viewer/UI contracts;
- move VizSpec, Relation Registry, Live Graph, business authorization, approval, or slot policy into Artifact Service;
- authorize SeeCode production generation;
- close Artifact Service LMMS delegation, File Service finalize, or ART-03B-02/03 runtime blockers.

**Status remains Draft.** The mapping is a design proposal to be reviewed against ART-01 and ART-04 before any implementation.
