---
name: pm-learn
description: AI-PM 项目认知采集（/pm-learn）。通过 MCP 工具 pm_discovery_* 完成「读码 → 澄清 → 提交」，把对当前仓库的理解整理为 AI-PM 平台的 Discovery 发现、澄清问题与知识提案，经用户在 Web 确认后沉淀为正式项目知识。
---

# /pm-learn — 项目认知采集

一次 `/pm-learn` = 一轮项目认知采集。你（执行端 Agent）是采集者；
AI-PM 平台是唯一的知识入库方。所有写回都是 propose-only。

## 适用场景

- 老项目接入：平台 Web 端已建项目并导入仓库，需要在本地深入读码补认知。
- 日常认知更新：代码结构变化后，补一轮发现与澄清。

## 前置检查（未激活时给指引，不要抛错）

1. **MCP 未连接**（工具列表里没有 `pm_discovery_*`）：
   停止执行，向用户输出配置指引：
   - 平台 MCP 端点：`<AI-PM 部署地址>/mcp`（Streamable HTTP，Bearer 认证）；
   - 在执行端的 MCP 配置（如 `.mcp.json`）中登记该端点与 API Key；
   - 配置完成后重新加载执行端，再运行 `/pm-learn`。
2. **无活跃 Session**（`pm_discovery_get` 返回 not found）：
   停止执行，指引用户先在 Web 控制台「项目认知访谈」中开始一次会话
   （或先完成仓库导入），再运行 `/pm-learn`。
3. **工作目录不是目标仓库**：向用户确认要分析的仓库路径后再继续。

## 流程

### 1. 取 session

调用 `pm_discovery_get`（不传 `session_id`，自动解析项目活跃会话）。
记录 `session_id`、`status` 与已有 `findings` / `questions`。

### 2. 读项目已有知识与上下文

调用 `pm_discovery_get_context`，拿四样东西：

- `documents`：已有知识文档（避免重复采集已知事实）；
- `repositories`：项目关联的仓库（多仓库时逐个分析）；
- `open_questions`：等待用户澄清的问题（Step 4 的主要输入）;
- `session`：本轮采集归属。

### 3. 分析本地 Git 仓库

用本地文件读取与检索（执行端原生能力，不依赖平台）：

- 先读 `README*`、`docs/**`、构建与依赖清单（`Cargo.toml` / `package.json` / `go.mod` …）；
- 再按模块读核心代码入口、路由/接口定义、数据模型；
- 产出两类候选发现：**已观察到的事实**（observed）与**推断**（speculation，
  带 0–1 的 confidence）。每条发现必须记录出处（`file` + 一句话说明）。

### 4. 与用户澄清（多轮）

针对 `open_questions` 和 Step 3 中无法从代码确定的问题，用执行端原生
对话能力向用户提问。然后一次性回传：

- `pm_discovery_submit_answers`：
  - `questions` 载荷：把 AI 新提出、尚未回答的问题落库（可带
    `why_blocking`，可带 `finding_id` 锚定某条发现）；
  - `answers` 载荷：把用户已回答的内容回传（`question_id` + `answer`）。

用户拒绝回答或表示不确定的问题：保持 `questions` 落库，不要编造答案。

### 5. 提交发现

调用 `pm_discovery_submit_findings`：

- `kind` 取值：`observed` / `user_confirmed` / `speculation` / `contradicted`；
- 每条带 `evidence: [{repository, file, description}]`，出处必须真实来自
  Step 3 的读码记录；
- `contradicted`（与已有知识冲突的发现）不要试图直接修改知识——冲突只能
  通过 Step 6 的提案处理；
- `speculation` 带 `confidence`，且默认会被标记为「待确认」，由用户裁决。

### 6. 提交知识提案（propose-only）

若本轮产生了值得沉淀的知识（新增文档或修订既有文档），
调用 `pm_discovery_submit_proposal`：

- `action: add`：新文档（`doc_key` + `doc_type` + `proposed_content`）；
- `action: modify`：修订既有文档——必须带 `document_id` 与
  `base_revision_id`（从 `pm_discovery_get_context` 的 documents 或平台读出）；
- 平台会走既有质量门与红线审查：提案可能被自动提交、挂起待人工确认或拒绝；
  `base_revision_id` 失效时返回 conflict——此时重新读取最新版本后可重试
  **一次**，再次失败即停止并汇报。

注意：你没有知识库的最终写入权。不要试图绕过该工具直接写文档。

### 7. 结束本轮

调用 `pm_discovery_complete`：

- 全部问题已回答、无遗留 → `status: REVIEW_REQUIRED`（推荐，交由用户审核）；
- 本轮只是阶段性提交，之后还会继续 → 保持会话不结束（支持跨天增量提交）。

### 8. 汇报

向用户输出：本轮提交的 findings / questions / answers / proposals 计数、
需要用户在 Web「项目认知访谈」中确认的事项清单、以及未解决的问题。

## 红线（不可违反）

1. **不拥有知识库最终写入权**：只提交发现、回答与提案；正式入库由 AI-PM 负责。
2. **证据优先**：没有代码出处的「发现」不提交；禁止编造 file 路径或内容。
3. **executor-neutral**：只依赖 MCP 工具名（`pm_discovery_*`），不调用任何
   特定执行端的私有 API；换一个执行端本文件同样可用。
4. **不做无人值守**：每次 `/pm-learn` 由人触发；澄清必须等真实用户回答。
5. **不做跨仓库架构关联分析**（跨仓调用关系 / 依赖图 / 部署拓扑）——属 P2 范围。

## 实测清单（装进新执行端时走一遍）

1. 未配置 MCP 时运行 → 应得到配置指引而非报错堆栈；
2. 配置 MCP 后无 session 运行 → 应得到「先去 Web 开始会话」的指引；
3. 正常轮：读码 → 澄清 → `pm_discovery_submit_*` → Web 确认闭环；
4. 提交一条 speculation → Web 发现卡片出现「待确认」徽标。