# DK-F 第 0 步预核实记录 V1.0

| 项 | 值 |
|---|---|
| 基点 | master `968494d`（KP-2 + DK-E 已合，全量 1328/1328） |
| 日期 | 2026-11-09 |
| 方法 | 读码钉证据（file:line），不虚构能力；只登记现状，不改代码 |
| 结论 | 三条均核实；另发现 **4 处真栈缺口（G-1~G-4）**，其中 G-1/G-2 直接决定 §8.3 能否在真栈全绿，须裁决后进 Stage 1 |

---

## 一、核实 1：「更正待知识应用」标记的消费链

| 链路 | 是否存在 | 证据 |
|---|---|---|
| 标记写入 | ✅ | `crates/pm-application/src/discovery/orchestrator_round.rs:353-385`：`discovery_question(status='deferred', why_blocking=NULL, finding_id=contradicted.id)`，正文 `[更正待知识应用] {target}：被否定事实「…」→ 更正后事实「…」…`；前缀**无命名常量**（局部 `format!`） |
| 标记携带的数据 | 部分 | 不含 kb `doc_key` / pc `object_key`；旧 finding 关系在 contradicted finding 的 evidence `"contradicts:{id}"`（:288）+ question 的 `finding_id` FK |
| KB 侧（KP-3）消费 | ❌ | `kb_service/*`、`kb_pc_handlers.rs`、`knowledge_services.rs`、`kb_memory_api.rs` 零 `discovery.*` 读取；`kb_pc_handlers.rs:483-512` 只有反向（分析完成→开访谈） |
| PC 侧（KP-2）消费 | ❌ | `pm-application/src/pc/*` 零 discovery 引用；`propose_from_analysis` 唯一生产调用方 = `PcAnalyzeRequestedHandler`（`kb_pc_handlers.rs:278`），三条件只吃分析信号 |
| 编排器上下文回显 | ✅（泛读） | `pm-ai/src/discovery_context.rs:489-498,761-777`：S6 列出 deferred 问题正文，不解析前缀、不动 KB/PC |
| 人工查看面 | ✅（REST） | `GET /v1/pm/projects/{pid}/discovery/sessions/{sid}`（`pm_discovery.rs:31-34`）返回 questions 含标记 |
| 人工写 `kb.document_revision` 的 REST | ❌ | 仅分析流水线 `kb_pc_analysis.rs:606` 写；K3 = 501（`knowledge_services.rs:310-323` PortNotWired） |
| 人工建 PC 提案的 REST | ❌ | `pc::create_proposal` 无 REST/MCP 暴露；人只能 P5 approve/reject AI 起草的提案（`knowledge_services.rs:706-750`） |
| K4 评审 | ✅ 但对象不同 | `POST /v1/kb/memories/{id}/review` 评审的是 `kb.memory_revision`（`kb_service/memory.rs:291`），不是 digest |
| finding 人工裁决 | ✅ | `POST /v1/pm/projects/{pid}/discovery/sessions/{sid}/findings/{fid}/verification`，body `{status, answer, confirmed_finding?, operation_id?}`（`pm_discovery.rs:320-363`） |

**结论**：标记今天是**只写 + 可查看**；设计稿 §12.2-4 写的「KB 由 KP-3 k4 评审链接收、PC 由 KP-2 接收」在代码里**没有接收端**。
**demo 台词据此写**：「更正已登记为待知识应用标记，KB/PC 在人工决定前零变化；知识应用的接收链属后续批（KP-3/KP-2 续作），本 demo 演示到人工裁决 findings 为止」。
可选的**人工代应用**（明确标注非标记消费）：经 `POST /v1/kb/memories` 由人提一条讨论记忆「认证方式=JWT」→ K4 评审 approve（落 `kb.memory_revision`，不改 digest）。

## 二、核实 2：前端会话页 interview 段渲染

| 项 | 现状 | 证据 |
|---|---|---|
| `landed / discovery_session_id / handoff` 消费 | ❌ 零 | `ai-frontend/src/api/conversations.ts:90-99` 响应类型为裸 `ConversationMessage`；`AssistantView.vue:164-173` 只取 id/created_at/role/content |
| `handoff/undo` 调用 | ❌ | 全前端无调用 |
| system 边界消息 | ⚠️ 渲染为 AI 气泡 | `AssistantView.vue:169` `role === "user" ? "user" : "ai"`；`ConversationRole = "user" \| "ai"`（`ui/ai-interaction/types.ts:353`）；`metadata` 类型 unknown 从未读 |
| discovery 段消息/问题/待办 | ❌ | `src/api/discovery.ts` 契约层齐（list/get/answer/defer/verification），**无任何非测试文件 import** |
| 回复获取 | 4s 轮询 | `AssistantView.vue:71,100-140`（增量 after/after_id） |
| 现有测试 | 11 + 7 例 | `AssistantView.test.ts`（须 mock vditor）、`conversations.test.ts` |

**连带后果（真栈可见缺陷）**：升级后用户继续在线程里发言 → 消息落 `discovery_message`，而页面只轮询 `conversation_message` → **用户自己的回答和编排器追问都不会出现在页面上**。Stage 2 必须走「最小接线」分支（预期如此，据实登记）。

## 三、核实 3：真 provider 与三开关

| 开关 | 读取点 | 取值 | compose 透传 |
|---|---|---|---|
| `AI_PM_CONVERSATION_ROUTER`（pm-api） | `pm-application/src/conversation_router.rs:49,97-113` | `on` / 其它=off（拼错 WARN） | ❌ |
| `AI_PM_CONVERSATION_REPLY`（ai-pm-worker） | `pm-ai/src/conversation_reply.rs:60-77` | 同上 | ❌ |
| `AI_PM_DISCOVERY_ORCHESTRATOR`（ai-pm-worker **+ pm-worker**） | `pm-ai/src/discovery_orchestrator.rs:107-128`；pm-worker `main.rs:44` 用作**导入自动开访谈闸** | 同上 | ❌ |
| `AI_PM_SNAPSHOT_ANALYSIS_MODEL` | `pm-ai/src/snapshot_model.rs:90-112` | auto(默认)/derived | ❌（默认即可） |
| `AI_PM_RUN_DISPATCH` | `run_dispatch/mod.rs:120-138` | on/probe/off | ✅ `docker-compose.ai-pm.yml:122-123` |

provider 解析三级：项目指定 → 租户首个 enabled 且 `configuration.model` 非空 → None（`team_llm.rs:1141-1211`）；REST `POST /v1/pm/tenants/{tid}/llm-providers` + `.../status {"action":"enable"}`（`checkpoint1-e2e.sh:102-110`）。凭据以 `AI_PM_SECRET_KEY` 加密存 `ai.team_llm_credential`。

## 四、真栈缺口（第 0 步新增发现）

| # | 缺口 | 证据 | 对 §8.3 影响 | 建议处置 |
|---|---|---|---|---|
| **G-1** | **生产 worker 未注入 `DiscoveryWriteBack`**——DK-D 写回钩子只在测试里接，生产默认 `NoWriteBack` | `apps/ai-pm-worker/src/main.rs:340-344` 只 `.with_context_builder(..)`；默认值 `discovery_orchestrator.rs:977` | 场景 1 在真栈**停在 REVIEW_REQUIRED，不起草需求、不分解任务** → B1/B2/B5/E1~E5 真栈全不可达 | DK-F 内补一行装配（`.with_write_back(DiscoveryWriteBack::new(..))`）+ 一例 main 装配断言；属「联调接线」不改逻辑体 |
| **G-2** | `ai.llm_call_record` **无 metadata 列**，`sections_dropped` 只进 WARN 日志 | `0001_baseline.sql:993-1008`；`call_record.rs:96-118`；`discovery_orchestrator.rs:1028-1032` | **D2 真栈必 FAIL**（DK-C/DK-B 已挂账至 DK-F） | 选项 a：迁移 `0003_llm_call_record_metadata.sql`（1 列 JSONB 可空，无回填）+ record_call 写入；选项 b：D2 真栈登记 FAIL/挂账，以日志 WARN 为替代证据 |
| **G-3** | 三开关未进任何 compose | 见§三 | 不改 compose 则真栈开不了 | ckpt overlay 内加 env 透传（只动 overlay，不改主 compose 默认） |
| **G-4** | 本机栈当前已瘫 | `pm-api / iam / token / gateway` Exited 10h；`aipm-ckpt-pg` Exited(127)；两 worker 因 DB 主机名解析失败 Restarting | Stage 1 须整栈重起 | 按 ckpt 配方重起（overlay + shell 级 DATABASE_URL/TOKEN_SERVICE_URL/JWKS_URL/AI_PM_SECRET_KEY） |

其它据实登记：
- 场景 2 PC 侧「人工应用更正」无任何 REST（§一），F2/F3 只能按 DK-E 口径「零提案行 + 标记可见 + kb/pc 零变化」判。
- 真 provider 需要 API Key；新 ckpt 库无存量 provider 行，须注册一次。
