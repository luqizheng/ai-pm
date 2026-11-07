-- 0050_response_delivery: ACP-04 Delivery 持久化
-- （联合数据契约 docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp04-05-joint-v1.md V1.0
--   §2.1；联合架构裁决 2026-11-07：五问①~⑤通过 + 0050 模型确认。
--   基线字段集 = V1.3.1 §12.3 + 联合契约修订。）
--
-- 取号：台账预留 0050（现最大 0049_client_registry；落地前已重核）。
-- 核心不变量：UNIQUE(response_id) = 一回复一投递账——失败重投复用原行
--（attempt_count+1），不因重投创建新行（V1.2 §1.4 / ACP-471）。
-- ⑤ 前向兼容：continuation_id 列先落可空（ACP-05/0051 身份注册后回填）；
--   不临时另建 Session 身份（三件定案见联合契约 §0⑤）。
-- ② 基线 §3.3：continuation_generation 与 lease_generation 分离；V1 恒 1（R-8）。

CREATE TABLE delivery.interaction_response_delivery (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),   -- delivery_id
    tenant_id               UUID NOT NULL REFERENCES core.tenant(id),
    project_id              UUID NOT NULL REFERENCES core.project(id),
    interaction_id          UUID NOT NULL REFERENCES delivery.task_interaction(id),
    response_id             UUID NOT NULL REFERENCES delivery.interaction_response(id),
    -- 0047 同名列镜像；存量行由接线批回填
    continue_command_id     UUID,
    run_id                  UUID NOT NULL REFERENCES delivery.task_run(id),
    -- ⑤ 前向兼容（ACP-05 落地后回填）；V1 恒 NULL 不阻塞任何路径
    continuation_id         UUID,
    -- ② Continuation 代次；V1 恒 1（R-8：ACP-05 前恒 1）
    continuation_generation INT  NOT NULL DEFAULT 1,
    status                  TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','delivering','delivered','consumed','failed')),
    attempt_count           INT  NOT NULL DEFAULT 0,
    last_attempt_at         TIMESTAMPTZ,
    delivered_at            TIMESTAMPTZ,
    consumed_at             TIMESTAMPTZ,
    last_error              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);

-- 一回复一投递账（重投复用原行）
CREATE UNIQUE INDEX uq_response_delivery_one
    ON delivery.interaction_response_delivery (response_id);
CREATE INDEX idx_response_delivery_interaction
    ON delivery.interaction_response_delivery (interaction_id);
-- N-6 告警面（unconsumed 指标切表：delivering/delivered 即未消费）
CREATE INDEX idx_response_delivery_unconsumed
    ON delivery.interaction_response_delivery (project_id, status)
    WHERE status IN ('delivering', 'delivered');
