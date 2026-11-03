-- 0032_s01_git_capability: S01 · I3-B Git 操作能力模型（B-6 方案 A，2026-11-04 默认生效）。
--
-- 依据：docs/ui/smark-pm-s01-i3-git-permission-and-repo-write-authorization-v1.md §3.1
--（六能力 read/modify/commit/push/create_pr/merge）+ §3.2 默认最小权限 + §3.3 依赖链
--（modify←read、commit←modify、push←commit；merge 独立授权）。
-- 判定层（交集 + 依赖闭包）在应用层：crates/pm-domain/src/git_capability.rs。
--
-- B-6 方案 A：`task_run` 新增 `policy_snapshot_hash TEXT`（I3 §5.4 SHA-256 内容
-- 寻址的 Run 授权快照锚点，I3-D 填充），保留既有 INT `policy_version` 作序号。
--
-- 无回填（AGENTS.md 规则 1）：授权行由本迁移之后的 API 显式授予；
-- 无授权 = 全部拒绝（默认最小权限，I3 §3.2）。

-- ── 1. Run 授权快照锚点（B-6 方案 A）────────────────────────────────
ALTER TABLE delivery.task_run ADD COLUMN policy_snapshot_hash TEXT;

-- ── 2. 仓库级 Git 能力授权（I3 §2.3 第二层：Project/Repository 策略）────
-- 授权主体：`core.principal`（真人或 Agent 执行主体均可被授权）；
-- 授权人：走管理 API 的项目 owner / 租户 admin（服务端校验）。
-- 撤销 = 删行（即时生效：判定层每次请求实时查库）；审计走 governance.audit_log。
CREATE TABLE integration.git_capability_grant (
    tenant_id     UUID NOT NULL,
    project_id    UUID NOT NULL,
    repository_id UUID NOT NULL,
    principal_id  UUID NOT NULL REFERENCES core.principal(id),
    capabilities  TEXT[] NOT NULL DEFAULT '{}',
    granted_by    UUID REFERENCES core.principal(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, project_id, repository_id, principal_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, repository_id)
        REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE CASCADE,
    CHECK (capabilities <@ ARRAY['read','modify','commit','push','create_pr','merge']::text[])
);

CREATE INDEX idx_git_capability_grant_principal
    ON integration.git_capability_grant (tenant_id, principal_id);
