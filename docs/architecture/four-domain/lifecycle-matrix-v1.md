# 对象生命周期与状态机矩阵 V1.0（审计产出）

> commit `05ce33d8` · 2026-11-15 · 只读审计产物
> 守卫分级：DB-CHECK / DB-TRIGGER / APP-CAS（乐观锁）/ APP-LOCK（行锁/advisory） / HUMAN（人工主体门）
> 大小写混用标注（⚡）：同一系统内状态词两套大小写并存，消费端需精确匹配。

## 1. KB 域

| 对象 | 状态字段 | 合法状态 | 转换入口（Service） | DB 守卫 | 人工门 | 状态权威 |
|---|---|---|---|---|---|---|
| kb.document_revision | status | draft → published / rejected（0023 扩展） | ingest 建行；`confirm_revision`（document.rs:60）→published；`close_report` 连带 →rejected（analysis_report.rs:481） | CHECK（BL:2893）+ trg_kb_revision_guard（0006：白名单单向、内容列冻结、DELETE 拒） | **HUMAN**：interactive+require_human+成员（document.rs:70-117） | 应用层 CAS（:199）+DB 白名单 |
| kb.memory_revision | review_status / version_status | PENDING_REVIEW→APPROVED/REJECTED/REVISION_REQUESTED⚡；version: NULL→CURRENT/WITHDRAWN→SUPERSEDED⚡ | `memory::review` 唯一写点（memory.rs:685/703/756） | guard_memory_revision（BL:228-276：INSERT 恒 PENDING、内容冻结、结论一次性、版本白名单）+13 条 CHECK | **HUMAN**：require_human（:629）+ K4 CAS（:662-681）+ 行锁（:421） | 双权威：DB 触发器白名单兜底 |
| kb.document_index_state | status | pending/indexed/stale/failed | indexer.rs 单一原语（:97-213） | CHECK | 无（派生） | 应用层 + 家族 advisory lock（:188） |
| ai.document_proposal（旧栈） | status | draft→quality_gates_passed→redline_pending→confirmed/commit_pending→committed/rejected/rolled_back（8 态） | submit.rs:43 / confirm_redline:290 / veto.rs:106 | 仅 CHECK（BL:893），**无触发器** | HUMAN：require_human（submit.rs:298） | 应用层（幂等账本 claim_operation:164） |
| ai.document_revision（旧栈） | status | draft/published/rejected/rolled_back | create_document / finalize_committed / revert_commit | 仅 CHECK（BL:929） | HUMAN（confirm） | 应用层 + Git（正文权威） |

## 2. ANALYSIS 域

| 对象 | 状态字段 | 合法状态 | 转换入口 | DB 守卫 | 人工门 | 状态权威 |
|---|---|---|---|---|---|---|
| ai.analysis_run | status | queued/analyzing/completed/failed（pm-ai）＋ **succeeded**（导入链）⚡ | start_run/context.rs:80-165（幂等复用+attempt+1）、finish_run:291、fail_run:312；导入链直插终态 proposal.rs:312 | **无 CHECK/无触发器/无乐观锁** | 无 | **应用层独占（分叉）** |
| integration.repo_analysis_report | status | analyzing→analyzed→closed；→failed；cancelled（**无写入者**）⚡小写 | ensure_for_intake:121（幂等 ON CONFLICT intake_id）/ mark_analyzed:222 / mark_failed:253 / close_report:387 | CHECK（0023）+ 部分索引 open-report 唯一 | **HUMAN**：close 门 ensure_closable（:552 interactive）；再分析 409 report_open（repo.rs:824） | 应用层（WHERE+FOR UPDATE） |
| integration.repo_import | status | queued/cloning/reading/analyzing/draft_ready/proposed/applied/rejected/failed/cancelled（10 态） | lifecycle.rs:197/456/738/1003、proposal.rs:374、advance.rs:54/65/99、archive.rs:118 | CHECK（BL:2774） | **HUMAN**：提案审批（governance.approval_request）+ apply | 应用层 |
| delivery.task_run | status/outcome | queued/claimed/running/succeeded/failed/timed_out/cancelled；outcome 4 值 | run_executor.rs:84/169/729/780 | CHECK（BL:1959）+ 租约 fencing（lease_generation） | HUMAN：review::decide | 应用层 |
| discovery.discovery_finding | verification_status | PENDING/CONFIRMED/REJECTED/CONFLICT⚡ | set_finding_verification_status_tx:672（唯一写点） | CHECK（BL:2360） | HUMAN：discovery/web.rs:119 record_finding_verification | 应用层 |
| design.review_finding | status | open→acknowledged→resolved | design/finding.rs:134（合法转换表 :125-131） | 无 CHECK（应用层白名单） | HUMAN（修订确认链） | 应用层 |
| ai.change_proposal | status | proposed/executed/redline_pending/escalated/vetoed/expired/failed（7 态） | orchestrator.rs:240（proposed）/ 导入链 proposal.rs:337（**直接 redline_pending**，双起点⚡） / approval.rs:97-120（executed） | CHECK（BL:742 区段） | HUMAN：审批请求 | 应用层 |

## 3. PROJECT_COGNITION 域

| 对象 | 状态字段 | 合法状态 | 转换入口 | DB 守卫 | 人工门 | 状态权威 |
|---|---|---|---|---|---|---|
| ai.cognition_run | status | pending/collecting/analyzing/completed/partially_completed/failed/cancelled（7 态；代码实际仅 completed 落地） | start_run（repo_cognition.rs:161-209） | CHECK（BL:808） | 无 | 应用层 |
| ai.cognition_item | state | proposed→confirmed/rejected/investigation/deferred/superseded（6 态；**superseded 零写入**；实装 4 转移） | decide_item（repo_cognition.rs:279-293，FOR UPDATE :229，非 proposed 409 :241） | CHECK（BL:788） | **名义人工、实际仅成员门**（pm_cognition.rs:279-318 未调 require_human/可信上下文）→ P0 | 应用层 |
| ai.project_knowledge | status | active/superseded/retired（**仅 active 实际出现**；insert-only 无 UPDATE） | 确认事务内 INSERT（repo_cognition.rs:263-276） | CHECK（BL:1113）+ confirmed_by NOT NULL | 同上（承继 decision 端点的弱门） | 应用层 |

## 4. PC 域

| 对象 | 状态字段 | 合法状态 | 转换入口 | DB 守卫 | 人工门 | 状态权威 |
|---|---|---|---|---|---|---|
| pc.proposal | status | draft→submitted→approved/rejected→applied/failed；→withdrawn/expired（8 态） | create:644 / submit:809 / record_decision:1063（允许 approved/rejected 互转=审阅流 v2）/ withdraw:1181 / publish 失败置 failed:277 / applied:597 | CHECK（BL:3263）+ trg_pc_proposal_anchors（非 draft 锚列冻结）+ 成员锚定约束触发器 | **HUMAN**：approve/reject 需 ensure_formal_authorized（proposal.rs:916） | 应用层 CAS（expected_version_no）+ FOR UPDATE |
| pc.approval | status | pending→approved/rejected | proposal.rs:819/1000/1047 | CHECK+trg_pc_approval_immutable（终态不可改、decided 必填） | 承继提案门 | 应用层 |
| pc.cognition_object | lifecycle_status | identity→draft→active→deprecated/superseded | upsert draft（object.rs）；active/deprecated 仅发布事务（publish.rs:628/644） | CHECK+trg_pc_object_lifecycle+identity 不可删 | 承继发布门 | 应用层+触发器 |
| pc.cognition_version | status | published/archived | 仅 publish.rs:455 | CHECK+双重触发器（不可变/不可删） | **HUMAN**：ensure_formal_authorized（publish.rs:100） | 发布事务（advisory+行锁+CAS 三层） |
| pc.write_operation | status | pending→completed；**git_committed 中间态已定义未启用**；failed | publish.rs:112/287/611 | CHECK（BL:3470）+ trg_pc_write_op_state（BL:458-467 允许 pending→git_committed→completed） | 承继 | 发布事务+幂等重放（:111-176） |
| pc.conflict | status | detected/under_review/awaiting_human/resolved/dismissed/reopened（6 态） | **无生产写入者/无 resolve 入口** | CHECK（BL:3122 区段） | 设计上应有人工裁决（缺失） | 空转（桩） |

## 5. 跨对象状态机要点

1. **大小写两套词表并存（⚡）**：大写系 = kb.memory_revision、discovery_finding/session、integration.intake_*；小写系 = kb.document_revision、pc.*、delivery/design、ai.change_proposal。统一 Finding 契约（D03/U05）落地时必须先归一。
2. **「审阅流 v2」特殊语义**：pc.proposal 的 approved/rejected 可互转（暂记可改期），终态性由 report close 批量发布收口——与「审批一次定型」的直觉不同，引用时须带 expected_version_no CAS。
3. **报告门三连**：analyze 409 report_open（再分析阻断）→ PC decide 409（报告 open 期提案冻结 pc/proposal.rs:921-933）→ close 单事务批量发布（analysis_report.rs:387-547）。报告状态是仓库分析链的总闸。
4. **死状态/死列清单**（schema 就绪但不可达）：repo_analysis_report.cancelled、cognition_item.superseded、project_knowledge.superseded/retired、analysis_run.succeeded-vs-completed 词表分叉、pc.write_operation.git_committed、kb.memory_revision.version_status=MERGED（CHECK 允许、无写入者）。
5. **一次性决策约束**：cognition_item 与 kb.memory_revision 均为「已决即 409」单向门；pc.proposal 反而允许暂记改期——三种决策定 型语义并存（对应 U04/U06 裁决时需统一词汇）。
