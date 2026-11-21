-- 0042: P4-ORG Phase 5 Project 生命周期收敛（DR-PA-05；守卫裁决=关联 Products 的
-- 任意有效 Product Owner，2026-11-21 人工裁决）。PA 基线 §5：Project Closed 与
-- Product Abandoned 是不同生命周期概念；无存量数据：状态域直改（AGENTS.md 规则 1）。

-- 两个主要生命周期状态 = {active, closed}（DR-PA-05①）。planning/initializing
-- 为导入流水线过程态（project_import 链中间值，非持久终态），一并放行。
ALTER TABLE core.project
    ADD CONSTRAINT ck_project_status CHECK (
        (status = ANY (ARRAY['active'::text, 'closed'::text, 'planning'::text, 'initializing'::text]))
    );
ALTER TABLE core.project ALTER COLUMN status SET DEFAULT 'active';
