-- 0013_policy_scope: 多级审批策略存储（organization / project / task_type）。
-- 依据：docs/ai-pm-slice-d-execution-pipeline-v1.md §5.6（R5）
--   ——「综合组织、项目、任务类型和任务级限制」；强制审批由高层决定，
--      低层（任务级 override）**不得覆盖放宽**。
--
-- 任务级仍由既有 delivery.task_policy_override 承载（NULL = 继承）。
-- 迁移号取 0013：0012 已由并行知识空间线预留。

CREATE TABLE IF NOT EXISTS delivery.policy_scope (
    scope TEXT NOT NULL CHECK (scope IN ('organization', 'project', 'task_type')),
    -- organization 用固定键 '*'（当前平台只有一个组织）；
    -- project 用 project_id 文本；task_type 用任务类型文本。
    scope_key TEXT NOT NULL,
    require_human_approval BOOLEAN NOT NULL,
    updated_by UUID NOT NULL REFERENCES core.principal(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (scope, scope_key)
);

-- 任务类型来源：delivery.task 增加可空 task_type（forward-only，不改变既有语义）。
ALTER TABLE delivery.task
    ADD COLUMN IF NOT EXISTS task_type TEXT;