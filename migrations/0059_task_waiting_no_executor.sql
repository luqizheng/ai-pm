-- 0059_task_waiting_no_executor: D-2=A 调度自动写 assignee 的等待侧落点
-- （裁决 docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md §〇/§三）。
--
-- 内容：delivery.task_waiting.reason CHECK 扩第五值 `no_executor_available`。
-- 0010:94-95 原四值（needs_info/needs_decision/needs_approval/external_dependency）
-- 的列级 CHECK 由 Postgres 自动命名 task_waiting_reason_check，扩值 = DROP + 重建。
--
-- 语义（裁决 §三「无合适执行者时」）：TaskReadyHandler 自动解析 Role 承接人失败
-- （role 未启用 / 项目无合格 dev_agent 成员）≠ 失败——写一条活跃等待行（reason=
-- no_executor_available），让 claimability 门③（task_waiting，policy.rs）持续 deny，
-- 等待恢复。恢复条件：后续 enable 角色 / 新增合格成员后重新触发 TaskReady，或人工
-- 介入。V1 不做自动重试器（TaskReadyHandler 头注与审计 payload 均写明）。
-- 严禁做成状态（稿 §12.2）：Task 状态机零改动，只是等待原因的枚举扩值。
--
-- questions/since/deadline 等其余列与约束照旧（默认 '[]'；应用层写入口始终带
-- 非空 questions）；活跃等待行唯一性仍由 uq_task_waiting_active 部分唯一索引兜底。

ALTER TABLE delivery.task_waiting DROP CONSTRAINT task_waiting_reason_check;

ALTER TABLE delivery.task_waiting ADD CONSTRAINT task_waiting_reason_check
    CHECK (reason IN (
        'needs_info', 'needs_decision', 'needs_approval', 'external_dependency',
        'no_executor_available'));
