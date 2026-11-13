# S03-ACP 实施设计 · 通信面切片（ACP-01 / ACP-02 / ACP-08）V1.2
# —— N-1 / N-2 设计闭合与命令查询面

| 项目 | 内容 |
|---|---|
| 文档性质 | ACP 切片实施设计稿（V1.2；专家三轮复审意见全部收编，进入收敛实施——收敛批两层交付见 §10.5） |
| 版本 | **V1.2**（收编独立复审 V1.0 全部 R-1~R-11 + P2×6；取代 V1.1，逐项可核，对照清单见修订记录） |
| 裁决状态 | R-ACP-1~9 已裁决（2026-11-07）；**R-ACP-14（情形5 取消重试）已裁=接受（2026-11-07 用户裁决）；R-ACP-15（偏离登记）与 R-ACP-16（离线恢复=方案 a）随本稿登记生效**。编号自复审建议的 10/11/12 顺移（10~13 已被 ACP-03 稿占用并裁决，INDEX 8；后实施者顺移纪律）。**专家复审（第 1 块：活风险三项，2026-11-07）：全部批准 + ACP-C1~C4 横向约束收编（§9）；第 2 块（六项取舍，2026-11-07）：四维持两修正（V-05-1 身份硬前置升格、发现语义独立为 `pm_discover_runs`、链加 `previous_chain_id`）；第 3 块（排期依赖，2026-11-07）：方向全赞同 + 三处调整（V-05-1 含撤销最小集 / ACP-09 两阶段 / ACP-04·05 联合设计）——**三轮复审全部收编，本稿进入收敛实施**（排期落 `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md`）** |
| 设计基线 | **S03-ACP V1.3.1（已冻结，2026-11-07 用户决议）**：D-01～D-07 为已生效架构裁决 |
| 覆盖切片 | ACP-01（Interaction 领域模型）· ACP-02（Process Termination）· ACP-08（Adapter 能力与集成） |
| 本稿闭合项 | §23.3：Worker Command 持久化/轮询/幂等/失败升级 · N-1（等待窗口调度互斥）· N-2（interaction.required 与消费回执物理通道）· Interaction CANCELLED/TIMED_OUT/唯一活跃约束 · awaiting_decision 映射 · Process Termination 失败处理 · Adapter 能力验证 · **等待窗口执行端离线恢复（R-11，§3.6）** |
| 不覆盖（另稿） | ACP-03（REST/UI 回复入口——**已实施，0047，见 INDEX 8**）· ACP-04（Delivery/Consumption 持久化细节）· ACP-05（Continuation 状态机）· ACP-06（Checkpoint/Workspace Lock）· ACP-07（Deadline 授权）· ACP-09（Model A 回归） |
| 代码基线 | ai-pm master @ **e1feb5f**（2026-11-07，实现先行批合并 + 真实栈 E2E PASS，报告见 INDEX 8a）；关键事实引用见 §2 |
| 关联裁决 | V-05-1（device 升级 Client Registry——**ACP-02 命令面硬前置**，R-3）· V-05-2（Client 自管 push + ADR-0002 口径修订）· V-05-5（Runner 走 MCP 自领）· A*（正式确认仅可信 REST/UI）· V-6（统一应用服务）· A-02（授权快照） |

## 修订记录

- **V1.0 → V1.1**：收编初步独立评审（P0×4/P1×5、ACP-437~445、R-ACP-1~9）——见 V1.1 修订记录，此处不再重复。
- **V1.1 → V1.2**（2026-11-07，收编独立复审 R-1~R-11 + P2×6；实现先行批 e1feb5f 事实同步）：
  - **R-1（P0-1）**：C3 事实改正（B3-2 = 旧 Run `timed_out` + 新建 attempt Run，非同 Run 重领）；S1 改为**整条迭代不可见**（实现=SQL 排除，§4.2）；S2/S3 拦截对象明确为 `recover_lost_run` 全部调用方 + spawn 前校验；情形 5 按真实路径重写并新增裁决项 **R-ACP-14**；ACP-428 文本修正（§4.1/§4.2/§8）。
  - **R-2（P0-2）+ R-11（对账新增）**：新增 **§3.6 同主体续领（same-owner re-lease）**——`pm_reacquire_lease` + 发现面 + 命令行 `reacquired_generation` 写回 + worker_id 同事务改路由；§3.3 ③ generation 比对规则修订为集合比对；R-ACP-16 裁决=方案 a（含离线超 TTL 恢复）；裁决 Runner 不本地持久化租约凭据；新增 ACP-446/447/452/453。
  - **R-3（P0-3）**：§3.3 ② 绑定对象改为经鉴权 Client 主体（V-05-1 后）；`execution_mode` 三分 **已实现**（spawn / runner: 前缀 spawn / human，§2 补核 E1）；`pm_report_interaction` 按 execution_mode 准入（**实现欠账，ACP-448**）；V-05-1 升级为命令面硬前置，过渡形态（`runner:` 前缀自报）显式登记；新增 ACP-448/449。
  - **R-4（P0-4）**：continue acked（INPUT_INJECTED）事务内 Task `awaiting_decision → running` + resolve `task_waiting` + `chain_open=false`；`consumed` 降为 Interaction 级事实；§4.1 情形 4 / §5.1 / §5.2 / §5.3 / §1.5 同步修订；新增 ACP-450/451。**实现注记**：e1feb5f 现行为 consumed 时转 running（interaction.rs mark_consumed），acked 事务内转 running 为待收敛项。
  - **R-5（P1-1）**：迁移事实改写——**0046（task_interaction + worker_command + task_run.scheduling_shield）/ 0047（interaction_response）已落地**（R-ACP-6 行备注更新）；后续批次自 0048 起。
  - **R-6（P1-2）**：`initial_input_injected` 按**注入方式**定义为"已被接收"（§1.4 表）；落 §7.1 每 Adapter 行。
  - **R-7（P1-3）**：Process 记录落点二选一 → **选 b)**：Process 事实权威 = `worker_command.receipt` + 审计 + `executor-events.jsonl`；§6 第 6 条"扩展清理账本"删除（F-10 的 cleanup 账本终态行登记为无害旁注，非权威载体）；Runner 本地进程账本 = WP-3 指针。
  - **R-8（P1-4）**：`session_id := task_run.executor_session`、`continuation_generation := 当前代次（ACP-05 前恒 1）`、terminate `process_id` V1 允许为空（按 Run/pid 文件定位）——登记为实施定案（§6）。
  - **R-9（P1-5）**：§4.3 声明**等待窗口续租 = 执行端存活信号，非 Agent 心跳**；guard→none 时重置 ②③ 陈旧判定起点；`report_progress(started)` 同态幂等已核对（§4.3）。
  - **R-10（P1-6）**：登记裁决项 **R-ACP-15**——命令面不依赖有效 Run Lease = 对冻结基线 §10.4/§16.3 的正式操作化修订（§9）。
  - **P2-1**：标记文件名正式改为 `interaction.{event_id}.json`（实现现用固定名+event_id 归档，收敛项，§1.2）。
  - **P2-2**：`timed_out` 判定者 = 平台侧定时清扫（**已实现**：worker 循环 `interaction_timeout_sweep`）+ REST/UI 读取时二次判定（§1.5 注、§5.3）。
  - **P2-3**：`task_waiting.questions` 写为单元素**数组** `[{question,options,interaction_id}]`（§5.3）。
  - **P2-4**：本切片 Outbox 事件子集显式声明（**已实现**：created/responded/consumed/timed_out/cancelled/retried/report_replayed 七项，§3.1）。
  - **P2-5**：ACP-445 互斥机制点名 = Task 行 `FOR UPDATE`（**已实现**，interaction.rs，§8）。
  - **P2-6**：§3.3 表头补"双持身份"说明（§3.3）。
  - 验收补 ACP-446~453（§8）；裁决记录增 R-ACP-14/11/12（§9）。
- **专家复审第 1 块意见收编（2026-11-07，活风险三项全部批准 + 补充约束；意见原文归档 `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-1.md`）**：
  - **ACP-448 补强**：execution_mode 准入与 **ACP-C1 命令路由不变量**为两层独立判定（准入管"能不能报"，路由不变量管"命令有无有效投递目标"），不得混为一套；历史 human Run 孤儿 terminate 命令 = **定向审计 + 修复，禁止直接删除**；命令修复不得在无独立证据下改 Task 状态。
  - **R-4 补强**：迟到/缺失/重复 consumed 回执**不得改变已确认的 Continuation 恢复结果**；旧链关闭后新链与旧 Interaction 保持**独立关联**；迁移期历史 responded+running 数据**按 continue ACK 证据分类修复**，禁止仅凭 Task 状态推断。
  - **R-16 关键修正**：**terminate ACKED 不作续领前置，改作新 Session 启动前置（ACP-C4）**——续领后五步时序落 §3.6；改路由 ≠ 重建命令（command_id/idempotency_key/payload 不变、claim_count 不重置）；generation fencing 覆盖旧执行端迟到 ACK；续领本身不自动创建新 Session。
  - 新增**横向架构约束 ACP-C1~C4**（§9 末）；验收补 ACP-454/455（§8）；E2E 必测场景四项（§8 末）。
- **专家复审第 2 块裁决收编（2026-11-07，六项取舍=四维持两修正；意见原文归档 `...-expert-review-2.md`）**：
  - **2.1 R-7 维持选 b** + 补强：receipt 须有**稳定 Schema 与版本**、区分 `terminate_requested`/`terminate_confirmed`、记录 Run generation（防旧回执污染）、`executor-events.jsonl` 仅诊断证据非平台权威、多 Process 独立控制/迁移/查询需求出现时重新评估建表（§6.6）。
  - **2.2 R-16 维持不持久化 token** + 补强：WP-3 增加**稳定 Client 身份持久化**（重启必须恢复原 Client 身份）；续领失败**指数退避 + jitter**；持续失败告警 + 人工处置入口（禁 panic 丢恢复上下文）；**身份失效/deadline 到期即停止自动恢复**（§3.6）。
  - **2.3 T3 维持严格不顺延** + 产品面：UI 可提供**快捷入口直接进 ACP-07 重新授权流程**（不改原 deadline、不自动延长决策有效期、回复不丢弃、新授权独立审计+独立有效期、未授权前不自动启动 Session）（§3.6）。
  - **2.4 V-05-1 修正（生产禁令）**：`lease_owner` 前缀匹配**仅限受控环境临时兼容，非生产安全身份**；client_name 必须**登记并绑定认证主体**；发现/续领最终必须验证稳定 Client 身份；**V-05-1 升格为 R-16 对外（生产）启用的硬前置**——身份安全能力未完成前禁止生产环境开放自报 client_name 续领；generation 被抢占**不得视为无害干扰**（§3.3/§3.6）。
  - **2.5 推翻双态 Poll → 独立工具 `pm_discover_runs`**：Poll 保持单 Run 命令轮询原契约不动；Discover 为独立业务能力（Client 名下待恢复 Run 发现），返回字段十个（run_id/execution_mode/interaction_id/interaction_status/deadline_at/shield_state/lease_generation/lease_status/terminate_command_id/terminate_status），仅限当前认证 Client 有权恢复的 Run、**不返回人工回复正文与 Command payload**、支持分页、重复调用无状态副作用（§1.6/§3.6）。
  - **2.6 Chain 维持多独立链 + 弱关联字段 `previous_chain_id`**：只引用同 Task 已关闭前驱链、禁自引用/循环、不改 retry_of 链内语义、不共享生命周期——只服务追溯/审计/UI；**不加冷却、不合并连续介入**（重复事件靠 event_id 幂等 + Session 标识 + Run generation 校验区分）（§5.1/§5.2；0048 迁移列）。
  - 收敛后重点验证六项（§8 末清单更新）。
- **专家复审第 3 块裁决收编（2026-11-07，排期依赖=方向全赞同 + 三处调整；意见原文归档 `...-expert-review-3.md`，排期全文落 `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md`）**：
  - **V-05-1 升格为平台级 Client Identity Foundation（独立基础切片，非 ACP 子任务）**：立即插入关键路径（可与 ACP-04 设计并行）；**最小生产可用集必须含撤销/禁用**（Client 登记 + client_id/client_name 唯一归属 + 认证主体绑定 + 凭据签发/验证 + **撤销或 disabled_at** + lease_client_id 回填 + **正式权限码字典**；能力三分动态管理/多凭据轮换/生命周期 UI 后补）——推翻我方"撤销可后补"倾向（**签发而无撤销不符合生产身份基座**：泄露凭据仍可 discover/续领/轮询/上报）；**撤销立即影响全部四类命令面/上报工具**，不能只禁续领放行轮询。
  - **迁移号拆分**：0048 = ACP 收敛（`reacquired_generation` + `previous_chain_id`）；0049 = Client Registry（登记表 + `lease_client_id` + 回填 + 权限字典）——独立审查/测试/回滚；**lease_client_id 历史回填三分类**（可靠对应→自动回填 / 仅自报 client_name→暂不认定 / 来源不明→legacy-unverified；历史兼容≠历史身份可信，保留 lease_owner 但不得据此获新续领权）；迁移号由单一实施负责人统一分配。
  - **收敛批拆两层交付（不做不可拆大 PR）**：**Correctness Fix**（ACP-448 + ACP-450/451 + **Model A 定向回归**，P0 立即）/ **Recovery Foundation**（V-05-1 + ACP-452/453 + ACP-C1~C4 + discover + chain 关联，依赖 V-05-1 身份安全完成）——同批不同 PR、独立提交独立验收。
  - **ACP-04 不推迟到最后**：与 ACP-05 **联合设计**先闭数据契约（Delivery 引用 Run 还是 Session / Consumption 是否需 continuation_generation / Delivery 是否参与 Task 完成判定 / 05 是否消费 04 持久化 / 04 是否依赖 05 未建的正式 Session 身份），按真实数据依赖定实施先后，默认 04 核心交付持久化先行；**不得临时另建一套 Session 身份**。
  - **ACP-09 改贯穿式验证工作包（两阶段）**：阶段 A 定向回归随 Correctness Fix（human Run 调 report 被拒 / Model A 既有 awaiting_decision 流程 / 回复后继续 / 无证据 Adapter / 重复 ACK 幂等）；阶段 B 完整回归在 ACP-05~07 后（存活/离线超租约/重启/续领/迟到 ACK/terminate 未确认禁启/deadline 到期/ACP-07 重授权/多次介入/多 Repo Delivery/Model A 全量）。
  - **ACP-05 边界**：先定义正式 Continuation 身份/状态/代次/生命周期，Checkpoint/Lock/Deadline 经明确接口接入——不顺带吸收 06/07 职责。
  - **task_repository P0 独立启动**（不等 ACP 收口/V-05-1）：链路=创建关联→仓库授权→claim→execution_allows→scope→Delivery/Commit，六约束（项目内有效仓库/创建时校验授权/禁跨项目/幂等/claim 与 execution_allows 同一事实/禁 MCP 伪造归属），**沿用既有 Project Repository 领域对象**不另建权限体系；task_waiting 空洞已由本切片补上（移出 P0 清单，留真实栈回归证据）。

---

## 0. 载体双形态声明（本稿最重要的一层适配）

V1.3.1 中 "Worker" 一词在 V0.5 语境下由 **ai-pm-runner（用户 PC 客户端）** 承担（V0.5 基线 §2.3），但服务器 Model B 通道（ai-pm-worker 进程内编排）维持现状不变（V-05-2 裁决③）。因此本稿所有协议面按**双形态等价**设计：

| 载体 | 部署位置 | 命令查询面接入方式 | Interaction 上报接入方式 |
| --- | --- | --- | --- |
| ai-pm-runner（V0.5 目标形态） | 用户 PC，持 Client 级凭据（V-05-1 device 升级签发） | **MCP 工具轮询**（`pm_poll_commands` / `pm_ack_command`） | MCP 工具（`pm_report_interaction` / `pm_report_consumption`） |
| ai-pm-worker（服务器 Model B，现状保持） | 服务端容器 | 直连同一应用服务（进程内，不经 HTTP） | 同一应用服务直连 |

**统一纪律（V-6 架构约束）**：两条接入路径**必须**落在同一业务应用服务（`pm-application`）上；MCP 工具与 Worker 进程内调用都只是适配层。禁止为 Runner 另写一套写入逻辑。

**V1 实现覆盖补注（2026-11-07 复审 F-3，e1feb5f 仍准确）**：命令消费与标记文件侦测**仅 Runner 载体接线**；服务器 ai-pm-worker 载体仅接了超时清扫（terminate/continue 消费与侦测未接）——按 V-05-2③ 属显式延后，非双形态已等价；Worker 载体的命令消费随服务器侧需求另行接线。

**实现先行批注记（e1feb5f）**：Runner 应用已落地（`apps/ai-pm-runner`：runner_loop / adapter / mcp 三模块），真实栈 E2E 两阶段 PASS（假 Agent + opencode 真 CLI，报告 INDEX 8a）。本稿 V1.2 相对该实现存在**三处设计收敛项**（实现欠账，随复审通过后的收敛批落地）：① acked 事务内 Task→running（R-4）；② `pm_report_interaction` 按 execution_mode 准入（R-3，ACP-448）；③ 离线恢复的发现面+续领（R-11，§3.6）。**WP-A 收敛后更新（2026-11-07）**：①已落地（`interaction.rs resume_execution_after_continue_tx`，ack_command 同事务接线，20 例绿）；②经 WP-A 复核**本就已实现**（interaction.rs:138-145 准入+零副作用，上一轮对账遗漏）并补集成测试 `acp448_human_mode_run_cannot_report_interaction`——三处欠账仅剩 ③（Recovery Foundation，依赖 V-05-1）。

本稿正文统一使用 "执行端" 指代 Runner / Worker 双形态。

---

## 1. N-2 闭合：interaction.required 与消费回执的物理通道

### 1.1 三段式通道模型

```text
Agent CLI（Codex / ZCode / DSH）
   │  ① Agent → 执行端：Adapter 侦测约定（Workspace 标记文件，§1.2）
   ▼
ai-pm-runner / ai-pm-worker（持有 Run Lease 身份）
   │  ② 执行端 → Platform：MCP 工具 pm_report_interaction（§1.3）
   ▼
Platform（pm-application InteractionService → task_interaction + awaiting_decision + 命令）
   │  ③ 回复消费方向：Platform 持久化回复（ACP-03，已实施）
   │     → worker_command 投递（§3）→ 新 Session 创建 + 初始输入注入（§1.4）
   ▼
执行端 → Platform：MCP 工具 pm_report_consumption（§1.5，消费回执）
```

### 1.2 ① Agent → 执行端：标记文件协议（完整版）

**主通道 = Workspace 标记文件；stdout JSONL 不作为 V1 通道**（理由：现状 stdout 解析仅服务于进程收割的 ExitOutcome 判定；混载引入截断/交织风险；标记文件可承载完整 payload 且可重读、可审计）。

**路径（对齐现状布局 F7）**：

```text
{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}/.aipm/interaction.{event_id}.json
```

`workspace_path` 为该 Run 的仓库 checkout 段；Checkpoint 相对路径（§6.1）按**同一 Workspace 根解析规则**。**文件名含 `event_id`（P2-1 采纳）**：Agent 写唯一名，执行端扫描 glob——并发出事件按文件天然并存，不再有"第二次写入覆盖第一次"的丢失窗口。协议全表：

| 项 | 约定 |
| --- | --- |
| Schema | `{"schema_version":"aipm.interaction.v1", "event_id", "reason", "title", "question", "options"?, "required_response", "checkpoint"?}`；`reason` 四值沿用等待语义（needs_info/needs_decision/needs_approval/external_dependency） |
| 大小限制 | ≤ 64 KiB，UTF-8；超限按非法处理 |
| 创建责任 | `.aipm/` 目录由 Agent 首次写入时创建（instruction.md 引导，§7.2）；**执行端不得代写标记文件**，只做校验、上报与归档改名 |
| 校验 | 执行端上报前校验 schema_version/reason 枚举/必填字段；不合格 → 改名 `interaction.{event_id}.invalid` 隔离 + 告警指标，**不产生 Interaction** |
| 归档 | 上报成功后改名 `interaction.{event_id}.reported`（**event_id 键控归档**，历史文件并存） |
| 重启扫描 | 执行端启动/重连时扫描工作区中未归档的 `interaction.{event_id}.json` 与历史 `*.reported`/`*.rejected`/`*.invalid`；未归档文件按 event_id 幂等重报（Platform 返回原 interaction_id） |
| 改名失败恢复 | 上报成功但改名失败 → 文件保持原名，下轮按同 event_id 重报（幂等）+ 重试改名；连续失败计告警指标 |
| 并发新事件 | 未归档文件并存时按写入时间顺序逐个上报；被唯一活跃链约束（§5.2）拒绝的 → 改名 `interaction.{event_id}.rejected` + 指标，不重试 |
| 幂等键 | `event_id`，Platform 侧 `UNIQUE(run_id, event_id)`（已实现，0046）；重复上报返回原 interaction_id 不新建 |

> **实现注记（e1feb5f）**：Runner 侦测现用固定名 `interaction.json`（两层路径约定）+ `interaction.{event_id}.reported/.rejected` 归档。事件不丢（侦测间隔内单文件语义成立），但 P2-1 的并发窗口存在——文件名含 event_id 为**待收敛项**，协议其余部分已对齐。

**责任边界（P1-4）**：标记文件**不是 Interaction 的权威存储**——Platform 的 `task_interaction` 是唯一权威。Agent 只能**创建新事件**，不得修改已提交事件（instruction.md 明示）；执行端负责校验、幂等、上报、归档；文件上报成功 ≠ 人类回复已提交，更 ≠ 已消费。

### 1.3 ② 执行端 → Platform：新 MCP 工具 `pm_report_interaction`

**不修改现有四工具契约**（工程纪律红线）。本工具沿用**执行面身份**（Run Lease 身份，见 §3.3 双轨表）：

```text
pm_report_interaction(
  project_id, task_id, run_id,
  lease_token, lease_generation,          -- T-032 双校验，同 pm_get_run_context
  event_id,                               -- 幂等键
  reason, title, question,
  options: Option<Vec<{value,label}>>,
  required_response: bool,
  checkpoint: Option<{relative_path, available}>
)
→ { interaction_id, status: "pending", deadline_at, task_status }
```

| 设计点 | 规则 |
| --- | --- |
| 身份与权限 | 权限码 `PM_MCP_AGENT_REPORT`（§1.6，**已落 F-7 双态门**）+ 租约三要素。**不新增人类确认语义，不违 A\***（人类回复仅 REST/UI，属 ACP-03） |
| **execution_mode 准入（R-3，专家复审批准 + 补强）** | **仅 `execution_mode ∈ {'spawn'}` 的 Run 可调用**（Runner / Worker spawn 形态）；`'human'`（Model A 真人 MCP 客户端）→ fail-closed 明确错误码拒绝且**不产生 Interaction、不产生任何命令副作用**——Model A 的等待走既有 `awaiting_decision`/`task_waiting` 入口（ACP-09 边界的操作化）。**两层判定分离（专家约束）**：execution_mode 负责**准入**；**ACP-C1 命令路由不变量**独立负责"命令存在有效可验证投递目标"——不得混为一套判定。**存量治理（专家约束）**：历史 human Run 下未终局的孤儿 terminate 命令 = 定向审计 + 修复（保留审计记录，禁止直接删除）；命令修复不得未经独立证据改变 Task 状态。**实现注记（WP-A 复核更新）**：准入校验**已实现**（interaction.rs run 装载处，非 spawn 拒绝零副作用）+ 集成测试 `acp448_human_mode_run_cannot_report_interaction` 钉住 |
| 幂等 | `UNIQUE(run_id, event_id)`（已实现）；重复上报返回原 `interaction_id` 不新建 |
| 介入链唯一 | 该 Task 存在**开放介入链**（§5.2）→ Conflict 拒绝 |
| awaiting_decision 映射 | 同一事务：Task → `awaiting_decision`（经既有 task_transition 门槛）+ `task_waiting` upsert + 调度保护状态 `human_wait`（§4.1）（已实现，单事务，interaction.rs） |
| deadline | `deadline_at = now() + AI_PM_HUMAN_INTERVENTION_TIMEOUT_SECS`（默认 120s）；持久化于 Platform，重启不重算 |
| 终止命令触发 | Interaction 持久化成功后，**同一事务**创建 `worker_command(agent.process.terminate)`（D-03；已实现） |

### 1.4 ③ 回复投递三阶段（P1-1）

回复经 REST/UI 持久化（ACP-03，已实施：0047 + 同事务 continue 下发）后，经 `agent.task.continue` 命令投递。三个阶段严格分离，**各阶段失败不得相互冒充**：

| 阶段 | 判据/证据 | 状态影响 |
| --- | --- | --- |
| **SESSION_CREATED** | 新进程 spawn 成功（pid/Job Object/进程组证据） | Continuation 域记录（ACP-05）；命令内部阶段，**单独不构成任何成功语义** |
| **INPUT_INJECTED** | 初始输入**已被接收**——按注入方式定义（R-6，见下表） | Delivery → `DELIVERED`；**`agent.task.continue` acked 的最低判据**；调度保护释放点 + **Task 恢复执行点（R-4，§5.3）** |
| **CONSUMPTION_CONFIRMED** | `pm_report_consumption` 双证据（§1.5） | Delivery → `CONSUMED`；`interaction.consumed_at` 落值——**Interaction 级事实，不承担 Task 状态/链开放职责（R-4）** |

**`initial_input_injected` 按注入方式的语义（R-6 采纳；基线 §8.6 精化，不重开 R-ACP-5）**：

| 注入方式 | "injected" 的证据 | 说明 |
| --- | --- | --- |
| 启动参数/提示文件（Codex `exec "<prompt>"`、ZCode 同类） | 回复内容是进程**启动参数/启动即读文件**的一部分（记录 spawn 参数摘要）；进程启动成功 ⇒ 必然接收 | 此时 `process_alive` 有意义 |
| stdin 流式注入 | 注入后观测到 Agent **首个输出轮次**（首条 stdout/JSONL 事件） | 仅"写入成功"不算 |
| 无法满足 | 不伪造；N-6 稳态 + 指标 | 不变 |

> **实现注记（e1feb5f）**：Runner = reply 落盘 + spawn + 存活探测（adapter.rs），属第一类（启动即读文件 ⇒ 必然接收），与 R-6 语义相容；spawn 参数摘要落 receipt。

**注入失败处置（不变量）**：SESSION_CREATED 成功但 INPUT_INJECTED 失败 → **立即终止该新进程**（未携带人工回复的进程不得存活）→ 命令 `ack(outcome=failed, error=injection_failed)` → 调度保护**不释放**（保持 `start_wait`）→ Delivery 按原 `delivery_id` 幂等重投 → continue 重试 → 新 Session。系统不变量：**任何存活进程要么已注入人工回复，要么正在被终止**。

因此 acked 从严判据：continue 命令 acked 要求 receipt 同时含 `session_created: true` **且** `input_injected: true`（已实现，worker_command.rs 从严校验）。

### 1.5 ④ 消费回执：新 MCP 工具 `pm_report_consumption`

```text
pm_report_consumption(project_id, task_id, run_id, lease_token, lease_generation,
                      delivery_id, interaction_id,
                      evidence: { kinds: [...], detail })
→ { interaction_status: "consumed", delivery_status }
```

| 设计点 | 规则 |
| --- | --- |
| 权限/身份 | 权限码 `PM_MCP_AGENT_REPORT` + 执行面租约三要素（已实现：verify_lease 前置） |
| 证据种类（V1） | `initial_input_injected`（§1.4 R-6 语义）+ `process_alive`（注入后进程存活，带 pid/job-object 证据）。二者同时具备才接受；证据不足 → 拒绝（**禁止伪造 CONSUMED**，已实现） |
| N-6 合法稳态 | Adapter 无法提供可靠证据时**不伪造回执**；`RESPONDED 未消费` 为**合法稳态**：不阻塞执行、不计失败，计告警指标（`interaction_unconsumed_total`），由 ACP-08 按 Adapter 分级声明。**R-4 后本稳态只影响 Interaction/Delivery 字段与告警，不影响执行**（Task 已在 acked 事务内恢复 running，§5.3） |
| 租约过期限定（R-2/R-11 后修订） | 回执仍走执行面租约校验；租约过期 → `LeaseError::Expired` 拒绝 → Interaction 停留 responded（N-6 稳态）。**执行端离线恢复后经 §3.6 续领取得新 token/generation 即可正常提交回执**——回执失败不再意味着"Run 终止在即"，而是"等待执行端恢复"（REST 查询面可见 + `interaction_shielded_runs` 指标） |
| 幂等 | 同一 `delivery_id` 只能有一条有效消费结果；重复提交返回原结果 |

### 1.6 权限边界（P1-5，新增工具不挂 READ）

现有四执行器工具沿用 `PM_MCP_READ` 历史语义**不动**（契约红线；随 V-05-1 迁移批再评估重命名）。本稿六个新工具设独立权限码：

| 工具 | 权限码 | 身份轨（§3.3） | 写语义 |
| --- | --- | --- | --- |
| `pm_report_interaction` | `PM_MCP_AGENT_REPORT` | Run Lease（+ execution_mode 准入） | 写 task_interaction / task_waiting / 调度保护 / 命令 |
| `pm_report_consumption` | `PM_MCP_AGENT_REPORT` | Run Lease | 写 delivery consumed / interaction consumed_at |
| `pm_poll_commands` | `PM_MCP_COMMAND` | Command Delivery | 命令领取（delivered + claim 租约） |
| `pm_ack_command` | `PM_MCP_COMMAND` | Command Delivery | 命令回执（acked/failed + receipt） |
| `pm_reacquire_lease`（§3.6 新增） | `PM_MCP_COMMAND` | Command Delivery | 同主体续领（generation+1 + 新 token + 命令改路由） |
| `pm_discover_runs`（§3.6 新增，专家第 2 块裁决：推翻双态 Poll，独立工具） | `PM_MCP_COMMAND` | Command Delivery | Client 名下待恢复 Run 发现（只读、分页、无状态副作用） |

权限码语义边界不变（A*：人类正式确认永不出现在以上任何码内）。**实现注记（F-7）**：权限码已落双态门——未授权=403 fail-closed（`IAM_PERM_3001`），授权后可用；真实栈 E2E 双态验证过。

---

## 2. 代码事实基线（V1.2 更新 @ e1feb5f）

| # | 事实 | 证据 |
| --- | --- | --- |
| C1 | MCP 四工具已齐且契约冻结：claim（一次性明文 lease_token + generation）/context/progress（心跳=续租）/submit | `crates/pm-mcp/src/tools.rs`、`server.rs` |
| C2 | Worker 纯 DB 轮询：`claim_and_start` + `poll_active_runs` 四信号 + 下半程续租；循环体第 ⑤ 步含 `finish_run`/`recover_lost` | `run_executor.rs` |
| **C3（R-1 改正）** | 回收路径 **B3-2 = 旧 Run 落 `timed_out` + 新建 Run（attempt_no+1、`supersedes_run_id`、deadline COALESCE 重新固化）**；同 Run 重领是 B3-1（仅 `executor_session IS NULL OR stage='dispatch'`）；`session_replaced` 属 B3-1 覆盖路径。等待窗口内 Run（running/started/有 session）**只可能走 B3-2** | `claim.rs:100-102,496-547`、`lease.rs:137-167` |
| C4 | 续租前置 `deadline_at > now()` 且**租约未过期**（两通道皆是）——**过期租约在任何通道都不可续活** | `lease.rs:203-222`、`run_context.rs:102,142-146` |
| C5 | `delivery.task_waiting`：reason 四值 + questions **JSONB 数组**（默认 `'[]'`）+ deadline 列 + 每任务一条活跃 | `0010:90-105` |
| C6 | Task 13 态含 `awaiting_decision`；合法出边仅 Cancelled/Ready/Running（**无 →Submitted**）；离开需 resolve | `0010:149-157`、`pm-domain/task_transition.rs:37-39,83-88` |
| C7 | 进程终止能力现成：Windows Job Object / Unix 进程组 + ExitOutcome 五值 | `codex_executor.rs:417-685` |
| C8 | MCP 鉴权面：purpose 映射 + Host 白名单 + 权限表（新工具双态门 F-7 已落） | `apps/pm-api/src/auth.rs` |
| **C9（R-5 后改写：已实现）** | `worker_command` / `task_interaction` / `scheduling_shield` 列 / `interaction_response` 全部落地（0046/0047）；服务层 `interaction.rs`（1068 行）/`worker_command.rs`（512 行）；MCP 新工具 report_interaction/poll_commands/ack_command/report_consumption；Runner 应用 `apps/ai-pm-runner` | 迁移 0046/0047；INDEX 8a E2E 报告 |
| C10 | Outbox 13 项基础目录 + 本切片新增 interaction.* 事件（§3.1 P2-4 子集） | `outbox.rs`、`interaction.rs` |
| C11 | Workspace 布局 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}`；全仓无删除 Run 工作区代码 | `port.rs`、`codex_executor.rs` |
| **补核 E1（R-3 事实）** | `execution_mode` 按领取通道显式三分：Worker→`spawn`；MCP `runner:` 前缀 client_name→`spawn`（过渡形态标记）；其余 MCP→`human`。`lease_owner` 仍为自报拼接文本（`mcp:{client_name}:{token前8}`，注释明言不是凭证）——**Client 主体写入为 V-05-1 后目标态** | `claim.rs:32,701-758` |
| **补核 E2** | 等待窗口存活路径已闭：Runner tick 在 awaiting_continue 期间**无条件心跳续租**（存活即租约不过期）；shield 三层屏蔽已实现（S1=SQL 排除 run_executor.rs、S2=claim.rs 重领前置拒绝、S3=spawn 前校验） | `runner_loop.rs tick①`、`run_executor.rs:248-253`、`claim.rs:494-510` |
| **补核 E3（默认时长）** | `AI_PM_RUN_LEASE_SECS=300`、`AI_PM_RUN_DEADLINE_DEFAULT_SECS=3600`、`AI_PM_HUMAN_INTERVENTION_TIMEOUT_SECS=120`、命令 claim 租约 300 | `run_dispatch/mod.rs`、`worker_command.rs` |
| **补核 E4** | 超时清扫已实现：worker 循环调 `interaction_timeout_sweep`（平台侧扫描器，P2-2） | `apps/ai-pm-worker/main.rs:161`、`interaction.rs:507` |

---

## 3. Worker Command 命令查询面（§23.3 闭合项 1）

### 3.1 存储：`worker_command` 表（**已落地：0046**）

按 V1.3.1 §10.2 全字段落地 + 本稿实施定案：

| 字段 | 实施定案 |
| --- | --- |
| `worker_id` | **= 下发时目标 Run 的 `lease_owner` 值**。不做广播命令（V1 无需求）。**R-11 修订**：§3.6 续领事务内可对该 Run 非终局命令**同事务改路由**（审计保留） |
| `lease_generation` | 下发时自 Run 读取固化（语义 = **`issued_generation`**）。**R-2/R-11 增补列 `reacquired_generation`**（可空；经 §3.6 续领时写回，待下一迁移批 0048+；同批含 `task_interaction.previous_chain_id`，专家 2.6） |
| `command_type` | V0.5 集合 = `agent.process.terminate` · `agent.task.continue`（已实现，常量钉住） |
| `payload` | JSONB：terminate = §5.2 报文（`session_id` 规则见 R-8，§6）；continue = §5.4 报文 + **guard 前置快照**（`continuation_start_frozen` + `agent_deadline_at`，F-1 已实现） |
| `status` | `issued → delivered → acked \| failed`（四态 CHECK；已实现） |
| `claimed_by` / `claimed_at` / `claim_expires_at` / `claim_count` | 领取租约元数据（§3.2，已实现） |
| 幂等 | `command_id` 主键；同 `(run_id, command_type, interaction_id)` 仅一条非 failed 活跃命令 |

**Outbox 事件子集（P2-4 采纳，已实现）**：本切片落 `interaction.created` / `interaction.responded` / `interaction.consumed` / `interaction.timed_out` / `interaction.cancelled` / `interaction.retried` / `interaction.report_replayed` 七项；命令生命周期不入 Outbox（§10.5 不变）。

### 3.2 领取租约与崩溃重投（已实现）

`delivered` 语义 =「已被某执行端实例领取，处于有界 claim 租约内」。重投规则（已按 V1.1 定案实现）：

```text
可领取条件：status = 'issued'
         OR (status = 'delivered' AND claim_expires_at < now())
         OR (status = 'delivered' AND claimed_by = 调用方)   -- 同体提前重领

领取（原子）：UPDATE ... SET status='delivered', claimed_by=?, claim_expires_at=now()+租约,
              claim_count=claim_count+1
              WHERE command_id=? AND worker_id=调用方 AND <可领取条件>
```

- `AI_PM_COMMAND_CLAIM_LEASE_SECS` 默认 300s；重投复用同一 `command_id`（执行端按 command_id 幂等执行）；**failed 仅由明确最终失败条件产生**（有界重试耗尽），**claim 过期永远不是 failed**。
- 指标：`worker_command_claim_expired_total`、`claim_count` 分布、`worker_command_pending_age`。

### 3.3 身份双轨：Run Lease 身份 vs Command Delivery 身份

> **双持身份说明（P2-6 采纳）**：Runner 形态同时持两种身份——**Client 凭据过 `/mcp` 鉴权（命令面）+ 租约三要素过工具校验（执行面），二者缺一不可**；不得把 Client 凭据当作执行面凭据使用，也不得把 lease 三要素用于命令面。

| 通道 | 身份 | 校验要素 | 依赖有效 Run Lease？ |
| --- | --- | --- | --- |
| **执行面工具**（get_run_context / report_progress / submit_result / report_interaction / report_consumption） | Run Lease 身份 | token（一次性明文）+ generation + run scope（T-032 / ADR-0002 D2 七点） | **是** |
| **命令面工具**（poll_commands / ack_command / **reacquire_lease**） | **Command Delivery 身份**（V-05-1 后 = 经鉴权 Client 主体；服务器 Worker 形态 = worker_id 进程内直连；**V1 过渡 = `runner:` 前缀自报 client_name**，风险登记见下） | ① 凭据有效且未撤销（V-05-1 撤销即通道关闭）② **worker_id 绑定**：命令路由目标 == 调用方身份 ③ **generation 集合比对（R-2/R-11 修订）**：`run.lease_generation ∈ {command.lease_generation（issued）, command.reacquired_generation（经 §3.6 续领）}`——服务端读库比对，无需 token | **否** |

理由与边界（R-10 登记为 R-ACP-15）：

- 等待窗口内 Deadline 过期使租约不可续（D-02 + C4），命令通道若仍要求有效租约即自断——必须解耦。
- 解耦后的安全性由 worker_id 绑定 + generation 集合比对提供等价 fencing；等待窗口内 B3-2 被 S2 拦截（§4.2），`lease_owner` 稳定（续领改路由除外，§3.6，同事务+审计）。
- **R-ACP-15（对冻结基线的正式操作化修订）**：基线 §10.4"领取命令时校验 Lease Token、过期 Worker 不能执行命令"与 §16.3"Worker 受 Run Lease 约束"，由本表命令面规则**取代**；执行面写操作仍受七点约束无豁免。登记规格与 R-ACP-7 相同（§9）。
- **过渡形态风险登记（R-3；专家第 2 块修正收紧）**：V-05-1 落地前，`runner:` 前缀 client_name 为调用方自报文本，**不构成密码学身份**——过渡态仅限受控环境（E2E/内网）临时兼容，**生产禁令见 §3.6 生产启用门**；**client_name 必须登记并绑定当前认证主体**（过渡期最低防线，登记制随收敛批落地）；V-05-1 Client Registry + 凭据签发为命令面**硬前置**，并**升格为 R-16 对外（生产）启用的硬前置**（ACP-435/436 在过渡态不可测，V-05-1 后闭合）。实现已按 `runner:` 前缀标 `execution_mode='spawn'`（E1），过渡债务显式登记，勿视为已解决。

### 3.4 执行端轮询节奏

- Runner：复用心跳 poll 循环，`pm_poll_commands` 与 `pm_report_progress` 同循环不同调用；命令轮询间隔 `AI_PM_COMMAND_POLL_INTERVAL_SECS`（默认 3s）。命令通道不依赖租约续租有效性（§3.3）。
- Worker（服务器形态）：进程内直连应用服务，逻辑同构。
- 领取后即时执行；回执失败（网络）→ 下一轮重领（claim 过期或 claimed_by==自方）后重新回执（幂等）。

### 3.5 失败升级

| 场景 | 升级规则 |
| --- | --- |
| 执行端离线（**R-11 修订**） | 命令保持 issued / delivered（claim 过期后可重投），等待恢复，不超时作废。**离线超过租约 TTL 的 Run 恢复路径 = §3.6 续领**（不再是无出口的死锁态）；REST 查询面 + `interaction_shielded_runs` 指标保证可见性 |
| terminate 执行失败 | 有界重试（`AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS=10`，次数/间隔配置化）；耗尽 → `ack(failed)` → Task 阻塞路径 + 告警 + `continuation_start_frozen_at` 落值（§6.5）+ Interaction 保持 pending |
| continue 执行失败（含注入失败） | 命令 failed（receipt 注明失败阶段）；回复保留幂等重投；未携带回复的新进程已终止（§1.4 不变量） |
| Platform 下发后命令悬挂 | `worker_command_pending_age` 超阈值告警 |

### 3.6 同主体续领（same-owner re-lease）（R-2 + R-11 方案 a，新增节）

**问题闭合**：实现先行批已闭合 Runner **存活**路径（等待窗口 tick 心跳续租，租约不过期，E2E PASS）。残留 = **执行端离线超过租约 TTL**（PC 休眠/崩溃/断网；Model A 真人关闭 MCP 客户端）：重启后旧一次性 token 丢失、旧租约已死（C4 两通道皆不可续）、同 Run 零租约重取路径、continue 命令路由旧 worker_id 永无人消费 + shield 挡回收 → 死锁（复审 P0-2 残留半边）。本节给唯一合法恢复路径。

**裁决（R-ACP-16，已裁 = 方案 a）**：同主体续领扩展。方案 B（Runner 本地加密持久化租约凭据）**不采用为独立路径**（TTL 窗口仍在；作为未来增强可叠加）。

**发现面（Runner 重启后认领在途 Run；专家第 2 块裁决：独立工具，推翻 V1.2 初稿的双态 Poll）**：

- **`pm_discover_runs`（新工具，`PM_MCP_COMMAND`）**：发现 = Client 名下待恢复 Run 的**独立业务能力**，与 Poll（单 Run 命令轮询）分离——`pm_poll_commands` 保持原契约不动。返回字段（十个，专家指定）：`run_id` / `execution_mode` / `interaction_id` / `interaction_status` / `deadline_at` / `shield_state` / `lease_generation` / `lease_status` / `terminate_command_id`（适用时）/ `terminate_status`（适用时）。
- **发现范围与安全**：只返回**当前认证 Client 有权恢复**的 Run（存在可投递 continue 命令且 `scheduling_shield='start_wait'`）；**不返回人工回复正文与 Command payload**；支持分页；**重复调用不产生状态副作用**（纯只读）。
- Client 身份匹配规则：V-05-1 后 = `run.lease_client_id == 经鉴权 Client 主体`（严格相等）；**过渡形态（仅受控环境）** = client_name 登记制 + `run.lease_owner LIKE 'mcp:{client_name}:%'` 前缀匹配（见 §3.3 生产禁令）。
- Runner 循环接线：启动/重连时执行一轮发现；发现命中 → 逐 Run 走续领（幂等，重复发现无副作用）。

**续领操作**（应用服务操作；Runner 形态 = 新工具 `pm_reacquire_lease(project_id, run_id)`；Worker 形态 = 进程内直连同函数）：

```text
前置（单事务内校验）：
  run.scheduling_shield = 'start_wait'
  AND 调用方 Client 身份 == 该 Run 的 Client 身份（规则同发现面）
  AND run.deadline_at > now()                            -- Deadline 不续（T3）
  AND 该 Run 存在 responded Interaction（回复已持久化，回复不因续领丢失）

效果（同事务）：
  1) 同 Run 重签租约：lease_generation + 1（generation 每次"成功获取执行权"递增，T2）、
     新一次性 token 返回调用方、lease_expires_at = now() + AI_PM_RUN_LEASE_SECS、
     lease_owner 重签为新值（过渡 = mcp:{client_name}:{新token前8}）；
  2) 该 Run 全部非终局命令（issued/delivered）worker_id 同事务改路由至新值，
     命令行写回 reacquired_generation = 新 generation（审计保留原值）；
  3) 审计 run.lease_reacquired（含原/新 generation、触发 command_id）。
```

- `deadline_at` **不动**（T3 字面一致；离线恢复不构成 Deadline 顺延——过期仍走 ACP-07 授权；专家第 1 块批准：deadline 到期后禁止自动恢复执行，已持久化人工回复保留，后续执行进 ACP-07 人工授权）。**专家第 2 块补产品面**：UI 可提供**快捷入口直达 ACP-07 重新授权流程**——不直接修改原 deadline、不自动延长人工决策有效期、已持久化回复不丢弃；新授权必须形成**独立审计记录与独立有效期**；未取得新授权前不得自动启动 Session。
- `attempt_no` 不变、Run 行不变（**不是 B3-2 新建 Run**；S2 屏蔽语义不受影响——续领是显式应用服务操作，不经回收路径）。
- 续领后旧 token/旧 generation 立即失效（token 哈希比对失败，ACP-447）；旧 worker_id 路由值失效（poll 旧值=空集）。
- **改路由 ≠ 重建命令（专家约束）**：`command_id` / `idempotency_key` / `payload` 保持不变；`claim_count` **不重置**（重新领取才计入实际投递次数）。
- **generation fencing 覆盖迟到 ACK（专家约束，ACP-C3）**：续领后，旧执行端（旧 worker_id / 旧 generation）对重路由命令提交 ACK → 拒绝（worker_id 绑定失败 + generation 集合比对不含旧值），不污染命令/Run 状态（ACP-454）。
- **续领本身不得自动创建新 Session（专家约束）**——续领只恢复租约与命令可达性；Session 启动仍由 continue 命令显式触发。
- **terminate 确认的落位（专家关键修正，ACP-C4）**：terminate ACKED **不是续领的前置条件**，而是**新 Session 启动的前置条件**。续领后的完整时序：
  1. 同 Client 身份发现名下 `start_wait` Run（发现面，上）；
  2. 满足本节前置 → 同事务续领（generation+1）；
  3. 同事务将未终局命令重路由至新 lease_owner；
  4. 新执行端消费重路由的 terminate 命令，确认旧 Session 已终止；**旧进程已不存在 → 幂等终止确认**（§6.8 既有幂等语义：进程已退出则直接 acked；孤儿进程仍在 → 按 pid 文件强杀，C7）；
  5. **仅当 terminate 已确认（或具备等价的旧进程不存在证明）才允许执行 continue、启动新 Session**（= 决策表情形 6 / §9.4 检查 8 的判据，ACP-455）。
- 与 R-2 的差异收编：R-2 原文"作为 continue 执行的第 0 步"收敛为本节"发现面 + 显式续领操作"，continue 执行前置的 guard 快照（F-1）校验新 generation 通过后照常执行。

**凭据持久化裁决（专家第 2 块维持 + 补强）**：Runner **不本地持久化 lease_token**——一次性明文丢失即走本节续领路径；本地持久化方案 B 不采用为独立路径。**WP-3 增加：稳定 Client 身份的本地持久化**（重启后必须恢复原 Client 身份，否则无法走发现面/续领）。**续领失败重试纪律**：指数退避 + jitter；持续失败 → 告警 + **人工处置入口**（不允许 panic 导致恢复上下文丢失）；**身份失效或 deadline 到期 → 停止自动恢复**（转 ACP-07 / 人工路径）。

**Model A 适用性**：真人 MCP 客户端在等待窗口关闭后重连，同样经发现面（按其登记的 client 身份）+ 续领恢复投递；`execution_mode='human'` 的 Run **不可**经 `pm_report_interaction` 造新介入（ACP-448），但续领/消费回执不受此限。

**生产启用门（专家第 2 块修正，对 §3.3 过渡形态的收紧）**：前缀匹配**仅限受控环境（E2E/内网）临时兼容，不构成生产安全身份**；**V-05-1 Client Registry 升格为 R-16 对外（生产）启用的硬前置**——身份安全能力（client_name 登记 + 绑定认证主体 + 稳定 Client 身份校验）未完成前，**禁止在生产环境开放基于自报 client_name 的发现与续领**；generation 被抢占**不得视为无害的执行干扰**（抢占即原执行端被踢出，属安全事件）。

---

## 4. N-1 闭合：持久化调度保护状态

### 4.1 调度保护状态机（`task_run.scheduling_shield`，已落地 0046）

**载体**：`task_run.scheduling_shield` 持久化列（CHECK：none/human_wait/start_wait/timed_out + 部分索引，已实现；R-ACP-9 裁决不变，ACP-05 设计时再评估迁移 continuation 域）。由 InteractionService 在各事务内**显式维护**，不由 Interaction.status 推导：

```text
none ──创建 Interaction──▶ human_wait ──回复持久化──▶ start_wait ──continue acked──▶ none
  │                           │                       （INPUT_INJECTED；同事务 Task→running，
  │                           │                         链关闭，R-4）
  │                           ├──超时清扫（平台侧，E4）──▶ timed_out ──retry_of 新建──▶ human_wait
  │                           │                        │
  └──Interaction CANCELLED◄──┼────────────────────────┤（Task 取消/归档 → 全部归 none）
                             ▼                        ▼
                        （Task 活：→ none）（未人工处置：保持）
```

**六情形决策表（V1.2 修订：情形 4/5 按实现事实与 R-1/R-4 重写）**：

| 情形 | Interaction | 调度保护 | 自动调度（poll 四信号/回收/无人值守） | 新 Session 启动 |
| --- | --- | --- | --- | --- |
| 1. 人工等待中 | `pending` | `human_wait` | **屏蔽** | 禁止 |
| 2. 已回复未启动 | `responded` 且无 acked continue | `start_wait` | **屏蔽** | 禁止（仅 continue 命令可启动；离线恢复走 §3.6 续领后由命令启动） |
| 3. 等待超时未处置 | `timed_out`（无 retry 后继） | `timed_out` | **屏蔽** | 禁止 |
| 4. 新 Session 已启动（**R-4 修订**） | `responded` 且 continue **acked**（consumption 未达亦然） | `none` | 正常（新进程归 poll 管辖） | —（Task 已在 acked 事务内回 running，§5.3） |
| 5. 介入已取消（Task 活）（**R-1 按事实重写**） | `cancelled` | `none` | 正常——但**真实路径是 B3-2**：等待窗口后租约大概率已过期，回收 = 旧 Run 落 `timed_out`（`release_reason='interaction_cancelled'` 新值，R-ACP-14）+ **新建 attempt Run**（`supersedes_run_id`，无回复携带——介入已撤销，无回复可存在） | 允许（经 B3-2 新 Run 重领路径，R-ACP-14 已裁接受） |
| 6. 旧 Process 退出未确认 | 任意 | 维持原值 | **屏蔽** | **禁止**（判据 = terminate 命令 ack 状态，R-7 b，§6） |

补充转换规则：

- `timed_out` 后人工显式处置（重试/ACP-07/取消 Task）才离开；**超时永不解释为同意**。
- Task 取消/归档：未决 Interaction 全部 `cancelled`，guard → `none`。
- **情形 5 的事实声明（R-1）**：取消后不存在"同 Run 携原始上下文重领"——原 Run 已被回收路径终局化；"原始上下文"由 B3-2 新 Run 的 context 重建承载（claim_core 既有逻辑）。是否接受此重试语义 = **R-ACP-14（已裁：接受，2026-11-07 用户裁决，理由见 §9）**。
- `interaction_shielded_runs` 指标按 guard 值分维；shielded Run 对自动调度不可见是**预期行为**。

### 4.2 屏蔽点落位（三处，全部服务端判定，已实现）

| # | 现有机制 | 屏蔽规则（V1.2 定稿） | 实现落点 |
| --- | --- | --- | --- |
| S1 | `poll_active_runs`（run_executor.rs） | **整条迭代不可见（R-1）**：poll 候选 SQL 直接排除 `scheduling_shield <> 'none'` 的 Run——①②③④⑤ 全部不执行，含第 ⑤ 步 `finish_run`/兜底续租/`recover_lost`；等待中的 Run 不会被收尾为终态、不会被 B3-2 新建。实现取"SQL 排除 + 节流可观测"而非逐 poll 审计（F-11：噪声大于价值） | `run_executor.rs:248-253`（已实现） |
| S2 | 回收重领 B3-2（`recover_lost_run`） | 拦截对象 = **`recover_lost_run` 全部调用方**（`claim_core` 回收分支 / `run_executor::recover_lost` / `reconcile_orphans` 清理后续，R-1）：guard ≠ `none` → 拒绝，隔离原因 = `scheduling_shield`，不新建任何 Run；逐次审计 `skip_task_run, release_reason=scheduling_shield`（F-11） | `claim.rs:494-510`（已实现） |
| S3 | 崩溃恢复 + 无人值守领取（`AI_PM_RUN_DISPATCH`） | spawn 前校验 guard ≠ `none` → 不启动。**恢复后不得启动未携带人工回复的新进程** | spawn 前校验（已实现） |

> **F-11 回写保留**：shielded Run 的持续可见性由 `interaction_shielded_runs` 指标承载（F-5，随 ACP-08 可观测切片落地）。

### 4.3 等待窗口内的租约语义

- 执行端在等待窗口内**保持租约并继续续租**（`pm_report_progress`）；Deadline 过期后续租失败——预期行为，不触发终止、不触发回收（S1 已屏蔽）。
- **R-9 声明**：等待窗口内的续租是**执行端存活信号，不是 Agent 心跳**——它刷新 `last_agent_heartbeat_at` 但此时并无 Agent 进程；该语义写进 `pm_report_progress` 的 API 注释与监控口径（陈旧判定不得以此信号误判"Agent 活着"）。
- **guard → none 时重置 ②③ 判定起点（R-9）**：continue acked 事务内同步重置执行端的心跳/进展陈旧判定起点（Runner：`last_heartbeat`/`last_detect` 之外另设进展基线；实现注记：Runner 现于 continue ack 后归零 `awaiting_continue` 并由新 Session 首报重建基线——陈旧误判风险由续租频率 ≤ 心跳间隔覆盖，收敛批补显式重置）。
- **`report_progress(started)` 幂等已核对（R-9）**：首报 `started` 分支仅在 `executor_session IS NULL` 时写会话（T-031，run_context.rs:671）；Task 侧 claimed→running 经 `apply_transition`——R-4 后 acked 事务已把 Task 转为 running，同态迁移为幂等跳过，无误判风险。
- 「guard ≠ none + lease 过期」为合法稳态：命令面按 §3.3 独立身份可达；**离线恢复 = §3.6 续领**；Deadline 失效仍走 ACP-07 授权（本稿只保证"拒绝得干净"）。
- `lease_owner` 在等待窗口内不变更（S2 拦截），命令路由目标稳定——**唯一例外 = §3.6 续领的同事务改路由**。

---

## 5. ACP-01：Interaction 领域模型（落地定案）

### 5.1 `task_interaction` 表（已落地 0046）

| 项 | 定案 |
| --- | --- |
| 状态枚举 | `pending / responded / consumed / timed_out / cancelled`（RESPONDED 未消费合法稳态见 §1.5） |
| `chain_open` | 持久化布尔列（部分唯一索引 `UNIQUE(task_id) WHERE chain_open` 钉住）。**V1.2 维护规则（R-4 修订）**：创建/重试时 true；**continue acked（INPUT_INJECTED）时 false（新 Session 已携带回复启动，链因执行恢复而关闭）**；`cancelled` 时 false；`consumed` 时 false（幂等兜底，正常链已在 acked 关闭）；`timed_out` 保持 true |
| `retry_of` | 超时重试挂链字段（链内语义，专家第 2 块确认不变） |
| `previous_chain_id`（专家第 2 块新增，0048 迁移列） | **链间弱关联**：指向同一 Task 的**已关闭前驱链**最新 Interaction——只服务历史追溯/审计/UI 展示；约束见 §5.2 规则 6 |
| `continuation_start_frozen_at` | terminate 重试耗尽时落值（启动冻结，§6.5） |
| 事务边界 | 创建 Interaction + Task 转 awaiting_decision + task_waiting upsert + guard=human_wait + terminate 命令创建 = **同一应用服务事务**（已实现） |
| CANCELLED 触发 | Task 取消/归档联动取消未决 Interaction；Task 活跃时的显式取消走管理操作（guard → none，情形 5） |

### 5.2 介入链（链级唯一约束，R-4 修订定义）

**定义**：介入链 = 一条 Interaction 及其全部 `retry_of` 后代。**链开放 ⟺ 链内最新 Interaction 满足其一**：

1. `pending`；
2. `responded` 且无 acked continue（回复已持久化、新 Session 未启动）；
3. `timed_out`（未人工处置）。

**链关闭**：continue **acked（INPUT_INJECTED）**（执行恢复，R-4）/ `cancelled` / `consumed`（兜底）。链关闭后允许新建 Interaction（ACP-451：新 Session 运行中再次介入 = 新链）。

业务规则：

1. **同一 Task 同时至多一条开放介入链**（`UNIQUE(task_id) WHERE chain_open`）。
2. 超时重试必须 `retry_of` 挂同一链；重试创建时旧行 `chain_open=false`（superseded）、新行 true。
3. `responded` 但新 Session 未启动 → 链开放 → 不允许新建。
4. `cancelled` → 链关闭 → 允许新建。
5. `task_waiting` 活跃行 ⟺ 链开放：链首开时 upsert、链关闭（acked/cancelled/consumed）时 resolve（同一事务，§5.3）。
6. **链间弱关联（专家第 2 块裁决）**：新建链时可写 `previous_chain_id` → 同一 Task **已关闭**的前驱链最新 Interaction；**禁止自引用或成环**（应用服务层校验：目标链必须已关闭且早于本链）；**不改变 `retry_of` 的链内语义、不共享生命周期与状态**——只服务历史追溯、审计与 UI 展示。**不加冷却时间、不合并连续介入**：重复事件与真正新介入的区分靠 event_id 幂等 + Session 标识 + Run generation 校验（ACP-C3），不靠时间窗。

### 5.3 awaiting_decision 映射（§23.3 闭合项 4，R-4 修订）

| 时刻 | Task 状态动作 | task_waiting 动作 |
| --- | --- | --- |
| Interaction 创建（链开） | → `awaiting_decision`（经 task_transition，open question 门槛满足） | upsert 活跃行：reason=interaction.reason，**questions=[{question, options, interaction_id}]（单元素数组，P2-3）**，deadline=deadline_at |
| Interaction → responded | 不变 | 不变 |
| **continue acked（INPUT_INJECTED）（R-4 核心修订）** | **同事务：`awaiting_decision → running`（经 task_transition，`waiting_resolved=true`）+ `chain_open=false` + guard → none**——执行恢复与消费证据**解耦**：新 Session 已携带回复启动即恢复执行，不等消费回执 | **resolve**（`resolution_note` 引用 interaction_id + delivery_id，`ended_by='system'`） |
| Interaction → consumed | 不变（Task 已在 acked 时回 running；consumed 为 Interaction 级事实） | 已 resolve（幂等跳过） |
| Interaction → timed_out（无 retry） | 保持 awaiting_decision | 保留活跃行；不自动 resolve（§8.2） |
| Interaction → cancelled（Task 活） | 链关闭 → 回转前置状态或随 Task 终态（既有映射：awaiting_decision→Running，waiting_resolved 由取消提供） | resolve（ended_by=system） |

> **实现注记（WP-A 已收敛 2026-11-07）**：acked 事务内转 running + 链关闭 + waiting resolve 已落地（`interaction.rs resume_execution_after_continue_tx`，由 `worker_command::ack_command` continue-acked 分支同事务调用；审计 `interaction.execution_resumed`）；mark_consumed 保留为防御性兜底（同态幂等跳过）。测试：lifecycle 翻转至 R-4 语义 + `acp450_repeated_continue_ack_is_idempotent`（重复回执 replayed 且状态稳定）+ ACP-451 新链断言，acp_comm_face 20/20 绿。DSH 欠账窗口已闭合。

**专家复审补强（2026-11-07，批准 R-4 对策 + 五点约束）**：① INPUT_INJECTED 是 Continuation 恢复的**业务确认边界**（前提：协议保证输入已被执行端接受且 Continuation 进入可执行状态——§1.4 R-6 语义已满足）；② acked 同事务四写 = Task→running + waiting_resolved=true + chain_open=false + task_waiting resolve + guard→none（即上表）；③ **迟到、缺失或重复的 consumed 回执不得改变已确认的 Continuation 恢复结果**（consumed 只落 consumed_at/指标，任何重复提交幂等返回原结果）；④ 旧链关闭后允许建新链，**旧 Interaction 与新链保持独立关联**（`retry_of` 只在链内使用，链间无关联字段——UI 按 task+链分组展示）；⑤ **迁移期历史 responded+running 数据按 continue ACK 证据分类修复**（有 acked continue → 补关链/resolve；无 → 维持现状待人工），禁止仅凭 Task 状态推断恢复成功。

---

## 6. ACP-02：Process Termination（落地定案）

1. **触发**：Interaction 持久化事务内创建 terminate 命令（已实现；D-03 终裁不重开）。
2. **执行**：执行端领命令（§3.2）→ 校验（command_id/task/run/generation 集合比对/status）→ 既有终止能力（Job Object/进程组，C7）→ 等待真实退出。
3. **回执**：`pm_ack_command(acked, receipt={exit_outcome, pid, terminated_at})`（从严判据：acked 必须 exit_outcome，已实现）；回执失败 → §3.4 重投。
4. **重试与超时**：`AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS=10` + 有界重试。
5. **重试耗尽**（R-ACP-7 已裁）：`ack(failed)` → Task 阻塞 + 告警 + `continuation_start_frozen_at` 落值；**回复提交入口不冻结**（§8.3 六项检查独立管辖）。
6. **Platform 收到 acked 后（R-7 选 b 定稿；专家第 2 块维持 + 五点补强）**：**Process 事实权威 = `worker_command.receipt` + 审计 + `executor-events.jsonl`**——不新建 process 记录表，不扩展 `task_run_cleanup` 作为 Process 记录载体（0035 形状不合：PK(run_id)/清理进度状态域，复核 §6-6 维持）。**receipt Schema 补强（专家约束，随收敛批）**：① receipt 必须有**稳定 Schema 与版本号**（结构化、版本化——当前为自由 JSONB，收敛批钉 schema_version）；② 必须区分 `terminate_requested` 与 `terminate_confirmed` 两个时刻；③ 必须记录 **Run generation**（防旧回执污染新 Session，ACP-C3）；④ `executor-events.jsonl` 仅作**执行端诊断证据，不作为平台权威事实**；⑤ 出现多 Process 独立控制/迁移/结构化查询需求时**重新评估建表**（本轮不建 `task_run_process`）。情形 6 / continue 前置检查 8 的唯一判据 = **terminate 命令 ack 状态**（存在未 acked/failed 的 terminate → 拒绝启动）。> 实现注记：F-10 在 terminate acked 时向 `task_run_cleanup` 写一行 `completed` 终态（清理闭环注记）——登记为**无害旁注**，非 Process 记录权威载体，不承担查询职责。Workspace 保留（§15 四不清理）。
7. **报文身份定案（R-8）**：terminate payload `session_id := task_run.executor_session`（Platform Session ID）；`process_id` V1 允许为空，执行端按 Run（pid 文件）定位进程——对基线 §5.2"绑定 Process"的这一放宽登记为实施定案。Interaction 的 `session_id` 同源派生；`continuation_generation` ACP-05 落地前恒 1。
8. **命令幂等**：重复 terminate（重投）→ 执行端发现进程已退出 → 直接 acked（已实现语义）。
9. **Runner 本地进程账本**（服务器侧清理机制触及不到用户 PC 进程）：列入 WP-3 Runner 设计项，本稿只立指针。

---

## 7. ACP-08：Adapter 能力与集成（V0.5 三家）

### 7.1 能力模型（静态声明，不建动态 Discovery）

`AgentCapabilities`（V1.3.1 §9）+ 本稿新增项，经 Client Registry（V-05-1）能力三分结构声明：

| 能力 | V0.5 要求 | Codex | ZCode | DSH |
| --- | --- | --- | --- | --- |
| Task Execution / Workspace Binding / New Session / Initial Context / Process Termination | §13.1 必须 | ✅ | ✅ | ✅ |
| `interaction_detection` | = `marker_file`（§1.2，文件名含 event_id） | `.aipm/interaction.{event_id}.json` 约定写入引导 | 同左 | 同左 |
| `consumption_evidence`（R-6 落行） | §1.5 双证据；**按注入方式定"已接收"** | 启动参数/提示文件注入 → `injected=启动成功`；`process_alive` 有效 | 同左 | **stdin 流式 → `injected=首个输出轮次`；无法满足 → N-6 合法稳态 + 告警，不得伪造** |

能力不足的 Client：调度②（能力过滤）拒绝派发 Model B Continuation 型任务；启动前报告，不允许运行中发现。

### 7.2 instruction.md 扩展（Codex 先行）

在现有 instruction 基础上追加：需要人工介入时写 `.aipm/interaction.{event_id}.json`（§1.2 Schema）；只创建新事件不修改已提交事件；不得通过任何 MCP 工具自称获得人工批准（A*）。**实现注记**：Runner E2E 经 `agent_wrapper.sh` + opencode 引导验证了标记文件链路；instruction.md 正式扩展随 Codex Adapter 批落地。

### 7.3 执行端进程面继承

Runner 的 spawn/收割/终止能力自 `codex_executor.rs` 演化：原子 pid 文件、ExitOutcome 五值、Job Object/进程组、孤儿按 pid 文件强杀——全部沿用。**已实现**（apps/ai-pm-runner/adapter.rs）。

---

## 8. 验收标准

沿用 ACP-401~445 编号不动（ACP-428 文本按 R-1 修正：**"执行端崩溃于等待窗口后恢复：不得启动未携带人工回复的新进程；B3-2 隔离+新建 Run 被 S2 拦截"**）。本版新增 **ACP-446~453**：

| 编号 | 场景 | 必须保证 |
| --- | --- | --- |
| **ACP-446**（R-2） | continue 已 delivered，执行端在执行前崩溃、claim 租约过期 | 同一 command_id 可重新领取并执行（重投幂等） |
| **ACP-447**（R-2/R-11） | 续领后，持有旧 generation/旧 token 的执行端调用任何执行面工具 | 拒绝（T2：token 哈希比对失败；generation 集合比对不含旧值） |
| **ACP-448**（R-3） | `execution_mode='human'` 的 Run 调用 `pm_report_interaction` | 明确错误码拒绝，**不产生 Interaction、不产生 terminate 命令**（待收敛实现） |
| **ACP-449**（R-3） | 持合法 Client 身份但非该 Run 持有者调用 `pm_poll_commands` | 空集 + 审计（worker_id 绑定） |
| **ACP-450**（R-4） | DSH（无消费证据）continue acked 后新 Session 提交交付 | **接受**（Task 已在 acked 事务回 running）；Interaction 停留 `responded` + `interaction_unconsumed_total` 计数 |
| **ACP-451**（R-4） | 新 Session 运行中再次上报介入 | 新链创建成功（旧链已因 acked 关闭） |
| **ACP-452**（R-11a） | 执行端离线超 `AI_PM_RUN_LEASE_SECS` 后恢复（Runner 重启 / Model A 重连） | 发现面命中 → §3.6 续领 → continue 在**同 Run** 上执行成功；`deadline_at` 不变、`attempt_no` 不变、Run 行不变 |
| **ACP-453**（R-11c） | 已回复但租约过期（离线窗口内） | 人工回复**不静默丢弃**：`interaction_response` 持久化在先；续领后投递成功；恢复前 REST 查询面可见（"等待执行端恢复"态）+ shield 指标 |
| **ACP-454**（专家约束，ACP-C3） | 续领后，旧执行端（旧 worker_id/旧 generation）对重路由命令提交 ACK，或以旧凭据做任何状态写入 | **拒绝且不污染**：worker_id 绑定失败 + generation 集合比对不含旧值；命令/Run/Interaction 状态不变，计告警指标 |
| **ACP-455**（专家约束，ACP-C4） | 续领成功但 terminate 未确认（无旧进程不存在证明）时执行 continue | **拒绝启动新 Session**（情形 6 / §9.4 检查 8 判据）；terminate 命令先行投递消费，确认后才放行 continue |
| ACP-445 补注（P2-5） | Task 取消与 Interaction 上报并发 | 互斥机制 = **Task 行 `FOR UPDATE`**（interaction.rs 事务首语句，已实现） |

**E2E 必测场景（专家两轮终审合并，随收敛批真实栈验证）**：第 1 块四项——① 无 consumed 证据的 Adapter（DSH 形态，对应 ACP-450）；② Runner 离线超租约 TTL 后恢复（ACP-452/453）；③ 旧 Runner 迟到 ACK（ACP-454）；④ terminate 未确认时禁止启动新 Session（ACP-455）。第 2 块六项——⑤ Runner 重启且 lease_token 已丢失时同主体恢复成功；⑥ 未授权 Client 无法发现或抢占其他 Client 的 Run；⑦ `pm_discover_runs` 不改变 `pm_poll_commands` 原契约（双工具并存回归）；⑧ deadline 到期不自动恢复执行、人工回复仍保留；⑨ 同一 Task 连续两次真实人工介入形成两条独立且可追溯的链（`previous_chain_id` 串联）；⑩ 旧 generation 的事件与回执不能污染新 Session。

---

## 9. 裁决记录

**2026-11-07 用户裁决：R-ACP-1~9 全部按本稿建议执行**（V1.1 复审前置裁决）：

| # | 事项 | 裁决 |
| --- | --- | --- |
| R-ACP-1 | N-2 物理通道 | ✅ a) 标记文件主通道 + MCP 上报（§1.2；文件名 V1.2 修订为含 event_id，P2-1） |
| R-ACP-2 | N-1 屏蔽机制 | ✅ a) 持久化 `scheduling_shield` + 六情形决策表（§4.1） |
| R-ACP-3 | 命令路由与身份 | ✅ a) worker_id=lease_owner + Command Delivery 独立身份（§3.3；V1.2 补齐 Client 主体链路 + V-05-1 硬前置，R-3） |
| R-ACP-4 | 双写 | ✅ a) 同应用服务事务双写（§5.3；V1.2 按 R-4 把执行恢复移入 acked 事务） |
| R-ACP-5 | 消费证据门槛 | ✅ a) 双证据硬门槛 + N-6 稳态 + 三阶段分离（§1.4/§1.5；V1.2 按 R-6 精化"injected"定义） |
| R-ACP-6 | 迁移号 | ✅ 备注改写（**R-5 事实更新**）：原定 0045~0046 → 0045 被 DEF-01 顺移占用 → **ACP 批落地为 0046（task_interaction + worker_command + scheduling_shield）/ 0047（interaction_response），已应用**；后续批次自 0048 起，落地前重核 `docs/reference/ai-pm-migration-ledger.md` |
| R-ACP-7 | terminate 失败后的冻结范围 | ✅ a) 仅冻结新 Session 启动，回复提交按 §8.3 保持开放（对基线 §11.4 的正式操作化，已随实现落地） |
| R-ACP-8 | 新权限码命名 | ✅ `PM_MCP_AGENT_REPORT` / `PM_MCP_COMMAND`（V1.2 增 `pm_reacquire_lease` 归 `PM_MCP_COMMAND`，§1.6） |
| R-ACP-9 | 调度保护状态载体 | ✅ a) `task_run.scheduling_shield` 列先行（已落地 0046） |

**V1.2 新增裁决项**：

| # | 事项 | 状态 |
| --- | --- | --- |
| **R-ACP-14**（R-1 引出；复审建议编号 10 因 ACP-03 稿 R-ACP-10~13 已裁被占，按后实施者纪律顺移） | 介入取消（情形 5）后是否接受"以 B3-2 新 attempt Run 重试" | ✅ **接受（2026-11-07 用户裁决）**。理由：①原 Run 事实已放弃（进程已被 terminate 杀 + 介入已显式撤销，不存在"半途工作"可保护）；②新 Run 经 context 重建携带原始上下文，deadline COALESCE 重新固化在此语义下可接受（人工显式取消 ≠ 自动刷新 Deadline，不违基线 §1.3 意图）；③替代方案（取消=Run 永终、Task 停滞待人工重启）徒增人工负担而无安全增益。随裁决定 `release_reason='interaction_cancelled'` 新值 |
| **R-ACP-15**（R-10，**登记生效**） | 命令面不依赖有效 Run Lease（worker_id 绑定 + generation 集合比对提供等价 fencing） | ✅ 登记为**对冻结基线 §10.4/§16.3 的正式操作化修订**（与 R-ACP-7 同规格）；执行面写操作仍受 ADR-0002 D2 七点约束，无豁免 |
| **R-ACP-16**（R-11，**已裁**） | 等待窗口执行端离线（超租约 TTL）恢复路径 | ✅ **方案 a：同主体续领扩展**（§3.6：发现面 + `pm_reacquire_lease` + 同事务改路由 + `reacquired_generation` 写回；2026-11-07 用户批准按方案 a 起草 V1.2；**专家复审批准 + 关键修正：terminate ACKED 从续领前置改判为 New Session 启动前置（§3.6 时序 5 步，ACP-C4）**）。方案 B（本地持久化凭据）不采用为独立路径。离线恢复不是 Deadline 顺延理由（T3 不变，专家批准）；已持久化人工回复永不因离线丢弃（ACP-453） |

### 横向架构约束（ACP-C1~C4，专家复审 2026-11-07 补充，全切片长期有效）

| # | 约束 | 落位 |
| --- | --- | --- |
| **ACP-C1** | 执行端命令必须具有**有效、可验证的路由目标**（命令创建事务内保证可投递性；与 execution_mode 准入两层分离） | §1.3 准入行；`pm_report_interaction` 准入 + 命令创建同事务校验 |
| **ACP-C2** | **INPUT_INJECTED 是业务恢复确认边界，consumed 仅为观测事实** | §1.4 / §5.3（R-4） |
| **ACP-C3** | 执行端回执必须校验 **Run generation**——旧 generation 的迟到 ACK 与一切状态写入不得污染新状态 | §3.3 ③ / §3.6 fencing / ACP-454 |
| **ACP-C4** | **租约续领与 Session 重启分离**——Session 重启必须经过旧 Session 终止确认（terminate ACKED 或等价的旧进程不存在证明） | §3.6 时序 / 情形 6 / ACP-455 |

**专家复审状态（第 1 块：活风险三项）**：ACP-448 / ACP-450/451 / ACP-452/453 设计方向**全部批准**，补充约束已收编上文；意见原文归档 `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-1.md`。

**专家复审状态（第 2 块：六项设计取舍，2026-11-07）**：**四维持两修正**——2.1 R-7 选 b 维持（receipt Schema 五点补强，§6.6）；2.2 R-16 不持久化 token 维持（WP-3 Client 身份持久化 + 退避/告警/停止条件，§3.6）；2.3 T3 严格不顺延维持（UI 快捷入口直达 ACP-07，§3.6）；**2.4 V-05-1 修正**（前缀匹配仅受控环境、client_name 登记制、V-05-1 升格为 R-16 生产启用硬前置，§3.3/§3.6 生产启用门）；**2.5 推翻双态 Poll**（独立工具 `pm_discover_runs`，Poll 契约不动，§1.6/§3.6）；2.6 Chain 维持多独立链 + 新增 `previous_chain_id` 弱关联（不加冷却不合并，§5.1/§5.2）。总评=**以上属 V1.2 实施设计收敛，不改变 ACP 总体架构**；意见原文归档 `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-2.md`。

**专家复审状态（第 3 块：排期依赖，2026-11-07，三轮复审完成）**：方向全赞同 + 三处调整——① **V-05-1 升格平台级 Client Identity Foundation**（立即插关键路径；最小生产可用集**必须含撤销/禁用 + 权限码字典**，推翻我方"撤销后补"倾向；撤销立即影响 discover/续领/轮询/上报全部工具面）；② **ACP-09 改贯穿式两阶段**（定向回归随 Correctness Fix；完整回归在 05~07 后）；③ **ACP-04 与 05 联合设计**先闭数据契约、按数据依赖定先后（默认 04 核心持久化先行），不推迟到最后。另裁：迁移 0048（ACP 收敛）/0049（Client Registry）拆分 + lease_client_id 回填三分类（历史兼容≠历史身份可信）；收敛批拆 Correctness Fix / Recovery Foundation 两层独立交付；**task_repository P0 独立启动**（六约束、沿用既有 Project Repository 领域对象）；AI Team V2 与 ACP = **双主线共享基础设施**（V2 可提前开始但不得另造第二套 Client Identity）。意见原文归档 `...-expert-review-3.md`；排期全景落 `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md`。

---

## 10. 工程纪律引用

1. **不修改 MCP 四工具契约**；新增六工具（report_interaction / report_consumption / poll_commands / ack_command / reacquire_lease / **discover_runs**）走独立权限码，`/mcp` 鉴权面仅扩权限表行。
2. 禁重建区继续有效：不建 task_execution/execution_result、不改 13 态状态机与 Run 7 态（scheduling_shield 为调度保护标志非状态机扩展）、WAITING_FOR_* 不做成状态、四工具契约不动、`recover_lost_run`/B3-2 回收语义不为本切片改写（续领 §3.6 是独立显式操作，不经回收路径）。
3. 全部写入经 `pm-application` 统一应用服务（V-6）；MCP/Worker/Runner 仅适配层。
4. 迁移：0046/0047 已应用（R-ACP-6 事实更新）；**后续按专家第 3 块拆分：0048 = ACP 收敛迁移（`worker_command.reacquired_generation` + `task_interaction.previous_chain_id`），0049 = Client Registry（登记表 + `task_run.lease_client_id` + 历史回填三分类 + 权限码字典）**——两类内容独立审查/测试/回滚，不混批；迁移号由单一实施负责人统一分配，进入共享分支后并行会话不得自建同号；落地前重核 `docs/reference/ai-pm-migration-ledger.md`；主 checkout 有并行会话在途物，禁 sweep、禁 `git add -A`。
5. 本稿落 `docs/scenarios/s03/ui/acp/`；**收敛批两层交付（专家第 3 块定稿，同批不同 PR、独立提交独立验收）**——**层 1 Correctness Fix（P0，WP-A 已落地 2026-11-07：①② 代码+测试完成，③ 定向回归部分=单测层完成（human 拒报/重复 ACK 幂等/链闭再介入），真实栈定向回归随下一台 Runner E2E 补；遗留=孤儿 terminate 存量定向审计（迁移期一次性脚本，随 Recovery Foundation 批））**：① acked 事务内 Task→running + 链关闭（R-4，ACP-450/451）✅；② `pm_report_interaction` execution_mode 准入（R-3，ACP-448）✅；③ Model A 定向回归（单测层 ✅ / 真实栈待补）。**层 2 Recovery Foundation（依赖 V-05-1 身份安全完成）**：④ `pm_discover_runs` + 续领 + 命令改路由（R-11/2.5，ACP-452/453 + 0048 列）；⑤ 标记文件名含 event_id（P2-1）；⑥ `previous_chain_id`（0048，2.6）；⑦ receipt Schema 版本化 + requested/confirmed + generation（2.1）；⑧ 续领退避/告警/人工处置 + 自动恢复停止条件（2.2）；⑨ UI 快捷入口 → ACP-07（2.3，随 ACP-07 稿）；⑩ 过渡身份 → V-05-1 Client 主体（R-16 生产启用硬前置，0049）。**排期全景见 `docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md`**。

---

**文档结束**
