# S01 补充裁决稿：再分析流程与业务身份认知闭环（V1）

> 状态：**待批准**（每项裁决 D-1~D-4 需用户逐条批准；批准前不实施）
> 日期：2026-11-15 · 来源：用户场景讨论（项目初始化如何形成 LLM 对项目的认知）+ full-dev 实测发现
> 定位：不推翻既有权威（final-baseline V1.2 / I6-B / S01-03 / S01-04 / discovery-v1），只补齐其未覆盖的四项裁决，并收编一处已登记冲突（见 qa/03 §6 C-13）。

## 0. 背景：用户心智模型与现状的差距

用户模型（原文归纳）：

1. 项目刚创建时 LLM 对项目一无所知；
2. 首次接入仓库分析 = 读 commit + 通读仓库文档（含 docx）→ 人工评审后写入 KB/PC → 记录「最后一次分析的 commit hash」；
3. 再次分析可选：a) 增量（自上次 commit 起只分析新增 commit 与改动文档）或 b) 全量重分析。

对照结论（Explore 全量通读 S01 docs/qa/ui，2026-11-15）：

| 模型点 | S01 状态 |
|---|---|
| 1 一无所知起点 | 未明文；等价红线存在（S01-BR-011/ADR-041：零仓库零设备可用） |
| 2 评审后写 KB/PC | ✅ 完全一致（P0 红线 ×5 处） |
| 2 通读全部文档/docx | 🔴 只裁决到「README/架构文档/项目文档」粒度；docx 全目录零命中；实现 `repo_facts::is_doc_candidate` = 根级+docs/ 的 md/mdx/rst/txt |
| 2 记录最后分析 commit | 🔴 S01 刻意拆成 Default Branch / Observed HEAD / Confirmed Commit 三概念，明确 `head_commit_sha`「不是分析时间」（s01-i5-code-facts L55）；「上次分析到哪」不存在 |
| 3a 增量 | 🔴 未覆盖；2026-11-13 手动增量分析端点存在但锚点=Confirmed/Observed（本次基准），且采集仍为全量 300-commit 窗口重跑 |
| 3b 全量重分析 | ⚠️ S01 内部矛盾：Cognition Run/Re-scan 可重复（S01-04 §10.1、S01-03 §8）vs 冷启动 Import 一次性（I6-B C-1，applied 后 409 `project_not_empty`）vs analyze 端点事实（可反复 202）。已登记 qa/03 C-13 |

另：实测发现「业务身份」认知链未闭环（本稿 D-4）——仓库分析只产技术侧认知，`product_profile`/`product_vision` 自 bootstrap 起空置（lifecycle=identity），项目是图书管理系统还是 OA 这类信息无处沉淀。

---

## D-1 · 分析锚点：`last_analyzed_commit`（新增第三类 commit 概念）

**问题**：增量分析（D-2）需要「上次分析到哪里」，现有三概念（Default Branch / Observed HEAD / Confirmed Commit）都不表达它。

**推荐方案**：
- `core.repository`（或其读模型）新增两列：`last_analyzed_commit_sha` + `last_analyzed_at`。
- 写入时机 = 分析任务（冷启动 Import 或手动 analyze）到 **COMPLETED（产 draft）** 时写入本次基准 commit（即 `repo_import_source.head_commit_sha` / analyze 端点受理的锚点 commit）。apply 与否不影响——分析完成即「已分析到」。
- 语义边界：与 Observed HEAD（核验回写）、Confirmed Commit（人工基准）正交；三者并存，UI 分列展示（沿用 S01-03 §5.1 三概念表式，扩为四行）。

**备选**：不加列，从 `integration.repo_import` 历史派生（最近一次 COMPLETED 任务的锚点）。缺点：查询绕、手动 analyze 与冷启动混在两张表。

**影响面**：repo 表 +1 迁移（新列，无数据迁移）；analyze/kb_analyze 完成路径各 +1 写入；仓库设置页 UI 展示一行。小。

## D-2 · 再分析：增量 vs 全量的划界与触发

**前置缺陷修复（必须先做，已在记忆/缺陷清单挂账）**：draft_ready 的导入不释放初始化名额 → 再分析 500（`uq_project_initialization_operation_active`）。正解二选一：apply/proposed 终态释放，或 start_import 撞唯一索引时返回 409+可读文案。**本项未修，增量/全量都跑不起来。**

**推荐方案**：
- **增量（默认）**：输入 = `(last_analyzed_commit, 本次锚点]` 的 commit 窗口 + 该区间内 `git diff --name-only` 命中的文档集（采集范围见 D-3）；LLM 层另附「既有 PC 认知摘要」做 merge 式重析（不是从零推）；产出仍走提案门（不变）。
- **全量（显式选择）**：等同现状（300-commit 窗口全采集 + 全量重析）。
- 触发 UI：仓库页「重新分析」旁增选择（默认增量；「整库重析」需二次确认）。
- 归属收编：冷启动 Import 保持一次性 bootstrap（I6-B C-1 不动）；「再分析」正式归属 = S01-03 §8 Re-scan 长期操作 + analyze 端点，两个流程分立（解 C-13）。

**备选**：永远全量（无增量实现）。缺点：浪费且大仓 LLM 成本高；用户模型 2.2.a 落空。

**影响面**：`repo_facts::collect` 增加区间参数（git log/diff 均现成）；LLM 提示词加「既有认知合并」段；前端一个选择控件。中。

## D-3 · 首次/全量分析的文档阅读范围（含 docx）

**问题**：S01 未定义扩展名清单；实现只读 md/mdx/rst/txt（根级+docs/）；docx 完全空白（但 Document Processing V1 已支持 docx/pdf 等八格式解析、KB 摄入已支持 docx）。

**推荐方案**：
- 采集范围维持现有路径规则（根级 + `docs/`，排除 node_modules/target/vendor/dist/.git），扩展名扩为 **md/mdx/rst/txt + docx/pdf**。
- docx/pdf **先经 docproc 解析为纯文本**再进 LLM 输入（不传二进制/原文）；解析失败的单文件跳过并计入 `truncated` 事实（不阻塞分析）。
- 数量/字节上限沿用 `repo_facts::Limits`（含 docx/pdf 各自上限），触顶照记「采集触顶截断」。

**备选**：任意深度全部 md。缺点：噪音（前端组件库、子模块文档），现有根级+docs/ 规则已是降噪裁决。

**影响面**：`is_doc_candidate` 扩展名 + docproc 文本抽取接线（worker 侧）；提案文案自动带上「N 个文档（含 M 个 office 文档）」。中。

## D-4 · 业务身份闭环：项目是什么，填到哪、谁来填

**问题**：「图书管理系统还是 OA、用户群体、背景、目标」读代码读不出来。三个座位已建但无 producer：
1. `product_profile` / `product_vision` PC 单例骨架（bootstrap 建，至今 identity 空置）；
2. I4 Project Goal（`doc_key='project-goal'`，占位「待补充」）；
3. Discovery 访谈（`ai-pm-project-discovery-v1.md` 已裁决「Onboarding = Project Discovery = 代码分析+文档阅读+**人机对话**」，阶段表第一行即「项目背景→project-overview.md」；实现 auto_onboarding 已能自动开 `INITIAL_ONBOARDING` 会话+派生必答问题，但 orchestrator 开关默认 off 且**本批 PC/KB 零写入**）。

**推荐方案（复用 Discovery 骨架，零新概念）**：
1. 打开自动访谈（`AI_PM_DISCOVERY_ORCHESTRATOR=on`，或为访谈单独开闸）；分析完成 → 自动开 `INITIAL_ONBOARDING`（已实现）。
2. 新增**身份综合轮**（discovery orchestrator 后续轮次，D-DK-10 之后）：LLM 输入 = 访谈答案 + 仓库分析产物（技术栈/模块/文档摘要）+ README/项目文档；输出三份**草案提案**：
   - `product_profile` 修订（系统定位：是什么系统、核心域、技术形态一段话）；
   - `product_vision` 修订（目标用户、要解决的问题、背景）；
   - `project-goal` 文档修订（按 I4 既有结构填充，走知识 8 态管线）。
3. 全部经既有提案门（PC P4/P5 + 知识 DocumentProposal）人工批准后转正——**不新增旁门**，维持「AI 提案、人类确认」红线。
4. UI 落点：总控台初始化卡片「项目认知」项从「进行中」升级为展示身份三件套的就绪度（profile/vision/goal 各一个 ✓）。

**填入位置的直接回答**（用户原问）：不是 project 表加字段，而是——业务身份 → PC `product_profile`/`product_vision`（经提案转正）；项目目标 → `doc_key='project-goal'` 知识文档（经提案发布）；业务背景叙述 → 访谈产出的 `project-overview.md` 进 KB。

**备选**：不做 LLM 综合，访谈答案由人工自行整理成文档（退化为表单，身份三件套仍靠手填）。

**影响面**：discovery orchestrator 一轮新增（最大件）；PC/知识提案接线复用既有；前端初始化就绪度展示小改。中大。

---

## 实施顺序建议（D-1~D-5 全部批准，2026-11-15 定稿）

1. **D-5 报告实体与门**（含 init-slot 500 正解：open 报告 = 设计化门，409 带出路）——其余各项的载体，最先做；
2. **D-1 锚点**（挂在报告关闭时固化）；
3. **D-2 增量划界 + D-3 文档范围**（同批，都在采集层；门与锚点就绪后增量才有意义）；
4. **D-4 身份闭环**（独立批，价值最大但件最大）。
   - D-5 第 3/4 点（知识审阅报告页签 + review-center 关联）可随 D-5 后端同批或紧随其后一批落地。

## 待批准清单

| # | 裁决项 | 推荐 | 备选 | 状态 |
|---|---|---|---|---|
| D-1 | 分析锚点列 | repo 表加 `last_analyzed_commit_sha`（COMPLETED 时写） | 从 import 历史派生 | ✅ 已批准（2026-11-15） |
| D-2 | 增量/全量 | 增量默认（commit 区间+diff 文档+认知合并），全量显式；冷启动与再分析分立 | 永远全量 | ✅ 已批准（2026-11-15） |
| D-3 | 文档范围 | md/mdx/rst/txt + docx/pdf（docproc 先解析、失败跳过） | 任意深度 md | ✅ 已批准（2026-11-15） |
| D-4 | 身份闭环 | Discovery 访谈开闸 + 身份综合轮起草三提案（全走提案门） | 人工自填 | ✅ 已批准（2026-11-15） |
| D-5 | 分析报告实体与生命周期 | 见下节 | — | ✅ 已批准（2026-11-15，用户同批追加） |
| D-6 | 统一人工待办中心 kind 目录 | review-center 演进为项目级待办中心（六 kind） | 维持三类不扩 | ✅ 已批准（2026-11-15） |

---

## D-5 · 仓库分析报告：实体、生命周期与审阅统一（2026-11-15 用户裁决）

> 用户原话归纳：「添加仓库分析报告的实体，PC 和 KB 等等都属于这个仓库分析报告。只有这个报告没有 close，不允许再次分析仓库；报告关闭后才能再次启动分析。知识审阅要能看到哪些报告需要审理、历史报告有什么——先报告，后报告内容，在知识审阅里一条条通过。人工审阅中应该要关联到这些审阅资料。」

**现状核对（2026-11-15 实查）**：报告实体不存在（产物靠 intake_id/pc.source 松散关联）；再分析只有进行中 409，draft_ready 卡初始化名额的 500 缺陷是该语义的"事故版"；知识审阅无报告层列表；review-center 聚合 discovery finding + ai.document_proposal + delivery，**不含 pc.proposal**。

**裁决内容**：

1. **实体**：新增仓库分析报告（`repo_analysis_report`）：一次分析运行 = 一份报告，聚合本次全部产物——KB 报告修订（`repo-{rid}` 文档的本次修订）、PC 修订集、PC 提案（经 `pc.source` 关联）。报告携带：仓库、commit、intake_id、产物计数快照、状态、closed_at/by。
   **状态机（2026-11-15 用户补裁）**：`analyzing`（分析中——任务受理即建报告行，进度复用既有 intake 阶段/百分比轮询）→ `analyzed`（分析完毕——draft 产出，报告 open 待审）→ `closed`（已关闭——门释放）；失败/取消 → `failed` / `cancelled`（终态，**不占门**，仅留历史）。**门语义**：`analyzing` 与 `analyzed` 期间该仓库禁再分析（409 带报告链接与当前状态）；`failed`/`cancelled`/`closed` 放行。
2. **生命周期与门**：`analyzed`（分析完毕/open）期间该仓库禁止再分析（analyze 端点 409 `report_open`，附报告链接与状态）——这同时是 init-slot 500 缺陷的正解（卡门从 bug 变设计，且给可读出路）。关闭 = 用户显式动作；关闭时未决项自动处置（PC 提案→withdrawn、KB 草稿→rejected），确认框列明各项计数。D-1 锚点在报告关闭时一并固化（`last_analyzed_commit` = 关闭报告的 commit）。
3. **知识审阅（/project/knowledge）**：新增首位置「分析报告」页签：报告列表按状态分区——**分析中**（带阶段/进度，复用既有 3s 轮询）/ **分析完毕待审理**（有未决项）/ 已关闭（历史）/ 失败（含失败原因）→ 点开报告 → 内容区逐条审阅（KB 修订确认 / PC 提案批准驳回——复用既有行内详情与 P4/P5 端点）→ 全部处置后「关闭报告」。
4. **人工审核中心（review-center）关联**：报告未决项进入审核中心聚合（新增 kind 或复用 proposal 卡片带 report 深链 `?report=`），点击直达知识审阅对应报告——两个面单一决策源，不另设决策端点。

**影响面**：+1 迁移（报告表）；analyze 端点门改造（409 语义）；报告关闭事务（含自动处置）；知识审阅新页签 + 报告详情；review-center 接一类卡片。中大（是 D-1/D-2 的载体，实施顺序提前到 D-2 之前）。

---

## D-6 · 统一人工待办中心：review-center kind 目录（2026-11-15 用户裁决）

> 用户诉求归纳：「用户的任务需要一个地方全部归纳——可能 runner 坏了要你修，可能 AI agent 在某台电脑上需要和你协作，可能发生分析决策，LLM 有疑惑需要决定。」

**现状核对（2026-11-15 实查）**：人工注意力分散四处——review-center（三类待决）/ devenv 页（runner 故障只在那一页）/ AI Chat（agent 需要人参与无待办概念）/ 任务页三态（总控台只有计数）。`/inbox` 是公告广播（announcements 已读），**不是待办**，勿混淆。

**裁决内容**：review-center 演进为**项目级统一人工待办中心**，kind 目录定版（插件式扩展，每类遵守既有纪律：单源计数=总控台 `human_action_summary` 同源、决策走各自域端点、卡片深链回各域页面）：

| # | Kind | 数据源 | 状态 |
|---|---|---|---|
| 1 | Finding（LLM 疑惑/发现） | `discovery.discovery_finding` + verification | ✅ 已有 |
| 2 | DocumentProposal（知识文档提案） | `ai.document_proposal` | ✅ 已有 |
| 3 | Delivery（验收审批） | `delivery.task` | ✅ 已有 |
| 4 | **ReportItem（分析报告未决项）** | D-5 报告的未决 PC 提案/KB 修订，深链 `?report=` 直达知识审阅报告 | 随 D-5 落地 |
| 5 | **RunnerHealth（环境要修）** | 设备/runner 失联（devenv 读模型）+ 任务执行 `needs_attention`/`blocked` 卡片化 | 新增 |
| 6 | **AgentAssist（agent 求助）** | AI agent 主动请求人介入的协作标记（源=协作会话"需要参与"标记；随 AI 协作工作台深化批落地，可后置） | 新增（可后置） |

**边界**：不把公告（/inbox）、普通通知塞进待办中心——待办的准入 = 「有人工决策/修复动作且阻塞某条流程」。

**影响面**：kind 4 随 D-5；kind 5 = devenv 读模型 + 任务三态接卡片（中）；kind 6 依赖协作会话标记先落地（后置）。
