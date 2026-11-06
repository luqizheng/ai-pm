-- 0040_s02_i6a_approver_grant: S02-I6-A 权限基座——Approver 显式授权载体（O-6 裁决落地）。
--
-- O-6 裁决（2026-11-06，用户授权实施裁决）：
--   载体 = core.approver_grant 单行状态表：每 (project, principal) 一行，
--   revoked_at 置空即生效；重复授予 = 复活行（granted_by/granted_at 刷新），
--   历史走 audit_log（approver.granted / approver.revoked，S02D-23 点号式 I6-A 扩展）。
--   0032/0033 先例（建表即撤）的教训=表形态未定不进库——本表最小面：
--   无权限码联动、无有效期、无审批流，唯一消费方=pm-application::requirement
--   的确认/治理闸。
--
-- 授权矩阵（O-6 配套裁决，Q-A1「Lead+显式 Approver」口径）：
--   confirm/reject/withdraw/restore/archive
--     = 真人主体 + 项目成员 + (Lead(role='owner') OR 本表生效行)
--   grant/revoke/list = Lead 专属；被授人须为在职成员且非现任 Lead
--     （Lead 本身已持权，授予行仅服务于「非 Lead 的显式 Approver」）。
--   偏差注记：S02-04 §7.4 职责表的严格读法（Lead 亦须显式授权）不采纳——
--   会导致新项目无人可确认的冷启动死锁；Q-A1 原文「Lead+显式 Approver」
--   取宽读。step-up 确认票据（A* 补齐路径，H-17/H-18 门禁）另列后续项，
--   不在本表范围。
--
-- 归因列随 project_member 同风格用硬 FK（0001 先例）；tenant 复合 FK 对齐 0019。

CREATE TABLE core.approver_grant (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    granted_by UUID NOT NULL REFERENCES core.principal(id),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    revoked_by UUID REFERENCES core.principal(id),
    tenant_id UUID NOT NULL,
    UNIQUE (project_id, principal_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id)
);

CREATE INDEX approver_grant_active_idx
    ON core.approver_grant (tenant_id, project_id)
    WHERE revoked_at IS NULL;
