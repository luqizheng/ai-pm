-- 0053_checkpoint1_kb_pc_minimal：Checkpoint-1 最小纵切——KB/PC 目标表最小子集。
-- （checkpoint-1：注册→建团→建项目关联仓库→仓库快照分析产出 PC/KB 草稿记录；
--   表设计依据 docs/architecture/knowledge-services/kb-document-target-ddl-draft-v1.md
--   与 docs/architecture/product-cognition/pc-01b/ V1.2 的**最小可用子集**。）
--
-- 编号注：台账预留 0049（Client Registry）/0050（ACP-04）/0051（ACP-05）/
-- 0052（ACP 收敛，Recovery Foundation 批），按台账纪律①取 max+1 = 0053。
--
-- 实施偏差登记（checkpoint-1 显式范围，ADR-06b 前不建 Git 权威）：
-- * kb.document_revision.content / pc.cognition_revision.content = 正文暂存
--   PG TEXT 列（正式形态 KBGIT/PCGIT 承载正文，本批不建 Git 写路径）；
-- * kb.document_revision.source_commit_sha = 仓库快照分析锚（正式稿归
--   evidence/source 定位）；
-- * pc.cognition_object.object_key = 派生幂等锚（正式稿=稳定身份机制 §pc-01b-02）；
-- * 关系/版本/提案/冲突/审批面（pc.relationship*、pc.cognition_version*、
--   pc.proposal*、pc.approval、pc.conflict*、pc.write_operation）全部不建，
--   归 PC-01C 正式实施；
-- * 正式认知写入门（M3）不破：全部产出 lifecycle_status='draft' /
--   verification_status='unverified' / status='draft'，等待人工确认。

-- ── ① KB：仓库快照知识文档（kb-document-target-ddl 最小子集）──────────────
CREATE SCHEMA IF NOT EXISTS kb;

CREATE TABLE kb.document (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL,
    project_id     UUID NOT NULL,
    doc_key        TEXT NOT NULL CHECK (doc_key ~ '^[A-Za-z0-9._-]+$'),
    doc_type       TEXT NOT NULL CHECK (doc_type IN
                     ('requirement','decision','architecture','knowledge')),
    canonical_path TEXT NOT NULL,
        -- KBGIT 逻辑路径（本批不建 Git 权威，仅登记定位）。
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, project_id, doc_key),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE TABLE kb.document_revision (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL,
    document_id       UUID NOT NULL REFERENCES kb.document(id),
    revision_no       INT  NOT NULL CHECK (revision_no >= 1),
    source_commit_sha TEXT,
        -- checkpoint 偏差列：仓库快照分析锚（分析的 HEAD commit）。
    content_hash      TEXT NOT NULL,
        -- sha256(markdown)；对账锚（同 hash 不重复建修订）。
    status            TEXT NOT NULL DEFAULT 'draft' CHECK (status IN
                        ('draft','published','rejected')),
    content           TEXT NOT NULL,
        -- 实施偏差：正文暂存 PG（KBGIT 后置，见头部偏差登记）。
    change_summary    TEXT,
    created_by        UUID,
        -- 归因列（不加 FK：worker 系统面 = dispatch principal）。
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_id, revision_no),
    UNIQUE (tenant_id, id)
);

-- append-only（DDL 草案纪律）：修订行拒 UPDATE/DELETE。
CREATE FUNCTION kb.reject_revision_mutation() RETURNS trigger AS $func$
BEGIN
    RAISE EXCEPTION 'kb.document_revision is append-only (drop trigger to migrate)';
END;
$func$ LANGUAGE plpgsql;

CREATE TRIGGER trg_kb_revision_append_only
    BEFORE UPDATE OR DELETE ON kb.document_revision
    FOR EACH ROW EXECUTE FUNCTION kb.reject_revision_mutation();

CREATE INDEX idx_kb_document_revision_document
    ON kb.document_revision (document_id, revision_no DESC);

-- ── ② PC：认知对象草稿（pc-01b V1.2 最小子集）────────────────────────────
CREATE SCHEMA IF NOT EXISTS pc;

-- 类型注册（14 类全量归 PC-01C；本批只播 checkpoint 派生用到的 4 类）。
CREATE TABLE pc.object_type (
    code        TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    description TEXT
);

INSERT INTO pc.object_type (code, label, description) VALUES
    ('repository',       '代码仓库',     '仓库快照身份对象（checkpoint-1 派生）'),
    ('module',           '模块/热点',    '热点文件映射的模块对象（checkpoint-1 派生）'),
    ('tech_stack',       '技术栈',       '语言/扩展名构成（checkpoint-1 派生）'),
    ('delivery_activity','交付动态',     '近期 commit 流摘要（checkpoint-1 派生）');

CREATE TABLE pc.cognition_object (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        UUID NOT NULL,
    project_id       UUID NOT NULL,
    object_type      TEXT NOT NULL REFERENCES pc.object_type(code),
    object_key       TEXT NOT NULL,
        -- checkpoint 偏差列：派生幂等锚（{repository_id}:{slot}）。
    name             TEXT NOT NULL,
    lifecycle_status TEXT NOT NULL DEFAULT 'draft' CHECK (lifecycle_status IN
                       ('identity','draft','active','deprecated','superseded')),
        -- AI 派生即 'draft'（待确认）；确认语义归人工（AI 禁自动确认）。
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, project_id, object_key),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE TABLE pc.cognition_revision (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL,
    object_id   UUID NOT NULL REFERENCES pc.cognition_object(id),
    revision_no INT  NOT NULL CHECK (revision_no >= 1),
    summary     TEXT,
    content     TEXT NOT NULL,
        -- 实施偏差：正文暂存 PG（PCGIT 后置，见头部偏差登记）。
    status      TEXT NOT NULL DEFAULT 'draft' CHECK (status IN
                  ('draft','published','rejected')),
    created_by  UUID,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (object_id, revision_no),
    UNIQUE (tenant_id, id)
);

CREATE INDEX idx_pc_cognition_revision_object
    ON pc.cognition_revision (object_id, revision_no DESC);

-- 来源四元组（结构化列——禁单 JSONB 混用语义，pc-01b-04 §纪律）。
CREATE TABLE pc.source (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL,
    project_id         UUID NOT NULL,
    target_domain      TEXT NOT NULL,
    target_type        TEXT NOT NULL,
    target_id          TEXT NOT NULL,
    target_revision_id TEXT,
    locator            JSONB,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

-- 证据（PC-01B-04：定位 + evidence_hash + verification_status + confidence；
-- confidence 与正式确认分离——高 confidence 不产生确认语义）。
CREATE TABLE pc.evidence (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL,
    project_id          UUID NOT NULL,
    source_id           UUID REFERENCES pc.source(id),
    cognition_object_id UUID NOT NULL REFERENCES pc.cognition_object(id),
    evidence_type       TEXT NOT NULL,
    locator             JSONB NOT NULL,
    evidence_hash       TEXT NOT NULL,
    verification_status TEXT NOT NULL DEFAULT 'unverified' CHECK (
                          verification_status IN
                          ('unverified','verified','refuted','conflicted')),
    confidence          TEXT NOT NULL DEFAULT 'medium' CHECK (confidence IN
                          ('high','medium','low')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_pc_evidence_object
    ON pc.evidence (cognition_object_id, created_at DESC);
CREATE INDEX idx_pc_evidence_intake
    ON pc.evidence ((locator->>'intake_id'));
