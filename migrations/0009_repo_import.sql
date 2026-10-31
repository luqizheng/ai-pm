-- 0009_repo_import: 仓库可达性状态 + 一次导入会话（草案）与来源事实快照。
-- 0008 已落地「项目 ↔ 仓库」纯 CRUD（name/url/role/描述），本迁移只做 forward-only
-- 补列与新增两张导入表，不改 0008 已有列语义。

-- 密钥：保留 0008 的「一个项目一把」主键语义，补吊销状态与创建人。
ALTER TABLE integration.project_ssh_key
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'revoked')),
    ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES core.principal(id),
    ADD COLUMN IF NOT EXISTS revoked_at TIMESTAMPTZ;

-- 仓库：可达性校验结果 + 绑定的密钥。一个项目共用一把 key，所以 ssh_key_id
-- 语义上恒等于 project_id（`project_ssh_key` 的主键），保留独立列便于未来一仓一键。
ALTER TABLE integration.project_repository
    ADD COLUMN IF NOT EXISTS ssh_key_id UUID,
    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'verified', 'unreachable', 'archived')),
    ADD COLUMN IF NOT EXISTS head_commit_sha TEXT,
    ADD COLUMN IF NOT EXISTS last_verified_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_error_code TEXT,
    ADD COLUMN IF NOT EXISTS last_error_detail TEXT;

CREATE INDEX IF NOT EXISTS idx_project_repository_status
ON integration.project_repository(project_id, status);

-- 一次导入会话：AI 草案只落这里，不直写业务表；apply 时才转成提案。
CREATE TABLE integration.repo_import (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id) ON DELETE CASCADE,
    requested_by UUID NOT NULL REFERENCES core.principal(id),
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN (
        'queued', 'cloning', 'reading', 'analyzing',
        'draft_ready', 'proposed', 'applied', 'rejected', 'failed', 'cancelled'
    )),
    stage_detail TEXT,
    progress INT NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
    model_name TEXT NOT NULL DEFAULT 'fake',
    draft JSONB,
    draft_revision BIGINT NOT NULL DEFAULT 0,
    dropped_items INT NOT NULL DEFAULT 0,
    error_code TEXT,
    error_detail TEXT,
    analysis_run_id UUID REFERENCES ai.analysis_run(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    proposed_at TIMESTAMPTZ,
    applied_at TIMESTAMPTZ,
    UNIQUE (project_id, id)
);

-- 每项目同时只允许一个进行中的导入。
CREATE UNIQUE INDEX repo_import_one_in_progress
ON integration.repo_import(project_id)
WHERE status IN ('queued', 'cloning', 'reading', 'analyzing');

CREATE INDEX idx_repo_import_project
ON integration.repo_import(project_id, created_at DESC);

-- 每个仓库在一次导入中的事实快照（拉取时点的 HEAD / 计数 / 是否截断）。
CREATE TABLE integration.repo_import_source (
    import_id UUID NOT NULL
        REFERENCES integration.repo_import(id) ON DELETE CASCADE,
    repository_id UUID NOT NULL
        REFERENCES integration.project_repository(id) ON DELETE CASCADE,
    head_commit_sha TEXT,
    commit_count INT NOT NULL DEFAULT 0,
    doc_count INT NOT NULL DEFAULT 0,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    truncated BOOLEAN NOT NULL DEFAULT false,
    facts JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (import_id, repository_id)
);