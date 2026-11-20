# PA-07：Task Execution Orchestration 与跨 Product 协作架构基线

**决策编号：** PA-07  
**文档版本：** V1.0（正式架构基线）  
**日期：** 2026-11-20  
**范围：** PA-07 新执行架构、Workflow、Task/Run、Task Dependency、Task Decision Review、Human Consultation、Product 生命周期、跨 Product 协作、执行权限与恢复  
**状态说明：** 正式架构基线。已确认决策与助手依据用户明确授权形成的技术裁决分别标注；实施细节以配套 Developer Implementation Specification 为准。

## 1. 总体设计约束

smark-pm 面向小型、紧密协作的软件研发团队，公司内部使用，并发量不高。架构必须优先简单、直接、可理解、可维护。

- 不照搬大型企业组织模型、复杂审批链或分布式协调体系。
- 不为理论上的极端规模提前建立独立子系统。
- AI-PM 应在授权范围内自主组织日常研发工作。
- 人类保留业务目标、正式需求、重大业务取舍和关键方向的决定权。
- 每增加一个实体、状态、流程或服务，都必须有当前规模下的实际收益。

## 2. 新执行架构的核心边界

### 2.1 不建立平台自有 Agent Work Unit

平台不建立自己的 Agent Work Unit 身份、组织与生命周期体系。AI-PM 直接组织正式 Task Execution，通过结构化提示词、项目上下文、执行流程、授权约束和交付契约驱动外部 Harness 完成工作。

- Harness 自主负责内部 Agent 身份、子 Agent 协作、能力组合、执行与内部恢复。
- 平台管理 Task、Run、Provider、Session 及正式执行结果。
- 平台不重复实现成熟 Harness 已具备的内部执行机制。
- 旧 Agent Work Unit 设计保留为历史讨论，不再自动视为新架构有效基线。

### 2.2 AI-PM 与 Harness 职责

AI-PM 自己负责：
- 需求理解、需求分析与澄清；
- 需求拆解、任务规划和依赖关系分析；
- 风险识别、项目进度和状态协调；
- 项目级技术规划、架构一致性判断；
- 项目知识整理及跨 Task 协调。

深入技术调查、详细技术方案、代码级分析，以及代码开发、测试、文档、演示文稿和部署等实际工程工作，可委派外部 Harness。

### 2.3 Workflow 与 Orchestrator

- 平台维护标准 Task Execution Workflow 模板。
- AI-PM 根据任务动态选择、组合和调整 Workflow；可生成任务专用 Workflow，但不得擅自删除适用的强制质量门禁。
- Workflow 模板支持版本管理，避免模板更新改变正在执行任务的要求。
- Orchestrator 按确定性条件推进阶段和门禁；只在语义判断、流程调整、异常决策或重大风险处理时唤起 AI-PM。
- Runner / Provider Adapter 负责 Harness 启动、执行管理和结果回报。
- Harness 负责实际工程工作及其内部 Agent 协作。
- Workflow Execution 记录 Task 的流程进度，不取代 Task、Run 或 Provider Session；阶段不必一一对应 Task 或 Run。

### 2.4 失败恢复

采用分层恢复：
1. Orchestrator / Runner 自动处理确定性基础设施问题和安全可重试故障。
2. AI-PM 处理智能恢复决策，包括修复、阶段重跑、Workflow 调整、Provider/Harness 切换或新建 Task。
3. 业务目标变化、重大决策、权限不足或无法安全恢复时，进入人工介入。

Workflow 阶段失败不自动等于正式 Task 失败。

## 3. Task、Run 与交付验证

### 3.1 Task Revision

- Task 保留稳定身份和 ID。
- 目标、验收条件及执行契约的变更通过新的 Task Revision 记录。
- 每个 Run 固定引用启动时使用的 Task Revision。
- 新 Revision 若改变当前 Run 的目标或验收条件，AI-PM 评估暂停/终止旧 Run，并按新 Revision 创建新 Run；仅补充不影响当前执行的信息时，可继续旧 Run。

### 3.2 Harness 内部验证与跨 Task 集成验证

- 单个 Task 内部的工程验证、单元测试、修复和重测由 Harness 自主完成。
- 跨 Task 集成验证是独立、预先规划的正式 Task，由 AI-PM 组织。
- 集成验证 Task 与开发 Task 共同构成跨 Task Workflow。
- 集成验证通过后，AI-PM 判断整体功能切片是否满足交付要求。
- 不把所有工作强行拆成 Workflow Stage。

### 3.3 Task Decision Review

Harness 执行期间若有人类参与，并共同作出导致原 Task 目标、需求、验收条件或重要技术方案变化的决定：
- Harness 可在当前上下文中继续完成调整后的工程工作。
- Task 可按实际情况完成或废弃。
- 任务结束时 Harness 必须向 AI-PM 提交决策报告。
- AI-PM 独立分析项目级影响，并按需创建一个或多个并行审查 Task。
- AI-PM 不介入 Harness 与人的日常工程对话；并非每个实现细节变化都触发审查。
- 涉及正式需求、Product 目标或重大业务取舍时，最终决定权仍属于人类。

审查范围按实际影响确定；AI-PM 必须独立识别潜在跨项目影响，不能只依赖 Harness 报告。

## 4. 重大变更暂停与恢复

### 4.1 Product 执行状态

Product 有独立研发执行状态：
- `Normal`
- `Suspended`
- `Resuming`
- `Archived`（归档，不再接收常规业务活动，保留历史）

重大变更需要暂停时，由 AI-PM 发起，Orchestrator 执行暂停指令。

Product 进入 `Suspended` 后，其关联 Projects 中受该 Product 影响的开发任务不得继续派发或执行。受控调查任务及 Human Consultation Task 可以继续。Task 不必批量改成 Suspended；保留各自真实生命周期状态，暂停原因、范围和恢复条件独立记录。

### 4.2 Human Consultation 与 Task Portfolio Review

- 可能涉及正式需求、Product 目标、架构基线或重大业务取舍的问题，进入项目级决策暂停流程。
- 暂停范围覆盖受影响 Product 及其关联 Projects；其他不受影响的 Product/Project 可以继续。
- 允许只为查明事实的调查，不得借调查继续实施有争议的方案。
- Human Consultation 完成并更新正式基线后，恢复前必须执行 Product 级 Task Portfolio Review。
- Review 覆盖在途、已派发、执行中、暂停、已完成任务及交付物、未派发任务，以及原计划但尚未创建的未来任务。
- Review 逐项检查目标、范围、验收条件、技术方案、依赖、交付物有效性、优先级和状态。
- 处理方式包括保留、修订（新 Task Revision）、重新执行/修复、废弃或由新 Task 替代。
- 完成任务组合一致性审查后，Product 进入 `Resuming`；AI-PM 按新任务组合逐步恢复，完成后回到 `Normal`。

## 5. 共享 Project 与多个 Product

### 5.1 Product 暂停门禁

已确认规则：
- Project 的开发执行权限取决于其关联 Product 的状态及具体任务影响范围。
- 任一关联 Product `Suspended`，Project 中受其影响的开发活动必须暂停。
- 同时服务多个 Product 的 Task，必须等待所有相关 Product 均允许执行。
- 与暂停 Product 完全无关、且授权和资源边界明确的工作可以继续。
- AI-PM 判断任务归属和影响范围；Orchestrator 执行门禁。

### 5.2 暂停期间的新任务

已确认规则：
- AI-PM 可以继续分析需求、拆解任务、制定计划。
- 可以创建正式 Task，但必须明确 Product 归属和影响范围。
- 与暂停 Product 无关且边界明确的 Task 可以正常派发。
- 涉及暂停 Product 或影响范围不明的 Task 不得派发。
- 不得通过新建 Task 绕过暂停门禁。

### 5.3 跨 Product 依赖

已确认规则：
- AI-PM 必须识别 Task 对暂停 Product 的代码、接口、架构和交付物依赖。
- 已完成必要验证且不受待决策事项影响的稳定依赖，可以继续使用。
- 可能被 Human Consultation 决策改变的依赖，不允许相关 Task 进入依赖该内容的执行阶段。
- 可明确切分的独立部分可以继续，但不得提前实施有争议部分。

### 5.4 暂停范围扩展

已确认规则：
- 调查发现暂停 Product 的变更可能影响其他 Product 时，AI-PM 必须独立评估。
- 影响局部且边界明确时，只限制相关 Task 或执行阶段。
- 若涉及其他 Product 的正式需求、业务目标、架构基线或重大业务取舍，应将该 Product 纳入决策暂停范围。
- 扩大暂停必须有明确影响依据和审计记录，不因理论关联而冻结整个 Product。

### 5.5 决策通知与 Product 自主权

已确认规则：
- Product A 的 Human Consultation 决策影响 Product B 时，必须正式通知 Product B。
- 通知内容是已正式确认的最终决定，包括变更内容、影响范围及生效信息，而非讨论意见或候选方案。
- Product B 自己的人类成员决定涉及自身正式需求、业务目标和重大业务取舍的事项。
- AI-PM 负责影响分析、建议和研发计划调整，不得代替 Product B 作出业务决策。
- Product B 未回应不能视为同意；不受影响的工作可继续，依赖未决事项的工作保持受限。
- Product B 明确反对时，记录正式决策异议；相关工作保持受限，由相关 Product 人类成员协商。不得由 AI-PM 擅自仲裁业务取舍。
- 双方无法达成一致时，冲突范围内工作可以长期暂停，无关工作继续。
- 协商解决后，各 Product 分别确认自身业务基线；恢复限制按各自 Product 状态和必要验证分别解除，不要求同时恢复。

### 5.6 决策记录与 AI-PM 责任

已确认规则：
- 各 Product 分别保存涉及自身业务基线的正式决策。
- 建立跨 Product 决策关联，连接双方决策及共同协商过程。
- 共享 Project 保存共同技术方案、接口约定和架构影响记录。
- Task Revision、Workflow 和集成验证结果可追溯到相关 Product 决策。
- 一个 Product 的决策记录不能代替另一个 Product 的正式决策。
- 一个 Project 只有一个 AI-PM 负责统一研发协调；AI-PM 必须分别遵守各 Product 的正式决策和授权边界。
- AI-PM 不能在 Product 业务目标冲突时选择某一方目标优先。

### 5.7 跨 Product 资源与执行优先级

已确认规则：
- AI-PM 可分析任务价值、紧急程度、依赖、风险和资源需求，并提出排期建议。
- 不改变业务承诺的执行顺序调整，可由 AI-PM 在授权范围内自主完成。
- 若调整会改变正式交付承诺、业务目标或重大资源取舍，必须由相关 Product 人类成员决定。
- 各 Product 保留自己的业务优先级；AI-PM 负责 Project 内实际执行排序。
- 不建立统一跨 Product 业务价值评分、优先级委员会或复杂排期审批系统。
- AI-PM 记录必要的排期依据，继续安排不冲突的工作。

## 6. 跨 Product Task 的交付、撤回与产品分化

本节包含用户明确确认的基础约束，以及助手依据用户整体授权形成的 V1.0 裁决；两类来源分别标注。

### 6.1 已确认的基础约束

- 一个正式 Task 可以关联多个 Product，但不因此复制多份 Task 生命周期。
- Task 的目标、验收条件和执行契约必须明确覆盖其关联 Product 的要求。
- Task 服务某 Product（业务受益归属）与 Task 依赖某 Product 的代码/接口/交付物是不同关系，不得混为一谈。
- Task Revision、Run 固定 Revision 的规则继续适用。

### 6.2 部分交付（V1.0 架构裁决）

**状态：纳入正式基线；依据用户对 PA-07 后续工作的整体授权，由助手完成技术与业务边界收敛。**

建议验收条件标明共同部分及各 Product 专属部分；允许某 Product 的独立交付条件先行通过，但只要仍有必须完成的条件未满足，整体 Task 不标记 Completed。共同能力缺陷应触发所有受影响 Product 的重新评估。

不新增 ProductTask 生命周期或子 Task 状态机。若工作可独立开发和验收，优先拆成不同 Task。

### 6.3 部分验收失败后的修复（V1.0 架构裁决）

**状态：纳入正式基线；依据用户授权形成。**

不默认重跑整个 Task 或 Run。AI-PM 分析失败影响范围，局部修复并验证；若共同代码或共享交付物受影响，则重新评估其他 Product 已通过的验收。原 Task 已完成后发现缺陷，建议创建关联修复 Task，保留原交付历史。

### 6.4 需求撤回（V1.0 架构裁决）

**状态：纳入正式基线；依据用户授权形成。**

一个 Product 撤回需求，不自动取消整个多 Product Task。AI-PM 重新评估剩余 Product 的目标和授权：
- 仅服务撤回方的工作停止派发；
- 共同工作需确认是否仍满足其他 Product 的需求；
- 剩余有效工作可通过新 Task Revision 保留；
- 无有效业务目标时废弃 Task，但保留历史；
- 已产生代码和交付物不自动丢弃，需评估复用或清理；
- 正在执行的 Run 按目标是否失效决定取消或继续；实质变更按 Task Revision 规则处理。

### 6.5 Product 归档或删除（V1.0 架构裁决）

**状态：纳入正式基线；依据用户授权形成。**

归档时不再接收常规新需求，AI-PM 检查未完成任务，由 Product 决定收尾、转移业务归属或废弃。完成交付及历史决策保留。

删除不得级联删除 Project、Task、Run、代码或决策历史；删除前须处理业务归属和未完成工作。V1 不建设复杂删除恢复子系统，常规操作优先采用归档。

### 6.6 产品分化原则（用户提出的方向，推荐进一步采纳）

**状态：纳入 V1.0 架构基线；分化判断由 AI-PM 根据业务目标、需求基线、路线图、验收标准和决策责任的实质分化提出建议，正式业务归属变化仍由相关人类决定。**

当原有 Product 的业务目标、需求基线、路线图、验收标准和决策责任逐渐分化时，不应不断扩大跨 Product Task 的复杂度来维持原边界。应考虑将其划分为独立 Product，例如 Product-for-A 与 Product-for-B。

- Product 是独立业务目标和产品演进责任的边界，不只是功能集合或任务分组。
- 产品分化不意味着必须拆分 Project、Repository 或共享技术组件。
- 不同 Product 可以共享 Project、代码仓库、架构和通用能力，同时分别维护需求、路线图、交付目标和业务决策。
- 普通配置差异、模块差异或客户定制，不足以自动触发新 Product。
- 只有形成独立业务目标、独立需求基线/演进计划和独立决策责任时，才考虑产品分化。
- 当前不引入 Product Family、Product Variant 等复杂模型；待真实业务需要时再扩展。
- 产品分化时，历史 Task 保留；未完成共同任务重新评估；差异化工作分别建立 Task；共享能力继续以共同研发任务或共享组件维护。

## 7. 简化架构结论

在当前团队规模下，优先复用：
- Product 及其执行状态；
- Project；
- Task 与 Product 关联；
- Task Revision；
- Task Dependency；
- Run；
- Workflow Execution；
- Human Consultation；
- Activity History / 审计记录。

不建议现在新增：
- 平台自有 Agent Work Unit 体系；
- 独立跨 Product 资源调度系统；
- 跨 Product 优先级委员会或统一业务价值评分引擎；
- ProductTask 生命周期；
- 多层复杂审批流；
- Product Family / Variant 等产品家族实体。

## 8. 架构收敛记录与决策来源

V1.0 已完成 PA-07 审查71–96。原 V0.3 中列为“待后续确认”的普通技术设计事项，已在用户整体授权下由助手完成裁决；涉及正式业务决策权的边界仍保留人类决定权。

- **用户明确确认的决策：** 例如 AI-PM 统一协调、Harness 内部 Agent 自主、Task Dependency V1 的 AND/DAG/有限条件、Product 暂停边界、跨 Product 业务决定权隔离等。
- **助手依据用户授权形成的 V1.0 裁决：** Task Decision Review 关闭条件、Human Consultation 最小状态、Product Resuming、Product 归档、跨 Product Task 部分验收、生命周期隔离、事务并发与异常恢复等。
- **必须由人类作出的事项：** 正式需求、Product 目标、重大业务取舍、超出既有授权的资源/交付承诺，以及跨 Product 各自业务基线的确认。

原 V0.3 第 8 节列出的七项待审查事项已分别由审查71–96覆盖，不再作为架构设计待办。工程实现映射、现有代码差距和测试工作转入配套《PA-07 Developer Implementation Specification》。

---

**维护纪律：** 已确认决策、用户提出但尚未完整裁决的方向、助手推荐待确认方案必须持续分开。后续版本不得将推荐方案静默升级为已确认基线。


## 9. Task Dependency 专题基线（审查71–72及配套规则）

**专题状态：** Task Dependency V1 设计已收敛。用户明确授权后续 Task Dependency 设计由助手按推荐方案主动推进；本节将已确认规则与按该授权采用的推荐设计一并作为当前架构基线。涉及重大业务决策权或正式授权边界时仍须人工决定。

### 9.1 职责边界与关系分类

Task Dependency 只表达正式 Task 之间的前置工作依赖，不承担所有业务关系或技术引用关系。

- **Task → Task：** 使用 Task Dependency 表达正式工作依赖，由 Orchestrator 用于派发门禁。
- **Task → 技术交付物：** API、Library、Repository Commit、Design Revision 等技术依赖，记录在适用的 Task Revision 执行上下文中，并固定必要的版本/交付物引用；不另建通用 Dependency Domain。
- **Product → Product：** 表达业务使用、服务或协作关系，不自动转化为 Task Dependency。
- **Task → Product：** 表达 Task 服务哪些 Product 的业务归属；与技术依赖分开建模。Product 状态门禁按 Task 的业务归属和实际影响范围检查。

共享 Repository 或共享 Product 使用关系本身，不足以推断某个 Task 必须等待另一个 Task。

### 9.2 依赖图与启动条件

已确认的 V1 规则：

- 一个 Task 可以有多个前置 Task，也可以有多个后继 Task。
- 多个前置 Task 采用 **AND 汇聚**：所有必要前置依赖均满足后，后继 Task 才允许启动。
- 同一组前置任务完成后，可以解锁多个互不依赖的后继 Task；后继任务可并行启动，仍须通过各自 Product 状态、权限、资源及其他调度门禁。
- V1 只支持 AND，不支持 OR（例如“前置 A 或前置 B 任一完成即可”）。不得通过隐式解释或调度器特例绕过这一限制。
- 依赖边可以声明必要的满足条件；不能简单地把所有依赖都理解为“前置 Task 进入 Completed 即可”。例如，后继工作可能要求前置 Task 完成特定验收，或交付指定成果。
- Orchestrator 只按已记录的依赖关系和满足条件进行确定性判断，不需要每次唤起 AI-PM。

示例：D 依赖 A、B、C；E 也依赖 A、B、C。只有 A、B、C 各自满足对应依赖条件后，D 与 E 才分别具备启动资格；D 与 E 之间没有依赖时，可以并行。

### 9.3 依赖环路

推荐采用有向无环依赖图（DAG）：

- 不允许 Task 依赖自身。
- 新建或修改依赖时，必须确定性检查是否形成直接或间接环路；若形成环路，拒绝该依赖变更并向 AI-PM 返回可理解的原因。
- 不需要单独的环路管理服务或人工审批流程；在依赖写入/更新的领域校验中完成检查即可。
- AI-PM 负责重新规划任务顺序或拆分工作，不能通过保留环路并期待运行时自行解决。

### 9.4 依赖满足与交付物固定

- Task Dependency 指向稳定的前置 Task 身份；依赖边还必须表达后继任务所要求的满足条件。
- 对于依赖某个具体成果的情况，应在后继 Task Revision 中记录该成果的明确引用及版本，而不是只依赖一个会随时间变化的“最新成果”指针。
- 前置 Task 满足条件后，后继 Task Revision/执行上下文应能追溯实际采用的前置 Revision 和交付物版本。
- 前置 Task 的状态为 Completed，不必然代表所有依赖条件都满足；Orchestrator 必须检查指定条件及其对应的有效交付/验收记录。
- 不允许以未验证、已撤回、已废弃或不再有效的交付物，默认为当前有效依赖成果。

### 9.5 前置 Task 失败、取消或废弃

核心原则：**前置 Task 异常终止，不自动级联取消后继 Task；后继 Task 保持未就绪，等待依赖恢复或调整。**

- Run 失败不等于 Task 失败。只要 Task 尚未正式终止，后继 Task 继续等待，AI-PM 可安排恢复、修复、重试或更换 Harness。
- 前置 Task 正式失败、取消或废弃，且未满足依赖条件时，后继 Task 不得派发。
- 不自动批量取消、失败或废弃后继 Task；保留其原有生命周期状态，由 Orchestrator 计算其依赖阻塞状态。
- AI-PM 判断采用恢复原 Task、重新执行、创建替代 Task、调整依赖/Revision，或在失去业务意义时废弃后继 Task。
- 若前置 Task 被替代，必须明确建立新的依赖关系，并确认新任务交付满足原后继任务所需条件；不得仅因替代任务存在就自动视为满足。
- 不为上述处理新增 Dependency Recovery 实体、独立状态机或级联工作流。

### 9.6 依赖变更与执行中的 Task

- **后继 Task 尚未启动：** AI-PM 可调整依赖；若变更影响目标、范围、验收条件或执行契约，创建新的 Task Revision，并重新检查依赖及派发条件。
- **后继 Task 已启动：** 不得追溯性地改写该 Run 已固定的 Task Revision、执行上下文或已采用的交付物版本。
- 若依赖变化不影响当前 Run 的目标、验收和安全性，当前 Run 可按原 Revision 继续；后续 Run 使用更新后的 Revision。
- 若依赖变化使当前 Run 的目标、验收条件、关键技术前提或安全性失效，AI-PM 应评估暂停/终止当前 Run，并按 Task Revision 规则重新组织执行。
- 依赖修改必须保留必要的原因、影响范围和历史记录；不得静默解除阻塞以促使任务启动。

### 9.7 调度职责与禁止事项

- **AI-PM：** 规划依赖、识别跨 Task/跨 Product 影响、决定恢复或调整方案；在业务目标或正式授权边界发生变化时提交人工决定。
- **Orchestrator：** 检查依赖图、AND 汇聚条件、依赖满足记录及其他确定性门禁；只在全部必要条件满足时允许派发。
- **Runner / Harness：** 不负责决定平台级 Task Dependency，也不能通过自行继续执行绕过 Orchestrator 门禁。
- 不增加 OR 表达式引擎、通用依赖类型注册表、全局依赖调度服务、复杂依赖优先级算法或人工逐条放行机制。

### 9.8 Task Dependency V1 决策摘要

| 项目 | V1 基线 |
|---|---|
| 一个 Task 的前置/后继数量 | 多个 |
| 多前置汇聚 | AND，全部必要依赖满足 |
| 多后继启动 | 可并行，分别检查各自门禁 |
| OR 依赖 | 不支持 |
| 依赖满足条件 | 可指定，按确定性规则检查 |
| 依赖环路 | 禁止，写入时校验 |
| 前置 Run 失败 | 不等同 Task 失败，不自动影响后继生命周期 |
| 前置 Task 失败/取消/废弃 | 后继不派发，不自动级联取消 |
| 替代前置 Task | 显式调整依赖并验证条件 |
| 技术交付物依赖 | 固定在 Task Revision 执行上下文，不另建 Dependency Domain |
| 依赖变更 | 保留历史；按是否影响当前 Run 决定继续或重组执行 |

---

**历史维护说明：** 上述段落保留 V0.3 时期的 Task Dependency 专题记录。V1.0 已通过审查84–96完成全局架构收敛；V0.3 中尚未收敛的普通技术方案已按用户授权形成 V1.0 裁决，重大业务决定权仍属于相关人类。


### 9.9 Task Dependency 环路校验与并发一致性（审查73）

Task Dependency 必须始终构成有向无环图（DAG）。

- 禁止 Task 自依赖，也禁止任何长度的间接循环。
- 新建或修改依赖边时，确定性检查从目标节点是否能够沿现有依赖关系到达源节点；若可以，则本次变更会形成环路，必须拒绝。
- 错误信息应尽可能提供具体循环路径，便于 AI-PM 修复规划。
- 不自动删除其他依赖边，不由 Orchestrator 在运行时尝试“化解”环路。
- 环路检查与依赖边写入必须在同一数据库事务和适当的并发保护下完成，避免并发请求分别通过检查、组合后形成环路。
- 关系数据库及普通图遍历即可满足 V1 需求，不引入图数据库或独立图计算服务。

### 9.10 依赖变化对各执行阶段的处理（审查74）

依赖关系变化立即影响未来调度，但不得追溯性改写已经发生的执行事实。

| 后继 Task 所处阶段 | 处理规则 |
|---|---|
| 尚未派发 | 重新计算依赖条件；不满足时阻止派发 |
| 已派发、Run 尚未启动 | 启动前重新校验；不满足时撤回派发并等待 AI-PM 处理 |
| Run 正在执行 | 不因依赖变化自动终止；通知 AI-PM 判断继续、暂停或终止 |
| Run 已完成、Task 尚未完成 | 重新评估交付物和验收条件；必要时阻止 Task 完成 |
| Task 已完成 | 保留历史结果；若新依赖影响其有效性，创建后续验证或修复 Task，不重开或篡改历史 |

进一步约束：

- Run 启动时固定其 Task Revision，并记录当次实际采用的依赖条件、前置 Task/Revision、交付物版本及条件检查结果。
- 当前依赖图用于未来调度；Run 快照用于解释当时为何允许执行。
- 依赖变更若实质改变 Task 目标、验收条件或执行契约，必须创建新 Task Revision；不能只修改依赖边规避 Revision 规则。
- 若依赖变化不影响当前 Run 的目标、验收和安全性，AI-PM 可以允许其按原 Revision 继续；若使当前执行依据失效，则按既有 Run/Revision 规则暂停或终止并重新组织执行。
- 不增加 Dependency Revision 实体。依赖边当前状态由依赖关系表表达，历史变化复用审计记录，Run 当时的执行依据由启动快照保留。

### 9.11 依赖条件的最小表达（审查75）

Task 是否进入 `Completed`，与后继任务所需的交付结果是否可用，不应被视为完全相同的判断。

V1 依赖边采用一个明确、有限的满足条件，不支持任意表达式、条件脚本或自定义布尔逻辑：

- `completed`：前置 Task 已正式完成。
- `accepted`：前置 Task 的相关交付结果已通过要求的验收。
- `deliverable_approved`：指定交付物已达到 Approved 状态。

其中 `deliverable_approved` 必须引用明确的交付物及其版本。若任务只需前置工作完成，不必强制引用交付物；若后继工作依赖具体成果，则必须固定该成果引用。

多个前置 Task 仍按 AND 汇聚，每条依赖分别检查自身条件。Orchestrator 只读取平台正式状态、验收记录和交付物状态，不自行进行语义验收；语义判断由 AI-PM 或相应验证流程负责。

### 9.12 已完成前置任务的交付物失效或替换（审查76）

前置 Task 曾经完成，不代表其交付物永远有效。

- 依赖具体成果时，后继 Task Revision 固定交付物版本；不得隐式跟随“最新版本”。
- 交付物被正式撤销或确认无效后，尚未启动的后继 Task 重新计算依赖条件；条件不满足则阻止派发。
- 已派发但 Run 尚未启动的后继 Task，在启动前重新校验；不满足时撤回派发。
- 正在执行的后继 Task 不自动终止，由 AI-PM 判断现有执行是否仍有效。
- 已完成的后继 Task 保留历史；如受影响，创建验证或修复 Task。
- 新版本出现不自动使旧版本失效。是否替换依赖引用，由 AI-PM 根据兼容性、验收要求和任务目标判断；实质影响目标或验收时创建新 Task Revision。

依赖检查必须基于明确版本和有效性记录，不允许将已撤销、已废弃或已确认无效的成果视为满足条件。

### 9.13 依赖变更的责任、提议与审计（审查77）

- AI-PM 负责正式创建、调整和解除 Task Dependency。
- Harness 和专业 Agent 可以报告缺失依赖、技术约束及证据，并提出修改建议，但不能直接修改正式依赖图。
- Orchestrator 只校验和执行门禁，不决定业务依赖是否应该存在。
- 人类可以提出调整要求；若变化涉及正式需求、验收条件或业务承诺，仍遵循对应的人类确认边界。
- 每次正式依赖变更应记录变更前后关系、原因、操作者和时间，复用现有审计机制，不新增依赖审批流程。

### 9.14 执行中发现新依赖（审查78）

Harness 在执行中发现新的前置工作时，应先报告 AI-PM，不得自行修改正式依赖图或绕过 Orchestrator 继续执行。

AI-PM 判断其性质：

- 若确实需要当前项目完成一项正式工作，则创建或识别对应 Task，建立 Task Dependency，并按审查74重新评估已派发或正在执行的后继 Task。
- 若只是 API 版本、Commit、Design Revision 等技术引用，且不需要另一个正式 Task 先完成，则补充到 Task Revision 执行上下文，不创建 Task Dependency。
- 若涉及外部授权或正式业务边界，按既有人工决策规则处理。

### 9.15 Product 门禁与 Task Dependency 保持独立（审查79）

Task Dependency 与 Product 执行状态是两套不同约束：

- Task Dependency 表达正式工作之间的先后关系。
- Product 状态表达某项业务范围当前是否允许开发。
- Product `Suspended` 不得伪装成未完成的前置 Task。
- Product 恢复也不意味着 Task Dependency 自动满足。

Orchestrator 派发前分别检查依赖条件、Product 状态及其他确定性门禁。所有必要条件满足后，Task 才具备派发资格。

### 9.16 依赖就绪状态和调度触发（审查80）

`blocked_by_dependency` 是调度计算状态，不是 Task 生命周期状态，不应写入或替代 Task 的正式生命周期状态。

V1 在以下事件发生时重新计算相关 Task 的依赖就绪情况：

- 前置 Task 状态变化；
- 前置交付物验收、批准、撤销或失效；
- Task Dependency 创建、修改或解除；
- Product 执行状态变化；
- Orchestrator 准备派发 Task。

派发前必须再次执行确定性校验，不能只依赖之前计算的就绪结果。服务重启后，对尚未派发的候选 Task 重新计算即可；不引入复杂周期调度服务。

### 9.17 Task Dependency V1 最小数据模型与历史快照（审查81）

V1 使用一张正式依赖关系表表达有向边，建议字段语义如下：

| 字段语义 | 用途 |
|---|---|
| `predecessor_task_id` | 前置 Task |
| `successor_task_id` | 后继 Task |
| `required_condition` | 该依赖边的满足条件 |
| `required_deliverable_ref` | 可选；明确指定交付物及版本 |
| `created_at` / `created_by` | 创建审计信息 |
| `updated_at` / `updated_by` | 最近修改审计信息 |

具体物理字段名可服从现有数据库命名规范，不要求照搬上述建议名。

约束：

- 同一方向的同一对 Task 不重复建立相同依赖。
- 自依赖及间接循环必须在同一事务内校验和拒绝。
- 依赖边的创建、修改和解除复用现有审计机制。
- Run 启动时保存依赖检查快照，至少能追溯前置 Task、适用 Revision、条件、交付物版本及检查结果。
- Run 快照是历史执行证据，不是第二套依赖权威数据；当前依赖关系表仍是未来调度的权威来源。
- 不建立 Dependency Group、Dependency Revision、Dependency Recovery 或独立依赖历史实体。

### 9.18 解除依赖与绕过门禁（审查82）

解除依赖表示 AI-PM 判断原有依赖已经不再成立、已由其他有效前置工作/交付物替代，或原计划需要正式修正；它不是临时忽略门禁。

- AI-PM 负责正式解除依赖并记录原因。
- 不提供 Harness 或普通执行流程可直接使用的 `force_start`、`ignore_dependency` 开关。
- 如果解除依赖改变 Task 的目标、验收条件或执行契约，必须按 Task Revision 规则处理。
- 依赖解除后重新计算后继 Task 的调度资格，仍须检查 Product 状态、权限、资源等其他门禁。
- 不以“紧急”为由保留一个隐式绕过路径；确需调整执行顺序时，正式调整依赖关系并保留审计记录。

### 9.19 外部工作与既有交付物作为前置条件（审查83）

并非所有前置条件都必须通过当前 Project 的 Task 表达：

- **当前 Project 需要完成的正式工作：** 建立前置 Task，使用 Task Dependency。
- **已经存在且可验证的成果：** 在后继 Task Revision 执行上下文中引用明确版本及验证依据，不创建没有实际工作的占位 Task。
- **由外部团队或系统负责的工作：** 记录外部交付物引用、责任来源和验证条件，不虚构由当前 Project 控制的前置 Task。
- **外部交付尚未满足：** AI-PM 保持后继 Task 不派发，直到外部条件被验证满足。

此类外部/既有成果引用属于执行上下文和技术依赖，不自动成为 Task Dependency 边。

### 9.20 Task Dependency V1 最终边界

Task Dependency V1 的正式能力为：多前置、多后继、AND 汇聚、并行解锁、有限且明确的依赖满足条件、DAG 环路禁止、确定性调度门禁、依赖变更审计、Run 历史快照，以及 AI-PM 负责的异常恢复和依赖调整。

明确不支持：OR 依赖、任意条件表达式、脚本化依赖、自动推断正式依赖、隐式跟随最新交付物、强制绕过依赖、独立依赖审批/恢复子系统、图数据库和独立调度引擎。

**专题状态说明：** 用户已明确授权助手对 Task Dependency 的普通技术设计主动裁决。审查71的关系分类和跨 Product 依赖边界、多个前置/后继与 AND 汇聚、审查72的失败处理、审查73的 DAG 与事务校验，以及审查74–83的具体规则，均作为 Task Dependency V1 架构基线；这不等于将 PA-07 其他领域的待确认推荐方案升级为用户确认决策。


## 10. Task Decision Review 与 Human Consultation（审查84–85）

### 10.1 Task Decision Review 关闭条件

Task Decision Review 只处理执行期间出现的实质性变化，不对 Harness 的每个工程细节进行审批。触发范围包括 Task 目标/范围、正式需求、验收条件、重要技术方案或可能影响其他 Task/Product/Project/交付物的决定。

Review 可以关闭的条件是：
1. Harness 已提交决策报告，包含变化、原因、证据和实际实现结果；
2. AI-PM 已独立分析影响范围，不仅复述 Harness 结论；
3. 每项重大影响均有明确处置：无影响、接受并记录、更新 Revision/技术基线、创建验证/修复/审查 Task，或转入 Human Consultation；
4. 不存在无人负责、无处理结论的重大影响。

Review 关闭不要求所有后续工作已完成；但所有后续事项必须有明确责任和状态。审查 Task 失败不等于原 Task 失败；证据不足时补充调查或重新审查。不得将审查失败解释为“没有问题”。

多个审查结论冲突时：技术事实冲突由 AI-PM 组织验证；授权范围内技术方案冲突由 AI-PM 裁决；不同 Product 的正式需求/目标/重大业务取舍冲突由相关 Product 人类分别决定，AI-PM 不仲裁业务价值。

Review 关闭记录应关联原 Task、决策报告、审查 Task、Revision 和相关交付物，记录接受项、基线更新、受影响工作、未决人类决定及后续任务。

### 10.2 Human Consultation

Human Consultation 是需要人类作出正式决定的工作项，不是通用审批系统或讨论聊天室。触发事项包括正式需求变化、Product 目标调整、重大业务取舍、跨 Product 业务冲突，以及超出 AI-PM 授权的重大资源/交付承诺。

最小状态：`pending`、`in_progress`、`decided`、`cancelled`。`decided` 必须包含决定内容、决定人、时间和适用范围；`cancelled` 仅表示事项不再需要处理，不代表默认同意原方案。既有决定被改变时追加新决定记录，不覆盖历史。

单 Product 事项由该 Product 有权人类决定；多 Product 事项由各 Product 分别确认涉及自身的正式基线。一个 Product 未回应不视为同意。冲突范围保持受限，无关工作继续；无法达成一致时允许长期保持冲突范围暂停，不建立跨 Product 委员会或投票机制。

AI-PM 可继续调查、准备方案和规划任务，但不得把候选方案写成正式基线，也不得在决策未完成时派发受影响开发工作。

## 11. Product Resuming 与再次暂停（审查86–87）

Product 从 `Suspended` 恢复必须经过 `Resuming`，采用“先重新评估，再逐步开放执行”，不批量修改 Task 状态。

进入 `Resuming` 后，AI-PM 对该 Product 执行有针对性的 Task Portfolio Review：核对正式决策/需求基线、在途 Task 与 Revision、Run 有效性、交付物版本和验收、Task Dependency、权限及资源门禁。无需重新执行所有历史任务，也不要求所有任务准备完毕才恢复。

Task 按真实生命周期保留状态：未派发任务重新检查；已派发未启动任务在启动前重检；运行中 Run 由 AI-PM 判断继续/暂停/终止；已完成 Task 不重开，需要补充工作则创建后续 Task；已取消/废弃 Task 不自动恢复。

多 Product Task 必须满足所有相关 Product 门禁。若 A 已恢复而 B 仍暂停，A 的独立工作可恢复，共同工作继续受限；不得通过修改 Product 关联规避门禁。

Product 进入 `Normal` 的条件：正式决策已同步、受影响 Task 已完成必要有效性评估、未决事项有明确责任和限制范围、门禁能够正确判断允许/禁止的工作，且不存在需要整体暂停的事项。不要求所有 Task 已完成或已启动。局部边界明确的问题可继续局部限制并让 Product 回到 `Normal`。

`Resuming` 期间再次发生变化：局部影响则局部限制；影响 Product 正式目标/核心需求/重大业务取舍则重新 `Suspended`。已运行 Run 不自动终止，由 AI-PM 判断。暂停事件分别记录，不覆盖历史。不新增 `ResumingFailed`、`PartiallyResumed` 等状态。

Product 状态恢复不会自动满足 Task Dependency；依赖仍按审查73–83规则重新计算。

## 12. Product 归档与研发资产生命周期（审查88）

V1 常规支持 `Archived`，不提供常规硬删除。归档前必须执行 Portfolio Review，检查未派发、已派发未启动、执行中、已完成未验收 Task、共享 Task、未决 Human Consultation、交付物及其他 Product 的依赖。

每项未完成工作必须明确处理为：完成收尾、取消、废弃、经有权人类决定转移业务责任，或保留为其他 Product 的共享工作。不得只归档 Product 而留下无责任人的在途工作。

归档不自动取消多 Product Task；不终止所有 Run；不删除 Repository、Commit、API、Design Revision、交付物、Task、Run、Workflow、决策或知识历史；不自动解除其他 Product 的依赖。共享资产按实际责任归属继续维护。

若消费者 Product 归档，共享基础 Product 不受影响；若共享基础 Product 归档，必须评估消费者替代、迁移和维护责任。Fork/扩展仍须由相关人类明确决定。

误创建且尚未产生正式关联记录的 Product，未来可通过受限纠错流程处理；这不构成 V1 常规 Delete 能力。

## 13. 跨 Product Task 的交付与验收完整性（审查89–90）

Task、交付物、Product 验收是三个不同层次：
- Task 生命周期表达正式工作是否完成；
- Deliverable 表达具体成果及其版本、来源和有效性；
- Product 验收表达某个 Product 的要求是否满足。

一个 Task 服务多个 Product 时，验收条件区分共同交付条件与各 Product 专属条件。各 Product 验收结果独立保存。A 验收通过不代表 B 通过；B 验收失败不自动否定 A 的独立结果。共同交付缺陷影响所有受影响 Product；专属功能缺陷原则上只影响对应 Product，除非有证据表明跨 Product 影响。

Task 仅在所有仍有效且必要的工作和验收条件满足后才进入 Completed。某 Product 撤回需求或归档后，其不再有效的专属范围经正式处理后不继续阻塞 Task；历史关联和验收记录保留。部分交付可以先行验收，但不能据此提前将整体 Task 标记 Completed。

部分验收失败时，AI-PM 判断局部修复、更新 Task Revision 或创建独立后续 Task。已经通过的验收结果保留；如果共同交付物后续被证实有缺陷，创建关联验证/修复工作，不篡改历史验收事实。

Task 已 Completed 后出现新需求或新缺陷，创建新的 Task；不重开或覆盖历史 Task。V1 不建立独立 Product Delivery 子系统，优先复用现有验收记录；若现有数据无法表达 Product 级验收，则在实施规范中采用最小的 Product 维度验收扩展。

## 14. 状态机与跨实体状态隔离（审查92）

各实体状态互相独立：Product 管理业务活动状态；Task 管正式工作生命周期；Run 管一次执行尝试；Workflow Execution 管流程推进；Human Consultation 管正式人类决策；Dependency/权限/资源/Product 门禁共同决定调度资格。

禁止无条件级联：Run Failed 不自动使 Task Failed；Run Succeeded 不自动使 Task Completed；Product Suspended 不批量改变 Task 状态；Dependency 不满足只产生调度阻塞，不产生 Task Failed；Workflow Failed 不自动终止全部 Task；Human Consultation Pending 只限制受影响范围。

终态保护：Task/Run/Workflow/Consultation 的历史终态不能被普通更新接口覆盖。需要继续工作时创建新 Run、新 Workflow Execution 或后续 Task；需要纠正历史事实时走受控纠错并保留审计。Run 固定启动时 Revision，重试创建新 Run。

**实现映射注意：** 以上是架构语义，不直接要求把每个描述性词汇新增为数据库枚举。当前代码基线中 Task 已有 13 态，Run 有 7 态（包括 `timed_out`）；实施必须先映射现有状态机，保留已存在且有实际语义的状态，不可照搬简化示例覆盖现有状态值。`blocked_by_dependency` 是计算所得调度状态，不是 Task 生命周期状态。

## 15. 权限与决策权（审查93）

- 人类决定正式需求、Product 目标、重大业务取舍及超出授权的资源/交付承诺。
- AI-PM 在授权范围内负责需求分析、Task 拆解、技术协调、Workflow 选择、排期、依赖管理和智能恢复；不代替 Product 人类决定业务方向。
- Orchestrator 执行确定性门禁、阶段推进和已授权状态转换，不创造业务授权。
- Runner / Provider Adapter 管理执行进程、会话、回报与基础设施恢复，不修改正式 Task 基线。
- Harness 在固定 Task Revision、授权、验收和预算边界内自主完成工程工作；可建议改变，不得直接改变正式需求、验收、Task Dependency 或授权。

Harness 与人类日常工程对话中发生实质性基线变化时，Harness 可在其当前上下文继续工作，但必须在任务结束提交决策报告，由 AI-PM 执行 Task Decision Review。多 Product Task 的正式基线变更必须由各相关 Product 分别确认自身部分。

关键检查点：Task 创建/修改、Revision 生效、派发前、Run 启动前、授权变化、交付提交、正式验收、Product 暂停/恢复/归档。Run 启动前必须重新检查 Task Revision、Product 门禁、Dependency、Repository/Provider 权限及资源条件。

权限不足不得降级为 Task Failed，不得自动降低门槛；记录拒绝原因，由 AI-PM 判断后续授权路径。授权撤销沿用安全策略：先撤销凭证并写入终态，再执行进程清理。

## 16. 事务、并发、幂等与异常恢复（审查94–95）

V1 使用 PostgreSQL 事务、行级锁、唯一约束、乐观并发控制、幂等键和现有审计/outbox，不引入分布式锁服务、Kafka、事件溯源平台或独立恢复微服务。

- Task Revision 更新使用预期版本/CAS；冲突时拒绝覆盖，由 AI-PM 重新读取并合并。
- Task Dependency DAG 检查与边写入处于同一事务和适当并发保护下。
- Run 派发/创建使用幂等键、唯一约束和事务；启动前重检门禁。
- Product 状态变化与 Run 启动竞争通过事务内最终检查、行锁/版本校验解决；外部进程生命周期不纳入数据库长事务。
- Harness 回报按 Run、执行代次和幂等标识验证；迟到或重复事件不得覆盖终态、重复创建交付或重复推进 Workflow。
- 数据库提交后才发布事件；复用现有 Outbox。消费者必须幂等。
- 依赖/门禁重算由关键状态事件触发，并在派发前再次执行；重启后重算未派发候选，不建复杂周期调度。

异常分层：Runner/Orchestrator 自动处理确定性基础设施故障和安全重试；AI-PM 决定修复、重跑、Workflow 调整、Provider/Harness 切换或新建 Task；人类处理业务目标、正式需求、重大取舍、超出授权及无法安全恢复事项。

外部 Harness 退出、Runner 重启、Provider 无响应或结果回报丢失时，先对账 Run/进程/执行标识/交付证据；不确定时 fail-closed，不猜测成功，不盲目重复启动。交付物存在但验收失败时保留交付和证据，Task 不自动完成。AI-PM 不可用时，Orchestrator 继续执行确定性安全门禁；需要语义判断的新工作不派发。

## 17. V1 明确不建设的能力

- 平台自有 Agent Work Unit 身份/组织/生命周期；
- OR/脚本化/任意布尔表达式依赖；
- 图数据库、独立依赖调度引擎、Dependency Recovery 子系统；
- 独立 Product Delivery 子系统；
- 跨 Product 优先级委员会、统一业务价值评分引擎；
- 通用复杂 RBAC/审批链；
- 分布式锁、Kafka、独立事件溯源平台；
- Runner HA、多实例竞争协调和容器化远程执行平台（除非未来业务规模明确要求）；
- 终态 Task/Run 的普通重开；
- Product 常规硬删除及级联删除研发历史。

## 18. 配套实施规范与代码现状基线

本架构文档定义目标边界，不等同数据库 DDL、API OpenAPI 或代码完成声明。配套文档《PA-07 Developer Implementation Specification V1.0》定义现有 PostgreSQL/Rust 模块映射、必要差距、事务/接口/测试要求。

代码现状参考 `runner-codebase-baseline-assessment.md`（2026-11-20，HEAD `a176e5c`，含未提交 P4 Phase 2 在途变更）。该报告覆盖执行体系关键模块与 41 个迁移，但明确存在未审查/浅查区域，且未运行全量测试、未连接 live DB。报告中的实现状态只代表该审查快照，不代表后续提交或 V1.0 目标已实现。

**V1.0 架构状态：** PA-07 设计基线定稿。实施差距、未决业务授权和代码验证事项转入配套实施规范及开发任务，不再作为普通架构专题反复展开。
