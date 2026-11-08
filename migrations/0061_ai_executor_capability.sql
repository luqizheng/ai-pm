-- 0061_ai_executor_capability: V2.0-D Capability Registry V1（渐进实现）。
-- 依据：docs/project-ai-team-v2/ai-pm-ai-team-kickoff-batch-record-v1.md 五.1
--       （Batch E 替换 ROLE_EXECUTOR_MAP 过渡映射）；V2.0 基线
--       ai-pm-v2-architecture-baseline-v1.md 内的 V2.0-D V1 渐进面。
--
-- 形态照 core.approver_grant（0040:23-38）先例：单行状态表 +
-- revoked_at 置空即生效 + 活跃部分索引。每 (tenant, principal, capability)
-- 一行；重复授予 = 复活既有行（revoked_at 置空 + granted_at 刷新），
-- 不允许并存的第二行（UNIQUE 兜底）。
--
-- 词表口径：V1 的 capability 词表 = ai.ai_role_template.role_key
-- （'project_manager' / 'requirement_analyst' / 'architect' / 'developer' /
-- 'tester'）。TEXT 无 FK：词表演进不锁库结构，应用层保证取值落在词表内。
--
-- source 语义：
--   agent_definition = bootstrap 由 core.tenant_agent × ai.agent_definition
--     派生的行，可重播（ensure_executor_capabilities 幂等播种）；
--   manual = 人工授予行，播种永不覆写（播种只 INSERT 全新行，见下）。
--
-- revoked 语义：revoked_at 非空 = 已撤销。bootstrap 播种只 INSERT 全新行
-- （ON CONFLICT DO NOTHING）：已存在但 revoked 的行**不复活**——复活是显式
-- 动作（人工置空 revoked_at / 显式授予），派生播种永不越权复活。
-- id 取 DEFAULT gen_random_uuid()（对齐 0040 同款单行状态表）。

CREATE TABLE ai.ai_executor_capability (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES core.tenant(id),
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    capability  TEXT NOT NULL,
    source      TEXT NOT NULL DEFAULT 'agent_definition'
                CHECK (source IN ('agent_definition', 'manual')),
    granted_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at  TIMESTAMPTZ,               -- 置空 = 生效
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, principal_id, capability)
);

CREATE INDEX idx_ai_executor_capability_active
    ON ai.ai_executor_capability (tenant_id, capability)
    WHERE revoked_at IS NULL;
