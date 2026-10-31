-- 0008_repositories: 项目 ↔ 代码仓库绑定 + 项目级 SSH 凭据（P0 仅关系 CRUD）。
-- 私钥明文永不落库：只存 AES-256-GCM 密文（nonce || ciphertext || tag），
-- 主密钥由 AI_PM_SECRET_KEY 派生（与 ai.llm_provider.api_key_ciphertext 同口径）。
-- 一个项目一把密钥，项目内所有仓库共用；不做 clone / 可达性校验（后续切片）。

CREATE TABLE integration.project_ssh_key (
    project_id UUID PRIMARY KEY REFERENCES core.project(id) ON DELETE CASCADE,
    label TEXT NOT NULL DEFAULT 'default',
    key_type TEXT NOT NULL DEFAULT 'ed25519' CHECK (key_type = 'ed25519'),
    public_key TEXT NOT NULL,
    private_key_ciphertext BYTEA NOT NULL,
    fingerprint TEXT NOT NULL,
    cipher_suite TEXT NOT NULL DEFAULT 'aes256gcm-v1',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE integration.project_repository (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'main'
        CHECK (role IN ('main', 'docs', 'service', 'infra')),
    default_branch TEXT,
    description TEXT,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, url),
    UNIQUE (project_id, id)
);

CREATE INDEX idx_project_repository_project
ON integration.project_repository(project_id, created_at DESC);