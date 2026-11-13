# Product Cognition 详细产品需求 V1.0 — 评审意见 V1

| 项目 | 内容 |
|---|---|
| 评审对象 | [product-cognition-detailed-requirements-v1.md](product-cognition-detailed-requirements-v1.md)（下称 PRD） |
| 上位文档 | [product-cognition-knowledge-base-definition-architecture-v1.md](product-cognition-knowledge-base-definition-architecture-v1.md)（下称上位稿） |
| 评审日期 | 2026-11-06 |
| 评审重点 | ① 认知对象模型 ② 变更审批机制 ③ P0/P1 建设优先级 |
| 代码基准 | ai-pm `813cc97` + 当前工作区（迁移至 0044） |
| 结论 | **有条件通过**：需求框架成立；三项重点各有阻断级问题，裁决后再交研发 |

---

## 0. 结论摘要

PRD 的目标、边界原则（AI 提议、人类确认、计划与现实分离）和八视角框架都成立，问题集中在「落到平台上会怎样」：

1. **对象模型**：八视角里约 9 类对象需要 PC 自己建模；Requirement、Design、ADR 只能是投影；Product Decision 在 PRD 内部归属自相矛盾。更大的问题是平台已有一套「正式认知」存储（S01-04），PRD 没有提及两者关系。
2. **审批机制**：PRD 实际只有一条管线（所有正式变更走同一个 Change Proposal），PC-FR-023 的四级风险没有落到这条管线上；而且平台策略引擎的默认方向（默认放行）与 PRD（默认审批）相反。
3. **建设优先级**：P0 有 16 项，其中 6 处依赖 P1、3 项没有验收标准；照现稿做完 P0，全局场景 A~H 只有 F、H 能完整验证。

| 编号 | 重点 | 严重度 | 问题 |
|---|---|---|---|
| R-PC-01 | ① | 阻断 | 与 S01-04 Project Cognition 的层级关系未定（V-05-7 待裁），平台已有正式认知存储 |
| R-PC-02 | ① | 阻断 | Product Decision 归属自相矛盾，平台已有 decision 文档类型，S03-05 待论证决策结构 |
| R-PC-03 | ① | 重要 | Requirements / Design 有被 PC 重复持有的风险，引用粒度未定 |
| R-PC-04 | ① | 重要 | Product Vision 与项目目标文档、S01-05 基线三处重叠 |
| R-PC-05 | ① | 重要 | 三种关系类型与来源、版本、冲突三套机制重复 |
| R-PC-06 | ① | 一般 | 内部结构与独立对象未区分，影响审批和版本粒度 |
| R-PC-07 | ② | 阻断 | 策略引擎默认放行，PC 写命令未登记即被 AI 直接执行（DEF-02 同型） |
| R-PC-08 | ② | 阻断 | 单一提议管线与四级风险不一致，多类变更不在任何一级 |
| R-PC-09 | ② | 重要 | 新增四套状态机互无映射，与 V0.5「零新机制」冲突 |
| R-PC-10 | ② | 重要 | 「授权人员」未落到现有审批人模型，PM 的授权范围无载体 |
| R-PC-11 | ② | 重要 | 规则经需求引入时，需求确认与规则确认的先后、互斥未定 |
| R-PC-12 | ③ | 阻断 | P0 依赖 P1 的倒挂 6 处 |
| R-PC-13 | ③ | 重要 | P0 中 3 项无验收标准，「基础能力」未界定，P0 末全局场景仅 2 个可验 |
| R-PC-14 | ③ | 一般 | 与 V0.5 MVP（WP-1 / V-05-6 / 验收故事线第 9 步）未对齐 |

---

## 1. 评审依据

- 两份 V1.0 文档（PRD、上位稿）。
- 代码事实：迁移 0003 / 0006 / 0014 / 0025 / 0036 / 0037 / 0039 / 0040 / 0044；`policy_engine.rs`、`command_center.rs`、`project_goal.rs`。
- 现行基线：[V0.5 MVP 范围基线 V1.2](../project-ai-team-v2/ai-pm-v05-mvp-scope-baseline-v1.md)、[S01-04 认知设计修订 V1.2](../ui/s01/s01-04-ai-pm-cognition-design-revision-v1-2.md)、[S03-00 权限与审批清单 V1.1](.docs/scenarios/s03/S03-00-permission-and-approval-inventory.md)。
- **关于 PC-G17**：全仓 docs 和两份 V1.0 中都没有这个编号。重点二暂以 PRD PC-FR-023 和上位稿 §8.3 的四级风险（低 / 普通 / 高 / 重大冲突）为对齐基准；PC-G17 原文到达后，以原文为准复核 §3.5 的审批矩阵。

---

## 2. 重点一：认知对象模型

### 2.1 结论

- 需要 PC 自己建模的独立对象：Vision、业务能力、功能模块、Actor、场景、业务流程、页面、业务对象、业务规则。
- 只能是投影（引用节点）：Requirement、Design、ADR。
- 待裁：Product Decision（PRD 内部三处说法不一）、功能点。
- 流程节点、页面区域、对象属性等应作为所属对象的内部结构，不单独确认。

### 2.2 平台已有的同名 / 同义资产

PRD 没有引用下列任何一项，但它们与 PRD 概念直接重叠：

| PRD 概念 | 已有资产 | 位置 | 关系 |
|---|---|---|---|
| Product Cognition / Cognition Item | `ai.cognition_item`（六域，状态 proposed / confirmed / rejected / investigation / deferred / superseded）+ `ai.project_knowledge`（确认后的正式认知，带 version、supersedes、confirmed_by） | `migrations/0036_s01_cognition.sql` | 同名；S01-04 V1.2 的 Business Domain、Product Cognition 两域覆盖 PRD 视角 1/2/4/5/7 |
| 正式认知版本 / 范围约束 | `ai.baseline_proposal` / `baseline_item`（类别含 scope、constraints）/ `ai.project_baseline` | `migrations/0037_s01_baseline.sql` | 与 Vision 的范围、边界、约束重叠 |
| 产品目标 | `ai.project_document` 中 `doc_key='project-goal'` 的文档；总控台初始化「项目目标」项据此判定 | `project_goal.rs:22`、`command_center.rs:749` | 与 Vision「核心目标」重叠 |
| Product Decision | `ai.project_document.doc_type='decision'`；`ai.knowledge_reference.reference_kind='decision'` | 0006 / 0014 | 决策已有 Markdown 文档载体 |
| Requirement | `delivery.requirement` / `requirement_revision` / `requirement_confirmation` | 0039 | PRD 已声明归 Requirements |
| Change Proposal | `ai.change_proposal`（risk_level、confidence、七态）+ `governance.approval_request`（1:1，required_role） | 0003 | 同名不同义 |
| 来源 | `source.source_input`（human_input / discovery / external_feedback / intake / import） | 0044 | 与 PC-FR-014 来源类型部分重叠 |
| 推断 / 确认 / 冲突 | `discovery.discovery_finding`（kind 含 speculation / contradicted；verification_status 含 CONFLICT） | 0025 / 0031 | 与 PC-FR-015、PC-FR-019 语义重叠；已是总控台「人工处理」计数来源 |
| Design / ADR | 无实体（S03-00 权限与审批清单 §3：技术设计批准、ADR 正式接受均「不存在」） | — | 投影目标尚未建立 |

### 2.3 八视角对象逐项分类

| 视角 | 对象 | 建议归类 | 权威归属 | 说明 |
|---|---|---|---|---|
| 1 Vision | 愿景、定位、价值、目标用户、范围、边界、约束 | PC 独立对象（项目内单例） | PC | 「核心目标」与 project-goal 二选一做唯一权威，见 R-PC-04 |
| 2 Capabilities | 业务能力（层级） | PC 独立对象 | PC | 平台无同类资产 |
| 3 Feature Modules | 模块、子模块 | PC 独立对象 | PC | 计划侧的产品模块；不得与 S01-04 Architecture 域的「核心模块」（系统模块，属现实侧证据）合并 |
| 3 Feature Modules | 功能点 | 待裁 | PC | 若功能点要单独关联需求或页面，就需要独立身份；否则作为模块内部结构 |
| 4 Actors & Scenarios | 用户角色 | PC 独立对象 | PC | 产品用户角色与平台成员角色（member.role、V2 project_role）不是一回事，命名须区分 |
| 4 Actors & Scenarios | 使用场景 | PC 独立对象 | PC | — |
| 5 Processes | 业务流程 | PC 独立对象 | PC | — |
| 5 Processes | 节点、状态、转换、条件、异常分支 | 流程内部结构 | PC | 随流程整体版本化、整体审批 |
| 6 Pages | 页面（清单、目的、所属模块、导航） | PC 独立对象 | PC | 视觉和交互细节归 Design；PC 只持页面的存在与产品关系 |
| 6 Pages | 区域、操作、交互结果、权限要求 | 页面内部结构 | PC | — |
| 7 Domain Objects & Rules | 业务对象 | PC 独立对象 | PC | 属性、生命周期为内部结构 |
| 7 Domain Objects & Rules | 业务规则 | PC 独立对象（受控最严） | PC | 正式业务规则的唯一权威；需求里的约束应引用规则，而不是复述 |
| 8 Requirements & Decisions | 需求 | 投影 | Requirements | PC 只存「对象 ↔ 需求」关系，不存需求正文和状态 |
| 8 Requirements & Decisions | 产品决策 | 待裁 | — | 见 R-PC-02 |
| （隐含） | 设计、ADR | 投影 | Design / Architecture | 见 R-PC-03 |
| 横切 | 冲突 | 治理记录（非认知对象） | PC | 有独立生命周期，不应同时是关系类型，见 R-PC-05 |
| 横切 | 质量问题、待人工事项 | 治理记录 | 总控台统一队列 | 见 §4.4 PC-I5 |

### 2.4 发现

#### R-PC-01【阻断】与 S01-04 Project Cognition 的层级关系未定

- **事实**：S01-04 V1.2 把 Product Cognition 定义为 Project Cognition 六域之一（`docs/s01-全局主菜单与信息架构设计基线/ui/s01-04-ai-pm-cognition-design-revision-v1-2.md:115`），其中 Business Domain 一域（`:111`）就含业务角色、业务对象、关键流程、业务规则。代码已落地 `ai.cognition_item` + `ai.project_knowledge`（0036），后者是确认后的正式认知。V0.5 范围基线把这一层级冲突登记为 V-05-7，状态仍是待裁（`ai-pm-v05-mvp-scope-baseline-v1.md:79,162`）。PRD 全文没有提到 S01-04。
- **附带事实**：代码六域为 identity / repository / domain / architecture / convention / evolution，与 V1.2 文档六域（有 Product Cognition、无 Repository）也不一致。
- **风险**：同一条「产品定位」或「业务规则」可能同时存在于 project_knowledge 和 PC 正式对象中，两边各自确认、各自版本。
- **建议**：PRD §2.4 增加 S01-04 边界一行，并在两种方案中裁决：
  - (a) 按内容切分：产品侧（定位、目标、业务领域、能力、场景、流程、规则）以 PC 为唯一正式权威；技术侧（架构、约定、仓库、演进）留在 S01-04，作为现实侧证据和 AI 上下文。已确认的产品侧 project_knowledge 条目作为 PC 初始草稿来源迁入，此后不双写。
  - (b) S01-04 整体降为 PC 的来源，不再称为正式认知。
  - 推荐 (a)，与 V-05-7 的现有推荐（PC 升格为独立能力，六域内容映射为视图切片）一致。

#### R-PC-02【阻断】Product Decision 归属自相矛盾

- **PRD 内部**：§3.6 写「正式产品决策由 Product Cognition 管理」；AC-011-04 写「需求和决策仍由各自所属模块管理」；§2.4 写 ADR 归 Design / Architecture；PC-FR-014 把 Product Decision 列为来源类型、PC-FR-017 把它列为影响分析对象，口吻都像外部对象。
- **平台**：`ai.project_document` 已有 `doc_type='decision'`（0006）；S03-00 权限与审批清单 §3.1 的 R6 边界把「五类业务决策」的结构路线留给 S03-05 论证。
- **建议**：裁决决策的唯一载体和所有者。推荐只建一种决策记录、按类别区分产品决策与 ADR：产品决策由 PC 确认，ADR 由 Design 接受；PC 视角 8 只做投影。载体（沿用 decision 类型的 Markdown 文档，还是结构化实体）须与 S03-05 一并定，否则 S03 会再建一套。

#### R-PC-03【重要】Requirements / Design 必须是投影，引用粒度要定

- **PRD 已声明**需求、设计的生命周期不归 PC（§2.4、PC-FR-011 功能范围），但下列条款会让 PC 事实上持有它们：
  - PC-FR-011 的需求内容列了需求目标、范围、依赖、约束；
  - PC-FR-016 把「Requirements / Design 发生变化」作为 Change Proposal 来源；
  - PC-FR-017 对 Requirements、Design「逐项提出修改建议」并「允许人工接受」；
  - PC-FR-023 高风险含「正式需求」「正式设计认知」。
- **Design 无落点**：V0.5 明确不实施 S03 Design/ADR 域（MVP 范围基线 §八 非目标第 2 条），PRD 中所有指向 Design 的条款在 V0.5 没有可引用的实体。
- **建议**：
  1. PC 中的需求、设计只是引用节点（类型 + 外部 ID），正文、状态、确认一律回源。
  2. 影响分析对需求、设计的「更新建议」只生成对方模块的待办（如需求的新修订草稿），不在 PC 内批准。
  3. 删除「正式设计认知」，或改述为「指向设计资产的关系」。
  4. 定引用粒度：关系指向需求身份（随当前修订漂移），还是指向某个修订（钉住）。PC-FR-018 要求能还原历史时间点，推荐两者都存：需求身份 + 建立关系时的修订号。

#### R-PC-04【重要】Product Vision 三处重叠

- 重叠来源：project-goal 文档（`project_goal.rs:22`，总控台初始化项「项目目标」据此判定，`command_center.rs:749`）；S01-05 基线的 identity / scope / constraints 类别（0037）；S01-04 的 identity 域。
- **建议**：「核心目标」在 PC 与 project-goal 之间二选一做唯一权威，另一方改为引用；范围、边界、约束由已确认的初始基线播种，此后以 PC 为权威，基线保持只读历史。

#### R-PC-05【重要】三种关系类型与三套机制重复

| 关系类型 | 重复的机制 | 建议 |
|---|---|---|
| Derived From | PC-FR-014 来源追溯（来源已是对象的多值属性，带位置、提取时间、提出者、审核人） | 来源只用来源记录；Derived From 删除，或只表示「对象派生自对象」 |
| Supersedes | PC-FR-018 版本链 + 确认状态 `SUPERSEDED` | 同一对象的新版本走版本链；Supersedes 只表示「对象 A 取代对象 B」 |
| Conflicts With | PC-FR-019 冲突实体（六态、双方依据、裁决理由） | 冲突只建实体；若保留此关系，只作为由已确认冲突派生的只读边 |

#### R-PC-06【一般】内部结构与独立对象未区分

流程的节点 / 状态 / 转换、页面的区域 / 交互、业务对象的属性 / 生命周期，以及可能的功能点，建议明确为所属对象的内部结构：随所属对象整体版本化、整体审批，不各自拥有确认状态。否则改一条状态转换就要发起多个提议，审批粒度失控（也影响重点二）。

### 2.5 本重点待裁

| 编号 | 待裁事项 | 推荐 |
|---|---|---|
| Q-PC-01 | PC 与 S01-04 六域的关系（即 V-05-7） | 方案 (a)：产品侧归 PC，技术侧留 S01-04 |
| Q-PC-02 | Product Decision 的唯一载体与所有者 | 一种决策记录按类别分属；与 S03-05 一并定 |
| Q-PC-03 | 需求引用粒度 | 需求身份 + 建立关系时的修订号 |
| Q-PC-04 | Vision「核心目标」与 project-goal 谁是权威 | 二选一，另一方改引用 |
| Q-PC-05 | 功能点是否独立对象 | 需要单独关联需求 / 页面时才独立 |

---

## 3. 重点二：变更审批机制

### 3.1 结论

对象新增、普通关联、正式业务规则变更**不应走同一审批流程**。PRD 现稿让所有正式变更走同一个六态 Change Proposal，PC-FR-023 的四级风险没有落到这条管线上。建议按「变更类别 × 风险等级」出一张审批矩阵（§3.5），取代「一律走提议」。

### 3.2 现行平台机制（对齐基准）

- **策略引擎**：默认 `auto_apply`（黑名单模型）；执行前升级条件 = 低置信度、影响面 > 10、存在未决冲突；红线命令带 required_role。确认类命令（`approve_requirement_revision`）的 AI 提案一律升级待人工，人工裁决也不得经执行器定稿，唯一合法路径是 REST/UI 统一服务（`policy_engine.rs:32,38,41-45`）。
- **审批实体**：approval_request、change_proposal、document_proposal、review_approval 四张（S03-00 权限与审批清单 §3）；另有确认记录 requirement_confirmation（确认人、身份、修订、内容哈希、动作、来源、审计 ID，0039），以及 cognition_item / baseline_item 的 decided_by（0036 / 0037）、discovery_finding 的核实状态。
- **审批人**：Lead（项目 owner）或 `core.approver_grant` 显式授权（0040），没有范围维度。
- **V0.5 范围基线**：PC 的建立 / 维护 / 验证链「复用知识提案 / 确认 / 否决链与质量门，零新机制」；「知识写入链新机制」列为非目标（`ai-pm-v05-mvp-scope-baseline-v1.md:84,170`）。

### 3.3 发现

#### R-PC-07【阻断】默认方向相反

- **PRD**：只有列举出的低风险动作可以自动执行，其余都要审批（白名单）。
- **平台**：未列入红线或升级条件的命令一律自动执行（黑名单，`policy_engine.rs:32`）。
- **风险**：PC 写命令如果接入现有命令执行器而没有逐个登记，AI 提出的「新增业务规则」会被直接执行。这与 S02-02 的 DEF-02（AutoApply 下 AI 定稿需求，修复见 `policy_engine.rs:41-44`）同型。
- **建议**：
  1. PRD 增加硬约束：PC 正式空间的写命令 fail-closed，未在审批矩阵登记的命令一律按高风险处理。
  2. PC 的确认类命令按 `approve_requirement_revision` 的方式处理：登记红线、执行器无条件拒绝、只能经 REST/UI 统一服务确认、MCP 不暴露确认工具。

#### R-PC-08【阻断】单一管线与四级风险不一致

- **同一件事两种说法**：PC-FR-016 规定「所有正式产品认知变更必须通过变更提议」，变更类型含「新增关系」；PC-FR-023 把「普通关系补充」列为普通风险（AI 提出、人工授权后执行），场景 D 同。两者是不是同一流程，PRD 没有说明。
- **多类变更不在任何一级**：PC-FR-016 的七种变更类型都没有风险属性；PC-FR-023 的高风险按对象类别列举（需求、规则、决策、设计、流程），能力、模块、Actor、场景、页面、业务对象、Vision 的变更不属于任何一级。
- **人工直接维护**：PC-FR-015 第 2 条写「用户正式提交并确认的信息可以直接进入相应确认流程」，PC-FR-016 的变更来源又包含「产品负责人主动调整」。负责人自己改是否也要发提议、再由另一人评审？提出者能否自批？都未定。单人项目（场景 A）会卡住。
- **初始化批量**：18.1 / 18.2 一次会产生大量草稿，逐对象发提议不可操作。S01-05 已有先例：一个提案含多个条目、逐条决定、项目内同时只允许一个待确认提案（0037）。

#### R-PC-09【重要】状态机过多且无映射

- PRD 新增四套状态：对象确认状态（PC-FR-015，五态）、提议状态（PC-FR-016，六态）、冲突状态（PC-FR-019，六态）、人工事项 Decision + Resolution（PC-FR-028，五态 + 六态）。没有一张表说明「一次批准动作如何驱动这些状态」；PC-NFR-004 要求一致，却没有一致性的定义。
- 平台已有六套同类状态：change_proposal 七态、document_proposal 八态、cognition_item 六态、baseline_item 五态、requirement_confirmation 动作、discovery_finding 核实状态。
- **建议**：
  1. 确认独立成记录，字段照 requirement_confirmation（确认人、身份、目标版本、内容哈希、动作、来源、审计 ID）。
  2. 对象自身只保留存在状态（草稿 / 正式 / 废弃）。
  3. 提议状态要么复用现有提案链，要么明确声明是新机制（这会推翻 V0.5「零新机制」，须单独裁决）。
  4. Decision / Resolution 只作用于待人工处理队列的条目，不作用于认知对象。

#### R-PC-10【重要】「授权人员」未落到审批人模型

- PRD 区分「项目负责人最终确认」与「PM 在授权范围内确认」（§2.2）；平台审批人模型只有 Lead 或 approver_grant，没有范围维度。
- **建议**：明确映射——项目负责人 = Lead；PM = approver_grant + 新增的授权范围（按对象类别或风险等级）。如果 V1 不做范围，PRD 应改为「所有确认人同权」。另外，批准须绑定内容版本：批准后内容再变，批准即失效（CAS）。这是 AC-023-03「授权范围绑定具体事项」的落地方式。

#### R-PC-11【重要】跨模块双重确认

- 需求引入新规则时，需求在 Requirements 确认，规则在 PC 确认，先后与互斥未定：
  - 需求先确认：规则仍未确认，需求已经依赖一条非正式规则；
  - 规则先确认：需求随后被拒，规则失去来源。
- **建议**：定「同一事实只确认一次」：规则以 PC 为唯一确认点；需求确认前检查其引用的规则均已正式确认，并与 AC-019-04 的冲突门合并为同一个需求确认前置检查。

### 3.4 本重点待裁

| 编号 | 待裁事项 | 推荐 |
|---|---|---|
| Q-PC-06 | 是否采纳 §3.5 审批矩阵；L1 是否允许有权限的提出者自确认 | 采纳；L1 允许「提交即确认」 |
| Q-PC-07 | PC 提议链复用现有提案链，还是新建 | 复用（与 V0.5「零新机制」一致）；若新建须同步修订 V0.5 基线 |
| Q-PC-08 | 审批人是否引入授权范围 | V1 先同权；需要 PM 分级时再加范围维度 |
| Q-PC-09 | 规则与需求的确认归属 | 规则只在 PC 确认，需求确认门做前置检查 |

### 3.5 建议审批矩阵（供裁决）

| 等级 | 适用变更 | 流程 | 谁能执行 | 谁能确认 | 正式版本 | 影响分析 |
|---|---|---|---|---|---|---|
| L0 自动 | 草稿空间内的任意编辑（含 AI 建草稿、提建议）；摘要、索引、不改含义的格式整理 | 无审批，留审计 | AI / 成员 | — | 不产生 | 否 |
| L1 单步确认 | 正式空间中：非受控对象（能力、模块、Actor、场景、页面、业务对象）的新增和描述性修改；普通关系（Supports、Realizes、Triggers、Involves、Displayed On、Depends On）的增删 | 提议 → 任一确认人确认；有权限的提出者可提交即确认 | 人；AI 仅在逐条授权后执行 | Lead / 审批人 | 产生 | 仅直接关联 |
| L2 正式审批 | 业务规则；业务流程（含状态转换）；Vision 的范围和边界；产品决策；Governed By 关系；正式对象的废弃与合并 | 提议（含冲突分析和影响分析）→ 确认人确认；AI 提案一律升级待人工；确认只走 REST/UI | 仅人 | Lead / 审批人（项目策略可要求提出者 ≠ 确认人） | 产生 | 直接 + 间接 |
| L3 冻结 | 涉及未决冲突的对象 | 相关 L1 / L2 确认暂停，等冲突裁决 | — | 冲突裁决人 | — | — |
| 不在 PC 审批 | 需求、设计、ADR 的内容和状态 | 各自模块审批；PC 只接收事件、做影响分析、给对方模块生成待办 | — | 对方模块 | — | 是 |

与现有策略引擎的对应关系：L0 ≈ auto_apply；L1 ≈ 执行前升级（escalate）；L2 ≈ 红线 + 执行器拒绝确认类命令；L3 部分对应 `unresolved_conflict` 升级条件，但现有条件只在执行前升级、不暂停确认，「按冲突范围暂停确认」是新增能力。

---

## 4. 重点三：P0/P1 建设优先级

### 4.1 结论

P0 共 16 项 FR，覆盖模型、关系、来源、确认、提议、版本、冲突检测与裁决、队列、权限、审计。其中 6 处依赖 P1，3 项没有验收标准；照现稿做完 P0，全局场景 A~H 只有 F、H 能完整验证。建议拆成 7 个切片（PC-I0~I6），PC-I1~I5 不依赖大模型。

### 4.2 R-PC-12【阻断】P0 依赖 P1 的倒挂

| # | P0 需求 | 依赖的 P1 内容 |
|---|---|---|
| 1 | PC-FR-002 概览展示 Vision、主要用户角色、核心流程（AC-002-01） | PC-FR-004 / 007 / 008 |
| 2 | PC-FR-016 提议必填「影响范围」（内容第 8 项） | PC-FR-017 |
| 3 | PC-FR-020 冲突解决后触发更新建议（功能要求第 8 条） | PC-FR-017 |
| 4 | PC-FR-019 检查新需求与现有需求、规则；AC-019-04 需求确认门；检查新资料与正式认知 | PC-FR-011 及 Requirements 确认服务改造；PC-FR-024 |
| 5 | PC-FR-022 队列条目的「处理状态」「最终决定」 | PC-FR-028 |
| 6 | PC-FR-016「AI 不能直接修改正式产品认知」（AC-016-02） | PC-FR-023 的执行权限 |

另有两处边界不清：PC-FR-003 要求「按视角筛选」，但 8 个视角中有 5 个在 P1；PC-FR-003 的 AC-003-04 与 PC-FR-013 的 AC-013-01 都是「对象 → 直接关联」，P0 与 P1 的界线重叠。建议一跳追踪归 P0（PC-FR-003），多跳追踪归 P1（PC-FR-013）。

### 4.3 R-PC-13【重要】可验收性

- **P0 中没有验收标准的需求**：PC-FR-012、PC-FR-020、PC-FR-030。另外 PC-FR-026、PC-FR-027 没有验收标准，也没有列入任何优先级。
- **「Big View 基础能力」未界定**：没有说明是 PC-FR-003 九项功能要求中的哪几项。
- **P0 结束时全局场景能否验证**：

| 场景 | P0 末可验 | 原因 |
|---|---|---|
| A 无 Repository 新项目 | 否 | 建立产品愿景依赖 PC-FR-004（P1） |
| B 从现有文档建立认知 | 否 | 依赖 PC-FR-024（P1） |
| C 新需求与历史规则冲突 | 否 | 依赖 PC-FR-011 和需求确认门 |
| D AI 发现模块关系缺失 | 否 | 依赖 PC-FR-021 质量检查（P1） |
| E 正式认知发生变化 | 否 | 依赖 PC-FR-017（P1） |
| F 历史认知还原 | 是 | PC-FR-014 / 018 均在 P0 |
| G 跨项目资料变化 | 否 | 依赖 PC-FR-024（P1） |
| H AI 高可信度推断 | 是 | PC-FR-015 / 016 均在 P0 |

### 4.4 建议切片

| 切片 | 目标 | 包含 | 退出验收 | 依赖 | 需要大模型 |
|---|---|---|---|---|---|
| PC-I0 前置裁决（非研发） | 定模型与审批规则 | Q-PC-01~12；对象类型字典、关系语义、审批矩阵（PRD OPEN-01 / 02 / 03 / 05） | 裁决落档 | — | 否 |
| PC-I1 认知骨架 | 项目自带 PC，人工维护草稿 | PC-FR-001 的非 AI 部分；PC-FR-004 最小面（定位、目标，从 P1 前移）；PC-FR-005；PC-FR-006；PC-FR-010 的对象与规则草稿；PC-FR-029 读写权限；PC-FR-030 创建 / 修改审计；PC-NFR-001 / 005 | AC-001-01 / 02、AC-004-01、AC-005-01、AC-006-01、AC-010-01 / 03、AC-029-01 / 02；补写审计 AC | PC-I0 | 否 |
| PC-I2 关系与来源 | 类型化关系、需求引用节点、来源记录 | PC-FR-012；PC-FR-013 的一跳部分；PC-FR-014（先支持用户输入、需求、source_input 三类来源） | 补写 PC-FR-012 的 AC；AC-005-02 / 03、AC-006-02 / 03、AC-010-02、AC-013-01、AC-014-03 | PC-I1 | 否 |
| PC-I3 正式化 | 确认、提议、版本、执行权限闸 | PC-FR-015 的确认部分；PC-FR-016 人工提议；PC-FR-018；PC-FR-023 的「AI 不得写正式空间」（从 P1 前移）；审批矩阵 L1 / L2；确认类命令登记红线 | AC-001-05、AC-005-05、AC-006-05、AC-010-05、AC-014-01、AC-015-01 / 03 / 04、AC-016-02~05、AC-018-01~05、AC-023-02~04、AC-029-03 / 04；场景 F | PC-I2 | 否 |
| PC-I4 概览与 Big View | 在已有对象上呈现全貌 | PC-FR-002（只展示已建的对象类型，其余显示空态）；PC-FR-003（类型 / 确认状态筛选、一跳追踪） | AC-002-01~04（以 Vision + 能力为准）、AC-003-01~05 | PC-I3 | 否 |
| PC-I5 冲突与待人工处理 | 人工登记冲突、结构化检查、接入总控台 | PC-FR-019 的人工登记与不依赖模型的结构检查；PC-FR-020；PC-FR-022（接入 `command_center.rs` 的「人工处理」聚合）；PC-FR-028 最小面（从 P1 前移：Decision / Resolution 分离） | 补写 PC-FR-020 的 AC；AC-019-02 / 03 / 05、AC-022-01~05、AC-028-01 / 02 | PC-I3 | 否 |
| PC-I6 AI 辅助 | AI 草稿、AI 提议、置信度、AI 冲突发现 | PC-FR-001 的 AI 部分；PC-FR-015 置信度；PC-FR-016 的 AI 提议；PC-FR-019 的 AI 发现 | AC-001-03、AC-005-04、AC-015-02、AC-016-01、AC-019-01；场景 H | PC-I3、PC-I5；项目级模型策略（0043 在途） | 是 |

不进 PC 切片：需求确认门（AC-019-04、场景 C）放在 Requirements 的统一应用服务里，与 PC-FR-011 一起排期。

每个切片都可以单独演示和验收；PC-I1~I5 不依赖大模型，可以先于模型链路稳定交付。

### 4.5 R-PC-14【一般】与 V0.5 MVP 未对齐

- **V-05-6 推荐档位**是「业务能力 / 功能模块先行，业务流程 / 页面交互 V0.6+」（`ai-pm-v05-mvp-scope-baseline-v1.md:161`）。PRD P0 另含业务对象与规则；建议保留（冲突检测和场景 C / F / H 都以规则为中心），同时更新 V-05-6 的推荐档位。PC-I1~I4 大致对应 WP-1 的 V-05-6 档位。
- **MVP 验收故事线第 9 步**（资料 / 讨论沉淀为 PC 提案 → 人工逐项确认 → Big View 更新，`:188`）依赖 AI 提议（PC-I6）和 PC-FR-024（PRD 列为 P1）。要么把 PC-FR-024 的最小面提入 MVP，要么改写第 9 步。

### 4.6 本重点待裁

| 编号 | 待裁事项 | 推荐 |
|---|---|---|
| Q-PC-10 | 是否采纳 §4.4 切片；PC-FR-004 最小面、PC-FR-023 写闸、PC-FR-028 最小面是否前移进 P0 | 采纳并前移 |
| Q-PC-11 | 需求确认门（AC-019-04）的归属与排期 | 移出 PC 的 P0，随 PC-FR-011 在 Requirements 统一服务中实现 |
| Q-PC-12 | MVP 故事线第 9 步与 PC-FR-024 的取舍 | 把 PC-FR-024 最小面（KB 已审核知识 → PC 草稿提议）提入 MVP |

---

## 5. 范围外顺带记录（不展开）

- **权威存储未声明**：上位架构是「Markdown + Git 管知识正文、PG 管业务状态」，需求域已裁「Markdown 唯一权威」。PC 对象的权威在 PG 还是 Markdown，PRD 没有说明（PC-NFR-009 只提到派生数据可重建）。
- **命名冲突**：Decision 至少有五种含义（产品决策、人工事项的 Decision Status、现有 decide 接口、ADR、`doc_type='decision'`）；Cognition、Change Proposal、Actor 也都与平台已有名称撞名。建议在 Domain Model 阶段给出命名表。

---

## 6. 待裁汇总

| 编号 | 重点 | 待裁事项 | 推荐 |
|---|---|---|---|
| Q-PC-01 | ① | PC 与 S01-04 六域的关系（V-05-7） | 产品侧归 PC，技术侧留 S01-04 |
| Q-PC-02 | ① | Product Decision 的唯一载体与所有者 | 一种决策记录按类别分属，与 S03-05 一并定 |
| Q-PC-03 | ① | 需求引用粒度 | 需求身份 + 修订号双存 |
| Q-PC-04 | ① | 「核心目标」的权威 | PC 与 project-goal 二选一 |
| Q-PC-05 | ① | 功能点是否独立对象 | 需要单独关联时才独立 |
| Q-PC-06 | ② | 审批矩阵与 L1 自确认 | 采纳，L1 允许提交即确认 |
| Q-PC-07 | ② | 提议链复用还是新建 | 复用 |
| Q-PC-08 | ② | 审批人授权范围 | V1 同权 |
| Q-PC-09 | ② | 规则与需求的确认归属 | 规则只在 PC 确认 |
| Q-PC-10 | ③ | 切片方案与前移项 | 采纳并前移 |
| Q-PC-11 | ③ | 需求确认门的归属 | 随 PC-FR-011 进 Requirements |
| Q-PC-12 | ③ | MVP 第 9 步与 PC-FR-024 | 提入 PC-FR-024 最小面 |

---

## 附录：证据索引

| 证据 | 位置 |
|---|---|
| 策略引擎默认放行 | `crates/pm-application/src/policy_engine.rs:32` |
| 未决冲突升级条件 | `crates/pm-application/src/policy_engine.rs:38` |
| 确认类命令只走 REST/UI、执行器拒绝（DEF-02 修复） | `crates/pm-application/src/policy_engine.rs:41-45` |
| 总控台「人工处理」聚合口径 | `crates/pm-application/src/command_center.rs:1108-1139` |
| 项目目标文档判定 | `crates/pm-application/src/command_center.rs:749`、`crates/pm-application/src/project_goal.rs:22` |
| AI 变更提议与审批请求 | `migrations/0003_platform.sql`（ai.change_proposal、governance.approval_request） |
| decision 文档类型 | `migrations/0006_knowledge_documents.sql`（ai.project_document） |
| 文档提议八态、知识引用 | `migrations/0014_knowledge_v2.sql` |
| 发现与核实状态 | `migrations/0025_discovery.sql`、`migrations/0031_s01_discovery_import_v2.sql` |
| S01-04 认知条目与正式认知 | `migrations/0036_s01_cognition.sql` |
| S01-05 基线提案、条目、版本 | `migrations/0037_s01_baseline.sql` |
| 需求确认记录 | `migrations/0039_s02_requirement_core.sql` |
| 显式审批人授权 | `migrations/0040_s02_i6a_approver_grant.sql` |
| 原始来源输入 | `migrations/0044_s02_i3_source_traceability.sql` |
| S01-04 六域定义 | `docs/s01-全局主菜单与信息架构设计基线/ui/s01-04-ai-pm-cognition-design-revision-v1-2.md:107-131` |
| V-05-6 / V-05-7、零新机制、非目标、故事线 | `docs/project-ai-team-v2/ai-pm-v05-mvp-scope-baseline-v1.md:79,84,161-162,170,188` |
| 四张审批实体、R6 边界、设计 / ADR 审批不存在 | `docs/scenarios/s03/S03-00-permission-and-approval-inventory.md` §3 |
