-- 0046_acp_communication_face: ACP 通信面（设计稿
-- docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp01-02-08-v1.md V1.1，
-- 基线 = S03-ACP V1.3.1（已冻结，2026-11-07 用户决议）。
-- 取号：R-ACP-6 已裁一批两表；0045 已被 s02_i3 占用（DEF-01 顺移），
-- 按台账纪律后实施者顺移 → 本迁移取 0046。
--
-- 三部分：
--   1) delivery.task_interaction —— ACP-01 Interaction 领域模型
--      （介入链 chain_open 持久化列承载唯一活跃约束，不由状态推导）
--   2) delivery.worker_command —— Platform→执行端命令查询面
--      （基线 §10.3 四态不动 + 本稿 V1.1 领取租约 claimed_* 列）
--   3) delivery.task_run.scheduling_shield —— N-1 调度保护状态
--      （调度器保护标志，非 Run 7 态状态机扩展；六情形决策表见设计稿 §4.1）

-- ═══════════════════════════════════════════════════════════════════
-- 1) delivery.task_interaction
--    状态机：pending → responded → consumed；pending → timed_out /
--    cancelled（V1.3.1 §7.3）。RESPONDED 未消费为合法稳态（评审 N-6）。
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE delivery.task_interaction (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    task_id       UUID NOT NULL REFERENCES delivery.task(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    -- 发起时的平台侧 executor_session（opaque 标识，非 Agent 内部 Session，0015/0020 口径）
    session_id    TEXT,
    -- 发起时的 lease_generation 快照（命令/续作校验用只读比对）
    continuation_generation INT NOT NULL DEFAULT 0,
    -- Agent 侧幂等键（标记文件 schema_version aipm.interaction.v1 的 event_id）；
    -- 重复上报按本键幂等返回原 id（设计稿 §1.2）
    event_id      TEXT NOT NULL,
    reason        TEXT NOT NULL
                  CHECK (reason IN ('needs_info', 'needs_decision', 'needs_approval', 'external_dependency')),
    title         TEXT NOT NULL,
    question      TEXT NOT NULL,
    options       JSONB,
    required_response BOOLEAN NOT NULL DEFAULT TRUE,
    -- 请求自带的 checkpoint 引用（文件在 Workspace，Platform 只登记，V1.3.1 §6）
    checkpoint_path  TEXT,
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'responded', 'consumed', 'timed_out', 'cancelled')),
    -- 介入链开放标志（V1.1 §5.2）：唯一活跃约束 = 同 Task 至多一条 chain_open 行；
    -- 服务层事务维护：创建/重试时新行 true 且旧 retry 行让出 false；
    -- consumed/cancelled → false；timed_out 保持 true（链未处置不关闭）
    chain_open    BOOLEAN NOT NULL DEFAULT TRUE,
    -- 人工等待截止（AI_PM_HUMAN_INTERVENTION_TIMEOUT_SECS，默认 120s；
    -- 持久化后 Platform 重启不重算、不顺延 —— V1.3.1 §8.1 / ACP-419）
    deadline_at   TIMESTAMPTZ NOT NULL,
    responded_at  TIMESTAMPTZ,
    consumed_at   TIMESTAMPTZ,
    timed_out_at  TIMESTAMPTZ,
    cancelled_at  TIMESTAMPTZ,
    -- 超时重试挂链字段（V1.3.1 §8.2 / R-10；重试时旧行 chain_open 让出）
    retry_of      UUID REFERENCES delivery.task_interaction(id),
    -- terminate 有界重试耗尽时落值（V1.1 §6.5 / R-ACP-7 已裁）：
    -- 仅冻结新 Session 启动（情形 6），不冻结人类回复提交入口
    continuation_start_frozen_at TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    UNIQUE (run_id, event_id)
);

-- 介入链唯一活跃约束（V1.1 §5.1/§5.2）
CREATE UNIQUE INDEX uq_task_interaction_open_chain
    ON delivery.task_interaction (task_id)
    WHERE chain_open;

CREATE INDEX task_interaction_run_status_idx
    ON delivery.task_interaction (run_id, status);
CREATE INDEX task_interaction_task_status_idx
    ON delivery.task_interaction (task_id, status);

-- ═══════════════════════════════════════════════════════════════════
-- 2) delivery.worker_command —— 命令查询面（持久化 + 轮询，基线 §10）
--    四态 issued → delivered → acked | failed（基线 §10.3，本迁移不改）；
--    V1.1 P0-2 补领取租约：delivered = 「已领取且处于有界 claim 租约内」，
--    claim 过期后同 worker 可用同一 command id 重投；claim 过期 ≠ failed。
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE delivery.worker_command (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    command_type  TEXT NOT NULL
                  CHECK (command_type IN ('agent.process.terminate', 'agent.task.continue')),
    -- 路由目标 = 下发时 run.lease_owner 快照（R-ACP-3：不做广播）；
    -- 命令面身份校验三要素之一（V1.1 §3.3：凭据 + worker_id 绑定 + generation 比对）
    worker_id     TEXT NOT NULL,
    task_id       UUID NOT NULL REFERENCES delivery.task(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    interaction_id UUID REFERENCES delivery.task_interaction(id),
    -- ACP-05 落 task_run_continuation 后补 FK（V1.3.1 §10.2 字段预留；continue 命令必填）
    continuation_id UUID,
    -- 下发时自 run 读取固化的执行代次（防过期执行端跨代次误领，只读比对不出示 token）
    lease_generation INT NOT NULL DEFAULT 0,
    payload       JSONB NOT NULL,
    status        TEXT NOT NULL DEFAULT 'issued'
                  CHECK (status IN ('issued', 'delivered', 'acked', 'failed')),
    -- 领取租约元数据（V1.1 §3.2）：同一 command id 可多次被同一 worker 重领
    claim_count   INT NOT NULL DEFAULT 0,
    claimed_by    TEXT,
    claimed_at    TIMESTAMPTZ,
    claim_expires_at TIMESTAMPTZ,
    issued_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at  TIMESTAMPTZ,
    acked_at      TIMESTAMPTZ,
    failed_at     TIMESTAMPTZ,
    last_error    TEXT,
    UNIQUE (project_id, id),
    -- delivered 必须携带领取租约（含提前重领改写 expires_at 的路径）
    CHECK (status <> 'delivered' OR (claimed_by IS NOT NULL AND claim_expires_at IS NOT NULL))
);

-- 幂等：同 (run, type, interaction) 至多一条非 failed 活跃命令（V1.1 §3.1）。
-- V1 两类命令恒带 interaction_id；interaction_id 为 NULL 的行不参与去重。
CREATE UNIQUE INDEX uq_worker_command_active
    ON delivery.worker_command (run_id, command_type, interaction_id)
    WHERE status <> 'failed';

-- 轮询主路径：按 worker 取 issued / claim 过期的 delivered
CREATE INDEX worker_command_poll_idx
    ON delivery.worker_command (worker_id, status, claim_expires_at);
CREATE INDEX worker_command_run_idx
    ON delivery.worker_command (run_id, status);

-- ═══════════════════════════════════════════════════════════════════
-- 3) delivery.task_run.scheduling_shield —— N-1 调度保护（V1.1 §4.1）
--    转换（InteractionService 各事务内显式维护，不由 Interaction.status 推导）：
--      none →(创建 Interaction)→ human_wait →(回复持久化)→ start_wait
--            →(continue acked，判据=session_created+input_injected)→ none
--      human_wait →(§8.2 超时判定)→ timed_out →(retry_of 新建)→ human_wait
--      timed_out（未人工处置）保持；CANCELLED（Task 活）→ none；
--      Task 取消/归档 → none；旧进程退出未确认（情形 6）维持原值，
--      新 Session 启动由命令执行前置检查（§9.4 检查 8 操作化）禁止。
--    本列是调度器保护标志，不是 Run 7 态状态机扩展（禁重建区红线不动）。
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE delivery.task_run
    ADD COLUMN scheduling_shield TEXT NOT NULL DEFAULT 'none'
        CHECK (scheduling_shield IN ('none', 'human_wait', 'start_wait', 'timed_out'));

CREATE INDEX idx_task_run_shielded
    ON delivery.task_run (scheduling_shield)
    WHERE scheduling_shield <> 'none';

COMMENT ON COLUMN delivery.task_run.scheduling_shield IS
    'N-1 人工等待调度保护：none/human_wait/start_wait/timed_out；'
    'poll_active_runs 四信号与 B3-2 重领对非 none 一律跳过（ACP-427/428）；'
    '由 InteractionService 事务维护，非 Run 状态机（设计稿 V1.1 §4）';
