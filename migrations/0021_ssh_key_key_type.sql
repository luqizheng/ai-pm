-- 0021：放宽 integration.project_ssh_key.key_type —— 支持导入外部私钥
--
-- 用途：仓库设置页新增「粘贴自有私钥」能力（后端 POST /v1/pm/projects/{id}/ssh-keys/import）。
--   导入时经 `ssh-keygen -y` 从私钥推导公钥，key_type 取推导公钥的首 token
--   （ssh-ed25519 / ssh-rsa / ecdsa-sha2-nistp256/384/521）。
--   原 CHECK 仅允许 'ed25519'（服务端生成路径的 DEFAULT 值，保留兼容），
--   导入外部密钥会被 CHECK 拒写，故放宽为白名单。
--
-- 迁移号：0021 = 当前 migrations/ 目录最大号 0020 + 1（台账 §三 纪律）。

ALTER TABLE integration.project_ssh_key
    DROP CONSTRAINT project_ssh_key_key_type_check;

ALTER TABLE integration.project_ssh_key
    ADD CONSTRAINT project_ssh_key_key_type_check CHECK (
        key_type IN ('ed25519'::text, 'ssh-ed25519'::text, 'ssh-rsa'::text,
                     'ecdsa-sha2-nistp256'::text, 'ecdsa-sha2-nistp384'::text,
                     'ecdsa-sha2-nistp521'::text)
    );
