-- 0015_task_schedule.sql
--
-- S04 · 任务排期与执行规划时间线（docs/ui/s04 S04-UI-06 Execution Timeline）。
--
-- 现状对照：`delivery.task`（0001 基线）无任何排期列——计划起止、工期与
-- 依赖类型均无承载，S04-UI-05/06 的甘特与校验面板只能靠前端假数据。
--
-- 核心立场：
--   ① **排期是任务自身的属性**，不是计划修订的附属——任务正式化后仍需可
--      单独调整起止与工期（跨计划修订的持续排期），故落在 delivery.task。
--   ② 计划修订（plan.task_plan_revision.content）里的人工建议起止仍保
--      留为「计划建议」；本表列是**生效排期**（materialize 后可改）。
--   ③ 依赖类型（FS/SS）与 lag 是**边**的属性，不是节点的——本次不改
--      delivery.task_dependency（保持依赖语义单一），边类型随依赖图读面
--      以「缺省 FS」呈现，避免为未落地的甘特连线语义预造列。
--   ④ 沿用仓库模式：无数据库级 FK（跨聚合引用由应用层保证）、text/int
--      与 jsonb 列、tenant/project 冗余列由既有表承担。
--
-- 无存量数据迁移（AGENTS.md §1：新项目直接改契约）。

ALTER TABLE delivery.task
    ADD COLUMN planned_start_at timestamp with time zone,
    ADD COLUMN planned_end_at timestamp with time zone,
    ADD COLUMN estimate_hours double precision;

ALTER TABLE delivery.task
    ADD CONSTRAINT task_schedule_range_check CHECK (
        planned_start_at IS NULL
        OR planned_end_at IS NULL
        OR planned_end_at >= planned_start_at
    );

ALTER TABLE delivery.task
    ADD CONSTRAINT task_estimate_hours_check CHECK (
        estimate_hours IS NULL OR estimate_hours > 0
    );

COMMENT ON COLUMN delivery.task.planned_start_at IS
    '生效排期起始时刻（人工可调；null = 未排期，S04-UI-06 归入「未排期托盘」）。';
COMMENT ON COLUMN delivery.task.planned_end_at IS
    '生效排期结束时刻；不得早于 planned_start_at。';
COMMENT ON COLUMN delivery.task.estimate_hours IS
    '计划工时（估算值，非实际工程时间；null = 未估算）。';
