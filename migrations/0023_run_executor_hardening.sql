-- 0023_run_executor_hardening: T-032 执行器加固四列（ADR-0002 + T1–T4 终裁）。
-- 依据：docs/ai-pm-slice-d3-executor-hardening-v1.md §3.3 / §3.4 / §4；
--       docs/ai-pm-adr-0002-executor-credential-separation.md §7。
-- 硬约束（终裁）：0011/0015 已应用不得回改——本迁移为新增 forward-only 文件；
-- 0022 号段已由 T-041（delivery_manifest）保留，T-083 discovery 顺移 0024。

ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS lease_generation       INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS deadline_at            TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_agent_heartbeat_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_progress_at       TIMESTAMPTZ;

COMMENT ON COLUMN delivery.task_run.lease_generation IS
    '执行权世代（T2 终裁）：新行 0，每次成功获取执行权（领取/重领）+1——首次有效领取即 1，重领再 +1；心跳续租与领取失败不递增；MCP 必须同时校验 token 与 generation，旧 generation 永不重新授权。执行权世代不等于普通凭据校验。';
COMMENT ON COLUMN delivery.task_run.deadline_at IS
    'Run 级绝对执行截止（T3 终裁）：首次成功 claim 时按 task.timeout_seconds 与环境默认上限固化；重领与心跳续租不得重置或延长；绝对截止优先于租约续期。';
COMMENT ON COLUMN delivery.task_run.last_agent_heartbeat_at IS
    '执行端 Agent 心跳（T4 终裁）：仅经 MCP 通道（renew/progress）刷新；与进程存活、执行进展、绝对截止四类信号分立；心跳不能自动视为执行进展。';
COMMENT ON COLUMN delivery.task_run.last_progress_at IS
    '执行进展时刻（T4 终裁）：执行阶段（stage）变化时刷新；与心跳独立阈值独立判定。';

-- 绝对截止扫描索引：poll / 兜底回收按 deadline 精确圈定活跃 Run。
CREATE INDEX IF NOT EXISTS idx_task_run_deadline
    ON delivery.task_run(deadline_at)
    WHERE status IN ('claimed', 'running') AND deadline_at IS NOT NULL;

-- Agent 心跳失效扫描索引：死锁进程检测（进程存活但心跳停滞）。
CREATE INDEX IF NOT EXISTS idx_task_run_agent_heartbeat
    ON delivery.task_run(last_agent_heartbeat_at)
    WHERE status IN ('claimed', 'running') AND last_agent_heartbeat_at IS NOT NULL;