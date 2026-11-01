# AI-PM 项目接入与认知建立（Project Onboarding / Project Discovery）设计 V1

> **文档性质**：设计依据（brainstorming 定稿，待评审 → 确认后进入实施计划）
> **版本**：V1
> **日期**：2026-11-01
> **来源**：与人类产品经理的多轮架构讨论（含双入口原型图、统一 Discovery Session 原型图）
> **遵循**：`todo.md` §0.1 brainstorming 硬门禁；`AGENTS.md` 新项目无存量兼容约束
> **上位文档**：
> - `docs/superpowers/specs/2026-10-31-multi-repo-project-import.md`（Web 入口的已落地实现依据）
> - `docs/ai-pm-mcp-executor-positioning-v1.md`（执行端 × MCP 定位基线，本设计遵守其 §4.2 知识污染红线与 §3.4 工具无关性）
> - `docs/ai-pm-markdown-knowledge-space.md`（Git 为权威 + PageIndex，本设计的 ProjectKnowledge 承载层）

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

## 一、双入口统一模型（关键架构决策）

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

## 二、与既有资产的关系（避免重做）

| 本设计概念 | 既有资产 | 处置 |
| --- | --- | --- |
| Web 自动分析 + 访谈 | `multi-repo-project-import`（0009 实现：clone→read→analyze→draft_ready→apply→4 提案→审批） | **reframe 成 `DiscoverySession(source=WEB)`**，复用其 `RepoAnalysisModel::draft` + `ProjectDraft` + 证据接地 |
| `DiscoverySession` / `Message` / `Question` / `Answer` / `Proposal` | `ProjectDraft`(含 `unknowns`) + 提案链 | 提升为独立 session 模型，新增子表；`unknowns` 映射为 `discovery_question` |
| 三类信息 fact / confirmed / speculation | `ground_draft` 仅丢弃无据项 | **扩展**：显式标注 provenance，speculation 禁止自动晋升知识库 |
| `ProjectRepository` | `integration.project_repository`(0008) + `repo_import_source`(0009) | 直接复用，不加新绑定表 |
| `ProjectKnowledge` / `Document` / `Version` | `ai.project_document` + `document_revision`(0006) + PageIndex（`ai-pm-markdown-knowledge-space`，已 23/23） | 复用；`DiscoveryProposal` → 现有 `propose_document` 知识提案链 |
| Codex `/pm-learn` + MCP | `ai-pm-mcp-executor-positioning-v1` §4.1 九工具 / §4.2 禁直写 | 新增 `pm_discovery_*` 语义工具，propose-only |
| 多 Agent 认知分工 | import 已建 5 人编制（owner / ai_pm / architect / developer / tester） | 编制映射到 discovery 角色分工（V1 不并行） |

---

## 三、数据模型（核心新增）

在 `project` 下新增独立**认知域**（`discovery_*` 表，候选迁移 `0022_discovery.sql`），**不绑执行端实例**：

```
Project
├── ProjectRepository        (既有 integration.project_repository)
│     ├── Repository A / B / C
├── DiscoverySession         (新增，统一认知过程)
│     ├── DiscoveryMessage   (AI ↔ 用户对话记录，带 source)
│     ├── DiscoveryFinding   (AI 发现：事实 / 推测，带 evidence + kind)
│     ├── DiscoveryQuestion  (AI 需用户澄清的问题)
│     ├── DiscoveryAnswer    (用户确认 / 补充)
│     └── DiscoveryProposal  (对知识库的变更提案)
└── ProjectKnowledge         (复用 ai.project_document + document_revision)
      ├── KnowledgeDocument
      └── KnowledgeVersion
```

**关键字段约定：**

| 实体 | 职责 | 关键字段 |
| --- | --- | --- |
| `discovery_session` | 一次项目认知建立过程 | `id, project_id, source∈{WEB,CODEX}, status, created_at` |
| `discovery_message` | 对话记录 | `session_id, speaker∈{ai,user}, source, content, seq` |
| `discovery_finding` | 项目事实或推测 | `session_id, kind∈{fact,confirmed,speculation}, claim, evidence[], actor_role?` |
| `discovery_question` | 待用户澄清 | `session_id, question, why_blocking, status∈{open,answered}` |
| `discovery_answer` | 用户回答 | `question_id, answer, answered_by` |
| `discovery_proposal` | 知识库变更提案 | `session_id, target_doc, action∈{add,modify}, content, evidence[], status∈{pending,approved,rejected}` |

**三类信息严格区分（守知识污染红线）：**
- `fact`：AI 从代码/文档中**确认**的内容（有 evidence 引用）。
- `confirmed`：用户**明确告诉** AI 的内容。
- `speculation`：AI 根据代码/文档**推断但未确认**的内容。
- **规则**：`speculation` 可存在于 session，但**禁止自动写入 `KnowledgeDocument`**；只有 `fact`/`confirmed` 或经用户批准的提案才能晋升为正式知识。

**状态机（`discovery_session.status`）：**
- `in_progress`：学习中
- `needs_user_input`：等待用户回答
- `review_required`：等待用户审核知识提案
- `completed`：本轮学习完成（**不等于项目已完全理解**）
- `paused`：暂时中断

**认知成熟度**：用多维度状态表达（背景 / 架构 / 当前情况 / 需求 / 决策 各自成熟度），**不用单一百分比**。

**session 不绑 Codex 实例**：可由多个来源贡献，甚至 Web 起头、Codex 续同一 session（`source` 记录每条产出来源）。

---

## 四、Web 入口（P0，部分已落地）

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

3. 把现有 `draft_ready + unknowns` **升级为 session 驱动的访谈 UI**：findings / questions / answers 可视化，用户逐条回答。
4. 审核 `DiscoveryProposal` → 批准后实体化为知识文件（复用 `propose_document`）。

---

## 五、Codex `/pm-learn` Skill（P1，executor-neutral）

交付物：一份 `SKILL.md`（命令式，名 `/pm-learn`），装进任意 MCP-capable 执行端（Trae / Codex / Claude Code / Cursor …），**禁止写死 Trae 专有 API**（守 §3.4 工具无关性 A1–A6）。

```
/pm-learn
 ├─ MCP 获取项目接入 Session（pm_discovery_get）
 ├─ 读取项目已有知识（pm_discovery_context）
 ├─ 分析当前 Git 仓库（grep / 读文件 / 读 README* / docs/**）
 ├─ 检查已有文档和代码
 ├─ 与用户澄清（多轮）
 ├─ 生成知识变更提案（DiscoveryProposal）
 ├─ MCP 提交 Discovery 结果（pm_discovery_submit_*）
 └─ AI-PM 验证并更新项目知识
```

**重要边界：Skill 不直接拥有项目知识库的最终写入权。** 它只能提交：
- 新发现的信息（finding）
- 用户确认的信息（confirmed）
- 建议新增 / 修改的知识文件（proposal）
- 证据来源（evidence）
- 尚未解决的问题（open-questions）

最终**验证、版本管理、正式入库由 AI-PM 负责**，避免 Codex 与 Web 双端改同一文件造成冲突。

---

## 六、MCP 接口（新增 `pm_discovery_*` 语义工具）

> 命名沿用既有 `pm_<verb>_<noun>`（如 `pm_submit_document`）；`pm.discovery.*` 为讨论用点分简写。命名最终需与 `ai-pm-mcp-executor-positioning-v1` Q1（`pm_*` vs `task.*`/`knowledge.*`）一并冻结。

| MCP Tool | 作用 | 落到既有链 |
| --- | --- | --- |
| `pm_discovery_get` | 获取项目认知 Session | 读 `discovery_session` |
| `pm_discovery_context` | 获取已有知识、仓库信息、待确认问题 | 读 knowledge + session |
| `pm_discovery_submit_findings` | 提交 AI 分析发现（含 kind + evidence） | 写 `discovery_finding` |
| `pm_discovery_ask_question` | 提交需用户澄清的问题 | 写 `discovery_question` |
| `pm_discovery_submit_answers` | 回传用户回答 | 写 `discovery_answer` |
| `pm_discovery_submit_proposal` | 提交知识库变更提案 | **复用 `propose_document`** 知识提案链 |
| `pm_discovery_complete` | 提交本轮学习完成状态 | 置 `status=completed / review_required` |

所有写回均为 **propose，非直写**（守 §4.2 污染红线）。

---

## 七、持续认知过程（非一次性）

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

## 八、多 Agent 认知分工（V1 不实现，预留）

未来一个项目可有多个 Agent 各负认知任务：

| Agent | 负责学习 |
| --- | --- |
| 产品经理 Agent | 业务背景、用户、需求、规划 |
| 架构师 Agent | 系统架构、模块关系、技术决策 |
| 技术负责人 Agent | 代码现状、技术债务、开发规范 |

由项目负责人 Agent 汇总。V1 不并行访谈，但 `discovery_finding` / `discovery_proposal` 预留 `actor_role` 字段以支持后续分工。

---

## 九、V1 实施顺序与 YAGNI

| 优先级 | 功能 | 说明 |
| --- | --- | --- |
| P0 | Project / Repository 接入 | 已落地（0008/0009/0019） |
| P0 | `DiscoverySession` 数据模型 | 新增 `0022_discovery.sql` |
| P0 | Web 项目访谈 | 扩展现有 import 访谈 UI |
| P0 | MCP `pm_discovery_*` 接口 | 新增工具面 |
| P1 | `/pm-learn` Skill | executor-neutral SKILL.md |
| P1 | Git 仓库结构与文档分析 | 复用 `RepoAnalysisModel` |
| P1 | 知识提案及审核 | 复用 `propose_document` |
| P2 | 多仓库架构关联分析 | 逐步增强 |
| P2 | 持续项目认知更新 | 后续增强 |

**V1 可限制代码分析深度，但数据库与 Session 模型须支持多仓库。**

**YAGNI（v1 不做）**：不并行多 Agent 访谈、不用单一百分比成熟度、不双端直写知识、不自动终止 session（由用户在 Web 点批准）。

---

## 十、开放待裁

| # | 问题 | 影响 | 建议 |
| --- | --- | --- | --- |
| Q1 | `DiscoverySession` 是新建独立表，还是把现有 `repo_import` 直接扩展为统一 session（加 `source`/`status`/`kind`）？ | 迁移与既有 import 测试兼容性 | 倾向**新建 `discovery_session`**，让 `repo_import(source=WEB)` 作为首个 producer 挂接，避免撼动已验收的 import 链路 |
| Q2 | MCP 命名 `pm_discovery_*` 与定位文档 Q1（`pm_*` vs `task.*`/`knowledge.*`）如何统一 | 对外契约 | 先按 `pm_discovery_*`，待 Q1 裁决时一并收敛 |
| Q3 | `discovery_finding.kind` 的 speculation 在 UI 如何呈现与晋级 | 用户体验 / 知识可信度 | 默认折叠为「待确认」，晋级需用户显式确认 |
| Q4 | 多仓库关联分析（P2）的边界 | 复杂度 | 留待 P2 单独设计，本 V1 仅保证数据模型可扩展 |

---

_本方案为产品设计参考，具体实施请结合公司合规与法务要求确认。_
