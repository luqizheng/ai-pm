-- S01-W13：Project 成员授权（`docs/ai-pm-s01-project-member-authorization-v1.md` §3）。
--
-- `core.project_member.role` 一列承载两类语义：
--   * 真人访问角色：owner / member —— 谁能管这个项目（本迁移的授权对象）
--   * Agent 职能定位：ai_pm / architect / developer / tester —— 任务承接定位
-- 既有 5 种取值全部在白名单内，**无需数据回填**（`AGENTS.md` 规则 1）。
ALTER TABLE core.project_member
    ADD CONSTRAINT ck_project_member_role
    CHECK (role IN ('owner', 'member', 'ai_pm', 'architect', 'developer', 'tester'));

-- 租户撤权级联：按 (tenant_id, principal_id) 定位真人项目成员行。
CREATE INDEX idx_project_member_tenant_principal
    ON core.project_member (tenant_id, principal_id);
