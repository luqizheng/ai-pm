-- 0041: P4-ORG Phase 4 Agent Registry 迁移（DR-PA-04③ / D-2，2026-11-20 签发）
-- 1) ai.project_agent_member：项目 × Agent 执行实例授权行（承接原
--    core.project_member.agent 职能行的两个项目级职责——resolve_project_executor
--    执行主体锚与 resolve_role_assignee registry 的项目授权 JOIN）。
--    能力（租户粒度）继续由 ai.ai_executor_capability 承载（registry-first 现状）。
-- 2) core.project_member.role CHECK 收敛为人类两值 {owner, member}：
--    Agent 职能（ai_pm/architect/developer/tester）不再是人类项目角色
--    （D-2：职能 ≠ 访问角色）。无存量数据：直接收敛（AGENTS.md 规则 1）。

CREATE TABLE ai.project_agent_member (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    role_kind text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_project_agent_member_kind CHECK (
        (role_kind = ANY (ARRAY['ai_pm'::text, 'architect'::text, 'developer'::text, 'tester'::text]))
    ),
    CONSTRAINT fk_project_agent_member_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id),
    CONSTRAINT fk_project_agent_member_project FOREIGN KEY (project_id) REFERENCES core.project(id) ON DELETE CASCADE,
    CONSTRAINT fk_project_agent_member_principal FOREIGN KEY (principal_id) REFERENCES core.principal(id)
);

ALTER TABLE ai.project_agent_member
    ADD CONSTRAINT project_agent_member_pkey PRIMARY KEY (project_id, principal_id);
CREATE INDEX idx_project_agent_member_tenant_kind
    ON ai.project_agent_member (tenant_id, role_kind);
CREATE INDEX idx_project_agent_member_principal
    ON ai.project_agent_member (principal_id);

ALTER TABLE core.project_member DROP CONSTRAINT ck_project_member_role;
ALTER TABLE core.project_member
    ADD CONSTRAINT ck_project_member_role CHECK ((role = ANY (ARRAY['owner'::text, 'member'::text])));
