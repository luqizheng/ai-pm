-- ============================================================
-- T-059 尾项（ADR-0002 L5/L6）：Run 执行形态列 + 触发人可空性核查
--
-- 上位：docs/ai-pm-adr-0002-executor-credential-separation.md §3 L5/L6
--   L5: execution_mode 显式区分执行形态，替代从 lease_owner 前缀推导；
--       V1 两值（human / spawn），remote / container 预留（不进 CHECK）。
--   L6: 无人值守允许触发人为空——0020 的 triggered_by_user_id 本就可空
--       （无 NOT NULL 约束，全仓生产代码无写入方），核查结论：无需迁移，
--       语义成文于本注释。审计口径「触发人 ≠ 监督人」由 ADR-0002 D4 承载。
--
-- 写入方（T-059 后续接线，不在本迁移内）：
--   claim_core 按 LeaseOwner 派生——Worker → 'spawn'，Mcp → 'human'；
--   review run（queue/worker）→ 'spawn'（平台 spawn 的审查进程）。
-- ============================================================
ALTER TABLE delivery.task_run
    ADD COLUMN execution_mode TEXT
        CHECK (execution_mode IN ('human', 'spawn'));

-- 存量行回填：不区分历史领取通道（V1 前全部为真人 MCP 领取或 worker 试领），
-- 统一记 NULL = 「迁移前未知」，不猜。
CREATE INDEX idx_task_run_execution_mode
    ON delivery.task_run (execution_mode)
    WHERE execution_mode IS NOT NULL;
