# Document Processing · TD-01 数据模型与数据库 DDL — V1

**所属系统：** PM-AI / smark-pm
**上游规格：** docs/architecture/knowledge-services/ai-pm-document-processing-v1.md（V1.0-R1，D-01～D-07 已裁决）
**文档状态：** 技术设计待评审
**范围：** TD-01（数据模型 + DDL）；TD-02/03/04/05 各自独立成文

---

## 0. 术语映射（关键决策，需确认）

规格中的 **Team** 在本仓唯一对应 `core.tenant`（仓库无 `core.team` 表；tenant 即产品语义中的团队/组织层）。D-01「Document 必须归属一个 Team」落地为：

> **所有 docproc 表 `tenant_id NOT NULL`；`project_id` 可空，非空时必须与 tenant 一致（应用层校验，不建跨 schema 硬 FK）。**

若产品语义未来引入独立于 tenant 的 Team 实体，需改本映射——当前按 tenant 执行。

## 1. 仓库惯例审计基线（本设计遵循）

| 惯例 | 证据 | 本设计采用 |
|---|---|---|
| 每域一个 PG schema | `design`（0010）、`pc`、`kb`、`delivery`… | 新建 **`docproc`** schema |
| uuid 主键 `gen_random_uuid()` | 0001 全部 | 是 |
| 每表 `tenant_id NOT NULL` | 0001 全部 | 是 |
| 枚举 = `text` + `CHECK (col = ANY(ARRAY[...]))`，不用原生 enum | 0010 等 | 是 |
| 不可变内容表用守卫触发器（禁 DELETE + 冻结内容列） | `design.design_revision`（0010） | document_version / artifact / source_mapping |
| 可变身份表带 `row_revision bigint`（乐观并发） | `design.design` | document / processing_job |
| 跨域引用不建硬 FK，应用层校验 | `design_requirement`、`task_requirement` | 引用 core.* 一律无 FK；docproc 域内允许 FK |
| 迁移号顺序 | 0001～0015 已用 | 实施时落 **0016_docproc_domain.sql** |

**命名避撞：** `ai.document_revision` / `ai.document_index_state` 已被知识文档域占用（Git 权威体系）。`docproc` 前缀 + 独立 schema 保证两域不混。ai.document_* 不做任何改动。

## 2. 实体清单（规格 §6 → 本设计）

| 规格 | 表 | 可变性 | 规格字段的落地修正 |
|---|---|---|---|
| 6.1 Document | `docproc.document` | 可变 | **storage_key/checksum 不在 Document 上**（规格列于此处，但 D-06 版本承载内容——Document 只保留 `current_version_id` 指针，避免双写漂移） |
| 6.2 Document Version | `docproc.document_version` | 不可变（append-only） | 增加 `file_size`、`uploaded_by`；version_number 从 1 单调递增 |
| 6.3 Processing Job | `docproc.processing_job` | 可变（状态机） | 增加 `config_fingerprint`、`attempts`、`progress`；幂等键约束见 §4.3 |
| 6.4 Artifact | `docproc.artifact` | 不可变（append-only） | artifact_type 7 值 CHECK；同 job 内 (type, checksum) 唯一 |
| 6.5 Source Mapping | `docproc.source_mapping` | 写一次后不可变 | source/target 用 jsonb 定位器（页码/章节路径/幻灯片号异构） |
| 6.6 业务关联 | 不建表 | — | 归业务域自有表（U-12）；docproc 不持有 KB/PC/需求实体 |
| 4.6 OCR | 不建表（V1） | — | OCR 页级结果进 `artifact.metadata`；P1 量级增长再独立成表 |

## 3. DDL（实施即 0016 迁移正文）

```sql
CREATE SCHEMA IF NOT EXISTS docproc;

-- ── ① Document 稳定身份 ─────────────────────────────────────────────────
CREATE TABLE docproc.document (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid,                                   -- D-01：可选 Project
    filename text NOT NULL,
    mime_type text NOT NULL,
    file_extension text NOT NULL,
    current_version_id uuid,                           -- 指向 docproc.document_version
    row_revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_filename_nonempty CHECK (length(btrim(filename)) > 0),
    CONSTRAINT document_ext_nonempty CHECK (length(btrim(file_extension)) > 0)
);
-- project 一致性（project 属于同一 tenant）应用层校验（跨域无 FK，同 design 域惯例）

CREATE INDEX idx_document_tenant ON docproc.document (tenant_id, created_at DESC);
CREATE INDEX idx_document_project ON docproc.document (tenant_id, project_id)
    WHERE project_id IS NOT NULL;

-- ── ② Document Version：原始文件版本（D-06），不可变 ──────────────────────
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
    CONSTRAINT document_version_number_check CHECK (version_number > 0),
    CONSTRAINT document_version_size_check CHECK (file_size > 0),
    CONSTRAINT document_version_checksum_nonempty CHECK (length(btrim(checksum)) > 0),
    CONSTRAINT document_version_unique UNIQUE (document_id, version_number)
);

CREATE INDEX idx_document_version_doc ON docproc.document_version (document_id, version_number DESC);

CREATE FUNCTION docproc.guard_document_version() RETURNS trigger
    LANGUAGE plpgsql AS $$
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

-- ── ③ Processing Job：转换任务（状态机 §5.2） ────────────────────────────
CREATE TABLE docproc.processing_job (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    document_version_id uuid NOT NULL REFERENCES docproc.document_version (id),
    processing_type text NOT NULL,                     -- parse / compare / export / ocr_detect
    status text DEFAULT 'pending' NOT NULL,            -- pending/processing/completed/partial_success/failed/cancelled
    parser_name text,
    parser_version text,
    config_version text NOT NULL,
    config_fingerprint text NOT NULL,                  -- 规范化 config 的哈希，幂等键成分
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
    CONSTRAINT job_type_check CHECK (processing_type = ANY (ARRAY['parse','compare','export','ocr_detect'])),
    CONSTRAINT job_status_check CHECK (status = ANY (ARRAY['pending','processing','completed','partial_success','failed','cancelled'])),
    CONSTRAINT job_progress_range CHECK (progress BETWEEN 0 AND 100),
    CONSTRAINT job_config_version_nonempty CHECK (length(btrim(config_version)) > 0)
);

-- 幂等：同一 (原始版本 × 类型 × 配置) 只允许一个非 cancelled 任务
-- （§5.3「避免重复生成相同产物」的数据库层锚点；FAILED 重试 = 复用本行回 pending）
CREATE UNIQUE INDEX uq_job_active_config
    ON docproc.processing_job (document_version_id, processing_type, config_fingerprint)
    WHERE status <> 'cancelled';

CREATE INDEX idx_job_version ON docproc.processing_job (document_version_id, created_at DESC);
CREATE INDEX idx_job_pending ON docproc.processing_job (status, created_at)
    WHERE status = 'pending';   -- Worker 拉取入口（TD-04 定 poll/claim 语义）

-- ── ④ Artifact：转换产物（D-06），不可变、只增不改 ────────────────────────
CREATE TABLE docproc.artifact (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    processing_job_id uuid NOT NULL REFERENCES docproc.processing_job (id),
    artifact_type text NOT NULL,                       -- 7 值见约束
    storage_key text NOT NULL,
    mime_type text NOT NULL,
    checksum text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,       -- V1 含 OCR 页级记录（规格 4.6）
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT artifact_type_check CHECK (artifact_type = ANY (ARRAY[
        'markdown','extracted_text','image','table_data','ocr_result','conversion_report','export_file'])),
    CONSTRAINT artifact_key_nonempty CHECK (length(btrim(storage_key)) > 0),
    CONSTRAINT artifact_unique_in_job UNIQUE (processing_job_id, artifact_type, checksum)
);
-- 「转换器升级不覆盖历史产物」由 append-only 保证：新 Job 产生新 Artifact 行，旧行永存

CREATE INDEX idx_artifact_job ON docproc.artifact (processing_job_id);
CREATE INDEX idx_artifact_type ON docproc.artifact (tenant_id, artifact_type, created_at DESC);

-- 守卫：禁 DELETE；归属与内容键不可变
CREATE FUNCTION docproc.guard_artifact() RETURNS trigger
    LANGUAGE plpgsql AS $$
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

-- ── ⑤ Source Mapping：原文↔产物追溯（§6.5），job 写一次后不可变 ───────────
CREATE TABLE docproc.source_mapping (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    artifact_id uuid NOT NULL REFERENCES docproc.artifact (id),
    source_location jsonb NOT NULL,    -- {page:5} / {slide:3} / {section_path:[...]} / {table_index:2}
    target_location jsonb NOT NULL,    -- {heading:"## 1.1"} / {line_start, line_end} / {table_ref}
    mapping_type text NOT NULL,        -- page_to_section / slide_to_section / table_to_table / asset_ref
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT mapping_type_check CHECK (mapping_type = ANY (ARRAY[
        'page_to_section','slide_to_section','table_to_table','asset_ref']))
);

CREATE INDEX idx_mapping_artifact ON docproc.source_mapping (artifact_id);
```

## 4. 关键不变式

### 4.1 归属与隔离（D-01）
- 全表 `tenant_id NOT NULL`；任何读/下载路径必须带 tenant 过滤（API 层强制，规格 §9.11）。
- `project_id` 非空时：应用层校验 `core.project.tenant_id = document.tenant_id`；不建跨 schema 硬 FK（同 design 域先例）。
- 跨 Team 读 → 按现有权限语义返回空集/404（与 S03 审计结论一致：集合读跨项目=200 空集）。

### 4.2 版本隔离（D-06）
- 原始文件变化 → 新 `document_version`（version_number = max+1，单事务分配，U-11 并发上传由 document.row_revision 乐观锁仲裁）。
- 转换器/配置升级 → 同一 version 下新 `processing_job`（新 config_fingerprint）→ 新 `artifact`。**任何路径都不改写旧行**（守卫触发器兜底）。
- Document.current_version_id 仅指针更新；下载原始文件永远经由 version 行解析 storage_key。

### 4.3 任务幂等与重试（§5.3，状态机细节归 TD-04）
- 入队幂等：`uq_job_active_config` 部分唯一索引——重复提交同配置任务命中唯一冲突，API 返回既有 job。
- 重试 = 同一行 FAILED → pending（attempts+1），不新建行、不产生重复 artifact（artifact 以 (job, type, checksum) 唯一兜底）。
- 任务失败不删任何产物与原始文件（守卫触发器 + 状态机）。

### 4.4 存储键约定（对齐 RustFS 落地，U-01 闭合：复用 file-service）
- **复用 file-service**（digua_file_sdk，per-tenant 服务 JWT，同 pm-api files.rs 代理模式）——RustFS bucket/物理路径由 file-service 托管，docproc **不直连 S3**（TD-01-2 就此闭合：无独立 bucket）。
- `storage_key` 列存 file-service 返回的文件ID；下载/预签名走 file-service 契约（docproc API 返回受控下载地址，规格 §8.4）。

## 5. 与现有域的边界

| 现有对象 | 关系 |
|---|---|
| `ai.project_document` / `ai.document_revision`（Git 权威知识文档） | **不碰**。docproc 产物进正式知识 = 业务域自行复制/确认（D-05、U-12） |
| `ai.document_section_index`（PageIndex） | 转换成功不触发索引；由业务域显式调用（规格验收「索引隔离」） |
| `integration.outbox_event`（任务完成事件，TD-02 用） | **注意冲突：其 project_id NOT NULL**——Team 级无项目文档发事件需哨兵值或改列。TD-02 必须裁决 |
| `core.tenant` / `core.project` | 引用不建 FK（§4.1） |

## 6. 本设计遗留到后续 TD 的事项

| 编号 | 事项 | 归属 |
|---|---|---|
| TD-01-1 | Team=tenant 映射确认 | 本文档评审 |
| TD-01-2 | ~~docproc 独立 bucket vs 复用 file-service bucket~~ **已闭合（§4.4）：复用 file-service，无独立 bucket** | ✅ |
| TD-01-3 | 单文件大小/数量上限默认值 | U-04（TD-04 给配置面） |
| TD-01-4 | outbox_event.project_id NOT NULL 的处理（哨兵 vs 改列） | TD-02 |
| TD-01-5 | OCR 页级记录溢出 metadata 后是否独立成表 | P1 评估 |

---

**评审通过后**：DDL 直接作为 `migrations/0016_docproc_domain.sql` 正文（按 §1 惯例补注释头），无需另行转换。
