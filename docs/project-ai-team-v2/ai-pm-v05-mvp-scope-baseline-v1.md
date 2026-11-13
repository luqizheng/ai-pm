# AI-PM V0.5 MVP 范围基线 V1.0

| 文档属性 | 内容 |
| --- | --- |
| 文档编号 | V05-00 |
| 版本 | V1.4（V1.3 + 第五批收纳：DK-0 对话编排全链（DK-A~F）+ 对话上下文整合批（Batch1~4）+ 前端 DK-F Stage 2；新增 §五之二/§九 11~12/§八.9/§六 WP-8~9/V-05-8） |
| 类型 | MVP Scope Baseline（产品第一个可演示闭环的范围基线） |
| 日期 | 2026-11-06（V1.4 修订 2026-11-09） |
| 用户裁决（第一批） | Big View = 产品统一领域模型总图；Runner = 用户 PC 客户端软件；简化版调度进 MVP |
| 用户裁决（第二批） | V-05-1 = **升级 devenv.device**；V-05-3 = **不取代、共存**（project_role 独立新实体，member.role 原样保留）；V-05-4 = **冻结 ACP V1.3.1 基线**（用户无异议按推荐）；V-05-5 = **ZCode / Codex / DeepSeek Harness 优先** + Runner 走 MCP 自领（无异议）；**Knowledge Base 纳入 V0.5**（目标 4）；V-05-2 展开解释后终裁 |
| 用户裁决（第三批） | 目标 2 正式命名 = **Product Cognition & Knowledge Base — Product Definition V1.0**：Product Cognition（独立产品能力：建立/维护/验证产品整体认知）· Product Big View（其核心全貌视图）· Product View（观察视角：业务能力/功能模块/业务流程/页面交互等）· Knowledge Base（并列独立能力：资料管理/知识提取/Discussion Memory/知识检索） |
| 用户裁决（第四批，2026-11-08） | **V0.5 增列「PC/KB 应用完善」**：KP-1（PC-01B 正式模型实施）/ KP-2（PC-01C 确认闭环）/ KP-3（KB 三层最小面）；PC-I6 全验收面剩余等显式切出 V0.6（§3.3/§八.8）。同日 V-05-6/7 实质关闭（Q-PC-01~13 已裁） |
| 用户裁决（第五批，2026-11-09，按推荐） | **收纳对话化入口三件**：A1 DK-0 对话编排全链（DK-A~F，独立节 §五之二 横切目标 2/3/4）+ A2 对话上下文整合批（Batch1~4，作为 A1 伴生批一并入册）+ A3 前端 DK-F Stage 2；**B1** 两 Demo 场景写入 §九 故事线 11/12（验收=DK-F §8.3 30/30 本机栈）；**B2** Stage 3（.10 部署批）**属 V0.5 收官内**挂账单批（三开关 off 时 .10 上对话编排不可见）；**B3** 三开关**代码默认 off 维持**，.10 部署 env 配 on |
| 目标 1 状态 | ✅ **2026-11-08 闭环**：检查点 1（注册→建团→建项目→关联仓库→PC/KB draft 全链 PASS，8d81fbd）+ 检查点 2 R4（.59 Runner 真二进制 cpk 全链→平台远端只读验证 accepted，三边界全关，**ACP-09 正式关闭**，`docs/acceptance/checkpoint2-r4-acceptance-record-v1.md`） |
| 前置资产 | V2 架构基线 V1.0（Q1=A/Q2=B/Q3=A）· V2 代码对照核验报告 V1.0 · S03-ACP V1.3.1（独立评审通过）· S01-08 知识库与 Git 备份设计稿 V1.0（四裁决已定，待实施） |
| 优先级裁决 | S01 后续切片降级为非迫切；S03-00 V1.2 审计并行推进、不阻塞 V0.5 |

---

## 一、V0.5 一句话定义

用户在 Platform 上为项目**创建 AI Team**（定义 Roles、授权自己的 Agent Client），通过部署在本机的 **ai-pm-runner** 驱动本地 AI 工具（Codex 等）执行任务；执行中 Agent 可发起人工介入，人类经 REST/UI 回复后任务在**同一 Run 内 Continuation 恢复**，直至交付验收。

这是产品的 MVP：三方通信（目标 1）是通路，AI Team 创建（目标 3）是入口，Product Cognition（目标 2，Product Big View 为其核心全貌视图）与 Knowledge Base（目标 4）是并列的认知/知识两大独立能力；V0.5 统一数据模型（WP-1b）保证全系统建在同一个模型上而不是四张皮。第五批收纳（V1.4）增补：**AI 对话框升级为项目的对话化入口**（§五之二）——访谈沉淀需求与知识、AI 提议一律经人类确认生效。

---

## 二、三方架构（目标 1）

### 2.1 事实澄清：Runner vs Worker（2026-11-06 代码核实）

| 项 | 现状 |
| --- | --- |
| ai-pm-worker | **服务端**容器内编排组件（六条循环：analysis job / repo_import / outbox / Run 调度执行 / review / 失败判定）。Model B 时经 `codex_executor.rs` 在**服务器本地磁盘** spawn Codex 子进程（镜像仓 `:209`、spawn `:472`、lease_token 注入 `:341`）。**不是用户 PC 软件。** |
| 用户 PC 现状 | 只有 Model A：外部 CLI 以真人 API Key 直连 `/mcp` 自领（`/install/codex` 引导）。执行端凭据 = 真人 API Key，**无独立 Client 身份层，无 PM-AI 客户端软件**。 |
| ai-pm-runner（新建） | **客户端软件，部署用户 PC**：持 Client 级凭据，向 Platform 自领任务，本地 spawn/收割 AI CLI（Codex / OpenCode / DeepSeek Harness）。 |

Runner 是 V2 §7.2「Worker / Execution Bridge」定位的**客户端化落点**：职责不变（执行桥梁，不做项目智能决策），部署位置从服务器搬到用户侧。

### 2.2 三方职责

```text
Human ──REST/UI──> AI-PM Platform（控制平面：AI Team / 调度 / 治理 / 知识协调）
                        │
                        │ MCP 四工具协议（复用，零新协议）+ 最小交互/命令面
                        ▼
                   ai-pm-runner（用户 PC，Client 凭据持有者）
                        │ Adapter（进程 spawn/收割/终止/Checkpoint/interaction 上报）
                        ▼
                   Agent Client（本地 CLI：Codex 先行；OpenCode / DeepSeek Harness 框架就位）
```

| 通道 | 协议 | 现状 |
| --- | --- | --- |
| Platform ↔ Runner | MCP 四工具（pm_claim_task / pm_get_run_context / pm_report_progress / pm_submit_result）+ Interaction/命令查询面 + discover/续领/checkpoint（26 工具） | ✅ **已全落地**（迁移 0046~0055；.59 真机三边界全关，ACP-09 关闭） |
| Runner ↔ Agent Client | Adapter（stdin/args/env/文件约定；spawn/ExitOutcome/Job Object/进程组能力自 `codex_executor.rs` 演化） | ✅ **V-05-5 落地**：ZCode → Codex → DeepSeek Harness 优先级链 + 模板库（CLI 真机档待 CLI 可用环境，其余协议层已验） |
| Human ↔ Platform | REST/UI 回复入口（ACP §8.3 六项权限检查 fail-closed） | ✅ **已建**（ACP-03 迁移 0047 + .59 真机验证） |

### 2.3 与 S03-ACP V1.3.1 的关系

- ACP V1.3.1（Task-Based Continuation，Model B）独立评审**通过**（P0×3 闭合、D-01~D-07 全落地），是 V0.5 人工介入/恢复链的设计基线，V0.5 启动时正式冻结（V-05-4）。
- 评审遗留两项进实施设计闭合：**N-1**（人工等待窗口与自动调度/回收互斥，ACP-427/428）与 **N-2**（`interaction.required` 物理通道）。Runner 客户端化后 **N-2 从跨进程协议问题简化为 Runner 内 Adapter 侦测约定**（CLI 输出/标记文件→Runner 上报），实现成本显著下降。
- N-6 的"RESPONDED 未消费合法稳态"在 Runner 形态下由 Adapter 持有注入证据（初始输入写入成功 + 进程存活），证据链更直接。
- ACP 中 "Worker" 一词在 V0.5 语境下由 **Runner** 承担；`worker_command` 表 + 命令查询面同样服务 Runner（终止/继续/回复投递）。

---

## 三、Product Cognition & Big View（目标 2，正式命名 V1.2）

### 3.1 正式概念关系（产品定义名：Product Cognition & Knowledge Base — Product Definition V1.0）

| 概念 | 正式定义 |
| --- | --- |
| Product Cognition | 独立的产品能力，负责建立、维护和验证产品整体认知 |
| Product Big View | Product Cognition 的核心全貌视图 |
| Product View | 对 Product Cognition 的不同观察视角（业务能力、功能模块、业务流程、页面交互等） |
| Knowledge Base | 与 Product Cognition 并列的独立产品能力，负责资料管理、知识提取、Discussion Memory 和知识检索 |

### 3.2 与既有资产的关系

- **设计基础**：S01-04 认知设计修订稿 V1.2（专家供稿 2026-11-05，待技术核验）的核心原则全部适用于 PC 能力：AI 自动准备材料、正式分析由人启动、认知结果逐项人工确认、已确认认知不可直接覆盖只能提案、混合 Context（核心认知默认注入 + 按任务检索）；`pm_cognition.rs`（迁移 0036，S01-04 V1 已实现，结构化事实派生不虚构 LLM）为种子。
- **层级关系已定（V-05-7 关闭，Q-PC-01~13）**：PC **全新建**——不复用 `ai.cognition_*`/`baseline_*`；S01-04 Project Cognition = **技术认知**六域，保持独立服务其自身领域（不改名、零迁移、不受 PC 表结构反向约束）；Product Plan 不属 PC（Q-PC-13），PC 仅维护对 Plan 的关系引用。原两框架映射疑虑按此口径消解。
- **权威文档族（正式稿已落盘，以其为准）**：产品定义 `docs/product-cognition-knowledge-base/product-cognition-knowledge-base-definition-architecture-v1.md` · 详细需求 V1.1 九文件（PC-FR-001~030）· 操作链终态 `docs/product/pc-kb-operation-v1.md`（commit→分析→人工确认）· 架构 `docs/architecture/knowledge-services/kb-pc-independent-services-architecture-v1.1.md`（13 ADR 全批）· PC-01B 数据模型 V1.1 七主题文件（17 表）· P2-13×PC-I6 LLM 裁决包（D-1~D-4 全裁）。

### 3.3 V0.5 范围（V-05-6 关闭 → 第四批收纳 KP 批次，2026-11-08）

正式实施基线 = **PC-I0～I6 七切片**（Q-PC-10 已裁）。现状对照（对照操作链终态文档与代码）：I0 ✅ / I1 半（0053 最小 6 表 ≠ 01B 正式 17 表）/ I2 半（来源/证据四元组有、关系面无）/ I3~I5 未动 / I6 起笔已落（f90d6f9：仓库快照 LLM 增强，迁移 0062 三模式 analysis_mode）。操作链前半场（commit→分析→落库→K2/P3 查看）已通；**断点 = 人工确认入口未建**（K2/P3 只能看不能确认）。

| 批 | 内容 | 依赖 | 兑现 |
| --- | --- | --- | --- |
| **KP-1** | PC-01B 实施：0053 最小表 → 正式 17 表收敛（对象/关系/版本成员/治理；迁移按台账取号） | PC-01B V1.1 设计已在 | PC-I1/I2 补全 |
| **KP-2** | PC-01C 确认闭环：proposal/审批端点（`pc.proposal*` / `pc.approval`）+ draft→确认→正式版本链（p4/p5 出 501） | KP-1 | 操作链终态最后一段；PC-I3 |
| **KP-3** | KB 三层最小面：k5 来源管理 + k4 Structured Memory + **Discussion Memory（落库 + 按 Task 注入）** | 独立，可与 KP-1 并行 | 目标 4 主交付物 |

**后置 V0.6（第四批显式切出）**：NO_CHANGE/CONFLICT 与冲突发现（等关系建模，AC-019 质量红线不容虚构）、Requirement/Design·ADR 影响分析族、完整性/一致性检查、主动认知维护（PC-I4/I5/I6 剩余）、Team Source Pool、知识检索全量面、KBGIT/PCGIT 正文权威、L0~L3 审批矩阵正式化。

### 3.4 原"统一领域模型总图"任务正名（与 Big View 解耦）

原 WP-1 的四路资产合并任务保留，正名为 **V0.5 统一数据模型（Platform Domain Model）**（WP-1b）：V2 §十一领域模型 + S03-ACP §12 实体表 + 既有真相源（delivery.task 13 态 / task_run 7 态 / devenv.device / team_llm…）+ 核验报告 §四禁重建区七条 → 统一模型总图 + 实体字典 + 三档标注（复用/新建/升级）。它是 V0.5 全部 DB/API 设计的**单一输入源**，**不再占用 Big View 名义**。红线不变：与核验报告 §四冲突的实体一律按报告处置。

---

## 四、AI Team 创建（目标 3）

范围 = V2.0-A/B/C/D 最小集 + 简化调度（①②③⑤）+ Project Settings → AI Team UI：

| V2 阶段 | V0.5 最小集 | 依据 |
| --- | --- | --- |
| V2.0-A Role Model | `project_role` 新实体（职责/Skills/Instructions/双 Contract/验收/约束/Status） | 核验 §六：member.role 六值混合两语义，Role=任务职责定义须独立实体 |
| V2.0-B Client Registry | Client 级凭据签发/吊销 + 能力三分（声明/验证/当前可用）+ client_type 字典；载体随 V-05-1 裁决（倾向 device 升级） | Runner 的认证身份就是 Client 身份 |
| V2.0-C Authorization | Project 授权关系 + permission_scope + 独立生命周期 + **撤销后旧凭证失效** | 现 binding 明文非授权（0027:57）；撤销链只有 Run 级 |
| V2.0-D Capability Registry | 最小集：Capability 字典 + 任务要求↔Client 能力匹配所必需的字段 | 调度②能力过滤的前置 |
| V2.0-E Scheduler（简化） | TaskReadyHandler 前插调度决策：①授权过滤→②能力过滤→③可用性过滤→⑤派发；**跳过④任务适配** | 用户裁决 2026-11-06 |
| UI | Project Settings → AI Team（Roles / Agent Clients / Task Scheduling） | V2 §十；Dashboard 不做 Client 管理页 |

**横切必须项**（核验报告 P0/P1 缺口，不补则本目标空转）：
- `delivery.task_repository` **生产写入口**（P0：授权范围源头声明无 API，全仓仅测试命中 INSERT）。
- `delivery.task_waiting` **生产写入口**（P1）+ `no_client_available` 等 reason 新枚举值（加枚举值，不是加状态——核验 §四.3）。

**V-05-3 裁决落点（不取代、共存）**：`project_role` 为全新独立实体（V2 §四十二字段结构），Task 以 `role_id` 引用它；`core.project_member.role` 六值 CHECK（0030:9，owner/member/ai_pm/architect/developer/tester）与 `ai.agent_definition` 三种子**原样保留、零迁移**。语义切分显式化：project_role=任务职责定义（谁负责什么专业工作），member.role=参与者身份（归属与访问）。命名纪律：域内两个 "role" 并存，文档与字段命名一律区分 project_role / member_role。

**V-05-1 裁决落点（升级 device）**：`devenv.device` 升级为 Client Registry 载体——加 Client 级凭据（签发/吊销）、能力三分结构（声明/验证/当前可用）、client_type 字典；`devenv.device_project_binding` 从「事实记录」升级为「授权授予 + 可撤销」（推翻 0027:57 边界注释，随迁移批修订注释）。

---

## 五、Knowledge Base（目标 4，正式定义 V1.2）

**正式定义**：与 Product Cognition **并列的独立产品能力**，负责资料管理、知识提取、Discussion Memory 和知识检索。同时是 MVP 闭环的收尾环节（S03-00 V1.2 §0.2 闭环最后一拍「知识库更新」）。

| 正式职责 | V0.5 范围 | 依据/现状 |
| --- | --- | --- |
| 资料管理 | KB-1 持久化与版本（Markdown+Git 正文权威 / PG 状态权威 / PageIndex 可重建派生，KnowledgeIndexer Adapter 隔离）+ KB-3 Git 备份（S01-08 实施）+ KB-4 浏览最小面（文件树/文档查看/版本历史） | markdown-knowledge-space 已定稿（12 节）；S01-08 V1.0 四裁决已定待实施 |
| 知识提取 | 从资料中提取结构化知识条目，经提案链人工确认沉淀；**与 PC 建立/维护链共用同一机制**（AI 提议、人类确认），零新建写入机制 | propose/confirm/veto + 质量门在跑（pm-mcp/tools.rs:155-238，核验 §四.5） |
| Discussion Memory | 会话/讨论记忆的持久化与按任务检索注入；前端载体 = AI Interaction 会话面板（feat/ai-interaction-library 已落地组件库 V1.0） | **新持久化对象**；V0.5 最小面 = 讨论记录落库 + 按 Task 检索注入 Agent Context |
| 知识检索 | 资料/知识/Discussion Memory 的统一检索入口（PageIndex 派生索引的消费方） | PageIndex 设计已定稿，索引器未实施 |

S01-08 四裁决（KB-3）：知识库仓库独立配置（R-01）·自动同步为主+手动 Sync Now（R-02）·GitHub/GitLab/Gitea/自建（R-03）·创建项目非必需（R-04）；失败不阻断、不丢数据、不自动覆盖远程冲突。

**实现现状（2026-11-08 对照 `docs/product/pc-kb-operation-v1.md` 与代码）**：三层只通中段——Extracted Content 半层（KB digest）随 checkpoint-1/PC-I6 起笔可用；Original Sources 管理（k5）、Structured Memory 评审（k4）、Discussion Memory、知识检索均未建；K1/K3/K4/K5/K6/P1/P2/P4/P5 九端点 501 骨架（`knowledge_services.rs`）。第四批收纳 **KP-3 兑现最小面**，其余后置 V0.6。

**与 V-05-2 的域边界（防两个 Git 裁决互相污染）**：知识库同步 = **平台侧 push**（平台持知识库仓库凭据、系统自动安排同步——S01-08 R-02 本义）；V-05-2（代码仓库 Client 自管 push）**不适用**知识库域。实施时 git 能力分两档：只读验证面（现有 `git_access.rs`，显式无 push）+ 知识库同步 push 面（WP-7 新增，仅限知识库仓库）。

---

## 五之二、对话化入口（目标 2/3/4 横切，2026-11-09 第五批收纳）

**定位**：AI 对话框（载体 = `ai.conversation`，迁移 0060）从纯聊天升级为项目的**对话化入口**——按载体 a+ 裁决（D-DK-1），每个线程 = 唯一会话目录，chat 段落 `conversation_message`、访谈段（interview）落 `discovery_message`，**chat→interview 单向升级永不回落**；判定在服务端（前端只消费发送落点回包 `landed`）。横切三目标：访谈产物沉淀为需求修订/认知/知识提案（目标 2/4，写回经统一应用服务 fail-closed，AI 无确认权）、知识注入支撑项目问答（目标 2/4）、任务分解承接调度（目标 3）。核心原则不变：**AI 提议、人类确认、已确认不可直接覆盖**。

**裁决依据**：D-DK-1~11（设计稿 `docs/product/ai-pm-dialog-orchestration-design-v1.md`）+ 整合批裁决（D-IC 系列，计划稿 `docs/product/ai-pm-assistant-context-integration-plan-v1.md`）。

| 批次 | 内容 | 迁移 | 状态 |
| --- | --- | --- | --- |
| **DK-A**（WP-8） | 载体判定规则 1~5（显式优先/本地关键词闸/LLM 分类≥0.7）+ 单向升级四步同事务 + 30 分钟撤销窗 + 意图分类；REST 唯一发送入口回 `landed` 落点信封 | 0002（conversation↔discovery 链接列 + 三元组 FK） | ✅ 已合 master |
| **DK-B**（WP-8） | 访谈编排器（worker 第八循环）：findings/questions 回填 + 槽位表 + 「问完毕」三条件 + 状态迁移走既有边；会话行锁三入口串行（C7） | 零迁移 | ✅ 已合 master |
| **DK-C**（WP-8） | 知识注入：11 节固定投影（S1~S6 数据节 + 5 脚手架节），**总额度 36,000 字符定值**，超限两步裁剪（整节删 S5→S4→S2→S3，再逐节截断）+ 被删节不静默 + 裁剪记账 | 0004（llm_call_record.metadata，落 `sections_dropped`） | ✅ 已合 master |
| **DK-D**（WP-8） | 写回通道（fail-soft）：REVIEW_REQUIRED 起草止步 `pending_confirmation` + 需求修订 basis 两层溯源 + 任务分解四校验 + 整批 ready 补断链防御；**`draft→ready` 真人闸** | 零迁移 | ✅ 已合 master |
| **DK-E**（WP-8） | 场景 2 更正链：correction 意图 fail-closed 识别 + contradicted/user_confirmed 双 finding + deferred「更正待知识应用」标记（零提案行；KB 挂 KP-3 k4、PC 挂 KP-2）+ 导入自动开 INITIAL_ONBOARDING 访谈 | 零迁移 | ✅ 已合 master |
| **DK-F**（WP-8） | Demo 真栈联调：两场景全链 + §8.3 全表 **30/30 PASS**（真栈缺陷修复×4，各带回归+变异检验）；验收 `docs/product/ai-pm-dk-f-acceptance-record-v1.md` | — | ✅ 已合 master |
| **整合批**（WP-9，A1 伴生批） | Batch 1 壳级助手上下文单一真源 + 指令栏快照交接；Batch 2 线程级上下文锚点（建会话定格 + 服务端真实性校验）；Batch 3A chat 精简注入（S1 项目画像 + S2 仓库事实）；Batch 3B 响应卡信封协议（8 类白名单 fail-closed，非法信封退化纯文本）；Batch 3C 交互写回端点 + 生命周期落库（一卡一生命周期）；Batch 4 UI 诚实化（注入源清单 + 附件禁用） | 0003（context_anchor）/ 0005（conversation_interaction） | ✅ 已合 master |
| **前端 Stage 2**（A3） | 两段时间线 + 边界分隔条（handoff.notice）+ 访谈待办块（open/deferred/已确认事实）；随整合定稿版移植合入（merge 67ba747c） | 零迁移 | ✅ 已合 master |

**三开关**（全部**代码默认 off**；B3 裁决：.10 部署批 env 配 on）：

| 开关 | 域 | 默认 |
| --- | --- | --- |
| `AI_PM_CONVERSATION_ROUTER` | 对话路由/载体升级（DK-A 判定规则 3~4） | off |
| `AI_PM_DISCOVERY_ORCHESTRATOR` | 访谈编排器 + 写回装配（DK-B/D 共用同一开关） | off |
| `AI_PM_CONVERSATION_REPLY` | chat 段会话回复链（assistant 回复） | off |

**验收状态**：DK-F §8.3 全表 30/30（本机 demo 栈，DeepSeek 真外呼，迁移链 0001→0002→0004）；§九 故事线 11/12 即两场景故事线。

**Stage 3（.10 窗口）= V0.5 收官内挂账单批（B2 裁决）**：镜像重发（含全部 DK/KP 产物）→ 重锚（迁移基线 squash 后旧库 checksum 不兼容）→ R3 最小 recreate → env 三开关 on + provider → R1~R8 回归 + 两场景冒烟。

---

## 六、工作包分解

| WP | 内容 | 依赖 | 交付 |
| --- | --- | --- | --- |
| WP-0 | 前置裁决批（V-05-1~5） | — | ✅ **五项全部已裁（2026-11-06），WP-0 清零，无阻塞项** |
| WP-1 | **Product Cognition 能力**（目标 2）：经第四批收纳具体化为 **KP-1（PC-01B 正式模型实施）+ KP-2（确认闭环）**（§3.3） | PC-I0~I6 基线（Q-PC-10） | 分析→**确认**→版本化闭环（Big View 数据面） |
| WP-1b | **V0.5 统一数据模型（Platform Domain Model）**（§3.4，原 Big View 任务正名）：四路资产合并 + 实体三档标注 | WP-0 | 模型总图 + 实体字典；V2 基线 §十一修订；全 WP 的 DB/API 设计输入 |
| WP-2 | AI Team 后端纵切：迁移批 + 应用服务 + REST + 审计 + task_repository/task_waiting 生产写入口 | WP-1 | ✅ 主体已落地（0049~0061 批次：Client Registry/Capability Registry/调度/回复链；简化调度竞争缓解 D-1=b 实施中） |
| WP-3 | ai-pm-runner 客户端：MCP 自领循环 + 本地 workspace + Adapter + interaction 上报 + Continuation 消费 + 离线恢复 | — | ✅ **已落地（2026-11-08，checkpoint-2）**：`apps/ai-pm-runner` 真二进制，.59 真机三边界全关（ACP-09 正式关闭）；Windows 计划任务形态=Runner 设计 §8.1 |
| WP-4 | 平台侧简化调度 | WP-2 | ✅ 简化版已落（pm-worker 第四循环）；**派发器↔Runner 竞争**已裁 D-1=b 缓解 / D-2=c 正式方向（并入调度入口闭合） |
| WP-5 | 人工介入与恢复（平台侧） | WP-3 接口对齐 | ✅ **已落地**：Interaction + REST 回复（0047）+ worker_command + 离线恢复（0052）+ AI 会话前端（Batch G）；UI 批按 U-1=C/U-2=A 排期 |
| WP-7 | **Knowledge Base 正式四职责**（§五）：第四批收纳具体化为 **KP-3（KB 三层最小面：k5 来源 + k4 Structured Memory + Discussion Memory 落库/按 Task 注入）**；S01-08 同步、浏览页、检索入口随 KP-3 及后续批 | 独立并行 | 资料可管理、记忆可沉淀、讨论可注入 |
| WP-6 | E2E MVP 验收（§九故事线 1~10 + ACP 验收子集 + **PC/KB 确认闭环验证**；V1.4 增故事线 11/12 对话两场景） | WP-1（KP-1/2）、KP-3、WP-2~5、WP-8/9 | 验收报告（**收官端到端验收待跑**，含 .10 Stage 3） |
| WP-8 | **对话编排（DK-0 全链，§五之二）**：DK-A~F 六批（载体判定/编排器/知识注入 36k 预算/写回真人闸/更正链/demo 真栈） | KP-1/KP-2/KP-3（写回落点）+ DK-A 底座先行 | ✅ **全部已合 master（2026-11-09，origin/master=67ba747c）**；三开关默认 off；Stage 3（.10）挂账随 WP-6 |
| WP-9 | **对话上下文整合批（WP-8 伴生批）**：Batch 1~4（壳级单一真源/线程锚点/chat 精简注入/信封协议+交互写回/UI 诚实化）+ 前端 Stage 2 两段时间线 | WP-8（共用 ai.conversation 载体） | ✅ 已合 master（迁移 0003/0005） |

依赖关系：WP-0 ✅ → KP-1 → KP-2 串行；KP-3 独立并行；WP-3/4/5 ✅；WP-8 依赖 KP 批落点、WP-9 伴生 WP-8；全部汇合于 WP-6（收官端到端验收 + .10 Stage 3）。

实施设计切片（ACP-01/02/08 等）在 WP-3/WP-5 开工前落盘，闭合 N-1/N-2 与命令查询面。

---

## 七、前置裁决清单（WP-0）

| 编号 | 裁决项 | 状态 |
| --- | --- | --- |
| V-05-1 | Client 载体 | ✅ **已裁（2026-11-06）：升级 `devenv.device`**（加凭据/能力三分/client_type；binding 升级为授权授予） |
| V-05-2 | Git 远端操作归属（代码仓库域） | ✅ **已裁（2026-11-06）：b) Client 自管 push**——Runner 通道本地 clone（用户自备仓库凭据）+ Agent 正常 commit/push + Runner 申报 SHA + 平台只读验证（`git_access.rs` 现有只读面，申报/验证错误码 tools.rs:751/794 现成）。**随裁三件事**：① ADR-0002 口径修订为「执行端不接触**平台持有的**凭据」，平台保留只读验证 key（仅验证不交付）；② V2 文档补注「撤销授权 = 平台不再派发，不收回用户个人仓库凭据——授权管派发，不管用户仓库身份」；③ 服务器 Model B 通道维持平台代理现状不变（非外部 Client）。知识库同步（S01-08）为平台侧 push，属另一域，不受本裁决影响 |
| V-05-3 | project_role 与 member.role 关系 | ✅ **已裁：不取代、共存**（project_role 新独立实体；member.role 六值与 agent_definition 零迁移） |
| V-05-4 | S03-ACP V1.3.1 基线冻结 | ✅ **已裁：冻结**（用户无异议，按推荐执行；N-1/N-2 进实施设计闭合清单） |
| V-05-5 | MVP 先行 Adapter + Runner 接入通道 | ✅ **已裁：ZCode / Codex / DeepSeek Harness 优先 + Runner 走 MCP 自领**（OpenCode/Trae/Claude Code 仅留接口位） |
| V-05-6 | Product Cognition V0.5 范围档位 | ✅ **关闭（2026-11-08 第四批收纳）**：档位由 PC-I0~I6 基线（Q-PC-10）+ KP-1/KP-2/KP-3 批次承载（§3.3）；PC-I4/I5/I6 剩余切出 V0.6 |
| V-05-7 | 与 S01-04 V1.2 六域 Project Cognition 的关系 | ✅ **关闭（Q-PC-01~13）**：PC 全新建；S01-04 Project Cognition = 技术认知六域，保持独立服务其自身领域（不改名/零迁移/不受反向约束）；Product Plan 不属 PC（Q-PC-13） |
| V-05-8 | 第五批收纳（对话化入口） | ✅ **已裁（2026-11-09，按推荐）**：A1 DK-0 全链（DK-A~F）入册为 §五之二横切节 + A2 整合批（Batch1~4）作伴生批一并入册 + A3 前端 Stage 2 登记；B1 两场景入 §九 故事线 11/12（验收=DK-F §8.3 30/30 本机栈）；B2 Stage 3（.10 部署批）属 V0.5 收官内；B3 三开关代码默认 off 维持、.10 env 配 on。编排细则裁决 D-DK-1~11 见设计稿，整合批 D-IC 见计划稿 |

---

## 八、非目标（V0.5 明确排除）

1. 完整调度第④阶段任务适配（复杂度/工具/资源/优先级评估）与人工指定 Client 覆盖。
2. S03-01~06 Design/ADR 域实施；S03-00 V1.2 审计并行推进，不阻塞、不被阻塞。
3. 知识**写入链新机制**（复用现有 propose/confirm/veto + 质量门 + 审计；KB 目标只加同步/备份/浏览面，不改写入链）。
4. Model A 裸 MCP 自领退役（保留；Runner 是其上的进程管理层）。
5. 多 Runner 高可用、Runner 自动升级、LLM Provider 凭据向 Client 透传（V2 §九明确不作为前置条件）。
6. OpenCode / Trae / Claude Code 完整 Adapter（仅框架接口就位；ZCode/Codex/DeepSeek Harness 为 V0.5 优先三家）。
7. 租户级 Team Onboarding（docs/team-onboarding/ 三稿，独立在途批，与本项目并行不混）。
8. **PC/KB 应用深化剩余面（2026-11-08 第四批收纳时切出 V0.6）**：NO_CHANGE/CONFLICT 与冲突发现、Requirement/Design·ADR 影响分析族、完整性/一致性检查、主动认知维护、Team Source Pool、知识检索全量面、KBGIT/PCGIT 正文权威、L0~L3 审批矩阵正式化。
9. **对话编排剩余面（2026-11-09 第五批收纳时切出 V0.6/挂账）**：检索式知识注入（DK-0 定版 = 固定节投影，无检索面）、provider 感知注入预算（36k 字符定值 vs Ollama 本地小窗 8k~32k token 的适配）、前端会话更早历史回翻（Batch H 挂账）、响应卡 defer 动作落写回端点（Batch 3C 挂账）、访谈 deferred「更正待知识应用」的知识应用闭环（随 KP-3 k4 / KP-2 后续批）。

---

## 九、MVP 验收故事线（WP-6）

1. 用户安装 ai-pm-runner 并连接 Platform → Client 注册/在线（V2.0-B）。
2. 项目创建 AI Team：定义 Roles（如 Developer）、授权自己的 Client（V2.0-A/C）。
3. 创建 Task（带 Role + 所需 Capability）→ 调度选中已授权且在线的 Client（①②③⑤）。
4. Runner 领取 Run → 本地 spawn ZCode/Codex → 执行 → 心跳/进展上报。
5. Agent 发出 `interaction.required` → Platform 生成人工待办 → 人类经 REST/UI 回复（fail-closed）。
6. Runner 收到回复投递 → **同一 Run 内 Continuation**：终止旧进程、新 Session/Process 携带原始上下文 + Checkpoint + 人工回复启动（D-01~D-07）。
7. 执行完成 → `pm_submit_result` → 交付验证 → 验收闭环（复用现有 manifest/verdict）。
8. 知识产物经提案链进入知识库（人工确认）→ 知识库按 S01-08 自动同步到用户 Git 服务器（KB-3）。
9. 项目资料/讨论沉淀为 PC 认知提案 → 人工经**确认入口（KP-2：`pc.proposal*` / `pc.approval`）逐项确认** → 正式版本闭合（KP-1 17 表模型）→ Big View 全貌更新（目标 2 闭环；核心原则：AI 提议、人类确认、已确认不可直接覆盖）。
10. 讨论/会话沉淀为 Structured Memory 与 Discussion Memory（KP-3）→ 按 Task 检索注入后续 Agent Context（目标 4 闭环）。
11. **（第五批，场景 1）** 用户在对话框录入新建项目 → AI 逐题访谈（INITIAL_ONBOARDING，问题带 why_blocking）→ 问毕（三条件与）→ 生成需求提案与前置任务（含「手机登录」示例需求）→ **draft→ready 真人闸** → 进调度（目标 3 对话化入口；验收 = DK-F §8.3 场景 1，本机 demo 栈）。
12. **（第五批，场景 2）** 旧项目导入 → 自动生成 KB/PC（分析链）→ 分析时 AI 提问项目背景（自动开 INITIAL_ONBOARDING 访谈）→ 对话问答项目情况（知识注入 36k 预算投影）→ 答错经**更正链**更正（contradicted/user_confirmed 双 finding + deferred「更正待知识应用」）→ KB/PC 更新（验收 = DK-F §8.3 场景 2，本机 demo 栈）。

验收标准：ACP-401~428 相关子集 + V0.5 新增验收（Client 凭据签发/撤销生效、无 Client 时 `no_client_available` 等待、等待窗口自动调度屏蔽 ACP-427/428、task_repository 写入口可用、知识库同步成功/失败重试不丢数据）+ **第四批新增**：PC 确认闭环（draft→proposal→人工确认→正式版本，AI 无确认权不回归）+ KB 三层（来源登记/Structured Memory 评审/Discussion Memory 落库并按 Task 注入）+ **第五批新增**：对话编排两场景（故事线 11/12，DK-F §8.3 全表 30/30 本机栈已过；**收官端到端验收**待 WP-6 整合后全链跑一遍，.10 真栈复验随 Stage 3 部署批）。

---

## 十、工程纪律

1. **迁移号按台账纪律对账取号**：先读 `docs/reference/ai-pm-migration-ledger.md` 再 `ls migrations/`（**2026-11-09 迁移基线合并后**：0001~0065 已 squash 进 `0001_baseline.sql`，新链自 **0002** 起，已用至 **0005**）；单一负责人分配，后实施者撞号无条件顺移并回填台账。
2. ai-pm 主 checkout 存在并行会话在途未提交物（team_llm 修改、0043 迁移、team-onboarding 三稿）：**禁 sweep、禁 git add -A**，只提交本批文件。
3. 禁重建区（核验报告 §四）继续有效：不建 task_execution/execution_result 新表、不改 13 态状态机、不动 MCP 四工具契约、WAITING_FOR_* 不做成状态。
4. Runner 凭据面遵循 ADR-0002 凭据分离与 team_llm_credential 先例（AES-256-GCM 替换制）。
5. 本文档与 Big View 产出落 `docs/project-ai-team-v2/`；commit 时机由用户定。
