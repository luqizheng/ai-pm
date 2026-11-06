# S02-04 · Requirement 需求领域总体设计与最终裁决 — 设计提示词

> - 记录日期：2026-11-05
> - 记录方式：原文完整落库（不做评审改写）
> - 状态：已记录，待执行 / 待评审
> - 领域：S02 · Requirement 需求领域（UI / UX 与总体设计线）

---

S02 UI Ux 设计，先记录

# S02-04 · Requirement 需求领域总体设计与最终裁决 — 设计提示词

你是一名资深软件架构师、DDD 领域建模专家和企业级研发管理平台产品架构师。

请针对 smark-pm 项目开展：

**S02-04 · Requirement 需求领域总体设计与最终裁决（Requirement Domain Overall Design & Final Arbitration）**

本任务不是普通的需求管理页面设计，而是对整个 Requirement 需求领域进行总体架构设计、领域关系定义、规则裁决和后续实施基线整理。

## 一、任务目标

建立统一、完整、可实施的 Requirement 领域模型。

需要回答：

1. 什么是 Requirement？
2. 原始需求与正式需求是什么关系？
3. Requirement 的生命周期如何管理？
4. Revision 如何表达需求版本？
5. AI 如何参与需求分析和提案？
6. 人类如何确认正式需求？
7. Requirement 如何与 Task、Finding、Decision、Knowledge 关联？
8. Requirement 如何成为研发执行的依据？
9. 需求变更如何追踪？
10. 如何保证需求事实、版本和执行依据的一致性？

最终形成能够指导 S02-05 详细设计及后续技术实施的总体设计基线。

## 二、必须继承的用户裁决

以下六项是已经确认的设计裁决，不得重新推翻或擅自修改。

### R1 — Knowledge Markdown 是唯一正文权威

需求正文以 Knowledge Markdown 为唯一正文权威。

数据库中的其他结构化数据、索引、摘要和读模型不得形成第二份相互竞争的需求正文权威。

必须明确：
- Markdown 正文的权威归属。
- Requirement 与 Knowledge 的关系。
- Revision 与正文版本的映射。
- 索引、摘要和结构化元数据的定位。
- 正文更新与版本追踪规则。

### R2 — 正式需求必须经过人类确认

AI 可以：
- 分析原始需求。
- 提取需求内容。
- 生成需求提案。
- 提出修改建议。
- 识别冲突和缺失信息。

但 AI 不得自动将需求确认为正式需求。

正式确认必须由具备相应权限的人类完成。

必须明确：
- 提案与正式需求的区别。
- 人类确认的业务语义。
- 确认权限。
- 确认审计记录。
- 拒绝、退回和修订流程。

### R3 — 第一阶段不建立 Project Baseline 实体

第一阶段不创建独立的项目 Baseline 实体。

项目需求基线由各正式 Requirement Revision 的集合表达。

必须明确：
- 什么构成正式需求集合。
- 如何确定某个 Task 使用的需求版本。
- 如何追踪需求变更。
- 如何判断执行依据是否过期。
- 如何避免引入未经确认的 Baseline 实体。

### R4 — Task 与 Requirement 的执行依据规则

同一个 Task 对同一个 Requirement，默认只允许一个执行依据。

必须明确：
- Task 如何引用 Requirement。
- Task 如何引用具体 Revision。
- 同一 Requirement 是否允许多次关联。
- Revision 发生变化时如何处理。
- 执行历史如何保持可追溯。
- 多需求共同支撑一个 Task 的情况如何表达。

不得让 Task 的执行依据随着 Requirement 最新版本变化而被静默替换。

### R5 — Finding 纳入 S02 需求治理体系

Finding 纳入 S02 需求治理范围。

必须明确：
- Finding 的领域定位。
- Finding 与 Requirement 的关系。
- Finding 与 Task 的关系。
- Finding 是否可以产生 Requirement 提案。
- Finding 的证据来源与追溯关系。
- Finding 如何进入需求治理流程。

不要将 Finding 简单等同于普通 Task 或 Notification。

### R6 — Decision 的语义由 S02 定义，实现与 S03 协同

S02 负责定义 Decision 在需求治理领域中的业务语义和关系规则。

S03 负责与技术决策等其他领域协同，明确技术实现。

必须区分：
- 需求治理中的 Decision。
- 技术架构决策 ADR。
- 人类对需求提案的确认行为。
- 普通操作审计事件。

不能将这些概念混为一谈。

## 三、领域模型设计要求

请完整梳理以下概念及其关系：

- Raw Requirement：原始需求。
- Requirement：正式需求。
- Requirement Revision：需求版本。
- Requirement Proposal：需求提案。
- Knowledge Document：知识正文载体。
- Finding：发现项。
- Decision：需求领域决策。
- Task：执行任务。
- Evidence：证据。
- Confirmation：正式确认行为。

注意：以上是需要分析的概念清单，不代表全部都必须成为独立数据库实体。

必须逐项判断：
- 是否为领域实体。
- 是否为值对象。
- 是否为关联关系。
- 是否为事件或操作记录。
- 是否为读模型。
- 是否已有对应数据库表。
- 是否需要新增数据结构。

不能仅因为概念名称不同，就机械地新增数据库表。

## 四、必须输出的架构图

请生成以下图表：

1. Requirement 领域上下文图。
2. 核心领域实体关系图（ER / Domain Relationship）。
3. Raw Requirement → Requirement Proposal → Human Confirmation → Formal Requirement 生命周期图。
4. Requirement Revision 版本演进图。
5. Requirement 与 Task 执行依据关系图。
6. Finding → Requirement 治理关联图。
7. Decision 领域语义关系图。
8. Knowledge Markdown 权威正文与数据库投影关系图。
9. AI 与 Human 的权限和职责边界图。

图表必须能够表达真实的业务关系，不得只展示概念名称。

## 五、现有数据库和代码映射

必须以现有代码和数据库 Migration 为事实依据。

逐项核验：
- 已存在的数据表。
- 已存在的字段。
- 已存在的外键和关联。
- 已存在的状态枚举。
- 已存在的服务与命令。
- 已存在的事件和审计记录。
- 已存在的 Knowledge Markdown 存储机制。
- 已存在的 Requirement / Task / Finding / Decision 实现。

每项结论标注：

- Existing：现有代码已实现。
- Partial：已有部分实现。
- Missing：尚未实现。
- Conflict：现有实现与目标模型冲突。
- Unverified：证据不足，无法确认。

必须给出具体代码路径、Migration 编号和相关字段。

禁止凭空编造数据库表、字段、API 或代码能力。

## 六、重点设计问题

请重点解决：

1. Raw Requirement 是否独立于 Requirement。
2. Requirement 与 Knowledge Document 是否一对一。
3. Requirement Revision 如何映射 Markdown 正文版本。
4. AI Proposal 如何转化为正式 Requirement。
5. 人类确认是否构成明确的领域操作。
6. Requirement 被拒绝、废弃、替换时如何保留历史。
7. Task 如何固定执行依据。
8. Finding 如何关联和推动需求治理。
9. Decision 如何与 Requirement 形成可追溯关系。
10. 非功能需求如何进入统一需求治理体系。

非功能需求必须纳入 S02，与功能需求共同进入需求治理体系，通过需求类型区分，并采用适合其类型的验收标准。

不得另设独立的非功能需求治理流程。

## 七、评审与最终裁决要求

已有设计稿及专家评审意见必须作为输入。

尤其注意：
- S02-04 V1.1 尚有待修订问题。
- R-7 至 R-10 是通过评审前必须解决的 P0 问题。
- DEF-01、DEF-02 必须形成强制修复裁决。
- DEF-03、DEF-04 纳入后续设计处理。

必须逐项说明：
- 原问题。
- 代码事实。
- 设计冲突。
- 最终裁决。
- 修订内容。
- 后续实施影响。

不得将未完成的 P0 问题标记为已解决。

S02-04 负责总体设计和裁决，不直接代替 S02-05 详细设计，也不代表相关代码已经完成实施。

## 八、最终交付物

请输出：

1. 《S02-04 Requirement 需求领域总体设计与最终裁决》。
2. 完整领域模型图。
3. 核心实体及关系定义。
4. 状态生命周期定义。
5. 权限与人类正式确认协议。
6. Requirement Revision 版本管理规则。
7. Task 执行依据规则。
8. Finding 与 Decision 关系定义。
9. Knowledge Markdown 权威正文规则。
10. 现有数据库映射与代码差异报告。
11. P0 问题逐项处理结果。
12. S02-05 详细设计输入基线。
13. 未决问题与风险清单。
14. Acceptance Criteria。

## 九、最终设计原则

**需求是研发执行的正式依据，而不是 AI 自动生成的一段文本。**

需求正文必须有唯一权威来源；需求版本必须可追溯；正式需求必须经过人类确认；Task 必须明确其执行依据；Finding 和 Decision 必须纳入统一的需求治理关系。

所有设计必须同时满足领域一致性、数据一致性、权限边界和可实施性。