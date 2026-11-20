-- 0039: P4-ORG Phase 2 Team 成员面切换（基线 V0.1 §3；DR-ORG-02，2026-11-19 签发）
-- 1) Team 面事实源切换：core.team_member（0038）承载全部 Team 成员关系；
--    core.tenant_member 收敛为 provision 预登记面（role 收敛 owner|member，
--    无第三角色；admin 从组织模型消失）。
-- 2) 邀请链挂 Team：tenant_invitation / tenant_invitation_link 增加 team_id；
--    邀请角色仅 member（Team Owner 只能任命产生，不得经邀请产生）。
-- 无存量数据：直接改契约，不做数据搬运（AGENTS.md 规则 1）。

ALTER TABLE core.tenant_member DROP CONSTRAINT tenant_member_role_check;
ALTER TABLE core.tenant_member
    ADD CONSTRAINT tenant_member_role_check CHECK ((role = ANY (ARRAY['owner'::text, 'member'::text])));

ALTER TABLE core.tenant_invitation ADD COLUMN team_id uuid NOT NULL;
ALTER TABLE core.tenant_invitation
    ADD CONSTRAINT fk_tenant_invitation_team FOREIGN KEY (team_id) REFERENCES core.team(id);
ALTER TABLE core.tenant_invitation DROP CONSTRAINT tenant_invitation_role_check;
ALTER TABLE core.tenant_invitation
    ADD CONSTRAINT tenant_invitation_role_check CHECK ((role = 'member'::text));

ALTER TABLE core.tenant_invitation_link ADD COLUMN team_id uuid NOT NULL;
ALTER TABLE core.tenant_invitation_link
    ADD CONSTRAINT fk_tenant_invitation_link_team FOREIGN KEY (team_id) REFERENCES core.team(id);
