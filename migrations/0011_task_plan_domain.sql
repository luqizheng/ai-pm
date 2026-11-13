-- 0011_task_plan_domain.sql
--
-- S04 · AI 任务拆解与研发组织（Development Planning & Task Orchestration）。
-- 设计依据：docs/scenarios/s04/ai-pm-s04-backend-design-proposal-v1.md（Q1/Q2/Q3/Q5/Q6
-- 裁决）+ 总体设计 docs/architecture/ai-pm-five-stage-business-architecture-v1.md S04 定义。
--
-- 核心立场：**不新建任务体系**。Task Plan 是新增的「计划」聚合层，正式任务
-- 全部复用 delivery.task（13 态状态机/依赖/仓库绑定/验收标准原样使用）。
--
-- 新建 `plan` schema：
--   ① plan.task_plan          计划聚合根（draft/in_review/approved/superseded；
--                             source_baseline_id 钉定 S03 冻结基线，Q4）
--   ② plan.task_plan_revision 计划内容不可变版本（任务草案 + 依赖键图；
--                             draft 可改稿，提交后冻结；SC-02 同款纪律）
--
-- 扩展（Q1 追溯 + 审批基座复用）：
--   ③ design.design_approval  增 plan_id 列 + kind 'task_plan'（R6 同款专表
--                             审批形态；target_hash 锚定计划内容哈希）
--   ④ delivery.task           增 plan_revision_id / plan_task_key 追溯列
--                             （Q2 重拆 diff 的映射锚；部分唯一索引保证
--                             同一计划修订下每个键至多正式化一次）

CREATE SCHEMA IF NOT EXISTS plan;

-- ── ① Task Plan 聚合根 ──────────────────────────────────────────────────────

CREATE TABLE plan.task_plan (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    code text NOT NULL,
    title text NOT NULL,
    status text DEFAULT 'draft' NOT NULL,
    current_revision_id uuid,
    source_baseline_id uuid,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_plan_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'in_review'::text, 'approved'::text, 'superseded'::text]))),
    CONSTRAINT task_plan_code_nonempty CHECK ((length(btrim(code)) > 0)),
    CONSTRAINT task_plan_title_nonempty CHECK ((length(btrim(title)) > 0)),
    CONSTRAINT task_plan_unique_code UNIQUE (tenant_id, project_id, code)
);

-- ── ② 计划内容版本（结构参照 design.design_revision）─────────────────────────

CREATE TABLE plan.task_plan_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    plan_id uuid NOT NULL,
    version integer NOT NULL,
    content jsonb NOT NULL,
    state text DEFAULT 'draft' NOT NULL,
    content_hash text NOT NULL,
    source text DEFAULT 'human' NOT NULL,
    instructions text,
    created_by uuid NOT NULL,
    submitted_at timestamp with time zone,
    approved_by uuid,
    approved_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_plan_revision_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'submitted'::text, 'approved'::text, 'rejected'::text, 'superseded'::text]))),
    CONSTRAINT task_plan_revision_source_check CHECK ((source = ANY (ARRAY['human'::text, 'ai'::text]))),
    CONSTRAINT task_plan_revision_version_check CHECK ((version > 0)),
    CONSTRAINT task_plan_revision_hash_nonempty CHECK ((length(btrim(content_hash)) > 0)),
    CONSTRAINT task_plan_revision_unique_version UNIQUE (plan_id, version)
);

CREATE INDEX idx_task_plan_revision_plan ON plan.task_plan_revision (plan_id, version DESC);

-- 不可变守卫：DELETE 禁止；内容列仅在 draft→draft 改稿时允许变化。
CREATE FUNCTION plan.guard_task_plan_revision() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'plan.task_plan_revision is append-only (delete forbidden)';
    END IF;
    IF NEW.content IS DISTINCT FROM OLD.content
       OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
       OR NEW.version IS DISTINCT FROM OLD.version
       OR NEW.plan_id IS DISTINCT FROM OLD.plan_id
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.project_id IS DISTINCT FROM OLD.project_id
       OR NEW.created_by IS DISTINCT FROM OLD.created_by
       OR NEW.source IS DISTINCT FROM OLD.source
       OR NEW.instructions IS DISTINCT FROM OLD.instructions
    THEN
        IF NOT (OLD.state = 'draft' AND NEW.state = 'draft') THEN
            RAISE EXCEPTION 'plan.task_plan_revision content columns are immutable once submitted';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_task_plan_revision_guard
    BEFORE UPDATE OR DELETE ON plan.task_plan_revision
    FOR EACH ROW EXECUTE FUNCTION plan.guard_task_plan_revision();

-- ── ③ 审批基座扩展：任务计划批准（R6 复用专表形态）────────────────────────────

ALTER TABLE design.design_approval ADD COLUMN plan_id uuid;

ALTER TABLE design.design_approval DROP CONSTRAINT design_approval_kind_check;
ALTER TABLE design.design_approval ADD CONSTRAINT design_approval_kind_check CHECK ((kind = ANY (ARRAY['design_revision'::text, 'adr_acceptance'::text, 'baseline_freeze'::text, 'task_plan'::text])));

ALTER TABLE design.design_approval DROP CONSTRAINT design_approval_target_check;
ALTER TABLE design.design_approval ADD CONSTRAINT design_approval_target_check CHECK (
    (kind = 'design_revision' AND design_id IS NOT NULL)
    OR (kind = 'adr_acceptance' AND adr_id IS NOT NULL)
    OR (kind = 'baseline_freeze' AND baseline_id IS NOT NULL)
    OR (kind = 'task_plan' AND plan_id IS NOT NULL)
);

-- ── ④ Task → 计划修订追溯列（Q1/Q2：重拆 diff 的映射锚）──────────────────────

ALTER TABLE delivery.task ADD COLUMN plan_revision_id uuid;
ALTER TABLE delivery.task ADD COLUMN plan_task_key text;

-- 同一计划修订下每个任务键至多正式化一次（幂等正式化的库级保证）。
CREATE UNIQUE INDEX uq_task_plan_revision_key
    ON delivery.task (plan_revision_id, plan_task_key)
    WHERE plan_revision_id IS NOT NULL;
