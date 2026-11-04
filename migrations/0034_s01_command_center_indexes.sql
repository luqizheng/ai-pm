-- 0034_s01_command_center_indexes: S01 · I5-C 项目总控台读模型所需索引。
--
-- 依据：docs/s01-i5-code-facts.md（I5-A 核验）
--   §8.2 「governance.audit_log 全仓只有 idx_audit_claimed_device 一个索引——
--          没有 (project_id, created_at) 索引」→ 缺口 I5-G-21
--   §4.2 「discovery_finding 的索引只有 idx_discovery_finding_session
--          (tenant_id, session_id, kind)，没有 project 级索引」→ 缺口 I5-G-22
--
-- 用途：
--   1. 「最近活动」块按项目取最近 N 条审计（ORDER BY created_at DESC, id DESC）
--      与其 count(*)——此前走 seq scan。
--   2. 「待人工处理」块按项目取 PENDING 的 Finding（project_id 维度聚合）。
--
-- 无回填（AGENTS.md 规则 1：纯索引迁移，无数据变更）。

-- ── 1. 审计：项目维度 + 时间倒序（活动流的读取路径）─────────────────────
CREATE INDEX IF NOT EXISTS idx_audit_log_project_time
    ON governance.audit_log (tenant_id, project_id, created_at DESC, id DESC);

-- ── 2. 发现：项目维度索引（project 级 Finding 聚合）────────────────────
CREATE INDEX IF NOT EXISTS idx_discovery_finding_project
    ON discovery.discovery_finding (tenant_id, project_id, verification_status, created_at DESC);
