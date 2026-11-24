-- 0059: R1（B03-01）对话主路径可靠性——编排轮失败对用户可见。
--
-- 缺陷（v1.0 业务验收 B03-01）：会话进入 REVIEW_REQUIRED 后编排器不再扫描，
-- 会话内新用户消息成为死信；且编排轮失败（无 Provider / 模型错误 / 输出不合规）
-- 只打进程日志，用户在 UI 上完全看不到任何异常。
--
-- 修复语义：
-- * 会话行持久化「最近一轮编排失败原因」，会话详情 API / 访谈面板直接展示；
--   成功一轮即清除。失败不改变状态机（状态跃迁仍只走 SESSION_EDGES 既有边），
--   编排器继续按既有冷却重试；用户侧恢复入口 = 既有人工跃迁
--   （REVIEW_REQUIRED/NEEDS_USER_INPUT → IN_PROGRESS / COMPLETED）。
-- * 死信主修复不在迁移层：路由落点回执携带访谈状态（SendOutcome.interview_status），
--   前端为 REVIEW_REQUIRED 提供人工复核动作（退回/完结）。

ALTER TABLE discovery.discovery_session
    ADD COLUMN last_round_error text,
    ADD COLUMN last_round_error_at timestamptz;
