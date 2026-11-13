-- 0022：project_ssh_key 多行化 + 仓库级绑定（仓库可各绑一把 key）
--
-- 背景：原 PK=(project_id) 单行模型只支持「一项目一把」；一个项目连多个
-- Git 服务器 / 多个账号时无法各配各的 key。本迁移：
--   1. key 表加独立 id，PK 改 (id)——项目下可并存多把 key；
--   2. is_default 标记「项目默认 key」（未绑定仓库的回退目标），
--      部分唯一索引保证每项目至多一把默认；
--   3. UNIQUE (project_id, label)——同名导入 = 替换该把 key（label 即身份）；
--   4. project_repository.ssh_key_id 启用：加 FK RESTRICT（同聚合惯例；
--      被引用的 key 行不可物理删除，吊销走状态位）。
--
-- 回退语义（应用层单一实现，见 pm-application repository/ssh_key.rs）：
--   仓库绑定 ssh_key_id → 用它；否则用项目默认 key → 否则最新 active key；
--   都没有 → verify 落 ssh_key_missing（unreachable），不报 500。
--
-- 迁移号：0022 = 目录最大 0021 + 1（台账 §三 纪律）。

ALTER TABLE integration.project_ssh_key
    ADD COLUMN id uuid DEFAULT gen_random_uuid();
ALTER TABLE integration.project_ssh_key ALTER COLUMN id SET NOT NULL;

ALTER TABLE integration.project_ssh_key DROP CONSTRAINT project_ssh_key_pkey;
ALTER TABLE integration.project_ssh_key
    ADD CONSTRAINT project_ssh_key_pkey PRIMARY KEY (id);

ALTER TABLE integration.project_ssh_key
    ADD COLUMN is_default boolean DEFAULT false NOT NULL;
CREATE UNIQUE INDEX uq_project_ssh_key_default
    ON integration.project_ssh_key (project_id) WHERE is_default;

CREATE UNIQUE INDEX uq_project_ssh_key_project_label
    ON integration.project_ssh_key (project_id, label);
CREATE INDEX idx_project_ssh_key_project_status
    ON integration.project_ssh_key (project_id, status);

ALTER TABLE integration.project_repository
    ADD CONSTRAINT project_repository_ssh_key_fk
    FOREIGN KEY (ssh_key_id) REFERENCES integration.project_ssh_key (id)
    ON DELETE RESTRICT;
