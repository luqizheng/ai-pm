-- 0063_runner_availability: D-2=c 领取型 Runner 可用性真源（设备级心跳）。
-- 依据：docs/project-ai-team-v2/ai-pm-v05-d2c-runner-availability-design-v1.md §八裁决；
--       docs/project-ai-team-v2/ai-pm-v05-d2c-implementation-baseline-v1.md §二。
-- 语义定版（基线 §三）：is_runner 是**静态事实**（worker 对领取型项目恒不领）；
-- last_heartbeat_at 是**可用性真源**（本期只落库与定义语义，不门控 worker 领取——
-- 「Runner 离线 → worker 抢活」的兜底接管是 V2 调度入口闭合的裁决范围，本批不越界）。

ALTER TABLE devenv.device
    ADD COLUMN is_runner         BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN last_heartbeat_at TIMESTAMPTZ;

COMMENT ON COLUMN devenv.device.is_runner IS
    '领取型 Runner 载体（D-2=c Q-D2C-1）：其绑定项目的 delivery run 由 Runner 显式领取
     （Model A）；worker 无人值守派发（Model B）在候选发现阶段静默排除。';
COMMENT ON COLUMN devenv.device.last_heartbeat_at IS
    '设备级心跳（可用性真源，Q-D2C-2）：已认证设备活动（领取/列表/上报/续领/发现/
     命令轮询）服务端归因刷新；新鲜度语义由 AI_PM_RUNNER_AVAILABILITY_STALE_SECS 定义。';

-- 可用性扫描索引：只覆盖 runner 载体（部分索引），供 V2 调度入口/可观测面按
-- 新鲜度圈定在线 Runner。
CREATE INDEX idx_device_runner_heartbeat
    ON devenv.device(last_heartbeat_at)
    WHERE is_runner;
