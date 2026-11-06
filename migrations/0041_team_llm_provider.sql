-- 0041_team_llm_provider: Team LLM Provider Management Phase 1（设计稿
-- docs/ui/smark-pm-team-llm-provider-management-v1.md §6/§7/§14；DEC-LLM-001~004；
-- 代码核验报告 docs/smark-pm-team-llm-provider-management-code-facts-v1.md Q-1~6
-- 已按建议裁决：Team≡tenant、路由 /v1/pm/tenants/{tenant_id}/llm-providers、
-- 0007 全局表退役、KMS/Vault 挂账沿用 AI_PM_SECRET_KEY）。
--
-- 内容：
--   1) ai.team_llm_provider：Team-owned Provider 实例（DEC-LLM-001：同 Team 可
--      多个同类型实例；provider_type 不设 DB CHECK——类型清单校验在服务层，
--      扩展 Adapter 不再需要三处同步改 CHECK/常量/前端联合类型）。
--      生命周期（§6.3）：not_configured → configured → connected|failed →
--      enabled ⇄ disabled → archived；测试成功≠自动启用。
--   2) ai.team_llm_credential：凭证独立表（§7）：AES-256-GCM 密文 + 末 4 位 hint；
--      替换制 = 新行 + provider.credential_id 重指向，旧行留痕（轮换历史）。
--      Team Owner 也不能回读明文——服务层只在调用/探测时解密。
--   3) ai.llm_call_record：模型调用记录（§14/AC-14，P1；消费方 = Phase 6 统一
--      LLM Gateway；连接测试也会落一行 purpose='connection_test'，使
--      「删除 Provider 前检查引用」（§14.4）从第一天就有事实可查）。
--   4) 退役 ai.llm_provider（0007）：全局单表、每类型一行，与 DEC-LLM-001
--      Team-owned 结构冲突；数据仅 5 条 disabled 种子、ai_analysis 主链仍接
--      FakeModel（TODO Task 17），无真实存量风险。repo_import 的
--      「全局第一条 enabled」兜底同批改租户作用域（repo_model::model_from_llm_provider）。
--      ai.project_ai_config.model_provider 列保留（bootstrap 种子 'fake'），
--      Phase 5 项目级模型策略落地时一并治理。

-- ═══════════════════════════════════════════════════════════════════
-- 1) Team Provider 实例
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE ai.team_llm_provider (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    name           TEXT NOT NULL,
    provider_type  TEXT NOT NULL,
    endpoint       TEXT NOT NULL,
    -- 调用参数过渡载体：Phase 3 模型目录落地前，model 等存 configuration JSONB
    -- （设计稿 §6.2 Configuration 字段）。
    configuration  JSONB NOT NULL DEFAULT '{}'::jsonb,
    status         TEXT NOT NULL DEFAULT 'not_configured'
                   CHECK (status IN ('not_configured', 'configured', 'connected',
                                     'failed', 'enabled', 'disabled', 'archived')),
    credential_id  UUID,                       -- 当前凭证；FK 在 credential 表后补（先有 provider 才有 credential）
    timeout_secs   INT NOT NULL DEFAULT 120 CHECK (timeout_secs BETWEEN 1 AND 600),
    revision       BIGINT NOT NULL DEFAULT 1,  -- 乐观锁（先例：command_executor CompensableTable 口径）
    last_tested_at TIMESTAMPTZ,
    created_by     UUID NOT NULL REFERENCES core.principal(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name),
    UNIQUE (tenant_id, id)
);

CREATE INDEX team_llm_provider_tenant_idx ON ai.team_llm_provider (tenant_id, status);

-- ═══════════════════════════════════════════════════════════════════
-- 2) 凭证（独立表；替换制留痕）
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE ai.team_llm_credential (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL REFERENCES core.tenant(id),
    provider_id        UUID NOT NULL,
    api_key_ciphertext BYTEA NOT NULL,
    api_key_hint       TEXT NOT NULL,
    created_by         UUID NOT NULL REFERENCES core.principal(id),
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 租户对齐组合 FK（0019 口径）：凭证与 Provider 必须同 Team。
    FOREIGN KEY (tenant_id, provider_id)
        REFERENCES ai.team_llm_provider (tenant_id, id) ON DELETE CASCADE,
    UNIQUE (tenant_id, id)
);

ALTER TABLE ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_credential_fk
    FOREIGN KEY (credential_id) REFERENCES ai.team_llm_credential (id) ON DELETE SET NULL;

-- ═══════════════════════════════════════════════════════════════════
-- 3) 调用记录（§14；错误分类 = §11 七类）
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE ai.llm_call_record (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES core.tenant(id),
    project_id        UUID REFERENCES core.project(id),
    provider_id       UUID NOT NULL,
    model             TEXT NOT NULL,
    purpose           TEXT,                  -- AI Role / connection_test（Phase 3 策略解析填充）
    status            TEXT NOT NULL CHECK (status IN ('ok', 'failed')),
    error_kind        TEXT,                  -- authentication/connection/timeout/rate_limit/provider_error/model_unavailable/configuration
    error_message     TEXT,
    prompt_tokens     BIGINT,
    completion_tokens BIGINT,
    latency_ms        BIGINT,
    analysis_run_id   UUID REFERENCES ai.analysis_run(id),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, provider_id)
        REFERENCES ai.team_llm_provider (tenant_id, id)
);

CREATE INDEX llm_call_record_tenant_idx ON ai.llm_call_record (tenant_id, created_at DESC);
CREATE INDEX llm_call_record_provider_idx ON ai.llm_call_record (provider_id);

-- ═══════════════════════════════════════════════════════════════════
-- 4) 租户审计通用目标列（对齐 governance.audit_log 的 target_type/target_id 口径；
--    tenant_audit_log 原只有 target_principal_id，Provider 等 Team 资源事件
--    的目标不是 principal）
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE governance.tenant_audit_log
    ADD COLUMN target_type TEXT,
    ADD COLUMN target_id   UUID;

-- ═══════════════════════════════════════════════════════════════════
-- 5) 退役全局表
-- ═══════════════════════════════════════════════════════════════════

DROP TABLE ai.llm_provider;
