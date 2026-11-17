-- 0026_projc_formal_chain.sql
--
-- P3-11 T-1/T-2/T-3 阶段一（纯增量，现网零行为变化）：projc 正式链机制补齐。
-- 依据：《Phase3-P3-11下一阶段实施方案-T1T2T3-v1.0.md》V1.1 §8.5 阶段一 +
-- W1 工作包；表结构真源 = migrations/0001_baseline.sql 同构行号在括号内注明。
--
-- 内容（全部为新增对象，零 pc.* 行为改动）：
--   * projc.relationship_type（0001:3369 同构 + 0007 owner_domain 列；CHECK 收敛
--     IN ('project_cognition')——本域关系确认权威恒为 ProjC）+ 种子 implements /
--     depends_on（directed L1；跨域端点走 0007 先例的 `domain.type` 键）。
--   * projc.relationship（0001:3293 同构）/ projc.relationship_revision（0001:3327
--     同构）。
--   * projc.conflict + projc.conflict_object（0001:3108/3134 同构；R-06：ProjC 自建
--     冲突对象，pc.conflict 维持 Legacy / Not for New Use，本迁移不触碰 pc.*）。
--   * projc.enforce_member_endpoint_closure（0001:314 同款指向 projc 表）+
--     version_member 端点闭合 CONSTRAINT TRIGGER（0001:7064 同款）。
--   * integration.repo_analysis_report ADD COLUMN projc_source_id uuid
--     （G-1 双源登记的 ProjC 快照通道锚；历史行为 NULL=显式 Legacy）。
--
-- 明确不含（阶段二）：pc.object_type snapshot 四类 status='deprecated' 字典废弃。
-- AGENTS.md 规则 1：无数据迁移、无回填、无兼容分支。

-- ── 1. 关系类型字典（确认权威域=ProjC）──────────────────────────────────────

CREATE TABLE projc.relationship_type (
    type_key text NOT NULL,
    label text NOT NULL,
    directionality text NOT NULL,
    source_object_types text[] NOT NULL,
    target_object_types text[] NOT NULL,
    allow_self_reference boolean DEFAULT false NOT NULL,
    allow_duplicate boolean DEFAULT false NOT NULL,
    governance_level text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    description text,
    owner_domain text NOT NULL DEFAULT 'project_cognition'::text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT relationship_type_directionality_check CHECK ((directionality = ANY (ARRAY['directed'::text, 'undirected'::text]))),
    CONSTRAINT relationship_type_governance_level_check CHECK ((governance_level = ANY (ARRAY['L1'::text, 'L2'::text]))),
    CONSTRAINT relationship_type_status_check CHECK ((status = ANY (ARRAY['active'::text, 'deprecated'::text]))),
    CONSTRAINT relationship_type_owner_domain_check CHECK ((owner_domain = 'project_cognition'::text))
);

-- 关系种子（directed L1；DomainRef target_domain 与 owner_domain 严格分离——
-- owner_domain 描述关系确认权威=ProjC，implements 的目标端对象身份属于 PC，
-- 引用方经 `pc.feature_module` ext 键与 DomainRef 契约指向，不构成跨域授权）。
INSERT INTO projc.relationship_type
    (type_key, label, directionality, source_object_types, target_object_types,
     allow_self_reference, allow_duplicate, governance_level, status, description, owner_domain)
VALUES
    ('implements', '实现（仓库模块→产品功能模块）', 'directed',
     '{repository_module}', '{pc.feature_module}', false, false,
     'L1', 'active',
     'Repository Module 实现 PC 的 Product Module（R-03 模块消歧：两域各自维护认知、经显式关系引用；ProjC 不因代码扫描自动创建 PC 对象）',
     'project_cognition'),
    ('depends_on', '依赖（仓库模块→仓库模块）', 'directed',
     '{repository_module}', '{repository_module}', false, false,
     'L1', 'active',
     '仓库模块间技术依赖（ProjC 域内边）',
     'project_cognition');

-- ── 2. 关系身份与修订（0001:3293/3327 同构）─────────────────────────────────

CREATE TABLE projc.relationship (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    relationship_type_key text NOT NULL,
    lifecycle_status text DEFAULT 'identity'::text NOT NULL,
    source_object_id uuid,
    target_object_id uuid,
    source_ext_domain text,
    source_ext_type text,
    source_ext_id text,
    source_ext_revision_id text,
    target_ext_domain text,
    target_ext_type text,
    target_ext_id text,
    target_ext_revision_id text,
    superseded_by_id uuid,
    current_revision_id uuid,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT relationship_check CHECK (((source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL))),
    CONSTRAINT relationship_check1 CHECK (((target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL))),
    CONSTRAINT relationship_check2 CHECK (((source_ext_id IS NULL) OR ((source_ext_domain IS NOT NULL) AND (source_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_check3 CHECK (((target_ext_id IS NULL) OR ((target_ext_domain IS NOT NULL) AND (target_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_check4 CHECK (((lifecycle_status <> 'superseded'::text) OR (superseded_by_id IS NOT NULL))),
    CONSTRAINT relationship_lifecycle_status_check CHECK ((lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text, 'deprecated'::text, 'superseded'::text])))
);

CREATE TABLE projc.relationship_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    relationship_id uuid NOT NULL,
    revision_no integer NOT NULL,
    relationship_type_key text NOT NULL,
    directionality text NOT NULL,
    source_object_id uuid,
    source_object_revision_id uuid,
    target_object_id uuid,
    target_object_revision_id uuid,
    source_ext_domain text,
    source_ext_type text,
    source_ext_id text,
    source_ext_revision_id text,
    target_ext_domain text,
    target_ext_type text,
    target_ext_id text,
    target_ext_revision_id text,
    properties jsonb DEFAULT '{}'::jsonb NOT NULL,
    change_summary text,
    document_id uuid,
    document_revision_id uuid,
    content_hash text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT relationship_revision_check CHECK (((source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check1 CHECK (((target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check2 CHECK (((source_object_id IS NULL) OR (source_object_revision_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check3 CHECK (((target_object_id IS NULL) OR (target_object_revision_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check4 CHECK (((source_ext_id IS NULL) OR ((source_ext_domain IS NOT NULL) AND (source_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_revision_check5 CHECK (((target_ext_id IS NULL) OR ((target_ext_domain IS NOT NULL) AND (target_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_revision_directionality_check CHECK ((directionality = ANY (ARRAY['directed'::text, 'undirected'::text]))),
    CONSTRAINT relationship_revision_revision_no_check CHECK ((revision_no >= 1))
);

-- ── 3. 冲突（0001:3108/3134 同构；R-06 ProjC 自建）──────────────────────────

CREATE TABLE projc.conflict (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conflict_type text NOT NULL,
    severity text NOT NULL,
    description text NOT NULL,
    detection_source text,
    status text DEFAULT 'detected'::text NOT NULL,
    resolution_note text,
    resolved_by uuid,
    resolved_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conflict_check CHECK (((status <> ALL (ARRAY['resolved'::text, 'dismissed'::text])) OR ((resolved_by IS NOT NULL) AND (resolved_at IS NOT NULL)))),
    CONSTRAINT conflict_conflict_type_check CHECK ((conflict_type = ANY (ARRAY['content_contradiction'::text, 'rule_contradiction'::text, 'scope_conflict'::text, 'relationship_conflict'::text, 'source_conflict'::text, 'revision_conflict'::text, 'reality_mismatch'::text, 'cross_domain_authority'::text]))),
    CONSTRAINT conflict_severity_check CHECK ((severity = ANY (ARRAY['critical'::text, 'major'::text, 'minor'::text]))),
    CONSTRAINT conflict_status_check CHECK ((status = ANY (ARRAY['detected'::text, 'under_review'::text, 'awaiting_human'::text, 'resolved'::text, 'dismissed'::text, 'reopened'::text])))
);

CREATE TABLE projc.conflict_object (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conflict_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    object_id uuid,
    relationship_id uuid,
    CONSTRAINT conflict_object_check CHECK (((object_id IS NOT NULL) <> (relationship_id IS NOT NULL)))
);

-- ── 4. 成员端点闭合（0001:314 函数同款指向 projc；0001:7064 触发器同款）──────

CREATE FUNCTION projc.enforce_member_endpoint_closure() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
    broken INT;
    v_tenant UUID; v_project UUID; v_version UUID;
BEGIN
    v_tenant  := COALESCE(NEW.tenant_id,  OLD.tenant_id);
    v_project := COALESCE(NEW.project_id, OLD.project_id);
    v_version := COALESCE(NEW.version_id, OLD.version_id);

    SELECT count(*) INTO broken
    FROM projc.version_member vm
    JOIN projc.relationship_revision rr
      ON (rr.tenant_id, rr.project_id, rr.relationship_id, rr.id)
       = (vm.tenant_id, vm.project_id, vm.relationship_id, vm.relationship_revision_id)
    WHERE (vm.tenant_id, vm.project_id, vm.version_id) = (v_tenant, v_project, v_version)
      AND ((rr.source_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM projc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.source_object_id
                  AND m2.object_revision_id = rr.source_object_revision_id))
        OR (rr.target_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM projc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.target_object_id
                  AND m2.object_revision_id = rr.target_object_revision_id)));

    IF broken > 0 THEN
        RAISE EXCEPTION 'version % has % relationship member(s) with endpoints missing from the version',
            v_version, broken;
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER trg_projc_vm_closure AFTER INSERT ON projc.version_member
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION projc.enforce_member_endpoint_closure();

-- ── 5. 0025 缺陷修正：proposal_member 锚定触发器绑定时序 ─────────────────────
-- 0025 将 `enforce_proposal_member_anchored` 误绑为 BEFORE 触发器——该函数约定
-- RETURN NULL（0001:377 同款语义），BEFORE 行触发器返回 NULL 会**静默吞掉全部
-- 成员插入**，提案/发布链因此必然空成员失败。pc 真源绑定为 AFTER 约束触发器
-- （0001:6994/7001），本迁移按真源重绑（阶段一纯增量范围内的正确性修正，随
-- 交付报告登记）。

DROP TRIGGER IF EXISTS trg_projc_proposal_member_anchored ON projc.proposal_member;
CREATE CONSTRAINT TRIGGER trg_projc_proposal_member_anchored
    AFTER DELETE OR UPDATE ON projc.proposal_member DEFERRABLE INITIALLY IMMEDIATE
    FOR EACH ROW EXECUTE FUNCTION projc.enforce_proposal_member_anchored();
CREATE CONSTRAINT TRIGGER trg_projc_proposal_member_anchored_ins
    AFTER INSERT ON projc.proposal_member DEFERRABLE INITIALLY IMMEDIATE
    FOR EACH ROW EXECUTE FUNCTION projc.enforce_proposal_member_anchored();

-- 0025 缺陷修正二：projc.write_operation 缺账本幂等唯一约束（pc 真源
-- 0001:5746 `write_operation_tenant_id_project_id_operation_id_key` 同款）；
-- 发布事务 ON CONFLICT (tenant_id, project_id, operation_id) 依赖它。
ALTER TABLE projc.write_operation
    ADD CONSTRAINT write_operation_tenant_id_project_id_operation_id_key
    UNIQUE (tenant_id, project_id, operation_id);

-- ── 6. 报告双源登记列（G-1；ProjC 快照通道锚）───────────────────────────────

ALTER TABLE integration.repo_analysis_report
    ADD COLUMN projc_source_id uuid;

-- ── 6. 索引（pc 同款子集）───────────────────────────────────────────────────

CREATE INDEX idx_projc_conflict_object ON projc.conflict_object USING btree (tenant_id, project_id, object_id);
CREATE INDEX idx_projc_conflict_rel ON projc.conflict_object USING btree (tenant_id, project_id, relationship_id);
CREATE INDEX idx_projc_conflict_status ON projc.conflict USING btree (tenant_id, project_id, status);
CREATE INDEX idx_projc_rel_revision_endpoint_src ON projc.relationship_revision USING btree (tenant_id, project_id, source_object_id, source_object_revision_id);
CREATE INDEX idx_projc_rel_revision_endpoint_tgt ON projc.relationship_revision USING btree (tenant_id, project_id, target_object_id, target_object_revision_id);
CREATE INDEX idx_projc_rel_revision_rel ON projc.relationship_revision USING btree (tenant_id, project_id, relationship_id, revision_no DESC);
CREATE INDEX idx_projc_relationship_source ON projc.relationship USING btree (tenant_id, project_id, source_object_id, lifecycle_status);
CREATE INDEX idx_projc_relationship_target ON projc.relationship USING btree (tenant_id, project_id, target_object_id, lifecycle_status);
CREATE INDEX idx_projc_relationship_scope_active ON projc.relationship USING btree (tenant_id, project_id, lifecycle_status);
CREATE INDEX idx_projc_relationship_type_active ON projc.relationship_type USING btree (status);

CREATE UNIQUE INDEX uq_projc_conflict_object ON projc.conflict_object USING btree (tenant_id, project_id, conflict_id, COALESCE((object_id)::text, ''::text), COALESCE((relationship_id)::text, ''::text));
CREATE UNIQUE INDEX uq_projc_relationship_identity ON projc.relationship USING btree (tenant_id, project_id, relationship_type_key, COALESCE((source_object_id)::text, ((((source_ext_domain || ':'::text) || source_ext_type) || ':'::text) || source_ext_id)), COALESCE((target_object_id)::text, ((((target_ext_domain || ':'::text) || target_ext_type) || ':'::text) || target_ext_id))) WHERE (lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text]));
