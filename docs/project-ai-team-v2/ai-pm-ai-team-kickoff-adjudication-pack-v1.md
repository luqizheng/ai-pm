# AI Team 开工前置裁决包 V1.0

> 状态：**已裁决 · 2026-11-08（D-1~D-4 全部 = A，见 §〇 裁决记录）**
> 日期：2026-11-08
> 目的：AI Team（V2 本体）开工前的最后一组硬前置。四项全部属「设计/Schema 硬门」——不裁不能开 DB/API 设计；裁完即可按 §七 启动序推进。
> 依据：`ai-pm-project-ai-team-v1-vs-code-comparison-v1.md`（2026-11-08 三路代码侦察）+ `ai-pm-v2-code-alignment-verification-v1.md`（2026-11-06，§七 待裁冲突点）+ V1.0 评审稿分篇 4（§14）。
> 裁决方式：直接回复，如「D-1=A，D-2=A，D-3=A，D-4=A」或逐项修改。每项均给出推荐及理由。

## 〇、裁决记录（2026-11-08）

| 编号 | 裁决项 | 用户裁决 |
| --- | --- | --- |
| D-1 | project_role 实体口径 | **A** 新建实体+不可变 Role Version，与 member.role 语义分离；Catalog 首批 5 角色；V1 不开放自定义 |
| D-2 | 调度入口 | **A** 自动写 assignee + claimability 扩门③可用性④能力匹配；可解释字段并入 policy_snapshot |
| D-3 | project_ai_model_policy UNIQUE(tenant_id,provider_id) | **A** 放开多项目共享 |
| D-4 | Work Session / 对话口径 | **A** V0.5 口径先行：结构化交互 + 最小 conversation/message 两表；稿 §8 Work Session 挂账 |

附带：V1.0 稿 Q2/Q4 未裁，按包默认挂账。自本记录起，四项裁决生效为 AI Team V2 本体实施基线。

---

## 一、裁决总览

| 编号 | 裁决项 | 选项 | 推荐 | 分类依据 |
| --- | --- | --- | --- | --- |
| D-1 | project_role 实体口径（Role 模型） | A / B | **A** | 专家已定「硬前置：Team Role≠Project Role」（对齐核验 §七.5） |
| D-2 | 调度入口设计 | A / B | **A** | 专家已定「设计硬前置」（对齐核验 §七.3） |
| D-3 | project_ai_model_policy 的 UNIQUE(tenant_id,provider_id) | A / B | **A** | Schema 前必裁（对齐核验 §七.4） |
| D-4 | Work Session / 对话口径 | A / B / C | **A** | V1.0 稿新增裁决项，V2.0 基线未覆盖（比对报告 §六 P0-4） |

附带（轻量，可随本包顺手裁，不阻塞设计）：V1.0 稿 Q2（专业 Role 执行方式，代码事实近 A）、稿 Q4（管理界面完整度，决定 UI 批次范围）。见 §六。

---

## 二、D-1：project_role 实体口径

### 现状

- **无 Role 实体**。现行承载是两层混合：
  - `core.project_member.role` 六值：真人 `owner/member` + Agent 职能 `ai_pm/architect/developer/tester`（0030:7-9，注释明言后四值是「任务承接定位，不是访问等级」，project_member.rs:44-62）；
  - `ai.agent_definition` 三种子（architect/developer/tester，0020:14-24），仅作身份/归因层。
- V1.0 稿 §3.2 建议 13 角色 Catalog + §4 六组字段（Identity/Responsibilities/Capabilities/Contract/Governance/Execution）+ §4.2 不可变 Role Version。
- 任务上的 `assignee_id` 由导入链 `assignee_role` 经 `resolve_assignee` 解析（executor.rs:601-618），解析不到记 `assignee_unresolved` finding。

### 选项

**A. 新建 project_role 实体 + 不可变 Role Version，与 member.role 语义分离（推荐）**
- 新实体：Role 模板（平台预置、role_key 唯一）→ 项目启用行（enabled/启停）→ Role Version（内容快照，含稿 §4 六组字段的裁剪版）。
- Task 创建时记录 `role_version_id`（稿 §17.2 不变量③，AC-08 依赖）。
- `member.role` 四 Agent 职能值**保留为承接定位**、不强行同批退役：resolve_assignee 改为「Role → 具备该 Role 的执行者」解析，旧四值语义逐步由 Role 接管，避免大爆炸迁移。
- Catalog 首批种子只播与现有域对齐的角色（project_manager / architect / developer / tester + requirement_analyst），稿的 13 角色作为目录声明分批启用（稿 §3.3 的 Enabled/Disabled/Not Configured/Unavailable 四态机制）。
- 自定义 Role 按 V1.0 稿 Q3=A 口径：V1 不开放。

**B. 不建新实体，扩展现有结构**
- 沿用 member.role 四值 + agent_definition 加列扩字段。
- 最小迁移量，但：无版本化（AC-08 永远缺）、职责/权限/契约三语义继续挤在一列、后续调度能力匹配（D-2）没有干净锚点。

### 推荐：A

理由：①专家已定性为硬前置，语义分离（Team Role≠Project Role、身份≠职责）绕不开；②Role Version 快照可直接复用现行 `policy_snapshot`+`policy_snapshot_hash` 内容寻址先例（比对报告 §四）；③D-2 调度匹配、按 Role 模型策略（比对报告 P2-12）都以 Role 实体为锚。

### 影响面 / 解锁

- 新迁移（建议 0056~0057：role 模板/项目启用/版本三表 + task 加 role_version_id）。
- bootstrap 种子、resolve_assignee 改造、`tenant_agent.is_enabled` 死列一并处置（0020:32）。
- **解锁**：D-2 的能力匹配锚点、AC-08、稿 Q3 关闭。

---

## 三、D-2：调度入口设计（V2.0-E ③④）

### 现状

- 调度唯一落点 `run_dispatch`：候选=queued/租约过期 → claimability 七门（retry_exhausted / policy_requires_approval / task_waiting / no_executor_assignee / no_repository_scope / repository_not_verified / baseline_unavailable，policy.rs:136-231）→ SKIP LOCKED 领取 → 不满足则退避回队。
- **assignee 为人工指定**；七门中无「可用性过滤」③与「按 Role/Capability 自动匹配」④（比对报告 §二）。
- 留痕现状：`release_reason`（deny 码）+ `policy_snapshot` JSON（含 policy_version/hash）+ 每次领取/跳过/重领审计（lease.rs:246-330）；**候选全量快照无**。
- 状态表达已按对齐核验 §五归并表裁过：SCHEDULING/ASSIGNED 归并进 Run 态，不再重复裁决。

### 选项

**A. 自动调度写 assignee + claimability 扩门（推荐）**
- TaskReadyHandler 前插调度决策：③可用性过滤（Client 凭据有效性/readiness/租约负载）④Role-Capability 匹配（依赖 D-1 实体），决策产出=写 `task.assignee_id`（或等价调度目标），后续照走现有七门与领取通道。
- 稿 §18.2 可解释字段并入现有留痕：`policy_snapshot` 增 `required_capabilities / rejected_candidates(原因) / selection_reason`，**不新建调度决策大表**。
- 无合适执行者时：`task_waiting.reason` 新增枚举值 `no_executor_available`（表与谓词已建，补生产写入口即可），**严禁做成状态**。

**B. 新增独立调度决策记录表**
- 调度决策与 assignee 分离，独立表存每次决策全量（候选/落选/策略版本）。
- 可解释性最强，但与现有 policy_snapshot+审计双轨重复，首版成本高。

### 推荐：A

理由：复用七门语义与现有留痕面，符合「最小新机制」纪律；候选全量快照降为 P1 后补（比对报告 §七 P1-7）。

### 影响面 / 解锁

- run_dispatch/policy.rs 扩门 + deny 码新增；task_waiting 生产写入口；TaskReadyHandler 改造。
- **解锁**：AC-04/05、V2.0-E 收口；依赖 D-1 的 Role 锚点（故 D-1 应先裁）。

---

## 四、D-3：project_ai_model_policy 的 UNIQUE(tenant_id,provider_id)

### 现状

- `ai.project_ai_model_policy`（0043:13-25）：PK(tenant_id, project_id) + `UNIQUE(tenant_id, provider_id)`——**一个 Provider 只能被一个项目指定**。
- 与 V2 §九「Team 授权范围内多项目共享 Provider」直觉相悖（对齐核验 §七.4 原文点名）。
- 解析语义：项目指定后严格不回退（team_llm.rs:1167-1184）；未指定=继承 Team 首个 enabled。

### 选项

**A. 放开为多项目共享（推荐）**
- 迁移重建唯一约束：去掉 UNIQUE(tenant_id,provider_id)（PK 已保证每项目一行）。
- PUT 端点校验同步放开（team_llm.rs:990-1125）。

**B. 保留专属限制**
- 一个 Provider 专属一个项目。须把该限制写进 V2 文档作为有意设计，并接受「多项目必须各自建 Provider」的运营成本。

### 推荐：A

理由：Team Provider 本就是团队共享资源（0041 模型）；项目级 policy 的语义是「项目选择用哪个」，不应反向垄断资源；保留限制无安全收益（凭据访问已有租户门禁）。

### 影响面 / 解锁

- 一个小迁移（索引重建）+ 端点校验。
- **解锁**：Schema 设计定稿；后续「按 Role 粒度 Model Policy」（比对报告 P2-12）以本裁决为前置。

---

## 五、D-4：Work Session / 对话口径

### 现状

- **后端零承载**：无对话端点、无表（routes.rs 全清单无 chat/conversation）。
- 前端资产：ai-interaction 三件套（AiConversationPanel/Message/Composer + CONTRACT V1.0）为纯展示件，fixtures 驱动，未接任何业务页面。
- **两种口径并存**：
  - V0.5 裁决口径：结构化交互（task_interaction）+ Discussion Memory，**非自由 Copilot**；
  - V1.0 稿 §8：完整 Work Session 实体（6 态 + Conversation/WorkSession/Task 三层模型 + task_waiting 类状态 WAITING_FOR_EXECUTION——该状态化形态已被 §12.2 裁决否决，须改形态）。
- AC-03/09（比对报告 §六）压在此处 + Task 17 主链大脑。

### 选项

**A. V0.5 口径先行：结构化交互 + Discussion Memory，不建 Work Session 实体（推荐）**
- 建**最小对话存储**（conversation + message 两表，Platform 内部承载 AI PM 对话记忆），交互仍走 task_interaction 结构化链。
- 稿 §8 全章挂账为后续版本（须改形态后引入：WAITING_FOR_* 并入 reason 表）。
- Task 17 接线后的首个消费方就是这个最小存储 + 三件套接线。

**B. 按稿建 Work Session 实体**
- ai_work_session + ai_work_session_task + 6 态状态机 + Conversation 层。
- 组织模型完整，但在主链大脑（Task 17）未接、AC-02 都不成立时先建组织层，顺序倒挂；且稿 §8.3 状态化须返工。

**C. 分层折中**
- 先建 Conversation 存储（同 A 的两表），Work Session 实体与 Task 关联表随 V2 第二批。
- 实质=A 的执行序换了一种说法。

### 推荐：A

理由：①AC-03/09 的产品价值来自「能对话的 AI PM」，瓶颈是大脑+存储，不是组织模型；②V0.5 口径是已裁基线，推翻它需要显式裁决而非默认扩张；③Work Session 与 V0.5 口径的冲突（比对报告 §八「待用户裁决」）本包给出台阶——先 A 后按需升级到 C，稿 §8 不报废。

### 影响面 / 解锁

- 新迁移（conversation/message 两表）+ 对话端点 + 前端三件套接线；依赖 Task 17。
- **解锁**：AC-02/03 的落地路径、稿 Q2 的承载面（AI PM 直接处理问答）。

---

## 六、附带裁决（轻量，不阻塞设计）

| 项 | 内容 | 代码事实 | 建议 |
| --- | --- | --- | --- |
| V1.0 稿 Q2 | 专业 Role 是否须独立 Agent 执行 | 现状=同一模型链多职能 + 外部 client 按 task 动态领取；B（每 Role 独立实例）与三禁区冲突 | 按稿推荐 A 裁掉，即「职责独立、执行资源共享」 |
| V1.0 稿 Q4 | 管理界面完整度 | /project/settings/ai 占位页；五子页全缺；Execution Monitor 数据面未裁（建事件表 vs 消费 audit/outbox） | A/B 均为纯增量；若裁 A，Monitor 数据面裁决须排进 UI 批次前 |

> 编号警示：此处 Q2/Q4 是 V1.0 评审稿 §14 的编号，≠ V2.0 基线已裁的 Q1/Q2/Q3。

---

## 七、工程前置与验证欠债（非裁决，排期项）

1. **Task 17：主链 FakeModel→真模型接线**（**不依赖任何裁决，可立即开工**）：openai_adapter 已写好零生产引用（openai_adapter.rs:46-115），接线点 ai-pm-worker/main.rs:59-61；顺带 `llm_call_record` 全量接入（现仅连接测试落记录）。产出后 D-4 的最小对话存储即有消费方。
2. Checkpoint-2 R3/R4 真机联调（待定时刻）。
3. Checkpoint-1 余 4 红 baseline 测试欠债。

---

## 八、裁决后的启动序

```text
今天        ── 裁 D-1~D-4（+可选 Q2/Q4）
并行        ── Task 17 接线（不依赖裁决）
D-1 落地后  ── 0056+ 迁移：Role 三表 + task.role_version_id（V2.0-A）
D-2 落地后  ── run_dispatch 扩门③④ + task_waiting 写入口（V2.0-E）
D-3 落地后  ── 0056 批内顺带重建 policy 唯一约束
D-4 落地后  ── conversation/message 两表 + 端点 + 三件套接线
其后        ── V2.0-D Capability Registry → C 余项（binding 授权升级）
             → G 裁决（Monitor 数据面）→ AI Team 管理 UI（Q4 口径）
```

## 附：一次性回复模板

```text
D-1=A（或 B）
D-2=A（或 B）
D-3=A（或 B）
D-4=A（或 B/C）
附：稿 Q2=A；稿 Q4=A（可选，不裁则默认挂账）
```
