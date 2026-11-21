-- PA07-IMP-009：per-device 单活跃 Run 服务端约束（部分唯一索引）。
--
-- 审计（PA07-IMP-002 §2 IMP-009，2026-11-21）：活跃约束仅有 per-Task 的
-- uq_task_run_active；凭据通道领取（lease_client_id = device_id，claim.rs
-- V-05-1 / ACP-461）无 device 级活跃检查——同一设备身份跑两个 runner 进程
-- 可并发领走两条 Run，此前仅靠 runner 客户端「V1 单槽位串行」约定兜底。
--
-- 规则：同一 device 同时至多一条 claimed/running Run。queued 未定设备
-- （lease_client_id IS NULL）不受影响；worker 领取（无 device 归属）不受
-- 影响；Run 进入终态即释放名额。将来若产品需要多槽位，必须显式改约束
-- （如列化 slot_no 参与唯一键），不得默认放开（规范 §10 IMP-009 验收）。
--
-- 与 B3 回收的兼容：lost Run 回收在同一领取事务内先置终态（释放本索引
-- 条目）再建/领新 Run，时序上不冲突；本索引反而封死「旧 Run 仍活跃时
-- 同设备再领新 Run」的重复执行面。
CREATE UNIQUE INDEX uq_task_run_device_active
    ON delivery.task_run (lease_client_id)
    WHERE status IN ('claimed', 'running') AND lease_client_id IS NOT NULL;
