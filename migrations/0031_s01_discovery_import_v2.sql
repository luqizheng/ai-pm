-- 0031_s01_discovery_import_v2: S01 · I6-B Discovery / Import 收敛与跨 Project 约束。
--
-- 依据：docs/ui/smark-pm-s01-i6-b-v1-1.md（V1.1 正文）+ docs/ui/smark-pm-s01-i6-b-v1-1-merge-spec.md（合并规格 V1.0）。
-- 冻结门槛：I6-B V1.1 §11.3 的 12 条（§9/§10/§11 需 I6-B-09 实测）。
--
-- 目标：
--   1. 草案留版本（D-1a）；分析轮次留档（R-4-1）；基准 write-once（N-1）。
--   2. Evidence 成为实体，且与 Attempt 用组合 FK 锚定（补-2 / R-6-2）。
--   3. 同 Project 归属由数据库保证（D-2a + 补-1/补-3）：两条链
--      Evidence → Attempt → Source → Import → Project 与 Finding → Session → Project。
--   4. 仓库有历史引用即不可物理删除（C-4，RESTRICT + 409 repository_has_history）。
--
-- AGENTS.md 规则 1（新项目、无存量数据）：不写回填脚本、不留兼容分支；
--   唯一例外是 discovery_finding / finding_repository / question / proposal 的
--   project_id 一次性结构对齐（I6-B V1.1 §0.5 C-14）——这些表被既有 Discovery
--   数据与下游 FK 引用，整表重建代价远大于一次 UPDATE。

-- ── 0. 组合唯一键（复合 FK 的父键锚点）────────────────────────────────
ALTER TABLE integration.repo_import
    ADD CONSTRAINT uq_repo_import_tenant_project_id UNIQUE (tenant_id, project_id, id);

ALTER TABLE integration.project_repository
    ADD CONSTRAINT uq_project_repository_tenant_project_id UNIQUE (tenant_id, project_id, id);

ALTER TABLE discovery.discovery_session
    ADD CONSTRAINT uq_discovery_session_tenant_project_id UNIQUE (tenant_id, project_id, id);

-- ── 1. Project 初始化操作（R-3-3 部分唯一索引 / R-3-4 权威落表）────────
CREATE TABLE core.project_initialization_operation (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL,
    project_id  UUID NOT NULL,
    import_id   UUID,
    generation  BIGINT NOT NULL,
    status      TEXT NOT NULL CHECK (status IN ('active','released','completed','cancelled')),
    created_by  UUID REFERENCES core.principal(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ,
    UNIQUE (tenant_id, project_id, generation),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX uq_project_initialization_operation_active
    ON core.project_initialization_operation (project_id) WHERE status = 'active';

-- ── 2. 草案版本表（D-1a：旧草案保留，不覆盖历史版本）─────────────────────
CREATE TABLE integration.repo_import_draft_version (
    tenant_id         UUID NOT NULL,
    import_id         UUID NOT NULL,
    revision          BIGINT NOT NULL,
    draft             JSONB NOT NULL,
    source_attempts   JSONB NOT NULL DEFAULT '[]'::jsonb,
    dropped_items     INT NOT NULL DEFAULT 0,
    generation_status TEXT NOT NULL CHECK (generation_status IN ('generating','succeeded','failed')),
    generated_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, import_id, revision),
    FOREIGN KEY (tenant_id, import_id) REFERENCES integration.repo_import (tenant_id, id) ON DELETE CASCADE
);

-- ── 3. Source 整表重建（R-4-1 事实迁 Attempt / R-6-1 四列键 / C-16）────
-- 该表无任何 FK 依赖方（见 I6-B V1.1 §3.4），可整体替换；不写回填。
DROP TABLE integration.repo_import_source;
CREATE TABLE integration.repo_import_source (
    tenant_id          UUID NOT NULL,
    project_id         UUID NOT NULL,
    import_id          UUID NOT NULL,
    repository_id      UUID NOT NULL,
    head_commit_sha    TEXT,                  -- 基准：write-once（N-1）
    current_attempt_no INT,                  -- 当前轮指针
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Source 归属约束（基线 §4.2）：UNIQUE (tenant_id, import_id, repository_id)
    -- 由主键等价满足；四列键供 Attempt / Evidence 的组合外键引用。
    PRIMARY KEY (tenant_id, import_id, repository_id),
    UNIQUE (tenant_id, project_id, import_id, repository_id),
    FOREIGN KEY (tenant_id, project_id, import_id)
        REFERENCES integration.repo_import (tenant_id, project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, repository_id)
        REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE RESTRICT
);

-- ── 4. Attempt 表（R-4-1 成功与失败都留档 / 补-2 组合键锚点）────────────
CREATE TABLE integration.repo_import_source_attempt (
    tenant_id     UUID NOT NULL,
    project_id    UUID NOT NULL,
    import_id     UUID NOT NULL,
    repository_id UUID NOT NULL,
    attempt_no    INT NOT NULL,
    status        TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','cancelled')),
    commit_sha    TEXT,
    facts         JSONB,
    commit_count  INT,
    doc_count     INT,
    size_bytes    BIGINT,
    truncated     BOOLEAN,
    error_code    TEXT,
    error_detail  TEXT,
    job_id        UUID,
    started_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at   TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, import_id, repository_id, attempt_no),
    UNIQUE (tenant_id, project_id, import_id, repository_id, attempt_no),
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id)
        REFERENCES integration.repo_import_source
               (tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE,
    CHECK (finished_at IS NULL OR finished_at >= started_at)
);

-- ── 5. Import：删单列 draft，draft_revision 降为指针（C-15）──────────────
ALTER TABLE integration.repo_import DROP COLUMN draft;

-- ── 6. Finding 升格（补-1 / C-17 / D-2a）──────────────────────────────
ALTER TABLE discovery.discovery_finding ADD COLUMN project_id UUID;
UPDATE discovery.discovery_finding f
   SET project_id = s.project_id
  FROM discovery.discovery_session s
 WHERE s.tenant_id = f.tenant_id AND s.id = f.session_id;
ALTER TABLE discovery.discovery_finding ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_finding
    ADD CONSTRAINT uq_discovery_finding_tenant_project_id UNIQUE (tenant_id, project_id, id);
ALTER TABLE discovery.discovery_finding DROP COLUMN evidence;
ALTER TABLE discovery.discovery_finding DROP CONSTRAINT discovery_finding_tenant_id_session_id_fkey;
ALTER TABLE discovery.discovery_finding
    ADD CONSTRAINT fk_discovery_finding_tenant_project_session
    FOREIGN KEY (tenant_id, project_id, session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id) ON DELETE CASCADE;

-- ── 7. Evidence 实体（I6-B-04 / R-6-2 CHECK 判别器 / 补-2）────────────
CREATE TABLE discovery.discovery_finding_evidence (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL,
    project_id    UUID NOT NULL,
    finding_id    UUID NOT NULL,
    evidence_type TEXT NOT NULL CHECK (evidence_type IN
                   ('code','test_result','commit','user_statement','system_observation')),
    description   TEXT,
    file_path     TEXT,
    symbol        TEXT,
    code_range    TEXT,
    import_id     UUID,
    repository_id UUID,
    commit_sha    TEXT,
    attempt_no    INT,
    created_by    UUID REFERENCES core.principal(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, project_id, finding_id)
        REFERENCES discovery.discovery_finding (tenant_id, project_id, id) ON DELETE CASCADE,
    -- Import 归属：基线 §4.2 要求「Evidence 的 Import 外键包含 tenant_id, project_id,
    -- import_id」。实现为**链式**——Evidence → Source(t,p,import,repository) → Import(t,p,id)
    -- 与 Evidence → Attempt(t,p,import,repository,attempt_no) → Source，两条链的父键都
    -- 带 project_id，等价保证「Evidence 的 Import 与其 Project 同 Project」。
    -- 不加直接的 Evidence → repo_import 外键：0025 的 Q1「唯一关联方向」不变量要求
    -- discovery 域**不得**引用 integration.repo_import（单向关联），专项设计稿优先级
    -- 高于正式基线措辞，且直接外键在此模型下是冗余的。
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id)
        REFERENCES integration.repo_import_source
               (tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id, attempt_no)
        REFERENCES integration.repo_import_source_attempt
               (tenant_id, project_id, import_id, repository_id, attempt_no) ON DELETE CASCADE,
    CHECK (
        (evidence_type IN ('code','test_result','commit')
          AND import_id IS NOT NULL AND repository_id IS NOT NULL
          AND commit_sha IS NOT NULL AND attempt_no IS NOT NULL)
        OR
        (evidence_type IN ('user_statement','system_observation')
          AND import_id IS NULL AND repository_id IS NULL
          AND commit_sha IS NULL AND attempt_no IS NULL)
    )
);
CREATE INDEX idx_discovery_evidence_finding
    ON discovery.discovery_finding_evidence(tenant_id, project_id, finding_id);

-- ── 8. finding_repository 升格（补-3：跨项目仓库挂 Finding 必须失败）─────
ALTER TABLE discovery.discovery_finding_repository ADD COLUMN project_id UUID;
UPDATE discovery.discovery_finding_repository r
   SET project_id = f.project_id
  FROM discovery.discovery_finding f
 WHERE f.tenant_id = r.tenant_id AND f.id = r.finding_id;
ALTER TABLE discovery.discovery_finding_repository ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_finding_repository
    DROP CONSTRAINT discovery_finding_repository_tenant_id_finding_id_fkey;
ALTER TABLE discovery.discovery_finding_repository
    DROP CONSTRAINT discovery_finding_repository_tenant_id_repository_id_fkey;
ALTER TABLE discovery.discovery_finding_repository
    ADD CONSTRAINT fk_finding_repository_tenant_project_finding
    FOREIGN KEY (tenant_id, project_id, finding_id)
    REFERENCES discovery.discovery_finding (tenant_id, project_id, id) ON DELETE CASCADE;
ALTER TABLE discovery.discovery_finding_repository
    ADD CONSTRAINT fk_finding_repository_tenant_project_repository
    FOREIGN KEY (tenant_id, project_id, repository_id)
    REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE RESTRICT;

-- ── 9. Question / Proposal 换轨携带 project ───────────────────────────
ALTER TABLE discovery.discovery_question ADD COLUMN project_id UUID;
UPDATE discovery.discovery_question q
   SET project_id = s.project_id
  FROM discovery.discovery_session s
 WHERE s.tenant_id = q.tenant_id AND s.id = q.session_id;
ALTER TABLE discovery.discovery_question ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_question
    DROP CONSTRAINT discovery_question_tenant_id_session_id_fkey;
ALTER TABLE discovery.discovery_question
    DROP CONSTRAINT discovery_question_tenant_id_finding_id_fkey;
ALTER TABLE discovery.discovery_question
    ADD CONSTRAINT fk_discovery_question_tenant_project_session
    FOREIGN KEY (tenant_id, project_id, session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id) ON DELETE CASCADE;
ALTER TABLE discovery.discovery_question
    ADD CONSTRAINT fk_discovery_question_tenant_project_finding
    FOREIGN KEY (tenant_id, project_id, finding_id)
    REFERENCES discovery.discovery_finding (tenant_id, project_id, id) ON DELETE CASCADE;

ALTER TABLE discovery.discovery_proposal ADD COLUMN project_id UUID;
UPDATE discovery.discovery_proposal p
   SET project_id = s.project_id
  FROM discovery.discovery_session s
 WHERE s.tenant_id = p.tenant_id AND s.id = p.session_id;
ALTER TABLE discovery.discovery_proposal ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_proposal
    DROP CONSTRAINT discovery_proposal_tenant_id_session_id_fkey;
ALTER TABLE discovery.discovery_proposal
    ADD CONSTRAINT fk_discovery_proposal_tenant_project_session
    FOREIGN KEY (tenant_id, project_id, session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id) ON DELETE CASCADE;

-- ── 10. Import ↔ Discovery Session 同 Project ─────────────────────────
ALTER TABLE integration.repo_import
    DROP CONSTRAINT fk_repo_import_tenant_discovery_session;
ALTER TABLE integration.repo_import
    ADD CONSTRAINT fk_repo_import_tenant_project_discovery_session
    FOREIGN KEY (tenant_id, project_id, discovery_session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id);

-- ── 11. 仓库删除保护（C-4）────────────────────────────────────────────
ALTER TABLE delivery.task_repository
    DROP CONSTRAINT task_repository_project_id_repository_id_fkey;
ALTER TABLE delivery.task_repository
    ADD CONSTRAINT fk_task_repository_tenant_project_repository
    FOREIGN KEY (tenant_id, project_id, repository_id)
    REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE RESTRICT;

-- ── 12. Import 名额索引重建（C-3：走新增迁移，不改 0009）──────────────
DROP INDEX IF EXISTS integration.repo_import_one_in_progress;
CREATE UNIQUE INDEX repo_import_one_active_per_project
    ON integration.repo_import (tenant_id, project_id)
    WHERE status IN ('queued','cloning','reading','analyzing');
