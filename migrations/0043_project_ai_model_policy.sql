-- 0043_project_ai_model_policy: Project AI Model Policy（设计稿 §10.2 Project Model
-- Override / §16 PUT /projects/{id}/ai-model-policies；DEC-LLM-002：Project 只能在
-- Team 授权范围内选择）。
--
-- 项目 → 指定 Team-owned Provider 实例（ai.team_llm_provider，同租户组合 FK 锁定）。
-- V1 语义（Phase 3 七 AI Role 策略解析前的过渡口径）：
--   - 每项目至多一条指定（UNIQUE(tenant_id, project_id)）；
--   - 未指定 = 继承 Team 解析（首个 enabled，创建序）；
--   - 指定的 Provider 被停用/归档后不再回退 Team 默认（§10.3 不静默降级），
--     解析返回 None → 上层以 model_name='fake' 可见呈现（既有 dev 语义）。
-- ai.project_ai_config 不动（Phase 5 项目级模型策略落地时一并治理，见核验报告 §8.3）。

CREATE TABLE ai.project_ai_model_policy (
    tenant_id   UUID NOT NULL REFERENCES core.tenant(id),
    project_id  UUID NOT NULL,
    provider_id UUID NOT NULL,
    revision    BIGINT NOT NULL DEFAULT 1,
    updated_by  UUID NOT NULL REFERENCES core.principal(id),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, project_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, provider_id)
        REFERENCES ai.team_llm_provider (tenant_id, id),
    UNIQUE (tenant_id, provider_id)
);

CREATE INDEX project_ai_model_policy_provider_idx ON ai.project_ai_model_policy (provider_id);
