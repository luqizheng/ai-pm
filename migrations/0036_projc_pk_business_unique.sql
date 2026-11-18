-- 0036_projc_pk_business_unique.sql
--
-- P3-D04-02 Step6 前置 GAP-38-A：projc 补主键（18 表）与业务身份唯一约束（4 条）
-- （负责人裁决批准设立 2026-11-18，拆分口径：38-A=PK+UNIQUE，38-B=FK 独立分析）。
-- 核对记录（实施前实测=合并任务书 §二）：16 表 PK 列=id；object_type/
-- relationship_type=type_key（与 pc 同款）；4 项业务身份组合重复=0；
-- 全部 PK 列现状 NOT NULL（无 NULL 语义变更）；已有唯一索引/约束不重复创建。
-- pc 侧 (tenant_id,id)/(tenant,project,id) 冗余身份族不复制（PK 后完整性功能
-- 被吸收——「按实际业务身份补充必要的」裁决口径）。
-- AGENTS.md 规则 1：纯增量无数据迁移。

-- ── 1. PRIMARY KEY ×18 ─────────────────────────────────────────────────────

ALTER TABLE projc.approval              ADD CONSTRAINT approval_pkey PRIMARY KEY (id);
ALTER TABLE projc.cognition_object      ADD CONSTRAINT cognition_object_pkey PRIMARY KEY (id);
ALTER TABLE projc.cognition_revision    ADD CONSTRAINT cognition_revision_pkey PRIMARY KEY (id);
ALTER TABLE projc.cognition_version     ADD CONSTRAINT cognition_version_pkey PRIMARY KEY (id);
ALTER TABLE projc.conflict              ADD CONSTRAINT conflict_pkey PRIMARY KEY (id);
ALTER TABLE projc.conflict_object       ADD CONSTRAINT conflict_object_pkey PRIMARY KEY (id);
ALTER TABLE projc.document              ADD CONSTRAINT document_pkey PRIMARY KEY (id);
ALTER TABLE projc.document_revision     ADD CONSTRAINT document_revision_pkey PRIMARY KEY (id);
ALTER TABLE projc.evidence              ADD CONSTRAINT evidence_pkey PRIMARY KEY (id);
ALTER TABLE projc.object_type           ADD CONSTRAINT object_type_pkey PRIMARY KEY (type_key);
ALTER TABLE projc.proposal              ADD CONSTRAINT proposal_pkey PRIMARY KEY (id);
ALTER TABLE projc.proposal_member       ADD CONSTRAINT proposal_member_pkey PRIMARY KEY (id);
ALTER TABLE projc.relationship          ADD CONSTRAINT relationship_pkey PRIMARY KEY (id);
ALTER TABLE projc.relationship_revision ADD CONSTRAINT relationship_revision_pkey PRIMARY KEY (id);
ALTER TABLE projc.relationship_type     ADD CONSTRAINT relationship_type_pkey PRIMARY KEY (type_key);
ALTER TABLE projc.source                ADD CONSTRAINT source_pkey PRIMARY KEY (id);
ALTER TABLE projc.version_member        ADD CONSTRAINT version_member_pkey PRIMARY KEY (id);
ALTER TABLE projc.write_operation       ADD CONSTRAINT write_operation_pkey PRIMARY KEY (id);

-- ── 2. 业务身份 UNIQUE ×4（pc 同款约束名逐字镜像）───────────────────────────

ALTER TABLE projc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_version_no_key
    UNIQUE (tenant_id, project_id, version_no);
ALTER TABLE projc.document
    ADD CONSTRAINT document_tenant_id_project_id_doc_key_key
    UNIQUE (tenant_id, project_id, doc_key);
ALTER TABLE projc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_document_id_revision_key
    UNIQUE (tenant_id, project_id, document_id, revision_no);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_relationship_id__key
    UNIQUE (tenant_id, project_id, relationship_id, revision_no);
