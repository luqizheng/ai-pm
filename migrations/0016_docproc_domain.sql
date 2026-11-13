-- 0016_docproc_domain.sql
--
-- Document Processing V1（平台级文档处理域）数据模型落地。
-- 设计真源：docs/architecture/knowledge-services/ai-pm-document-processing-td01-data-model-v1.md（含七项
-- 裁决 D-01~D-07 与仓库惯例审计基线）；规格真源：
-- docs/architecture/knowledge-services/ai-pm-document-processing-v1.md。
--
-- 新建 `docproc` schema，五张表：
--   ① docproc.document           Document 稳定身份（Team 级 = tenant，可选
--                                Project 关联，D-01；只留 current_version_id
--                                指针，内容列下沉 version——修正规格 6.1）
--   ② docproc.document_version   原始文件版本（D-06；append-only 守卫）
--   ③ docproc.processing_job     转换任务（状态机 + 入队幂等部分唯一索引
--                                + Worker 租约列——表即队列，TD-04）
--   ④ docproc.artifact           转换产物（D-06；append-only 守卫，转换器
--                                升级只增不改）
--   ⑤ docproc.source_mapping     原文↔产物追溯（规格 §6.5；jsonb 定位器）
--
-- 附带契约变更（AGENTS 规则 1：直接改契约，不设哨兵）：
--   integration.outbox_event.project_id 放开 NOT NULL——Team 级文档（无
--   project）也要发完成事件；既有查询均为 `= project_id` 过滤，NULL 行
--   天然不命中，零兼容分支。

CREATE SCHEMA IF NOT EXISTS docproc;

-- ── ① Document 稳定身份 ─────────────────────────────────────────────────

CREATE TABLE docproc.document (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid,                                   -- D-01：可选 Project（同一 tenant，应用层校验）
    filename text NOT NULL,
    mime_type text NOT NULL,
    file_extension text NOT NULL,
    upstream_tenant_key text DEFAULT '-' NOT NULL,     -- 上游租户键（file-service tid 镜像；
                                                       -- worker 按租户铸服务 JWT 用）
    current_version_id uuid,                           -- 指向 docproc.document_version
    row_revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_pkey PRIMARY KEY (id),
    CONSTRAINT document_filename_nonempty CHECK (length(btrim(filename)) > 0),
    CONSTRAINT document_ext_nonempty CHECK (length(btrim(file_extension)) > 0)
);

CREATE INDEX idx_document_tenant ON docproc.document (tenant_id, created_at DESC);
CREATE INDEX idx_document_project ON docproc.document (tenant_id, project_id)
    WHERE project_id IS NOT NULL;

-- ── ② Document Version：原始文件版本（D-06），不可变 ─────────────────────

CREATE TABLE docproc.document_version (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    document_id uuid NOT NULL REFERENCES docproc.document (id),
    version_number integer NOT NULL,
    storage_key text NOT NULL,                         -- file-service 文件ID（RustFS 由其托管，D-02）
    file_size bigint NOT NULL,
    checksum text NOT NULL,                            -- sha256 hex
    source_metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    uploaded_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_version_pkey PRIMARY KEY (id),
    CONSTRAINT document_version_number_check CHECK (version_number > 0),
    CONSTRAINT document_version_size_check CHECK (file_size > 0),
    CONSTRAINT document_version_checksum_nonempty CHECK (length(btrim(checksum)) > 0),
    CONSTRAINT document_version_unique UNIQUE (document_id, version_number)
);

CREATE INDEX idx_document_version_doc
    ON docproc.document_version (document_id, version_number DESC);

-- append-only 守卫：DELETE 禁止；归属与内容键不可变（D-06 版本永存）。
CREATE FUNCTION docproc.guard_document_version() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'docproc.document_version is append-only (delete forbidden)';
    END IF;
    IF NEW.document_id IS DISTINCT FROM OLD.document_id
       OR NEW.version_number IS DISTINCT FROM OLD.version_number
       OR NEW.storage_key IS DISTINCT FROM OLD.storage_key
       OR NEW.file_size IS DISTINCT FROM OLD.file_size
       OR NEW.checksum IS DISTINCT FROM OLD.checksum
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
    THEN
        RAISE EXCEPTION 'docproc.document_version is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_document_version_guard
    BEFORE UPDATE OR DELETE ON docproc.document_version
    FOR EACH ROW EXECUTE FUNCTION docproc.guard_document_version();

-- ── ③ Processing Job：转换任务（规格 §5.2 状态机；表即队列，TD-04） ───────

CREATE TABLE docproc.processing_job (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    document_version_id uuid NOT NULL REFERENCES docproc.document_version (id),
    processing_type text NOT NULL,                     -- parse / compare / export / ocr_detect
    status text DEFAULT 'pending' NOT NULL,
    parser_name text,
    parser_version text,
    config_version text NOT NULL,
    config_fingerprint text NOT NULL,                  -- 规范化 config 哈希（幂等键成分）
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    progress integer DEFAULT 0 NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    lease_owner text,                                  -- Worker 领取租约（TD-04 SKIP LOCKED）
    locked_until timestamp with time zone,             -- 租约到期；过期可被重新领取
    error_code text,
    error_detail jsonb,
    started_at timestamp with time zone,
    completed_at timestamp with time zone,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT processing_job_pkey PRIMARY KEY (id),
    CONSTRAINT job_type_check CHECK (processing_type = ANY (ARRAY['parse'::text, 'compare'::text, 'export'::text, 'ocr_detect'::text])),
    CONSTRAINT job_status_check CHECK (status = ANY (ARRAY['pending'::text, 'processing'::text, 'completed'::text, 'partial_success'::text, 'failed'::text, 'cancelled'::text])),
    CONSTRAINT job_progress_range CHECK (progress BETWEEN 0 AND 100),
    CONSTRAINT job_config_version_nonempty CHECK (length(btrim(config_version)) > 0)
);

-- 入队幂等（规格 §5.3「避免重复生成相同产物」的数据库层锚点）：同一
-- （原始版本 × 类型 × 配置）只允许一个非 cancelled 任务；FAILED 重试 =
-- 复用本行回 pending，不新建行。
CREATE UNIQUE INDEX uq_job_active_config
    ON docproc.processing_job (document_version_id, processing_type, config_fingerprint)
    WHERE status <> 'cancelled';

CREATE INDEX idx_job_version ON docproc.processing_job (document_version_id, created_at DESC);
CREATE INDEX idx_job_pending ON docproc.processing_job (status, created_at)
    WHERE status = 'pending';

-- ── ④ Artifact：转换产物（D-06），不可变、只增不改 ────────────────────────

CREATE TABLE docproc.artifact (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    processing_job_id uuid NOT NULL REFERENCES docproc.processing_job (id),
    artifact_type text NOT NULL,
    storage_key text NOT NULL,                         -- file-service 文件ID
    mime_type text NOT NULL,
    checksum text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,       -- V1 含 OCR 页级记录（规格 §4.6）与报告
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT artifact_pkey PRIMARY KEY (id),
    CONSTRAINT artifact_type_check CHECK (artifact_type = ANY (ARRAY['markdown'::text, 'extracted_text'::text, 'image'::text, 'table_data'::text, 'ocr_result'::text, 'conversion_report'::text, 'export_file'::text])),
    CONSTRAINT artifact_key_nonempty CHECK (length(btrim(storage_key)) > 0),
    CONSTRAINT artifact_checksum_nonempty CHECK (length(btrim(checksum)) > 0),
    -- 执行幂等（TD-04 §4）：同一任务内同类型同内容只落一行，重试不重复
    CONSTRAINT artifact_unique_in_job UNIQUE (processing_job_id, artifact_type, checksum)
);

CREATE INDEX idx_artifact_job ON docproc.artifact (processing_job_id);
CREATE INDEX idx_artifact_type ON docproc.artifact (tenant_id, artifact_type, created_at DESC);

-- append-only 守卫：DELETE 禁止；归属与内容键不可变（转换器升级 = 新 Job
-- 新 Artifact 行，旧行永存，规格 §6.4）。
CREATE FUNCTION docproc.guard_artifact() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'docproc.artifact is append-only (delete forbidden)';
    END IF;
    IF NEW.processing_job_id IS DISTINCT FROM OLD.processing_job_id
       OR NEW.artifact_type IS DISTINCT FROM OLD.artifact_type
       OR NEW.storage_key IS DISTINCT FROM OLD.storage_key
       OR NEW.checksum IS DISTINCT FROM OLD.checksum
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
    THEN
        RAISE EXCEPTION 'docproc.artifact is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_artifact_guard
    BEFORE UPDATE OR DELETE ON docproc.artifact
    FOR EACH ROW EXECUTE FUNCTION docproc.guard_artifact();

-- ── ⑤ Source Mapping：原文↔产物追溯（规格 §6.5） ─────────────────────────

CREATE TABLE docproc.source_mapping (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    artifact_id uuid NOT NULL REFERENCES docproc.artifact (id),
    source_location jsonb NOT NULL,    -- {page:5} / {slide:3} / {paragraph:42} / {sheet,row} / {table_index}
    target_location jsonb NOT NULL,    -- {heading:"## 1.1"} / {line_start,line_end} / {table_ref}
    mapping_type text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT source_mapping_pkey PRIMARY KEY (id),
    CONSTRAINT mapping_type_check CHECK (mapping_type = ANY (ARRAY['page_to_section'::text, 'slide_to_section'::text, 'paragraph_to_section'::text, 'sheet_to_section'::text, 'table_to_table'::text, 'asset_ref'::text]))
);

CREATE INDEX idx_mapping_artifact ON docproc.source_mapping (artifact_id);

-- ── 契约变更：outbox 放行 Team 级事件（无 project） ───────────────────────

ALTER TABLE integration.outbox_event ALTER COLUMN project_id DROP NOT NULL;
