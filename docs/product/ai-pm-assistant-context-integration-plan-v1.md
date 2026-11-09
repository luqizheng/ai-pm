# AI 助手对话 × 上下文整合计划 V1

| | |
|---|---|
| 日期 | 2026-11-09 |
| 状态 | 待裁（D-IC-1~7） |
| 范围 | 壳层指令栏 / 右上下文栏 / 会话页 / 会话 REST / worker 回复链 / 响应卡信封协议 |
| 不覆盖 | discovery 访谈编排链（DK-B~E 已裁不改动）；文件上传面（挂账）；KB 文档消费面（D-IC-4 裁后另立） |
| 输入 | DK-0 对话编排设计 V1（注入口径）；DK-C 注入基建；DK-A 载体判定；AI Interaction Component Library V1.0 |

## 〇、覆盖

① 两块壳层 UI 与数据链定位 · ② 现状审计（十二个事实 + 四条断点） · ③ 整合原则 · ④ 四批次实施（Batch 3 = chat 注入 / 信封展示 / 交互写回生命周期三纵切） · ⑤ 待裁清单 D-IC-1~7 · ⑥ 分批验收清单 · ⑦ 迁移预估

## 一、定位（事实权威）

| # | 事实 | 位置 |
|---|---|---|
| F-1 | 底部 AI 指令栏 `CommandBar`：作用域串（项目 / Repository / 分支）纯展示；提交仅入队 `{text}` 纯文本；附件按钮（Paperclip）无 handler | `ai-frontend/src/components/shell/CommandBar.vue:53-58,74-82,139-147` |
| F-2 | 右上下文栏 `ContextPanel`：「助手可用的上下文」卡与指令栏共用同一派生串，与后端真实注入零关联；仓库详情真实字段仅 defaultBranch/role；相关文档 mock-only（真实态整段不渲染）；pin 按钮静态 | `ai-frontend/src/components/shell/ContextPanel.vue:79-106,183,200-212` |
| F-3 | 指令栏→会话页单槽交接仅 `{text}`，上下文在交接中丢失 | `ai-frontend/src/stores/shell.ts:69` |
| F-4 | 会话页 `toTimelineMessage` 只映射 `{id, role, createdAt, text}`，从不组装 `interaction` 信封 → 真栈 8 种交互卡一条不出现 | `ai-frontend/src/views/project/assistant/AssistantView.vue:164-173` |
| F-5 | chat 回复链模型上下文 = 固定语言镜像 system prompt + 最近 20 轮历史，项目/仓库知识注入 = 零（与 DK-0 F20 一致） | `crates/pm-ai/src/conversation_reply.rs:312-345` |
| F-6 | 六节注入（36k 预算）只在 discovery 访谈编排器生效，chat 段不享受 | `crates/pm-ai/src/discovery_context.rs` |
| F-7 | `ai.conversation` 仅身份/标题/状态列，无任何上下文锚点；已有 `discovery_session_id` 升级链接列先例（0002） | `migrations/0001_baseline.sql:816-827`、`migrations/0002_conversation_discovery_link.sql` |
| F-8 | 项目上下文双源：指令栏/上下文栏走 shell store；会话页取 projectId 走 scope store，两者不同步 | `ai-frontend/src/views/project/assistant/AssistantView.vue:53-61` |
| F-9 | 响应卡组件库完工：`AiResponseCard`（477 行）+ 8 类型响应卡 + `AiConversationMessage` 八卡映射表就绪，22 单测全绿 | `ai-frontend/src/components/ui/ai-interaction/{card,responses,conversation}/` |
| F-10 | 信封零生产零消费：`ai.conversation_message.metadata jsonb NOT NULL DEFAULT '{}'` 列已在但零语义约定；REST 写侧 `AppendMessagePayload` 只收 `content/content_type`；worker 落纯文本 | `migrations/0001_baseline.sql:840`、`ai-frontend/src/api/conversations.ts:50-54` |
| F-11 | 生命周期状态机已建：`InteractionStatus` 11 态（created/presented/in-progress/submitted/validating/processing/accepted/rejected/deferred/resolved/failed）+ `SubmissionPhase` 四相（idle/submitting/success/error）+ `SubmitResult{recorded,reference}`；四类卡接受/拒绝/提交/失败的**组件内反馈已测**（结果条/记录号/错误重试/原因保留） | `ai-frontend/src/components/ui/ai-interaction/types.ts:32-43,95-102` |
| F-12 | 交互写回 mock-only + 状态不回读：`submitAdapter.ts` 自述「目录页/测试用」，提交落 mock 记录号（`INT-*`）；交互状态仅存组件内存，刷新/重进即回退 presented——真栈无任何接收交互提交的端点，终态无持久化 | `ai-frontend/src/components/ui/ai-interaction/submitAdapter.ts:1-9` |

### 四条断点

1. **承诺脱节**：UI 宣称助手具备「项目 / Repository / main · 只读上下文」（F-1/F-2），后端实际注入为零（F-5）——用户以为 AI 知道仓库，实际它只知道最近 20 轮文本。
2. **声明丢失**：即使前端想带上下文，交接链（F-3）与消息写侧（F-10）都没有承载位；线程亦无绑定声明（F-7）。
3. **卡片空转**：交互卡设计系统完工（F-9）但信封协议读写两侧均不存在（F-4/F-10），运行时零消费。
4. **写回空转**：接受/拒绝/提交/失败的反馈只存在于组件内存（F-11 组件面已测），真栈无端点（F-12）——终态不落库、刷新即丢、无记录号权威来源，双端（他人/他设备）看到的交互状态互不同步。

## 二、整合原则

1. **注入在服务端编排**（DK-0 已裁口径）：前端只负责上下文声明的单一真源与诚实展示，不拼 prompt。
2. **只读投影**：注入源不建视图、不写冗余表；缓存仅进程内 TTL（沿 D-DK）。
3. **复用 DK-C 基建**：预算裁剪、`sections_included/dropped` 记账、`<context>` 数据非指令声明、凭据零注入扫描。
4. **fail-closed / fail-safe**：信封解析失败一律回退纯文本，不伪造卡片；上下文声明白名单外字段服务端拒收。

## 三、实施批次

### Batch 1 — 前端单一真源 + 快照交接（纯前端，无迁移）

- shell store 新增 `assistantContext`（project id/name/description + repository + 预留 route 锚点位）；`CommandBar` 作用域串、`ContextPanel` 上下文卡、会话页三处改消费同一对象，消灭三处各自派生（解 F-1/F-2/F-8 的派生分裂）。
- `pendingAssistantPrompt` 从 `{text}` 升级为 `{text, context 快照}`——提交时刻定格，导航后不漂移（解 F-3）。
- 项目双源收敛按 D-IC-3 裁决执行。

### Batch 2 — 线程上下文锚点（迁移 0003 + REST）

- `ai.conversation` 增线程级上下文锚点列（按 D-IC-1，建议 `context_repo_name` / `context_branch` 或单 JSONB 列）；建会话时声明，消息级不收（防每条漂移、防伪造）。
- `POST /conversations` 收可选 context 字段；服务端白名单校验：仅接受本项目真实存在的 repo/branch，非法值 400；对齐 0002 链接列先例，不加状态不加枚举。
- 存量会话全部 NULL = 无锚点，零回填（沿 0002 语义）。

### Batch 3 — 双纵切（后端为主，复用 DK-C 基建）

**3A · chat 精简注入**

- `conversation_reply` 接入 chat 版注入子集：按 D-IC-2，建议最小两节 = **S1 项目画像 + 仓库/代码事实**，与 UI 承诺范围严格一致——注入面 = 承诺面。
- chat 预算独立常量（建议 8,000 字符，访谈 36k 不动）；沿用逐节上限、低优先级整节裁剪、`sections_included/dropped/chars_total` 记账。
- 沿用防注入三闸：`<context>` 按数据对待声明、事实引用白名单、provider/凭据零注入扫描断言。

**3B · 响应卡信封协议（零迁移）**

- 信封 = `metadata.envelope`（`ai.conversation_message.metadata` 列已在，F-10；**零迁移**）。
- 生产者按 D-IC-5 裁决：V1 建议场景驱动——确定性服务链产卡（线程升级邀请、访谈入口等 DK-A 既有事件点），LLM 自由文本暂不产卡，结构化输出挂账。
- 服务端写入校验：type ∈ 8 类型白名单、payload 按类型 schema 校验、不合法即落纯文本（fail-closed）。
- 消费端：REST 读侧透出 metadata（列已回读，仅补契约文档）→ 前端 `toTimelineMessage` 解析 `envelope` 组装 `interaction`（`types.ts:363` 信封字段已在）；白名单外/解析失败回退纯文本——8 张已完工卡首次真栈激活。

**3C · 交互写回与生命周期闭环（接受 / 拒绝 / 提交 / 失败四反馈全真栈）**

- 新端点 `POST /v1/pm/projects/{id}/conversations/{cid}/messages/{mid}/interaction`：body 按 8 类型分发（answer / accept / reject+reason / submit / defer / decision+confirmation），返回权威终态 + 服务端生成记录号（`reference`，取代前端 mock `INT-*`）。
- **生命周期落库**（结构按 D-IC-7）：状态机沿 `InteractionStatus` 单向迁移（presented → 终态 accepted/rejected/deferred/resolved/failed），服务端拒绝回退/跳变；终态幂等——重复提交同一交互返回既有记录（200）不产生第二条。
- **四反馈语义定版**（沿 F-11 组件面，真栈逐项对齐）：
  - 接受 → `accepted` + 记录号 + 卡转只读（文案「已记录」，不写「已生效」）；
  - 拒绝 → `rejected` + 原因随记录落库，回读可见；
  - 提交 → `submitted` + 记录号；Decision 类须先过本人确认勾选（服务端复验，缺失 400）；
  - 失败 → 校验类 400（卡片错误相 + 原文保留可改）、服务端错 5xx（错误相 + 重试入口）、无记号不伪造成功。
- **状态回读**：信封 `status` 终态化——写回成功后服务端更新消息信封状态（或以交互表为准回读，按 D-IC-7），刷新/重进/他设备看到同一终态，不回退 presented。
- 权限：会话属主本人才能提交交互；Decision 类走人工确认链语义（`recorded ≠ 生效`）。

### Batch 4 — UI 诚实化 + 双栈验收

- `ContextPanel`「助手可用的上下文」卡改为展示真实注入源清单（与 3A 两节一一对应）+ 是否附加文件的真实状态；pin 按真实行为接线或移除。
- 附件按钮按 D-IC-6 执行（建议诚实禁用+挂账）；相关文档面按 D-IC-4 执行。
- 验收见 §五。

## 四、待裁清单

| # | 裁决点 | 选项（推荐在前） |
|---|---|---|
| D-IC-1 | 上下文绑定粒度 | **线程级**（建会话时定格；对齐 0002 先例） / 消息级（每条可变，漂移+伪造面大） |
| D-IC-2 | chat 注入范围 | **最小两节**（S1 项目画像 + 仓库事实，8k） / 全六节复用（成本↑，与访谈注入边界模糊） |
| D-IC-3 | 项目双源收敛 | **shell store 为真源**，scope store 跟随 / scope 为真源 / 保持双源仅文档化映射 |
| D-IC-4 | 相关文档面 | **继续隐藏**（V1） / 接 KB 文档源（KB-PC 已有文档面，另立批） |
| D-IC-5 | 信封生产者 V1 | **场景驱动**（确定性链产卡；LLM 结构化挂账） / LLM 结构化输出（类型漂移+注入风险，需强校验） |
| D-IC-6 | 附件能力 | **诚实禁用+挂账** / V1 接文件上传（牵 object-storage 未实现，E-4） |
| D-IC-7 | 交互终态落库结构 | **独立交互记录表**（迁移 0004：`ai.conversation_interaction`，记录号/终态/原因/决策人为列，信封 status 作回读投影） / 信封 `metadata.envelope.status` 就地更新（零新表，但审计弱、并发覆盖风险、决策人无列） |

## 五、分批验收清单

**Batch 1**
- A-1 三处消费同源：改 store 一处，指令栏串/上下文卡/会话页联动更新（单测）。
- A-2 快照交接：提交后切换项目再进会话页，发送携带的是提交时刻快照（单测）。
- A-3 切换中不回落旧项目（沿 Figma「07」，回归）。

**Batch 2**
- A-4 迁移 0003 up/down 干净；存量会话锚点全 NULL。
- A-5 REST：合法 repo/branch 201 落列；不存在 repo 400；白名单外字段拒收；他人会话不可改锚点（404 同形防探测）。

**Batch 3A**
- A-6 注入记账：`sections_included/dropped/chars_total` 落账（超长构造用例必过）。
- A-7 凭据零注入：provider 表字段关键字扫描断言（沿 D1）。
- A-8 chat 预算独立：8k 裁剪不触碰访谈 36k 路径（隔离用例）。
- A-9 无锚点会话（存量）行为与现状逐字节一致（向后兼容）。

**Batch 3B**
- A-10 信封白名单：8 类型外落纯文本；payload 破格落纯文本；两条均记 WARN 计数。
- A-11 前端解析失败回退纯文本，不留空白、不抛未处理 rejection（沿 7fd662d 坑）。
- A-12 真栈一卡激活 E2E：场景驱动卡在真实会话页内联渲染并可交互。

**Batch 3C（生命周期四反馈）**
- A-16 接受：accept 201 → 服务端记录号 → 卡转 `accepted` 只读；文案全链无「已生效」。
- A-17 拒绝：reject 携带原因 → `rejected` 落库；刷新后原因仍可见。
- A-18 提交：submit → `submitted` + 记录号；Question 答齐才放行（未答 400 带逐项校验错）；Decision 未勾选本人确认 400（服务端复验）。
- A-19 失败：校验错 400（错误相、原文保留可改）；服务端错 5xx（错误相 + 重试可用）；重试成功后终态正确。
- A-20 幂等与单向：终态重复提交返回既有记录不新增；presented → 终态外的迁移（回退/跳变）服务端 409。
- A-21 回读持久化：刷新/重进/他设备看到同一终态与记录号，不回退 presented。
- A-22 权限：非会话属主提交交互 404 同形；Question 草稿不上服务端（存本地）。

**Batch 4**
- A-13 上下文题 E2E：「当前仓库默认分支是什么」在带锚点会话中答对；无锚点会话答「不知道」不编造。
- A-14 上下文卡展示 = 实际注入面（两节来源清单）逐项对齐。
- A-15 mock / 真实双栈全过；`pnpm test` 全绿 + 既有门禁。

## 六、迁移与工作量预估

| 批次 | 迁移 | 主要改动 | 预估 |
|---|---|---|---|
| Batch 1 | 无 | shell store + 两壳组件 + 会话页映射 | 前端 1 天 |
| Batch 2 | **0003**（线程上下文锚点） | ai.conversation 加列 + POST /conversations 校验 | 后端 0.5~1 天 |
| Batch 3A | 无（复用 0003 锚点） | conversation_reply 注入子集 + 记账 | 后端 1~1.5 天 |
| Batch 3B | 无（metadata 列已在） | worker 落 envelope 校验 + 前端解析映射 | 前后端各 0.5~1 天 |
| Batch 3C | **0004**（按 D-IC-7=A：interaction 表） | 交互写回端点 + 生命周期落库/幂等/回读 + 前端真 submit 注入 | 后端 1~1.5 天 + 前端 0.5~1 天 |
| Batch 4 | 无 | ContextPanel 诚实化 + E2E | 0.5~1 天 |

**坑位预警**（沿既有记录）：rustfmt 单文件防波及（`--file`）；AssistantView 测试须 mock vditor；reka 组件触发器是 button；i18n yaml 顶层键=文件名 lastSegment；前端契约改动同步 `conversations.test.ts`。

## 七、对照设计委托书合规审计（2026-11-09）

对照《AI Interaction Component Library V1.0》Figma Design Prompt（用户提供的原始委托书），代码实现逐条核验：

| 委托书验收条款 | 实现证据 | 结论 |
|---|---|---|
| §11-1 八类共用一套卡架构 | `AiResponseCard` 基座 + 8 variants | ✅ |
| §11-2 每类配齐交互控件 | 四类主卡逐一核验（见 §一 F-9） | ✅ |
| §11-3 多问题共存一卡 | `QuestionPayload.questions[]` + `QuestionKind` 六题型（single/multi/text/textarea/number/date）全支持 | ✅ |
| §11-4 多答案一次提交 | responses-b「答齐后一次性提交」 | ✅ |
| §11-5 Accept ≠ Approve 可辨 | 三处测试锁定：Recommendation Accept=记录不写「生效」；Proposal 接收≠正式审批；Decision 本人确认+二次确认 | ✅ |
| §11-6 已提交卡不消失 | responses「转为只读汇总」 | ✅ |
| §11-7 pending/submitting/success/failure 四态 | `SubmissionPhase` idle/submitting/success/error | ✅ |
| §11-8 跨页复用 | controls 20 件 + ConfirmationDialog 共享件 = 21 | ✅ |
| §11-9 遵循现有设计系统 | tokens 变量强制 + scoped styles（CONTRACT 硬约束） | ✅ |
| §11-10 不绕过业务授权/审批 | **组件面** ✅（recorded≠生效、AI 不冒充决策人）；**真栈面** ⏳ 待 Batch 3C 写回端点 + 服务端复验（A-18） | ⏳ |

其余关键对照：§4.4 六题型全实现（`types.ts:180-186` + `QuestionResponse.vue:418-498`）；§4.5 决策人/时间/记录号三要素已测；§6 十一态生命周期与 `InteractionStatus` **逐字 1:1**，且委托书自注「presentation states，非业务实体状态机」——与 D-IC-7 最小落库口径一致；§7 会话集成七要求全过（卡内联/宽度自适应/折叠/历史可读/提交结果/Composer 固定/Context Panel）；§8 帧 01-16 + 控件库页 = 12 目录页全映射（IndexView `frames` 表）。

不适用代码验收：§9 Figma 交互原型（设计资产）；§10 视觉质量（需浏览器实测或视觉验收，归 Batch 4）。
