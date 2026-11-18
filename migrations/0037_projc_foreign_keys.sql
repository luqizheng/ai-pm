-- 0037_projc_foreign_keys.sql
--
-- P3-D04-02 Step6 前置 GAP-38-B：projc 外键补齐（负责人裁决「按推荐实施」2026-11-18）。
-- 79 条边=pc 真源约束名/列序/DEFERRABLE 逐字镜像（pc→projc 换 schema；core.* 不变）；
-- 全部 NO ACTION（pc 默认逐字）。前置：38-A PK/业务身份 UNIQUE（0036）在位。
-- FK 引用面 UNIQUE ×12（pc 同名镜像；引用列组合此前未在 projc 建——38-A「业务身份必要」
-- 口径下属 FK 引用面必需，非冗余身份族复制）。
-- 实施前孤儿预检=79 边全部 0（2026-11-18 dev 库实测）；AGENTS.md 规则 1：纯增量无数据迁移。

-- ── 1. FK 引用面 UNIQUE ×12 ────────────────────────────────────────────────
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_tenant_id_project_id_object_id_id_key
    UNIQUE (tenant_id, project_id, object_id, id);
ALTER TABLE projc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.conflict
    ADD CONSTRAINT conflict_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.document
    ADD CONSTRAINT document_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_document_id_id_key
    UNIQUE (tenant_id, project_id, document_id, id);
ALTER TABLE projc.proposal
    ADD CONSTRAINT proposal_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_relationship_id_id_key
    UNIQUE (tenant_id, project_id, relationship_id, id);
ALTER TABLE projc.source
    ADD CONSTRAINT source_tenant_id_project_id_id_key
    UNIQUE (tenant_id, project_id, id);

-- ── 2. FK ×79（pc 逐字镜像）────────────────────────────────────────────────

-- approval
ALTER TABLE projc.approval
    ADD CONSTRAINT approval_decided_by_fkey
    FOREIGN KEY (decided_by) REFERENCES core.principal(id);
ALTER TABLE projc.approval
    ADD CONSTRAINT approval_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.approval
    ADD CONSTRAINT approval_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.approval
    ADD CONSTRAINT approval_tenant_id_project_id_proposal_id_fkey
    FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES projc.proposal(tenant_id, project_id, id);

-- cognition_object
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_current_revision_fkey
    FOREIGN KEY (tenant_id, project_id, id, current_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, object_id, id) DEFERRABLE;
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_object_type_fkey
    FOREIGN KEY (object_type) REFERENCES projc.object_type(type_key);
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_superseded_by_fkey
    FOREIGN KEY (tenant_id, project_id, superseded_by_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.cognition_object
    ADD CONSTRAINT cognition_object_updated_by_fkey
    FOREIGN KEY (updated_by) REFERENCES core.principal(id);

-- cognition_revision
ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_document_fkey
    FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES projc.document(tenant_id, project_id, id);
ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_document_revision_fkey
    FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id) REFERENCES projc.document_revision(tenant_id, project_id, document_id, id);
ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_composite_fkey
    FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_project_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);

-- cognition_version
ALTER TABLE projc.cognition_version
    ADD CONSTRAINT cognition_version_confirmed_by_fkey
    FOREIGN KEY (confirmed_by) REFERENCES core.principal(id);
ALTER TABLE projc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_source_proposal_id_fkey
    FOREIGN KEY (tenant_id, project_id, source_proposal_id) REFERENCES projc.proposal(tenant_id, project_id, id);

-- conflict
ALTER TABLE projc.conflict
    ADD CONSTRAINT conflict_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.conflict
    ADD CONSTRAINT conflict_resolved_by_fkey
    FOREIGN KEY (resolved_by) REFERENCES core.principal(id);
ALTER TABLE projc.conflict
    ADD CONSTRAINT conflict_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.conflict
    ADD CONSTRAINT conflict_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);

-- conflict_object
ALTER TABLE projc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_project_id_conflict_id_fkey
    FOREIGN KEY (tenant_id, project_id, conflict_id) REFERENCES projc.conflict(tenant_id, project_id, id);
ALTER TABLE projc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_project_id_object_id_fkey
    FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_project_id_relationship_id_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES projc.relationship(tenant_id, project_id, id);

-- document
ALTER TABLE projc.document
    ADD CONSTRAINT document_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.document
    ADD CONSTRAINT document_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);

-- document_revision
ALTER TABLE projc.document_revision
    ADD CONSTRAINT document_revision_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_document_id_fkey
    FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES projc.document(tenant_id, project_id, id);
ALTER TABLE projc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);

-- evidence
ALTER TABLE projc.evidence
    ADD CONSTRAINT evidence_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.evidence
    ADD CONSTRAINT evidence_object_revision_fkey
    FOREIGN KEY (tenant_id, project_id, object_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, id);
ALTER TABLE projc.evidence
    ADD CONSTRAINT evidence_relationship_revision_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_revision_id) REFERENCES projc.relationship_revision(tenant_id, project_id, id);
ALTER TABLE projc.evidence
    ADD CONSTRAINT evidence_source_composite_fkey
    FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES projc.source(tenant_id, project_id, id);
ALTER TABLE projc.evidence
    ADD CONSTRAINT evidence_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);

-- proposal
ALTER TABLE projc.proposal
    ADD CONSTRAINT proposal_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.proposal
    ADD CONSTRAINT proposal_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.proposal
    ADD CONSTRAINT proposal_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.proposal
    ADD CONSTRAINT proposal_tenant_id_project_id_source_id_fkey
    FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES projc.source(tenant_id, project_id, id);

-- proposal_member
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_object_id_fkey
    FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_object_id_object_revi_fkey
    FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, object_id, id);
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_proposal_id_fkey
    FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES projc.proposal(tenant_id, project_id, id);
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_relationship_id_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES projc.relationship(tenant_id, project_id, id);
ALTER TABLE projc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_relationship_id_relat_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id) REFERENCES projc.relationship_revision(tenant_id, project_id, relationship_id, id);

-- relationship
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_current_revision_fkey
    FOREIGN KEY (tenant_id, project_id, id, current_revision_id) REFERENCES projc.relationship_revision(tenant_id, project_id, relationship_id, id) DEFERRABLE;
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_relationship_type_key_fkey
    FOREIGN KEY (relationship_type_key) REFERENCES projc.relationship_type(type_key);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_source_object_id_fkey
    FOREIGN KEY (tenant_id, project_id, source_object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_superseded_by_id_fkey
    FOREIGN KEY (tenant_id, project_id, superseded_by_id) REFERENCES projc.relationship(tenant_id, project_id, id);
ALTER TABLE projc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_target_object_id_fkey
    FOREIGN KEY (tenant_id, project_id, target_object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);

-- relationship_revision
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_relationship_type_key_fkey
    FOREIGN KEY (relationship_type_key) REFERENCES projc.relationship_type(type_key);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_document_id_doc_fkey
    FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id) REFERENCES projc.document_revision(tenant_id, project_id, document_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_document_id_fkey
    FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES projc.document(tenant_id, project_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_relationship_id_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES projc.relationship(tenant_id, project_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_source_object__fkey1
    FOREIGN KEY (tenant_id, project_id, source_object_id, source_object_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, object_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_source_object_i_fkey
    FOREIGN KEY (tenant_id, project_id, source_object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_target_object__fkey1
    FOREIGN KEY (tenant_id, project_id, target_object_id, target_object_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, object_id, id);
ALTER TABLE projc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_target_object_i_fkey
    FOREIGN KEY (tenant_id, project_id, target_object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);

-- source
ALTER TABLE projc.source
    ADD CONSTRAINT source_collected_by_fkey
    FOREIGN KEY (collected_by) REFERENCES core.principal(id);
ALTER TABLE projc.source
    ADD CONSTRAINT source_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);

-- version_member
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_object_id_fkey
    FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES projc.cognition_object(tenant_id, project_id, id);
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_object_id_object_revis_fkey
    FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id) REFERENCES projc.cognition_revision(tenant_id, project_id, object_id, id);
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_relationship_id_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES projc.relationship(tenant_id, project_id, id);
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_relationship_id_relati_fkey
    FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id) REFERENCES projc.relationship_revision(tenant_id, project_id, relationship_id, id);
ALTER TABLE projc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_version_id_fkey
    FOREIGN KEY (tenant_id, project_id, version_id) REFERENCES projc.cognition_version(tenant_id, project_id, id);

-- write_operation
ALTER TABLE projc.write_operation
    ADD CONSTRAINT write_operation_created_by_fkey
    FOREIGN KEY (created_by) REFERENCES core.principal(id);
ALTER TABLE projc.write_operation
    ADD CONSTRAINT write_operation_tenant_id_fkey
    FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);
ALTER TABLE projc.write_operation
    ADD CONSTRAINT write_operation_tenant_id_project_id_fkey
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);
