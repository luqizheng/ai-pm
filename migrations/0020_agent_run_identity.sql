-- 0020_agent_run_identity: Agent 三层与 Run 身份归因（T-021b γ+δ）
-- 依据：docs/ai-pm-tenant-infrastructure-v1.md §3.8、§4.4；
--       docs/ai-pm-adr-0001-tenant-agent-executor-identity.md D3/D6。
--
-- 内容：
--   1) ai.agent_definition —— 全局可复用的 Agent 角色定义（D3 第一层）
--   2) core.tenant_agent   —— 租户启用的 Agent 实例，映射到一个 dev_agent principal
--   3) delivery.task_run 三列：agent_id / triggered_by_user_id / executor_id
--
-- 项目授权仍复用 core.project_member，不另造 grant 表（D3）。
-- executor_id 不加外键：Executor Registry 归 T-030，届时补 FK（D6）。

-- 1) 全局角色定义
CREATE TABLE ai.agent_definition (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code         TEXT NOT NULL,
    display_name TEXT NOT NULL,
    role_kind    TEXT NOT NULL CHECK (role_kind IN ('architect', 'developer', 'tester')),
    description  TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (code)
);

INSERT INTO ai.agent_definition (code, display_name, role_kind, description)
VALUES
    ('architect', '架构 Agent', 'architect', '负责技术方案与架构一致性'),
    ('developer', '开发 Agent', 'developer', '负责实现与交付'),
    ('tester', '测试 Agent', 'tester', '负责验证与失败判定')
ON CONFLICT (code) DO NOTHING;

-- 2) 租户 Agent 实例
--    一个 principal 只能代表一个租户 Agent 实例；`is_enabled=false` 时 MCP 领取 403。
CREATE TABLE core.tenant_agent (
    tenant_id         UUID NOT NULL REFERENCES core.tenant(id) ON DELETE CASCADE,
    agent_definition_id UUID NOT NULL REFERENCES ai.agent_definition(id),
    principal_id      UUID NOT NULL REFERENCES core.principal(id),
    is_enabled        BOOLEAN NOT NULL DEFAULT true,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, agent_definition_id),
    UNIQUE (principal_id)
);

-- 3) Run 身份归因（D6：executor_id 无 FK，Registry 归 T-030）
--    created_by / approved_by 不写 Run——审计行已承载，双写会产生两套真相。
ALTER TABLE delivery.task_run
    ADD COLUMN agent_id            UUID REFERENCES core.principal(id),
    ADD COLUMN triggered_by_user_id UUID REFERENCES core.principal(id),
    ADD COLUMN executor_id         UUID;

-- 系统租户的 3 个 dev_agent 实例**不在这里种**：由 `bootstrap` 在该租户首次
-- 建项时创建（`ensure_tenant_agents`）。迁移只建结构，避免空库出现
-- `core.principal` 行而破坏 `/v1/pm/principals` 的零总数契约。
