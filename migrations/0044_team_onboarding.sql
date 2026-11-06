-- 0044_team_onboarding: Team Onboarding & Team Management（设计稿
-- docs/team-onboarding/team-onboarding-team-management-spec-v1.md（产品裁决
-- 2026-11-06：Q1 首登必须建团或入团 / Q2 创建者自动 Owner / Q3 邮箱邀请+邀请链接）
-- 与 docs/team-onboarding/team-domain-model-database-design-v1.md §14 现实扫描
-- 裁决：**Team ≡ core.tenant（DEC-LLM-001 口径，Extend 不重建）**——设计稿的
-- core.team/team_membership 分别由 core.tenant / core.tenant_member（0019，
-- owner/admin/member 三角色现成）承担，project 保持 tenant_id 组合 FK 不加
-- team_id。本迁移只补：① tenant 团队档案列；② 邮箱邀请、邀请链接、链接
-- 领用三张邀请面（设计稿 §3.3/§3.4/§3.5）。
--
-- 现实约束（与设计稿的三处有意差异，代码注释同步）：
--   a) principal 无 email 列、JWT 无 email claim → 邀请的 email 匹配在
--      接受时按客户端提交的 iam UserInfo email 尽力比对（V1 裁决）；
--   b) V1 无 SMTP → 邀请只返回接受路径供复制分享，不投递邮件；
--   c) lmms 一个用户只属一个租户 → 邀请接受只可能发生在 token 所在租户内
--      （tid 绑定），同用户多团队待上游支持，本表结构已可扩展。
--   过期 = 惰性转移：accept 命中过期时置 EXPIRED 并拒绝；列表返回计算态。

-- ═══════════════════════════════════════════════════════════════════
-- 1) tenant 团队档案列（设计稿 §3.1 的 name/slug/description 落位：
--    display_name 即团队名；slug 全局唯一走部分唯一索引，NULL=未认领团队）
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE core.tenant
    ADD COLUMN slug        TEXT,
    ADD COLUMN description TEXT,
    ADD COLUMN updated_at  TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE UNIQUE INDEX uq_tenant_slug ON core.tenant (slug) WHERE slug IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════
-- 2) 邮箱邀请（设计稿 §3.3）：角色只允许 admin|member——OWNER 不可经邀请
--    产生（spec §5.1 / 权限矩阵）；同一团队对同一邮箱只允许一张 PENDING
--    邀请（部分唯一索引，§3.3 建议约束）；accepted_principal_id 而非
--    accepted_by 命名，对齐 tenant_audit_log.target_principal_id 口径。
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE core.tenant_invitation (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            UUID NOT NULL REFERENCES core.tenant(id),
    email                TEXT NOT NULL,
    email_normalized     TEXT NOT NULL,
    role                 TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    token_hash           TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'PENDING'
                         CHECK (status IN ('PENDING', 'ACCEPTED', 'EXPIRED', 'REVOKED')),
    invited_by           UUID NOT NULL REFERENCES core.principal(id),
    expires_at           TIMESTAMPTZ NOT NULL,
    accepted_principal_id UUID REFERENCES core.principal(id),
    accepted_at          TIMESTAMPTZ,
    revoked_at           TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    CHECK (expires_at > created_at)
);

-- 防同一团队同一邮箱多张有效待处理邀请（设计稿 §3.3）。
CREATE UNIQUE INDEX uq_tenant_invitation_pending_email
    ON core.tenant_invitation (tenant_id, email_normalized)
    WHERE status = 'PENDING';

CREATE INDEX tenant_invitation_tenant_status_idx
    ON core.tenant_invitation (tenant_id, status);
CREATE INDEX tenant_invitation_email_idx
    ON core.tenant_invitation (email_normalized);

-- ═══════════════════════════════════════════════════════════════════
-- 3) 邀请链接（设计稿 §3.4）：default_role 只允许 'member'——V1 链接
--    只发 MEMBER，禁止经链接产生 ADMIN/OWNER（权限矩阵「ADMIN 不得通过
--    邀请链接创建 ADMIN 或 OWNER」）。max_uses NULL = 不限次数；
--    used_count ≤ max_uses 落 CHECK（§8 完整性约束表）。
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE core.tenant_invitation_link (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL REFERENCES core.tenant(id),
    token_hash   TEXT NOT NULL,
    default_role TEXT NOT NULL CHECK (default_role IN ('member')),
    status       TEXT NOT NULL DEFAULT 'ACTIVE'
                 CHECK (status IN ('ACTIVE', 'EXHAUSTED', 'EXPIRED', 'REVOKED')),
    max_uses     INTEGER CHECK (max_uses IS NULL OR max_uses > 0),
    used_count   INTEGER NOT NULL DEFAULT 0,
    expires_at   TIMESTAMPTZ NOT NULL,
    created_by   UUID NOT NULL REFERENCES core.principal(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at   TIMESTAMPTZ,
    UNIQUE (tenant_id, id),
    CHECK (used_count >= 0),
    CHECK (max_uses IS NULL OR used_count <= max_uses),
    CHECK (expires_at > created_at)
);

-- Token 只能按哈希查找，全局唯一（防枚举；明文 token 不落库）。
CREATE UNIQUE INDEX uq_tenant_invitation_link_token_hash
    ON core.tenant_invitation_link (token_hash);

CREATE INDEX tenant_invitation_link_tenant_status_idx
    ON core.tenant_invitation_link (tenant_id, status);

-- ═══════════════════════════════════════════════════════════════════
-- 4) 链接领用记录（设计稿 §3.5）：同用户不能重复使用同一链接
--    UNIQUE(link_id, principal_id)；used_count 的消耗在服务层以
--    链接行 FOR UPDATE 事务内进行，并发争最后一个名额只赢一个。
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE core.tenant_invitation_link_redemption (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL REFERENCES core.tenant(id),
    link_id     UUID NOT NULL,
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    redeemed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, link_id)
        REFERENCES core.tenant_invitation_link (tenant_id, id),
    UNIQUE (link_id, principal_id)
);
