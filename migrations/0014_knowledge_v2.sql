-- 0014_knowledge_v2: 知识空间 V2——提案、质量门、红线、否决回滚与派生索引。
-- 依据：docs/ai-pm-markdown-knowledge-space.md §七（核心表）/§八（发布状态机）/§九（索引）/§十一（质量门）。
-- 迁移号 0014（0011=D1 调度、0012=TaskReady 幂等、0013=R5 审批已占用）。
--
-- 正文权威仍在独立 Git 知识根；本迁移只扩业务状态与派生数据。document_proposal
-- 的 proposed_markdown 是「候选正文」，仅在提案生命周期内存在，提交后正文仍以
-- Git 修订为准，不构成第二权威。

-- 1) document_revision 补齐来源、分析与红线确认证据。
ALTER TABLE ai.document_revision
    ADD COLUMN IF NOT EXISTS source_event_id        UUID,
    ADD COLUMN IF NOT EXISTS analysis_run_id        UUID,
    ADD COLUMN IF NOT EXISTS redline_confirmed_by   UUID REFERENCES core.principal(id),
    ADD COLUMN IF NOT EXISTS published_at           TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_document_revision_source_event
    ON ai.document_revision (source_event_id)
    WHERE source_event_id IS NOT NULL;

-- 2) 提案：AI-PM 生成的候选修改 + 质量门结果 + 红线/否决状态。
CREATE TABLE IF NOT EXISTS ai.document_proposal (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id             UUID NOT NULL REFERENCES core.project(id),
    -- 新文档首次提案时 document_id 为空，提交后才绑定。
    document_id            UUID REFERENCES ai.project_document(id),
    doc_key                TEXT NOT NULL CHECK (doc_key ~ '^[A-Za-z0-9._-]+$'),
    doc_type               TEXT NOT NULL CHECK (
        doc_type IN ('requirement', 'decision', 'architecture', 'knowledge')
    ),
    canonical_path         TEXT NOT NULL,
    base_revision_id       UUID REFERENCES ai.document_revision(id),
    base_content_hash      TEXT,
    proposed_markdown      TEXT NOT NULL,
    proposed_content_hash  TEXT NOT NULL,
    change_summary         TEXT,
    source_event_id        UUID,
    analysis_run_id        UUID,
    status                 TEXT NOT NULL DEFAULT 'draft' CHECK (status IN (
        'draft', 'quality_gates_passed', 'redline_pending', 'confirmed',
        'commit_pending', 'committed', 'rejected', 'rolled_back'
    )),
    quality_gate_result    JSONB NOT NULL DEFAULT '{}'::jsonb,
    redline_kind           TEXT,
    redline_confirmed_by   UUID REFERENCES core.principal(id),
    redline_confirmed_at   TIMESTAMPTZ,
    resulting_revision_id  UUID REFERENCES ai.document_revision(id),
    rejection_reason       TEXT,
    created_by             UUID NOT NULL REFERENCES core.principal(id),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_document_proposal_project_status
    ON ai.document_proposal (project_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_document_proposal_document
    ON ai.document_proposal (document_id, created_at DESC)
    WHERE document_id IS NOT NULL;

-- 3) 派生索引状态：一个修订在一个 provider 下最多一条。
CREATE TABLE IF NOT EXISTS ai.document_index_state (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id            UUID NOT NULL REFERENCES core.project(id),
    document_revision_id  UUID NOT NULL REFERENCES ai.document_revision(id) ON DELETE CASCADE,
    provider              TEXT NOT NULL,
    external_document_id  TEXT,
    indexed_content_hash  TEXT,
    index_version         TEXT,
    status                TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'indexed', 'stale', 'failed')
    ),
    last_error            TEXT,
    indexed_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_revision_id, provider)
);

CREATE INDEX IF NOT EXISTS idx_document_index_state_project
    ON ai.document_index_state (project_id, status);

-- 4) 证据引用：决策结论引用了哪篇文档、哪个章节、哪个版本。
CREATE TABLE IF NOT EXISTS ai.knowledge_reference (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id            UUID NOT NULL REFERENCES core.project(id),
    document_id           UUID NOT NULL REFERENCES ai.project_document(id) ON DELETE CASCADE,
    document_revision_id  UUID NOT NULL REFERENCES ai.document_revision(id) ON DELETE CASCADE,
    section_anchor        TEXT,
    section_title         TEXT,
    start_line            INT,
    reference_kind        TEXT NOT NULL DEFAULT 'evidence' CHECK (
        reference_kind IN ('evidence', 'decision', 'constraint')
    ),
    source_kind           TEXT NOT NULL CHECK (
        source_kind IN ('analysis_run', 'event', 'manual')
    ),
    source_id             UUID,
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_knowledge_reference_document
    ON ai.knowledge_reference (document_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS uq_knowledge_reference_identity
    ON ai.knowledge_reference (document_revision_id, coalesce(section_anchor, ''), source_kind, coalesce(source_id, '00000000-0000-0000-0000-000000000000'::uuid));