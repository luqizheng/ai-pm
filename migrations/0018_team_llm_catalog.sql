-- 0018: Team LLM Model Catalog / Authorization / Policy（Phase 3A）。
-- 设计基线：docs/ui/smark-pm-team-llm-provider-management-phase3-design-v1.md §4/§11；
-- 上游产品裁决：docs/ui/smark-pm-team-llm-provider-management-v1.md §8/§9/§10（V1.0 Approved）。
--
-- 模型粒度取代 provider.configuration->>'model' 单值自由文本；授权两态
-- （team_wide/revoked，§9 V1 口径，restricted 留 additive 扩展）；Team 七 Role
-- 默认策略（role_key='general' 为解析兜底）+ 项目 Role 覆盖（恢复继承=删行）。
-- 零凭证列（凭证仍独立存 ai.team_llm_credential）。
--
-- 惯例：UUID PK + UNIQUE(tenant_id,id) 组合键支撑组合 FK（0019 模式）；
-- revision 乐观锁；TIMESTAMPTZ；snake_case。

-- ─── ai.team_llm_model：模型目录（AC-04） ───────────────────────────
CREATE TABLE ai.team_llm_model (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    model_id text NOT NULL,
    display_name text NOT NULL,
    capabilities text[] DEFAULT '{}'::text[] NOT NULL,
    context_window integer,
    enabled boolean DEFAULT false NOT NULL,
    source text DEFAULT 'manual'::text NOT NULL,
    last_verified_at timestamp with time zone,
    revision bigint DEFAULT 1 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT team_llm_model_context_window_check CHECK (
        context_window IS NULL OR context_window > 0
    ),
    CONSTRAINT team_llm_model_source_check CHECK (source = ANY (ARRAY['discovered'::text, 'manual'::text])),
    -- §8.2 六能力词表；未知能力不得编造（空数组 = 未验证）
    CONSTRAINT team_llm_model_capabilities_check CHECK (
        capabilities <@ ARRAY['text_generation'::text, 'reasoning'::text, 'structured_output'::text,
                              'tool_calling'::text, 'vision'::text, 'long_context'::text]
    )
);

ALTER TABLE ONLY ai.team_llm_model
    ADD CONSTRAINT team_llm_model_pkey PRIMARY KEY (id);
ALTER TABLE ONLY ai.team_llm_model
    ADD CONSTRAINT team_llm_model_tenant_id_id_key UNIQUE (tenant_id, id);
-- 同 Provider 下模型身份唯一（重名登记拒绝）
ALTER TABLE ONLY ai.team_llm_model
    ADD CONSTRAINT team_llm_model_provider_model_key UNIQUE (tenant_id, provider_id, model_id);
-- 组合 FK 锁 Team 一致性：模型必须挂在本租户的 Provider 上（§14 约束 2）
ALTER TABLE ONLY ai.team_llm_model
    ADD CONSTRAINT team_llm_model_provider_fkey FOREIGN KEY (tenant_id, provider_id)
        REFERENCES ai.team_llm_provider (tenant_id, id) ON DELETE CASCADE;

CREATE INDEX team_llm_model_provider_idx ON ai.team_llm_model USING btree (tenant_id, provider_id);
CREATE INDEX team_llm_model_enabled_idx ON ai.team_llm_model USING btree (tenant_id, enabled);

-- ─── ai.team_llm_model_authorization：模型授权（AC-05） ─────────────
-- enabled（能力面，目录表）与 authorization（许可面）独立：撤销不删目录、
-- 不改 enabled，不静默改写项目配置（§9）。
CREATE TABLE ai.team_llm_model_authorization (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    model_id uuid NOT NULL,
    -- 列名 state（授权状态）：authorization 是 PG 保留字，不能裸用作列名；
    -- JSON 契约字段仍为 authorization（AuthorizationRow Serialize 不变）
    state text DEFAULT 'team_wide'::text NOT NULL,
    granted_by uuid NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT team_llm_model_authorization_state_check CHECK (state = ANY (ARRAY['team_wide'::text, 'revoked'::text]))
);

ALTER TABLE ONLY ai.team_llm_model_authorization
    ADD CONSTRAINT team_llm_model_authorization_pkey PRIMARY KEY (id);
ALTER TABLE ONLY ai.team_llm_model_authorization
    ADD CONSTRAINT team_llm_model_authorization_tenant_id_id_key UNIQUE (tenant_id, id);
-- 一模型一授权行（撤销 = 行内状态翻转，保留审计连续性）
ALTER TABLE ONLY ai.team_llm_model_authorization
    ADD CONSTRAINT team_llm_model_authorization_model_key UNIQUE (tenant_id, model_id);
ALTER TABLE ONLY ai.team_llm_model_authorization
    ADD CONSTRAINT team_llm_model_authorization_model_fkey FOREIGN KEY (tenant_id, model_id)
        REFERENCES ai.team_llm_model (tenant_id, id) ON DELETE CASCADE;

-- ─── ai.team_llm_model_policy：Team 按 AI Role 默认模型（AC-06） ────
-- role_key 七值（设计稿 §10.1，任务书 §3 裁决 D-P3-1）；未配置 Role 无行 =
-- 解析继承 general；general 行缺失 = 解析末级显式 Configuration Error。
CREATE TABLE ai.team_llm_model_policy (
    tenant_id uuid NOT NULL,
    role_key text NOT NULL,
    model_id uuid NOT NULL,
    updated_by uuid NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT team_llm_model_policy_role_key_check CHECK (role_key = ANY (ARRAY[
        'general'::text, 'ai_pm_lead'::text, 'requirement_analysis'::text,
        'task_analysis'::text, 'risk_analysis'::text, 'architecture_analysis'::text,
        'knowledge_management'::text
    ]))
);

ALTER TABLE ONLY ai.team_llm_model_policy
    ADD CONSTRAINT team_llm_model_policy_pkey PRIMARY KEY (tenant_id, role_key);
ALTER TABLE ONLY ai.team_llm_model_policy
    ADD CONSTRAINT team_llm_model_policy_model_fkey FOREIGN KEY (tenant_id, model_id)
        REFERENCES ai.team_llm_model (tenant_id, id);

-- ─── ai.project_ai_model_override：项目 Role 覆盖（AC-08，修 D-2） ──
-- 删行 = 恢复继承 Team；写入时模型必须 enabled + team_wide 授权（解析同款
-- 严格校验，§6.1）。现 project_ai_model_policy（Provider 约束）零改动保留。
CREATE TABLE ai.project_ai_model_override (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    role_key text NOT NULL,
    model_id uuid NOT NULL,
    updated_by uuid NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT project_ai_model_override_role_key_check CHECK (role_key = ANY (ARRAY[
        'general'::text, 'ai_pm_lead'::text, 'requirement_analysis'::text,
        'task_analysis'::text, 'risk_analysis'::text, 'architecture_analysis'::text,
        'knowledge_management'::text
    ]))
);

ALTER TABLE ONLY ai.project_ai_model_override
    ADD CONSTRAINT project_ai_model_override_pkey PRIMARY KEY (tenant_id, project_id, role_key);
ALTER TABLE ONLY ai.project_ai_model_override
    ADD CONSTRAINT project_ai_model_override_project_fkey FOREIGN KEY (tenant_id, project_id)
        REFERENCES core.project (tenant_id, id) ON DELETE CASCADE;
ALTER TABLE ONLY ai.project_ai_model_override
    ADD CONSTRAINT project_ai_model_override_model_fkey FOREIGN KEY (tenant_id, model_id)
        REFERENCES ai.team_llm_model (tenant_id, id);

-- ─── llm_call_record.model_ref：调用记账的模型归因（AC-14 前置） ────
ALTER TABLE ai.llm_call_record ADD COLUMN model_ref uuid;
ALTER TABLE ai.llm_call_record
    ADD CONSTRAINT llm_call_record_model_ref_fkey FOREIGN KEY (tenant_id, model_ref)
        REFERENCES ai.team_llm_model (tenant_id, id);