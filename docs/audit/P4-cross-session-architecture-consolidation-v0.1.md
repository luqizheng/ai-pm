# Phase 4（P4）跨 Session 架构讨论整合记录 V0.1

-   项目：smark-pm / PM-AI
-   整理日期：2026-11-19
-   文档性质：跨 Session 讨论整合与下一 Session 交接基线
-   状态：**讨论整合稿，非 Phase 4 总体冻结文件**
-   用途：上传至资料库，作为新 Session 继续 P4 架构讨论的上下文入口

------------------------------------------------------------------------

## 0. 本文使用规则

本文整合 P4 已有阶段性设计、冻结决定和近期跨 Session
的新讨论。必须区分：

  -----------------------------------------------------------------------------------------------------------------------
  标记                    含义                                 使用规则
  ----------------------- ------------------------------------ ----------------------------------------------------------
  **FROZEN / 已冻结**     已经正式人工裁决并通过相应冻结评审   作为约束继承，不得在普通讨论中重新解释或覆盖

  **CONFIRMED /           用户在近期跨 Session 讨论中明确同意  作为当前讨论基线；尚未经过正式 DR
  本轮已确认**                                                 编号、冲突审查和整体冻结，不得称为 P4 正式冻结

  **PROPOSED / 设计建议** 既有设计文档中的建议或助手建议       不因写入本文而生效

  **OPEN / 待讨论**       尚未由用户裁决，或缺少必要证据       新 Session 继续讨论

  **IMPACT REVIEW /       新确认方向可能改变既有冻结契约       先做显式影响分析；未经用户批准，不得静默修改既有冻结裁决
  影响审查**                                                   
  -----------------------------------------------------------------------------------------------------------------------

本文不授权代码、Migration、数据库或部署实施。P4-01、P4-02
文档冻结也不代表已经实施。

------------------------------------------------------------------------

# 第一部分：P4 已有冻结基线（必须继承）

## 1. P4-01 --- Product 领域模型

**状态：FROZEN V1.0（2026-11-17，架构评审 PASS）**

冻结锚点：`002c825c`。DR-01～DR-06 已逐项 ACCEPTED；DR-07 仅有
PROPOSED-DRAFT 骨架，留给 P4-03 独立裁决。

核心冻结事实：

1.  Product 归属 Team（领域语义），物理承载为 `tenant_id`。
2.  V1 不变量：Product 不能比 Team 存活更久；不存在跨 Team
    产品转移。Team dissolve 时 Product 进入 `dissolved`
    系统终态，不可恢复。
3.  Product 主体为 `core.product`，UUID PK，组合键遵循 tenant
    隔离惯例；`product_key` 租户内唯一，创建后不可变。
4.  **Product 主体身份与 Product Cognition
    认知对象身份严格分离**。`core.product.id`
    是主体锚点；`pc.cognition_object.id` 是认知对象身份。P4-03
    只能决定引用/归属方式，不能合并身份。
5.  Product 生命周期：`active / archived`，archived
    可恢复、无硬删；archived
    可读、禁止修改和新增关联，既有关联不自动解除。
6.  Product、Project、Association、PC Version
    生命周期严格分离；禁止生命周期自动传播。
7.  创建 Product：tenant owner/admin；变更、归档、恢复、移交：Product
    Owner ∪ tenant admin。
8.  Product 列表最小字段集可由全租户成员查看；详情仅 Product Owner ∪
    tenant admin 可见，其余 403。
9.  不设 `product_member`；Product 权限与 Project
    权限双向隔离，不自动穿透。
10. DR-07 留给 P4-03：若认知上移触及已冻结 PC 模型 B01～B10，必须单独
    DR + 架构评审，不能顺手改动。

## 2. P4-02 --- Product--Project 关系

**状态：FROZEN V1.0（2026-11-17，最终架构冻结评审 PASS，12/12
项核查通过）**

冻结锚点：`0a0e0e12`。DR-01～DR-06 已逐项 ACCEPTED。

核心冻结事实：

1.  Product 与 Project 为 N:M；V1 不设 `is_primary`；不关联 Product
    的独立 Project 合法。
2.  关系采用独立聚合实体 `core.product_project_link`。
3.  Link 状态为 `active | ended`；ended
    是终态，历史行永久保留；重建关系必须创建新 id，不复用旧 id。
4.  Link
    写操作由单一执行者完成，必须满足双侧授权交集：`(Product Owner ∪ tenant admin) ∧ (Project Owner ∪ tenant admin)`。
5.  Link 建立与结束分别留审计事件；结束不使用物理 DELETE。
6.  Team dissolve 时同事务将全部 active Link 置 ended；Product archived
    不自动结束 Link。
7.  P4-03 可以使用 Link 作为 Product 与 Project
    之间的关系底座，但不得回改 P4-02 冻结条文。

## 3. P3-11 / P3-D04 等上游约束

P4 必须继承 P3-11 双域权威边界：

-   Product Cognition（PC）与 Project
    Cognition（ProjC）不共享身份、状态机、确认权威、正文权威。
-   跨领域引用使用 DomainRef 契约；引用不等于授权。
-   正式认知确认必须由人类完成；AI
    可以分析、建议、起草，但不能自行确认正式认知。
-   跨领域访问原有冻结原则要求调用方授权、目标领域授权、目标对象当前状态约束形成闭环。
-   冲突对象归主要冲突认知所属领域；没有 Global Conflict 域。
-   跨项目聚合应由检索/聚合层完成，不共享正式认知权威。
-   Legacy Transitional 必须有界，不引入双写。

**特别注意：** 本轮新确认的"Agent 跨领域自动读取、无需逐次授权"与 P3-11
原有跨领域授权闭环存在潜在冲突。本文只登记为待影响审查方向，不宣称已经修改
P3-11 冻结规则。详见第六部分。

## 4. P4-03 V1.1 既有设计基线（尚非冻结裁决）

资料库现有文件：`01-P4-03-V1.1-Full-Design.md`，其自身状态为 Draft，所有
Recommendation 均为 PROPOSED，正式架构应由阶段 3 DR
逐项裁决及冻结评审决定。

该设计推荐方案 C，但**推荐不等于用户已经正式裁决**：

-   PC-P：Product 级正式产品认知权威，锚定 Product。
-   PC-J：Project 级产品域认知，承载项目特有对象、派生对象和局部差异。
-   ProjC：Project 技术认知，保持独立。
-   产品级正式认知与项目级上下文认知是不同内容、不同身份，不构成同一认知身份双重权威。
-   项目消费 PC-P 采用版本钉扎；产品发布新版本后，项目获得
    update-pending 感知，必须显式再基线或显式忽略并留痕，不能自动覆盖。
-   派生对象是项目自有的新身份，不会取得 PC-P 修改权。
-   项目实施反馈进入 PC-P 候选/提案流程，必须经过产品级人类确认。
-   生命周期按主体响应、关联响应、认知版本生命周期和正文传播分开处理。
-   与 P3-D04 的 DomainRef / resolver /
    引用语义必须做契约级兼容性核查；不得替 P3-D04 裁决。

P4-03 V1.1 原设计中的 DR-01～DR-09
均是待裁决输入，不能当成已经通过人工裁决的事实。特别是 DR-08
跨域消费授权和 DR-09 生命周期处置仍需按原设计的职责边界处理。

------------------------------------------------------------------------

# 第二部分：Product 与 Cognition 的近期讨论基线

## 5. Product、Product Cognition、Project Cognition 的关系

以下方向已在近期讨论中明确确认：

1.  Product 是独立的产品管理实体，不应被简化为 Project 的父级。
2.  每个 Product 拥有独立的 Product Cognition。
3.  Product Cognition 是产品正式认知权威；Project Cognition 是具体
    Project 的正式认知权威。两者保持独立。
4.  Product 与 Project 仍遵循 P4-02 已冻结的 N:M 关系；Product 不是
    Project 的所有者，也不因 Link 关系取得 Project 认知权威。
5.  Product Cognition 可以从 Project
    实践中发现通用产品价值，但不能把客户特定需求自动提升为产品正式认知。
6.  客户/项目特定要求首先归 Project Cognition。AI Product Lead
    可以分析其产品化价值，但不因此取得项目需求的正式记录权威。
7.  Product Plan 不属于 Product Cognition 正式认知对象；继承
    Q-PC-13，归产品规划/项目管理领域。
8.  Product Plan 可以引用 Product Cognition
    的正式版本作为规划依据，但不能成为 PC 的权威来源。

## 6. Product Plan

已确认方向：

1.  Product Plan 是独立于 Product Cognition 的正式产品规划对象。
2.  Product Plan 有正式版本历史。
3.  同一时刻只有一个正式生效的 Product Plan。
4.  可以存在多个并行 Draft / Alternative 方案，供 AI
    分析、比较和人类选择。
5.  AI Product Lead 是 Product Plan 的主要维护者，可以提出调整、起草
    Revision。
6.  Product Plan 的正式变更必须由人类批准；批准后形成独立
    Revision，记录变更内容、原因、提出者、批准者和依据。
7.  Product Lead 可以根据 Project 进展、依赖、风险、产品化候选等提出
    Plan 调整，但不得越权修改 Project 的正式内容或状态。
8.  Product Plan 从规划角度统筹产品目标、优先级、阶段目标、Project
    组合和建设顺序；不取代 Project 自身生命周期。

## 7. Project Proposal 与 Productization Candidate

### 7.1 Project Proposal

已确认：

-   Project Proposal 与 Product Plan 是两个独立对象。
-   Proposal 用于论证一个新 Project
    是否值得建立，描述目标、范围、产品价值、依赖、验收等。
-   AI Product Lead 可以提出和撰写 Project Proposal。
-   Proposal 获批后可以进入 Product Plan，但不等于
    Plan，也不意味着立即启动 Project。
-   正式创建 Project 必须经过人类批准。

### 7.2 Productization Candidate（产品化候选）

已确认：

1.  Productization Candidate 是独立机制。
2.  它不是 Product Cognition 的正式认知对象，也不是某个 Project 的需求。
3.  来源不限于 Project 实践：可以由 AI 从项目实践中发现，可以由 AI
    主动发现，也可以由人类产品负责人直接与 Product AI Agent 沟通提出。
4.  AI 负责分析、整理、关联证据、评估价值和跟踪。
5.  候选可以被否决、暂缓、合并或持续观察。
6.  只有经过人类确认后，相关内容才可能进入正式 Product Cognition 或
    Product Plan。
7.  项目实践证据应保留来源与关联，不得因候选创建而改变原 Project
    Cognition 的权威归属。

## 8. Product Lifecycle Event、定期检查与警告

已确认：

1.  Project 生命周期独立于 Product Plan。Plan 中移除 Project
    不等于删除/归档 Project；Project 被取消也不自动修改 PC。
2.  Project 被移除或发生重大生命周期变化时，应发出事件通知 AI Product
    Lead。
3.  事件通知的目的是让 AI Product Lead
    自行分析影响，而不是让系统按固定规则自动修改 Plan。
4.  AI Product Lead 应分析 Product Plan、产品能力、其他
    Project、依赖关系和产品风险，提出调整方案。
5.  AI Product Lead 可以识别受影响的其他 Project，但不能直接修改其他
    Project 的正式内容或状态；具体项目由对应 AI Project Lead
    和人类负责人处理。
6.  除事件触发外，还应支持人类设置定期检查周期，例如每天晚上。
7.  定期检查覆盖 Product Plan 与各 Project
    实际进度一致性、依赖、延期、停滞、产品能力缺口、风险、产品化候选和规划偏差等。
8.  检查发现的问题生成警告并通知人类。
9.  警告是持续管理对象，保留状态、证据、影响范围和处理历史；不能因为通知已发送或下一次检查未复现就自动消失，必须按明确规则关闭。
10. 采用"定期检查 + 重大事件即时通知"。重大 Project
    生命周期变化、关键依赖断裂、关键阻塞、重大范围或目标变化等，不等待下次定期检查。

## 9. AI Product Lead 的工作记忆与 Product Management 界面

已确认：

1.  AI Product Lead 必须拥有持久化长期工作记忆，不能只依赖对话上下文。
2.  工作记忆与正式 Product Cognition
    分离。工作记忆包括历史检查发现、未解决警告、被人类否决的建议、持续观察的产品方向、影响
    Plan 的历史决策和待跟进事项。
3.  工作记忆不能绕过人工确认变成正式 Product Cognition。
4.  人类可以查看、纠正和管理工作记忆，包括查看跟踪事项和警告历史、停止跟踪某方向、要求重新检查、修正
    AI 对历史决策的理解。
5.  纠正工作记忆不等于修改正式 PC 或 Plan。
6.  Product Management 有独立工作界面，作为人类产品负责人管理
    Product、监督 AI 的主要入口。
7.  界面应覆盖 PC、Plan/备选方案、Project Proposal、Productization
    Candidates、Project
    Portfolio、定期检查报告、即时警告、工作记忆和待处理事项。
8.  人类可以用自然语言向 AI Product Lead
    下达管理指令，但自然语言不能绕过权限、Human Review 和正式确认流程。
9.  AI Product Lead
    可以主动向人类发起讨论，提供发现、证据、分析和建议；讨论本身不等于正式决策。
10. 人类需要参与的事项统一进入系统级 Human Review 模块，不由每个 Agent
    建立独立审批中心。

## 10. Human Review 与 Agent 后续行动闭环

已确认：

1.  Human Review 生命周期由系统统一管理，业务 Agent 不自行维护审批状态。
2.  Product、Project 或其他 Agent
    需要人类参与的审核、讨论、正式确认等统一进入 Human Review。
3.  审核完成后，系统自动将批准、拒绝或要求修改的结果及必要原因反馈给原发起
    Agent。
4.  人类审核可以附加意见或修改要求，不限于 approve/reject。
5.  Agent
    必须明确确认收到审核结果，并说明下一步如何处理。收到结果不等于行动完成。
6.  系统跟踪 Agent
    是否确认接收；超过设定时间未确认，产生持续警告并通知人类。
7.  Agent 确认后，系统继续跟踪实际后续行动；超时未完成则再次警告。
8.  Agent
    无法按承诺完成时，必须主动报告阻塞原因、影响、已尝试措施和新的预计完成时间；不能等到超时才报警，也不能编造不可靠
    ETA。
9.  人类可以接受新期限、提供帮助、调整要求或取消任务。
10. 行动完成后不一律再次人工审核：

-   普通分析/整理：提交结果记录即可；
-   涉及正式 PC、Plan、Project 状态等权威变更：走对应领域正式确认；
-   人类明确要求复核：重新进入 Human Review。

11. 人类可以随时中止 Agent
    后续行动。中止保留已完成工作，记录原因、发起人、时间和停止结果；未完成工作标记中止/待处理，并检查业务状态。
12. 中止执行不等于撤销已经完成的正式变更；例如已经批准生效的 Plan
    Revision 不自动回滚。
13. 人工恢复时，Agent
    先读取中止前执行记录，检查已完成/未完成工作和当前业务状态，重新评估原计划有效性，向人类说明恢复计划和风险；超出原审批范围的变更重新进入
    Human Review。
14. 恢复的是工作目标和上下文，不是盲目恢复旧执行状态。

------------------------------------------------------------------------

# 第三部分：统一 Agent 基础设施与身份

## 11. Agent 基础设施

已确认 Product AI Lead、Project AI Lead 和专项 Agent 使用统一 Agent
基础设施。统一的是执行和治理基础设施，不是领域认知权威。

基础设施范围：

-   Agent Runtime；
-   持久化工作记忆；
-   任务跟踪；
-   Human Review 接口；
-   审核结果反馈；
-   超时与持续警告；
-   中止与恢复；
-   审计记录；
-   Token 与费用预算控制。

Product Cognition、Project Cognition
等领域的正式权威、业务职责和写入权限仍然独立。

## 12. Agent Identity 与 Lead 的稳定性

已确认：

1.  每个 Agent
    有独立系统身份，用于追踪工作记忆、执行记录、权限范围、审核提交和审计行为。
2.  AI Product Lead 是绑定到 Product 的持久化逻辑
    Agent，不是某个模型、固定进程、容器或聊天会话。
3.  每个 Product 绑定固定的 AI Product Lead
    逻辑身份；用户明确否决"可以更换 AI Product
    Lead"的假设。不得把模型切换、配置升级或 Runtime 实例变化解释为更换
    Lead。
4.  AI Project Lead 采用相同的逻辑架构，职责和权限不同。
5.  Agent
    Identity、领域绑定、核心职责和持久化记忆长期稳定；模型、配置版本、Skills、Tools
    和 Runtime 执行实例可以按受控方式变化。
6.  Agent 不需要一直运行。它按人类请求、定时任务或事件触发执行；完成后
    Runtime 可以释放资源，下次通过持久化身份、配置和记忆恢复上下文。
7.  底层模型只是推理引擎，模型升级或切换不改变 Agent Identity。

## 13. Agent 核心定义与配置升级控制

用户已明确选择 A：

-   系统统一管理 Agent 基础定义、核心职责、系统级行为约束和权限边界。
-   人类可以在受控范围内调整模型、扩展
    Skills、配置工具和允许开放的运行参数。
-   人类不能通过普通配置随意改写 AI Product Lead 的核心职责。
-   系统可通过受控版本升级演进 Agent 核心定义。
-   升级 Agent 不等于更换 Agent；原有 Identity、Product
    绑定、工作记忆和审计历史保持连续。

## 14. Agent 委派与专项 Agent 创建

用户已确认：

1.  AI Product Lead 可以按任务需要创建或调用专项
    Agent，例如需求分析、架构分析、风险分析等。
2.  专项 Agent 不成为 Product 的 Lead，不拥有 Product Cognition
    正式写入权威。
3.  专项 Agent 仅在被委派任务范围内执行，返回结果、依据和发现。
4.  AI Product Lead 整合结果、作出领域内判断并承担后续产品管理责任。
5.  不局限于系统预先定义的专项 Agent 类型；Agent
    可以根据任务需要创建新的专项 Agent。
6.  允许递归委派：专项 Agent 可以继续调用或创建下级专项 Agent。
7.  下级 Agent 的权限不得超过其上级当前任务获得的授权。
8.  系统记录 Agent 身份、创建来源、委派关系、任务范围、执行记录和结果。
9.  结果沿委派链逐级返回原始委派 Agent。
10. 委派深度、数量、运行时间和资源消耗必须受系统约束，避免无限扩张。
11. 创建专项 Agent 不等于创建新的 Product AI Lead 或 Project AI Lead。

------------------------------------------------------------------------

# 第四部分：跨领域协作与 Agent 预算控制

## 15. 跨领域协作的近期确认方向

用户明确确认：

-   Agent 之间允许自动跨领域协作。
-   Agent 可以自动读取其他领域的正式认知对象。
-   读取、搜索、分析、引用不需要每次向人类申请授权。
-   **允许读，不允许越权写。**
-   Agent 不因跨领域读取获得目标领域正式认知的修改权或确认权。
-   例如 Project Agent 可以读取 Product Cognition
    的正式产品规则；Product Agent 可以读取 Project Cognition
    的项目实践，但不能直接修改项目正式认知。
-   Product Agent 发现项目实践具有产品化价值时，应通过 Productization
    Candidate / 正式提案链处理，不得直接写入正式 PC。
-   专项 Agent
    只能在其任务范围内读取和分析，不因创建或委派自动获得正式写入权。

### 15.1 必须进行的影响审查

这项新方向与 P3-11 已冻结的跨领域权限闭环（调用方授权 + 目标领域授权 +
目标对象状态约束）存在潜在冲突，也与 P4-03 V1.1 §2.3 / §7.8
中"引用不等于授权、Project 成员不自动获得 PC-P
读取权、消费授权需显式建立"的推荐设计存在张力。

因此，当前准确状态是：

-   **用户已确认的产品方向**：Agent
    在系统定义的协作范围内可以自动跨领域读取，无需逐次人工授权；跨领域写入仍禁止越权。
-   **尚未完成的架构裁决**：如何把该方向与 P3-11、DomainRef、P4-03
    DR-08、Product 详情可见性和租户隔离闭合。
-   不得把本轮确认解释成"所有 Agent 可读取所有租户的所有数据"。
-   不得把自动读取解释成跨领域写权限。
-   不得直接修改 P3-11
    冻结规则；必须先提交显式影响分析，列出需要保留、细化或申请变更的条文。
-   新 Session 必须优先处理这一冲突，形成明确的访问策略边界和兼容方案。

## 16. Token 与费用预算：总体原则

用户已确认 Agent
调查和委派必须使用统一预算治理机制，避免递归委派、并行执行或自动调查造成无限
Token 消耗。

### 16.1 单次调查共享总预算

1.  一次完整调查任务建立一个总预算。
2.  原始 Agent、全部下级
    Agent、递归委派、跨领域读取和子任务模型调用均计入同一个调查预算。
3.  子任务可以分配子预算，但所有子预算之和受总预算约束。
4.  不允许通过新建 Agent、递归委派、拆分子任务或切换模型绕过总预算。
5.  达到预算阈值/上限时，系统暂停继续调查和新增委派。
6.  Agent 向人类提交已完成成果、已消耗
    Token、未解决问题、预计追加需求和继续调查价值。
7.  人类批准追加预算后，任务才可以继续。
8.  Agent 可以申请追加，但不能自行批准。

### 16.2 同时限制总量与消耗速率

用户选择 B：

-   **Total Budget**：单次完整调查 Token 总量上限。
-   **Rate Limit**：单位时间内 Token
    消耗速率上限，防止短时间并行调用导致成本突增。
-   **Sub-budget**：可为子任务分配预算，但子预算共享总预算。
-   并行 Agent 数量也纳入系统治理，防止并发爆发。
-   预算超限时暂停相关新增调用/委派，并按规则申请人类追加预算。

### 16.3 人类预算控制权

用户确认人类可以调整预算，并可以设置 Agent 长期预算策略：

1.  系统为任务类型提供默认预算、速率上限等默认策略。
2.  人类可以调整单次调查预算。
3.  人类可以为 Agent 设置长期预算上限。
4.  长期策略至少包括：
    -   每日 Token 消耗上限；
    -   单次调查 Token 上限；
    -   并行 Agent 数量上限；
    -   哪些任务可自动执行；
    -   哪些任务达到阈值后必须申请追加预算；
    -   Token 消耗速率限制。
5.  Agent 可以提交预算追加申请，但不能自行批准。
6.  系统统一执行限制，禁止通过递归委派或创建新 Agent 绕过策略。

### 16.4 Token 与真实费用并行治理

用户已确认同时记录和控制：

-   Token 数量：用于控制模型调用规模；
-   实际费用：用于控制真实成本；
-   模型及调用明细：用于审计、归因和成本分析。

**尚未确定的数值与细节：**

-   默认单次调查 Token 上限；
-   每日 Token 上限；
-   Rate Limit 的时间窗口与阈值；
-   并行 Agent 数量默认上限；
-   达到多少 Token 或费用时先预警、多少时强制暂停；
-   追加预算的默认申请粒度；
-   费用预算按模型/供应商/租户/Agent/任务如何分摊；
-   Token 计量口径（输入、输出、缓存命中、工具调用等）及费用核算口径。

这些数值不要由 Agent
或设计文档自行编造，应在后续预算策略设计中由人类确认。

------------------------------------------------------------------------

# 第五部分：当前已确认决策清单

下表是近期新增的用户确认项，作为下一 Session
的讨论基线；它们尚未全部转写为正式 DR，也未完成与 P3-11/P4-03
的兼容审查。

  --------------------------------------------------------------------------------------------------------------------------------
  ID                      已确认方向                                                                       状态
  ----------------------- -------------------------------------------------------------------------------- -----------------------
  N-01                    Product 拥有独立 Product Cognition；Project Cognition 仍是项目正式认知权威       CONFIRMED

  N-02                    Product Plan 不属于 PC；独立版本化，同一时刻一个正式生效版本，可有               CONFIRMED
                          Draft/Alternative                                                                

  N-03                    Project Proposal 与 Product Plan 独立；Proposal 获批后可进入 Plan，Project       CONFIRMED
                          创建仍需人类批准                                                                 

  N-04                    Productization Candidate 独立于正式 PC 和 Project                                CONFIRMED
                          需求；来源包括项目实践、人类提出、AI 主动发现                                    

  N-05                    AI Product Lead 是 Product 持续产品负责人；与 AI Project Lead 平级协作           CONFIRMED

  N-06                    Product Lead 负责维护 Plan，正式变更由人类批准并形成 Revision                    CONFIRMED

  N-07                    Project 生命周期事件通知 Product Lead，由其分析影响并提出调整，不自动改 Plan     CONFIRMED
                          或其他 Project                                                                   

  N-08                    定期检查 + 重大事件即时通知；警告持续保留，按明确规则关闭                        CONFIRMED

  N-09                    Product Lead 有持久化工作记忆，与正式 PC 分离；人类可查看、纠正和管理            CONFIRMED

  N-10                    Product Management 独立界面；自然语言管理指令不能绕过正式流程                    CONFIRMED

  N-11                    所有人类审核事项进入统一 Human Review；系统管理生命周期并反馈结果给 Agent        CONFIRMED

  N-12                    Agent                                                                            CONFIRMED
                          接收审核结果后需确认接收、说明行动、跟踪完成；支持主动报告阻塞、中止和人工恢复   

  N-13                    Product/Project/专项 Agent 使用统一 Agent                                        CONFIRMED
                          Runtime、记忆、任务、审核、警告、中止恢复和审计基础设施                          

  N-14                    Agent 有独立系统身份；Product AI Lead 绑定固定逻辑身份，不允许替换               CONFIRMED

  N-15                    系统管理 Lead 核心定义和职责；人类可受控配置模型、Skills、Tools                  CONFIRMED

  N-16                    Agent 可创建/调用专项 Agent；允许递归委派和创建新                                CONFIRMED
                          Agent；系统限制深度、数量、时间和资源                                            

  N-17                    Agent 可自动跨领域读取正式认知，不需要逐次人工授权；不允许越权写入               CONFIRMED；IMPACT
                                                                                                           REVIEW

  N-18                    一次完整调查的全部委派链共享总 Token 预算                                        CONFIRMED

  N-19                    同时限制 Token 总量和单位时间消耗速率                                            CONFIRMED

  N-20                    人类可调整单次预算，并设置 Agent 长期预算策略                                    CONFIRMED

  N-21                    Token 数量和实际费用并行限制/记录，保留模型及调用明细                            CONFIRMED
  --------------------------------------------------------------------------------------------------------------------------------

------------------------------------------------------------------------

# 第六部分：待影响审查与未决问题

## 17. P3-11 跨域权限闭环与自动读取的冲突（最高优先级）

必须在新 Session 首先处理。

需要明确：

1.  自动读取是 Agent Runtime 的系统级服务权限，还是 Agent
    身份自身的固有权限？
2.  是否所有正式认知对象都可自动读，还是只限 Product Cognition / Project
    Cognition 等 Agent 协作所需领域？
3.  自动读取是否受 Team/tenant 隔离约束？应明确不能跨 tenant 越界。
4.  是否仍需检查目标对象当前状态（例如
    archived、dissolved、withdrawn、非正式版本等）？
5.  人类用户的 Product/Project 详情可见性与 Agent
    后台读取权限是否分成两个权限面？
6.  DomainRef 是否仍只承担引用/定位，自动读取由 Agent Runtime
    的系统级读取策略负责？
7.  对 P4-03 DR-08
    的消费授权设计，是保留显式对象级授权、改为系统策略授权，还是采用分层模型？
8.  如何审计 Agent
    读取了什么、为哪个任务读取、读取了哪些版本、依据何种系统策略？
9.  如何防止某个 Agent 创建大量专项 Agent 后扩大读取面？
10. 是否需要支持对特定领域、对象类型或任务类别禁止自动读取？

**硬约束：** 自动读取不等于自动写入；PC、ProjC
的正式权威、状态机和人类确认门必须保持独立。任何对 P3-11
冻结条文的变更都要显式列出条文编号、变更理由、影响面和迁移/兼容方案，等待人类裁决。

## 18. Agent 身份与专项 Agent 生命周期

已确认逻辑方向，但仍需设计：

-   Lead Agent Identity 与专项 Agent Identity 的标识和关系；
-   临时专项 Agent 与可复用专项 Agent 的区别；
-   新创建专项 Agent 的配置模板、职责范围和默认权限；
-   专项 Agent 任务结束后是归档、休眠还是保留可复用；
-   专项 Agent 的工作记忆保留策略；
-   Agent 核心配置版本与运行实例版本的记录方式；
-   系统升级 Agent 核心定义时，旧任务如何继续、是否需要兼容版本；
-   创建者、委派者、执行者和最终责任 Agent 的审计关系。

这些属于设计细化，不应重新讨论"Lead 是否可替换"这一已明确否决事项。

## 19. Human Review 与预算审批之间的关系

需要在后续明确：

-   追加 Token/费用预算是否进入统一 Human Review；
-   预算申请是否需要展示完整调查状态、阶段成果、估算依据和剩余问题；
-   预算申请批准后如何向 Agent 回传；
-   追加预算是否只增加当前调查预算，还是可以临时覆盖长期 Agent 策略；
-   人类能否为某次任务临时降低长期策略上限；
-   预算审批拒绝后，Agent 如何整理已有成果并结束/挂起任务。

## 20. Agent 预算策略细化

需要人类确定数值前，先由设计稿定义预算模型和核算语义：

-   Budget Scope：tenant / Agent / task / investigation / delegation
    tree；
-   Total Token Budget 与 Rate Limit 的计量口径；
-   费用预算与 Token 预算之间的关系；
-   多模型、多供应商成本统一折算；
-   并行 Agent 的总量和速率记账；
-   工具调用产生的模型消耗是否统一归属原始调查；
-   预警阈值、暂停阈值、硬上限、追加预算和超时行为；
-   系统默认策略与人类长期策略的优先级；
-   防止子任务、重试、缓存、模型切换或新建 Agent 绕过预算的机制。

## 21. P4-03 原设计 DR 的后续处理

P4-03 V1.1 原设计包含 DR-01～DR-09。近期新增的 Product Lead、Product
Plan、候选、Agent 体系和自动跨域读取可能改变若干 DR 的输入条件。

后续应：

1.  保留 P4-03 V1.1 原稿作为历史设计输入，不覆盖原文件。
2.  先完成近期新方向与 DR-01～DR-09 的映射。
3.  识别哪些 DR 仍可沿用、哪些需要补充、哪些需要重新提出裁决。
4.  对 DR-08 跨域权限重点做 P3-11 影响审查。
5.  对 Product Plan 继续遵守 Q-PC-13：Plan 不进入 PC 正式认知域。
6.  不得因为 Product Lead 工作记忆、候选池或 Agent
    任务记录而引入新的正式认知权威。
7.  正式认知变更仍必须进入对应领域的人类确认链。
8.  P4-01/P4-02 冻结决定不因 P4-03 新设计而自动重开。

## 22. P4 后续工作顺序建议（仅建议，待用户确认）

1.  **P4 跨 Session 基线复核**：确认本文与资料库现有冻结文件、P4-03 V1.1
    的完整性。
2.  **跨域自动读取影响审查**：先解决 N-17 与 P3-11 / P4-03 DR-08
    的契约冲突。
3.  **Agent 领域模型与身份架构**：定义 Lead、专项
    Agent、Identity、Configuration、Memory、Runtime、Delegation。
4.  **Agent 任务与 Human Review
    闭环**：定义任务状态、审核状态、结果反馈、确认接收、行动跟踪、中止和恢复。
5.  **Token/费用预算模型**：定义预算层级、总量、速率、费用、并行度、追加审批和策略优先级。
6.  **Product Management 领域模型**：整合
    PC、Plan、Proposal、Productization
    Candidate、Portfolio、警告与工作记忆。
7.  **事件、定期检查与主动讨论机制**：定义触发、调度、警告生命周期和审计。
8.  **回到 P4-03 DR-01～DR-09**：在上游依赖和新输入清楚后，逐项裁决。
9.  **P4-04/P4-05 等后续切片**：依照 P4
    总体规划及实际依赖重新排定，不预先假设顺序。

此顺序是助手建议，不是用户已经批准的正式 Phase 4 排期。

------------------------------------------------------------------------

# 第七部分：新 Session 交接说明

## 23. 新 Session 应如何使用本文

新 Session 开始时：

1.  将本文作为跨 Session 讨论整合基线。
2.  同时读取资料库原始文件，尤其：
    -   `P4-00_Current_Architecture_Ownership_Audit_V1.0.md`
    -   `P4-03-00-current-cognition-architecture-audit-v1.1.md`
    -   `01-P4-03-V1.1-Full-Design.md`
    -   `09-P4-01-P4-02-Frozen-Decisions.md`
    -   P4-01 / P4-02 原始冻结设计和 DR 文件（若已上传资料库）。
3.  不得把本文中 CONFIRMED 的新方向自动标成 FROZEN；应先做一致性审查、DR
    编号和正式裁决归档。
4.  不得重开 P4-01 DR-01～DR-06、P4-02 DR-01～DR-06、P3-11
    冻结条文，除非先提出明确变更理由并获得用户授权。
5.  不得把 P4-03 V1.1 中的 PROPOSED 推荐当成用户裁决。
6.  对冲突先列证据、冻结条文、变更影响和备选兼容方案，再请用户一次确认一个核心问题。
7.  讨论仍遵循"人类负责产品/项目，AI Agent 协助执行"；AI
    提议不等于人类批准。
8.  当前优先讨论问题是：**如何在不破坏 P3-11
    双域权威、租户隔离和正式写入权的前提下，实现 Agent
    跨领域自动读取。**

## 24. 资料来源与溯源

本次整合所依据的资料库文件与历史讨论包括：

-   `P4-00_Current_Architecture_Ownership_Audit_V1.0.md`：P4
    总体现状、差距、P4-04/P4-05 依赖与早期建议。
-   `P4-03-00-current-cognition-architecture-audit-v1.1.md`：P4-03 阶段
    1 现状审计、P3-11/P4-01/P4-02 硬约束。
-   `01-P4-03-V1.1-Full-Design.md`：P4-03 阶段 2 完整设计建议，所有 DR
    仍需人工裁决。
-   `09-P4-01-P4-02-Frozen-Decisions.md`：P4-01/P4-02
    冻结决定及继承核对。
-   2026-11-18 至 2026-11-19 跨 Session Product / Product Cognition / AI
    Product Lead / Agent 基础设施讨论。
-   2026-11-19 关于专项 Agent 递归委派、跨领域自动读取、共享 Token
    预算、速率限制、人类长期预算策略、Token 与实际费用双重治理的确认。

**本文完成。**
