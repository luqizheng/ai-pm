-- SEM Callback Inbox acceptance harness — TEST-ONLY fixture schema.
--
-- This file is NOT an ai-pm migration and MUST never run against any shared
-- or production database. The harness creates a dedicated ephemeral database
-- and applies this file there, then drops the database after the run.
--
-- It mirrors, as closely as practical:
--   * migrations/0001_baseline.sql  — delivery.task_run, integration.inbound_event
--     (the exact columns/constraints the callback contract depends on),
--   * migrations/0013_task_run_event.sql — delivery.task_run_event including the
--     REPLACED event-type CHECK (8 original types + sem_infrastructure_observed),
--   * the candidate §14 DDL — delivery.task_run_sem_execution,
--   * the candidate §3 DDL additions — task_run cursor column, inbound_event
--     SEM columns/CHECK/partial index/FK.
--
-- core.project / full core.principal are reduced to the minimum the FK graph
-- needs; the harness does not exercise them.

CREATE SCHEMA core;
CREATE SCHEMA delivery;
CREATE SCHEMA integration;

CREATE TABLE core.principal (
    id uuid PRIMARY KEY,
    kind text NOT NULL
);

CREATE TABLE delivery.task_run (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    task_id uuid NOT NULL,
    attempt_no integer NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL
        CONSTRAINT task_run_status_check CHECK (
            status IN ('queued','claimed','running','succeeded','failed','timed_out','cancelled')),
    run_kind text DEFAULT 'delivery'::text NOT NULL
        CONSTRAINT task_run_run_kind_check CHECK (run_kind IN ('delivery','review')),
    lease_generation integer DEFAULT 0 NOT NULL,
    lease_client_id uuid,
    -- candidate §3 cursor column
    execution_event_sequence bigint NOT NULL DEFAULT 0
        CONSTRAINT task_run_execution_event_sequence_nonnegative CHECK (execution_event_sequence >= 0),
    CONSTRAINT task_run_pkey PRIMARY KEY (id)
);

CREATE UNIQUE INDEX uq_task_run_tenant_project_execution
    ON delivery.task_run (tenant_id, project_id, id);

CREATE TABLE integration.inbound_event (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    actor_id uuid NOT NULL REFERENCES core.principal(id),
    source text NOT NULL,
    event_type text NOT NULL,
    idempotency_key text NOT NULL,
    payload jsonb NOT NULL,
    payload_hash text NOT NULL,
    status text DEFAULT 'received'::text NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    -- candidate §3 SEM columns
    execution_id uuid,
    execution_event_sequence bigint,
    event_envelope_hash text,
    CONSTRAINT inbound_event_pkey PRIMARY KEY (id),
    CONSTRAINT uq_inbound_event_project_id UNIQUE (project_id, id),
    CONSTRAINT uq_inbound_event_tenant_id UNIQUE (tenant_id, id),
    CONSTRAINT uq_inbound_event_project_source_idem UNIQUE (project_id, source, idempotency_key),
    -- candidate §3 source-shape CHECK (NULL-safe grouping, 2026-11-25 revision)
    CONSTRAINT inbound_event_sem_sequence_pair CHECK (
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
    )
);

CREATE UNIQUE INDEX uq_inbound_event_sem_execution_sequence
    ON integration.inbound_event (execution_id, execution_event_sequence)
    WHERE source = 'sem';

ALTER TABLE integration.inbound_event
    ADD CONSTRAINT fk_inbound_event_sem_execution
    FOREIGN KEY (tenant_id, project_id, execution_id)
    REFERENCES delivery.task_run (tenant_id, project_id, id);

-- delivery.task_run_event: exact 0013 shape with the REPLACED event-type
-- allow-list (8 original + sem_infrastructure_observed) and the candidate §3
-- SEM sequence CHECK + partial unique index.
CREATE TABLE delivery.task_run_event (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    client_id uuid,
    event_type text DEFAULT '' NOT NULL,
    lease_generation integer DEFAULT 0 NOT NULL,
    dedupe_tag text DEFAULT '' NOT NULL,
    occurred_at timestamp with time zone DEFAULT now() NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    detail jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_run_event_type_check CHECK ((event_type = ANY (ARRAY[
        'run_claimed'::text,
        'run_started'::text,
        'run_stage_changed'::text,
        'run_finished'::text,
        'command_issued'::text,
        'command_delivered'::text,
        'command_acked'::text,
        'command_failed'::text,
        'sem_infrastructure_observed'::text
    ]))),
    CONSTRAINT task_run_event_generation_check CHECK ((lease_generation >= 0)),
    CONSTRAINT task_run_event_dedupe_tag_len CHECK ((length(dedupe_tag) <= 200)),
    CONSTRAINT task_run_event_sem_sequence_check CHECK (
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
            -- binding_epoch is required-and-positive for the five binding-scoped
            -- events and MUST be absent/null for execution.unbound_release_applied
            -- (P0-07 tombstone: fabricating an epoch is forbidden).
            -- 2026-11-28 harness finding: the earlier blanket requirement
            -- rejected every valid sixth-event observation.
            AND (
                (
                    COALESCE(detail->>'sem_event_type', '') = 'execution.unbound_release_applied'
                    AND COALESCE(detail->>'binding_epoch', '') = ''
                )
                OR COALESCE(detail->>'binding_epoch', '') ~ '^[1-9][0-9]*$'
            )
        )
        OR event_type <> 'sem_infrastructure_observed'
    )
);

CREATE UNIQUE INDEX uq_task_run_event
    ON delivery.task_run_event (run_id, event_type, lease_generation, dedupe_tag);

CREATE UNIQUE INDEX uq_task_run_event_sem_sequence
    ON delivery.task_run_event (run_id, dedupe_tag)
    WHERE event_type = 'sem_infrastructure_observed';

-- Candidate §14 canonical single-table execution projection.
CREATE TABLE delivery.task_run_sem_execution (
    task_run_id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    dispatch_state text NOT NULL,
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
    host_isolation_disposition_id uuid,
    dispatch_fence_id uuid,
    binding_event_id uuid,
    binding_event_sequence bigint,
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
    CONSTRAINT task_run_sem_execution_process_identity_check CHECK (
      (launch_id IS NULL AND containment_id IS NULL
        AND process_start_identity IS NULL AND systemd_invocation_id IS NULL)
      OR
      (launch_id IS NOT NULL AND containment_id IS NOT NULL
        AND length(containment_id) > 0
        AND process_start_identity IS NOT NULL
        AND systemd_invocation_id IS NOT NULL)
    ),
    CONSTRAINT task_run_sem_execution_release_evidence_check CHECK (
      dispatch_state <> 'released'
      OR process_exit_evidence_id IS NOT NULL
      OR stop_fence_id IS NOT NULL
    ),
    CONSTRAINT task_run_sem_execution_host_isolation_check CHECK (
      dispatch_state <> 'host_isolated'
      OR (host_isolation_disposition_id IS NOT NULL
        AND process_exit_evidence_id IS NULL
        AND stop_fence_id IS NULL)
    ),
    CONSTRAINT task_run_sem_execution_exit_evidence_state_check CHECK (
      process_exit_evidence_id IS NULL
      OR dispatch_state IN ('release_pending', 'released')
    ),
    CONSTRAINT task_run_sem_execution_stop_fence_state_check CHECK (
      stop_fence_id IS NULL
      OR dispatch_state IN ('bound', 'release_pending', 'released')
    ),
    CONSTRAINT task_run_sem_execution_sequence_check CHECK (
      last_event_sequence >= 0
      AND (binding_event_sequence IS NULL OR last_event_sequence >= binding_event_sequence)
    )
);
