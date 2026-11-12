-- 0020：Visualization Engine 基础表（viz schema · ADR-VIZ-006 / 13 号数据模型设计）
--
-- 用途：AI-PM Visualization Engine 的产物存储——图形产物（Artifact）、
-- 发布审批（Approval）、渲染产物（Render Output）、槽位配置（Slot Config）。
--
-- 设计口径（13 号 docs/visualization/ai-pm-viz-13-v0-2-data-model-design-v1.md，
-- 并入 ADR V1.0.1 附录 A 的 M-1~M-8 修订）：
--   1. **共享基础设施，非独立业务域**（ADR-VIZ-001）：本 schema 只存展示层簿记
--      （版本 / 审批 / 渲染产物），不持有任何业务事实——节点与边的语义权威在
--      各业务领域。可视化**零业务写路径**（AC-16）。
--   2. **Artifact 与 Approval 状态机分离**（R-04）：artifact.state
--      （draft/published/stale/rejected）与 approval.status
--      （pending/approved/rejected/cancelled）分表分列；artifact 无 decided_by /
--      decision_reason 列——审批语义归 approval 表。
--   3. **发布版不可变**（ADR-VIZ-006）：published 的 spec 不可改写；published
--      只能向 stale 迁移（触发器守卫）。重新生成 = 新版本，不覆盖历史发布版。
--   4. **解释图不可发布**（DV-13 / R-06）：nature='explanatory' 的产物不得进入
--      published 状态（DB CHECK 强制，非仅 UI 约束）——「AI 生成 · 非正式认知」
--      不能借发布流程变成正式认知。
--   5. **每槽位至多一条 pending 审批**（并发由数据库唯一约束保障，不靠应用层
--      先查后写——ADR-VIZ-006 / R-04）。
--   6. **乐观并发双锚**：approval.target_hash（被审 artifact 的 spec_hash 快照）
--      + expected_version_no（CAS 期望版本号）——裁决时哈希漂移即 409
--      （对齐 design_approval / pc.approval 先例）。
--   7. **Spec 快照原文必留**（Git 化推迟的条件三）：spec jsonb 存全文，
--      防「只有渲染产物、丢失图形描述」。
--   8. **跨聚合无 FK，同聚合有 FK**（仓库惯例）：artifact↔approval /
--      render_output 同 schema 建 FK；对业务表的引用一律应用层校验。
--   9. **容量治理**（R-05）：render_output 按 format 分列（text/bytea），
--      失败产物不落本表（只进审计与 job 记录）；保留期与版本上限由
--      slot_config 承载，清理 sweep 在 worker 挂载。
--
-- 迁移号：0020。
--   取号依据（2026-11-12 实测复核，替代此前 0061 的假设式论证）：
--   ① 台账 §〇 明文：旧链 0001~0065 已合并为 0001_baseline.sql，
--      **后续新迁移自 0002 起编号，不再受旧链历史与空号约束**。
--   ② 实测当前有效迁移集 migrations/ 最大号 = 0019 → 下一个 = 0020。
--   ③ 实测真实环境（infra postgres 库 _sqlx_migrations）= max 18 / 18 条，
--      **无任何旧链残留**（0044~0054 等）→ 不存在"复用历史号触发 VersionMissing"的风险，
--      此前"必须 >0060"的假设不成立，已纠正。
--   ④ sqlx 不要求版本号连续，只要求库内已应用版本 ⊆ 文件集合。

CREATE SCHEMA IF NOT EXISTS viz;

-- ── ① 图形产物 ──────────────────────────────────────────────────────────
CREATE TABLE viz.artifact (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    slot_key text NOT NULL,
    version_no integer NOT NULL,
    state text DEFAULT 'draft' NOT NULL,
    nature text DEFAULT 'business' NOT NULL,
    spec jsonb NOT NULL,
    spec_hash text NOT NULL,
    source_hash text NOT NULL,
    provenance jsonb NOT NULL,
    renderer_id text,
    renderer_version text,
    created_by uuid NOT NULL,
    published_by uuid,
    published_at timestamp with time zone,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_artifact_pk PRIMARY KEY (id),
    CONSTRAINT viz_artifact_slot_key_check CHECK (length(btrim(slot_key)) > 0),
    CONSTRAINT viz_artifact_version_check CHECK (version_no >= 1),
    CONSTRAINT viz_artifact_state_check CHECK (state = ANY (ARRAY['draft'::text,'published'::text,'stale'::text,'rejected'::text])),
    CONSTRAINT viz_artifact_nature_check CHECK (nature = ANY (ARRAY['business'::text,'explanatory'::text])),
    CONSTRAINT viz_artifact_nature_publish_check CHECK (state <> 'published' OR nature = 'business'),
    CONSTRAINT viz_artifact_spec_hash_check CHECK (length(btrim(spec_hash)) > 0),
    CONSTRAINT viz_artifact_source_hash_check CHECK (length(btrim(source_hash)) > 0),
    CONSTRAINT viz_artifact_provenance_check CHECK (jsonb_typeof(provenance) = 'object')
);

CREATE UNIQUE INDEX uq_viz_artifact_slot_version ON viz.artifact (project_id, slot_key, version_no);
CREATE INDEX idx_viz_artifact_slot_state ON viz.artifact (tenant_id, project_id, slot_key, state, created_at DESC);
CREATE INDEX idx_viz_artifact_created ON viz.artifact (tenant_id, project_id, created_at DESC);

-- 发布版不可变守卫：published 的 spec 不可改写；published 只能向 stale 迁移。
CREATE FUNCTION viz.guard_artifact_immutability() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state = 'published' AND NEW.spec IS DISTINCT FROM OLD.spec THEN
        RAISE EXCEPTION 'published artifact spec is immutable';
    END IF;
    IF OLD.state = 'published' AND NEW.state NOT IN ('published','stale') THEN
        RAISE EXCEPTION 'published artifact cannot leave published/stale';
    END IF;
    -- stale 是终态（13 号 §4.1）：过期版本永久保留可查；重新生成 = 新版本行，
    -- 不得把 stale 行改回 draft/published（否则历史快照会被篡改）。
    IF OLD.state = 'stale' AND NEW.state <> 'stale' THEN
        RAISE EXCEPTION 'stale artifact is terminal';
    END IF;
    NEW.row_revision := OLD.row_revision + 1;
    RETURN NEW;
END $$;

CREATE TRIGGER trg_viz_artifact_guard BEFORE UPDATE ON viz.artifact
    FOR EACH ROW EXECUTE FUNCTION viz.guard_artifact_immutability();

-- ── ② 发布审批（与 Artifact 状态机分离，R-04）──────────────────────────
CREATE TABLE viz.approval (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    artifact_id uuid NOT NULL,
    slot_key text NOT NULL,
    target_hash text NOT NULL,
    expected_version_no integer NOT NULL,
    status text DEFAULT 'pending' NOT NULL,
    requested_by uuid NOT NULL,
    decided_by uuid,
    decision_reason text,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_approval_pk PRIMARY KEY (id),
    CONSTRAINT viz_approval_status_check CHECK (status = ANY (ARRAY['pending'::text,'approved'::text,'rejected'::text,'cancelled'::text])),
    CONSTRAINT viz_approval_target_hash_check CHECK (length(btrim(target_hash)) > 0),
    CONSTRAINT viz_approval_reject_reason_check CHECK (status <> 'rejected' OR (decision_reason IS NOT NULL AND length(btrim(decision_reason)) > 0)),
    CONSTRAINT viz_approval_decided_check CHECK (status = 'pending' OR decided_by IS NOT NULL),
    CONSTRAINT viz_approval_fk_artifact FOREIGN KEY (artifact_id) REFERENCES viz.artifact(id)
);

-- 每槽位至多一条 pending（数据库级并发保障，R-04）
CREATE UNIQUE INDEX uq_viz_approval_pending ON viz.approval (project_id, slot_key) WHERE status = 'pending';

-- 审批裁决守卫：锚点列不可变；pending 可转三终态；终态不可再改（对齐 pc.approval 先例）。
CREATE FUNCTION viz.guard_approval_decision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.target_hash <> OLD.target_hash
       OR NEW.expected_version_no <> OLD.expected_version_no
       OR NEW.created_at <> OLD.created_at THEN
        RAISE EXCEPTION 'approval anchor columns are immutable';
    END IF;
    IF OLD.status <> 'pending' THEN
        RAISE EXCEPTION 'approval already decided';
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER trg_viz_approval_guard BEFORE UPDATE ON viz.approval
    FOR EACH ROW EXECUTE FUNCTION viz.guard_approval_decision();

-- ── ③ 渲染产物（失败产物不落本表——只进审计与 job 记录，R-05）────────────
CREATE TABLE viz.render_output (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    artifact_id uuid NOT NULL,
    format text NOT NULL,
    content text,
    content_bytes bytea,
    content_hash text NOT NULL,
    byte_size integer NOT NULL,
    renderer_id text NOT NULL,
    renderer_version text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_render_output_pk PRIMARY KEY (id),
    CONSTRAINT viz_render_output_format_check CHECK (format = ANY (ARRAY['html'::text,'svg'::text,'png'::text])),
    CONSTRAINT viz_render_output_content_check CHECK (
        (format IN ('html','svg') AND content IS NOT NULL AND content_bytes IS NULL)
        OR (format = 'png' AND content IS NULL AND content_bytes IS NOT NULL)
    ),
    CONSTRAINT viz_render_output_hash_check CHECK (length(btrim(content_hash)) > 0),
    CONSTRAINT viz_render_output_fk_artifact FOREIGN KEY (artifact_id) REFERENCES viz.artifact(id)
);

CREATE UNIQUE INDEX uq_viz_render_output_artifact_format ON viz.render_output (artifact_id, format);

-- ── ④ 槽位配置（无行时取默认策略：require_approval=true / business / 20 版 / 180 天）──
CREATE TABLE viz.slot_config (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    slot_key text NOT NULL,
    view_kind text NOT NULL,
    require_approval boolean DEFAULT true NOT NULL,
    auto_stale boolean DEFAULT true NOT NULL,
    allowed_nature text DEFAULT 'business' NOT NULL,
    max_versions integer DEFAULT 20 NOT NULL,
    retention_days integer DEFAULT 180 NOT NULL,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_slot_config_pk PRIMARY KEY (id),
    CONSTRAINT viz_slot_config_slot_key_check CHECK (length(btrim(slot_key)) > 0),
    CONSTRAINT viz_slot_config_nature_check CHECK (allowed_nature = ANY (ARRAY['business'::text,'explanatory'::text,'both'::text])),
    CONSTRAINT viz_slot_config_max_versions_check CHECK (max_versions >= 1),
    CONSTRAINT viz_slot_config_retention_check CHECK (retention_days >= 0)
);

CREATE UNIQUE INDEX uq_viz_slot_config_slot ON viz.slot_config (project_id, slot_key);
