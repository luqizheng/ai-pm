-- 0024_knowledge_section_index: 章节级派生索引 + 写入幂等账本（P6 后半 / T-060）。
-- 依据：docs/ai-pm-t060-knowledge-index-v1.md §4（数据模型）、§5.3（唯一状态转换原语）、
--        §5.8（Project 级写入协调）。
--
-- 边界：Git 知识根里的 Markdown 文件是**唯一权威正文**；本迁移只落**可重建的派生
-- 数据**。正文哈希或索引版本一变，派生数据即失效，检索回退直读 Git。

-- 1) 章节索引（provider = 'db-section' 的派生表）。
--    provider 是维度列：未来 PageIndex 自带派生表，不共用本表。
--    tenant_id 由修订行带出，由组合外键强制与修订租户一致（不靠应用层填对）。
CREATE TABLE IF NOT EXISTS ai.document_section_index (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            UUID NOT NULL,
    project_id           UUID NOT NULL,
    document_revision_id UUID NOT NULL,
    provider             TEXT NOT NULL,
    section_no           INT  NOT NULL,
    level                SMALLINT NOT NULL,
    title                TEXT NOT NULL,
    anchor               TEXT NOT NULL,
    content              TEXT NOT NULL,
    start_line           INT  NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_revision_id, provider, section_no)
);

CREATE INDEX IF NOT EXISTS idx_document_section_project
    ON ai.document_section_index (tenant_id, project_id, provider);
CREATE INDEX IF NOT EXISTS idx_document_section_revision
    ON ai.document_section_index (document_revision_id, provider);

ALTER TABLE ai.document_section_index
    ADD CONSTRAINT fk_document_section_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.document_section_index
    ADD CONSTRAINT fk_document_section_tenant_revision
        FOREIGN KEY (tenant_id, document_revision_id)
        REFERENCES ai.document_revision (tenant_id, id) ON DELETE CASCADE;

-- 2) 写入幂等账本：同一 (project_id, operation_id) 只产生一条 Revision。
--    不是 outbox、不是 job——没有消费者、没有事件、没有后台扫描。
CREATE TABLE IF NOT EXISTS ai.knowledge_write_operation (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL,
    project_id   UUID NOT NULL,
    operation_id TEXT NOT NULL,
    command      TEXT NOT NULL CHECK (
        command IN ('create_document', 'submit_proposal', 'confirm_redline', 'veto_revision')
    ),
    -- 本次操作产出的提案 / 修订（veto 可能两者皆空，用 NULL）。
    proposal_id  UUID,
    revision_id  UUID,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, operation_id)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_write_op_revision
    ON ai.knowledge_write_operation (revision_id)
    WHERE revision_id IS NOT NULL;

ALTER TABLE ai.knowledge_write_operation
    ADD CONSTRAINT fk_knowledge_write_op_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
