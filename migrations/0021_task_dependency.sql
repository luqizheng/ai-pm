-- 0021_task_dependency: Task 前置依赖（T-024 / T-023 设计稿 §3）
-- 依据：docs/ai-pm-task-dependency-v1.md（用户过稿）
-- 原则：forward-only；新表；同项目由两条复合外键保证（D3）。
--
-- 语义：`task_id` 是下游（依赖方），`depends_on_task_id` 是上游（被依赖方）。
-- 边表本身是租户业务表：`tenant_id NOT NULL` + `(tenant_id, project_id)` 组合
-- 外键（ADR-0001 I1/I5）。

CREATE TABLE delivery.task_dependency (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL,
    project_id          UUID NOT NULL,
    task_id             UUID NOT NULL,          -- 下游（依赖方）
    depends_on_task_id  UUID NOT NULL,          -- 上游（被依赖方）
    created_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    UNIQUE (task_id, depends_on_task_id),
    UNIQUE (tenant_id, id),
    CHECK (task_id <> depends_on_task_id),

    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- 两条外键共用同一个 project_id：跨项目边在库层不可表示（D3）
    FOREIGN KEY (project_id, task_id)
        REFERENCES delivery.task (project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, depends_on_task_id)
        REFERENCES delivery.task (project_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_task_dependency_upstream
    ON delivery.task_dependency (project_id, depends_on_task_id);