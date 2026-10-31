-- 0009_task_state_machine: Run / 等待 / 取代关系 / 任务级策略覆盖 / Outbox
--                          + 13 值 status 约束与默认值（新系统，无回填）
-- 规格：docs/ai-pm-task-state-machine-v1.md §11.5 §12.4 §19.5 §20.4 §21.1

ALTER TABLE delivery.task
    ADD COLUMN superseded_by_task_id UUID,
    ADD COLUMN attempt_count INT NOT NULL DEFAULT 0,
    ADD COLUMN consecutive_failure_count INT NOT NULL DEFAULT 0,
    ADD COLUMN last_failure_category TEXT,
    ADD COLUMN retry_exhausted BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE delivery.task
    ADD CONSTRAINT fk_task_superseded_by
    FOREIGN KEY (project_id, superseded_by_task_id)
    REFERENCES delivery.task(project_id, id);

-- §1.4 / §19.5：执行记录另立归属，重试创建新 Run，不覆盖旧记录。
CREATE TABLE delivery.task_run (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    task_id UUID NOT NULL,
    attempt_no INT NOT NULL CHECK (attempt_no >= 1),
    stage TEXT, -- Q9.1 未裁阶段清单，先留空
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'timed_out', 'cancelled')),
    outcome TEXT
        CHECK (outcome IN ('succeeded', 'failed', 'timed_out', 'cancelled')),
    timeout_seconds INT,
    -- §15.5 Run 级策略快照（创建时写入，之后不再变更）
    policy_snapshot JSONB,
    policy_version INT,
    override_version BIGINT,
    policy_hash TEXT,
    evaluated_at TIMESTAMPTZ,
    repository_policy_snapshots JSONB,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_id, attempt_no),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id)
);
CREATE INDEX idx_task_run_task_status ON delivery.task_run(task_id, status);
CREATE INDEX idx_task_run_active ON delivery.task_run(task_id)
    WHERE status IN ('queued', 'running');

-- §19.5：失败记录结构化且不可变；每 Run 至多一条。
CREATE TABLE delivery.task_run_failure (
    run_id UUID PRIMARY KEY REFERENCES delivery.task_run(id),
    project_id UUID NOT NULL,
    category TEXT NOT NULL CHECK (category IN (
        'infrastructure', 'execution', 'validation', 'permission',
        'policy_violation', 'missing_dependency', 'unknown')),
    retryable BOOLEAN NOT NULL,
    reason_code TEXT NOT NULL,
    exit_code INT,
    timeout_seconds INT,
    evidence_ref TEXT,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE OR REPLACE FUNCTION delivery.reject_mutation_of_failure_record()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'delivery.task_run_failure is append-only';
END
$$;

CREATE TRIGGER task_run_failure_immutable
    BEFORE UPDATE OR DELETE ON delivery.task_run_failure
    FOR EACH ROW EXECUTE FUNCTION delivery.reject_mutation_of_failure_record();

-- §11.5：显式双向可追溯关系，支持「一新任务取代多个旧任务」。
CREATE TABLE delivery.task_supersession (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    old_task_id UUID NOT NULL,
    new_task_id UUID NOT NULL,
    reason TEXT NOT NULL CHECK (length(btrim(reason)) > 0),
    created_by UUID NOT NULL REFERENCES core.principal(id),
    approval_request_id UUID REFERENCES governance.approval_request(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (old_task_id, new_task_id),
    CHECK (old_task_id <> new_task_id),
    FOREIGN KEY (project_id, old_task_id) REFERENCES delivery.task(project_id, id),
    FOREIGN KEY (project_id, new_task_id) REFERENCES delivery.task(project_id, id)
);
CREATE INDEX idx_task_supersession_new ON delivery.task_supersession(new_task_id);

-- §12.4：状态与等待上下文分离存储；同一任务只允许一条活跃等待，历史保留。
CREATE TABLE delivery.task_waiting (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    task_id UUID NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN (
        'needs_info', 'needs_decision', 'needs_approval', 'external_dependency')),
    questions JSONB NOT NULL DEFAULT '[]',
    requested_from UUID REFERENCES core.principal(id),
    since TIMESTAMPTZ NOT NULL DEFAULT now(),
    deadline TIMESTAMPTZ,
    ended_at TIMESTAMPTZ,
    ended_by UUID REFERENCES core.principal(id),
    resolution_note TEXT,
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id)
);
CREATE UNIQUE INDEX uq_task_waiting_active ON delivery.task_waiting(task_id)
    WHERE ended_at IS NULL;

-- §15.4：任务级策略覆盖只存差异（NULL = 继承）。
CREATE TABLE delivery.task_policy_override (
    task_id UUID PRIMARY KEY,
    project_id UUID NOT NULL,
    auto_accept BOOLEAN,
    auto_merge BOOLEAN,
    auto_publish_knowledge BOOLEAN,
    require_human_approval BOOLEAN,
    additional_checks JSONB NOT NULL DEFAULT '[]',
    updated_by UUID NOT NULL REFERENCES core.principal(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id)
);

-- §20.4：Outbox 首版数据结构（含幂等键 / 租约 / 结构版本）。
CREATE TABLE integration.outbox_event (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    event_type TEXT NOT NULL,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'delivered', 'dead_letter')),
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_until TIMESTAMPTZ,
    lease_owner TEXT,
    last_error TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    schema_version INT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ
);
CREATE INDEX idx_outbox_claim ON integration.outbox_event(status, next_attempt_at)
    WHERE status IN ('pending', 'processing');

-- §九 M4 / §21.1：13 值 lowercase snake_case 为唯一合法取值。
ALTER TABLE delivery.task ALTER COLUMN status SET DEFAULT 'draft';

ALTER TABLE delivery.task
    ADD CONSTRAINT task_status_valid CHECK (status IN (
        'draft', 'ready', 'claimed', 'running', 'awaiting_decision', 'blocked',
        'submitted', 'reviewing', 'completed', 'changes_requested', 'failed',
        'cancelled', 'superseded')),
    ADD CONSTRAINT task_failure_category_valid CHECK (
        last_failure_category IS NULL OR last_failure_category IN (
            'infrastructure', 'execution', 'validation', 'permission',
            'policy_violation', 'missing_dependency', 'unknown'));
