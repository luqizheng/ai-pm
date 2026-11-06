# Project AI Team V2 架构基线 · 现有代码对照核验报告 V1.0

> 状态：核验完成 · 对照对象 `docs/project-ai-team-v2/ai-pm-v2-architecture-baseline-v1.md`（Q1=A / Q2=B / Q3=A）
> 日期：2026-11-06
> 基线：ai-pm master @ a19fdb7（含工作区在途未提交物，已单独标注）
> 方法：四路并行代码侦察（AI 配置链路 / Task Run 管线 / MCP·Device·身份 / 仓库授权模型），全部结论附 file:line 证据，关键断言已人工抽查复核。

---

## 一、总结论

1. **V2 的"Task 与 TaskExecution 分离"已经存在，且比 V2 草案更细**：`delivery.task_run`（0010~0032 十次演化）就是 TaskExecution——独立主键、attempt_no 分轨编号、7 值状态机、租约/世代 fencing/绝对截止/双心跳、失败与清理账本、只追加语义。V2 领域模型中的 `task_execution / execution_result` 重新建表 = 推倒既有执行体系，**违反 V2 自己的第 1 条红线精神（Control Plane 不重造执行 Runtime，同样不重造既有执行数据面）**。

2. **V2 的任务状态机是现有 13 态状态机的真子集**。DRAFT→READY→RUNNING→SUBMITTED→REVIEWING→COMPLETED 每一环都已在 `delivery.task` CHECK（0010:149-157）+ 30 条边守卫矩阵（pm-domain/task_transition.rs）+ 唯一受守卫通道 `apply_task_transition` 中存在。V2 的 SCHEDULING/ASSIGNED/DISPATCHED 三态被现有 Run 的 `queued→claimed→running` 语义等价覆盖；V2 的 `WAITING_FOR_*` 若做成状态，与已裁决的「awaiting_decision 单状态 + task_waiting 原因表」（0010:89-106，waiting.rs:5-8 §12.2 裁决）直接冲突，属倒退。

3. **MCP「任务领取/执行/提交」协议面已完整**：`pm_claim_task / pm_get_run_context / pm_report_progress / pm_submit_result` 四工具 + Run 级 lease_token（hash 存储 + generation fencing）+ 跨项目隔离（非成员 NotFound 防探测）。V2 §7「Adapter 层」在 V1 形态下就是 MCP 本身 + codex_executor spawn 通道，不需要新建协议。

4. **真正的缺口集中在六处**（详见 §六）：Role 实体、Capability Registry、自动调度策略层（可用性过滤+能力匹配）、Client 级凭据与授权-撤销语义、Client 级 Git 凭据下沉（架构级冲突待裁）、Native AI-PM 主链模型接线。

---

## 二、V2 概念 → 现有实现对照总表

| V2 提议实体/机制 | 现有对应 | 判定 | 证据 |
| --- | --- | --- | --- |
| `task`（工作单元） | `delivery.task`（13 态 + 契约四列 + 依赖/等待/取代） | **复用** | 0010:147-158；0015:15-19 |
| `task_dependency` | `delivery.task_dependency`（0021，DAG + FK 防跨项目） | **复用** | 0021:9-28 |
| `task_execution` | `delivery.task_run`（attempt_no/租约/fencing/失败/清理） | **复用，禁止重建** | 0010:17-44；0011:26-83 |
| `execution_event` | outbox_event + 执行端磁盘 executor-events.jsonl + audit_log；**库内无 run 事件表** | 部分复用；监控面数据源需裁决（§六.6） | 0012:7-31；port.rs:141-151 |
| `execution_result` | `task_run.integration_status/request_fingerprint` + `task_repository_commit` 逐仓事实 + `v_task_delivery_manifest` 对账视图 | **复用** | 0022:21-81,105-181 |
| `execution_artifact` | `delivery.task_acceptance_criterion` + verdict（0002/0026）+ manifest | **复用** | 0026:15-46 |
| `task_review` | Run `run_kind='review'` 分轨 + `review_verdict(approved/changes_requested/inconclusive)` + D5 review 循环 | **复用** | 0026:15-79 |
| `project_client_authorization` | `devenv.device_project_binding`——**结构相似但被明文否认为授权机制**（0027:57「Binding 是声明与事实记录，不是权限授予机制」） | **需升级裁决**（§七.2） | 0027:56-80 |
| `agent_client`（owner_user_id） | `devenv.device`（`owner_principal_id` NOT NULL + FK→tenant_member，0027:26,48-50）+ `core.tenant_agent`（归因层，非 Client） | **骨架 70% 重合，缺凭据层**（§六.4） | 0027:23-51 |
| `agent_client_capability` | `devenv.device.readiness` 六层快照 + `last_check` JSONB（仅展示/审计，不进授权） | **缺**（Capability Registry 是 V2.0-D 新实体） | 0027:36-39；checks.rs:139-164 |
| `project_role`（职责定义） | 不存在。`core.project_member.role` 六值混合两种语义（真人访问角色 owner/member + Agent 职能 ai_pm/architect/developer/tester，0030:3-9）；`ai.agent_definition` 仅三 agent 种子 | **缺**（V2.0-A 新实体） | 0030:6-9；0020:14-29 |
| Platform 自动调度（Q2=B） | **不存在调度器**。现状=人工指定 assignee + 七道资格门 claimability_assessment + 双通道领取（MCP 自领 Model A / worker spawn Model B，SKIP LOCKED） | **缺策略层**（§六.3） | policy.rs:139-234；claim.rs:103-138 |
| 知识写入红线（§八） | propose-only 工具 + 质量门 + 人工确认 + 审计已在跑（pm_propose_document / pm_confirm_document_proposal / pm_veto_document_revision） | **复用** | pm-mcp/tools.rs:155-238 |
| Native AI-PM 用 Team Provider（§三.1/§九） | **链路未闭合**：主分析链仍 FakeModel（Task 17 TODO）；仅 repo_import 支线真调 Team Provider | **缺接线**（§六.6） | ai-pm-worker/main.rs:59-61；repo_model.rs:244-265 |
| Git 操作归外部 Client（§二边界表） | **架构相反**：服务端 mirror clone + worktree 注入，外部执行端全程不接触 Git 远端凭据 | **架构级冲突待裁**（§七.3） | codex_executor.rs:110-250 |

---

## 三、逐域核验

### 3.1 AI 配置与 LLM 链路

- **`ai.project_ai_config`**（0003:52-73 + 0019 加列）：项目级 AI 总开关 + `automation_mode(ai_lead|controlled)` + AI 执行身份锚 `ai_principal_id`。五个消费方：bootstrap 种子（bootstrap.rs:288-297）、job claim 过滤（job_queue.rs:41-44）、run 冻结快照（pm-ai/context.rs:109-116,167-173）、AI 身份（orchestrator.rs:222-227）、监管刹车 freeze/unfreeze（supervision/freeze.rs:43）。**但 `model_provider/model_name/model_config` 恒为 'fake'，模型字段名存实亡**——0041:23、0043:11 两次挂账「Phase 5 治理」未动。
- **`ai.llm_provider`（0007）已在 0041:117 DROP**，无残留。现行真相源 = `ai.team_llm_provider` + `ai.team_llm_credential`（0041）：7 态状态机（not_configured→configured→connected|failed→enabled⇄disabled→archived）、凭证 AES-256-GCM 替换制留痕、REST `/v1/pm/tenants/{tenant_id}/llm-providers*` 全套、审计落 tenant_audit_log。provider_type 服务层枚举 = openai/anthropic/gemini/deepseek/openai_compatible（anthropic 无适配器，create 时不校验 adapter_available，test 时才报 AdapterUnavailable——张力点）。
- **`ai.project_ai_model_policy`（0043，untracked，并行会话在途）**：项目级 Provider 指定（PK(tenant_id,project_id)），配套 GET/PUT `/v1/pm/projects/{project_id}/ai-model-policies` 端点同样未提交。解析语义：指定后严格不回退（上层可见回退 fake）；未指定 = 继承 Team 首个 enabled。**注意 UNIQUE(tenant_id,provider_id) 使一个 Provider 只能被一个项目指定**，与「Team 范围内多项目共享」直觉相悖，V2 对照时需确认是否有意。
- **唯一真发 LLM 的两条路**：①repo_import 冷启动（ai-pm-worker/main.rs:236 → repo_model.rs:244-265，经 `resolve_repo_call_config` 三级解析，失败静默回退 FakeRepoAnalysisModel——与 0043 严格不回退口径不一致，同仓两种降级哲学并存）；②主分析链 **FakeModel 硬编码**（ai-pm-worker/main.rs:59-61，TODO Task 17；已写好的 `OpenAiCompatibleModel`（openai_adapter.rs:139）零生产引用）。cognition/baseline 均确定性派生不调 LLM。
- **`ai.llm_call_record`（0041:81-98）半接入**：只有连接测试写入（team_llm.rs:662-676），主链与 repo_import 真实调用不落记录→token/成本/七类错误分类全盲；且「有记录即拒删」（team_llm.rs:921-929）使 delete 端点对测过的 Provider 近乎不可达。
- **死表**：`ai.skill_definition` / `ai.project_skill_binding`（0003:75-104）bootstrap 写 5 行后运行时零读取，真路由是 skill_router.rs:9-25 硬编码 match——「接线还是删除」需随 V2 裁决。`ai.agent_definition`（0020:14-29）是活的：喂 `ensure_tenant_agents` 建租户三 Agent 身份（bootstrap.rs:360-404），**不是旧内部 Agent 残留，是 ADR-0001 D3 三层身份结构**。
- **并行会话在途（未提交）**：0043/0044 两迁移、requirement_traceability.rs、team_llm 双侧项目策略端点、pm_requirements.rs 追溯端点。本次核验按工作区版本口径。

### 3.2 Task / Task Run 执行管线

- **Task↔Run 分离是本仓最核心设计**（0010:17 注释「执行记录另立归属，重试创建新 Run，不覆盖旧记录」）。Run 7 态：`queued/claimed/running/succeeded/failed/timed_out/cancelled`（0011:26-28）；Rust 状态常量 run_dispatch/mod.rs:26-47。租约 13 列（0011:31-46）+ 硬化 4 列（lease_generation/deadline_at/双心跳，0023:7-11）。
- **活跃唯一性**：`uq_task_run_active` 每 task 至多一条活跃 delivery Run（0011:82-83，0026 收窄）；review Run 分轨（0026:52-64）。
- **Task 13 态 + 30 边守卫**：pm-domain/task.rs:15-72、task_transition.rs:32-164（E1 delivery_submitted / E2 acceptance_evidence_complete / B4-A..D 终止性失败 / superseded 三前置）。唯一通道 `apply_task_transition`（command_executor/transition.rs:38），无绕行路径。
- **等待语义**：`awaiting_decision` 单状态 + `delivery.task_waiting` 原因表（needs_info/needs_decision/needs_approval/external_dependency 四值，0010:90-106）。**注意 task_dependency.rs:131-138 自述「尚无 waiting 写入口」**——waiting 表无生产写入方。
- **派发链**：outbox `TaskReady` → TaskReadyHandler（outbox_handlers.rs:41-144：锁 Project→锁 Task→资格校验 ready+assignee 非空→活跃 Run 幂等→insert，23505 幂等降级）。双 worker：pm-worker=纯 outbox 消费者；ai-pm-worker=六条循环（analysis job/repo_import/outbox/Run 调度执行/review/失败判定），全数据库轮询无 MQ。
- **结果回写双路**：MCP `pm_submit_result`（逐仓 commit 验证→task_repository_commit→task→Submitted）；worker spawn `finish_run` 条件 UPDATE 先到先得（run_executor.rs:868-887）+ failure 行 + RunCompleted outbox（handler 只记审计不改状态）。
- **执行事件**：**库内无 task_run_event 表**；0012 的「幂等」是 run 上 source_event_id 列。事件持久化 = outbox + 执行端磁盘 jsonl（port.rs:141-151）。V2 Execution Monitor（§10.4）的数据源需在此裁决（§七.6）。

### 3.3 MCP / Device / 执行者身份

- **MCP 19 工具**（pm-mcp/server.rs），四件执行工具齐全：claim（签发一次性 lease_token，明文只在响应/环境变量，库内 sha256）/run_context/progress（续租+心跳+首报 started）/submit_result。挂载 pm-api 同进程 `/mcp`（routes.rs:286），认证三层：require_bearer（真人 JWT）→ require_mcp_permission（PM_MCP_READ/DOCUMENT_WRITE）→ attach_device_claim（**非法声明不拒绝只忽略+审计**，device_claim.rs:107-111）。
- **`devenv.device`**：`owner_principal_id NOT NULL` + FK→tenant_member（0027:26,48-50）——**设备级个人归属已存在**；status active/revoked；readiness 六层由服务端计算不采信客户端（checks.rs:135-164）。六端点齐（pm_devices.rs:32-43）。**官方边界（mod.rs:1-12）：Device 是目录实体，不持凭据、不进认证链**。
- **`core.tenant_agent`**：归因层角色实例（agent_definition→tenant_agent→project_member 三层，ADR-0001 D3），`UNIQUE(principal_id)`；**`is_enabled` 是死信列**（0020:32 注释声称的 MCP 403 门禁未实现，全仓无读取方）。
- **`core.principal.kind`** = human/dev_agent/ai_pm/service（0001:33-35），无 'agent'/'executor' 取值；`principal_identity` = (provider,tenant_id,subject) 三元组映射（0018/0019）。**执行端 MCP 凭据 = 真人 API Key**，dev_agent principal 寄生真人身份，无独立 Client 身份层。
- **授权链**（claim）：真人成员校验（authz.rs:33-57，dev_agent 行不能用于校验）→ resolve_project_executor（81-98）→ claim_core 单事务（状态守卫+租约签发，claim.rs:690-741）。**`devenv.device_project_binding` 与 `core.tenant_agent` 均不在任何授权路径上**。租约五重校验（hash/generation/status/租约期/deadline，run_context.rs:122-155）。
- **撤销现状**：设备 revoke 只停归因（Revoked→忽略+审计），**不能使任何凭据失效**；lease_token 撤销链完整但绑定 Run 不绑定 Client；JWT 无吊销列表（间接路径=principal.is_active / tenant_member 级联）。

### 3.4 Repository 授权模型

- **git_capability_grant 历史**：0032 建（principal×repo×六能力+依赖闭包，默认 deny-all）→ **0033:26 整表 DROP**，裁决原文「不区分，都可以读写」。0033:16-21 保留：租户/项目隔离、Task 仓库范围、verified 门槛、Run 授权快照+`policy_snapshot_hash`（0032:15 引入且仍在用）、归档门禁。
- **最终粒度**：`RepoAccess` 单档位 ReadWrite（repo_access.rs:19-46，六能力字符串显式拒绝）；可访问仓 = `task_repository 关联 ∩ status='verified'`（repo_access.rs:61-92）。执行期实时交集 `execution_allows` = 定格快照 ∩ 仍 verified ∩ 未归档，快照缺项 deny-all（repo_access.rs:110-131；run_executor.rs:669-672）——**「每次执行检查授权有效性」这一半 V2 要求已具备，只是检查对象是 Run 不是 Client**。
- **上游空洞（P0 级发现）**：**生产代码不存在任何 `INSERT INTO delivery.task_repository`**（全仓仅测试命中）——Task↔仓库关联没有 API/MCP 维护入口，授权范围源头声明只存在于测试中。V2 做 Client 级 permission_scope 时必须一并补。
- **SSH key**：项目级一把（PK=project_id），AES-256-GCM，轮换覆盖式、吊销被 verified 仓引用阻塞；**明文只在服务端内存/临时文件**（ssh_key.rs:205-211「不暴露 REST」）。执行端交付 = 服务端 mirror clone + 从镜像切 worktree（codex_executor.rs:185-250），外部 Codex 只拿一次性 lease_token（codex_executor.rs:342），**全程不接触 Git 远端凭据**。
- **成员模型**：project_member.role 六值混合两语义（0030:3-5）；Rust 侧真人角色仅 owner/member 两档，无 admin/lead 分层（project_member.rs:14-42）。「授权者」概念仅存在于 requirement 域（approver_grant 0040，Lead OR 显式授权）。仓库域无授权者概念：任何项目成员都能 CRUD 仓库和 SSH key（support.rs:131-147 只查成员存在）。

---

## 四、重复建设风险清单（V2 数据库/API 设计的禁重建区）

1. **`task_execution` / `execution_result` 新表** → 用 `delivery.task_run` + `task_repository_commit` + manifest 视图。V2 §十一「Task 和 TaskExecution 必须分开」已被 0010/0011 满足，正文应改为引用现有表。
2. **新任务状态机** → 用现有 13 态 + 30 边矩阵。V2 §6.3 建议状态须归并进现有语义（对照见 §五）。
3. **`WAITING_FOR_CLIENT/DEPENDENCY/HUMAN` 三态** → 并入 `awaiting_decision` + `task_waiting.reason`（§12.2 已裁决范式）；若需新增原因值（如 no_client_available）是**加枚举值**不是加状态。
4. **任务领取/心跳/提交新协议** → MCP 四工具已是该协议，V2 新 Client 类型接入 = 在 claim.rs 的设备归因与凭据层扩展，不是新协议。
5. **知识写入通道** → propose/confirm/veto + 质量门 + 审计已在跑，V2 §八红线与现有实现一一对应。
6. **执行期仓库授权判定** → `execution_allows` 快照∩实时交集机制已满足「每次执行检查授权」，扩展点是检查对象加 Client 维度，不是新机制。
7. **Task 依赖/等待/取代** → task_dependency / task_waiting / task_supersession 三边表+守卫+幂等已齐。

---

## 五、V2 §6.3 状态机逐态归并表

| V2 建议态 | 现有归宿 | 说明 |
| --- | --- | --- |
| DRAFT | `draft` | 一致 |
| READY | `ready` | 一致；TaskReady outbox 由此触发建 Run |
| SCHEDULING | （无独立态） | 被 Run `queued` + claimability 七道门覆盖 |
| ASSIGNED | `task.assignee_id` 字段 + Run `queued→claimed` | 现状 assignee 人工指定；Q2=B 自动化改的是**谁写 assignee/如何选 Client**，不是加状态 |
| DISPATCHED | Run `claimed` | 租约签发即派发完成 |
| RUNNING | Run `running` + Task `running` | 双层一致 |
| SUBMITTED | `submitted`（E1 守卫） | 一致 |
| REVIEWING | `reviewing` + review Run 分轨 + review_verdict | 现有更细 |
| COMPLETED | `completed`（E2 守卫） | 一致 |
| WAITING_FOR_CLIENT | 建议：`task_waiting.reason` 新增枚举值 | 勿做状态（§12.2） |
| WAITING_FOR_DEPENDENCY | 已有：依赖不满足时 Run 退避回队不判失败（claim.rs:582-611） | 无需新态 |
| WAITING_FOR_HUMAN | `awaiting_decision` | 已有 |
| BLOCKED / FAILED / CANCELLED | `blocked` / `failed` / `cancelled` | 一致 |

---

## 六、缺口清单（真正缺什么，按 V2.0-A~H 映射）

| 阶段 | V2 内容 | 现状 | 缺口定性 |
| --- | --- | --- | --- |
| V2.0-A Role Model | project_role 实体（职责/Skills/Instructions/双 Contract/验收/约束） | **无此实体**。member.role 六值混合两语义；agent_definition 仅三种子 | **新建**。注意与 project_member.role 的语义切分：V2 Role=任务职责定义，member.role=参与者身份，两者不能混同 |
| V2.0-B Client Registry | 个人级 agent_client + capability 三分（声明/验证/当前可用） | `devenv.device` 有 owner 归属与 readiness，但立场是「目录实体不进认证链」 | **半缺**。缺：Client 级凭据（签发/吊销）、能力三分结构、client_type 字典。决策点=升级 device 还是新表（§七.2） |
| V2.0-C Project Authorization | project_client_authorization + permission_scope + 独立生命周期 + 撤销生效 | `device_project_binding` 结构相似但明文非授权；撤销链路只有 Run 级 | **需升级/新建**。核心缺口=「撤销授权→旧凭证失效」的直接绑定：现在根本没有可吊销的 Client 级凭证 |
| V2.0-D Capability Registry | 统一能力描述和验证 | readiness JSONB 仅展示；六能力模型已随 0033 撤销 | **新建**。可复用绑定声明的 ⊆ 校验模式（binding.rs:45-52）与服务端计算原则（checks.rs:135-138 不采信客户端） |
| V2.0-E Task Scheduler | 自动选择（授权过滤→能力过滤→可用性过滤→任务适配→派发） | 七道资格门+依赖判定+策略四层已有（≈①②⑤）；**③可用性过滤、④按 Role/Capability 自动匹配执行者缺失**；assignee 人工指定 | **新增策略层**。落点=TaskReadyHandler 前插调度决策 + claimability 扩门，不需要新派发机制 |
| V2.0-F Adapter / Worker | 多 Client Adapter 层 | MCP 通道天然多客户端（任何 MCP 工具都能走 Model A）；spawn 通道仅 codex/mock（AI_PM_RUN_EXECUTOR） | **部分有**。增量=codex 之外的 spawn 适配器（若有）；V2 §7 的 Adapter 职责（认证/状态同步/结果回传/中断）在 MCP 通道已实现 |
| V2.0-G Execution Monitor | 执行监控 UI 数据面 | `GET /v1/pm/task-runs` 只读已有；worker 内 poll_active_runs 已有 | **小缺口**：run 级事件/阶段/耗时查询面（库内无事件表，§七.6 裁决）+ 汇总端点 |
| V2.0-H Result & Knowledge | 交付验收+知识统一写入 | manifest 对账视图、review verdict、知识红线全在跑 | **基本齐**。增量=验收条件与 Role 的 Acceptance Criteria 挂接 |

**横切缺口（不属于任何阶段但必须进 DB/API 设计）**：
- **task_repository 生产写入口**（P0：授权范围源头声明无 API，上游空洞）。
- **task_waiting 生产写入口**（P1：表已建、谓词已写，无写入方）。
- **Native 主链模型接线**（Task 17）+ `project_ai_config.model_*` 退役路径（Phase 5 挂账）——V2 §三.1「Native AI-PM 使用 Team Provider」的主张当前在主链上不成立。
- **llm_call_record 全量接入**（主链/repo_import 真实调用不落记录）。
- `tenant_agent.is_enabled` 死列与注释失实（0020:32）——实现或删注释。
- skill_definition/project_skill_binding 死表处置（接线或删）。

---

## 七、需用户裁决的冲突点

1. **Git 操作归属（架构级）**：V2 §二边界表写「Git 工程操作 → 外部 Agent Client（受授权约束）」；现有架构是服务端代理全部 Git 远端操作（mirror clone/worktree/push 由平台持有项目级 SSH key），外部端零凭据。二选一：**a)** V2 正文修订为「Git 远端操作由 Platform 代理，Client 在平台交付的 workspace 内工作」——零新增攻击面，推荐；**b)** 真·Client 级 Git 凭据下沉（per-client 签发+吊销+作用域）——推翻 ADR-0002 的凭据分离立场，P0 安全面扩大。本报告默认按 a) 对照。
2. **agent_client 载体**：升级 `devenv.device`（已有 owner/status/绑定/readiness，但需推翻「不进认证链」边界注释并加凭据层）vs 新建 `agent_client` 表（干净但与 device 产生双轨）。倾向前者：device 的目录面被六端点+bootstrap 消费，推倒成本高；升级=把 binding 从「事实记录」升级为「授权授予+可撤销」+ 发 Client 凭据。
3. **调度入口与状态表达**：Q2=B 自动调度写 assignee（维持现有七道门语义）还是新增调度决策记录表？V2 §6.3 的 SCHEDULING/ASSIGNED 是否接受归并为 Run 态（§五表）？
4. **project_ai_model_policy 的 UNIQUE(tenant_id,provider_id)**（0043:24）：一个 Provider 只能被一个项目指定是否有意？V2 §九「Team 授权模型范围内多项目共享」直觉与之相悖，若有意限制须写进 V2 文档。
5. **Role 与 member.role 关系**：V2 project_role 是否取代 member.role 的 Agent 职能四值（ai_pm/architect/developer/tester）？若是，0030 的 CHECK 与 bootstrap 三 Agent 种子需随迁。
6. **执行事件持久化**：Execution Monitor（§10.4）要展示「当前执行阶段/异常与阻塞原因」；现有数据源=audit_log+outbox+磁盘 jsonl（库外）。是建 `task_run_event` 表（0012 名字被占用，需新名）还是 Monitor 直接消费 audit/outbox？
7. **Phase 5 双轨收敛时机**：`project_ai_config.model_*`（fake 死字段）与 team_llm 真相源的收敛是否提前到 V2.0 批次？不提前则 V2 文档 §三.1 需注记「主链接线为前置依赖」。

---

## 八、对后续 DB/API 设计的输入

- **迁移号基线**：0043/0044 已被并行会话占用（在途未提交）。V2 批次自 **0045 起**，落地前须先确认并行批已合入。
- **禁止动作重申**：不改 13 态任务状态机、不建 task_execution/execution_result 新表、不动 MCP 四工具契约、不把 WAITING_FOR_* 做成状态。
- **可直接复用的模式**：Run 授权快照+内容寻址哈希（policy_snapshot/policy_snapshot_hash）→ Client 授权快照可复用同一模式；binding 的 ⊆ 校验；readiness 服务端计算；AES-256-GCM 凭证替换制（team_llm_credential 先例）→ Client 凭据存储。
- **schema 归属建议**：Client/授权/Role/Capability 归 `core` 或新 `agent` schema；调度策略配置可挂 `delivery`；避免再开无谓 schema。

---

## 附：本次核验的证据基线

- 迁移全景：40 个迁移、8 schema（ai/core/delivery/devenv/discovery/governance/integration/source）。
- 抽查复核项：0033 DROP git_capability_grant、device.owner_principal_id、FakeModel TODO、binding 非授权注释、Run 7 态 CHECK、Task 13 态 CHECK、review_verdict 三值——全部与侦察报告一致。
- 在途未提交物（并行会话，照实计入）：0043/0044、requirement_traceability.rs、team_llm.rs（apps+crates）、pm_requirements.rs、tests/team_llm.rs。
