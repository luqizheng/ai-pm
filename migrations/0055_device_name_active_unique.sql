-- 0055_device_name_active_unique: 修正 0049 的 uq_device_tenant_name 与
-- devenv 0027 既有注册语义的正面冲突（checkpoint-1 基线 4 红中的 devenv 2 红）。
--
-- 冲突事实（tests/devenv.rs，0027 批既有断言）：
--   ① revoked_device_is_not_auto_restored_on_reregister —— 撤销后同名重注册
--      = 合法（旧行留历史 + 新行新 id）；全量 UNIQUE 让 revoked 行永久占名。
--   ② 同租户不同 owner 可各自注册（可见性测试用固定名）。
-- 修正语义（Q-D 精神保留）：**活跃名**租户内唯一——client_name 归属唯一性
-- 只在参与路由/展示的活跃设备上有意义；revoked 行不占名，重注册不再阻塞。
-- 取号：初取 0054 与并行 ACP-06/07 批（0054_acp06_07）撞号，按后实施者
-- 纪律顺移 0055。0049 已发布不可改，走新迁移重建。

ALTER TABLE devenv.device
    DROP CONSTRAINT uq_device_tenant_name;

CREATE UNIQUE INDEX uq_device_tenant_active_name
    ON devenv.device (tenant_id, name)
    WHERE status = 'active';
