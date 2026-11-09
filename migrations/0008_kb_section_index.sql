-- 0008_kb_section_index.sql
--
-- WP-3 · KB 域检索索引设施（检索架构 V1.0 任务书 §四）。
--
-- 术语（WP-3 裁决约束一）：**PostgreSQL ts_rank ≠ BM25**。本迁移交付的是
-- PostgreSQL FTS（tsvector + GIN）+ 应用层 CJK bigram 预分词；真 BM25 由
-- WP-5 依真实语料实测裁决（D-8，GATED）。
--
-- 设计（对齐 ai 栈 db-section 已验证的形态，域边界红线：不碰 ai.document*）：
-- - `kb.document_section_index`：章节树派生行（一对多），provider='kb-section'
--   新命名域（不复用 ai 栈的 'db-section'）；`search_tsv` 由应用侧 CJK bigram
--   预分词后 `to_tsvector('simple', ...)` 写入。
-- - `kb.document_index_state`：五态台账（pending/indexed/stale/failed + 哈希与
--   版本），有效性判定要素：content_hash + index_version + **家族现行性** +
--   provider（embedding 模型版本要素 WP-6 后启用）。
--
-- 不变量（沿用 ai 栈）：`status ≠ 'indexed'` ⇒ 该 (revision, provider) 的章节行
-- 数 = 0；状态转换只经应用层单一原语。

CREATE TABLE kb.document_section_index (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    provider text NOT NULL DEFAULT 'kb-section',
    section_no integer NOT NULL,
    level smallint NOT NULL,
    title text NOT NULL,
    anchor text NOT NULL,
    content text NOT NULL,
    start_line integer NOT NULL,
    search_tsv tsvector,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_section_index_revision_no_check
        CHECK (section_no >= 0),
    CONSTRAINT uq_kb_section_index_rev_provider_no
        UNIQUE (document_revision_id, provider, section_no)
);

CREATE INDEX idx_kb_section_index_scope
    ON kb.document_section_index (tenant_id, project_id, provider);
CREATE INDEX idx_kb_section_index_tsv
    ON kb.document_section_index USING gin (search_tsv);
CREATE INDEX idx_kb_section_index_doc
    ON kb.document_section_index (document_id);

CREATE TABLE kb.document_index_state (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    provider text NOT NULL DEFAULT 'kb-section',
    indexed_content_hash text,
    index_version text,
    status text NOT NULL DEFAULT 'pending',
    last_error text,
    indexed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_index_state_status_check
        CHECK (status IN ('pending', 'indexed', 'stale', 'failed')),
    CONSTRAINT uq_kb_index_state_rev_provider
        UNIQUE (document_revision_id, provider)
);

CREATE INDEX idx_kb_index_state_doc
    ON kb.document_index_state (document_id, provider);
