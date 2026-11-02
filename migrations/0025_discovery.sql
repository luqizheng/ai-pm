-- 0025_discovery: 项目认知域（Discovery，T-083）。
-- 依据：docs/ai-pm-project-discovery-v1.md V1.1 §四 + 附录 A（用户 Q1–Q7 已裁决）。
-- 与 0009 repo_import（仓库接入）是不同业务域：Import 负责接入，Discovery 负责理解项目。
-- forward-only；遵循 0019 多租户模式（tenant_id NOT NULL + 组合 FK + 每表 (tenant_id,id) 唯一）。
-- 相对附录 A 草案的实施偏差（1 处，随设计稿附录回填）：discovery_question 增加可空
-- finding_id——设计稿 §五 发现卡片契约「用户选择写 discovery_answer +
-- finding.verification_status 推进」要求 question 与 finding 可关联（Q3 UI 原型）。

CREATE SCHEMA IF NOT EXISTS discovery;

-- 1. 认知会话：一次项目认知建立过程；不绑执行端实例，可多来源贡献；允许并行（Q5）。
CREATE TABLE discovery.discovery_session (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('WEB','CODEX','SYSTEM')),
    kind TEXT NOT NULL CHECK (kind IN ('INITIAL_ONBOARDING','UPDATE','TASK_DISCOVERY')),
    status TEXT NOT NULL DEFAULT 'IN_PROGRESS' CHECK (status IN (
        'IN_PROGRESS','NEEDS_USER_INPUT','REVIEW_REQUIRED','COMPLETED','PAUSED'
    )),
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_discovery_session_project
    ON discovery.discovery_session(tenant_id, project_id, created_at DESC);
CREATE INDEX idx_discovery_session_status
    ON discovery.discovery_session(tenant_id, status);

-- 2. 对话消息。
CREATE TABLE discovery.discovery_message (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    speaker TEXT NOT NULL CHECK (speaker IN ('ai','user')),
    source TEXT NOT NULL CHECK (source IN ('WEB','CODEX','SYSTEM')),
    content TEXT NOT NULL,
    seq BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, session_id, seq),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE
);

-- 3. 发现：四类 provenance（Q3）+ 可信度 + 校验状态。
CREATE TABLE discovery.discovery_finding (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('observed','user_confirmed','speculation','contradicted')),
    content TEXT NOT NULL,
    confidence DOUBLE PRECISION CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
    verification_status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (verification_status IN ('PENDING','CONFIRMED','REJECTED','CONFLICT')),
    evidence JSONB NOT NULL DEFAULT '[]'::jsonb,   -- [{repository, file, description}]
    actor_role TEXT,                                -- 预留多 Agent 分工（设计稿 §十）
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_discovery_finding_session
    ON discovery.discovery_finding(tenant_id, session_id, kind);

-- 4. finding ↔ 仓库（多对多；Q4 V1 支持多仓库关联）。
CREATE TABLE discovery.discovery_finding_repository (
    tenant_id UUID NOT NULL,
    finding_id UUID NOT NULL,
    repository_id UUID NOT NULL,
    PRIMARY KEY (tenant_id, finding_id, repository_id),
    FOREIGN KEY (tenant_id, finding_id)
        REFERENCES discovery.discovery_finding (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, repository_id)
        REFERENCES integration.project_repository (tenant_id, id) ON DELETE CASCADE
);

-- 5. 待澄清问题。finding_id 是实施补列：发现卡片的裁决问答锚定到具体 finding。
CREATE TABLE discovery.discovery_question (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    finding_id UUID,
    question TEXT NOT NULL,
    why_blocking TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','answered','deferred')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, finding_id) REFERENCES discovery.discovery_finding (tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_discovery_question_session
    ON discovery.discovery_question(tenant_id, session_id, status);

-- 6. 用户回答。
CREATE TABLE discovery.discovery_answer (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    question_id UUID NOT NULL,
    answer TEXT NOT NULL,
    answered_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, question_id)
        REFERENCES discovery.discovery_question (tenant_id, id) ON DELETE CASCADE
);

-- 7. 知识变更提案（Q7：base_revision_id 即 base_version，写入前校验；失效置 conflict）。
-- 前置修正：0019 §4 的 15 张引用表清单未含 ai.document_proposal，而下方组合 FK
-- 引用 (tenant_id, id)，先补租户内唯一键（forward-only 追加，不改已应用迁移）。
ALTER TABLE ai.document_proposal
    ADD CONSTRAINT uq_document_proposal_tenant_id UNIQUE (tenant_id, id);
CREATE TABLE discovery.discovery_proposal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('add','modify')),
    target_document TEXT NOT NULL,           -- 目标文档 key/path
    document_id UUID,                        -- modify 时指向既有 ai.project_document
    base_revision_id UUID,                   -- Q7 提交时的知识版本；add 时为 NULL
    proposed_content TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '[]'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','rejected','conflict')),
    document_proposal_id UUID,               -- 落地到 ai.document_proposal 后的关联
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document (tenant_id, id),
    FOREIGN KEY (tenant_id, base_revision_id) REFERENCES ai.document_revision (tenant_id, id),
    FOREIGN KEY (tenant_id, document_proposal_id) REFERENCES ai.document_proposal (tenant_id, id)
);
CREATE INDEX idx_discovery_proposal_session
    ON discovery.discovery_proposal(tenant_id, session_id, status);

-- 8. 唯一关联方向（Q1）：repo_import → discovery_session，不做反向 FK。
--    注：组合 FK 不能用 ON DELETE SET NULL（tenant_id NOT NULL），保留默认 NO ACTION。
ALTER TABLE integration.repo_import
    ADD COLUMN discovery_session_id UUID,
    ADD CONSTRAINT fk_repo_import_tenant_discovery_session
        FOREIGN KEY (tenant_id, discovery_session_id)
        REFERENCES discovery.discovery_session (tenant_id, id);
CREATE INDEX idx_repo_import_discovery_session
    ON integration.repo_import(tenant_id, discovery_session_id);