-- 0025_projc_domain_bootstrap.sql
--
-- P3-11 R-01/R-02：Project Cognition 独立 schema（projc.*）机制引导。
-- 裁决依据：《Phase3-裁决稿-P3-11仓库分析认知归属与双域治理-v1.1.md》（R-01 独立
-- schema、R-02 双域权威边界、R-03 模块消歧、R-05 delivery_activity 不纳入、R-06
-- pc.conflict=Legacy）及专家评审意见（DomainRef target_domain 与
-- relationship_type.owner_domain 分离；共享基础设施 ≠ 共享业务权威）。
--
-- 复用 pc.* 已验证的机制模式（内容寻址 + CAS 推号、行不可变触发器、生命周期
-- CHECK、提案锚定、幂等账本），不复用业务表（红线：共享技术基础设施不得产生
-- 共享业务权威；四域注册表 2026-11-16 实证 pc.* 核心对象全库 0 行=迁移红利窗口）。
-- AGENTS.md 规则 1：新领域从零建表，无存量、无兼容分支。
--
-- Stage 1 范围（对象/修订/版本/提案/审批/来源/证据/账本 13 表 + 触发器 + 索引）：
--   * 服务层写入者：pm-application::projc（upsert_object_draft / register_source /
--     register_evidence）——仓库分析链重挂（T-2）的目标落点。
--   * 提案/审批表结构先建（零写入者，T-2 启用），供审阅流 v2 路由与 close 分流发布。
-- 明确不含（后续任务包，均已在裁决稿登记）：
--   * projc.relationship(_type/_revision) 与成员端点闭合触发器 → T-2（与关系草稿
--     服务同批；跨域关系类型注册沿用 0007 owner_domain 机制）。
--   * projc.conflict(+object) → P3-D04（R-06：pc.conflict 即日起 Legacy /
--     Not for New Use，不复活、不视图伪装迁移）。
--   * v_revision_versioned 视图、FTS 索引 → 检索适配层（P3-10 前置任务）。
--   * close_report 过渡守卫（T-1）→ 独立实施任务项，等 P2-01~08 落地
--     （P2「领域零改动红线」禁改 close 事务；E2E-02 断言随 T-1 同步调整）。
--
-- 有意语义差异（对照 pc.*，均登记于差异报告）：
--   * origin_kind 增 'repo_analysis'（仓库分析 Finding 起草的提案）；
--   * source_kind 以 'product_cognition' / 'repo_analysis' 替换 pc 侧
--     'project_cognition'（target_domain 语义分离：本表登记的是来源指向）；
--   * write_operation 命令前缀 projc_（PCGIT 后置同 pc：暂只发布幂等用）；
--   * 文档 slug 前缀 projc-object-、canonical_path projc/object/；
--   * projc.object_type_singleton 恒 false（V1 无单例技术对象；函数保留以承载
--     uq 部分索引机制）；
--   * 对象字典种子不带 'snapshot' layer——本域对象是一等技术认知（R-05：动态
--     指标不得升级为正式认知对象）。
--
-- 正文权威：PG content 列（与 pc/kb 同款已知偏差；Git 权威为目标态，P3-D02.2
-- 冻结裁决：不进 Phase 3 关键路径，禁 PG/Git 双主）。

CREATE SCHEMA projc;

-- ── 1. 对象字典 ─────────────────────────────────────────────────────────────

CREATE TABLE projc.object_type (
    type_key text NOT NULL,
    label text NOT NULL,
    description text,
    layer text DEFAULT 'snapshot'::text NOT NULL,
    governance_level text DEFAULT 'L1'::text NOT NULL,
    singleton boolean DEFAULT false NOT NULL,
    doc_template_key text,
    status text DEFAULT 'active'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT object_type_governance_level_check CHECK ((governance_level = ANY (ARRAY['L1'::text, 'L2'::text]))),
    CONSTRAINT object_type_layer_check CHECK ((layer = ANY (ARRAY['L1_identity'::text, 'L2_structure'::text, 'L3_usage'::text, 'L4_semantic'::text, 'L5_decision'::text, 'snapshot'::text]))),
    CONSTRAINT object_type_status_check CHECK ((status = ANY (ARRAY['active'::text, 'deprecated'::text])))
);

INSERT INTO projc.object_type (type_key, label, description, layer, governance_level, singleton, status) VALUES
    ('repository', '仓库', '仓库技术认知身份对象（R-03 分流承接；含分支/HEAD/规模快照语义）', 'L1_identity', 'L1', false, 'active'),
    ('tech_stack', '技术栈', '语言/扩展名构成等技术构成认知', 'L2_structure', 'L1', false, 'active'),
    ('repository_module', '仓库模块', '仓库代码结构中的技术模块（专家 R-03 消歧：非 PC Product Module）', 'L2_structure', 'L1', false, 'active');

CREATE FUNCTION projc.object_type_singleton(key text) RETURNS boolean
    LANGUAGE sql IMMUTABLE
    AS $$ SELECT false $$;

-- ── 2. 对象身份与正文承载 ───────────────────────────────────────────────────

CREATE TABLE projc.cognition_object (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    object_type text NOT NULL,
    object_key text,
    display_name text NOT NULL,
    lifecycle_status text DEFAULT 'draft'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    current_revision_id uuid,
    superseded_by_id uuid,
    created_by uuid NOT NULL,
    updated_by uuid,
    deprecated_at timestamp with time zone,
    CONSTRAINT cognition_object_lifecycle_status_check CHECK ((lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text, 'deprecated'::text, 'superseded'::text])))
);

CREATE TABLE projc.document (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    doc_key text NOT NULL,
    canonical_path text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE TABLE projc.document_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    revision_no integer NOT NULL,
    git_commit_sha text,
    content_hash text NOT NULL,
    content text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_revision_revision_no_check CHECK ((revision_no >= 1))
);

CREATE TABLE projc.cognition_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    object_id uuid NOT NULL,
    revision_no integer NOT NULL,
    change_summary text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    analysis_mode text DEFAULT 'derived'::text,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    content_hash text NOT NULL,
    source_context text,
    CONSTRAINT cognition_revision_analysis_mode_check CHECK ((analysis_mode = ANY (ARRAY['derived'::text, 'derived_fallback'::text, 'llm'::text]))),
    CONSTRAINT cognition_revision_revision_no_check CHECK ((revision_no >= 1))
);

-- ── 3. 正式版本（发布事务目标面；服务链 T-2 启用）───────────────────────────

CREATE TABLE projc.cognition_version (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version_no bigint NOT NULL,
    status text DEFAULT 'published'::text NOT NULL,
    confirmed_by uuid NOT NULL,
    confirmed_at timestamp with time zone DEFAULT now() NOT NULL,
    source_proposal_id uuid NOT NULL,
    snapshot_hash text NOT NULL,
    predecessor_version_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cognition_version_status_check CHECK ((status = ANY (ARRAY['published'::text, 'archived'::text]))),
    CONSTRAINT cognition_version_version_no_check CHECK ((version_no >= 1))
);

CREATE TABLE projc.version_member (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version_id uuid NOT NULL,
    object_id uuid,
    object_revision_id uuid,
    relationship_id uuid,
    relationship_revision_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT version_member_check CHECK ((((object_id IS NOT NULL) AND (object_revision_id IS NOT NULL) AND (relationship_id IS NULL) AND (relationship_revision_id IS NULL)) OR ((relationship_id IS NOT NULL) AND (relationship_revision_id IS NOT NULL) AND (object_id IS NULL) AND (object_revision_id IS NULL))))
);

-- 成员端点闭合触发器随 relationship 表（T-2）同批启用（pc.enforce_member_endpoint_closure 同款）。

CREATE TABLE projc.write_operation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    operation_id text NOT NULL,
    command text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    expected_content_hash text NOT NULL,
    git_commit_sha text,
    object_revision_id uuid,
    relationship_revision_id uuid,
    version_id uuid,
    last_error text,
    retry_count integer DEFAULT 0 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT write_operation_command_check CHECK ((command = ANY (ARRAY['projc_commit_object_revision'::text, 'projc_publish_version'::text]))),
    CONSTRAINT write_operation_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'git_committed'::text, 'completed'::text, 'failed'::text, 'abandoned'::text])))
);

-- ── 4. 提案 / 审批（结构先建，T-2 启用写入者）───────────────────────────────

CREATE TABLE projc.proposal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    rationale text NOT NULL,
    origin_kind text NOT NULL,
    source_id uuid,
    governance_level text NOT NULL,
    risk_level text DEFAULT 'medium'::text NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    expected_version_no bigint NOT NULL,
    proposal_content_hash text NOT NULL,
    status text DEFAULT 'draft'::text NOT NULL,
    submitted_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT proposal_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT proposal_expected_version_no_check CHECK ((expected_version_no >= 0)),
    CONSTRAINT proposal_governance_level_check CHECK ((governance_level = ANY (ARRAY['L1'::text, 'L2'::text]))),
    CONSTRAINT proposal_origin_kind_check CHECK ((origin_kind = ANY (ARRAY['ai_agent'::text, 'kb_memory'::text, 'manual_input'::text, 'requirement_change'::text, 'conflict_resolution'::text, 'repo_analysis'::text]))),
    CONSTRAINT proposal_risk_level_check CHECK ((risk_level = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text, 'critical'::text]))),
    CONSTRAINT proposal_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'submitted'::text, 'approved'::text, 'rejected'::text, 'withdrawn'::text, 'expired'::text, 'applied'::text, 'failed'::text])))
);

CREATE TABLE projc.proposal_member (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    change_kind text NOT NULL,
    object_id uuid,
    object_revision_id uuid,
    relationship_id uuid,
    relationship_revision_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT proposal_member_change_kind_check CHECK ((change_kind = ANY (ARRAY['create_object'::text, 'update_object'::text, 'create_relationship'::text, 'update_relationship'::text, 'deprecate_object'::text, 'deprecate_relationship'::text]))),
    CONSTRAINT proposal_member_check CHECK ((((object_id IS NOT NULL) AND (object_revision_id IS NOT NULL) AND (relationship_id IS NULL) AND (relationship_revision_id IS NULL)) OR ((relationship_id IS NOT NULL) AND (relationship_revision_id IS NOT NULL) AND (object_id IS NULL) AND (object_revision_id IS NULL)))),
    CONSTRAINT proposal_member_check1 CHECK (((change_kind <> ALL (ARRAY['update_object'::text, 'deprecate_object'::text])) OR (object_id IS NOT NULL))),
    CONSTRAINT proposal_member_check2 CHECK (((change_kind <> ALL (ARRAY['update_relationship'::text, 'deprecate_relationship'::text])) OR (relationship_id IS NOT NULL)))
);

CREATE TABLE projc.approval (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_content_hash text NOT NULL,
    expected_version_no bigint NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    decided_by uuid,
    decision_reason text,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT approval_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text])))
);

-- ── 5. 来源与证据（DomainRef target 侧契约：kb.memory_ref/pc.source 同款四字段）──

CREATE TABLE projc.source (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    target_domain text NOT NULL,
    target_type text NOT NULL,
    target_id text NOT NULL,
    target_revision_id text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    source_kind text NOT NULL,
    revision_missing_reason text,
    section_anchor text,
    start_line integer,
    description text,
    source_content_hash text,
    collected_by uuid NOT NULL,
    locator text,
    CONSTRAINT source_kb_document_channel_check CHECK (((source_kind <> 'kb_document'::text) OR ((target_domain = 'knowledge_base'::text) AND (target_revision_id IS NOT NULL)))),
    CONSTRAINT source_kind_check CHECK ((source_kind = ANY (ARRAY['kb_document'::text, 'kb_memory'::text, 'manual_input'::text, 'requirement'::text, 'requirement_revision'::text, 'discovery_finding'::text, 'product_cognition'::text, 'repo_analysis'::text, 'external_reference'::text]))),
    CONSTRAINT source_revision_missing_check CHECK (((target_revision_id IS NOT NULL) OR (revision_missing_reason IS NOT NULL)))
);

CREATE TABLE projc.evidence (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    source_id uuid,
    evidence_type text NOT NULL,
    evidence_hash text NOT NULL,
    verification_status text DEFAULT 'unverified'::text NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    object_revision_id uuid NOT NULL,
    relationship_revision_id uuid,
    target_domain text,
    target_type text,
    target_id text,
    target_revision_id text,
    file_path text,
    symbol text,
    code_range text,
    commit_sha text,
    section_anchor text,
    start_line integer,
    end_line integer,
    excerpt text,
    description text,
    created_by uuid NOT NULL,
    CONSTRAINT evidence_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT evidence_subject_xor_check CHECK (((object_revision_id IS NOT NULL) <> (relationship_revision_id IS NOT NULL))),
    CONSTRAINT evidence_type_check CHECK ((evidence_type = ANY (ARRAY['code'::text, 'test_result'::text, 'commit'::text, 'document_section'::text, 'meeting'::text, 'user_statement'::text, 'system_observation'::text]))),
    CONSTRAINT evidence_type_locator_check CHECK ((((evidence_type = ANY (ARRAY['code'::text, 'test_result'::text, 'commit'::text])) AND (file_path IS NOT NULL) AND (commit_sha IS NOT NULL) AND (target_domain IS NULL)) OR ((evidence_type = 'document_section'::text) AND (target_domain = 'knowledge_base'::text) AND (target_revision_id IS NOT NULL) AND (file_path IS NULL) AND (commit_sha IS NULL)) OR ((evidence_type = ANY (ARRAY['meeting'::text, 'user_statement'::text, 'system_observation'::text])) AND ((excerpt IS NOT NULL) OR (description IS NOT NULL)) AND (target_domain IS NULL)))),
    CONSTRAINT evidence_verification_status_check CHECK ((verification_status = ANY (ARRAY['unverified'::text, 'verified'::text, 'refuted'::text, 'conflicted'::text])))
);

-- ── 6. 触发器函数（pc.* 同款语义，域内改前缀）───────────────────────────────

CREATE FUNCTION projc.enforce_object_lifecycle() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'projc.cognition_object % is terminal (%)', OLD.id, OLD.lifecycle_status;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded object % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION projc.forbid_identity_delete() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.lifecycle_status <> 'identity' THEN
        RAISE EXCEPTION 'projc.% % is not identity; deletion is forbidden', TG_TABLE_NAME, OLD.id;
    END IF;
    RETURN OLD;
END;
$$;

CREATE FUNCTION projc.forbid_row_update() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'row % in %.% is immutable', OLD.id, TG_TABLE_SCHEMA, TG_TABLE_NAME;
END;
$$;

CREATE FUNCTION projc.forbid_version_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (OLD.version_no IS DISTINCT FROM NEW.version_no)
       OR (OLD.confirmed_by IS DISTINCT FROM NEW.confirmed_by)
       OR (OLD.confirmed_at IS DISTINCT FROM NEW.confirmed_at)
       OR (OLD.source_proposal_id IS DISTINCT FROM NEW.source_proposal_id)
       OR (OLD.snapshot_hash IS DISTINCT FROM NEW.snapshot_hash)
       OR (OLD.predecessor_version_id IS DISTINCT FROM NEW.predecessor_version_id)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'projc.cognition_version % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'published' AND NEW.status NOT IN ('published','archived') THEN
        RAISE EXCEPTION 'projc.cognition_version % status may only move published -> archived', OLD.id;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION projc.enforce_write_op_state() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status IN ('completed','abandoned') AND NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'projc.write_operation % is terminal (%)', OLD.id, OLD.status;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION projc.forbid_proposal_content_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status <> 'draft' THEN
        IF (OLD.title IS DISTINCT FROM NEW.title)
           OR (OLD.rationale IS DISTINCT FROM NEW.rationale)
           OR (OLD.origin_kind IS DISTINCT FROM NEW.origin_kind)
           OR (OLD.governance_level IS DISTINCT FROM NEW.governance_level)
           OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
           OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
           OR (OLD.source_id IS DISTINCT FROM NEW.source_id) THEN
            RAISE EXCEPTION 'projc.proposal % is %; content anchor columns are immutable', OLD.id, OLD.status;
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION projc.enforce_proposal_member_anchored() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE p_status TEXT;
BEGIN
    SELECT status INTO p_status FROM projc.proposal
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.proposal_id, OLD.proposal_id);
    IF p_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'proposal % is %; members are anchored by proposal_content_hash',
            COALESCE(NEW.proposal_id, OLD.proposal_id), p_status;
    END IF;
    RETURN NULL;
END;
$$;

CREATE FUNCTION projc.forbid_approval_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (OLD.proposal_id IS DISTINCT FROM NEW.proposal_id)
       OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
       OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'projc.approval % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'pending' THEN
        IF NEW.status NOT IN ('approved','rejected') THEN
            RAISE EXCEPTION 'projc.approval % may only move from pending to a terminal status', OLD.id;
        END IF;
        IF NEW.decided_by IS NULL OR NEW.decided_at IS NULL THEN
            RAISE EXCEPTION 'projc.approval % terminal status requires decider and time', OLD.id;
        END IF;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'projc.approval % is already decided and cannot change status', OLD.id;
    END IF;
    RETURN NEW;
END;
$$;

-- ── 7. 触发器 ───────────────────────────────────────────────────────────────

CREATE TRIGGER trg_projc_object_delete BEFORE DELETE ON projc.cognition_object
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_identity_delete();
CREATE TRIGGER trg_projc_object_lifecycle BEFORE UPDATE ON projc.cognition_object
    FOR EACH ROW EXECUTE FUNCTION projc.enforce_object_lifecycle();
CREATE TRIGGER trg_projc_document_revision_immutable BEFORE DELETE OR UPDATE ON projc.document_revision
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();
CREATE TRIGGER trg_projc_cognition_version_immutable BEFORE UPDATE ON projc.cognition_version
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_version_mutation();
CREATE TRIGGER trg_projc_write_op_state BEFORE UPDATE ON projc.write_operation
    FOR EACH ROW EXECUTE FUNCTION projc.enforce_write_op_state();
CREATE TRIGGER trg_projc_proposal_anchors BEFORE UPDATE ON projc.proposal
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_proposal_content_mutation();
CREATE TRIGGER trg_projc_proposal_member_anchored BEFORE INSERT OR UPDATE OR DELETE ON projc.proposal_member
    FOR EACH ROW EXECUTE FUNCTION projc.enforce_proposal_member_anchored();
CREATE TRIGGER trg_projc_approval_immutable BEFORE UPDATE ON projc.approval
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_approval_mutation();

-- ── 8. 索引（pc.* 同款子集；relationship 侧索引随 T-2）──────────────────────

CREATE INDEX idx_projc_cognition_revision_object ON projc.cognition_revision USING btree (object_id, revision_no DESC);
CREATE INDEX idx_projc_revision_document ON projc.cognition_revision USING btree (tenant_id, project_id, document_revision_id);
CREATE INDEX idx_projc_evidence_subject ON projc.evidence USING btree (tenant_id, project_id, object_revision_id);
CREATE INDEX idx_projc_proposal_status ON projc.proposal USING btree (tenant_id, project_id, status, created_at DESC);
CREATE INDEX idx_projc_source_kind ON projc.source USING btree (tenant_id, project_id, source_kind);
CREATE INDEX idx_projc_source_target ON projc.source USING btree (tenant_id, project_id, target_domain, target_type);
CREATE INDEX idx_projc_vm_version ON projc.version_member USING btree (tenant_id, project_id, version_id);
CREATE INDEX idx_projc_vm_object ON projc.version_member USING btree (tenant_id, project_id, object_id, object_revision_id);
CREATE INDEX idx_projc_write_op_pending ON projc.write_operation USING btree (tenant_id, project_id, status) WHERE (status = ANY (ARRAY['pending'::text, 'git_committed'::text, 'failed'::text]));

CREATE UNIQUE INDEX uq_projc_object_singleton ON projc.cognition_object USING btree (tenant_id, project_id, object_type) WHERE ((projc.object_type_singleton(object_type) = true) AND (lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text])));
CREATE UNIQUE INDEX uq_projc_evidence_identity ON projc.evidence USING btree (tenant_id, project_id, source_id, COALESCE((object_revision_id)::text, (relationship_revision_id)::text), evidence_type, COALESCE(file_path, ''::text), COALESCE(section_anchor, ''::text), COALESCE(code_range, ''::text), COALESCE(commit_sha, ''::text));
CREATE UNIQUE INDEX uq_projc_approval_pending ON projc.approval USING btree (proposal_id) WHERE (status = 'pending'::text);
CREATE UNIQUE INDEX uq_projc_pm_target ON projc.proposal_member USING btree (tenant_id, project_id, proposal_id, COALESCE((object_id)::text, ''::text), COALESCE((relationship_id)::text, ''::text)) WHERE (change_kind = ANY (ARRAY['update_object'::text, 'deprecate_object'::text, 'update_relationship'::text, 'deprecate_relationship'::text]));
CREATE UNIQUE INDEX uq_projc_vm_object ON projc.version_member USING btree (tenant_id, project_id, version_id, object_id, object_revision_id) WHERE (object_id IS NOT NULL);
CREATE UNIQUE INDEX uq_projc_vm_relationship ON projc.version_member USING btree (tenant_id, project_id, version_id, relationship_id, relationship_revision_id) WHERE (relationship_id IS NOT NULL);
