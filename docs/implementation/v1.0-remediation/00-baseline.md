# 00 — v1.0 修复基线（Task 0）

- 日期：2026-11-24
- 基线 commit：`3a50c7cd`（master = origin = github，工作区 clean）
- 依据：`docs/acceptance/v1.0-business/`（FINAL-REPORT + 9 阶段文档，2026-11-24）
- 性质：五项 P1 缺陷修复的起点基线。**原验收证据只读保留，不修改。**

## 1. 环境与运行时状态核实

| 项 | 状态 |
|---|---|
| git | master=3a50c7cd，clean，双远端一致 |
| full-dev 栈 | 27 容器 Up；worker/api 近期重启过，其余 9h |
| worker 开关 | `AI_PM_CONVERSATION_REPLY=on`、`AI_PM_DISCOVERY_ORCHESTRATOR=on`、`AI_PM_RUN_DISPATCH=off`（**.env 部署态，非代码默认**） |
| 模型目录 | `ai.team_llm_provider`=1、`ai.team_llm_model`=1、`ai.team_llm_model_authorization`=1、`ai.team_llm_model_policy`=1（验收期操作员经 REST API 补齐的三件套仍在） |
| TaskFlow 项目 | 74a545ea-379c-4c97-bd0e-0e5dc7c8bb02，4 个 discovery 会话：3 COMPLETED + 1 NEEDS_USER_INPUT（验收期 REVIEW_REQUIRED 会话已人工完结，**当前库中无 REVIEW_REQUIRED**） |
| worker 日志 | 2026-11-23T23:34Z 有一次 `to_review_required=1` 真实跃迁；编排器持续活跃 |
| GitHub | 无开放 Issue；PR#1/#2/#3 均已合并/关闭，无在途修复 |

## 2. 结构性发现（运行时 + 代码核实）

1. **消息无状态机**：`ai.conversation_message`（id/conversation_id/role/content/content_type/metadata/created_at）与 `discovery.discovery_message`（id/tenant_id/session_id/speaker/source/content/seq/created_at）都**没有处理状态字段**。消息入库后「是否有回复」完全取决于后台循环是否拾取——死信的结构性根因。
2. **`transitionDiscoverySession` 前端 API 函数已存在**（`ai-frontend/src/api/discovery.ts:124`，POST `/v1/pm/projects/{id}/discovery/sessions/{sid}/status`），但 views/components **零调用**（仅测试 mock）——缺的是 UI 入口，不是 API 封装。
3. **模型目录前端零接线确认**：全前端 grep `llm-models|llm-model-policies|llm-model-authorizations` 零命中 → B01-04 未修复。
4. **designs.ts 只读确认**：仅 list/get/listRequirements/listFindings/listAdrs/listBaselines/evaluateReadiness/listApprovals/getWorkspace/listAnalysis，无 create/update/submit/approve/link → B04-02 未修复。

## 3. 缺陷追踪矩阵

> 代码映射（根因/模块/API/页面/模型/测试）在下方各小节，由代码调查填充。

### B03-01（P1）REVIEW_REQUIRED 会话死信 → R1
- **验收记录**：03-knowledge.md §3
- **复现**：会话进入 REVIEW_REQUIRED 后，chat 发含需求关键词消息 → 编排器只扫 IN_PROGRESS/NEEDS_USER_INPUT → 消息无回复无提示；UI 无完结会话入口 → 永久黑洞
- **根因（四处断点，代码已核实）**：
  1. 路由：`conversation_router.rs:352-354` `session_is_active` 把 REVIEW_REQUIRED 视为活跃 → 规则 1（:549-553）消息照常续接落访谈（201，无提示）；落库后线程最新消息为 system 边界行（:29-31 不变量），永久脱离 chat 回复循环扫描集（`conversation_reply.rs:163`）
  2. 编排器：`discovery_orchestrator.rs:1306` `scan_pending` 显式 `WHERE s.status IN ('IN_PROGRESS','NEEDS_USER_INPUT')` → REVIEW_REQUIRED 内新用户消息无人消费；写回扫描面（:1242-1262）只起草需求草案、不解除状态不回复（:1166-1167 注释亲述断头路）
  3. 状态机出口存在但不可达：`session.rs:239-240` REVIEW_REQUIRED→IN_PROGRESS/COMPLETED 两条边在；REST `POST .../discovery/sessions/{sid}/status`（`pm_discovery.rs:37-40`）与 MCP `pm_discovery_complete`（`tools.rs:1445`）可用；**前端 `transitionDiscoverySession`（discovery.ts:124）零生产调用**，DiscoveryView/AssistantView 只读展示状态（DiscoveryView.vue:216 纯 Badge；AssistantView.vue:344-355 纯文本）
  4. 消息无状态列：`discovery.discovery_message` 与 `ai.conversation_message` 均无处理状态字段（「待处理」=派生态）；编排器/chat 失败仅内存 WARN+30s 冷却，不落库不可见
- **影响模块**：pm-application（router/session）、pm-ai（orchestrator/reply）、pm-api（ai_conversations/pm_discovery）、ai-pm-worker、前端 AssistantView/DiscoveryView/discovery.ts
- **相关 API**：POST `/v1/pm/projects/{pid}/conversations/{cid}/messages`、POST `/v1/pm/projects/{pid}/discovery/sessions/{sid}/status`、GET session detail
- **数据模型**：`discovery.discovery_session.status`（0001_baseline.sql:2465-2479，DB 只查值域不查边）、`discovery.discovery_message`、`ai.conversation.discovery_session_id`（0002）
- **现有测试**：状态机=pm-application/tests/discovery.rs:113；HTTP=pm-api/tests/discovery.rs:156；router=pm-api/tests/conversation_router.rs（**无 REVIEW_REQUIRED 收新消息用例**）；编排器=pm-ai/src/discovery_orchestrator/tests.rs:629（三条件绿→REVIEW_REQUIRED）；`writeback_sweep.rs:257-270` 钉死「REVIEW_REQUIRED 不是编排候选」（前提保留，不推翻）
- **目标验收**：REVIEW_REQUIRED 会话内新消息有明确处理状态与用户提示；提供用户处理/完结入口；完成后恢复处理；全场景无永久悬挂

### B09-01（P1）chat AI 无项目正式记录 → R2
- **验收记录**：09-project-summary.md §3、03-knowledge.md B03-02
- **根因（已核实）**：chat 注入面=「最小两节」——`chat_context.rs:105-168` 仅 S1 项目画像（2k）+ S2 锚点仓库事实（4k）+ 用户手工 attach 的对象节（`chat_context.rs:202-247`，`ai.conversation_context_item`）；库内需求/设计/ADR/任务/发现记录不自动进入
- **可复用资产**：访谈第八循环已有六节投影 `discovery_context.rs`（profile/pc 认知/kb 摘要/requirements approved 修订/tasks/interview_state，预算 36k）；`ai_context_items.rs:1011-1248 load_injection_sources`（approved 正文注入+租户/项目过滤）；隔离模式=全部读 SQL `tenant_id+project_id` 首过滤
- **目标验收**：AI 能引用/定位正式记录、区分草稿与正式、项目隔离、变更后取最新版、上下文选择与长度控制

### B01-04（P1）模型目录前端零调用 → R3-A
- **验收记录**：01-project-creation.md §3 B01-04
- **根因（已核实）**：后端三面齐全（`team_llm_catalog.rs`：GET/POST `/v1/pm/teams/{id}/llm-models`、PUT `llm-model-authorizations`、GET/PUT `llm-model-policies`，Team Owner/Admin 权限）；前端 `api/llm.ts` 仅 provider 面+旧 project policy 面，**零 catalog 调用**；设置页 `SettingsTeamAiView.vue` 只管 Provider
- **激活所需数据形状**（`test_seed.rs:8-59` 权威）：provider enabled + `team_llm_model` enabled + `team_llm_model_authorization` team_wide + `team_llm_model_policy` role_key='general'
- **选模链**：`team_llm_catalog.rs:896-1052` resolve_model_for_role（project override → role policy → general 兜底），失败报 `general_policy_missing` 等 Configuration 错
- **目标验收**：UI 可完成目录+授权+general 策略配置；Provider 启用后 AI 即可用；不暴露密钥

### B01-01（P1）开箱不可用且无引导 → R3-B
- **验收记录**：01-project-creation.md §3 B01-01
- **根因（已核实）**：三开关默认 off（compose `${AI_PM_CONVERSATION_REPLY:-off}` 等，F22 新计费型生产者默认关）；worker off 时循环不 spawn；无 Provider 时 reply 只打 WARN skip 不落库不通知（`conversation_reply.rs:191-201`，测试钉死 `no_provider_skips_without_reply_or_record`）；**前端无任何「AI 未启用」感知信号源**——无 ai-status 端点，`ai-team/overview` 的 bound 语义不校验目录/授权/策略，chat 面板无 typing/pending/错误指示（grep 零命中）
- **目标验收**：未配置时 UI 显式提示原因+配置入口；配置后真正可用；基础项目功能不受影响；不擅自开启有外部副作用的循环

### B04-02（P1）Design/ADR 全链无 UI → R4
- **验收记录**：04-design-adr.md §3 B04-02
- **根因（已核实）**：前端只读——`api/designs.ts` 13 函数全 GET（自述「写路径一律走后端命令，前端只调不改状态」），无 adrs.ts；后端全生命周期 API 齐全（`pm_designs.rs:781-882`：create/revision/submit/link/approvals/decide/supersede/deprecate/blocking/freeze）
- **可复用模式**：需求确认哈希+CAS 对话框（RequirementsView.vue:306-329）、审核中心理由输入（ReviewDecisionReasonInput.vue）、HumanApprovalIndicator 组件
- **权限**：AI 可起草/提交/请求批准（source='ai' 留痕）；**AI 禁批**（`require_human_conn`，SC-05，测试 design_s03.rs:228）；MCP 无 S03 确认类工具
- **i18n 缺口**：gate key `scope_defined`/`acceptance_ready` 无翻译（后端 readiness.rs:183-192 vs yaml gate 段不匹配）
- **目标验收**：TaskFlow 全流程可操作：创建 Design→关联需求→修改出 Revision→创建 ADR→关联→人工接受→人工批准→就绪门禁检查

## 4. 统一安全回归清单（每批次必跑）

1. 跨项目访问隔离 2. 人工审批权限 3. AI 自动批准拦截 4. 并发/乐观锁 5. 正式记录与草稿隔离 6. REQ/TD/ADR/任务关联 7. 审计日志 8. 归档与状态变化 9. API 权限校验 10. 前端隐藏≠后端授权

六道审批闸门回归：需求人工确认 / Design 人工审批 / ADR 人工接受 / 任务批确认 / mark-ready AI 403 / 任务分解 B5 阻断。

## 5. 批次与交付映射

| 批次 | 缺陷 | 交付文档 | PR |
|---|---|---|---|
| R1 | B03-01 | R1-conversation-reliability.md | 待建 |
| R2 | B09-01 | R2-project-memory.md | 待建 |
| R3 | B01-01/B01-04 | R3-ai-onboarding-model-catalog.md | 待建 |
| R4 | B04-02 | R4-design-adr-ui.md | 待建 |
| 回归 | 全部 | docs/acceptance/v1.0-remediation/FINAL-REPORT.md | — |
