-- ============================================================
-- T-088：开发环境目录域（devenv）
--
-- 上位：docs/ai-pm-t088-bootstrap-server-v1.md V1.1 §7.1
-- 前置：0026_delivery_review.sql 已应用；本迁移为 forward-only。
--
-- 跨租户完整性（P0-1）：
--   `core.principal` 是 ADR-0001 §3.2 的**全局共享表，没有 tenant_id**，
--   租户归属由 `core.principal_identity` / `core.tenant_member` 表达。
--   因此「principal ∈ tenant」在库层的唯一既有可引用键是
--   `core.tenant_member` 的主键 (tenant_id, principal_id)——本迁移直接引用它，
--   **不修改任何核心表**。
--
-- 生命周期（status）与就绪快照（readiness）**正交两列**：
--   status    = 能否被接受为「有效登记设备」（active / revoked）
--   readiness = 最近一次自检的结论（设备自身就绪，与调用人无关）
-- ============================================================

CREATE SCHEMA IF NOT EXISTS devenv;

-- ── 设备目录 ───────────────────────────────────────────────
-- id 由**客户端生成**并持久化在本地 device.json：主键冲突即并发登记的天然闸门。
CREATE TABLE devenv.device (
    id                  UUID PRIMARY KEY,               -- 客户端生成的 UUID
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    owner_principal_id  UUID NOT NULL,
    name                TEXT NOT NULL,
    platform            TEXT NOT NULL,          -- 'windows' | 'macos' | 'linux'
    os_version          TEXT,
    executor_kind       TEXT NOT NULL DEFAULT 'codex',
    executor_version    TEXT,
    -- 生命周期：能否被接受为「有效登记设备」。V1 只有 active / revoked。
    status              TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active','revoked')),
    -- 就绪快照：最近一次自检的结论（六层取值见设计稿 §7.3）。
    readiness           TEXT
                        CHECK (readiness IN ('installed','connected','authenticated',
                                             'ready','degraded','failed')),
    last_check_at       TIMESTAMPTZ,
    last_check          JSONB,
    revoked_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    -- ★ 跨租户完整性 #1：设备 owner 必须属于该租户。
    --   组合 FK → core.tenant_member 的 PK；删行语义 NO ACTION（不 CASCADE）：
    --   移除租户成员前必须先撤销其设备（设计稿 M2）。
    CONSTRAINT fk_device_tenant_owner
        FOREIGN KEY (tenant_id, owner_principal_id)
        REFERENCES core.tenant_member (tenant_id, principal_id)
);

CREATE INDEX idx_device_owner ON devenv.device (tenant_id, owner_principal_id);
CREATE INDEX idx_device_tenant ON devenv.device (tenant_id, created_at DESC);

-- ── 设备 ↔ 项目绑定 ─────────────────────────────────────────
-- Binding 是**声明与事实记录**，不是权限授予机制（评审 P0-2）。
CREATE TABLE devenv.device_project_binding (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id               UUID NOT NULL,
    device_id               UUID NOT NULL,
    project_id              UUID NOT NULL,
    -- 用户声明的仓库 id 集（⊆ 该项目的 integration.project_repository.id）
    declared_repository_ids JSONB NOT NULL DEFAULT '[]',
    -- 服务端已确认的仓库身份与项目归属（⊆ declared）。
    -- ★ 不代表服务端验证过远程机器上的本地工作目录或 Git 工作区可用。
    verified_repositories    JSONB NOT NULL DEFAULT '[]',
    bound_by                UUID NOT NULL,
    bound_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    unbound_at              TIMESTAMPTZ,       -- 软删除：解绑后行保留，供审计与历史
    FOREIGN KEY (tenant_id, device_id) REFERENCES devenv.device (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- ★ 跨租户完整性 #3：绑定人必须属于该租户
    CONSTRAINT fk_binding_tenant_bounder
        FOREIGN KEY (tenant_id, bound_by)
        REFERENCES core.tenant_member (tenant_id, principal_id),
    UNIQUE (tenant_id, id),
    UNIQUE (device_id, project_id)             -- 幂等：重复绑定不产生重复行
);

CREATE INDEX idx_binding_device ON devenv.device_project_binding (device_id);
CREATE INDEX idx_binding_project ON devenv.device_project_binding (project_id);

-- ── 审计：真人身份与设备声明分离（评审 P0-1b）────────────────
-- actor_client / actor_auth_mode 已在 0018 / 0019 存在，本迁移只加设备声明列；
-- 与 T-059 的 task_run.execution_mode 分属不同表，不重叠。
ALTER TABLE governance.audit_log
    ADD COLUMN IF NOT EXISTS claimed_device_id UUID REFERENCES devenv.device(id);
CREATE INDEX idx_audit_claimed_device ON governance.audit_log (claimed_device_id)
    WHERE claimed_device_id IS NOT NULL;

ALTER TABLE governance.tenant_audit_log
    ADD COLUMN IF NOT EXISTS claimed_device_id UUID REFERENCES devenv.device(id);
CREATE INDEX idx_tenant_audit_claimed_device ON governance.tenant_audit_log (claimed_device_id)
    WHERE claimed_device_id IS NOT NULL;
