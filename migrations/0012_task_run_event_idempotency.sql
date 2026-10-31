-- 0012_task_run_event_idempotency: 事件级幂等——为 Run 绑定创建它的 outbox 事件，
-- 使「一个事件至多创建一条 Run」由数据库唯一索引兜底。
-- 依据：docs/ai-pm-taskready-idempotency-v1.md §五 L1 / §六。
-- 迁移号取 0012（0011 属 D1 调度；0013 属 R5 多级审批）。

-- 1) 事件来源列（可空：租约恢复路径新建的 Run 无对应事件，历史 Run 亦为 NULL）。
ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS source_event_id UUID;

-- 2) 存量核查（S6 风格：显式失败并输出可定位的冲突事件，不静默删除）。
--    首次执行时该列全为 NULL（核查空转）；重复执行迁移时若有同事件重复 Run 即中止。
DO $$
DECLARE r RECORD;
BEGIN
    FOR r IN
        SELECT source_event_id, count(*) AS n
          FROM delivery.task_run
         WHERE source_event_id IS NOT NULL
         GROUP BY source_event_id
        HAVING count(*) > 1
    LOOP
        RAISE EXCEPTION
            '0012: outbox event % already created % runs; resolve before indexing',
            r.source_event_id, r.n;
    END LOOP;
END $$;

-- 3) 事件级唯一：一个 outbox 事件至多创建一条 Run（部分唯一索引，仅约束非空值）。
CREATE UNIQUE INDEX IF NOT EXISTS uq_task_run_source_event
    ON delivery.task_run(source_event_id)
    WHERE source_event_id IS NOT NULL;