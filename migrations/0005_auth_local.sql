-- 0005_auth_local: pm-api 本地会话。
-- IAM 上游服务就绪前的过渡：pm-api 自发 opaque token，登录时按 display_name
-- 找或建人类 principal（apps/pm-api/src/auth.rs）。真实认证/角色/权限由 IAM 接管后弃用。
CREATE TABLE core.auth_session (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    refresh_token UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_auth_session_principal ON core.auth_session(principal_id);
