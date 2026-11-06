-- =====================================================================
-- ai-pm 演示种子（Slice A 联调）—— 幂等，可重复执行。
--
-- 用法（本编排 aipm-pg 容器）：
--   docker exec -i infrastructure-aipm-pg-1 psql -U aipm -d aipm < scripts/seed-demo.sql
--   （旧 dev 容器：docker exec -i aipm-pg-dev psql -U aipm -d aipm < scripts/seed-demo.sql）
--
-- 租户维度（2026-11-03）：所有业务表 tenant_id NOT NULL，统一取
--   (provider='lmms', external_key='1') —— 即 JWT `tid` 对应的租户；
--   pm-api 首次带 token 请求时会自动 ensure_tenant 创建它（不存在则本脚本先建）。
--   种子里的三个 principal 另加入该租户；**当前登录主体**（active human）
--   自动补为 DEMO 项目 owner，使审计/任务归属与登录用户一致。
--
-- 数据形态对齐前端经典页契约（frontend/src/api/mock/fixtures.ts）：
-- 1 个客户 + 1 个 DEMO 项目 + 3 个 principal（human/ai_pm/dev_agent）+
-- 3 条需求（含 approved 定稿链）+ 6 条任务（多状态）+ 1 条 pending 审批
-- （完整链 event→run→proposal→approval）+ 若干审计行。
-- 固定 UUID + ON CONFLICT DO NOTHING，重复执行不产生重复数据。
-- =====================================================================

BEGIN;

-- 0) 租户（幂等；pm-api 也会按 JWT tid 自动创建） -----------------------------
INSERT INTO core.tenant (provider, external_key, display_name) VALUES
  ('lmms', '1', '演示租户')
ON CONFLICT (provider, external_key) DO NOTHING;

-- 1) 客户与项目 ------------------------------------------------------------
INSERT INTO core.customer (id, tenant_id, code, name, short_name, industry, status, created_at) VALUES
  ('11111111-1111-1111-1111-111111111111',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   'DUNPAI', '盾牌科技（演示）', '盾牌科技', '软件', 'active', now() - interval '30 days')
ON CONFLICT DO NOTHING;

INSERT INTO core.project (id, tenant_id, customer_id, code, name, description, status, revision, created_at, updated_at) VALUES
  ('22222222-2222-2222-2222-222222222222',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '11111111-1111-1111-1111-111111111111',
   'AIDEMO', '盾牌总控台（联调演示）', 'AI-PM 联合开发演示项目', 'active', 1,
   now() - interval '30 days', now() - interval '1 day')
ON CONFLICT DO NOTHING;

-- 2) 操作者 principals + 成员 ---------------------------------------------
INSERT INTO core.principal (id, kind, display_name, is_active, created_at) VALUES
  ('33333333-3333-3333-3333-333333333331', 'human',    '陆知远（负责人）', true, now() - interval '30 days'),
  ('33333333-3333-3333-3333-333333333332', 'ai_pm',    'AI-PM 助理',       true, now() - interval '30 days'),
  ('33333333-3333-3333-3333-333333333333', 'dev_agent', '盾牌开发智能体',   true, now() - interval '30 days')
ON CONFLICT DO NOTHING;

INSERT INTO core.project_member (project_id, principal_id, role, tenant_id) VALUES
  ('22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333331', 'owner',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1')),
  ('22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333332', 'ai_pm',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1')),
  ('22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333333', 'developer',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'))
ON CONFLICT (project_id, principal_id) DO NOTHING;

-- 2b) 租户成员（tenant_member.role ∈ owner/admin/member）--------------------
INSERT INTO core.tenant_member (tenant_id, principal_id, role) VALUES
  ((SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '33333333-3333-3333-3333-333333333331', 'owner'),
  ((SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '33333333-3333-3333-3333-333333333332', 'member'),
  ((SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '33333333-3333-3333-3333-333333333333', 'member')
ON CONFLICT (tenant_id, principal_id) DO NOTHING;

-- 2c) 当前登录主体（pm-api 自动 provision 的 active human）补为 DEMO 项目 owner
INSERT INTO core.project_member (project_id, principal_id, role, tenant_id)
SELECT '22222222-2222-2222-2222-222222222222', p.id, 'owner', pi.tenant_id
  FROM core.principal p
  JOIN core.principal_identity pi ON pi.principal_id = p.id
  JOIN core.tenant t ON t.id = pi.tenant_id AND t.provider = 'lmms' AND t.external_key = '1'
 WHERE p.kind = 'human' AND p.is_active
ON CONFLICT (project_id, principal_id) DO NOTHING;

-- 2d) 当前登录主体同时补为租户 owner（全平台管理员语义：租户级角色取库内
--     core.tenant_member.role —— pm-api 不读 token 的 roles，见 auth.rs D5）
INSERT INTO core.tenant_member (tenant_id, principal_id, role)
SELECT pi.tenant_id, p.id, 'owner'
  FROM core.principal p
  JOIN core.principal_identity pi ON pi.principal_id = p.id
 WHERE p.kind = 'human' AND p.is_active
ON CONFLICT (tenant_id, principal_id) DO NOTHING;

-- 3) 需求与修订（requirement 指针留空先插 → 修订 → 回填，绕开循环 FK）--------
INSERT INTO delivery.requirement (id, tenant_id, project_id, code, title, current_revision_id, row_revision, created_at) VALUES
  ('66666666-6666-6666-6666-666666666661',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', 'REQ-101', '登录会话与刷新', NULL, 1, now() - interval '20 days'),
  ('66666666-6666-6666-6666-666666666662',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', 'REQ-102', '任务看板与流转', NULL, 1, now() - interval '15 days'),
  ('66666666-6666-6666-6666-666666666663',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', 'REQ-103', '审批红线确认', NULL, 1, now() - interval '10 days')
ON CONFLICT DO NOTHING;

INSERT INTO delivery.requirement_revision (id, tenant_id, requirement_id, project_id, version, content, state, content_hash, created_by, approved_by, created_at) VALUES
  ('77777777-7777-7777-7777-777777777711',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '66666666-6666-6666-6666-666666666661', '22222222-2222-2222-2222-222222222222', 1,
   '{"markdown": "# 登录会话 v1\n- 账号密码登录\n- access 15min"}', 'superseded', 'seed-hash-v1',
   '33333333-3333-3333-3333-333333333331', NULL, now() - interval '20 days'),
  ('77777777-7777-7777-7777-777777777712',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '66666666-6666-6666-6666-666666666661', '22222222-2222-2222-2222-222222222222', 2,
   '{"markdown": "# 登录会话 v2\n- 账号密码登录\n- access 15min / refresh 30d\n- 网关 jti 黑名单"}', 'approved', 'seed-hash-v2',
   '33333333-3333-3333-3333-333333333331', '33333333-3333-3333-3333-333333333331', now() - interval '18 days'),
  ('77777777-7777-7777-7777-777777777721',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '66666666-6666-6666-6666-666666666662', '22222222-2222-2222-2222-222222222222', 1,
   '{"markdown": "# 任务看板 v1\n- 六态流转\n- 乐观锁编辑"}', 'draft', 'seed-hash-v3',
   '33333333-3333-3333-3333-333333333331', NULL, now() - interval '15 days'),
  ('77777777-7777-7777-7777-777777777731',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '66666666-6666-6666-6666-666666666663', '22222222-2222-2222-2222-222222222222', 1,
   '{"markdown": "# 审批红线 v1\n- 红线命令必须人工确认\n- 同意即执行提案命令"}', 'draft', 'seed-hash-v4',
   '33333333-3333-3333-3333-333333333331', NULL, now() - interval '10 days')
ON CONFLICT DO NOTHING;

-- 4) 任务（多状态多优先级） --------------------------------------------------
INSERT INTO delivery.task (id, tenant_id, project_id, milestone_id, title, description, status, priority, assignee_id, revision, created_at, updated_at) VALUES
  ('88888888-8888-8888-8888-888888888801',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', NULL, '打通网关登录链路',   'gateway→iam→token 全链',     'completed',   'high',   '33333333-3333-3333-3333-333333333331', 3, now() - interval '9 days',  now() - interval '2 days'),
  ('88888888-8888-8888-8888-888888888802',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', NULL, 'pm-api JWKS 验签',   'Bearer 中间件 + tid 强制',   'reviewing',   'high',   '33333333-3333-3333-3333-333333333333', 2, now() - interval '7 days',  now() - interval '1 day'),
  ('88888888-8888-8888-8888-888888888803',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', NULL, '/v1/pm 列表端点',    '六组裸 payload 列表',        'running',     'urgent', '33333333-3333-3333-3333-333333333333', 1, now() - interval '5 days',  now() - interval '5 hours'),
  ('88888888-8888-8888-8888-888888888804',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', NULL, '任务编辑乐观锁',     'revision 不匹配 409',        'draft',       'normal', NULL,                                   1, now() - interval '4 days',  now() - interval '4 days'),
  ('88888888-8888-8888-8888-888888888805',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', NULL, 'SSE 审计实时流',     'overview 实时面板',          'blocked',     'normal', NULL,                                   1, now() - interval '3 days',  now() - interval '3 days'),
  ('88888888-8888-8888-8888-888888888806',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', NULL, '演示种子脚本',       '本脚本',                     'completed',   'low',    '33333333-3333-3333-3333-333333333331', 2, now() - interval '2 days',  now() - interval '6 hours')
ON CONFLICT DO NOTHING;

-- 5) 待审批链（event → run → proposal → pending approval） -------------------
INSERT INTO integration.inbound_event (id, tenant_id, project_id, actor_id, source, event_type, idempotency_key, payload, payload_hash, received_at) VALUES
  ('99999999-9999-9999-9999-999999999901',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222',
   '33333333-3333-3333-3333-333333333331', 'feishu', 'bug_report', 'seed-demo-1', '{"text": "看板拖拽偶发丢序"}', 'seed-hash', now() - interval '8 hours')
ON CONFLICT DO NOTHING;

INSERT INTO ai.analysis_run (id, tenant_id, project_id, event_id, status, model_name, policy_version, config_snapshot, created_at) VALUES
  ('99999999-9999-9999-9999-999999999902',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222',
   '99999999-9999-9999-9999-999999999901', 'succeeded', 'fake-model', 1, '{}', now() - interval '8 hours')
ON CONFLICT DO NOTHING;

INSERT INTO ai.change_proposal (id, tenant_id, project_id, analysis_run_id, proposal_seq, target_type, target_id, expected_revision, command_type, proposed_data, rationale, evidence, risk_level, confidence, status, created_at) VALUES
  ('99999999-9999-9999-9999-999999999903',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222',
   '99999999-9999-9999-9999-999999999902', 0, 'task', '88888888-8888-8888-8888-888888888804', 1,
   'update_task_status',
   '{"kind": "update_task_status", "data": {"task_id": "88888888-8888-8888-8888-888888888804", "expected_revision": 1, "new_status": "running"}}',
   '看板丢序根因在状态未及时流转，建议将任务编辑乐观锁任务置为进行中', '[]', 'medium', 'medium', 'proposed',
   now() - interval '6 hours')
ON CONFLICT DO NOTHING;

INSERT INTO governance.approval_request (id, tenant_id, project_id, proposal_id, status, required_role, created_at) VALUES
  ('99999999-9999-9999-9999-999999999904',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222',
   '99999999-9999-9999-9999-999999999903', 'pending', 'project_owner', now() - interval '6 hours')
ON CONFLICT DO NOTHING;

-- 6) 审计行（历史感） --------------------------------------------------------
INSERT INTO governance.audit_log (id, tenant_id, project_id, actor_id, action, target_type, target_id, created_at) VALUES
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa1',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333331', 'task.create', 'task', '88888888-8888-8888-8888-888888888801', now() - interval '9 days'),
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa2',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333331', 'requirement.approve_revision', 'requirement', '66666666-6666-6666-6666-666666666661', now() - interval '18 days'),
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaa3',
   (SELECT id FROM core.tenant WHERE provider = 'lmms' AND external_key = '1'),
   '22222222-2222-2222-2222-222222222222', '33333333-3333-3333-3333-333333333333', 'command.execute', 'task', '88888888-8888-8888-8888-888888888801', now() - interval '2 days')
ON CONFLICT DO NOTHING;

-- 回填 REQ-101 定稿指针（绕开 requirement↔revision 循环 FK）
UPDATE delivery.requirement SET current_revision_id = '77777777-7777-7777-7777-777777777712', lifecycle_status = 'active', row_revision = 2
WHERE id = '66666666-6666-6666-6666-666666666661' AND current_revision_id IS NULL;

COMMIT;
