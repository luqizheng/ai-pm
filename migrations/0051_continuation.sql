-- 0051_continuation: ACP-05 Continuation 正式身份与状态机
-- （联合数据契约 docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp04-05-joint-v1.md
--   V1.0 §3.1；联合架构裁决 2026-11-07：0051 暂定确认；**Recovery Foundation
--   批已先落地**（裁决：ack 推进与 R-4 恢复同事务，恢复设施未建则语义倒挂）。
--   ACP-05 边界：先定义正式 Continuation 身份/状态/代次/生命周期，
--   Checkpoint/Lock/Deadline（06/07）经明确接口接入，不顺带吸收其职责。）
--
-- 取号：台账暂定预留 0051（0050 已落；落地前已重核）。
-- 身份冻结（联合裁决）：Continuation 身份 = 本表 id；Platform Session ID =
-- task_run.executor_session；新 Session 标识 = worker_command.receipt.session_id。
-- 不允许出现第四种 Session 身份。

CREATE TABLE delivery.continuation (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL REFERENCES core.project(id),
    task_id               UUID NOT NULL REFERENCES delivery.task(id),
    run_id                UUID NOT NULL REFERENCES delivery.task_run(id),
    origin_interaction_id UUID NOT NULL REFERENCES delivery.task_interaction(id),
    -- 代次 = 同 Run 内第 N 次继续（基线 §3.3；与 lease_generation / attempt_no
    -- 完全分离——三者互不相干，UNIQUE(run_id, generation) 自然隔离 B3-2 attempt）
    generation            INT  NOT NULL,
    status                TEXT NOT NULL DEFAULT 'open'
                          CHECK (status IN ('open', 'started', 'consumed', 'closed', 'frozen')),
    frozen_at             TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at            TIMESTAMPTZ,
    closed_at             TIMESTAMPTZ,
    UNIQUE (project_id, id),
    UNIQUE (run_id, generation)
);

CREATE INDEX idx_continuation_run
    ON delivery.continuation (run_id, status);
CREATE INDEX idx_continuation_origin
    ON delivery.continuation (origin_interaction_id);
