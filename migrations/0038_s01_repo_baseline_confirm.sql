-- S01-03 §5 Branch 与 Commit 基准治理（统一核验清单 G-05 / U-R12）。
--
-- 三个概念严格分离（S01-03 V1.2 §5.1）：
--   default_branch   仓库当前配置的默认分支（PATCH 可改，verify 可回写）；
--   head_commit_sha  Observed HEAD —— verify 自动回写，可自动检测变化；
--   selected_commit  Confirmed Commit —— 项目确认采用的版本，**只能**经
--                    POST /v1/pm/repositories/{id}/update-version 由人类确认写入，
--                    PATCH /v1/pm/repositories/{id} 不得触碰（应用层保证）。
--
-- selected_* 之前的旧值与确认人/时间即「历史含义」的承载（R-03-04：
-- 不覆盖历史执行引用的 Commit；Baseline/Agent Run 引用各自表中的 sha，不回改）。

ALTER TABLE integration.project_repository
    ADD COLUMN IF NOT EXISTS selected_branch TEXT,
    ADD COLUMN IF NOT EXISTS selected_commit TEXT,
    ADD COLUMN IF NOT EXISTS selected_confirmed_at TIMESTAMPTZ,
    -- 归因列（审计口径，不加 FK：auth-off/种子场景 principal 可能不入库）。
    ADD COLUMN IF NOT EXISTS selected_confirmed_by UUID;

-- 人工确认历史（append-only）：谁在何时把基准从什么改到什么（§6.4 活动记录）。
CREATE TABLE IF NOT EXISTS integration.repository_version_confirmation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    repository_id UUID NOT NULL REFERENCES integration.project_repository(id) ON DELETE CASCADE,
    previous_branch TEXT,
    previous_commit TEXT,
    selected_branch TEXT NOT NULL,
    selected_commit TEXT NOT NULL,
    reason TEXT,
    -- 归因列（审计口径，不加 FK）。
    confirmed_by UUID NOT NULL,
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_repo_version_confirmation_repo
    ON integration.repository_version_confirmation (repository_id, confirmed_at DESC);
