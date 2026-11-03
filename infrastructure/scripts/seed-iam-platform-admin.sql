-- infrastructure/scripts/seed-iam-platform-admin.sql
-- ─────────────────────────────────────────────────────────────────────────────
-- 让「首个注册用户」成为全平台管理员，并把 ai-pm 的 MCP 权限码接入 IAM 权限目录。
--
-- 背景（内网测试机 / 两机拓扑实测）：
--   * lmms IAM 侧的权限目录出厂只带一条 IAM_SYSTEM_READ（见 lmms
--     iam-service 的 V00x 迁移种子），没有 ai-pm 的 MCP 权限码；
--   * pm-api 的 /mcp 门禁按 JWT claims 的 permissions 判定
--     （apps/pm-api/src/auth.rs：PM_MCP_READ 必带，写工具另需
--     PM_MCP_DOCUMENT_WRITE）；
--   * credential-login 登录路径不做「客户端权限交集」，JWT permissions
--     = 用户所绑角色的权限并集 —— 所以只要给角色授权即可。
--
-- 本脚本做三件事：
--   1) 往 iam_permissions 补 PM_MCP_READ / PM_MCP_DOCUMENT_WRITE；
--   2) 把权限目录中全部 enabled 权限绑给 tenant_admin 角色
--      （tenant_admin 角色本身由 lmms 迁移种子创建，也是注册通道默认赋予的角色）；
--   3) 清理 admin 的 platform_admin 绑定（见下方「为什么不是 platform_admin」）。
--
-- ⚠️ 为什么不是 lmms 的 platform_admin 角色：
--   lmms 的 iam-service 把「持有 platform_admin 角色」直接翻译成 JWT 的
--   `ct=true`（跨租户令牌）—— 见 services/iam-service/src/service/auth_svc.rs
--   `let cross_tenant = roles.iter().any(|r| r == "platform_admin");`
--   而 ai-pm 的 V1 边界是「只接受租户内令牌」：ct=true → 403 IAM_PERM_3004
--   （docs/ai-pm-principal-identity-v1.md §鉴权矩阵）。
--   两者直接冲突：绑 platform_admin = 令牌必然被 ai-pm 业务端点拒绝。
--   因此「全平台管理员」在 ai-pm 侧的正确表达是：
--     · lmms 角色 = tenant_admin（ct=false）
--     · 权限码 = 权限目录全部条目（含 PM_MCP_*）
--     · ai-pm 库内 core.tenant_member.role='owner' + core.project_member='owner'
--       （由 scripts/seed-demo.sql 的 2c/2d 段补齐）
--
-- 用法（目标库 = lmms 栈的 postgres 容器里的 coder 库）：
--   docker exec -i infrastructure-postgres-1 psql -U postgres -d coder \
--     < infrastructure/scripts/seed-iam-platform-admin.sql
--
-- 幂等：可重复执行。
-- ⚠️ 仅用于测试/内网环境：生产应由 IAM 的正式管理接口分配权限，
--    而不是直接改库。
-- ─────────────────────────────────────────────────────────────────────────────

BEGIN;

-- 1) 权限目录：ai-pm 的 MCP 权限码 -------------------------------------------
INSERT INTO iam_permissions
  (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at)
VALUES
  ('perm-pm-mcp-read', 'PM_MCP_READ', 'MCP 只读',
   '调用 /mcp 的只读工具（pm_list_tasks 等）', 'pm', 'mcp', 'read', 900, 'enabled', now(), now()),
  ('perm-pm-mcp-document-write', 'PM_MCP_DOCUMENT_WRITE', 'MCP 文档写入',
   '调用 /mcp 的文档写工具（submit/propose/confirm/veto）', 'pm', 'mcp', 'write', 901, 'enabled', now(), now())
ON CONFLICT (code) DO NOTHING;

-- 2) tenant_admin ← 权限目录全部 enabled 权限 --------------------------------
INSERT INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT r.id, p.id, now()
  FROM iam_roles r
  CROSS JOIN iam_permissions p
 WHERE r.code = 'tenant_admin'
   AND p.status = 'enabled'
   AND p.deleted_at IS NULL
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- 3) 解除 admin 的 platform_admin 绑定（否则 JWT ct=true → ai-pm 403 IAM_PERM_3004）
DELETE FROM iam_user_roles ur
 USING iam_users u, iam_roles r
 WHERE ur.user_id = u.id
   AND ur.role_id = r.id
   AND u.username = 'admin'
   AND r.code = 'platform_admin';

COMMIT;

-- 核对：应看到 admin 拥有 platform_admin，且 platform_admin 覆盖权限目录全部条目
--   SELECT u.username, r.code AS role FROM iam_user_roles ur
--     JOIN iam_users u ON u.id = ur.user_id
--     JOIN iam_roles r ON r.id = ur.role_id ORDER BY 1,2;
--   SELECT r.code AS role, p.code AS permission FROM iam_role_permissions rp
--     JOIN iam_roles r ON r.id = rp.role_id
--     JOIN iam_permissions p ON p.id = rp.permission_id ORDER BY 1,2;
