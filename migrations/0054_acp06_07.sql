-- 0054_acp06_07: ACP-06 Checkpoint 登记 / Workspace Lock（操作化）+
-- ACP-07 Deadline 重新授权（联合实施设计 acp06-07-joint-v1.md §1.2/§3.1；
-- 联合基线批 5；取号 tentative 0054——0053 已被 KB-PC checkpoint 占，落地前已重核）。

-- ── ACP-06：Checkpoint 登记元数据（正文永远在 Workspace，Platform 仅元数据）──
CREATE TABLE delivery.task_checkpoint (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    task_id       UUID NOT NULL REFERENCES delivery.task(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    interaction_id UUID REFERENCES delivery.task_interaction(id),
    file_path     TEXT NOT NULL,
    content_hash  TEXT NOT NULL,
    file_size     BIGINT NOT NULL,
    session_id    TEXT,
    registered_via TEXT NOT NULL DEFAULT 'executor_report'
                  CHECK (registered_via IN ('executor_report', 'worker_receipt')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    -- 幂等：同 Run 同内容只登记一次（重复上报返回原行）
    UNIQUE (run_id, content_hash)
);
CREATE INDEX idx_task_checkpoint_run
    ON delivery.task_checkpoint (run_id, created_at DESC);

-- ── ACP-07：Deadline 重新授权（独立审计 + 独立有效期；原 run.deadline_at 零改动）──
CREATE TABLE delivery.task_run_execution_authorization (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    interaction_id UUID REFERENCES delivery.task_interaction(id),
    authorized_by UUID NOT NULL REFERENCES core.principal(id),
    reason        TEXT NOT NULL,
    new_deadline_at TIMESTAMPTZ NOT NULL,
    granted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL,
    revoked_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);
CREATE INDEX idx_run_authorization_active
    ON delivery.task_run_execution_authorization (run_id)
    WHERE revoked_at IS NULL;
