-- 0007_relationship_owner_domain_seeds.sql
--
-- WP-2 · 关系种子与多跳遍历引擎（检索架构 V1.0 任务书 §三）。
--
-- 背景：跨域多跳检索（R-03 / Q2=A）的两块前置——① 关系类型注册表补 owner_domain
-- 与指向 KB 端点的新种子；② 遍历需要**双端隔离**（关系两端的端点对象必须同租户
-- 同项目，不能只查 relationship 自身归属——否则跨项目引用的脏数据会经端点连接
-- 渗入遍历结果）。
--
-- ── owner_domain 语义（WP-2 终裁）────────────────────────────────────────
-- owner_domain = **该类型关系实例的确认权威域**，**不是**「哪个服务暂时能创建或
-- 维护这条关系」。PC 不拥有 S02/KB 对象，就没有资格做这些关系的确认权威。
--
-- 允许复用（基础设施层）：关系存储结构 / draft-active 状态表达 / 关系类型注册表 /
-- 端点合法性校验 / 审计 / 通用状态迁移底层能力。
-- 不允许混用（业务确认层）：PC 关系的业务确认权限 / PC 的 Proposal-Publish 流程 /
-- PC 的正式认知确认入口 / KB 的 Document Revision 发布确认流程。
--
-- 终裁：requirement-implemented-by-design 的 owner = **kb**（不是 pc 代持）——
-- V1 生产恒 draft（KB 独立关系确认面 Phase 7 交付后才可 active），RT-005 以
-- fixture Active 行验收引擎能力。
--
-- ── 关系类型与 owner_domain 必须一致（裁决要求 ③④）──────────────────────────
-- 1) 存量 12 条全回填 'pc'；
-- 2) 新增 KB 种子明确 'kb'；
-- 3) owner_domain 必须与关系两端的领域归属一致；
-- 4) 创建关系时校验关系类型所属领域（应用层；PcEndpoint::External 只说明可引用
--    外部对象，不代表任何领域都可创建任意关系——**禁双 ext 绕过 owner_domain**）；
-- 5) 查询恒过滤 status='active'；
-- 6) 既有 12 条 PC 种子不因遍历设计删除。

ALTER TABLE pc.relationship_type
    ADD COLUMN owner_domain text NOT NULL DEFAULT 'pc';

ALTER TABLE pc.relationship_type
    ADD CONSTRAINT relationship_type_owner_domain_check
    CHECK (owner_domain IN ('pc', 'kb'));

-- 存量 12 条：全部归 PC（既有语义不变）。
UPDATE pc.relationship_type SET owner_domain = 'pc' WHERE owner_domain = 'pc';

-- ── 新增关系类型种子（8 条；端点数组沿既有 12 条写法）──────────────────────
-- 语义映射见任务书 §6.4 勘误版：规则→需求用既有 constrains 反向覆盖（**不新增
-- rule-enforced-by-requirement**——同一条正式关系的两个查询方向）；能力/对象→需求
-- 保留直达边（链式可达 ≠ 语义等价）。

INSERT INTO pc.relationship_type
    (type_key, label, directionality, source_object_types, target_object_types,
     allow_self_reference, allow_duplicate, governance_level, status, description, owner_domain)
VALUES
    ('capability-realized-by-requirement', '能力由需求实现', 'directed',
     '{business_capability}', '{requirement.requirement_revision}', false, false,
     'L2', 'active', '业务能力由需求修订实现或支撑（直达边，保留独立语义）', 'pc'),
    ('object-referenced-by-requirement', '对象被需求引用', 'directed',
     '{domain_object}', '{requirement.requirement_revision}', false, false,
     'L2', 'active', '业务对象被需求修订直接引用（不经规则中转）', 'pc'),
    ('process-implemented-by-design', '流程由设计实现', 'directed',
     '{business_process}', '{kb.document_revision}', false, false,
     'L2', 'active', '业务流程由技术设计实现', 'pc'),
    ('module-covered-by-design', '模块由设计覆盖', 'directed',
     '{feature_module}', '{kb.document_revision}', false, false,
     'L2', 'active', '功能模块由技术设计覆盖', 'pc'),
    ('requirement-implemented-by-design', '需求由设计实现', 'directed',
     '{requirement.requirement_revision}', '{kb.document_revision}', false, false,
     'L2', 'active', '需求修订由技术设计实现（owner=kb：两端均非 PC 对象，PC 无确认权）', 'kb'),
    ('scenario-verified-by-acceptance', '场景由验收验证', 'directed',
     '{scenario}', '{delivery.requirement_acceptance_criterion}', false, false,
     'L2', 'active', '产品场景由需求验收条件验证', 'pc'),
    ('design-constrained-by-adr', '设计受决策约束', 'directed',
     '{kb.document_revision}', '{kb.document_revision}', false, false,
     'L2', 'active', '技术设计受 ADR 约束（KB 域内边；确认权威=KB）', 'kb'),
    ('adr-documented-in', '决策记录于技术文档', 'directed',
     '{kb.document_revision}', '{kb.document_revision}', false, false,
     'L2', 'active', '架构决策记录于技术文档（KB 域内边；确认权威=KB）', 'kb');

-- ── 端点作用域判定（双端隔离的机械入口）────────────────────────────────────
-- 遍历时，关系两端的端点对象必须属于同一租户/项目。KB 域内边的两端是 KB 文档，
-- PC 边的两端是 PC 认知对象，跨域边混合两端。统一用一个判定函数：
--   对象端（object_id 非空）→ pc.cognition_object 归属校验；
--   外部端（ext 四元组）→ 按 ext_domain 分派到对应域的表校验。

CREATE FUNCTION pc.endpoint_in_scope(
    p_object_id uuid,
    p_ext_domain text,
    p_ext_type text,
    p_ext_id text,
    p_tenant_id uuid,
    p_project_id uuid
) RETURNS boolean
    LANGUAGE plpgsql
    STABLE
    AS $$
DECLARE
    v_uuid uuid;
BEGIN
    IF p_object_id IS NOT NULL THEN
        RETURN EXISTS (
            SELECT 1 FROM pc.cognition_object o
             WHERE o.id = p_object_id
               AND o.tenant_id = p_tenant_id
               AND o.project_id = p_project_id
        );
    END IF;

    IF p_ext_id IS NULL OR p_ext_domain IS NULL THEN
        RETURN false;
    END IF;

    -- ext_id 必须是合法 UUID 才能安全转换（避免 ::uuid 抛错）。
    IF p_ext_id !~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$' THEN
        RETURN false;
    END IF;
    v_uuid := p_ext_id::uuid;

    IF p_ext_domain = 'knowledge_base' THEN
        RETURN EXISTS (
            SELECT 1 FROM kb.document d
             WHERE d.id = v_uuid
               AND d.tenant_id = p_tenant_id
               AND d.project_id = p_project_id
        );
    END IF;

    IF p_ext_domain = 'requirement' THEN
        RETURN EXISTS (
            SELECT 1 FROM delivery.requirement_revision r
             WHERE r.id = v_uuid
               AND r.project_id = p_project_id
        );
    END IF;

    IF p_ext_domain = 'delivery' THEN
        RETURN EXISTS (
            SELECT 1 FROM delivery.requirement_acceptance_criterion a
             WHERE a.id = v_uuid
               AND a.project_id = p_project_id
        );
    END IF;

    IF p_ext_domain = 'project_cognition' THEN
        RETURN EXISTS (
            SELECT 1 FROM ai.cognition_item i
             WHERE i.id = v_uuid
               AND i.tenant_id = p_tenant_id
               AND i.project_id = p_project_id
        );
    END IF;

    RETURN false;
END;
$$;

-- ── 遍历用索引（关系表 + 端点两列）──────────────────────────────────────────
CREATE INDEX IF NOT EXISTS idx_relationship_scope_active
    ON pc.relationship (tenant_id, project_id, lifecycle_status);

CREATE INDEX IF NOT EXISTS idx_relationship_type_active
    ON pc.relationship_type (status);
