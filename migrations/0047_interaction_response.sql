-- 0047_interaction_response: ACP-03 Human Response（设计稿
-- docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp03-v1.md V1.0，
-- V1.3.1 §12.2 全字段 + §8.4/§9.3 幂等与并发约束）。
--
-- 取号：现最大 0046 → 本迁移 0047（R-ACP-13 已裁，落地前重核台账）。
-- 不可覆盖（§8.4）：无 UPDATE 路径；后续修改 = 新审计记录。

CREATE TABLE delivery.interaction_response (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),   -- response_id
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    project_id     UUID NOT NULL REFERENCES core.project(id),
    interaction_id UUID NOT NULL REFERENCES delivery.task_interaction(id),
    task_id        UUID NOT NULL REFERENCES delivery.task(id),
    run_id         UUID NOT NULL REFERENCES delivery.task_run(id),
    responded_by   UUID NOT NULL REFERENCES core.principal(id),
    response_payload JSONB NOT NULL,
    idempotency_key   TEXT NOT NULL,
    -- §8.3 六项检查结果快照（谁/何通道/何角色/检查①~⑥结论）——审计取证面
    authorization_context JSONB NOT NULL,
    -- 同事务下发的 continue 命令（V1 投递事实；delivery 细账随 ACP-04 前向兼容）
    continue_command_id UUID,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);

-- 幂等：同 (interaction, idempotency_key) 重放返回原行（§8.4）
CREATE UNIQUE INDEX uq_interaction_response_idem
    ON delivery.interaction_response (interaction_id, idempotency_key);

-- §9.3 并发裁决：同 Interaction 至多一条有效回复（败者 409，无静默覆盖）
CREATE UNIQUE INDEX uq_interaction_response_single
    ON delivery.interaction_response (interaction_id);

CREATE INDEX interaction_response_task_idx
    ON delivery.interaction_response (task_id, created_at);
