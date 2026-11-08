-- 0064_pc_formal_model：KP-1——PC-01B 正式数据模型落地（0053 最小表 → 正式模型收敛）。
-- 设计权威：docs/architecture/product-cognition/pc-01b/ 七主题文件 V1.2
-- （00 总纲 / 01 领域模型 / 02 对象与关系 / 03 版本成员与闭合 / 04 治理追溯 /
--   05 Git-PG 与跨域 / 06 约束与实施）；差异复核 C-1~C-4 已采纳。
--
-- 编号：0064（原取 0063，与 D-2=c runner_availability 撞号——对方 20:31:56
-- 先落地 4 秒，本批按台账纪律②后实施顺移；台账已回填双占记录）。KP-3 并行
-- 会话同日活跃，谁后落地谁顺移，禁改既有迁移。
--
-- 表清单（物理 18 张 = 01B §15.2 的 17 项 + pc.conflict_object——§14.2/§15.2
-- 矩阵均含该表，§1#1 摘要枚举漏计，以矩阵为准）：
--   类型注册：pc.object_type（0053 收敛）/ pc.relationship_type（新建）
--   正文承载：pc.document / pc.document_revision（新建，C-1 域内闭环）
--   核心六表：pc.cognition_object / pc.cognition_revision（0053 收敛）/
--             pc.relationship / pc.relationship_revision / pc.cognition_version /
--             pc.version_member（新建）
--   治理：pc.proposal / pc.proposal_member / pc.approval / pc.conflict /
--         pc.conflict_object（新建）
--   追溯：pc.source / pc.evidence（0053 收敛）
--   账本：pc.write_operation（新建）
--
-- 实施偏差登记（KP-1 显式裁决/默认，验收记录同源）：
-- * a) PCGIT 正文权威后置（范围基线 V1.3 §3.3 将「KBGIT/PCGIT 正文权威」显式
--      切出 V0.6）：pc.document_revision 暂存 content TEXT NOT NULL（正文权威
--      = PG，正式形态迁移时随 ADR-06b 框架处置）；git_commit_sha 可空（本批
--      无 Git 写路径，恒 NULL）。pc.write_operation 本批仅用于发布幂等
--      （command='pc_publish_version'），Git 两态走账本状态机、无 Git 侧动作。
-- * b) checkpoint-1 四派生类型保留（P2-13 裁决 D-2=A「对象形态不变」）：
--      repository / tech_stack / delivery_activity / module 以 layer='snapshot'
--      保留（01B §5.1「新增类型 = 种子迁移追加」同款受控路径），非 14 类正典。
-- * c) analysis_mode 列保留在 pc.cognition_revision（0062 既有，D-3=A 可观测）；
--      pc.document_revision 不重复该列。
-- * d) uq_pc_evidence_identity 增 coalesce(commit_sha,'')（01B §9.3 原式在同
--      subject 同 source 多 commit 证据下撞键——commit 类证据身份必须含锚）。
-- * e) 0053 存量对象 object_key 含 ':'/'/'（非 §11.2 slug），收敛 doc_key 用
--      兜底形态 pc-object-{object_id}，canonical_path = pc/{object_type}/{object_id}.md。
-- * 0053 收敛策略（用户裁决 2026-11-08：原地收敛保留草稿）：正文迁
--      pc.document_revision、证据按时间钉修订、locator JSONB 拆结构化列，
--      全部草稿行保留；同迁移对全新库（0053 空表）自然退化为主键路径。
-- * ADR-06b 六问时序闸：PC 正文自本迁移起域内闭环（pc.document*），PC 对
--      ai.document* 零新增引用（kb-pc-contracts guard_ai_container_users.rs
--      守卫持续断言）；KB 域 kb.* 本批零触碰。
--
-- M3 门不破：本批全部服务写入仍 draft/PROPOSED；AI 无确认权（正式写入闸 =
-- 真人主体 + A3 权限，归服务层与 KP-2 端点面）。

-- ════════════════════════════════════════════════════════════════════
-- ⓪ 公共触发器函数（01B §15.3 / §15.4 / §5.2 / §6.5 / §8.3 / §10 / §12.2）
-- ════════════════════════════════════════════════════════════════════

-- append-only 行禁改删（0026:257-262 同构）
CREATE FUNCTION pc.forbid_row_update() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    RAISE EXCEPTION 'row % in %.% is immutable', OLD.id, TG_TABLE_SCHEMA, TG_TABLE_NAME;
END;
$func$;

-- 版本头锚定保护（0026:273-296 同构）：锚定列 once-written；status 仅 published→archived
CREATE FUNCTION pc.forbid_version_mutation() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF (OLD.version_no IS DISTINCT FROM NEW.version_no)
       OR (OLD.confirmed_by IS DISTINCT FROM NEW.confirmed_by)
       OR (OLD.confirmed_at IS DISTINCT FROM NEW.confirmed_at)
       OR (OLD.source_proposal_id IS DISTINCT FROM NEW.source_proposal_id)
       OR (OLD.snapshot_hash IS DISTINCT FROM NEW.snapshot_hash)
       OR (OLD.predecessor_version_id IS DISTINCT FROM NEW.predecessor_version_id)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'pc.cognition_version % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'published' AND NEW.status NOT IN ('published','archived') THEN
        RAISE EXCEPTION 'pc.cognition_version % status may only move published -> archived', OLD.id;
    END IF;
    RETURN NEW;
END;
$func$;

-- 身份行 DELETE 防护（V1.1 R10/F-10）：仅 identity 态可删（初始化残留）
CREATE FUNCTION pc.forbid_identity_delete() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF OLD.lifecycle_status <> 'identity' THEN
        RAISE EXCEPTION 'pc.% % is not identity; deletion is forbidden', TG_TABLE_NAME, OLD.id;
    END IF;
    RETURN OLD;
END;
$func$;

-- 单例谓词（V1.1 R12/F-12 定稿：IMMUTABLE 硬编码，与字典 singleton 列一致性
-- 由服务层启动校验承担；部分索引谓词不可查表）
CREATE FUNCTION pc.object_type_singleton(key TEXT) RETURNS boolean
    LANGUAGE sql IMMUTABLE AS
$func$ SELECT key IN ('product_profile','product_vision') $func$;

-- 对象生命周期：终态不可复活；superseded 必填承接对象（§5.2）
CREATE FUNCTION pc.enforce_object_lifecycle() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'pc.cognition_object % is terminal (%)', OLD.id, OLD.lifecycle_status;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded object % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END;
$func$;

-- 关系身份锚定：端点/类型不可变；终态不可复活（§6.5）
CREATE FUNCTION pc.enforce_relationship_anchors() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF (OLD.relationship_type_key IS DISTINCT FROM NEW.relationship_type_key)
       OR (OLD.source_object_id IS DISTINCT FROM NEW.source_object_id)
       OR (OLD.target_object_id IS DISTINCT FROM NEW.target_object_id)
       OR (OLD.source_ext_domain IS DISTINCT FROM NEW.source_ext_domain)
       OR (OLD.source_ext_type IS DISTINCT FROM NEW.source_ext_type)
       OR (OLD.source_ext_id IS DISTINCT FROM NEW.source_ext_id)
       OR (OLD.target_ext_domain IS DISTINCT FROM NEW.target_ext_domain)
       OR (OLD.target_ext_type IS DISTINCT FROM NEW.target_ext_type)
       OR (OLD.target_ext_id IS DISTINCT FROM NEW.target_ext_id) THEN
        RAISE EXCEPTION 'pc.relationship % endpoint anchors are immutable', OLD.id;
    END IF;
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'pc.relationship % is terminal', OLD.id;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded relationship % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END;
$func$;

-- RR 端点身份一致性（V1.1 R3/F-03）：函数体声明 %ROWTYPE，须待 pc.relationship
-- 建表后创建（见 §③ trg_pc_rel_revision_endpoint_match 前置）。

-- Proposal 内容锚定（V1.1 R4/F-04）：submitted 后锚定列 once-written（§10.1）
CREATE FUNCTION pc.forbid_proposal_content_mutation() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF OLD.status <> 'draft' THEN
        IF (OLD.title IS DISTINCT FROM NEW.title)
           OR (OLD.rationale IS DISTINCT FROM NEW.rationale)
           OR (OLD.origin_kind IS DISTINCT FROM NEW.origin_kind)
           OR (OLD.governance_level IS DISTINCT FROM NEW.governance_level)
           OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
           OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
           OR (OLD.source_id IS DISTINCT FROM NEW.source_id) THEN
            RAISE EXCEPTION 'pc.proposal % is %; content anchor columns are immutable', OLD.id, OLD.status;
        END IF;
    END IF;
    RETURN NEW;
END;
$func$;

-- 提案成员锚定：draft 之外禁改成员集（§10.1）
CREATE FUNCTION pc.enforce_proposal_member_anchored() RETURNS trigger LANGUAGE plpgsql AS $func$
DECLARE p_status TEXT;
BEGIN
    SELECT status INTO p_status FROM pc.proposal
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.proposal_id, OLD.proposal_id);
    IF p_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'proposal % is %; members are anchored by proposal_content_hash',
            COALESCE(NEW.proposal_id, OLD.proposal_id), p_status;
    END IF;
    RETURN NULL;
END;
$func$;

-- 审批锚定 + 单向状态（0026:273-296 同构，§10.2）
CREATE FUNCTION pc.forbid_approval_mutation() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF (OLD.proposal_id IS DISTINCT FROM NEW.proposal_id)
       OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
       OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'pc.approval % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'pending' THEN
        IF NEW.status NOT IN ('approved','rejected') THEN
            RAISE EXCEPTION 'pc.approval % may only move from pending to a terminal status', OLD.id;
        END IF;
        IF NEW.decided_by IS NULL OR NEW.decided_at IS NULL THEN
            RAISE EXCEPTION 'pc.approval % terminal status requires decider and time', OLD.id;
        END IF;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'pc.approval % is already decided and cannot change status', OLD.id;
    END IF;
    RETURN NEW;
END;
$func$;

-- 写账本状态机：终态不可变（§12.2）
CREATE FUNCTION pc.enforce_write_op_state() RETURNS trigger LANGUAGE plpgsql AS $func$
BEGIN
    IF OLD.status IN ('completed','abandoned') AND NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'pc.write_operation % is terminal (%)', OLD.id, OLD.status;
    END IF;
    RETURN NEW;
END;
$func$;

-- 版本成员端点闭合（§8.3，V1.1/R2）：行级触发、整版本计数校验、DEFERRED
CREATE FUNCTION pc.enforce_member_endpoint_closure() RETURNS trigger LANGUAGE plpgsql AS $func$
DECLARE
    broken INT;
    v_tenant UUID; v_project UUID; v_version UUID;
BEGIN
    v_tenant  := COALESCE(NEW.tenant_id,  OLD.tenant_id);
    v_project := COALESCE(NEW.project_id, OLD.project_id);
    v_version := COALESCE(NEW.version_id, OLD.version_id);

    SELECT count(*) INTO broken
    FROM pc.version_member vm
    JOIN pc.relationship_revision rr
      ON (rr.tenant_id, rr.project_id, rr.relationship_id, rr.id)
       = (vm.tenant_id, vm.project_id, vm.relationship_id, vm.relationship_revision_id)
    WHERE (vm.tenant_id, vm.project_id, vm.version_id) = (v_tenant, v_project, v_version)
      AND ((rr.source_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM pc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.source_object_id
                  AND m2.object_revision_id = rr.source_object_revision_id))
        OR (rr.target_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM pc.version_member m2
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
$func$;

-- ════════════════════════════════════════════════════════════════════
-- ① 正文承载两表（V1.2/C-1 域内闭环；偏差 a：正文暂存 PG）
-- ════════════════════════════════════════════════════════════════════

CREATE TABLE pc.document (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    project_id     UUID NOT NULL,
    doc_key        TEXT NOT NULL,
        -- 正典形态 pc:{object_type}:{object_key}；object_key 非 slug 时
        -- pc-object-{object_id}（偏差 e，§11.2-3 兜底）。
    canonical_path TEXT NOT NULL,
        -- PCGIT 逻辑路径（正典 pc/{object_type}/{object_key}.md；偏差 a：本批
        -- 无 Git 写路径，仅登记定位）。
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, doc_key),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE TABLE pc.document_revision (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    project_id     UUID NOT NULL,
    document_id    UUID NOT NULL,
    revision_no    INT  NOT NULL CHECK (revision_no >= 1),
    git_commit_sha TEXT,
        -- 偏差 a：PCGIT 后置，本批恒 NULL；正式形态 NOT NULL（§8.2-5 必指 PCGIT）。
    content_hash   TEXT NOT NULL,
        -- sha256(markdown)，0039:19-21 口径。
    content        TEXT NOT NULL,
        -- 偏差 a：正文暂存 PG（正式权威 PCGIT，迁址随 ADR-06b 框架）。
    created_by     UUID NOT NULL REFERENCES core.principal(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, document_id, revision_no),
    UNIQUE (tenant_id, project_id, document_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES pc.document (tenant_id, project_id, id)
);

CREATE TRIGGER trg_pc_document_revision_immutable
    BEFORE UPDATE OR DELETE ON pc.document_revision
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();

-- ════════════════════════════════════════════════════════════════════
-- ② 0053 五表收敛（§5/§6/§9；原地收敛保留草稿，全新库空表自然退化）
-- ════════════════════════════════════════════════════════════════════

-- ── 2a pc.object_type：注册表收敛（列补齐 + 14 正典类型种子；偏差 b 四类型保留）
ALTER TABLE pc.object_type RENAME COLUMN code TO type_key;
ALTER TABLE pc.object_type
    ADD COLUMN layer TEXT NOT NULL DEFAULT 'snapshot'
        CONSTRAINT object_type_layer_check CHECK (layer IN
            ('L1_identity','L2_structure','L3_usage','L4_semantic','L5_decision','snapshot')),
    ADD COLUMN governance_level TEXT NOT NULL DEFAULT 'L1'
        CONSTRAINT object_type_governance_level_check CHECK (governance_level IN ('L1','L2')),
    ADD COLUMN singleton BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN doc_template_key TEXT,
    ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
        CONSTRAINT object_type_status_check CHECK (status IN ('active','deprecated')),
    ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- 14 正典类型（§3.3；L1 身份/L2 结构/L3 使用/L4 语义/L5 决策）
INSERT INTO pc.object_type (type_key, label, layer, governance_level, singleton, doc_template_key, status, description) VALUES
    ('product_profile',    '产品档案',   'L1_identity', 'L1', true,  'product-profile',    'active', '产品身份与基本信息（单例；描述修改 L1）'),
    ('product_vision',     '产品愿景',   'L1_identity', 'L2', true,  'product-vision',     'active', '产品方向与价值主张（单例；全变更 L2）'),
    ('business_domain',    '业务域',     'L2_structure', 'L1', false, NULL, 'active', '业务域划分'),
    ('business_capability','业务能力',   'L2_structure', 'L1', false, NULL, 'active', '业务能力（废弃/合并 L2）'),
    ('feature_module',     '功能模块',   'L2_structure', 'L1', false, NULL, 'active', '功能模块'),
    ('feature',            '功能',       'L2_structure', 'L1', false, NULL, 'active', '功能点（按关联与治理需要决定是否独立建模）'),
    ('actor',              '参与者',     'L3_usage',     'L1', false, NULL, 'active', '使用角色'),
    ('scenario',           '场景',       'L3_usage',     'L1', false, NULL, 'active', '使用场景'),
    ('business_process',   '业务流程',   'L3_usage',     'L2', false, NULL, 'active', '业务流程（流程及状态转换 L2）'),
    ('page',               '页面',       'L3_usage',     'L1', false, NULL, 'active', '页面'),
    ('interaction',        '交互',       'L3_usage',     'L1', false, NULL, 'active', '页面交互'),
    ('domain_object',      '领域对象',   'L4_semantic',  'L1', false, NULL, 'active', '业务语义对象'),
    ('business_rule',      '业务规则',   'L4_semantic',  'L2', false, NULL, 'active', '业务规则（全生命周期 L2）'),
    ('product_decision',   '产品决策',   'L5_decision',  'L2', false, NULL, 'active', '正式产品决策');

-- 偏差 b：checkpoint 四派生类型显式标注 snapshot 层（默认值已覆盖，显式 UPDATE 自证）
UPDATE pc.object_type SET layer = 'snapshot'
 WHERE type_key IN ('repository','tech_stack','delivery_activity','module');

-- ── 2b pc.cognition_revision：先于对象表收敛（对象表复合 FK 反向依赖修订行）──
ALTER TABLE pc.cognition_revision
    ADD COLUMN project_id UUID,
    ADD COLUMN document_id UUID,
    ADD COLUMN document_revision_id UUID,
    ADD COLUMN content_hash TEXT,
    ADD COLUMN source_context TEXT;
        -- §5.3 生成语境摘要（人工/AI/KB，追溯链起点之一）。

-- 归属/正文迁移回填（0053 行：object→document 1:1，revision→document_revision 1:1）
-- 系统主体回填锚 = ai-pm-dispatch service principal（0015:67 播种，全库存在）。
UPDATE pc.cognition_revision cr
   SET project_id    = o.project_id,
       tenant_id     = cr.tenant_id
  FROM pc.cognition_object o
 WHERE o.id = cr.object_id;

UPDATE pc.cognition_revision
   SET created_by = '00000000-0000-0000-0000-000000000001'
 WHERE created_by IS NULL;

-- 每对象一文档（doc_key 兜底形态，偏差 e）；每修订一文档修订（revision_no 顺延）
INSERT INTO pc.document (tenant_id, project_id, doc_key, canonical_path)
SELECT DISTINCT
    o.tenant_id,
    o.project_id,
    'pc-object-' || o.id::text,
    'pc/' || o.object_type || '/' || o.id::text || '.md'
FROM pc.cognition_object o;

INSERT INTO pc.document_revision
    (tenant_id, project_id, document_id, revision_no, git_commit_sha, content_hash, content, created_by)
SELECT
    o.tenant_id,
    o.project_id,
    d.id,
    cr.revision_no,
    NULL,
    encode(digest(cr.content, 'sha256'), 'hex'),
    cr.content,
    COALESCE(cr.created_by, '00000000-0000-0000-0000-000000000001')
FROM pc.cognition_revision cr
JOIN pc.cognition_object o ON o.id = cr.object_id
JOIN pc.document d ON d.tenant_id = o.tenant_id AND d.project_id = o.project_id
                  AND d.doc_key = 'pc-object-' || o.id::text;

UPDATE pc.cognition_revision cr
   SET document_id          = dr.document_id,
       document_revision_id = dr.id,
       content_hash         = dr.content_hash
  FROM pc.document_revision dr
 WHERE dr.tenant_id = cr.tenant_id
   AND dr.document_id = (SELECT d.id FROM pc.document d
                          JOIN pc.cognition_object o ON o.tenant_id = d.tenant_id
                                                       AND o.project_id = d.project_id
                           WHERE d.doc_key = 'pc-object-' || o.id::text
                             AND o.id = cr.object_id)
   AND dr.revision_no = cr.revision_no;

ALTER TABLE pc.cognition_revision
    ALTER COLUMN project_id SET NOT NULL,
    ALTER COLUMN document_id SET NOT NULL,
    ALTER COLUMN document_revision_id SET NOT NULL,
    ALTER COLUMN content_hash SET NOT NULL,
    ALTER COLUMN created_by SET NOT NULL;

-- analysis_mode 放宽可空（偏差 c′）：该列是分析产物归因（0062/D-3=A）；
-- 服务层人工/链路外草稿非分析产物 → NULL（语义真值优于默认值误标）。
ALTER TABLE pc.cognition_revision
    ALTER COLUMN analysis_mode DROP NOT NULL;

-- 约束：租户/项目键、复合引用（C-2 项目级组合 FK）、修订号唯一
-- （对象表项目级身份键先于本表复合 FK 存在）
ALTER TABLE pc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_project_id_key
        UNIQUE (tenant_id, project_id, id);
ALTER TABLE pc.cognition_revision DROP CONSTRAINT cognition_revision_object_id_fkey;
ALTER TABLE pc.cognition_revision DROP CONSTRAINT cognition_revision_object_id_revision_no_key;
ALTER TABLE pc.cognition_revision
    ADD CONSTRAINT cognition_revision_project_fkey
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    ADD CONSTRAINT cognition_revision_object_composite_fkey
        FOREIGN KEY (tenant_id, project_id, object_id)
            REFERENCES pc.cognition_object (tenant_id, project_id, id),
    ADD CONSTRAINT cognition_revision_document_fkey
        FOREIGN KEY (tenant_id, project_id, document_id)
            REFERENCES pc.document (tenant_id, project_id, id),
    ADD CONSTRAINT cognition_revision_document_revision_fkey
        FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id)
            REFERENCES pc.document_revision (tenant_id, project_id, document_id, id),
    ADD CONSTRAINT cognition_revision_tenant_project_id_key
        UNIQUE (tenant_id, project_id, id),
    ADD CONSTRAINT cognition_revision_tenant_project_object_no_key
        UNIQUE (tenant_id, project_id, object_id, revision_no),
    ADD CONSTRAINT cognition_revision_tenant_project_object_id_key
        UNIQUE (tenant_id, project_id, object_id, id);

ALTER TABLE pc.cognition_revision RENAME COLUMN summary TO change_summary;
ALTER TABLE pc.cognition_revision
    DROP CONSTRAINT cognition_revision_status_check,
    DROP COLUMN status,
    DROP COLUMN content;

CREATE INDEX idx_pc_revision_document
    ON pc.cognition_revision (tenant_id, project_id, document_revision_id);

-- append-only（01B §5.3；0053 时代该表无触发器，收敛后启用）
CREATE TRIGGER trg_pc_revision_immutable
    BEFORE UPDATE OR DELETE ON pc.cognition_revision
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();

-- ── 2c pc.cognition_object：身份收敛（01B §5.2）──
ALTER TABLE pc.cognition_object RENAME COLUMN name TO display_name;
ALTER TABLE pc.cognition_object
    ALTER COLUMN object_key DROP NOT NULL;  -- identity 占位行允许无 key（§5.2）
ALTER TABLE pc.cognition_object
    ADD COLUMN current_revision_id UUID,
    ADD COLUMN superseded_by_id UUID,
    ADD COLUMN created_by UUID,
    ADD COLUMN updated_by UUID REFERENCES core.principal(id),
    ADD COLUMN deprecated_at TIMESTAMPTZ;

UPDATE pc.cognition_object
   SET created_by = '00000000-0000-0000-0000-000000000001'
 WHERE created_by IS NULL;

-- 当前修订指针回填 = 最大 revision_no（关联子查询，量级 = 存量对象数）
UPDATE pc.cognition_object o
   SET current_revision_id = (
        SELECT r.id FROM pc.cognition_revision r
         WHERE r.object_id = o.id
         ORDER BY r.revision_no DESC LIMIT 1
       );

ALTER TABLE pc.cognition_object
    ALTER COLUMN created_by SET NOT NULL,
    ADD CONSTRAINT cognition_object_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES core.principal(id),
    ADD CONSTRAINT cognition_object_superseded_by_fkey
        FOREIGN KEY (tenant_id, project_id, superseded_by_id)
            REFERENCES pc.cognition_object (tenant_id, project_id, id);

-- 当前修订指针：自引用循环 → DEFERRABLE（回填值已合法后挂约束）
ALTER TABLE pc.cognition_object
    ADD CONSTRAINT cognition_object_current_revision_fkey
        FOREIGN KEY (tenant_id, project_id, id, current_revision_id)
            REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id)
            DEFERRABLE INITIALLY IMMEDIATE;

-- 单例约束（Profile/Vision；§5.2，谓词 IMMUTABLE 硬编码）
CREATE UNIQUE INDEX uq_pc_object_singleton
    ON pc.cognition_object (tenant_id, project_id, object_type)
    WHERE pc.object_type_singleton(object_type) = true
      AND lifecycle_status IN ('identity','draft','active');

CREATE TRIGGER trg_pc_object_lifecycle
    BEFORE UPDATE ON pc.cognition_object
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_object_lifecycle();

CREATE TRIGGER trg_pc_object_delete
    BEFORE DELETE ON pc.cognition_object
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete();

-- ── 2d pc.source：来源收敛（01B §9.2 + C-4 四元组/撤 FK）──
-- 注：旧 locator JSONB 先拆解后卸载，再以新模型 TEXT locator 重建（§9.2 列名）。
ALTER TABLE pc.source
    ADD COLUMN source_kind TEXT,
    ADD COLUMN revision_missing_reason TEXT,
    ADD COLUMN section_anchor TEXT,
    ADD COLUMN start_line INT,
    ADD COLUMN description TEXT,
    ADD COLUMN source_content_hash TEXT,
    ADD COLUMN collected_by UUID;

-- 旧行定位器拆解：content_hash → 结构化指纹列。
-- 现网既有行全部 target_domain='knowledge_base'（checkpoint 分析产物）。
UPDATE pc.source
   SET source_kind = CASE target_domain
                         WHEN 'knowledge_base' THEN 'kb_document'
                         WHEN 'requirement'    THEN 'requirement'
                         WHEN 'project_cognition' THEN 'project_cognition'
                         ELSE 'external_reference' END,
       source_content_hash = locator->>'content_hash',
       collected_by = '00000000-0000-0000-0000-000000000001';

ALTER TABLE pc.source
    ALTER COLUMN source_kind SET NOT NULL,
    ALTER COLUMN collected_by SET NOT NULL,
    ADD CONSTRAINT source_kind_check CHECK (source_kind IN
        ('kb_document','kb_memory','manual_input','requirement','requirement_revision',
         'discovery_finding','project_cognition','external_reference')),
    ADD CONSTRAINT source_collected_by_fkey
        FOREIGN KEY (collected_by) REFERENCES core.principal(id),
    ADD CONSTRAINT source_tenant_project_id_key UNIQUE (tenant_id, project_id, id),
    ADD CONSTRAINT source_revision_missing_check
        CHECK (target_revision_id IS NOT NULL OR revision_missing_reason IS NOT NULL),
    ADD CONSTRAINT source_kb_document_channel_check
        CHECK (source_kind <> 'kb_document'
               OR (target_domain = 'knowledge_base' AND target_revision_id IS NOT NULL));

ALTER TABLE pc.source DROP COLUMN locator;
ALTER TABLE pc.source ADD COLUMN locator TEXT;
        -- 新模型自由定位（路径/URL/记忆 ID 等单一值）；存量行该值为空——
        -- checkpoint 产物定位已由四元组 + source_content_hash 完整承载。

CREATE INDEX idx_pc_source_target ON pc.source (tenant_id, project_id, target_domain, target_type);
CREATE INDEX idx_pc_source_kind   ON pc.source (tenant_id, project_id, source_kind);

-- ── 2e pc.evidence：证据收敛（01B §9.3 + C-4 四元组 + Revision 级钉扎）──
ALTER TABLE pc.evidence
    ADD COLUMN object_revision_id UUID,
    ADD COLUMN relationship_revision_id UUID,
    ADD COLUMN target_domain TEXT,
    ADD COLUMN target_type TEXT,
    ADD COLUMN target_id TEXT,
    ADD COLUMN target_revision_id TEXT,
    ADD COLUMN file_path TEXT,
    ADD COLUMN symbol TEXT,
    ADD COLUMN code_range TEXT,
    ADD COLUMN commit_sha TEXT,
    ADD COLUMN section_anchor TEXT,
    ADD COLUMN start_line INT,
    ADD COLUMN end_line INT,
    ADD COLUMN excerpt TEXT,
    ADD COLUMN description TEXT,
    ADD COLUMN created_by UUID;

-- 证据主体回填：按创建时序钉到该对象当时的最新修订（分析事务内顺序写入：
-- object → revision → evidence，created_at 单调）。
UPDATE pc.evidence e
   SET object_revision_id = (
        SELECT r.id FROM pc.cognition_revision r
         WHERE r.object_id = e.cognition_object_id
           AND r.created_at <= e.created_at
         ORDER BY r.revision_no DESC LIMIT 1
       ),
       created_by = '00000000-0000-0000-0000-000000000001';

-- 类型/定位回填：repo_facts（仓库快照摘要证据）→ document_section，
-- 四元组钉 KB 文档修订（C-4），槽位 → section_anchor，旧 locator JSON 原样
-- 归档进 description（TEXT，不违 JSONB 纪律）。
UPDATE pc.evidence e
   SET evidence_type = 'document_section',
       target_domain = 'knowledge_base',
       target_type   = 'document',
       target_id     = s.target_id,
       target_revision_id = s.target_revision_id,
       section_anchor = e.locator->>'slot',
       description    = e.locator::text
  FROM pc.source s
 WHERE s.id = e.source_id
   AND e.evidence_type = 'repo_facts';

ALTER TABLE pc.evidence
    ALTER COLUMN object_revision_id SET NOT NULL,
    ALTER COLUMN created_by SET NOT NULL,
    ADD CONSTRAINT evidence_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES core.principal(id);

ALTER TABLE pc.evidence
    DROP CONSTRAINT evidence_cognition_object_id_fkey,
    DROP CONSTRAINT evidence_source_id_fkey,
    DROP COLUMN cognition_object_id,
    DROP COLUMN locator;

ALTER TABLE pc.evidence
    ADD CONSTRAINT evidence_source_composite_fkey
        FOREIGN KEY (tenant_id, project_id, source_id)
            REFERENCES pc.source (tenant_id, project_id, id),
    ADD CONSTRAINT evidence_object_revision_fkey
        FOREIGN KEY (tenant_id, project_id, object_revision_id)
            REFERENCES pc.cognition_revision (tenant_id, project_id, id),
        -- relationship_revision FK 后置（该表属 §③，建表后挂），见下
    ADD CONSTRAINT evidence_subject_xor_check
        CHECK ((object_revision_id IS NOT NULL) <> (relationship_revision_id IS NOT NULL)),
    ADD CONSTRAINT evidence_type_check CHECK (evidence_type IN
        ('code','test_result','commit','document_section',
         'meeting','user_statement','system_observation')),
    ADD CONSTRAINT evidence_type_locator_check CHECK (
        ( evidence_type IN ('code','test_result','commit')
          AND file_path IS NOT NULL AND commit_sha IS NOT NULL
          AND target_domain IS NULL )
        OR
        ( evidence_type = 'document_section'
          AND target_domain = 'knowledge_base'
          AND target_revision_id IS NOT NULL
          AND file_path IS NULL AND commit_sha IS NULL )
        OR
        ( evidence_type IN ('meeting','user_statement','system_observation')
          AND (excerpt IS NOT NULL OR description IS NOT NULL)
          AND target_domain IS NULL )
    );

-- 证据身份唯一（偏差 d：commit_sha 入式）
CREATE UNIQUE INDEX uq_pc_evidence_identity
    ON pc.evidence (tenant_id, project_id, source_id,
        coalesce(object_revision_id::text, relationship_revision_id::text),
        evidence_type, coalesce(file_path,''), coalesce(section_anchor,''),
        coalesce(code_range,''), coalesce(commit_sha,''));
CREATE INDEX idx_pc_evidence_subject
    ON pc.evidence (tenant_id, project_id, object_revision_id);
CREATE INDEX idx_pc_evidence_subject_rel
    ON pc.evidence (tenant_id, project_id, relationship_revision_id);

-- ════════════════════════════════════════════════════════════════════
-- ②′ pc.relationship_type：关系类型注册表（01B §6.2；依赖③，先建）
-- ════════════════════════════════════════════════════════════════════

CREATE TABLE pc.relationship_type (
    type_key              TEXT PRIMARY KEY,
    label                 TEXT NOT NULL,
    directionality        TEXT NOT NULL CHECK (directionality IN ('directed','undirected')),
    source_object_types   TEXT[] NOT NULL,
    target_object_types   TEXT[] NOT NULL,
    allow_self_reference  BOOLEAN NOT NULL DEFAULT false,
    allow_duplicate       BOOLEAN NOT NULL DEFAULT false,
    governance_level      TEXT NOT NULL CHECK (governance_level IN ('L1','L2')),
    status                TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','deprecated')),
    description           TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ════════════════════════════════════════════════════════════════════
-- ③ 关系两表（01B §6；Relationship → Relationship Revision 循环 FK 两步法）
-- ════════════════════════════════════════════════════════════════════

CREATE TABLE pc.relationship (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    relationship_type_key TEXT NOT NULL REFERENCES pc.relationship_type(type_key),
    lifecycle_status      TEXT NOT NULL DEFAULT 'identity'
        CHECK (lifecycle_status IN ('identity','draft','active','deprecated','superseded')),
    source_object_id      UUID,
    target_object_id      UUID,
    source_ext_domain     TEXT,
    source_ext_type       TEXT,
    source_ext_id         TEXT,
    source_ext_revision_id TEXT,
    target_ext_domain     TEXT,
    target_ext_type       TEXT,
    target_ext_id         TEXT,
    target_ext_revision_id TEXT,
    superseded_by_id      UUID,
    current_revision_id   UUID,
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, target_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, superseded_by_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    CHECK ( (source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL) ),
    CHECK ( (target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL) ),
    CHECK ( source_ext_id IS NULL OR (source_ext_domain IS NOT NULL AND source_ext_type IS NOT NULL) ),
    CHECK ( target_ext_id IS NULL OR (target_ext_domain IS NOT NULL AND target_ext_type IS NOT NULL) ),
    CHECK ( lifecycle_status <> 'superseded' OR superseded_by_id IS NOT NULL )
);

-- 活跃关系身份唯一（函数唯一索引容忍跨域 NULL；§6.3）
CREATE UNIQUE INDEX uq_pc_relationship_identity
    ON pc.relationship (tenant_id, project_id, relationship_type_key,
        coalesce(source_object_id::text,
                 source_ext_domain || ':' || source_ext_type || ':' || source_ext_id),
        coalesce(target_object_id::text,
                 target_ext_domain || ':' || target_ext_type || ':' || target_ext_id))
    WHERE lifecycle_status IN ('identity','draft','active');

CREATE INDEX idx_pc_relationship_source
    ON pc.relationship (tenant_id, project_id, source_object_id, lifecycle_status);
CREATE INDEX idx_pc_relationship_target
    ON pc.relationship (tenant_id, project_id, target_object_id, lifecycle_status);

CREATE TRIGGER trg_pc_relationship_anchors
    BEFORE UPDATE ON pc.relationship
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_relationship_anchors();

CREATE TRIGGER trg_pc_relationship_delete
    BEFORE DELETE ON pc.relationship
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete();

CREATE TABLE pc.relationship_revision (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    relationship_id       UUID NOT NULL,
    revision_no           INT  NOT NULL CHECK (revision_no >= 1),
    relationship_type_key TEXT NOT NULL REFERENCES pc.relationship_type(type_key),
    directionality        TEXT NOT NULL CHECK (directionality IN ('directed','undirected')),
    source_object_id      UUID,
    source_object_revision_id UUID,
    target_object_id      UUID,
    target_object_revision_id UUID,
    source_ext_domain     TEXT,
    source_ext_type       TEXT,
    source_ext_id         TEXT,
    source_ext_revision_id TEXT,
    target_ext_domain     TEXT,
    target_ext_type       TEXT,
    target_ext_id         TEXT,
    target_ext_revision_id TEXT,
    properties            JSONB NOT NULL DEFAULT '{}'::jsonb,
    change_summary        TEXT,
    document_id           UUID,
    document_revision_id  UUID,
    content_hash          TEXT NOT NULL,
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, relationship_id, revision_no),
    UNIQUE (tenant_id, project_id, relationship_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, source_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, source_object_id, source_object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, target_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, target_object_id, target_object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, document_id)
        REFERENCES pc.document (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id)
        REFERENCES pc.document_revision (tenant_id, project_id, document_id, id),
    CHECK ( (source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL) ),
    CHECK ( (target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL) ),
    CHECK ( source_object_id IS NULL OR source_object_revision_id IS NOT NULL ),
    CHECK ( target_object_id IS NULL OR target_object_revision_id IS NOT NULL ),
    CHECK ( source_ext_id IS NULL OR (source_ext_domain IS NOT NULL AND source_ext_type IS NOT NULL) ),
    CHECK ( target_ext_id IS NULL OR (target_ext_domain IS NOT NULL AND target_ext_type IS NOT NULL) )
);

CREATE INDEX idx_pc_rel_revision_rel
    ON pc.relationship_revision (tenant_id, project_id, relationship_id, revision_no DESC);
CREATE INDEX idx_pc_rel_revision_endpoint_src
    ON pc.relationship_revision (tenant_id, project_id, source_object_id, source_object_revision_id);
CREATE INDEX idx_pc_rel_revision_endpoint_tgt
    ON pc.relationship_revision (tenant_id, project_id, target_object_id, target_object_revision_id);

CREATE TRIGGER trg_pc_rel_revision_immutable
    BEFORE UPDATE OR DELETE ON pc.relationship_revision
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();

-- 证据对关系修订的引用（§② 收敛面后置：该表此处方存在）
ALTER TABLE pc.evidence
    ADD CONSTRAINT evidence_relationship_revision_fkey
        FOREIGN KEY (tenant_id, project_id, relationship_revision_id)
            REFERENCES pc.relationship_revision (tenant_id, project_id, id);

-- RR 端点身份一致性（V1.1 R3/F-03）：Revision 端点必须等于父 Relationship 行（§6.5）
-- （%ROWTYPE 须在建表后解析，故函数创建紧邻触发器）
CREATE FUNCTION pc.enforce_rel_revision_endpoint_match() RETURNS trigger LANGUAGE plpgsql AS $func$
DECLARE r_row pc.relationship%ROWTYPE;
BEGIN
    SELECT * INTO r_row FROM pc.relationship
     WHERE tenant_id = NEW.tenant_id AND project_id = NEW.project_id
       AND id = NEW.relationship_id;
    IF (r_row.relationship_type_key IS DISTINCT FROM NEW.relationship_type_key)
       OR (r_row.source_object_id IS DISTINCT FROM NEW.source_object_id)
       OR (r_row.target_object_id IS DISTINCT FROM NEW.target_object_id)
       OR (r_row.source_ext_domain IS DISTINCT FROM NEW.source_ext_domain)
       OR (r_row.source_ext_type IS DISTINCT FROM NEW.source_ext_type)
       OR (r_row.source_ext_id IS DISTINCT FROM NEW.source_ext_id)
       OR (r_row.target_ext_domain IS DISTINCT FROM NEW.target_ext_domain)
       OR (r_row.target_ext_type IS DISTINCT FROM NEW.target_ext_type)
       OR (r_row.target_ext_id IS DISTINCT FROM NEW.target_ext_id) THEN
        RAISE EXCEPTION 'relationship_revision % endpoint identity does not match relationship %',
            NEW.id, NEW.relationship_id;
    END IF;
    RETURN NEW;
END;
$func$;

CREATE TRIGGER trg_pc_rel_revision_endpoint_match
    BEFORE INSERT ON pc.relationship_revision
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_rel_revision_endpoint_match();

-- 循环 FK 第二步：关系当前修订指针（DEFERRABLE，§6.3）
ALTER TABLE pc.relationship
    ADD CONSTRAINT relationship_current_revision_fkey
        FOREIGN KEY (tenant_id, project_id, id, current_revision_id)
            REFERENCES pc.relationship_revision (tenant_id, project_id, relationship_id, id)
            DEFERRABLE INITIALLY IMMEDIATE;

-- ════════════════════════════════════════════════════════════════════
-- ④ 治理五表（01B §10；Proposal → Member/Approval，Conflict → 圈定范围）
-- ════════════════════════════════════════════════════════════════════

CREATE TABLE pc.proposal (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    project_id          UUID NOT NULL,
    title               TEXT NOT NULL,
    rationale           TEXT NOT NULL,
    origin_kind         TEXT NOT NULL CHECK (origin_kind IN
                          ('ai_agent','kb_memory','manual_input','requirement_change',
                           'conflict_resolution')),
    source_id           UUID,
    governance_level    TEXT NOT NULL CHECK (governance_level IN ('L1','L2')),
    risk_level          TEXT NOT NULL DEFAULT 'medium'
                          CHECK (risk_level IN ('low','medium','high','critical')),
    confidence          TEXT NOT NULL DEFAULT 'medium'
                          CHECK (confidence IN ('high','medium','low')),
    expected_version_no BIGINT NOT NULL CHECK (expected_version_no >= 0),
    proposal_content_hash TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'draft' CHECK (status IN
                          ('draft','submitted','approved','rejected','withdrawn',
                           'expired','applied','failed')),
    submitted_at        TIMESTAMPTZ,
    created_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES pc.source (tenant_id, project_id, id)
);

CREATE INDEX idx_pc_proposal_status
    ON pc.proposal (tenant_id, project_id, status, created_at DESC);

CREATE TRIGGER trg_pc_proposal_anchors
    BEFORE UPDATE ON pc.proposal
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_proposal_content_mutation();

CREATE TABLE pc.proposal_member (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                 UUID NOT NULL REFERENCES core.tenant(id),
    project_id                UUID NOT NULL,
    proposal_id               UUID NOT NULL,
    change_kind               TEXT NOT NULL CHECK (change_kind IN
                                ('create_object','update_object','create_relationship',
                                 'update_relationship','deprecate_object','deprecate_relationship')),
    object_id                 UUID,
    object_revision_id        UUID,
    relationship_id           UUID,
    relationship_revision_id  UUID,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES pc.proposal (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id)
        REFERENCES pc.relationship_revision (tenant_id, project_id, relationship_id, id),
    CHECK (
        (    object_id IS NOT NULL AND object_revision_id IS NOT NULL
         AND relationship_id IS NULL AND relationship_revision_id IS NULL )
        OR
        (    relationship_id IS NOT NULL AND relationship_revision_id IS NOT NULL
         AND object_id IS NULL AND object_revision_id IS NULL )
    ),
    CHECK ( change_kind NOT IN ('update_object','deprecate_object') OR object_id IS NOT NULL ),
    CHECK ( change_kind NOT IN ('update_relationship','deprecate_relationship')
            OR relationship_id IS NOT NULL )
);

CREATE UNIQUE INDEX uq_pc_pm_target
    ON pc.proposal_member (tenant_id, project_id, proposal_id,
        coalesce(object_id::text,''), coalesce(relationship_id::text,''))
    WHERE change_kind IN ('update_object','deprecate_object',
                          'update_relationship','deprecate_relationship');

CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored
    AFTER UPDATE OR DELETE ON pc.proposal_member
    DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW
    EXECUTE FUNCTION pc.enforce_proposal_member_anchored();

CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored_ins
    AFTER INSERT ON pc.proposal_member
    DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW
    EXECUTE FUNCTION pc.enforce_proposal_member_anchored();

CREATE TABLE pc.approval (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    proposal_id           UUID NOT NULL,
    proposal_content_hash TEXT NOT NULL,
    expected_version_no   BIGINT NOT NULL,
    status                TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','approved','rejected')),
    decided_by            UUID REFERENCES core.principal(id),
    decision_reason       TEXT,
    decided_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES pc.proposal (tenant_id, project_id, id)
);

CREATE UNIQUE INDEX uq_pc_approval_pending
    ON pc.approval (proposal_id) WHERE status = 'pending';

CREATE INDEX idx_pc_approval_proposal
    ON pc.approval (tenant_id, project_id, proposal_id, created_at DESC);

CREATE TRIGGER trg_pc_approval_immutable
    BEFORE UPDATE ON pc.approval
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_approval_mutation();

CREATE TABLE pc.conflict (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES core.tenant(id),
    project_id        UUID NOT NULL,
    conflict_type     TEXT NOT NULL CHECK (conflict_type IN
                        ('content_contradiction','rule_contradiction','scope_conflict',
                         'relationship_conflict','source_conflict','revision_conflict',
                         'reality_mismatch','cross_domain_authority')),
    severity          TEXT NOT NULL CHECK (severity IN ('critical','major','minor')),
    description       TEXT NOT NULL,
    detection_source  TEXT,
    status            TEXT NOT NULL DEFAULT 'detected' CHECK (status IN
                        ('detected','under_review','awaiting_human','resolved',
                         'dismissed','reopened')),
    resolution_note   TEXT,
    resolved_by       UUID REFERENCES core.principal(id),
    resolved_at       TIMESTAMPTZ,
    created_by        UUID NOT NULL REFERENCES core.principal(id),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    CHECK ( status NOT IN ('resolved','dismissed')
            OR (resolved_by IS NOT NULL AND resolved_at IS NOT NULL) )
);

CREATE INDEX idx_pc_conflict_status ON pc.conflict (tenant_id, project_id, status);

CREATE TABLE pc.conflict_object (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conflict_id     UUID NOT NULL,
    tenant_id       UUID NOT NULL REFERENCES core.tenant(id),
    project_id      UUID NOT NULL,
    object_id       UUID,
    relationship_id UUID,
    FOREIGN KEY (tenant_id, project_id, conflict_id) REFERENCES pc.conflict (tenant_id, project_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    CHECK ( (object_id IS NOT NULL) <> (relationship_id IS NOT NULL) )
);

-- V1.1 R1/F-01：id 主键 + 独立表达式唯一索引（内联表达式 PK 非法）
CREATE UNIQUE INDEX uq_pc_conflict_object
    ON pc.conflict_object (tenant_id, project_id, conflict_id,
        coalesce(object_id::text,''), coalesce(relationship_id::text,''));

CREATE INDEX idx_pc_conflict_object ON pc.conflict_object (tenant_id, project_id, object_id);
CREATE INDEX idx_pc_conflict_rel    ON pc.conflict_object (tenant_id, project_id, relationship_id);

-- ════════════════════════════════════════════════════════════════════
-- ⑤ 版本两表（01B §7；Version → Member，发布即原子）
-- ════════════════════════════════════════════════════════════════════

CREATE TABLE pc.cognition_version (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id              UUID NOT NULL REFERENCES core.tenant(id),
    project_id             UUID NOT NULL,
    version_no             BIGINT NOT NULL CHECK (version_no >= 1),
    status                 TEXT NOT NULL DEFAULT 'published'
                             CHECK (status IN ('published','archived')),
    confirmed_by           UUID NOT NULL REFERENCES core.principal(id),
    confirmed_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    source_proposal_id     UUID NOT NULL,
    snapshot_hash          TEXT NOT NULL,
    predecessor_version_id UUID,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, version_no),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_proposal_id)
        REFERENCES pc.proposal (tenant_id, project_id, id)
);

CREATE TRIGGER trg_pc_version_immutable
    BEFORE UPDATE ON pc.cognition_version
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_version_mutation();

CREATE TRIGGER trg_pc_version_delete
    BEFORE DELETE ON pc.cognition_version
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();

CREATE TABLE pc.version_member (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                 UUID NOT NULL REFERENCES core.tenant(id),
    project_id                UUID NOT NULL,
    version_id                UUID NOT NULL,
    object_id                 UUID,
    object_revision_id        UUID,
    relationship_id           UUID,
    relationship_revision_id  UUID,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, version_id)
        REFERENCES pc.cognition_version (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id)
        REFERENCES pc.relationship_revision (tenant_id, project_id, relationship_id, id),
    CHECK (
        (    object_id IS NOT NULL AND object_revision_id IS NOT NULL
         AND relationship_id IS NULL AND relationship_revision_id IS NULL )
        OR
        (    relationship_id IS NOT NULL AND relationship_revision_id IS NOT NULL
         AND object_id IS NULL AND object_revision_id IS NULL )
    )
);

CREATE UNIQUE INDEX uq_pc_vm_object
    ON pc.version_member (tenant_id, project_id, version_id, object_id, object_revision_id)
    WHERE object_id IS NOT NULL;
CREATE UNIQUE INDEX uq_pc_vm_relationship
    ON pc.version_member (tenant_id, project_id, version_id, relationship_id, relationship_revision_id)
    WHERE relationship_id IS NOT NULL;

CREATE INDEX idx_pc_vm_version ON pc.version_member (tenant_id, project_id, version_id);
CREATE INDEX idx_pc_vm_object  ON pc.version_member (tenant_id, project_id, object_id, object_revision_id);
CREATE INDEX idx_pc_vm_rel     ON pc.version_member (tenant_id, project_id, relationship_id, relationship_revision_id);

CREATE TRIGGER trg_pc_vm_immutable
    BEFORE UPDATE OR DELETE ON pc.version_member
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();

-- 库层最后防线：绕过发布服务的直写也在 COMMIT 前被闭合校验拦截（§8.3）
CREATE CONSTRAINT TRIGGER trg_pc_vm_closure
    AFTER INSERT ON pc.version_member
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_member_endpoint_closure();

-- ════════════════════════════════════════════════════════════════════
-- ⑥ 写幂等账本（01B §12.2；偏差 a：本批仅发布幂等用）
-- ════════════════════════════════════════════════════════════════════

CREATE TABLE pc.write_operation (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    operation_id          TEXT NOT NULL,
    command               TEXT NOT NULL CHECK (command IN
                            ('pc_commit_object_revision','pc_commit_relationship_revision',
                             'pc_publish_version')),
    status                TEXT NOT NULL DEFAULT 'pending' CHECK (status IN
                            ('pending','git_committed','completed','failed','abandoned')),
    expected_content_hash TEXT NOT NULL,
    git_commit_sha        TEXT,
    object_revision_id    UUID,
    relationship_revision_id UUID,
    version_id            UUID,
    last_error            TEXT,
    retry_count           INT NOT NULL DEFAULT 0,
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, project_id, operation_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE TRIGGER trg_pc_write_op_state
    BEFORE UPDATE ON pc.write_operation
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_write_op_state();

CREATE INDEX idx_pc_write_op_pending
    ON pc.write_operation (tenant_id, project_id, status)
    WHERE status IN ('pending','git_committed','failed');

-- ════════════════════════════════════════════════════════════════════
-- ⑦ 关系类型种子（§15.5：矩阵 17 行折叠为 12 键；R8/F-08）+ 派生视图
-- ════════════════════════════════════════════════════════════════════
-- 注：derived_from / supported_by 由 pc.source/pc.evidence 追溯结构承载，
-- 不注册为关系类型（§15.5 注记）；跨域端点以 domain.type 形态入集合。

INSERT INTO pc.relationship_type
    (type_key, label, directionality, source_object_types, target_object_types,
     allow_self_reference, allow_duplicate, governance_level, status, description) VALUES
('supports', '支撑',
 'directed',
 '{product_vision,interaction}',
 '{project_goal.project_goal,scenario}',
 false, false, 'L1', 'active',
 'Vision 支撑 Project Goal（跨域）/ Interaction 支撑 Scenario（同键双语义，§15.5 注记）'),
('covers', '覆盖',
 'directed',
 '{product_vision}',
 '{business_domain}',
 false, false, 'L1', 'active', '愿景覆盖业务域'),
('contains', '包含',
 'directed',
 '{business_domain,feature_module,page}',
 '{business_capability,feature,interaction}',
 false, false, 'L1', 'active',
 'Domain→Capability / Module→Feature / Page→Interaction（同键三语义，§15.5 注记）'),
('realized_by', '实现于',
 'directed',
 '{business_capability,scenario}',
 '{feature_module,business_process}',
 false, false, 'L1', 'active', '能力/场景由模块或流程实现'),
('specified_by', '规格于',
 'directed',
 '{feature}',
 '{requirement.requirement_revision}',
 false, false, 'L1', 'active', '功能由需求修订规格化（Q-PC-03 Revision 级引用）'),
('participates_in', '参与',
 'directed',
 '{actor}',
 '{scenario}',
 false, false, 'L1', 'active', '参与者参与场景'),
('uses', '使用',
 'directed',
 '{scenario}',
 '{feature}',
 false, false, 'L1', 'active', '场景使用功能'),
('governed_by', '受约束于',
 'directed',
 '{business_process,domain_object}',
 '{business_rule}',
 false, false, 'L2', 'active', '流程/对象受业务规则约束（固定 L2）'),
('implements', '实现',
 'directed',
 '{page}',
 '{feature}',
 false, false, 'L1', 'active', '页面实现功能'),
('constrains', '约束',
 'directed',
 '{business_rule}',
 '{requirement.requirement_revision}',
 false, false, 'L2', 'active', '业务规则约束需求修订（固定 L2）'),
('affects', '影响',
 'directed',
 '{product_decision}',
 '{product_profile,product_vision,business_domain,business_capability,feature_module,feature,actor,scenario,business_process,page,interaction,domain_object,business_rule,product_decision}',
 false, false, 'L2', 'active', '决策影响面（全部 14 类对象；固定 L2）'),
('informs', '告知',
 'directed',
 '{product_decision}',
 '{requirement.requirement_revision}',
 false, false, 'L2', 'active', '决策告知需求修订（固定 L2）');

-- Revision 版本化派生视图（§5.3 注记 1：不冗余 revision_status）
-- （实现勘误：01B §15.3 片段 LATERAL 内仅 SELECT 1，外层引用 vm.object_revision_id
--   解析失败；此处 LATERAL 显式投影该列，语义不变。）
CREATE VIEW pc.v_revision_versioned AS
SELECT r.tenant_id, r.project_id, r.object_id, r.id AS object_revision_id,
       (vm.object_revision_id IS NOT NULL) AS in_published_version
FROM pc.cognition_revision r
LEFT JOIN LATERAL (
    SELECT v.object_revision_id FROM pc.version_member v
    WHERE v.tenant_id = r.tenant_id AND v.project_id = r.project_id
      AND v.object_revision_id = r.id LIMIT 1
) vm ON true;
