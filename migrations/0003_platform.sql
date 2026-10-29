-- 0003_platform: integration / ai / governance schemas (architecture doc §7.1–7.4, §11.2)
-- with V2 deltas: integration.notification, proposal_seq, self_review, confidence,
-- executed_audit_id, rollback_failed intervention, compensation_command, skill seed.

-- §7.2 inbound events
CREATE TABLE integration.inbound_event (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    actor_id UUID NOT NULL REFERENCES core.principal(id),
    source TEXT NOT NULL,
    event_type TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    payload_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'received',
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, source, idempotency_key),
    UNIQUE (project_id, id)
);

-- §11.2 job queue
CREATE TABLE integration.job (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    event_id UUID NOT NULL
        REFERENCES integration.inbound_event(id),
    job_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (event_id, job_type)
);

-- V2: supervision-desk notification stream
CREATE TABLE integration.notification (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    source_proposal_id UUID UNIQUE,
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    veto_window_until TIMESTAMPTZ,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- §7.1 AI-PM configuration
CREATE TABLE ai.project_ai_config (
    project_id UUID PRIMARY KEY REFERENCES core.project(id),

    ai_principal_id UUID NOT NULL
        REFERENCES core.principal(id),

    model_provider TEXT NOT NULL,
    model_name TEXT NOT NULL,
    model_config JSONB NOT NULL DEFAULT '{}',

    strategy TEXT NOT NULL DEFAULT 'single_strong_model'
        CHECK (strategy = 'single_strong_model'),

    automation_mode TEXT NOT NULL DEFAULT 'ai_lead'
        CHECK (automation_mode IN ('ai_lead', 'controlled')),

    policy_version INT NOT NULL DEFAULT 1,
    config_revision BIGINT NOT NULL DEFAULT 1,

    enabled BOOLEAN NOT NULL DEFAULT true,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ai.skill_definition (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    current_version TEXT NOT NULL
);

CREATE TABLE ai.project_skill_binding (
    project_id UUID NOT NULL
        REFERENCES ai.project_ai_config(project_id),
    skill_id TEXT NOT NULL
        REFERENCES ai.skill_definition(id),
    skill_version TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    settings JSONB NOT NULL DEFAULT '{}',
    PRIMARY KEY (project_id, skill_id)
);

-- Seed the five V2 skills.
INSERT INTO ai.skill_definition (id, name, description, current_version) VALUES
    ('requirement_analysis', 'Requirement Analysis',
     'Analyze raw customer needs and requirement revisions for ambiguity, gaps and conflicts.', 'v1'),
    ('task_analysis', 'Task Analysis',
     'Decompose approved requirements into tasks with estimates and dependencies.', 'v1'),
    ('risk_detection', 'Risk Detection',
     'Detect schedule, scope and quality risks across requirements and tasks.', 'v1'),
    ('architecture_consistency', 'Architecture Consistency',
     'Check changes against documented architecture decisions and constraints.', 'v1'),
    ('scheduling_orchestration', 'Scheduling Orchestration',
     'Orchestrate milestones and task scheduling within capacity constraints.', 'v1');

-- §7.2 AI decision records
CREATE TABLE ai.analysis_run (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    event_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    model_name TEXT NOT NULL,
    policy_version INT NOT NULL,
    config_snapshot JSONB NOT NULL,
    context_snapshot JSONB,
    self_review JSONB,
    result JSONB,
    failure_code TEXT,
    attempt_count INT NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, event_id)
        REFERENCES integration.inbound_event(project_id, id)
);

CREATE TABLE ai.change_proposal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    analysis_run_id UUID NOT NULL,
    proposal_seq INT NOT NULL,
    target_type TEXT NOT NULL,
    target_id UUID,
    expected_revision BIGINT,
    command_type TEXT NOT NULL,
    proposed_data JSONB NOT NULL,
    rationale TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '[]',
    risk_level TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'medium'
        CHECK (confidence IN ('low', 'medium', 'high')),
    status TEXT NOT NULL DEFAULT 'proposed'
        CHECK (status IN (
            'proposed', 'executed', 'redline_pending',
            'escalated', 'vetoed', 'expired', 'failed'
        )),
    executed_at TIMESTAMPTZ,
    executed_audit_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (analysis_run_id, proposal_seq),
    FOREIGN KEY (project_id, analysis_run_id)
        REFERENCES ai.analysis_run(project_id, id)
);

-- §7.3 red-line confirmation, human intervention, audit
CREATE TABLE governance.approval_request (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    proposal_id UUID NOT NULL UNIQUE
        REFERENCES ai.change_proposal(id),
    status TEXT NOT NULL DEFAULT 'pending',
    required_role TEXT NOT NULL,
    decided_by UUID REFERENCES core.principal(id),
    decision_reason TEXT,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE governance.human_intervention (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    intervention_type TEXT NOT NULL CHECK (
        intervention_type IN (
            'veto', 'rollback', 'freeze', 'unfreeze',
            'mode_switch', 'policy_change', 'rollback_failed'
        )
    ),
    target_proposal_id UUID
        REFERENCES ai.change_proposal(id),
    target_command_id UUID,
    actor_id UUID NOT NULL REFERENCES core.principal(id),
    reason TEXT NOT NULL,
    executed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE governance.audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    actor_id UUID NOT NULL REFERENCES core.principal(id),
    action TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id UUID,
    before_data JSONB,
    after_data JSONB,
    compensation_command JSONB,
    compensated_command_id UUID,
    source_event_id UUID,
    analysis_run_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ai.agent_feedback (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    recipient_id UUID NOT NULL REFERENCES core.principal(id),
    related_event_id UUID NOT NULL
        REFERENCES integration.inbound_event(id),
    feedback_type TEXT NOT NULL,
    content JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'available',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- §7.4 recommended indexes
CREATE INDEX idx_task_project_status
ON delivery.task(project_id, status);

CREATE INDEX idx_requirement_project
ON delivery.requirement(project_id, status);

CREATE INDEX idx_event_project_time
ON integration.inbound_event(project_id, received_at DESC);

CREATE INDEX idx_analysis_status_time
ON ai.analysis_run(status, created_at);

CREATE INDEX idx_proposal_project_status
ON ai.change_proposal(project_id, status);

CREATE INDEX idx_proposal_executed_time
ON ai.change_proposal(project_id, executed_at DESC)
WHERE executed_at IS NOT NULL;

CREATE INDEX idx_intervention_project_time
ON governance.human_intervention(project_id, created_at DESC);

CREATE INDEX idx_notification_project_time
ON integration.notification(project_id, created_at DESC);

-- §11.2 claim index for the job queue
CREATE INDEX idx_job_claim
ON integration.job(status, available_at)
WHERE status = 'queued';
