-- 0057_drop_project_policy_provider_unique: D-3=A 放开
-- ai.project_ai_model_policy 的 UNIQUE (tenant_id, provider_id)
-- （裁决 docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md §〇）。
--
-- 0043 内联 UNIQUE 让「一个 Provider 只能被一个项目指定」，与 V2 §九
-- 「Team 授权范围内多项目共享 Provider」语义相悖；删除后同一 Provider 可被
-- 多项目各自指定。PK (tenant_id, project_id) 与普通索引
-- project_ai_model_policy_provider_idx（0043:27）不动。
-- 约束未显式命名，PG 自动名为 project_ai_model_policy_tenant_id_provider_id_key。
-- 取号：初取时 0056 已被并行 S02-I5 验收批占用，按后实施者纪律顺移取
-- 当时最大号+1 = 0057。

ALTER TABLE ai.project_ai_model_policy
    DROP CONSTRAINT project_ai_model_policy_tenant_id_provider_id_key;
