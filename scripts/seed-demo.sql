-- 演示数据：与前端 mock（frontend/src/api/mock/fixtures.ts）同源的样例，
-- 全部固定 UUID + ON CONFLICT DO NOTHING，可重复执行。
-- 应用：docker exec -i aipm-pg-dev psql -U aipm -d aipm < scripts/seed-demo.sql
-- 清理：TRUNCATE governance.audit_log, governance.approval_request, ai.change_proposal,
--       ai.analysis_run, integration.inbound_event, delivery.task, delivery.requirement,
--       delivery.milestone, core.project, core.principal, core.customer;

-- 客户
INSERT INTO core.customer (id, name, status) VALUES
  ('00000000-0000-0000-0000-000000000001', '星河集团', 'active'),
  ('00000000-0000-0000-0000-000000000002', '云帆科技', 'active')
ON CONFLICT (id) DO NOTHING;

-- 成员主体
INSERT INTO core.principal (id, kind, display_name, is_active) VALUES
  ('00000000-0000-0000-0000-000000000011', 'human', '张伟', true),
  ('00000000-0000-0000-0000-000000000012', 'human', '李娜', true),
  ('00000000-0000-0000-0000-000000000013', 'dev_agent', 'dev-agent-01', true),
  ('00000000-0000-0000-0000-000000000014', 'ai_pm', 'AI-PM', true)
ON CONFLICT (id) DO NOTHING;

-- 项目
INSERT INTO core.project (id, customer_id, code, name, description, status, revision, created_at, updated_at) VALUES
  ('00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000001', 'SHIELD', '星河客服中台', '面向星河集团的多渠道客服与工单中台。', 'active', 12, now() - interval '80 days', now() - interval '1 day'),
  ('00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000002', 'ATLAS', '云帆数据门户', '云帆科技统一数据门户与自助报表。', 'active', 7, now() - interval '50 days', now() - interval '2 days'),
  ('00000000-0000-0000-0000-000000000023', '00000000-0000-0000-0000-000000000001', 'HORIZON', '星河移动端', '客服中台配套的移动端 App，规划阶段。', 'planning', 2, now() - interval '10 days', now() - interval '3 days')
ON CONFLICT (id) DO NOTHING;

-- 里程碑
INSERT INTO delivery.milestone (id, project_id, name, planned_at, status) VALUES
  ('00000000-0000-0000-0000-000000000031', '00000000-0000-0000-0000-000000000021', 'M1 需求定版', now() + interval '14 days', 'planned'),
  ('00000000-0000-0000-0000-000000000032', '00000000-0000-0000-0000-000000000021', 'M2 内测发布', now() + interval '45 days', 'planned'),
  ('00000000-0000-0000-0000-000000000033', '00000000-0000-0000-0000-000000000022', 'M1 取数向导', now() + interval '30 days', 'planned')
ON CONFLICT (id) DO NOTHING;

-- 入站事件（analysis_run 的 FK 前置）
INSERT INTO integration.inbound_event (id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, status, received_at) VALUES
  ('00000000-0000-0000-0000-000000000041', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000013', 'dev_agent', 'analyze_request', 'seed-evt-41', '{}', 'seed', 'processed', now() - interval '3 days'),
  ('00000000-0000-0000-0000-000000000042', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000013', 'dev_agent', 'analyze_request', 'seed-evt-42', '{}', 'seed', 'processed', now() - interval '2 days')
ON CONFLICT (id) DO NOTHING;

-- 分析运行
INSERT INTO ai.analysis_run (id, project_id, event_id, status, model_name, policy_version, config_snapshot, finished_at, created_at) VALUES
  ('00000000-0000-0000-0000-000000000051', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000041', 'succeeded', 'glm-5.3-flash', 1, '{}', now() - interval '3 days', now() - interval '3 days'),
  ('00000000-0000-0000-0000-000000000052', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000042', 'succeeded', 'glm-5.3-flash', 1, '{}', now() - interval '2 days', now() - interval '2 days')
ON CONFLICT (id) DO NOTHING;

-- 变更提案
INSERT INTO ai.change_proposal (id, project_id, analysis_run_id, proposal_seq, target_type, target_id, command_type, proposed_data, rationale, risk_level, confidence, status, created_at) VALUES
  ('00000000-0000-0000-0000-000000000061', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000051', 1, 'requirement', '00000000-0000-0000-0000-000000000084', 'requirement.revise', '{}', 'SLA 告警阈值建议调整为 30 分钟。', 'medium', 'high', 'redline_pending', now() - interval '9 hours'),
  ('00000000-0000-0000-0000-000000000062', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000052', 1, 'requirement', '00000000-0000-0000-0000-000000000086', 'requirement.revise', '{}', '订阅分发建议补一个失败重试策略。', 'medium', 'medium', 'redline_pending', now() - interval '1 day'),
  ('00000000-0000-0000-0000-000000000063', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000051', 2, 'task', '00000000-0000-0000-0000-000000000092', 'task.reorder', '{}', '微信适配器优先级提升。', 'low', 'high', 'executed', now() - interval '2 days'),
  ('00000000-0000-0000-0000-000000000064', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000052', 2, 'release', NULL, 'release.publish', '{}', '门户 v0.3 发布窗口建议。', 'high', 'medium', 'vetoed', now() - interval '4 days')
ON CONFLICT (id) DO NOTHING;

-- 需求
INSERT INTO delivery.requirement (id, project_id, code, title, status, row_revision, created_at) VALUES
  ('00000000-0000-0000-0000-000000000081', '00000000-0000-0000-0000-000000000021', 'REQ-101', '工单聚合视图', 'approved', 3, now() - interval '40 days'),
  ('00000000-0000-0000-0000-000000000082', '00000000-0000-0000-0000-000000000021', 'REQ-102', '渠道接入标准化（微信/邮件）', 'approved', 1, now() - interval '35 days'),
  ('00000000-0000-0000-0000-000000000083', '00000000-0000-0000-0000-000000000021', 'REQ-103', '客服绩效报表', 'draft', 2, now() - interval '12 days'),
  ('00000000-0000-0000-0000-000000000084', '00000000-0000-0000-0000-000000000021', 'REQ-104', 'SLA 倒计时告警', 'draft', 1, now() - interval '6 days'),
  ('00000000-0000-0000-0000-000000000085', '00000000-0000-0000-0000-000000000022', 'REQ-201', '自助取数向导', 'approved', 1, now() - interval '30 days'),
  ('00000000-0000-0000-0000-000000000086', '00000000-0000-0000-0000-000000000022', 'REQ-202', '报表订阅与分发', 'draft', 1, now() - interval '9 days'),
  ('00000000-0000-0000-0000-000000000087', '00000000-0000-0000-0000-000000000022', 'REQ-203', '数据源健康巡检', 'superseded', 2, now() - interval '28 days')
ON CONFLICT (id) DO NOTHING;

-- 任务
INSERT INTO delivery.task (id, project_id, milestone_id, title, description, status, priority, assignee_id, revision, created_at, updated_at) VALUES
  ('00000000-0000-0000-0000-000000000091', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000031', '工单聚合视图 API 设计', NULL, 'done', 'high', '00000000-0000-0000-0000-000000000013', 3, now() - interval '20 days', now() - interval '4 days'),
  ('00000000-0000-0000-0000-000000000092', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000031', '微信渠道适配器实现', NULL, 'in_progress', 'high', '00000000-0000-0000-0000-000000000011', 2, now() - interval '10 days', now() - interval '1 day'),
  ('00000000-0000-0000-0000-000000000093', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000032', 'SLA 倒计时前端组件', NULL, 'backlog', 'normal', '00000000-0000-0000-0000-000000000012', 1, now() - interval '5 days', now() - interval '5 days'),
  ('00000000-0000-0000-0000-000000000094', '00000000-0000-0000-0000-000000000021', NULL, '绩效报表指标口径确认', '与业务方确认统计口径。', 'blocked', 'normal', '00000000-0000-0000-0000-000000000012', 4, now() - interval '8 days', now() - interval '2 days'),
  ('00000000-0000-0000-0000-000000000095', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000032', '渠道接入回归测试用例', NULL, 'backlog', 'low', NULL, 1, now() - interval '3 days', now() - interval '3 days'),
  ('00000000-0000-0000-0000-000000000096', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000033', '取数向导 DSL 设计', NULL, 'in_progress', 'urgent', '00000000-0000-0000-0000-000000000013', 2, now() - interval '14 days', now() - interval '1 day'),
  ('00000000-0000-0000-0000-000000000097', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000033', '报表订阅队列与投递', NULL, 'backlog', 'normal', NULL, 1, now() - interval '6 days', now() - interval '6 days'),
  ('00000000-0000-0000-0000-000000000098', '00000000-0000-0000-0000-000000000022', NULL, '数据源巡检任务拆分', NULL, 'cancelled', 'low', '00000000-0000-0000-0000-000000000011', 3, now() - interval '25 days', now() - interval '20 days')
ON CONFLICT (id) DO NOTHING;

-- 审批请求（红线确认队列）
INSERT INTO governance.approval_request (id, project_id, proposal_id, status, required_role, decided_by, decision_reason, decided_at, created_at) VALUES
  ('00000000-0000-0000-0000-000000000071', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000061', 'pending', 'project_owner', NULL, NULL, NULL, now() - interval '9 hours'),
  ('00000000-0000-0000-0000-000000000072', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000062', 'pending', 'project_owner', NULL, NULL, NULL, now() - interval '1 day'),
  ('00000000-0000-0000-0000-000000000073', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000063', 'approved', 'project_owner', '00000000-0000-0000-0000-000000000011', '影响面可控，同意执行。', now() - interval '2 days', now() - interval '2 days'),
  ('00000000-0000-0000-0000-000000000074', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000064', 'rejected', 'project_owner', '00000000-0000-0000-0000-000000000012', '发布窗口与客户活动冲突，驳回。', now() - interval '3 days', now() - interval '4 days')
ON CONFLICT (id) DO NOTHING;

-- 审计日志
INSERT INTO governance.audit_log (id, project_id, actor_id, action, target_type, target_id, created_at) VALUES
  ('00000000-0000-0000-0000-0000000000a1', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000013', 'requirement.propose_revision', 'requirement', '00000000-0000-0000-0000-000000000083', now() - interval '9 hours'),
  ('00000000-0000-0000-0000-0000000000a2', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000011', 'approval.decide', 'approval_request', '00000000-0000-0000-0000-000000000073', now() - interval '2 days'),
  ('00000000-0000-0000-0000-0000000000a3', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000014', 'command.execute', 'command', NULL, now() - interval '2 days'),
  ('00000000-0000-0000-0000-0000000000a4', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000012', 'approval.decide', 'approval_request', '00000000-0000-0000-0000-000000000074', now() - interval '3 days'),
  ('00000000-0000-0000-0000-0000000000a5', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000013', 'task.create', 'task', '00000000-0000-0000-0000-000000000097', now() - interval '6 days'),
  ('00000000-0000-0000-0000-0000000000a6', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000014', 'task.reorder', 'task', '00000000-0000-0000-0000-000000000093', now() - interval '4 days'),
  ('00000000-0000-0000-0000-0000000000a7', '00000000-0000-0000-0000-000000000021', '00000000-0000-0000-0000-000000000011', 'requirement.approve_revision', 'requirement', '00000000-0000-0000-0000-000000000082', now() - interval '30 days'),
  ('00000000-0000-0000-0000-0000000000a8', '00000000-0000-0000-0000-000000000022', '00000000-0000-0000-0000-000000000011', 'human_intervention.freeze', 'project', '00000000-0000-0000-0000-000000000022', now() - interval '7 days')
ON CONFLICT (id) DO NOTHING;
