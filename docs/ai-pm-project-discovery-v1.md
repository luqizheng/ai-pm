# AI-PM 项目接入与认知建立（Project Onboarding / Project Discovery）设计 V1.1

> **文档性质**：设计依据（brainstorming 定稿 → 用户逐项评审 Q1–Q7 后修订，待评审 → 确认后进入实施计划）
> **版本**：V1.1
> **日期**：2026-11-01
> **来源**：与人类产品经理的多轮架构讨论（含双入口原型图、统一 Discovery Session 原型图、发现卡片原型图）
> **遵循**：`todo.md` §0.1 brainstorming 硬门禁；`AGENTS.md` 新项目无存量兼容约束
> **上位文档**：
> - `docs/superpowers/specs/2026-10-31-multi-repo-project-import.md`（Web 入口的已落地实现依据）
> - `docs/ai-pm-mcp-executor-positioning-v1.md`（执行端 × MCP 定位基线，本设计遵守其 §4.2 知识污染红线与 §3.4 工具无关性）
> - `docs/ai-pm-markdown-knowledge-space.md`（Git 为权威 + PageIndex，本设计的 ProjectKnowledge 承载层）

### 变更记录

| 版本 | 日期 | 变更内容 | 触发 |
| --- | --- | --- | --- |
| V1 | 2026-11-01 | 首次落盘：双入口统一 DiscoverySession 模型 | 产品讨论定稿 |
| V1.1 | 2026-11-01 | 并入用户逐项评审 **Q1–Q7 裁决**：①四域边界（Import/Discovery/Task/Run）②Q1 独立建表 + 单向 FK + source/kind 分离 ③Q3 四类 finding + confidence/verification_status ④Q4 多仓库支持边界 ⑤新增 Q5 并行 Session / Q6 暂停恢复 / Q7 版本乐观并发 ⑥发现卡片 UI | 用户评审 |

---

## 〇、一句话定位

> **Project Onboarding 不是 Git Import，而是 Project Discovery**：通过代码分析、文档阅读和人机对话，让 AI 逐步建立对一个既有软件项目的业务背景、技术架构、当前状态与未来规划的认知，并将**经确认的认知**沉淀为可持续维护的项目知识。

核心产品判断：**AI-PM 不应要求用户在接入老项目时，先把所有背景、架构、需求、技术债务手工填写一遍**。它应像一个真正的 AI 技术负责人：
1. 先了解项目是什么、为什么存在；
2. 阅读已有代码和文档，建立初步认知；
3. 发现信息缺口和相互矛盾之处；
4. 主动向用户提问，澄清关键问题；
5. 将确认过的信息整理成项目知识；
6. 后续随 Task 执行不断修正和完善认知。

---

## 一、四个业务域边界（关键架构前提）

**Discovery 是独立于 Repository Import、Task、Run 的业务域。** 这四个概念边界一旦确定，后续 Web / Codex / MCP / 多 Agent 的架构才清晰：

| 业务域 | 职责 | 权威表 |
| --- | --- | --- |
| **Repository Import** | 把代码仓库**接入**系统（clone / 读 / 授权） | `integration.repo_import` |
| **Discovery** | **理解**项目（建认知、提问、确认、提案） | `discovery_session` 及子表 |
| **Task** | **推动**项目发生变化（业务任务） | `delivery.task` |
| **Run** | **执行**具体工作（一次执行实例） | `delivery.task_run` |

> 关键裁定：**不把 `repo_import` 扩展为统一 Session 表**。否则未来 Discovery 来源扩大到 `/pm-learn`、手动认知更新、Task 执行中发现、定期健康检查、其他 Agent 主动发起时，它会长成一个职责不清的通用工作流表。二者是不同的业务概念。

---

## 二、双入口统一模型

两个入口**不是两套独立的项目初始化流程**，而是**两个 producer 向同一个 `DiscoverySession` 写，最后汇入同一个 `ProjectKnowledge`**。

```
┌─────────────────┐        ┌─────────────────┐
│  Web 入口        │        │  Codex /pm-learn │
│  Git URL 授权    │        │  本地代码深度分析 │
│  可视化访谈       │        │  开发者访谈       │
│  知识审核         │        │  技术认知采集     │
└────────┬────────┘        └────────┬────────┘
         │ source=WEB               │ source=CODEX
         ▼                          ▼
   ┌───────────────────────────────────────┐
   │        DiscoverySession（统一认知过程）   │
   │  message / finding / question /         │
   │  answer / proposal  + 状态机            │
   └───────────────────┬───────────────────┘
                        ▼
              ┌─────────────────────┐
              │   ProjectKnowledge   │
              │  (Markdown + PageIndex)│
              └─────────────────────┘
```

- **Web**：项目创建、仓库授权、可视化访谈、知识审核——适合团队管理者/负责人。
- **Codex `/pm-learn`**：本地代码深度分析、开发者访谈、技术认知采集——适合已有开发环境的复杂老项目。
- **AI-PM**：统一管理 session、状态、审批、正式知识。
- **最推荐体验**：用户在 Web 创建项目并连接 Git → AI 初步分析后发现需深入代码 → 提示打开 Codex 执行 `/pm-learn` → Codex 完成后结果自动回 Web → 用户在 Web 确认最终认知。**Web 与 Codex 是一个工作流的两个界面，不是竞争入口。**

---

## 三、与既有资产的关系（避免重做）

| 本设计概念 | 既有资产 | 处置 |
| --- | --- | --- |
| Web 自动分析 + 访谈 | `multi-repo-project-import`（0009 实现：clone→read→analyze→draft_ready→apply→4 提案→审批） | **作为 `DiscoverySession(source=WEB)` 的一个 producer**，复用其 `RepoAnalysisModel::draft` + `ProjectDraft` + 证据接地 |
| `DiscoverySession` / `Message` / `Question` / `Answer` / `Proposal` | `ProjectDraft`(含 `unknowns`) + 提案链 | **独立建表**（Q1），新增子表；`unknowns` 映射为 `discovery_question` |
| 四类信息 observed / user_confirmed / speculation / contradicted | `ground_draft` 仅丢弃无据项 | **扩展**：显式 provenance + confidence + verification_status |
| `ProjectRepository` | `integration.project_repository`(0008) + `repo_import_source`(0009) | 直接复用，不加新绑定表 |
| `ProjectKnowledge` / `Document` / `Version` | `ai.project_document` + `document_revision`(0006) + PageIndex（`ai-pm-markdown-knowledge-space`，已 23/23） | 复用；`DiscoveryProposal` → 现有 `propose_document` 知识提案链（含 base_revision/hash 漂移检测） |
| Codex `/pm-learn` + MCP | `ai-pm-mcp-executor-positioning-v1` §4.1 九工具 / §4.2 禁直写 | 新增 `pm_discovery_*` 语义工具，propose-only |
| 多 Agent 认知分工 | import 已建 5 人编制（owner / ai_pm / architect / developer / tester） | 编制映射到 discovery 角色分工（V1 不并行） |

---

## 四、数据模型（Q1 裁决后）

在 `project` 下新增独立**认知域**（`discovery_*` 表，候选迁移 `0022_discovery.sql`），**不绑执行端实例**。

### 4.1 表结构与关系

```
Project
├── ProjectRepository            (既有 integration.project_repository)
│     ├── Repository A / B / C
├── repo_import                  (既有，接入过程；新增 discovery_session_id 单向 FK)
│     └── discovery_session_id ──┐
├── discovery_session  ◄──────────┘  (新增，独立认知过程)
│     ├── discovery_message      (对话记录，带 source)
│     ├── discovery_finding      (发现：四类 kind + evidence + confidence + verification_status)
│     ├── discovery_finding_repository  (finding ↔ 多仓库，Q4)
│     ├── discovery_question     (待用户澄清)
│     ├── discovery_answer       (用户确认/补充)
│     └── discovery_proposal     (知识变更提案，带 base_version)
└── ProjectKnowledge            (复用 ai.project_document + document_revision)
      ├── KnowledgeDocument
      └── KnowledgeVersion
```

> **关联方向（Q1 关键）**：只保留 **`repo_import.discovery_session_id → discovery_session.id`** 一个方向，不做双向外键。旧的 `repo_import` 记录保持兼容，新的导入流程挂接 `DiscoverySession`。

### 4.2 关键字段约定

| 实体 | 职责 | 关键字段 |
| --- | --- | --- |
| `discovery_session` | 一次项目认知建立过程 | `id, project_id, source, kind, status, created_by, created_at, updated_at` |
| `discovery_message` | 对话记录 | `session_id, speaker∈{ai,user}, source, content, seq` |
| `discovery_finding` | 项目发现 | `session_id, kind, content, confidence, verification_status, actor_role?` |
| `discovery_finding_repository` | finding ↔ 仓库（多对多） | `finding_id, repository_id` |
| `discovery_question` | 待用户澄清 | `session_id, question, why_blocking, status∈{open,answered,deferred}` |
| `discovery_answer` | 用户回答 | `question_id, answer, answered_by` |
| `discovery_proposal` | 知识库变更提案 | `session_id, target_document, base_version, proposed_content, action∈{add,modify}, evidence[], status∈{pending,approved,rejected,conflict}` |

**`source` 与 `kind` 必须分开，不可混为一谈（Q1）：**

| 字段 | 取值 | 含义 |
| --- | --- | --- |
| `source` | `WEB` / `CODEX` / `SYSTEM` | 来自什么入口/执行端 |
| `kind` | `INITIAL_ONBOARDING` / `UPDATE` / `TASK_DISCOVERY` | 什么业务类型的认知过程 |
| `status` | `IN_PROGRESS` / `NEEDS_USER_INPUT` / `REVIEW_REQUIRED` / `COMPLETED` / `PAUSED` | 过程状态 |

### 4.3 四类 finding 与知识晋升规则（Q3 裁决）

| kind | UI 展示 | 能否进入正式知识 |
| --- | --- | --- |
| `observed` | 已观察到 | 可以，仍需按知识审核策略处理 |
| `user_confirmed` | 用户已确认 | 可以 |
| `speculation` | 待确认 | **不可以直接进入正式知识** |
| `contradicted` | 存在冲突 | **不可自动覆盖原知识**（必须走提案） |

`discovery_finding` 增两个字段：
- `confidence`（0–1）：**模型判断的辅助信息，不能代替事实确认**。
- `verification_status`：如 `PENDING` / `CONFIRMED` / `REJECTED` / `CONFLICT`。

JSON 示例：

```json
{
  "kind": "speculation",
  "content": "支付模块可能尚未完成",
  "confidence": 0.78,
  "evidence": [
    {
      "repository": "backend",
      "file": "src/payment/mod.rs",
      "description": "存在未实现的支付接口"
    }
  ],
  "verification_status": "PENDING"
}
```

**冲突处理（易忽略）**：AI 发现的信息若与现有知识库冲突（`contradicted`），**不能直接覆盖原知识**；必须生成知识变更提案，由用户确认后更新，并保留历史版本。

### 4.4 并发、恢复与版本控制（Q5 / Q6 / Q7）

- **Q5 并行 Session**：允许 Web 与 Codex 同时对同一项目发起 `DiscoverySession`（并行采集）；但**知识提案统一审核**，不在采集阶段争抢写入。
- **Q6 暂停与恢复**：session 支持 `PAUSED` → `IN_PROGRESS`、跨天恢复、增量提交（不要求一次访谈跑完）。
- **Q7 防并发覆盖（版本乐观并发控制）**：每个知识提案携带 `target_document` + `base_version` + `proposed_content` + `evidence`；正式写入时校验 `base_version` 是否仍有效，无效则拒绝并置 `status=conflict`，**不允许后写者静默覆盖**。
  - 复用既有 `document_revision` / `propose_document` 的 `base_revision_id` 与内容 hash 漂移检测（`KnowledgeError::Conflict("document content drifted from its recorded hash")`）。

### 4.5 状态机与成熟度

**状态机（`discovery_session.status`）**：`IN_PROGRESS` / `NEEDS_USER_INPUT` / `REVIEW_REQUIRED` / `COMPLETED` / `PAUSED`。
- `COMPLETED` = **本轮学习完成，不等于项目已完全理解**。

**认知成熟度**：用多维度状态表达（背景 / 架构 / 当前情况 / 需求 / 决策 各自成熟度），**不用单一百分比**。

**session 不绑执行端实例**：可由多个来源贡献，甚至 Web 起头、Codex 续同一 session（`source` 记录每条产出来源）。

---

## 五、发现与确认 UI（Q3 原型）

「项目认知发现」卡片是把 `discovery_finding(kind=speculation)` 变成可裁决交互的核心组件：

```
┌──────────────────────────────────────────────┐
│ 项目认知发现                          [待确认] │
│                                                │
│ 支付模块可能尚未完成                            │
│                                                │
│ AI 根据代码分析推断：支付接口中存在尚未实现的逻辑。 │
│                                                │
│ 分析证据                                        │
│   backend/src/payment/mod.rs                    │
│   发现未实现的支付接口。                         │
│                                                │
│ 请确认实际情况                                  │
│   ● 确实尚未完成                                │
│   ○ 已经通过外部服务实现                        │
│   ○ 目前没有计划实现                            │
│   ○ 暂时无法确认                                │
│                                                │
│               [        确认        ]            │
└──────────────────────────────────────────────┘
```

- 卡片头部显示 kind 徽标（`待确认` / `已确认` / `存在冲突`）。
- 「分析证据」展示 `evidence[].file` + `description`，让用户能回到代码核对。
- 用户选择即写 `discovery_answer`，并把 `verification_status` 从 `PENDING` 推进到 `CONFIRMED` / `REJECTED`；若选项对应外部实现，则同时产生一条 `user_confirmed` finding。

---

## 六、Web 入口（P0，部分已落地）

创建项目后**不立即进入普通项目首页**，而是进入**项目接入向导**（原型图：Step 1/4「让 AI 先了解你的项目」）。

1. **创建项目** → **接入 Git**（GitHub 授权 / Git URL，必选）→ 提示「可以先接入仓库，稍后再完成项目认知」。
2. **访谈非无限聊天，而是有明确目标的 AI 访谈**（阶段表为初始建议，非强制全生成）：

| 阶段 | AI 工作内容 | 产出 |
| --- | --- | --- |
| 项目背景 | 询问业务目标、用户、历史 | project-overview.md |
| 仓库认知 | 分析仓库、模块、依赖 | repositories/*.md |
| 架构认知 | 分析技术架构与模块关系 | architecture.md |
| 当前情况 | 了解已完成 / 未完成 / 问题 | current-status.md |
| 需求梳理 | 收集现有需求与规划 | requirements/ |
| 技术决策 | 收集已知决策及原因 | decisions/ |
| 待确认事项 | 记录无法确认的信息 | open-questions.md |

3. 把现有 `draft_ready + unknowns` **升级为 session 驱动的访谈 UI**：findings / questions / answers 可视化，用户逐条回答（见 §五 发现卡片）。
4. 审核 `DiscoveryProposal` → 批准后实体化为知识文件（复用 `propose_document`，校验 `base_version`）。

---

## 七、Codex `/pm-learn` Skill（P1，executor-neutral）

交付物：一份 `SKILL.md`（命令式，名 `/pm-learn`），装进任意 MCP-capable 执行端（Trae / Codex / Claude Code / Cursor …），**禁止写死 Trae 专有 API**（守 §3.4 工具无关性 A1–A6）。

```
/pm-learn
 ├─ MCP 获取项目认知 Session（pm_discovery_get）
 ├─ 读取项目已有知识（pm_discovery_get_context）
 ├─ 分析当前 Git 仓库（grep / 读文件 / 读 README* / docs/**）
 ├─ 检查已有文档和代码
 ├─ 与用户澄清（多轮）
 ├─ 生成知识变更提案（DiscoveryProposal）
 ├─ MCP 提交 Discovery 结果（pm_discovery_submit_*）
 └─ AI-PM 验证并更新项目知识
```

**重要边界：Skill 不直接拥有项目知识库的最终写入权。** 它只能提交：
- 新发现的信息（finding）
- 用户确认的信息（user_confirmed）
- 建议新增 / 修改的知识文件（proposal，含 `base_version`）
- 证据来源（evidence）
- 尚未解决的问题（open-questions）

最终**验证、版本管理、正式入库由 AI-PM 负责**，避免 Codex 与 Web 双端改同一文件造成冲突。

---

## 八、MCP 接口（Q2 裁决）

> **命名**：为 `pm_discovery_*`，**不实现别名**（MCP Tool 名是对外契约，不宜频繁改名）。域划分：`pm_*` = AI-PM 业务域；`task_*` = Task 生命周期；`knowledge_*` = 知识库操作；Discovery 是独立业务过程，故 `pm_discovery_*` 合理。最终命名与 `ai-pm-mcp-executor-positioning-v1` Q1 一并冻结。

| MCP Tool | 作用 | 落到既有链 |
| --- | --- | --- |
| `pm_discovery_get` | 获取项目认知 Session | 读 `discovery_session` |
| `pm_discovery_get_context` | 获取已有知识、仓库信息、待确认问题 | 读 knowledge + session |
| `pm_discovery_submit_findings` | 提交 AI 分析发现（含 kind / evidence / confidence / verification_status） | 写 `discovery_finding` + `_repository` |
| `pm_discovery_submit_answers` | 回传用户回答（也可承载 AI 提问） | 写 `discovery_answer` / `discovery_question` |
| `pm_discovery_submit_proposal` | 提交知识库变更提案（含 base_version） | **复用 `propose_document`** 知识提案链 |
| `pm_discovery_complete` | 提交本轮学习完成状态 | 置 `status=COMPLETED / REVIEW_REQUIRED` |

所有写回均为 **propose，非直写**（守 §4.2 污染红线）。`ask_question` 在 V1 可由 `pm_discovery_submit_answers` 的 question 载荷承载，暂不单列工具。

---

## 九、持续认知过程（非一次性）

Project Onboarding **不是一次性初始化**。闭环：

```
第一次接入老项目（建背景 / 架构 / 当前情况）
        ↓
日常 Task 开发（发现架构变化 / 新业务规则 / 技术债 / 需求变更）
        ↓
更新 Project Knowledge
        ↓
下一次 Agent 执行时获取最新认知
```

含义：AI-PM 不只是管理 Task，还持续维护项目认知状态。与既有「Markdown 知识库 + PageIndex」天然契合——Markdown 是知识正式载体，数据库管 Session/状态/版本/关联，PageIndex 管检索。

---

## 十、多 Agent 认知分工（V1 不实现，预留）

| Agent | 负责学习 |
| --- | --- |
| 产品经理 Agent | 业务背景、用户、需求、规划 |
| 架构师 Agent | 系统架构、模块关系、技术决策 |
| 技术负责人 Agent | 代码现状、技术债务、开发规范 |

由项目负责人 Agent 汇总。V1 不并行访谈，但 `discovery_finding` / `discovery_proposal` 预留 `actor_role` 字段以支持后续分工。

---

## 十一、多仓库支持边界（Q4）

**V1 必须支持**：
1. 一个项目关联多个仓库（复用 `project_repository`）。
2. 每个 Finding 可关联一个或多个仓库（`discovery_finding_repository`）。
3. 每个仓库拥有独立的分析状态（复用 `repo_import_source.status`）。
4. `DiscoverySession` 可以分析多个仓库。

**V1 不做（留 P2）**：
- 自动识别跨仓库调用关系。
- 自动生成完整系统依赖图。
- 自动判断多个仓库的部署拓扑。
- 自动识别微服务之间的业务边界。

---

## 十二、V1 实施顺序与 YAGNI

| 优先级 | 功能 | 说明 |
| --- | --- | --- |
| P0 | Project / Repository 接入 | 已落地（0008/0009/0019） |
| P0 | `DiscoverySession` 数据模型 | 新增 `0022_discovery.sql`（含 finding/proposal/base_version） |
| P0 | Web 项目访谈 | 扩展现有 import 访谈 UI + 发现卡片 |
| P0 | MCP `pm_discovery_*` 接口 | 新增 6 个语义工具 |
| P1 | `/pm-learn` Skill | executor-neutral SKILL.md |
| P1 | Git 仓库结构与文档分析 | 复用 `RepoAnalysisModel` |
| P1 | 知识提案及审核（含版本校验） | 复用 `propose_document` |
| P2 | 多仓库架构关联分析 | 逐步增强 |
| P2 | 持续项目认知更新 | 后续增强 |

**V1 可限制代码分析深度，但数据库与 Session 模型须支持多仓库。**

**YAGNI（v1 不做）**：不并行多 Agent 访谈、不用单一百分比成熟度、不双端直写知识、不自动终止 session（由用户在 Web 点批准）、不做跨仓库深度分析（Q4）。

---

## 十三、已裁决结论（Q1–Q7）

| 编号 | 裁决 |
| --- | --- |
| **Q1** | **新建独立 `discovery_session`**；`repo_import` 保持独立，仅通过 `repo_import.discovery_session_id` 单向关联；`source` 与 `kind` 分开 |
| **Q2** | 暂用 `pm_discovery_*`，不实现别名；统一命名裁决前保持单一体系 |
| **Q3** | finding 分 `observed / user_confirmed / speculation / contradicted`；speculation 默认待确认、禁止直接晋级；增 `confidence` + `verification_status`；冲突走提案不覆盖 |
| **Q4** | V1 支持多仓库关联，不做跨仓库深度分析（P2） |
| **Q5** | 允许并行 `DiscoverySession`，知识提案统一审核 |
| **Q6** | Session 支持暂停、恢复、增量提交 |
| **Q7** | 知识提案采用 `base_version` 版本校验，防并发覆盖 |

**核心结论**：Discovery 是独立于 Repository Import、Task、Run 的业务域。Import 负责把仓库接入系统，Discovery 负责理解项目，Task 负责推动项目变化，Run 负责执行具体工作。四个边界确定后，Web / Codex / MCP / 多 Agent 的架构自然清晰。

---

---

## 附录 A：迁移 `0022_discovery.sql` DDL 草案（待实施）

> **状态**：草案。经评审确认后落到 `migrations/0022_discovery.sql`；届时需同步更新 schema 计数 / 租户隔离测试期望（现有权威口径「43 张表」与 `core_schema` 断言）。本草案遵循 0019 多租户模式：`tenant_id` + `(tenant_id, project_id)` 组合 FK + 每表 `(tenant_id, id)` 唯一。
>
> 依赖前提（0019 已建立）：`core.project` / `integration.project_repository` / `ai.project_document` / `ai.document_revision` / `ai.document_proposal` 均具备 `(tenant_id, id)` 唯一，可作为组合 FK 目标。

```sql
-- 0022_discovery: 项目认知域（Discovery）。
-- 与 0009 repo_import（仓库接入）是不同业务域：Import 负责接入，Discovery 负责理解项目。
-- forward-only；遵循 0019 多租户模式。

CREATE SCHEMA IF NOT EXISTS discovery;

-- 1. 认知会话：一次项目认知建立过程；不绑执行端实例，可多来源贡献；允许并行（Q5）。
CREATE TABLE discovery.discovery_session (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('WEB','CODEX','SYSTEM')),
    kind TEXT NOT NULL CHECK (kind IN ('INITIAL_ONBOARDING','UPDATE','TASK_DISCOVERY')),
    status TEXT NOT NULL DEFAULT 'IN_PROGRESS' CHECK (status IN (
        'IN_PROGRESS','NEEDS_USER_INPUT','REVIEW_REQUIRED','COMPLETED','PAUSED'
    )),
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_discovery_session_project
    ON discovery.discovery_session(tenant_id, project_id, created_at DESC);
CREATE INDEX idx_discovery_session_status
    ON discovery.discovery_session(tenant_id, status);

-- 2. 对话消息。
CREATE TABLE discovery.discovery_message (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    speaker TEXT NOT NULL CHECK (speaker IN ('ai','user')),
    source TEXT NOT NULL CHECK (source IN ('WEB','CODEX','SYSTEM')),
    content TEXT NOT NULL,
    seq BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, session_id, seq),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE
);

-- 3. 发现：四类 provenance（Q3）+ 可信度 + 校验状态。
CREATE TABLE discovery.discovery_finding (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('observed','user_confirmed','speculation','contradicted')),
    content TEXT NOT NULL,
    confidence DOUBLE PRECISION CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
    verification_status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (verification_status IN ('PENDING','CONFIRMED','REJECTED','CONFLICT')),
    evidence JSONB NOT NULL DEFAULT '[]'::jsonb,   -- [{repository, file, description}]
    actor_role TEXT,                                -- 预留多 Agent 分工
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_discovery_finding_session
    ON discovery.discovery_finding(tenant_id, session_id, kind);

-- 4. finding ↔ 仓库（多对多；Q4 V1 支持多仓库关联）。
CREATE TABLE discovery.discovery_finding_repository (
    tenant_id UUID NOT NULL,
    finding_id UUID NOT NULL,
    repository_id UUID NOT NULL,
    PRIMARY KEY (tenant_id, finding_id, repository_id),
    FOREIGN KEY (tenant_id, finding_id)
        REFERENCES discovery.discovery_finding (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, repository_id)
        REFERENCES integration.project_repository (tenant_id, id) ON DELETE CASCADE
);

-- 5. 待澄清问题。
CREATE TABLE discovery.discovery_question (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    question TEXT NOT NULL,
    why_blocking TEXT,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','answered','deferred')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_discovery_question_session
    ON discovery.discovery_question(tenant_id, session_id, status);

-- 6. 用户回答。
CREATE TABLE discovery.discovery_answer (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    question_id UUID NOT NULL,
    answer TEXT NOT NULL,
    answered_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, question_id)
        REFERENCES discovery.discovery_question (tenant_id, id) ON DELETE CASCADE
);

-- 7. 知识变更提案（Q7：base_revision_id 即 base_version，写入前校验；失效置 conflict）。
CREATE TABLE discovery.discovery_proposal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    session_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('add','modify')),
    target_document TEXT NOT NULL,          -- 目标文档 key/path
    document_id UUID,                        -- modify 时指向既有 ai.project_document
    base_revision_id UUID,                   -- Q7 提交时的知识版本；add 时为 NULL
    proposed_content TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '[]'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','approved','rejected','conflict')),
    document_proposal_id UUID,               -- 落地到 ai.document_proposal 后的关联
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session (tenant_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document (tenant_id, id),
    FOREIGN KEY (tenant_id, base_revision_id) REFERENCES ai.document_revision (tenant_id, id),
    FOREIGN KEY (tenant_id, document_proposal_id) REFERENCES ai.document_proposal (tenant_id, id)
);
CREATE INDEX idx_discovery_proposal_session
    ON discovery.discovery_proposal(tenant_id, session_id, status);

-- 8. 唯一关联方向（Q1）：repo_import → discovery_session，不做反向 FK。
--    注：组合 FK 不能用 ON DELETE SET NULL（tenant_id NOT NULL），保留默认 NO ACTION。
ALTER TABLE integration.repo_import
    ADD COLUMN discovery_session_id UUID,
    ADD CONSTRAINT fk_repo_import_tenant_discovery_session
        FOREIGN KEY (tenant_id, discovery_session_id)
        REFERENCES discovery.discovery_session (tenant_id, id);
CREATE INDEX idx_repo_import_discovery_session
    ON integration.repo_import(tenant_id, discovery_session_id);
```

### 附录 A.1 落地注意

1. **测试期望同步**：本迁移新增 7 张表（含 join 表），激活前需更新 `core_schema` / 租户隔离计数断言（现有权威口径 43 张）。
2. **`discovery` 新 schema 的权限**：确认应用角色对该 schema 有 USAGE/CREATE（参照既有 schema grant）。若团队不愿引入新 schema，可改落 `integration`，但会削弱「四域边界」表达。
3. **`base_version` 语义**：DDL 用 `base_revision_id` 承载（指向 `ai.document_revision`），比裸 BIGINT 更类型安全；Q7 的"版本失效即 conflict"由应用层在写入 `ai.document_proposal` 时校验（复用既有 hash 漂移检测）。
4. **并行 session（Q5）**：刻意不加「每项目唯一进行中」的部分唯一索引，与 `repo_import` 的单导入约束区分。

---

_本方案为产品设计参考，具体实施请结合公司合规与法务要求确认。_
