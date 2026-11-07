-- ACP-09 E2E 确定性起点：cpk 凭据 + 已领取 Run（稳定身份 client:{device}）
-- 已知测试令牌：e2e lease_token = "e2e0f1e2d3c4b5a69788796a5b4c3d2e1f0ab"（64hex）
-- 其 sha256 由驱动侧计算后写入。
DELETE FROM delivery.worker_command WHERE run_id IN (SELECT id FROM delivery.task_run WHERE task_id='44444444-4444-4444-4444-444444444444');
DELETE FROM delivery.interaction_response_delivery WHERE run_id IN (SELECT id FROM delivery.task_run WHERE task_id='44444444-4444-4444-4444-444444444444');
DELETE FROM delivery.continuation WHERE run_id IN (SELECT id FROM delivery.task_run WHERE task_id='44444444-4444-4444-4444-444444444444');
DELETE FROM delivery.task_checkpoint WHERE run_id IN (SELECT id FROM delivery.task_run WHERE task_id='44444444-4444-4444-4444-444444444444');
DELETE FROM delivery.task_run_execution_authorization WHERE run_id IN (SELECT id FROM delivery.task_run WHERE task_id='44444444-4444-4444-4444-444444444444');
DELETE FROM delivery.task_waiting WHERE task_id='44444444-4444-4444-4444-444444444444';
DELETE FROM delivery.interaction_response WHERE interaction_id IN (SELECT id FROM delivery.task_interaction WHERE task_id='44444444-4444-4444-4444-444444444444');
DELETE FROM delivery.task_interaction WHERE task_id='44444444-4444-4444-4444-444444444444';
DELETE FROM delivery.task_run WHERE task_id='44444444-4444-4444-4444-444444444444';
UPDATE delivery.task SET status='claimed' WHERE id='44444444-4444-4444-4444-444444444444';
INSERT INTO delivery.task_run
  (project_id, task_id, attempt_no, status, run_kind, execution_mode, stage,
   lease_owner, lease_token_hash, lease_generation, lease_client_id,
   lease_expires_at, deadline_at, next_dispatch_at, heartbeat_at, tenant_id)
VALUES
  ('33333333-3333-3333-3333-333333333333','44444444-4444-4444-4444-444444444444',1,
   'claimed','delivery','spawn','started','client:66666666-6666-6666-6666-666666666666',
   '@@token_hash',1,'66666666-6666-6666-6666-666666666666',
   now()+interval '300 seconds', now()+interval '3600 seconds', now(), now(),
   'df894d40-73f8-4a2c-9e86-fe69874be30d');
UPDATE devenv.device
  SET credential_key_hash='@@cpk_hash', credential_issued_at=now(),
      credential_revoked_at=NULL,
      granted_permissions=ARRAY['PM_MCP_READ','PM_MCP_AGENT_REPORT','PM_MCP_COMMAND']
  WHERE id='66666666-6666-6666-6666-666666666666';
