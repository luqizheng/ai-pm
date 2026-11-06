# AI-PM V0.5 MVP 范围基线 V1.0

| 文档属性 | 内容 |
| --- | --- |
| 文档编号 | V05-00 |
| 版本 | V1.1（V1.0 + 第二批裁决收编 + Knowledge Base 纳入） |
| 类型 | MVP Scope Baseline（产品第一个可演示闭环的范围基线） |
| 日期 | 2026-11-06 |
| 用户裁决（第一批） | Big View = 产品统一领域模型总图；Runner = 用户 PC 客户端软件；简化版调度进 MVP |
| 用户裁决（第二批） | V-05-1 = **升级 devenv.device**；V-05-3 = **不取代、共存**（project_role 独立新实体，member.role 原样保留）；V-05-4 = **冻结 ACP V1.3.1 基线**（用户无异议按推荐）；V-05-5 = **ZCode / Codex / DeepSeek Harness 优先** + Runner 走 MCP 自领（无异议）；**Knowledge Base 纳入 V0.5**（目标 4）；V-05-2 展开解释后终裁 |
| 前置资产 | V2 架构基线 V1.0（Q1=A/Q2=B/Q3=A）· V2 代码对照核验报告 V1.0 · S03-ACP V1.3.1（独立评审通过）· S01-08 知识库与 Git 备份设计稿 V1.0（四裁决已定，待实施） |
| 优先级裁决 | S01 后续切片降级为非迫切；S03-00 V1.2 审计并行推进、不阻塞 V0.5 |

---

## 一、V0.5 一句话定义

用户在 Platform 上为项目**创建 AI Team**（定义 Roles、授权自己的 Agent Client），通过部署在本机的 **ai-pm-runner** 驱动本地 AI 工具（Codex 等）执行任务；执行中 Agent 可发起人工介入，人类经 REST/UI 回复后任务在**同一 Run 内 Continuation 恢复**，直至交付验收。

这是产品的 MVP：三方通信（目标 1）是通路，AI Team 创建（目标 3）是入口，Knowledge Base（目标 4）是闭环收尾，Big View（目标 2）是保证四者建在同一个模型上而不是四张皮。

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
| Platform ↔ Runner | MCP 四工具（pm_claim_task / pm_get_run_context / pm_report_progress / pm_submit_result）+ Interaction/命令查询最小面（待实施设计） | 四工具已齐（核验报告 §3.3）；Runner 循环 = 现有 worker run_executor 的客户端化抽取 |
| Runner ↔ Agent Client | Adapter（stdin/args/env/文件约定；spawn/ExitOutcome/Job Object/进程组能力自 `codex_executor.rs` 演化） | **V0.5 Adapter 优先级（V-05-5）：ZCode → Codex → DeepSeek Harness**；其余仅留 Adapter 接口位 |
| Human ↔ Platform | REST/UI 回复入口（ACP §8.3 六项权限检查 fail-closed） | 待建（ACP V1.3.1 §8） |

### 2.3 与 S03-ACP V1.3.1 的关系

- ACP V1.3.1（Task-Based Continuation，Model B）独立评审**通过**（P0×3 闭合、D-01~D-07 全落地），是 V0.5 人工介入/恢复链的设计基线，V0.5 启动时正式冻结（V-05-4）。
- 评审遗留两项进实施设计闭合：**N-1**（人工等待窗口与自动调度/回收互斥，ACP-427/428）与 **N-2**（`interaction.required` 物理通道）。Runner 客户端化后 **N-2 从跨进程协议问题简化为 Runner 内 Adapter 侦测约定**（CLI 输出/标记文件→Runner 上报），实现成本显著下降。
- N-6 的"RESPONDED 未消费合法稳态"在 Runner 形态下由 Adapter 持有注入证据（初始输入写入成功 + 进程存活），证据链更直接。
- ACP 中 "Worker" 一词在 V0.5 语境下由 **Runner** 承担；`worker_command` 表 + 命令查询面同样服务 Runner（终止/继续/回复投递）。

---

## 三、Product Big View（目标 2）

**定义**：产品统一领域模型总图——把以下四路资产合并为一张权威模型，作为 V0.5 全部 DB/API 设计的**单一输入源**：

1. V2 基线 §十一领域模型（agent_client / project_role / task / task_execution…）
2. S03-ACP §12 实体表（Run / Interaction / Continuation / Checkpoint / Workspace / worker_command）
3. 既有真相源（delivery.task 13 态 / delivery.task_run 7 态 / devenv.device / ai.team_llm_provider…）
4. 核验报告 §四「禁重建区」七条（防止 Big View 把已否决的重建设再次画进去）

**产出物（WP-1）**：统一模型总图 + 实体字典 + 每实体三档标注（复用 / 新建 / 升级）+ 现有表映射。同步修订 V2 基线正文 §十一（改为引用现有表而非新建清单）。

**红线**：Big View 是模型权威，不是迁移清单；凡与核验报告 §四冲突的实体一律按报告处置。

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

## 五、Knowledge Base（目标 4，V1.1 新增）

**定位**：MVP 闭环的收尾环节（S03-00 V1.2 §0.2 闭环最后一拍「知识库更新」）。范围 = 既有写入链复用 + S01-08 实施 + 最小浏览面。

| 项 | 范围 | 依据/现状 |
| --- | --- | --- |
| KB-1 持久化与版本 | Markdown+Git 正文权威、PG 状态权威、PageIndex 可重建派生（KnowledgeIndexer Adapter 隔离） | ai-pm-markdown-knowledge-space.md 已定稿（12 节） |
| KB-2 写入链 | 复用 propose/confirm/veto + 质量门 + 人工确认 + 审计，**零新建机制** | pm-mcp/tools.rs:155-238 在跑（核验 §四.5） |
| KB-3 Git 备份（S01-08 实施） | 知识库仓库独立配置（R-01）·自动同步为主+手动 Sync Now（R-02）·GitHub/GitLab/Gitea/自建（R-03）·创建项目非必需（R-04）；失败不阻断、不丢数据、不自动覆盖远程冲突 | S01-08 V1.0 四裁决已定，待实施 |
| KB-4 浏览最小面 | 知识库页：文件树 + 文档查看 + 版本历史 | 前端 |

**与 V-05-2 的域边界（防两个 Git 裁决互相污染）**：知识库同步 = **平台侧 push**（平台持知识库仓库凭据、系统自动安排同步——S01-08 R-02 本义）；V-05-2（代码仓库 Client 自管 push）**不适用**知识库域。实施时 git 能力分两档：只读验证面（现有 `git_access.rs`，显式无 push）+ 知识库同步 push 面（WP-7 新增，仅限知识库仓库）。

---

## 六、工作包分解

| WP | 内容 | 依赖 | 交付 |
| --- | --- | --- | --- |
| WP-0 | 前置裁决批（V-05-1~5） | — | ✅ **五项全部已裁（2026-11-06），WP-0 清零，无阻塞项** |
| WP-1 | Product Big View V1.0（§三章程；KB 域实体一并纳入） | WP-0 | 模型总图 + 实体字典 + 三档映射；V2 基线 §十一修订 |
| WP-2 | AI Team 后端纵切：迁移批（0045 起）+ 应用服务 + REST + 审计 + task_repository/task_waiting 生产写入口 | WP-1 | Roles/Client/授权/凭据/能力全链 API |
| WP-3 | ai-pm-runner 客户端：MCP 自领循环 + 本地 workspace（`{root}/{run_id}/`）+ Adapter 框架 + **ZCode/Codex/DeepSeek Harness Adapter** + interaction 上报 + Continuation 消费 | WP-2（凭据）；骨架可与 WP-1 并行 | Runner 可执行文件（Windows 优先） |
| WP-4 | 平台侧简化调度：调度决策插入 TaskReadyHandler 链前 + 无 Client 等待语义 + Run 快照扩展 Client 维度 | WP-2 | 建 Task→自动选中已授权在线 Client |
| WP-5 | 人工介入与恢复（平台侧）：Interaction 实体 + REST/UI 回复入口 + worker_command + 等待窗口调度屏蔽（N-1/ACP-427/428）+ AI Team 前端页 + 待办入口 | WP-3 接口对齐 | 介入→回复→Continuation 全链 |
| WP-7 | Knowledge Base：S01-08 实施（知识库 Git 同步 push 面）+ KB-4 浏览页 + 写入链接线验证 | S01-08 设计稿；与 WP-2~5 并行 | 知识可写入、可浏览、可同步用户 Git |
| WP-6 | E2E MVP 验收（§九故事线 + ACP 验收子集 + KB 同步验证） | WP-2~5、WP-7 | 验收报告 |

依赖关系：WP-0 → WP-1 →（WP-2 → WP-4/WP-5）；WP-3 骨架并行、凭据接入依赖 WP-2；WP-7 独立并行；全部汇合于 WP-6。

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

---

## 八、非目标（V0.5 明确排除）

1. 完整调度第④阶段任务适配（复杂度/工具/资源/优先级评估）与人工指定 Client 覆盖。
2. S03-01~06 Design/ADR 域实施；S03-00 V1.2 审计并行推进，不阻塞、不被阻塞。
3. 知识**写入链新机制**（复用现有 propose/confirm/veto + 质量门 + 审计；KB 目标只加同步/备份/浏览面，不改写入链）。
4. Model A 裸 MCP 自领退役（保留；Runner 是其上的进程管理层）。
5. 多 Runner 高可用、Runner 自动升级、LLM Provider 凭据向 Client 透传（V2 §九明确不作为前置条件）。
6. OpenCode / Trae / Claude Code 完整 Adapter（仅框架接口就位；ZCode/Codex/DeepSeek Harness 为 V0.5 优先三家）。
7. 租户级 Team Onboarding（docs/team-onboarding/ 三稿，独立在途批，与本项目并行不混）。

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

验收标准：ACP-401~428 相关子集 + V0.5 新增验收（Client 凭据签发/撤销生效、无 Client 时 `no_client_available` 等待、等待窗口自动调度屏蔽 ACP-427/428、task_repository 写入口可用、知识库同步成功/失败重试不丢数据）。

---

## 十、工程纪律

1. **迁移号自 0045 起**（0043/0044 已被并行会话占用），落地前重核 `docs/ai-pm-migration-ledger.md`。
2. ai-pm 主 checkout 存在并行会话在途未提交物（team_llm 修改、0043 迁移、team-onboarding 三稿）：**禁 sweep、禁 git add -A**，只提交本批文件。
3. 禁重建区（核验报告 §四）继续有效：不建 task_execution/execution_result 新表、不改 13 态状态机、不动 MCP 四工具契约、WAITING_FOR_* 不做成状态。
4. Runner 凭据面遵循 ADR-0002 凭据分离与 team_llm_credential 先例（AES-256-GCM 替换制）。
5. 本文档与 Big View 产出落 `docs/project-ai-team-v2/`；commit 时机由用户定。
