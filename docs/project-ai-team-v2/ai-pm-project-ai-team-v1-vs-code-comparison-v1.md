# Project AI Team V1.0 评审稿 × 当前代码综合比对报告 V1.0

> 状态：比对完成 · 只陈述事实与差距，裁决权在用户
> 日期：2026-11-08
> 比对对象：
> - 评审稿：`docs/project-ai-team-v2/ai-pm-project-ai-team-v1-review-draft-0{0..5}-*.md`（2026-11-08 落盘转录）
> - 代码：ai-pm 工作区 2026-11-08 口径（master 领先 origin 5 commits 未 push，含并行会话在途改动；迁移至 0055）
> - 裁决基准：`ai-pm-v2-architecture-baseline-v1.md`（Q1=A/Q2=B/Q3=A 已裁）+ `ai-pm-v2-code-alignment-verification-v1.md`（2026-11-06）+ V0.5 范围基线
> 方法：三路并行代码侦察（DB 迁移 / 后端模块与调度 / 前端 UI），全部结论附 file:line 证据。

---

## 一、总结论

1. **概念命名体系完全不同，但语义覆盖率很高。** V1.0 稿 §16.3 的 12 个实体（project_ai_team / ai_role / ai_role_version / ai_client / ai_task / ai_task_assignment / ai_task_execution / ai_work_session / ai_task_dependency / …）在代码中**零存在**（全仓 SQL+Rust 检索无命中）；但其主张的机制大多已有等价实现——TaskExecution≡`delivery.task_run`、依赖≡`delivery.task_dependency`、Client Registry≡`devenv.device`+0049 凭据、Team Provider≡0041、审批红线≡红线命令+require_human。**差异主要是命名与建模形态，不是能力空白。**

2. **V1.0 稿与 V2.0 已裁基线高度同源。** 稿 §0.1 的 AT-D01~D11 与 V2.0 裁决（Q1=A/Q2=B/Q3=A、不建内部多 Agent Runtime、Task/Execution 分离）方向一致；真正的增量主张集中在五处：AI Team 实体化组织、Role Catalog + Role Definition 版本化、Work Session、调度可解释留痕（候选/落选快照）、统一事件目录 + correlation/causation。

3. **真正的缺口（稿有、代码无、V2 亦未落地）**：①Role 实体与版本化（V2.0-A，仍开放）；②调度 ③可用性过滤 ④按 Role/Capability 自动匹配执行者（V2.0-E，assignee 现为人工指定）；③Conversation/Work Session 后端（零端点零表，前端三件套未接）；④Native AI-PM 主链真模型（FakeModel 仍在生产路径，Task 17 TODO）；⑤AI Team 管理 UI（/project/settings/ai 占位页）。

4. **与现行架构冲突/倒退的点（勿照搬稿的原始形态）**：
   - 稿的 `ai_task / ai_task_assignment / ai_task_execution / ai_work_session` 新建表 → 触碰 V2 对齐核验 §四禁重建区与三禁区①（task_run 即 TaskExecution）。
   - 稿 §6.3/§8.3 的 `WAITING_FOR_CLIENT / WAITING_FOR_EXECUTION` 状态形态 → 与已裁决的「`awaiting_decision` 单状态 + `task_waiting` 原因表」（0010:89-106）冲突，属倒退；需要时是**加原因枚举值，不是加状态**。
   - 稿 §5 的 Role×6 动作权限矩阵 → 现行等价机制是「红线 12 命令×required_role + claimability 七门 + MCP 权限码 + approver_grant」；矩阵化是新实体决策（随 V2.0-A），不是补漏。
   - 稿 §9.4 把 UNAUTHORIZED/DISABLED/REVOKED 塞进单一状态列 → 现行是 `status(active/revoked) × readiness(六层) × 凭据 revoked_at × granted_permissions` 正交模型，稿形态反而更粗。

5. **2026-11-06 对齐核验之后代码已前进一大截**（比对稿内状态时须注意时效）：0049 Client Registry（cpk 凭据+granted_permissions+`task_run.lease_client_id`）落地 → V2.0-B「Client 级凭据签发/撤销」大半闭环、V2.0-C 撤销语义在凭据级闭环；0050~0052 回复投递/Continuation/恢复基础；0053 Checkpoint-1 KB/PC 最小表；0054 ACP-06/07；Checkpoint-2 Runner 远端验证 R1/R2 三 commits（未 push）；`task_repository` 生产写入口已补（`task_repositories.rs:67-73`，对齐核验时的 P0 空洞已闭）；需求列表/详情页已接真后端（工作区在途，`RequirementsView.vue:12-27`）。

---

## 二、概念映射总表（V1.0 稿 → 现行代码）

| V1.0 稿概念（位置） | 现行代码对应 | 判定 |
| --- | --- | --- |
| Project AI Team（`project_ai_team`，§16.2） | 无独立实体。Team≡`core.tenant`（0044:5-7）；项目级 AI 开关与身份锚=`ai.project_ai_config`（0003:52-73） | 概念被拆解承载，实体不存在 |
| AI Project Manager 独立管理角色（AT-D12，§0.2） | `core.principal.kind='ai_pm'`（0001:33-35）+ `project_member.role='ai_pm'`（0030:7-9）+ `project_ai_config.ai_principal_id` 身份锚 + orchestrator AI 身份（orchestrator.rs:222-227） | **身份层已落**；管理协调行为=orchestrator 命令链，主链模型仍 FakeModel |
| 专业 Role 目录 13 项（§3.2） | `project_member.role` 四 Agent 职能值 ai_pm/architect/developer/tester（0030）+ `ai.agent_definition` 三种子（0020:14-24） | **子集覆盖 4/13**，无 Catalog 实体、无 P0~P3 分级、无启停状态 |
| Role Definition 六组字段（§4） | 无对应实体；`agent_definition` 仅 code+role_kind，无 instructions/schema/version | **缺**（=V2.0-A） |
| Role Version 不可变版本 + Task 快照（§4.2/§17.2） | 无。task 上无 role/version 引用；导入链的 `assignee_role` 只是解析临时值（repo_model.rs:115-131 → resolve_assignee 落 assignee_id） | **缺** |
| 权限矩阵 6 动作×Role（§5） | 红线 12 命令×required_role（policies/ai-pm-ai-lead-v2.yaml:15-40）+ claimability 七门 + MCP 权限码（auth.rs:668-681）+ `core.approver_grant`（0040） | **机制不同、红线语义已实现**；矩阵实体不存在 |
| P_effective 五层交集（§5.1） | 调度策略四层合成 organization→project→task_type→task（policy.rs:57-77）+ 仓库执行期快照∩实时交集 `execution_allows`（repo_access.rs:110-131）+ Run 授权快照哈希（0032:15） | **四层已有**；client 维度经 0049 granted_permissions 落地一半 |
| Task Dispatch Engine（§6.1） | `run_dispatch` 唯一调度落点：SKIP LOCKED + claimability 七门 + 退避回队（claim.rs:304-435；policy.rs:136-231；lease.rs:91-114） | **骨架已有**；③可用性过滤 ④按 Role/Capability 自动匹配**缺**（V2.0-E），assignee 人工指定 |
| Client 选择依据 Capability/Model Policy/Availability（§6.2） | 无匹配逻辑。现状=人工 assignee（须 `principal.kind='dev_agent'`，policy.rs:179-190）+ executor_kind 记录 | **缺**（与 V2 调度③④缺口同一件事） |
| WAITING_FOR_CLIENT / NEEDS_AUTHORIZATION（§6.3） | `awaiting_decision`+`task_waiting.reason`（waiting.rs:15-24）+ Run `requeue_with_backoff` 回 queued（lease.rs:91-114）+ `scheduling_shield`（0046:140-142） | **语义有、形态不同**；稿的状态化做法已被 §12.2 裁决否决 |
| 并行 / 依赖 / Human Gate（§6.4） | `delivery.task_dependency` DAG+环检测（0021）+ 每 task 活跃 Run 唯一（0011:82-83）+ shield human_wait/start_wait | **已实现** |
| AI Task（`ai_task`，§7.1） | `delivery.task` 13 态（task.rs:13-29）+ 30 边守卫（task_transition.rs） | **复用，禁新建**（稿 10 态 ⊂ 13 态，见 §五.2） |
| Task Assignment（`ai_task_assignment`，§7/§16） | `task.assignee_id` + Run 租约（claimed/lease_owner）+ `task_run.lease_client_id`（0049:28-35） | **字段级承载**，无独立分配表 |
| TaskExecution（`ai_task_execution`，§7.2） | `delivery.task_run` 7 态 + attempt_no + 租约/世代 fencing（0010:18-39；0011:26-53） | **复用，禁新建**（三禁区①） |
| 结果验收六步（§7.3） | E1 delivery_submitted / E2 acceptance_evidence_complete 守卫 + review Run 分轨+verdict（0026）+ `v_task_delivery_manifest` 对账 | **已实现且比稿更严** |
| Conversation（§8.2） | 后端无任何对话端点/表（routes.rs 全清单无 chat/conversation）；前端 `ai-interaction` 三件套为纯展示件未接后端 | **缺**（前端仅结构资产：CONTRACT.md+三件套+fixtures） |
| Work Session（`ai_work_session`，§8） | 无。Task 级最近似物=`task_interaction`+`continuation`（0046/0051）；前端全仓检索 workSession 零命中 | **缺**；且与 V0.5「结构化交互+Discussion Memory、非自由 Copilot」口径需先调和 |
| 个人 Client Registry（`ai_client`，§9） | `devenv.device`（0027）+ 0049 升级：cpk 凭据/credential_revoked_at/`granted_permissions`/`task_run.lease_client_id`（0049:9-35）；撤销每请求即时生效（auth.rs:219-274） | **已落**（走的正是 V2 对齐核验 §七.2 推荐的 device 升级路线） |
| Project Client Binding=授权（§9.3） | `devenv.device_project_binding`（0027:58-79）——**明文非授权机制**（devenv/mod.rs:7「声明与事实记录」）；`project_accessible` 按成员资格实时派生不落库（mod.rs:8-10） | **部分缺**：授权升级（可撤销授予+scope）未做，撤销目前只有凭据级 |
| Client 状态 6 值（§9.4） | `device.status(active/revoked)` × `readiness(六层 installed→failed)`（devenv/mod.rs:28-58）+ Run 级心跳三列（run_context.rs:540-541）——设备目录本身无心跳字段 | **覆盖、形态不同**（正交双列 vs 稿单列 6 值） |
| Team LLM Provider（§10） | `ai.team_llm_provider`+`team_llm_credential`（0041）+ REST 全套 + 项目级 `ai.project_ai_model_policy`（0043） | **已落**（项目级）；**按 Role 模型策略缺** |
| Provider 失败明确失败不静默切换（§10） | 项目指定后解析严格不回退（team_llm.rs:1167-1184）；但 repo_import 解析失败回退 Fake 并 `model_name='fake'` 留痕（repo_model.rs:259-264，注释自认过渡语义） | **基本满足**，残留一处过渡语义 |
| 知识写入统一协调（§11） | propose/confirm/veto + 质量门 + `ai.knowledge_write_operation` 幂等账本（0024:43-56）+ KB-PC 服务化（0048 intake + 0053 kb/pc schema + kb-pc-contracts 13 事件信封） | **已实现**，与稿 §11 流程几乎逐条对应 |
| Domain 权威对象 7 类（§11 表） | Requirement=delivery.requirement 三表（0039）；Cognition=ai.cognition_*（0036）/pc.cognition_*（0053）；Baseline=ai.baseline_*（0037）；Knowledge=kb.document*（0053）+ documents；Task=delivery.task；Design/ADR=documents 域 | **已落**（命名不同） |
| Project Settings→AI Team 五子页 UI（§12） | `/project/settings/ai` 占位页（router/index.ts:168-172）；repositories/runners 真数据页已有；Team LLM 页在租户设置 `/settings/team/ai-providers`；Execution Monitor 读端点 `GET /v1/pm/task-runs` 有、前端 `api/execution.ts` **零消费** | **缺**（Q4 范畴） |
| Dispatch 可解释记录（§18.2） | `policy_snapshot` JSON（含 policy_version/policy_hash/effective_policy）+ `release_reason`（七门 deny 码）+ `dispatch_attempts/next_dispatch_at` + 每次领取/跳过/重领写审计（claim.rs:851-935；lease.rs:246-330） | **大部分有**；候选/落选全量快照缺（候选是运行时查询，不留痕） |
| 失败三分类+有限重试（§18.3） | `RetryPolicy{max_run_attempts:3, max_consecutive_failures:3}`（task_run.rs:53-126）+ `last_failure_category` + `infrastructure_retry_count` + 指数退避 cap | **已实现** |
| 统一事件目录（§19） | outbox 目录：15 类有 handler 生产 + 9 类登记未产（outbox.rs:107-298）；KB-PC 13 事件带 correlation_id/causation_id（kb-pc-contracts/envelope.rs:26,29）；**平台主 outbox 与 audit_log 无 correlation/causation 字段** | **部分**；主链缺两 ID，事件目录远小于稿的 17 类 |
| 审批不属于 Role、Platform 强制（§20） | ①执行器红线：红线命令+非 Human→`RedLineBlocked`（executor.rs:49-62）②主体校验 `require_human`（requirement.rs:188-202；knowledge_proposal/support.rs:8-19）③调度审批证据 fail-closed（policy.rs:89-120）④「人类正式确认永不入 MCP 权限码」（auth.rs:662-667） | **已实现**，稿 §20.2 五条与实现几乎一一对应 |
| 实施范围 S1~S4（§13） | V0.5 四工作包路线（v05-execution-roadmap-v1.md）；S1/S2 的 Role/Client 部分≈V2.0-A/B（B 已落大半）；S3 调度执行=run_dispatch 已有；S4 UI 未动 | **路线重叠**，稿的分期可作为 V0.5 之后的批次参考 |

---

## 三、AI Task 状态映射（稿 §7.1 的 10 态 ⊂ 现行 13 态）

| 稿状态 | 现行归宿 | 说明 |
| --- | --- | --- |
| CREATED | `draft` | 一致 |
| READY | `ready` | 一致；TaskReady outbox 由此建 Run |
| ASSIGNED | （无独立态） | `task.assignee_id` 字段 + Run `queued→claimed` 覆盖；对齐核验 §五已裁归并 |
| RUNNING | `running` + Run `running` | 双层一致 |
| REVIEW | `reviewing`（+`submitted`/`changes_requested`） | 现行更细（review 分轨+verdict） |
| COMPLETED / BLOCKED / FAILED / CANCELLED | `completed` / `blocked` / `failed` / `cancelled` | 一致 |
| NEEDS_HUMAN | `awaiting_decision` | 单状态+原因表；勿做成状态 |
| — | `submitted` / `changes_requested` / `superseded` | 稿缺的三个现行态（验收提交/返工/取代） |

TaskExecution：稿 7 态与 Run 7 态一一对应（CREATED≈queued、DISPATCHED→claimed、TIMEOUT→timed_out，其余同名）。

---

## 四、V1.0 稿 §16.3 的 12 实体逐个判定

| 稿实体 | 判定 | 现行对应/说明 |
| --- | --- | --- |
| project_ai_team | **不建** | tenant≡Team（0044）、project_ai_config 承载项目级 AI 开关；新建=第二套组织边界，违反已裁模型 |
| ai_role_template | **缺（V2.0-A 设计输入）** | `ai.agent_definition` 仅三种子；稿的 13 Role Catalog/分类/优先级可作种子集扩展 |
| project_ai_role | **缺（V2.0-A）** | 现状 `project_member.role` 六值混合（真人 owner/member + Agent 四职能）；V2 对齐核验 §六已点名语义切分问题 |
| ai_role_version | **缺（V2.0-A）** | 全仓无 Role 版本化先例；稿 §4.2/§17.2 快照原则与现行 `policy_snapshot`+`policy_snapshot_hash` 内容寻址模式同构，可复用该模式 |
| ai_client | **不建** | 0049 已按 device 升级路线落地（cpk 凭据+权限码）；新建 agent_client 表=双轨（对齐核验 §七.2 已倾向升级路线且已走通） |
| project_ai_client_binding | **部分缺** | `device_project_binding` 仍是事实记录；「授权升级」（可撤销授予+scope）未做——稿 §9.3 的七字段清单可作升级设计输入 |
| ai_task | **不建** | `delivery.task` 13 态即任务本体（禁重建区 1） |
| ai_task_assignment | **不建** | assignee_id+租约+lease_client_id 字段级已覆盖；独立分配表无消费方 |
| ai_task_execution | **不建** | `delivery.task_run` 即 TaskExecution（三禁区①，禁重建区 1） |
| ai_work_session | **缺，先调和再立** | 与 V0.5「结构化交互非自由对话」口径冲突；若立须先过用户裁决（见 §六.3） |
| ai_work_session_task | **缺，同上** | 依赖 Work Session 裁决 |
| ai_task_dependency | **不建（已有同名无前缀版）** | `delivery.task_dependency`（0021）DAG+环检测+防跨项目 FK 全齐 |

**稿 §17.1 九条不变量核验**：①同项目唯一 Team——N/A（无 Team 实体；project_ai_config 每项目一行）；②唯一有效 Manager——`ai_principal_id` 唯一锚（0003）满足语义；③Task 保留 Role Version 引用——**不满足**（无 Role Version，属 V2.0-A 缺口）；④每次尝试独立 Execution——`UNIQUE(task_id, attempt_no)`（0010:37）满足；⑤撤销授权不接受新分配——cpk 凭据撤销即时（auth.rs:219-274）满足凭据级；⑥完成 Task 不重开——四终态守卫+supersession 走新 task 满足；⑦历史不丢——run/failure append-only+审计满足；⑧项目隔离——组合 FK+非成员 NotFound（authz.rs:33-57）满足；⑨审计全覆盖——约 20 个模块 append_audit+命令审计满足。**9 条中 7 条已被现行约束保证，1 条 N/A，1 条（Role Version）随 V2.0-A 落地。**

---

## 五、逐章比对要点（按分篇归组）

### 5.1 分篇 1（§0–§3：裁决边界/定位/组织/Catalog）

- **§0.1 AT-D01~D11**：与 V2.0 已裁基线一致，代码状态见 §二映射表；无新信息量。
- **§0.2 AT-D12（AI PM 独立管理角色）**：身份四件套已落（principal.kind/project_member.role/ai_principal_id/orchestrator 身份），但「管理协调」行为受制于主链 FakeModel——**AT-D12 的产品主张当前兑现度低**。
- **§2 组织架构**：稿明言「逻辑组织关系，不代表常驻 Agent 实例」——与现行「agent_definition 三种子只是身份/归因层，非常驻进程」完全一致，无冲突。
- **§3 Role Catalog 13 项**：现行 4 职能值（ai_pm/architect/developer/tester）。稿多出的 requirement_analyst/product_cognition/project_cognition/knowledge_manager/qa_engineer 等**无承载**；且稿自己把 Product/Project Cognition、Knowledge Manager 列为独立 Role，而现行 cognitions 是确定性派生域（pm_cognition/pm_baseline 不调 LLM）——**若按稿建 Catalog，等于给这些域引入 LLM 执行体，是新能力不是改名**。
- **§3.3 Role 四态（Enabled/Disabled/Not Configured/Unavailable）**：无对应；`tenant_agent.is_enabled` 是死列（0020:32，对齐核验 §3.3）——若做启停，先把该死列处理掉。

### 5.2 分篇 2（§4–§8：Role 定义/权限/调度/生命周期/Work Session）

- **§4 Role Definition 六组字段**：稿的字段组（Identity/Responsibilities/Capabilities/Contract/Governance/Execution）比 V2.0-A 现有描述更结构化，**可直接作为 V2.0-A 的 schema 设计输入**；Contract 组（input_schema/output_schema）与现行 MCP 工具的输入输出+验收条件体系可对接。
- **§5 权限矩阵**：稿 §5.1 四条「重要说明」在现行代码中全部有对应强制点（Developer EXECUTE 受授权=七门+repo 交集；KM WRITE_DRAFT≠确认权=propose/confirm 分离；AI PM 不能绕审批=红线通道；Client 权限≤Task 授权=granted_permissions+policy_snapshot）。**矩阵本身**（Role×动作）是新实体，随 V2.0-A。
- **§6.2 选择依据表**：七条件中 Authorization/Task Permission/Availability/Model Policy 四项目前不进调度匹配（只有 claimability 七门中的 assignee/repo/baseline/policy 门）；稿的「不按 Client 名称调度」原则与现行一致（executor_kind 只记录不选择）。
- **§6.3 无合适 Client**：稿 7 场景在现行全部有归宿（Native→主链、正常分配→spawn/MCP 双通道、离线→租约回收/重领/timed_out+supersede、无候选→退避回队、授权不足→cpk 401/403、执行失败→failure 账本+RetryPolicy、需人类→shield+awaiting_decision）。**稿若被采纳，只需把「WAITING_FOR_CLIENT」落为 `task_waiting.reason` 新枚举值（如 `no_client_available`），严禁做成状态。**
- **§7.3 结果验收**：稿 6 步 vs 现行 E1/E2 守卫+review 分轨+逐仓 commit 对账——现行覆盖且更严；稿的「输出契约检查」对应稿 §4 Contract 组，现行由 acceptance criterion + manifest 承担。
- **§8 Work Session**：**稿与现行差距最大的部分**。后端零承载；前端仅有未接线的展示三件套；现行 `task_interaction/continuation` 是 Task 级执行期交互，不是用户级工作会话。稿 §8.3 的 WAITING_FOR_EXECUTION 态违反 §12.2 裁决范式。**且 V0.5 口径（结构化交互+Discussion Memory，非自由 Copilot）比稿的 Conversation/WorkSession 野心小**——两套口径需用户裁决孰先。

### 5.3 分篇 3（§9–§12：Client/Provider/知识/UI）

- **§9 Client**：0049 后稿 §9.2 九字段几乎全有对应（owner_user_id→owner_principal_id、supported_capabilities→granted_permissions 三权限码、connection_status/last_heartbeat→readiness+last_check（Run 级心跳另计）、authorization_status→binding 行）。**缺口=授权升级**（§9.3 七字段中「可执行范围/解除授权时间」目前只有凭据级 revoked_at，binding 本身仍非授权）。
- **§10 模型策略**：稿「Role 决定职责 / Model Policy 决定资源 / Dispatch 决定执行资源是三个独立决策」——现行恰好对应三个独立机制（project_member.role / project_ai_model_policy / run_dispatch），**结构吻合**；缺的只是 policy 挂到 Role 粒度（现为项目粒度）。
- **§11 知识治理**：稿的六步流程（Analysis→Proposed→Validation→Coordination→Human Approval→Authoritative Update）与现行 knowledge_proposal→质量门→confirm/veto→权威表落库逐环对应；稿的 Domain 权威表与现行表清单见 §二映射。**稿在此章无增量主张，可作为行为规范文档直接引用实现。**
- **§12 UI**：/project/settings 已有 general(占位)/members(占位)/repositories(真)/runners(真)/ai(占位)——**稿的入口主张（Settings→AI Team）与现有骨架一致**；五子页全缺；Execution Monitor 的数据面裁决（对齐核验 §七.6：建事件表 vs 消费 audit/outbox）仍未裁。

### 5.4 分篇 4（§13–§15：范围/Q2-Q4/总结）

- **§13.2 暂不建设清单**：七项与现行约束完全同向（不建通用 Runtime=三禁区、不静默切 Provider=严格解析、AI 不审批=红线）。
- **§14 Q2/Q3/Q4 与代码事实的接近度**（注意：这是事实陈述，不是裁决，编号≠V2.0 基线 Q1-Q3）：
  - **Q2（专业 Role 执行方式）**：现行事实=同一 Native 模型链承担多职能产出 + project_member.role 四职能定位 + 外部 client 按 task 动态领取——**与 A（职责独立、执行资源共享）同构**；B（每 Role 独立 Agent 实例）与三禁区冲突。
  - **Q3（自定义 Role）**：现行 agent_definition 三种子平台预置、零自定义入口——**与 A 一致**；按 A 落地 V2.0-A 即零迁移。
  - **Q4（管理界面）**：现状=占位页，A/B 都是纯增量；A 的主要成本在 Execution Monitor 数据面（未裁）与五子页。
- **§15 总结**：稿自评「不能视为冻结基线」与本次比对结论一致。

### 5.5 分篇 5（§16–§22：Domain/事件/红线/验收）

- **§16.1 六子域**：Team Organization/Role Definition/Client Management/Dispatch & Execution/Work Coordination/Governance——现行模块划分（pm-application 下 devenv/run_dispatch/interaction/knowledge_proposal/requirement… + kb-pc-contracts）已近似按域组织，**稿的「不建六套微服务」主张与现行模块化单体一致**。
- **§18.1 Planning/Dispatch 分离**：现行已有对应物——orchestrator（AI 规划命令链）vs run_dispatch（资源分配唯一落点），职责分离事实上成立；缺口仅在主链 FakeModel 与调度 ③④。
- **§19 事件**：稿 17 类事件与现行 outbox 目录映射——TASK_COMPLETED≈RunCompleted、TASK_EXECUTION_FAILED≈失败账本+processing.failed、APPROVAL_*≈approval_request/decide+审计、CLIENT_AUTHORIZED/REVOKED≈设备/凭据审计（tenant_audit_log）、TEAM_CREATED/ROLE_* 全缺、WORK_SESSION_* N/A。**correlation_id/causation_id 仅 KB-PC 信封有（envelope.rs:26,29），主 outbox 没有**——稿的字段清单可作主 outbox 扩列设计的输入（增量改造，非冲突）。
- **§20 审批红线**：稿 §20.1 八行表与现行强制点映射完整（需求确认→requirement require_human；Cognition 变更→kb/pc 决策端点；ADR/Design→documents 红线确认；权限变更→tenant/project members 管理面+approver_grant；高风险工程→四层策略 require_human_approval+执行授权 0054；普通分配→Platform 策略）。**稿 §20.2 五条 Platform 规则在代码中全部成立**——这是全稿与实现吻合度最高的一章。

---

## 六、AT-AC-01~15 逐条现状判定

| ID | 稿验收标准 | 判定 | 现状依据 |
| --- | --- | --- | --- |
| AT-AC-01 | 创建 Project 自动初始化 AI Team+AI PM | **部分** | bootstrap 自动建 project_ai_config+AI 身份锚+租户三 Agent（bootstrap.rs:288-297,360-404）；但「AI Team 实体」不存在，且非稿语义的 Team |
| AT-AC-02 | 无外部 Client 仍能 Native AI 问答 | **缺** | 主链 FakeModel（ai-pm-worker/main.rs:59-61 Task 17 TODO）；无对话端点/表；openai_adapter 已写好但零生产引用（openai_adapter.rs:46-115） |
| AT-AC-03 | AI PM 建 Work Session 组织需求分析 | **缺** | 无 Work Session；需求工作现行=REST 人类路径（pm_requirements.rs 15 端点）+外部 Commit 接管 intake（0048） |
| AT-AC-04 | 多 Role 并行分析并汇总 | **部分** | 多 task 并行+依赖 DAG+活跃 Run 唯一已有；「按 Role 组织并发」的调度层无 |
| AT-AC-05 | 未授权 Client 不被调度 | **部分** | cpk 凭据+granted_permissions 撤销即时（0049/auth.rs:219-274）；但 binding 非授权、调度无 client 维度授权门 |
| AT-AC-06 | 离线 Client 不接新任务，已有执行按恢复策略 | **基本满足** | Run 级：租约过期回收/重领/timed_out+supersede（claim.rs:575-640）；设备目录本身无心跳字段（心跳在 Run） |
| AT-AC-07 | 失败保留 Execution，可重试或人工 | **满足** | task_run_failure append-only+RetryPolicy 3/3+退避+retry_exhausted 门 |
| AT-AC-08 | Role 更新→新 Task 用新版本，旧 Task 保留旧版 | **缺/不适用** | 无 Role Version 实体（V2.0-A）；稿的快照模式可用现行 policy_snapshot 内容寻址先例 |
| AT-AC-09 | 切换工作后原 Work Session 可恢复 | **缺** | Work Session 零承载（前后端皆无） |
| AT-AC-10 | AI 不能自行正式需求审批 | **满足** | 红线命令+require_human+审批证据 fail-closed+MCP 权限码纪律（executor.rs:49-62；requirement.rs:188-202；auth.rs:662-667） |
| AT-AC-11 | 正式知识更新走权威 Domain 规则 | **满足** | propose/confirm/veto+质量门+幂等账本+KB-PC 服务化 |
| AT-AC-12 | 撤销授权→不接新任务，历史与审计保留 | **基本满足** | 凭据撤销即时+append-only run/failure+审计；binding 软删（unbound_at） |
| AT-AC-13 | 项目间不得越权读写 | **满足** | 组合 FK 防跨项目边（0021:26-30）+非成员 NotFound（authz.rs:33-57）+Run 授权快照实时交集 |
| AT-AC-14 | Provider 故障明确失败不静默切换 | **部分** | 项目指定严格不回退（team_llm.rs:1167-1184）；残留 repo_import 回退 fake 过渡语义（repo_model.rs:259-264）；无用户可见失败提示面 |
| AT-AC-15 | 从 Work Session 追踪到 Task/Assignment/Execution | **部分** | Task→Run→审计/manifest 可追；Work Session 与 Assignment（独立分配记录）层缺 |

**小结：满足/基本满足 6，部分 6，缺 3（AC-02/03/09 全部压在「主链真模型 + 对话/Work Session 后端」这一个瓶颈上）。**

---

## 七、缺口清单（V1.0 稿视角，合并 V2 既有缺口）

**P0**
1. Native AI-PM 主链真模型接线（Task 17）——AC-02 直接依赖；adapter 已就绪（openai_adapter.rs）。
2. Role 实体 + 版本化（V2.0-A）——稿 §4 六组字段 + §3.2 Catalog 是现成设计输入；AC-08 依赖。
3. 调度 ③可用性过滤 ④按 Role/Capability 自动匹配执行者（V2.0-E）——AC-04/05 依赖；落点=TaskReadyHandler 前插调度决策 + claimability 扩门（对齐核验 §六既有结论）。
4. 对话/交互后端（端点+表+store）——AC-03/09 依赖；**须先裁决 Work Session/Conversation 与 V0.5「结构化交互」口径的关系**。

**P1**
5. Project Client Binding 授权升级（V2.0-C 余项：可撤销授予+scope 落到 binding，不止凭据级）——AC-05 依赖。
6. 主 outbox 增 correlation_id/causation_id（KB-PC 信封已有先例）。
7. 调度候选/落选全量留痕（现仅 release_reason+审计，无候选快照）——稿 §18.2 字段表是输入。
8. Execution Monitor 数据面裁决+汇总端点+前端消费（api/execution.ts 现零消费）。
9. `llm_call_record` 全量接入（主链/repo_import 真调用不落记录）。
10. `task_waiting` 生产写入口（若采纳 no_client_available 原因值则顺带补；表与谓词已建，对齐核验已点名无写入方）。

**P2**
11. AI Team 管理 UI 五子页 + Execution Monitor 页（Q4=A 时；入口骨架已在）。
12. 按 Role 粒度的 Model Policy（现项目粒度）。
13. Role Catalog 扩到稿的 13 角色口径时，Cognition/KB 域是否引入 LLM 执行体——**新能力决策，不是补名**。

---

## 八、对 V1.0 稿的处置建议（供裁决参考）

**可直接吸收（无冲突，多为「实现已如此，稿可作规范文本」）**
- §7.3 结果验收、§11 知识治理、§18.3 失败三分类、§20 审批红线：现行实现与稿几乎逐条对应，稿文本可直接作为行为规范/验收规范引用。
- §17.1 九条不变量：7 条已被现行约束保证，可原样进入后续 DB 设计的约束清单。
- §19 事件字段（correlation/causation）与 §18.2 可解释字段：作为主 outbox 扩列与调度留痕的设计输入。

**改造后吸收（换形态，不改语义）**
- §6.3/§8.3 的 WAITING_FOR_* → `task_waiting.reason` 枚举值；§7.1 状态 → 归并进 13 态（见本报告 §三）；§9.4 Client 状态 → 采纳现行 status×readiness 正交模型；§16.3 实体名 → 全部替换为现行表名（§四对照）。

**否决/禁改写（触碰禁重建区与已裁裁决）**
- 新建 ai_task / ai_task_assignment / ai_task_execution / project_ai_team 表。
- 把 WAITING_FOR_CLIENT 等做成 Task/Work Session 状态。
- 单列六值 Client 状态替代正交模型。

**待用户裁决**
- Q2/Q3/Q4（本稿编号）：代码事实分别接近 A/A/未定；裁决后即可按 §七缺口清单排批次。
- Work Session/Conversation 是否立项，以及与 V0.5「结构化交互+Discussion Memory」口径的先后关系。
- Role Catalog 是否按稿扩到 13 角色（涉及给 Cognition/KB 域引入 LLM 执行体，见 §七 P2-13）。

---

## 附：证据基线与时效声明

- 三路侦察覆盖：migrations/ 0029~0055 全 54 文件、apps/pm-api routes.rs 全端点清单、crates/pm-application run_dispatch/interaction/devenv/device_credential/team_llm/knowledge_proposal、crates/pm-mcp、apps/ai-pm-worker、ai-frontend/src 路由与 api 消费面。
- 关键断言抽查：13 态/7 态枚举原文、0049 凭据列、binding 非授权注释、FakeModel TODO、红线 yaml、awaiting_decision 原因四值——与 V2 对齐核验报告（2026-11-06）交叉一致；0049~0055 为其后新增，已按 2026-11-08 工作区口径计入。
- 工作区含并行会话在途改动（如 RequirementsView 接真、Checkpoint-2 R1/R2 三 commits 未 push）；本报告按工作区现状口径，非 origin/master 口径。
- 本报告只记录事实与差距；Q2/Q3/Q4 及所有「待用户裁决」项的正式裁决权在用户。
