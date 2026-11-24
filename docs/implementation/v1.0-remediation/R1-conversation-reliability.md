# R1 — 对话主路径可靠性（B03-01 修复）

- 日期：2026-11-24
- 分支：`remediation/r1-conversation-reliability`
- 对应缺陷：**B03-01（P1）** REVIEW_REQUIRED 会话内需求关键词消息永久死信（`docs/acceptance/v1.0-business/03-knowledge.md` §3）
- 基线：`00-baseline.md` §3 B03-01（根因四处断点已核实）

## 1. 根因（复述）

用户在 chat 发消息 → router 规则 1 把 REVIEW_REQUIRED 视为活跃（`session_is_active` 只排除 COMPLETED/PAUSED）→ 消息 201 落入访谈段 → 编排器 `scan_pending` 只扫 `IN_PROGRESS/NEEDS_USER_INPUT`（不扫 REVIEW_REQUIRED，设计如此：REVIEW_REQUIRED=等人工复核，编排器永不解除）→ chat 回复循环因「线程最新消息是 system 边界行」也看不到 → 前端无状态分支、无处理入口（`transitionDiscoverySession` 零调用）→ **消息被接收但永远无人消费，用户零感知**。

消息本身无状态列——「待处理」是派生态（最新消息 speaker=user + 会话在扫描集）。死信 = 消息落在扫描集之外的状态里。

## 2. 修复方案（不推翻既有安全语义）

**原则**：REVIEW_REQUIRED 的人工复核闸门**保留不变**（不把 REVIEW_REQUIRED 改成普通成功态；编排器仍不扫它；`writeback_sweep.rs` 钉死的前提用例原样保留）。修复的是「用户不可见、无入口、无恢复」三件事：

| # | 断点 | 修复 |
|---|---|---|
| 1 | 发送回执不携带访谈状态，前端无从感知 | `SendOutcome` 新增 `interview_status`（landed=interview 时必有）：路由锁会话行后如实带回当前状态 |
| 2 | 无人工处理入口 | AssistantView 访谈段 + DiscoveryView 会话面板新增处理块：REVIEW_REQUIRED → 「退回继续访谈」（→IN_PROGRESS）/「复核通过并完结」（→COMPLETED）；NEEDS_USER_INPUT → 「完结会话」（→COMPLETED）。走既有 REST `POST .../discovery/sessions/{sid}/status`（既有状态机边，无新边） |
| 3 | 失败不可见 | 新迁移 0059：`discovery_session.last_round_error/last_round_error_at`；编排器四种轮次失败（no_provider/model_error/db_error/invalid_output）落库，成功一轮即清除；会话详情/访谈面板展示 |
| 4 | 人工动作无审计 | REST status 端点改走 `transition_session_with_audit`：跃迁与 `governance.audit_log` 行（action=`discovery_session_transitioned`，after_data={from,to}）同事务 |

## 3. 修复后的消息生命周期

```
用户发送 → 路由判定落点
├─ chat：入线程，回复循环消费（原有）
├─ 访谈 IN_PROGRESS / NEEDS_USER_INPUT：入访谈段，编排器消费（原有）
└─ 访谈 REVIEW_REQUIRED：消息照常落库（记录不丢），回执带
   interview_status=REVIEW_REQUIRED → UI 显式提示「等待人工复核，AI 暂不处理」
   → 用户二选一：
     a) 退回 IN_PROGRESS → 编排器恢复扫描，消费该消息（既有谓词，零改动）
     b) 完结 COMPLETED → 线程脱链，后续消息回落 chat（既有 A6 语义）
```

每一步用户可见：落点状态在发送回执 + 访谈面板状态徽标 + 处理块；失败在 `last_round_error` 行；全部跃迁有审计行。

## 4. 变更清单

**后端**
- `migrations/0059_discovery_round_error.sql`：会话表新增失败标记两列
- `crates/pm-application/src/discovery/session.rs`：视图加 `last_round_error/at`；`record_round_error`（500 字符截断）/`clear_round_error`；`transition_session_with_audit`（跃迁+审计同事务）
- `crates/pm-application/src/conversation_router.rs`：`SendOutcome.interview_status`；`append_user_to_interview` 返回锁后会话状态
- `crates/pm-ai/src/discovery_orchestrator.rs`：四类失败落库、成功清除（尽力而为，不影响冷却/重试语义）
- `apps/pm-api/src/pm_discovery.rs`：status handler 改走带审计跃迁，actor=真实主体（含 auth_mode）

**前端**
- `src/api/conversations.ts`：`SendMessageOutcome.interview_status`
- `src/api/discovery.ts`：`DiscoverySession.last_round_error/at`
- `src/views/project/assistant/AssistantView.vue`：访谈段处理块（提示 + 退回/完结按钮 + 失败行 + 409 竞态重拉）
- `src/views/project/discovery/DiscoveryView.vue`：会话处理块（同上，动作后重载列表+详情）
- `locales/{zh-CN,en}/assistant.yaml`、`locales/{zh-CN,en}/s01/discovery.yaml`：6+7 个新 key（check:i18n 通过）

## 5. 测试

**新增自动化测试（全部通过）**
| 层 | 用例 | 覆盖 |
|---|---|---|
| pm-ai（lib r1 模块） | `round_failure_persists_and_success_clears` | 模型失败落库 `model_error:*`、状态机不推进、成功清除 |
| | `resumed_review_required_session_rejoins_the_scan_set` | REVIEW_REQUIRED 不在扫描集（前提不变）；人工退回后消息被消费 |
| | `round_error_is_truncated_to_a_bounded_length` | 5000 字符原因截断到 500 |
| pm-api（conversation_router） | `send_into_review_required_interview_reports_status` | 升级回执带 IN_PROGRESS；REVIEW_REQUIRED 期发送 201+回执如实；消息落库不丢；编排/chat 均不消费；人工退回恢复 |
| pm-api（discovery） | `human_transition_writes_audit_and_exposes_round_error_fields` | NEEDS_USER_INPUT→COMPLETED 审计行 from/to；会话视图新列暴露 |
| 前端 DiscoveryView | 4 例 | 复核面板条件渲染、退回/完结调真实命令、NEEDS_USER_INPUT 只给完结、409 竞态重拉对齐 |
| 前端 AssistantView | 3 例 | REVIEW_REQUIRED 提示+双按钮、跃迁调用参数、失败标记可见 |

**回归（受影响面全量）**
- `cargo test -p pm-ai --lib discovery_orchestrator`（138 例）
- `cargo test -p pm-api --test conversation_router --test discovery`（15+7 例）
- 前端 `DiscoveryView.test.ts` + `AssistantView.test.ts` 33 例
- `pnpm run check:i18n`（zh-CN/en 6311 key 对齐）+ `vue-tsc --noEmit`
- 安全回归清单见 §6

## 6. 统一安全基线核对（R1 面）

| 项 | 结论 |
|---|---|
| 人工审批边界 | **未放松**：REVIEW_REQUIRED→COMPLETED/IN_PROGRESS 仍是人工动作（编排器零改动不碰该状态；测试钉死前提保留） |
| AI 自动批准拦截 | 不涉及（R1 无审批面改动） |
| 审计完整性 | 新增：人工跃迁审计行（同事务，真实主体+auth_mode） |
| 跨项目隔离 | status 端点原有 `project_in_tenant` 守卫未动；新审计函数继承同一边界 |
| 并发/幂等 | 跃迁 `FOR UPDATE` 行锁+边校验（409）不变；轮次幂等（StaleRound）不变；失败落库幂等覆写 |
| 状态机 | **零新边**：`SESSION_EDGES` 10 条原样；DB CHECK 原样 |

## 7. 验收标准对照（任务书 §四）

- 正常消息 ✅（原有路径，回归覆盖）
- 需求关键词触发 REVIEW_REQUIRED ✅（HTTP 用例：发送回执如实 + 消息落库）
- 用户完成必要确认 / 拒绝建议 ✅（既有 findings 裁决面 + 新退回/完结入口）
- AI 处理失败 ✅（`last_round_error` 落库+展示，自动重试提示；状态诚实不伪造）
- 网络中断后恢复 ✅（发送失败保留原文+重发，既有；轮询自恢复，既有）
- 重复提交 ✅（按钮 transitioning 防抖 + 服务端 409 竞态重拉）
- 会话重新打开 ✅（COMPLETED 回落 chat，既有用例 `completed_interview_falls_back_to_chat` 回归覆盖）
- 页面刷新后状态恢复 ✅（全部状态服务端驱动：会话行/审计/失败标记，无本地派生）
- 无永久悬挂消息 ✅（死信三出口：等复核提示 → 退回恢复消费 / 完结回落 chat）

## 8. 真实业务场景验证（待运行态回归）

full-dev 部署态验证步骤（业务回归批次执行，不在本批次声明通过）：
1. 会话进入 REVIEW_REQUIRED 后在 chat 发含需求关键词消息 → 期望：发送成功、访谈段出现「等待人工复核」提示块
2. 点「退回继续访谈」→ 编排器下一轮消费消息
3. 点「复核通过并完结」→ 会话 COMPLETED，后续消息回落 chat
4. 断开 Provider 后发消息 → 访谈段出现失败标记行
