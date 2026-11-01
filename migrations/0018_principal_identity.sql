-- 0018_principal_identity: JWT 调用者 → 真实主体（T-021a / β）
-- 依据：docs/ai-pm-principal-identity-v1.md §5（范围已按 ADR-0001 收窄为 β）
-- 原则：forward-only；不改 core.principal 的 kind 取值；无回填、无兼容分支。
--
-- 说明：上游 claims.tid 落在 `claims_tenant_key`（TEXT），**不叫 tenant_id**：
-- 内部 tenant_id 是 T-021b 的 core.tenant 外键（UUID），两者不是同一物
-- （ADR-0001 §7.1）。新增列名若取 tenant_id，T-021b 会不得不在同一张新表上
-- 改名 + 改类型，属无谓返工。

CREATE TABLE core.principal_identity (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider          TEXT NOT NULL,          -- V1 只写 'lmms'
    claims_tenant_key TEXT NOT NULL,          -- claims.tid 字符串化（int64 → text）
    subject           TEXT NOT NULL,          -- claims.sub（iam user id）
    principal_id      UUID NOT NULL REFERENCES core.principal(id),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, claims_tenant_key, subject)
);

CREATE INDEX idx_principal_identity_principal ON core.principal_identity(principal_id);

-- 审计补三列：能回答「谁的身份 + 用什么形态的 token」。
-- actor_id 仍是执行主体（HTTP 写路径 = 真人 principal，MCP = 项目内 dev_agent/ai_pm）。
ALTER TABLE governance.audit_log
    ADD COLUMN IF NOT EXISTS actor_identity_id UUID REFERENCES core.principal_identity(id),
    ADD COLUMN IF NOT EXISTS actor_client      TEXT,
    ADD COLUMN IF NOT EXISTS actor_auth_mode   TEXT;   -- 'interactive' | 'api_key' | 'unknown'
