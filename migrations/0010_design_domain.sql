-- 0010_design_domain.sql
--
-- S03 · 分析、设计与技术决策域（总体设计 V1.0 五项裁决 Q1~Q5 落地；核验基线
-- docs/s03/S03-00-code-facts-report.md 七项核心缺口 + docs/ui/
-- smark-pm-s03-01-analysis-design-technical-decisions-v1.md §3/§4/§6）。
--
-- 新建 `design` schema，九张表 + 一列：
--   ① design.design            TechnicalDesign 稳定身份（Q1/原则 1）
--   ② design.design_revision   不可变设计版本（Q2/原则 3；库级内容不可变守卫）
--   ③ design.design_requirement   设计↔需求多对多，绑具体需求修订（Q1；形态
--                                 参照 delivery.task_requirement：应用层校验、
--                                 无 FK 约束，与既有链路一致）
--   ④ design.adr               ADR 独立实体 + 六态生命周期（Q3/原则 4）
--   ⑤ design.adr_revision      ADR 不可变决策内容版本（D-03：接受后内容冻结，
--                                 决策改变 = 新 ADR + supersede 替代链）
--   ⑥ design.adr_design_revision  ADR↔设计版本引用（§3.4 关键约束二）
--   ⑦ design.adr_requirement   ADR↔需求修订引用
--   ⑧ design.review_finding    设计评审问题登记（绑定具体 revision，可关闭）
--   ⑨ design.design_approval   专表审批（R6：设计批准/ADR 接受独立领域语义；
--                                 形态参照 governance.review_approval 专表先例，
--                                 不复用 1:1 硬绑 change_proposal 的 approval_request）
--   ⑩ design.implementation_baseline  实施基线不可变快照（Q5/§6.3；判定报告
--                                 与内容哈希同行冻结）
--   ⑪ delivery.task.implementation_baseline_id  Task → 基线追溯列（S03-00
--                                 差异矩阵 E3「Task/Run/Delivery 追溯列」；S04
--                                 创建任务时钉定不可变实施依据，SC-08）
--
-- 不可变保护（S03-00 §16.3 R4：应用层 + 数据库层共同保障）：
--   - design_revision：DELETE 禁止；内容列不可变（state/审批列除外）；
--   - adr_revision：DELETE 禁止；state='draft' 时 content 可改，提交后不可变；
--   - implementation_baseline：DELETE 禁止；仅 status/superseded_by 可变（显式替代）。
--
-- 真人门（Q4/原则 5）：批准/接受/冻结只经 REST + 真人主体（应用层 require_human
-- + 适配层 ensure_trusted_context）；MCP 不提供任何确认类工具（A* 约束②）。

CREATE SCHEMA IF NOT EXISTS design;

-- ── ① TechnicalDesign 稳定身份 ──────────────────────────────────────────────

CREATE TABLE design.design (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    code text NOT NULL,
    title text NOT NULL,
    lifecycle_status text DEFAULT 'active' NOT NULL,
    current_revision_id uuid,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT design_lifecycle_check CHECK ((lifecycle_status = ANY (ARRAY['active'::text, 'withdrawn'::text, 'archived'::text]))),
    CONSTRAINT design_code_nonempty CHECK ((length(btrim(code)) > 0)),
    CONSTRAINT design_title_nonempty CHECK ((length(btrim(title)) > 0)),
    CONSTRAINT design_unique_code UNIQUE (tenant_id, project_id, code)
);

-- ── ② DesignRevision 不可变版本（结构参照 delivery.requirement_revision）────

CREATE TABLE design.design_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    design_id uuid NOT NULL,
    version integer NOT NULL,
    content jsonb NOT NULL,
    state text NOT NULL,
    content_hash text NOT NULL,
    source text DEFAULT 'human' NOT NULL,
    analysis_run_id uuid,
    base_current_revision_id uuid,
    created_by uuid NOT NULL,
    submitted_at timestamp with time zone,
    approved_by uuid,
    approved_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT design_revision_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'pending_confirmation'::text, 'approved'::text, 'rejected'::text, 'superseded'::text]))),
    CONSTRAINT design_revision_source_check CHECK ((source = ANY (ARRAY['human'::text, 'ai'::text]))),
    CONSTRAINT design_revision_version_check CHECK ((version > 0)),
    CONSTRAINT design_revision_hash_nonempty CHECK ((length(btrim(content_hash)) > 0)),
    CONSTRAINT design_revision_unique_version UNIQUE (design_id, version)
);

CREATE INDEX idx_design_revision_design ON design.design_revision (design_id, version DESC);

-- 不可变守卫：DELETE 禁止；内容/版本/来源列仅在「draft→draft 改稿」时允许变化
-- （进入 pending_confirmation 即冻结，§4.2）；state 与审批列可流转。
CREATE FUNCTION design.guard_design_revision() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'design.design_revision is append-only (delete forbidden)';
    END IF;
    IF NEW.content IS DISTINCT FROM OLD.content
       OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
       OR NEW.version IS DISTINCT FROM OLD.version
       OR NEW.design_id IS DISTINCT FROM OLD.design_id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.project_id IS DISTINCT FROM OLD.project_id
       OR NEW.created_by IS DISTINCT FROM OLD.created_by
       OR NEW.source IS DISTINCT FROM OLD.source
       OR NEW.analysis_run_id IS DISTINCT FROM OLD.analysis_run_id
       OR NEW.base_current_revision_id IS DISTINCT FROM OLD.base_current_revision_id
    THEN
        IF NOT (OLD.state = 'draft' AND NEW.state = 'draft') THEN
            RAISE EXCEPTION 'design.design_revision content columns are immutable once submitted';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_design_revision_guard
    BEFORE UPDATE OR DELETE ON design.design_revision
    FOR EACH ROW EXECUTE FUNCTION design.guard_design_revision();

-- ── ③ 设计↔需求多对多（绑具体需求修订；无 FK，应用层校验，同 task_requirement）──

CREATE TABLE design.design_requirement (
    design_revision_id uuid NOT NULL,
    design_id uuid NOT NULL,
    requirement_id uuid NOT NULL,
    requirement_revision_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT design_requirement_unique UNIQUE (design_revision_id, requirement_id)
);

CREATE INDEX idx_design_requirement_design ON design.design_requirement (design_id);
CREATE INDEX idx_design_requirement_requirement ON design.design_requirement (requirement_id, requirement_revision_id);

-- ── ④ ADR 独立实体（六态生命周期；状态与当前内容指针在身份行）────────────────

CREATE TABLE design.adr (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    code text NOT NULL,
    title text NOT NULL,
    status text DEFAULT 'proposed' NOT NULL,
    current_revision_id uuid,
    blockers_implementation boolean DEFAULT false NOT NULL,
    superseded_by_adr_id uuid,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT adr_status_check CHECK ((status = ANY (ARRAY['proposed'::text, 'under_review'::text, 'accepted'::text, 'rejected'::text, 'superseded'::text, 'deprecated'::text]))),
    CONSTRAINT adr_code_nonempty CHECK ((length(btrim(code)) > 0)),
    CONSTRAINT adr_title_nonempty CHECK ((length(btrim(title)) > 0)),
    CONSTRAINT adr_unique_code UNIQUE (tenant_id, project_id, code)
);

-- ── ⑤ ADR 决策内容版本（D-03：不可变决策记录；提交后内容冻结）────────────────

CREATE TABLE design.adr_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    adr_id uuid NOT NULL,
    version integer NOT NULL,
    content jsonb NOT NULL,
    state text DEFAULT 'draft' NOT NULL,
    content_hash text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT adr_revision_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'submitted'::text]))),
    CONSTRAINT adr_revision_version_check CHECK ((version > 0)),
    CONSTRAINT adr_revision_hash_nonempty CHECK ((length(btrim(content_hash)) > 0)),
    CONSTRAINT adr_revision_unique_version UNIQUE (adr_id, version)
);

CREATE FUNCTION design.guard_adr_revision() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'design.adr_revision is append-only (delete forbidden)';
    END IF;
    IF NEW.state = 'submitted'
       AND (NEW.content IS DISTINCT FROM OLD.content
            OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
            OR NEW.version IS DISTINCT FROM OLD.version
            OR NEW.adr_id IS DISTINCT FROM OLD.adr_id
            OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
            OR NEW.project_id IS DISTINCT FROM OLD.project_id
            OR NEW.created_by IS DISTINCT FROM OLD.created_by)
    THEN
        RAISE EXCEPTION 'design.adr_revision content columns are immutable once submitted';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_adr_revision_guard
    BEFORE UPDATE OR DELETE ON design.adr_revision
    FOR EACH ROW EXECUTE FUNCTION design.guard_adr_revision();

-- ── ⑥ ADR↔设计版本引用 ─────────────────────────────────────────────────────

CREATE TABLE design.adr_design_revision (
    adr_id uuid NOT NULL,
    adr_revision_id uuid NOT NULL,
    design_id uuid NOT NULL,
    design_revision_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT adr_design_revision_unique UNIQUE (adr_revision_id, design_revision_id)
);

CREATE INDEX idx_adr_design_revision_design ON design.adr_design_revision (design_id, design_revision_id);

-- ── ⑦ ADR↔需求修订引用 ─────────────────────────────────────────────────────

CREATE TABLE design.adr_requirement (
    adr_id uuid NOT NULL,
    adr_revision_id uuid NOT NULL,
    requirement_id uuid NOT NULL,
    requirement_revision_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT adr_requirement_unique UNIQUE (adr_revision_id, requirement_id)
);

-- ── ⑧ 设计评审问题登记（绑定具体 revision；可关闭，多轮）────────────────────

CREATE TABLE design.review_finding (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    design_id uuid NOT NULL,
    design_revision_id uuid NOT NULL,
    finding_kind text DEFAULT 'issue' NOT NULL,
    severity text DEFAULT 'warning' NOT NULL,
    body text NOT NULL,
    status text DEFAULT 'open' NOT NULL,
    resolution_note text,
    raised_by uuid NOT NULL,
    resolved_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    resolved_at timestamp with time zone,
    CONSTRAINT review_finding_kind_check CHECK ((finding_kind = ANY (ARRAY['issue'::text, 'question'::text, 'suggestion'::text]))),
    CONSTRAINT review_finding_severity_check CHECK ((severity = ANY (ARRAY['info'::text, 'warning'::text, 'blocker'::text]))),
    CONSTRAINT review_finding_status_check CHECK ((status = ANY (ARRAY['open'::text, 'acknowledged'::text, 'resolved'::text]))),
    CONSTRAINT review_finding_body_nonempty CHECK ((length(btrim(body)) > 0))
);

CREATE INDEX idx_design_review_finding_revision ON design.review_finding (design_revision_id, status);

-- ── ⑨ 专表审批（R6：独立领域语义；形态参照 governance.review_approval）────────

CREATE TABLE design.design_approval (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    kind text NOT NULL,
    design_id uuid,
    adr_id uuid,
    baseline_id uuid,
    target_id uuid NOT NULL,
    target_hash text NOT NULL,
    status text DEFAULT 'pending' NOT NULL,
    requested_by uuid NOT NULL,
    decided_by uuid,
    decision_reason text,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT design_approval_kind_check CHECK ((kind = ANY (ARRAY['design_revision'::text, 'adr_acceptance'::text, 'baseline_freeze'::text]))),
    CONSTRAINT design_approval_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text]))),
    CONSTRAINT design_approval_target_check CHECK (
        (kind = 'design_revision' AND design_id IS NOT NULL)
        OR (kind = 'adr_acceptance' AND adr_id IS NOT NULL)
        OR (kind = 'baseline_freeze' AND baseline_id IS NOT NULL)
    ),
    CONSTRAINT design_approval_hash_nonempty CHECK ((length(btrim(target_hash)) > 0))
);

-- 每个目标至多一条 pending 审批（防止重复排队；后至请求 409）。
CREATE UNIQUE INDEX uq_design_approval_pending ON design.design_approval (target_id) WHERE status = 'pending';

-- ── ⑩ 实施基线不可变快照（Q5 §6.3：一次实施授权所依据的冻结依据）──────────────

CREATE TABLE design.implementation_baseline (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    code text NOT NULL,
    status text DEFAULT 'frozen' NOT NULL,
    content jsonb NOT NULL,
    content_hash text NOT NULL,
    readiness jsonb NOT NULL,
    frozen_by uuid NOT NULL,
    frozen_at timestamp with time zone DEFAULT now() NOT NULL,
    superseded_by_baseline_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT implementation_baseline_status_check CHECK ((status = ANY (ARRAY['frozen'::text, 'superseded'::text]))),
    CONSTRAINT implementation_baseline_code_nonempty CHECK ((length(btrim(code)) > 0)),
    CONSTRAINT implementation_baseline_hash_nonempty CHECK ((length(btrim(content_hash)) > 0)),
    CONSTRAINT implementation_baseline_unique_code UNIQUE (tenant_id, project_id, code)
);

-- 冻结即不可变：DELETE 禁止；内容/判定/冻结人不可变；仅显式替代可改两列。
CREATE FUNCTION design.guard_implementation_baseline() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'design.implementation_baseline is append-only (delete forbidden)';
    END IF;
    IF NEW.content IS DISTINCT FROM OLD.content
       OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
       OR NEW.readiness IS DISTINCT FROM OLD.readiness
       OR NEW.frozen_by IS DISTINCT FROM OLD.frozen_by
       OR NEW.frozen_at IS DISTINCT FROM OLD.frozen_at
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.project_id IS DISTINCT FROM OLD.project_id
       OR NEW.code IS DISTINCT FROM OLD.code
    THEN
        RAISE EXCEPTION 'design.implementation_baseline frozen columns are immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_implementation_baseline_guard
    BEFORE UPDATE OR DELETE ON design.implementation_baseline
    FOR EACH ROW EXECUTE FUNCTION design.guard_implementation_baseline();

-- ── ⑪ Task → 实施基线追溯列（S04 创建任务时钉定不可变实施依据）───────────────

ALTER TABLE delivery.task
    ADD COLUMN implementation_baseline_id uuid;
