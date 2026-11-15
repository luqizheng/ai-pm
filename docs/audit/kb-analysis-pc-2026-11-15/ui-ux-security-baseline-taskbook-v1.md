# PM-AI UI/UX 安全基线与 SEC-P0～P1 交互影响审查任务书 V1.0

> **状态**：已落盘待评审（本仓核对结论见附录 A，2026-11-16）
> **进度（2026-11-16 追记）**：G1 ✅ → G2 ✅（V-1 MATCH / V-2 PARTIAL 仅 P2）→ G3 ✅
> （阶段 D 真实业务链 10/10：S1~S6 + B1~B4，含人工门/失败恢复/**通知状态/审计主体**）。
> **阶段 E（UI-SEC-01~14）尚未启动**；本文档的 UI 交互要求仍待 Phase E 实施与签收。
> **版本**：V1.0 · **日期**：2026-11-16
> **专项类型**：产品体验规范 + 安全交互影响审查
> **上游依据**：三域架构裁决 V1.0（Q1~Q12）· 四域映射审计任务书 V1.1 · V-1/V-2 专项任务书 V1.0 · 交付物 05 V1.1 · SEC-P0 修复及 G1 验收（ee4cd046）· SEC-P1-1 修复（e16e228d）· AI Collaboration Workspace/审批交互/Context Panel 设计基线

## 0. 任务书目的

本任务书用于审查 PM-AI 当前及规划中的 UI/UX 是否充分表达系统的安全边界，重点覆盖 SEC-P0-1～5 和 SEC-P1-1～2 所涉及的用户可见交互。不重新设计整个产品，不改变领域模型、权限模型或后端安全修复方案。

目标：
1. 建立统一的 UI/UX 安全交互基线。
2. 识别 SEC-P0～P1 缺陷对现有页面、操作流程和反馈机制的影响。
3. 明确必须调整、建议调整和无需调整的页面。
4. 建立 UI 状态与后端授权结果的一致性要求。
5. 为 Playwright E2E 和安全回归测试提供 UI 验收标准。
6. 防止前端交互设计被误认为后端安全控制。

## 1. 范围与边界

### 1.1 审查范围

| 安全问题 | 级别 | UI/UX 关注点 |
|---|---|---|
| SEC-P0-1：事件接口缺少 Project 可见性保护 | P0 | 事件来源、操作主体、项目上下文、触发反馈 |
| SEC-P0-2：跨租户审批 | P0 | 审批对象、项目归属、人工审批主体、执行结果 |
| SEC-P0-3：跨租户 Veto | P0 | 目标命令、撤销影响、操作反馈 |
| SEC-P0-4：Freeze/Unfreeze | P0 | 自动化模式、项目上下文、状态变更确认 |
| SEC-P0-5：跨租户通知读取 | P0 | 通知列表可见范围、项目归属、数据隔离 |
| SEC-P1-1：Project Cognition 决策缺少显式 Human 主体校验 | P1 | 人工确认入口、身份展示、决策主体记录 |
| SEC-P1-2：身份拒绝、身份伪造、授权失效负向测试缺失 | P1 | 拒绝反馈、操作状态不变、失败后恢复 |

### 1.2 不在本任务范围

不因本任务书自动启动：全局导航重构 / Personal Workspace 重设计 / Project Dashboard 全局布局调整 / Repository Analysis 核心流程重设计 / KB·PC·Project Cognition 对象模型调整 / 权限或认证系统重构 / Schema Migration / Git 正文权威迁移 / 新增审批业务类型 / 扩展或修改八类 Response Envelope 公共契约。

### 1.3 安全职责边界

- UI 展示的身份不是可信身份凭证。
- UI 中的 Project ID 不是授权证明。
- 隐藏按钮不是权限控制。
- 前端路由拦截不是租户隔离。
- 前端过滤通知不是数据访问控制。
- 操作成功提示必须以服务端成功响应为依据。
- 后端拒绝请求后，前端不得继续显示操作成功状态。

所有真实授权必须在服务端执行。

## 2. UI/UX 安全基线

### 2.1 身份表达规范

| 主体类型 | UI 显示名称 | 语义 |
|---|---|---|
| Human | 人工用户 | 真实用户身份 |
| Agent | AI Agent | AI 执行或提议主体 |
| Service | 系统服务 | 后台服务主体 |
| System | 系统 | 系统生成或自动触发的活动 |

要求：
1. 关键操作记录必须区分操作发起者与最终确认者。
2. Agent 提议不得显示为人工确认。
3. Service 执行不得显示为人工操作。
4. Human Actor 展示必须来自可信服务端响应。
5. 禁止前端通过请求参数自行声明 kind=human 并视为有效身份。

Project Cognition 必须保留自身 Human Actor 决策身份契约，不得用 PC 审批记录替代。

### 2.2 Project 上下文规范

跨项目操作必须明确显示所属 Project。适用：审批 / Veto / Freeze·Unfreeze / 事件触发 / 通知 / 项目级 AI 自动化操作。页面或卡片至少应识别：Project 名称 / 所属 Team（需要展示时）/ 操作对象 / 操作来源 / 当前操作状态。

禁止仅依靠不可见的 Project ID 表达操作上下文。目标对象不属于当前用户可访问范围时，UI 不得通过错误消息、对象名称、状态或其他辅助信息泄露其存在性。

### 2.3 操作影响范围规范

可能触发执行、撤销、审批或自动化状态变更的操作，必须展示可理解的影响范围，至少区分：仅影响当前记录 / 影响 Task 或 Run / 影响 Project / 可能触发后台 Agent 或 Worker / 可能改变已执行命令或正式知识状态。不确定影响范围时不得自行推断为无影响。

### 2.4 服务端状态一致性规范

1. 请求发出不代表操作成功。
2. 请求超时不代表失败也不代表成功。
3. HTTP 4xx 不得显示为操作成功。
4. HTTP 5xx 或网络错误时不得擅自修改服务端状态展示。
5. 操作失败后应重新获取可信状态。
6. 并发状态变化时提示用户刷新或重新确认。
7. 重试不得造成重复审批、重复 Veto 或重复发布。

### 2.5 权限拒绝反馈规范

| 场景 | UI 反馈 | 数据展示要求 |
|---|---|---|
| 未认证 | 请重新登录或完成认证 | 不展示受保护数据 |
| 无 Project 访问权限 | 无权访问该项目 | 不泄露项目内部信息 |
| 无操作权限 | 当前账号无权执行此操作 | 可说明缺少的权限，不泄露其他租户信息 |
| 对象不可见 | 对象不存在或当前不可访问 | 统一避免跨租户存在性探测 |
| 状态冲突 | 数据已发生变化，请刷新后重试 | 保留可信原状态 |
| 服务暂时不可用 | 操作暂未完成，请稍后重试 | 不提前展示成功 |
| 请求结果不确定 | 正在确认操作结果，建议刷新状态 | 不盲目重复执行 |

前端按稳定错误契约展示，不得暴露内部数据库错误、SQL 错误或敏感堆栈。

## 3. SEC-P0 交互影响审查

### 3.1 SEC-P0-1：事件接口（POST /api/v1/events）

风险：跨租户事件注入可能向其他 Project 写入事件、触发分析 Job、污染审计。UI 审查对象：项目事件列表 / 事件详情 / Agent 活动记录 / 事件触发的分析执行记录。

调整要求：展示事件所属 Project；展示来源主体及类型；区分人工/Agent/系统触发；对关联分析任务展示实际执行状态；前端传入的 actor 不得成为可信审计主体；请求被拒时不得留下"已成功创建"的乐观记录。

验收：越权事件请求失败；事件列表无虚假成功记录；无分析 Job 被创建；合法项目事件仍正常显示。

### 3.2 SEC-P0-2：跨租户审批（POST /api/v1/supervision/approvals/{id}/decision）

风险：用户可能批准其他租户红线审批并触发真实 AI 命令执行。UI 审查对象：审批中心 / 审批卡片 / 审批详情 / AI Collaboration Workspace Actions 区域 / 审批历史。

审批卡片建议展示：Project / 发起主体（Human·Agent·Service）/ 审批对象 / 操作内容 / 影响范围 / 当前状态 / 最终决策人（服务端返回真实 Human Actor）/ 决策时间（服务端确认时间）。

交互要求：AI 提议与人工审批视觉与语义区分；审批按钮只表示发起请求不代表授权成功；后端拒绝后状态不得被前端提前更新为 Approved；审批完成后重新读取状态；审批对象已变化时要求重新确认；不允许前端构造 actor_id 或 kind 冒充人工审批。

验收：无权限用户不能通过 UI 完成审批；越权请求不改变审批状态；不触发 AI 命令；合法审批人仍能正常审批；审批历史展示实际决策主体。

### 3.3 SEC-P0-3：跨租户 Veto（POST /api/v1/supervision/veto）

风险：针对其他租户已执行 AI 命令发起回滚。UI 审查对象：AI 命令详情 / 执行历史 / Veto 撤销入口 / 命令补偿记录。

调整要求：显示目标命令所属 Project；展示命令内容与执行状态；展示撤销/补偿影响；高影响操作提供确认步骤；完成后以服务端状态更新界面；失败时不得显示"已回滚"。

验收：无权限主体无法撤销其他 Project 命令；拒绝后命令状态不变；不产生补偿记录；合法 Owner 仍能按业务规则执行 Veto。

### 3.4 SEC-P0-4：Freeze / Unfreeze

风险：未经授权用户可能修改其他 Project 自动化模式。UI 审查对象：Project Settings / 自动化模式设置 / Project Command Center / 自动化执行状态展示。

调整要求：展示 Project 名称 / 当前模式 / 目标模式 / 影响范围 / 操作状态。推荐交互：选择 → 展示变更前后状态及影响 → 确认 → 服务端验证 → 成功后刷新 → 拒绝或失败保留原状态。禁止仅通过本地 Toggle 改变正式自动化模式。

验收：越权操作被拒；配置无变化；合法 Owner 操作仍成功；页面展示与服务端最终状态一致。

### 3.5 SEC-P0-5：跨租户通知读取（GET /api/v1/projects/{id}/notifications）

风险：跨租户用户读取其他 Project 通知及敏感 payload。UI 审查对象：全局通知中心 / Project Notification Panel / 通知详情 / 通知跳转链接。

调整要求：通知数据必须由服务端按权限过滤；每条通知有明确 Project 上下文；跳转后重新验证目标页访问权限；前端不得以隐藏通知项代替后端过滤；不得经摘要/预览/搜索索引/缓存泄露无权内容；未授权通知不得进入前端可见数据模型。

验收：越权请求被拒；响应不含其他租户 payload；列表与详情均不泄露；合法用户正常展示。

## 4. SEC-P1 交互影响审查

### 4.1 SEC-P1-1：Project Cognition Human Actor 身份门

现状：决策端点曾缺显式 Human 强制校验（后端修复已落地 e16e228d：require_human_decision_context）。UI 配合要求：正式确认入口明确标记为人工决策；AI Agent 只能提出 Proposal 不得被表达为正式确认人；确认前展示 Item/证据/影响；确认后显示服务端返回的真实 Human Actor；不得以前端身份类型字段决定 Human；无法确认身份时不得进入成功状态。

保留语义：AI 提议≠人工确认；Repository 不识别 JWT 身份；Service 层承担授权边界；Project Cognition 用自己的 Human Actor 契约；PC Approval 不替代其决策记录。

验收：非 Human 主体无法完成正式确认；伪造身份字段无法绕过；授权失效后旧会话不能继续决策；合法 Human 正常完成；决策记录显示实际 Human Actor。

### 4.2 SEC-P1-2：负向测试与 UI 异常反馈

| 测试组 | 场景 | UI 期望 |
|---|---|---|
| 身份拒绝 | Agent / Service 调正式决策 | 不显示确认成功 |
| 身份伪造 | 请求体或 Header 伪造 Human | 不显示确认成功 |
| 授权失效 | 成员资格撤销或凭据失效 | 不显示确认成功，重新读取状态 |

每组断言：HTTP 拒绝 / DB 无未授权写入 / 前端无错误成功状态 / 按钮与详情状态可恢复 / 合法人工路径正常。

## 5. 页面级影响矩阵

本节是页面审查清单，不代表已确认现有页面全部存在这些功能（现状核对见附录 A.2）。

| 页面 / 模块 | 影响级别 | 处理意见 |
|---|---|---|
| Approval Center | 高 | 必须审查审批对象、Project、Human Actor 和执行状态 |
| AI Collaboration Workspace / Actions | 高 | 必须审查审批、执行、撤销和权限反馈 |
| Project Settings / Automation Mode | 高 | 必须审查 Freeze / Unfreeze |
| Project Notification Center | 高 | 必须审查租户隔离和通知可见性 |
| Project Activity / Event Timeline | 中高 | 审查事件主体和触发状态 |
| Project Cognition Review | 高 | 必须审查 Human Actor 确认入口 |
| Project Dashboard | 中 | 检查跨模块状态是否准确反映服务端结果 |
| Repository Analysis | 低 | 不重设计分析主流程，仅检查事件和操作主体展示 |
| KB Review | 低 | 不改变 KB 确认语义，仅检查审批上下文 |
| Product Cognition | 低 | 不改变 PC 对象模型，仅检查主体展示 |
| Personal Workspace | 无直接调整 | 暂不纳入重设计 |
| 全局导航 | 无直接调整 | 保持现有 IA 基线 |

## 6. 统一交互状态模型

安全敏感操作组件应统一具备：Idle / Submitting / Confirming / Succeeded / Rejected / Conflict / Failed / Unknown 八态（含义与展示要求见专家原文，Rejected=安全拒绝反馈、Unknown=查询最终状态不盲目重复操作）。具有副作用的操作必须结合后端幂等语义设计重试交互。UI 不得自行创造后端没有提供的操作状态或审批状态。

## 7. 设计规范与组件要求

### 7.1 安全敏感组件（建议职责名，不要求精确代码名或立即新增七个组件）

ProjectContextHeader / ActorIdentity / ApprovalImpactSummary / SensitiveActionConfirmation / PermissionDeniedState / OperationResultState / AuditTrailSummary。

### 7.2 视觉表达原则

保持专业、克制、高信息密度风格；身份类型用稳定图标+文字标签，不依赖颜色作为唯一识别；高风险操作在确认与影响说明上区分；不使用模糊的"AI 已确认"表达人工审批结果；不以隐藏控件制造安全错觉。

## 8. 测试与验收方案

### 8.1 测试层次

| 层次 | 载体 | 验证内容 |
|---|---|---|
| L2 | Rust 集成测试 | 后端授权、身份拒绝、对象归属、零副作用 |
| L3 | Playwright | 页面状态、权限反馈、交互结果、状态刷新 |
| L4 | 完整业务链 E2E | 审批、发布、Close、通知等真实链路 |

### 8.2 UI 安全测试清单

UI-SEC-01 审批卡片正确显示所属 Project / 02 Agent Proposal 不显示为 Human Decision / 03 跨租户审批失败后不显示 Approved / 04 Veto 失败后不显示已撤销 / 05 Freeze 失败后自动化模式保持原状态 / 06 通知列表不出现未授权 Project 内容 / 07 事件拒绝后不出现虚假成功事件 / 08 Agent 无法通过 UI 完成 Project Cognition 正式确认 / 09 身份伪造后 UI 不显示人工确认成功 / 10 授权撤销后旧会话操作被拒绝 / 11 操作超时后 UI 不盲目重复执行 / 12 合法 Owner·Human 路径未被安全修复误伤 / 13 通知跳转目标重新执行服务端访问校验 / 14 页面刷新后状态与服务端最终状态一致。

### 8.3 证据要求

每项测试保留：页面/组件名、场景与主体、Project·Tenant 夹具、请求响应、UI 前后状态、后端 DB 副作用断言、截图或 Playwright trace、对应 SEC 缺陷编号、执行结果。不得将仅有截图、按钮隐藏或前端提示的测试判定为安全通过。

## 9. 实施阶段与依赖

Phase A SEC-P0 后端修复（已完成，UI 仅影响盘点）→ Phase B L2 基础设施（已完成）→ Phase C V-1/V-2 安全回归（已完成，**G2 已通过**）→ Phase D 真实业务链 E2E（Repository Analysis→Report→KB Revision 确认→PC Proposal→Human Approval→Publish→Close，重点验证人工确认门/执行结果/异常恢复/通知状态）——**已完成：10/10，通知状态与审计主体已入链断言**；UI 级 UI-SEC-01~14 仍属 Phase E。→ Phase E UI/UX 安全基线签收（页面影响审查/组件规范/L3·L4 证据/遗留登记）。

## 10. 分层验收门

| 门 | 内容 |
|---|---|
| G-UI-1 | 安全交互设计完成：SEC-P0~P1 页面影响矩阵完成；高风险操作交互规范明确；身份与 Project 上下文表达明确；未经批准的全局 UI 重构未发生 |
| G-UI-2 | 服务端安全契约可用：SEC-P0 修复过 G1 ✅；V-1 Human Actor 身份契约形成明确实现与响应语义 ✅（e16e228d）；权限拒绝和状态冲突响应契约可供前端使用（现状=message-only，见附录 A.4） |
| G-UI-3 | UI 安全回归通过：UI-SEC-01~14 完成；未授权操作无虚假成功状态；合法操作未被误伤；页面刷新与服务端状态一致 |
| G-UI-4 | 完整业务链验证：与交付物 05 的 G3·G4 衔接；审批、发布、Close 的 UI 状态正确；关键操作主体和审计信息可追溯 |

**G-UI-2 ≠ G-UI-3 ≠ G-UI-4**；任何一层未通过，不得用下一层的部分成功替代。

## 11. 不批准事项

不授权：修改四域对象模型 / 修改 Schema / 启动 Git 权威迁移 / 改变 KB·PC·Project Cognition 正式确认语义 / 通过前端实现租户隔离或人工身份校验 / 扩展公共 Response Envelope / 全局重构 UI 导航·Dashboard·Workspace / 将 UI 安全测试通过视为后端安全专项通过 / 将 SEC-P0 修复批准等同于 V-1/V-2 专项签收。

## 12. 交付物

D-UI-01 页面影响审查表 / D-UI-02 UI·UX 安全交互规范 / D-UI-03 安全敏感组件规范 / D-UI-04 UI 安全测试用例（UI-SEC-01~14）/ D-UI-05 Playwright 测试证据 / D-UI-06 遗留问题清单 / D-UI-07 UI·UX 安全基线签收记录（G-UI-1~4）。

## 13. 待确认事项（U-01~U-06，事实基建议见附录 A.5）

U-01 Actor 展示粒度（全量 vs 仅安全敏感操作）；U-02 Project 上下文显示范围；U-03 高影响操作统一二次确认；U-04 结果不确定时的恢复模式（须服从后端幂等契约）；U-05 审计信息可见范围（服从既有领域权限策略）；U-06 通知不可见反馈（不得泄露目标存在性）。

## 14. 版本记录

| 版本 | 日期 | 修改说明 |
|---|---|---|
| V1.0 | 2026-11-16 | 首次建立 UI/UX 安全基线，覆盖 SEC-P0-1~5、SEC-P1-1~2，建立页面影响矩阵、交互状态、测试与分层验收门。 |

---

## 附录 A：本仓现状核对结论（2026-11-16，ai-frontend src 实证）

### A.1 关键事实：SEC-P0×5 五个被修端点**当前没有任何前端调用方**

全 src（排除 test/mock）grep 实证：`POST /api/v1/events`、`/api/v1/supervision/veto|freeze|unfreeze|approvals/{id}/decision`、`GET /api/v1/projects/{id}/notifications` **零调用**。这五个是 legacy/外部 intake 面（供 lmms 网关与兼容通道），前端实际走：
- 通知/公告：`api/notifications.ts` → `notification.announcements.listMine`（BFF 公告通道，**非** legacy feed）；
- 事件流：`api/events.ts` → `GET /v1/pm/events/stream`（SSE 审计流，非 POST events）；
- 审批/干预：ReviewCenterPage / DeliveryReviewView / HumanInterventionView 走 `/v1/pm/*` 新接口（review-approvals 等，后端已有真人门）。

**结论**：SEC-P0×5 对前端无直接暴露面，§3 各节"UI 审查对象"中的审批卡片/事件列表/Freeze 设置/通知 Panel 大多为**规划态或不存在的页面**。阶段 E 的实际工作量集中在 SEC-P1-1 的真实 UI 面（A.2）与状态一致性既有缺陷（A.3），P0 五项在 UI 侧的验收收敛为"前端继续不直连 legacy 端点 + 后端回归保持绿"。

### A.2 真实存在、受 SEC-P1-1/P1 修复影响的 UI 面（§5 矩阵现状修订）

| 任务书矩阵项 | 现状 UI 面 | 后端门 | UI 影响 |
|---|---|---|---|
| Project Cognition Review | `views/project/cognition/CognitionPage.vue`（decideCognitionItem → /v1/pm/.../cognition/items/{id}/decision） | require_human_decision_context（e16e228d）✅ | 高：确认入口标记人工决策、确认后展示服务端 decided_by |
| Baseline 逐项决策（V1-EXT-01，矩阵未列） | `api/baseline.ts:61`（消费方=知识页 baseline 页签） | 同上共享门 ✅ | 高：同上，**建议矩阵补行** |
| KB 审阅中心 | `views/project/review-center/ReviewCenterPage.vue` + 知识页 3 页签待办 | 后端各自真人门 ✅ | 中：审批上下文/主体展示 |
| PC 提案决策 | `views/project/knowledge/AnalysisReportsPanel.vue`（/v1/pc/proposals/{id}/decision） | ensure_formal_authorized ✅ | 中：Agent 提议与人工确认视觉区分 |
| 交付域审批 | `views/project/execution/DeliveryReviewView.vue`（review-approvals） | 后端已有门 ✅ | 中 |
| Approval Center / Automation Mode 设置 / Event Timeline / Notification Panel（legacy） | **不存在**（规划态） | — | 矩阵改"无现状 UI 面"；不得反向造页面 |

### A.3 §2.4/§6 的现成反例（G-UI-1 前置必修，均为已知未提交/未修缺陷）

1. **ToastHost 从未挂载，useToast 静默 no-op**——"Rejected/Failed 状态展示"目前整链路无出口；
2. 分栏页页顶 Alert 不可见（仓库重分析 409 无反馈缺陷的同根因）；
3. i18n 裸 key、窄主区表单破损等已知 UI 缺陷（v1.0 冒烟清单）会影响 §6 状态模型的可读性。

**建议**：G-UI-1 的第一项实施动作=修复 ToastHost 挂载 + 统一拒绝反馈出口，否则 UI-SEC-03~05/09 的"不显示成功"断言没有可观测载体。

### A.4 错误契约现状（§2.5 依赖）

本仓硬约定：**API 错误体只有 message**（无结构化错误码，除 iam_* 前缀码）。§2.5"按稳定错误契约展示"的现实口径=HTTP 状态码 + message；任务书 §1.2 禁止扩展 Response Envelope 与此一致——前端分类只能靠状态码族（401/403/404/409/5xx），附录建议在 D-UI-02 中明确该映射表。

### A.5 U-01~U-06 事实基建议（供裁决，非结论）

| 项 | 建议 | 依据 |
|---|---|---|
| U-01 | 分层：决策/审批/veto/freeze 等安全敏感操作强制展示 Actor 四分；一般活动流不强制 | 信息密度原则 + §7.2 |
| U-02 | 安全敏感操作卡片必须显示 Project；单项目页内操作以页面上下文为准，不逐卡重复 | 现有页面均为项目上下文路由 |
| U-03 | veto/freeze/发布统一二次确认（SensitiveActionConfirmation 一套） | 影响范围同级别（§2.3） |
| U-04 | 统一"查询服务端最终状态→展示实际结果→幂等允许重试"；幂等契约现成（pc-decision:{id}、AlreadyDecided 409） | S6/§4.3 |
| U-05 | 服从既有领域权限策略，不新造审计可见性模型 | §13 自身要求 |
| U-06 | 列表中不出现（服务端过滤）+ 深链直访 404，与后端"不可见即 404 不泄露存在性"口径一致 | require_project_visible 语义 |

### A.6 流程建议

本任务书与阶段 D（真实业务链 E2E）**并行执行**：G-UI-2 三项依赖已全部满足（G1✅、Human Actor 契约落地✅、错误契约=message-only 现状可用）；UI-SEC-08/09 可直接复用 v1_cognition_gate 的 L2 断言作后端对照。执行顺序建议：先修 A.3 两个既有缺陷（ToastHost/Alert）→ 再做 UI-SEC 用例，否则断言无载体。

### A.7 阶段 D 执行后追记：E2E 脚本是否需要更新（2026-11-16）

**结论：本文档不要求改动阶段 D 的链脚本；UI-SEC-01~14 属 Phase E，未在阶段 D 落地。**

- 阶段 D 载体 = `playwright.chain.config.ts` + `e2e/chain/*.spec.ts`（API 级真链，非 UI）；
  已执行 **10/10 PASS**（S1~S6 + B1~B4），判定见 `phase-d-g3-g4-verdict.md`。
- 为衔接 G-UI-4「审批/发布/Close 状态与主体可追溯」，阶段 D 已补两类**链路级**断言：
  1. 通知状态：`pc_proposal_pending` / `kb_document_revision_published` / `pc_version_published`
     （`integration.notification`，按 dedup_key 去重）；
  2. 审计主体：`kb.document_revision.published` 与 `analysis_report.closed` 的
     `governance.audit_log.actor_id` = 真人 owner principal。
- **仍未覆盖（Phase E 范围）**：页面/组件级 UI-SEC-01~14、页面状态与服务端一致性、
  拒绝反馈可见性（依赖 A.3 ToastHost/Alert 修复）。不得以阶段 D 的 API 断言替代 UI 安全签收。
- 本文档 §3 的 SEC-P0 五项前端零调用方结论不变（A.1）：阶段 E 不新增 legacy 页面，
  只审查真实存在的 Project Cognition / Baseline / KB / PC / 交付审批面。
