# 20 · P0 规格 ↔ 实现事实源偏差（R92–R102）

> **性质**：本文件**不是**新规格，而是「规格写法 ≠ 实现行为」的**实测对照表**。
> **裁决口径**：以我们的实现为事实源（规格与实现冲突时，实现是事实；规格按本表回写）。
> **来源**：2026-11-11 P0 阶段 B~D 可执行套件落地实测（`ai-frontend/e2e/p0-0{3,4,5,6,7}-*.spec.ts`，41 collected / 38 PASS / 3 NOT_IMPLEMENTED）。
> **落地记录**：`test-case/e2e-flow-execution-report-v1.md` §1.4（逐例断言）与 §3.2（本表）。

---

## 1. 偏差总表

| 编号 | 规格原写法 | 实现事实（实测 + 代码位置） | 影响与回写建议 |
|---|---|---|---|
| **R92** | 13 号 R23：intake 的 `analysis_request` 触发分析，产生 `ai.analysis_run` 与 `change_proposal` | intake 的 analysis_request 路由到 **kb_analyze**（kb stub），**不产生** analysis_run；`ai_analysis` job 的真实入口是 `POST /api/v1/events`（`apps/pm-api/src/ingestion.rs:103`，"New event row plus its ai_analysis job"） | 13 号 R23 需改写为「分析链入口 = events API」；断言改用 `POST /api/v1/events` |
| **R93** | 分析终态判定词 = `succeeded` | `ai.analysis_run.status` 终态字面量是 **`completed`**（`succeeded` 是 Run 的词汇） | 全套断言词统一为 `completed`（`failed` 仍为失败态） |
| **R94** | `ai.analysis_run.model_name` 反映命中的 provider | `model_name` 来自 `ai.project_ai_config`（bootstrap 默认 `'fake'`），**不随 provider 变**；provider 命中由 worker 日志 + mock 调用记录证明 | 断言放宽为「如实记录」（`fake` / `mock-e2e` 皆可），provider 命中另证 |
| **R95** | 缺 `X-AI-PM-Device-Id` → 拒绝 | `DeviceClaim::absent()` **仍执行**（C176：无归属但放行——T-088 语义） | 断言方向翻转：缺设备头**放行**；隔离由「非成员主体」承担 |
| **R96** | `pm_get_task_context` 入参 = `task_id` | 入参名是 **`task_ref`**（`crates/pm-mcp/src/tools.rs:78-83`） | 用错参数名 → 工具报「missing field」错误（假红）；断言与夹具统一用 `task_ref` |
| **R97** | 任务有承接人 → 可被领取 | 可领取性判定第 4 条要求承接人 `core.principal.kind='dev_agent'`（`run_dispatch/policy.rs:179-190`）；真人承接 → Run 被判 `no_executor_assignee` 退避回队（永远不进执行） | 物化链必须派给 **dev_agent**（项目 bootstrap 播种的 `developer` 主体，`bootstrap.rs:386`）；断言需覆盖「真人承接被拒」 |
| **R98** | 基线不可解析 → 仍会进执行 | 可领取性判定第 6 条：基线不可解析 → `baseline_unavailable` 退避（`run_dispatch/policy.rs:216-230`） | 失败注入夹具必须给 `head_commit_sha`，否则永远到不了执行器（假绿） |
| **R99** | 交付提交可在 Run 结束后补交 | 租约只在 `status ∈ {claimed, running}` 且 `lease_token_hash` 未清时有效（`run_context.rs:124-142`）；Run 收尾后提交 → `lease_invalid` | 交付必须在 Run **活跃期**提交（真实链路：执行端在进程退出前经 MCP 提交）；E2E 用「释放文件」协议制造该窗口 |
| **R100** | 16 号场景 B：设备未绑定该项目 → 工具层拒绝 | MCP 读面授权 = **项目成员资格**（`pm-mcp/src/tools.rs` `ensure_member`）；`X-AI-PM-Device-Id` 只做**归因**（设备心跳）。设备绑定的强制点在**派发侧**（D-2=c）：绑定 runner 设备的项目，worker 候选发现直接排除（`run_dispatch/claim.rs:344-380`） | 16 号场景 B 改写为「派发侧谓词」；读面隔离由「非成员主体」承担 |
| **R101** | release manifest 含 per-file sha256 列表 | manifest 是**整包 sha256** + `contents` 清单（`apps/pm-api/src/skills_package.rs:34-52`） | 断言按整包哈希：`sha256(zip bytes) == manifest.sha256` |
| **R102** | `reindex_required` 是索引表列 | 它是**质量门结果 JSON** 的字段（`knowledge_quality.rs:30` → `ai.document_proposal.quality_gate_result`）；`ai.document_index_state` 无此列 | 断言落点改到提案行：`quality_gate_result->>'reindex_required' = 'true'` |

---

## 2. 由偏差牵出的产品缺陷（已修复，实现已与契约对齐）

| 缺陷 | 现象 | 修复口径 |
|---|---|---|
| **DEF-P0-03**（P2） | 知识域人工门把「授权拒绝」报成 **409 Conflict**（规格与需求域同款人工门均为 403） | `KnowledgeError` 新增 `Forbidden`；知识域 `require_human` 改抛 `Forbidden` → HTTP **403 KNOWLEDGE_FORBIDDEN**；`pm-mcp` 同步映射 `ToolError::Forbidden` |
| **DEF-P0-04**（P2） | 同一条 review 审批**重复裁决返回 200**（裁决其实没生效） | 服务层改抛 `Err(ApprovalError::AlreadyDecided)` → HTTP **409**（API 契约早已按 409 定义，409 分支此前是死代码）；删除随之失效的 `ApprovalOutcome::AlreadyDecided` 变体 |
| **DEF-P0-05**（P2·flaky） | 依赖图 advisory lock 的 `bind(Uuid)` 撞上 `$1::text` 参数 → 随机 `invalid byte sequence for encoding "UTF8"` → 任务完成/审核裁决 500 | 锁键改绑**字符串**并给 SQL 加 `task-graph:` 前缀（与知识索引锁的语句文本不再相撞——sqlx 语句缓存按 SQL 文本复用，参数类型错配会被服务端当 text 解析 uuid 的 16 字节）；并发回归用例同步对齐 |

---

## 3. 知识提案面（提案质量门）的硬约束（回写 13/14/17 号的构造指引）

质量门 8 规则（`crates/pm-application/src/knowledge_quality.rs`）里与 E2E 构造直接相关的三条：

| 规则 | 约束 | 构造要求 |
|---|---|---|
| 2 · 来源 | `source_ref` / `source_event_id` / `analysis_run_id` 至少一个 | 请求体必须带 `source_ref`（或落事件/分析 run） |
| 6 · 引用 | 正文必须含 `source_events:` 或 `projects/` 锚点 | 正文追加一行 `来源：projects/<projectId>/kb/<docKey>` |
| 8 · 索引 | 正文至少一个 Markdown 标题 | 正文须有 `#` 标题 |

**状态语义**（`knowledge_proposal/submit.rs:144-150`）：门禁不过 → `rejected`；声明红线 → `redline_pending`（等人工确认，**不写 Git**）；否则 → **直接 `committed`**（自动发布）。

> 因此「提案 ≠ 正式知识、确认后才发布」的断言，**必须声明红线**（`redline_kind` ∈ `external_commitment` / `delete_history` / `rewrite_delivered`），
> 否则提案面会直接落 published，断言方向翻转。

---

## 4. 派发面（Run 领取）的可领取性清单（回写 15 号的构造指引）

可领取性判定（`run_dispatch/policy.rs:135-231`，全部通过才领取）：

| # | 判定 | 不满足的后果 |
|---|---|---|
| 1 | 重试未耗尽 | `retry_exhausted` 退避 |
| 2 | 审批证据满足（`require_human_approval`） | `policy_requires_approval` 退避 |
| 3 | 无活跃等待行 | `task_waiting` 退避 |
| 4 | 承接人是 `dev_agent` | `no_executor_assignee` 退避（**R97**） |
| 5 | 至少一个可写仓 | `no_repository_scope` 退避 |
| 6 | 全部声明仓 `verified` | `repository_not_verified` 退避 |
| 7 | 基线可解析（`base_commit` 或 `head_commit_sha`） | `baseline_unavailable` 退避（**R98**） |

> 退避是**指数**的（`2^attempts` 秒），故上述任一条不满足的表现都是「Run 永远 queued、`dispatch_attempts` 递增」，而非显式报错——
> 排障时先看 `delivery.task_run.release_reason`（取值为上述 reason 之一）。

---

## 5. 与本表相关的可执行载体

| 内容 | 位置 |
|---|---|
| 阶段 B~D 五个套件（34 例） | `ai-frontend/e2e/p0-03-ai-pm.spec.ts` / `p0-04-knowledge-authority.spec.ts` / `p0-05-task-delivery.spec.ts` / `p0-06-mcp.spec.ts` / `p0-07-knowledge-governance.spec.ts` |
| 共享 fixture / 助手 / 栈装配 | `ai-frontend/e2e/p0/fixtures.ts` / `helpers.ts` / `global-setup.ts` |
| 一键执行 | `bash test-case/evidence/p0/run-p0.sh`（41 collected / 38 PASS / 3 NOT_IMPLEMENTED） |
| 逐例断言与实测值 | `test-case/e2e-flow-execution-report-v1.md` §1.4 |
