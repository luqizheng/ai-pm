# 四域目标对象模型与跨域业务契约设计基线 V1.0

> 正式设计基线草案 · 设计日期 2026-11-15 · 适用系统 PM-AI / smark-pm
> 承接：[三域架构裁决记录 V1.0](architecture-ruling-v1.md) · **非实施设计**（不定义 DDL/API/Migration SQL/代码改造）
> 落盘说明：2026-11-15 由专家提供全文，本仓按审计事实逐节核对（核对结论见文末附录 A）。

## 0. 文档定位与设计原则

### 0.1 设计目标

将已通过的架构裁决转化为可供后续详细设计、数据库映射审计和 Migration 任务书使用的目标对象模型。本阶段解决五个问题：

1. 四个领域分别拥有哪些正式业务对象。
2. 各对象之间具有什么业务关系。
3. 四个领域如何引用彼此的对象。
4. 哪些业务契约需要跨领域统一。
5. 正式内容、执行状态、人工决策分别由谁负责。

**不直接定义** PostgreSQL DDL、API 实现、Migration SQL 或具体代码改造。

### 0.2 核心架构原则

| 原则 | 设计要求 |
|---|---|
| 领域独立 | 四个领域各自拥有正式业务对象 |
| 身份独立 | 不因跨域引用而共享对象身份 |
| 内容权威 | 每个领域明确正式内容权威 |
| 状态权威 | PostgreSQL 管理业务状态、身份和版本引用 |
| 决策独立 | 人工确认由对象所属领域负责 |
| 契约统一 | 统一跨域表达，不统一领域内部实现 |
| 引用优先 | 跨域通过稳定引用协作，不复制权威对象 |
| 可追溯 | 所有正式结论能够追溯来源、证据和人工决策 |

特别强调：**四域是业务领域划分，不代表四套数据库，也不代表必须建立四个独立 Schema。**

## 1. 四域总体架构

### 1.1 四域定位

- **KB — Knowledge Base（知识权威）**：可复用知识、经验、规则、方法及其来源、审核和版本管理。
- **Analysis — 分析领域（分析权威）**：分析执行、分析报告、报告修订、发现、结论和建议。
- **PC — Product Cognition（产品认知权威）**：产品定位、业务领域、能力、模块、角色、场景、流程、业务对象和规则。
- **Project Cognition — 项目技术认知（项目技术认知权威）**：项目架构、技术约定、仓库事实、技术决策及项目技术知识。

跨域公共业务契约：**Run Contract · Finding Contract · Evidence Reference · Human Decision Contract**。
公共契约只规定身份、引用、表达与互操作规则，不取代领域对象及其生命周期。

### 1.2 四域权威边界

| 内容类型 | 唯一归属领域 |
|---|---|
| 可复用知识、经验、方法 | KB |
| 仓库分析、分析报告、分析发现 | Analysis |
| 产品定位、业务规则、产品能力 | PC |
| 项目架构、技术约定、仓库事实 | Project Cognition |

同一信息可能被多个领域引用，但只能有一个正式权威归属。例：
- Analysis 发现架构问题 → Analysis Finding。
- 项目技术认知可基于证据提出技术认知 Proposal。
- KB 在人工确认跨项目复用价值后，形成独立 Memory 提案。
- PC 可引用分析证据，但不能因此获得项目技术认知的维护权。

## 2. 四域正式对象模型

### 2.1 KB — 知识库领域

职责：将原始资料、知识提案和可复用知识转化为正式知识资产。

核心对象：

| 对象 | 身份 | 职责 |
|---|---|---|
| Source | KB Source | 外部资料、导入来源及来源元数据 |
| Document | KB Document | 文档业务身份 |
| Document Revision | KB Document Revision | 文档版本及内容引用 |
| Memory | KB Memory | 正式可复用知识 |
| Memory Revision | KB Memory Revision | 知识内容版本 |
| Knowledge Review | KB Review | 知识审核过程 |
| Knowledge Proposal | KB Proposal | 待确认知识提案 |

对象关系：
```
Source └── Document └── Document Revision
Knowledge Proposal └── Review └── Memory └── Memory Revision
```

权威规则：
1. KB 独立维护知识内容和知识生命周期。
2. Document 与 Memory 是不同业务对象。
3. Document Revision 与 Memory Revision 不共享版本身份。
4. Memory 必须经过 KB 自身的正式确认流程。
5. 其他领域产生的确认产物不能自动成为 Memory。
6. 正式知识正文按 Q4 决策排期 Git 化；完成前 PostgreSQL 仍为现行正文权威。

特别约束：审阅中心可提供统一入口，但 KB 保留自身审核语义和审核权。

### 2.2 Analysis — 分析领域（本次重点）

职责：分析活动的业务身份、分析结果、结果版本及其证据链。

核心对象：

| 对象 | 身份 | 职责 |
|---|---|---|
| Analysis Run | Analysis Run | 一次分析执行活动 |
| Intake Task | Intake Task | 导入编排及业务工作流 |
| Analysis Report | Analysis Report | 分析报告业务身份 |
| Report Revision | Report Revision | 报告内容版本 |
| Analysis Finding | Analysis Finding | 分析发现 |
| Analysis Conclusion | Analysis Conclusion | 分析结论 |
| Analysis Recommendation | Analysis Recommendation | 分析建议 |

#### 2.2.1 三个关键对象必须分开

- **Analysis Run（执行活动统一身份）**：执行目标/输入/上下文/发起主体/时间/状态/产出引用。
- **Intake Task（导入编排实体）**：导入任务身份/流程状态/输入资料组织/处理阶段/与分析结果的业务关联。**Intake Task 不是 Run，也不应被强行改造成 Run。**
- **Analysis Report（分析结果实体）**：报告身份/主题/关联项目或仓库/关联分析活动/Findings+Conclusions+Recommendations/修订历史。

#### 2.2.2 Analysis 对象关系

箭头表达业务关联，不代表已确定一对一数据库外键。（附图：Intake Task ⇕ Analysis Run → Analysis Report → Report Revision 1..N（每 Revision 对应一个 Git Commit）→ Findings / Conclusions / Recommendations）

#### 2.2.3 重新分析与报告修订（必须严格保护的身份边界）

| 操作 | Run | Report | Report Revision |
|---|---|---|---|
| 首次分析 | 新建 | 新建 | 新建 |
| 重新分析 | 新建 | 可新建报告或建立新分析轮次 | 根据结果产生 |
| 修订原报告 | 不要求新建 | 保持原身份 | 新建 Revision |
| 修正文档格式或措辞 | 不要求新建 | 保持原身份 | 新建 Revision |
| 重新分析并取代旧报告 | 新建 | 按明确的轮次策略决定 | 产生对应新版本 |

"重新分析是否沿用同一 Report 身份"仍需后续详细对象模型确认。已确定：**Report Revision 不依附 KB Document Revision**；Analysis 是 Report 的业务身份和版本管理主体；**Git 是报告正文权威**。

### 2.3 PC — Product Cognition

职责：维护产品正式认知，确保需求、设计、业务规则与产品结构的一致性。

核心对象：Cognition Proposal / Cognition Object / Cognition Version / Cognition Conflict / Cognition Evidence / Cognition Review。

认知对象范围：产品定位、目标用户、业务领域、产品能力、模块、角色、场景、流程、业务对象、业务规则。

生命周期：Evidence → Cognition Proposal → Review/Conflict Detection → Human Confirmation → Formal Cognition Object → Cognition Version → 后续修订/冻结/退役。

强制规则：
1. AI 可提出 Proposal，但不能替代人工确认。
2. Proposal 不等于正式 Cognition Object。
3. Cognition Object 的版本由 PC 管理。
4. Conflict 必须具备 Detection、Review、Resolution、Freeze Lifecycle、Audit 五项完整能力。
5. **PC 的人工确认完备不代表 Project Cognition 的人工确认完备。**
6. PC 501 占位不得被视为完整实现。
7. git_committed 不可达状态不得标记为已实现。

### 2.4 Project Cognition — 项目技术认知

职责：维护单个项目的技术事实、技术约定、架构认知及仓库相关知识。

核心对象：Project Knowledge Proposal / Project Knowledge / Project Knowledge Version / Project Evidence / Project Decision / Project Cognition Review。

认知范围：系统架构、技术栈、技术约定、仓库结构、模块依赖、技术债务、工程规范、项目级技术决策、仓库事实与实现状态。

生命周期：Analysis Finding / External Evidence → Project Knowledge Proposal → Human Review → Human Confirmation → Formal Project Knowledge → Version / Revision。

特别约束：当前 `ai.project_knowledge` 归属 **Project Cognition**（不是 KB）；其正式内容权威及 Git/Markdown 迁移方案另行确认，**不能套用 KB 的 Q4 裁决**。

## 3. 跨域对象引用模型

### 3.1 基本原则

四域可引用其他领域对象，但不能通过引用获得所有权。（例：Analysis 引用 KB Document / Project Repository；PC 引用 Analysis Report；Project Cognition 引用 Analysis Finding；KB 引用 Analysis Report 作知识来源——均为引用，非归属转移。）

### 3.2 统一对象引用结构

```
ObjectReference:
  domain: string
  object_type: string
  object_id: string
  version_id: optional<string>
  tenant_id: string
  project_id: optional<string>
```

规则：
1. 引用必须保留来源领域。
2. 引用必须能定位对象稳定身份。
3. 复现历史事实必须引用具体版本。
4. 不能默认引用最新版本。
5. 必须执行租户、项目及领域访问权限校验。
6. 被引用对象退役时，引用记录仍应保留可追溯性。
7. 对象身份不得因迁移数据库或 Git 仓库而改变。

注：目标契约；字段是否 UUID/复合标识、DB 存储方式留待技术详细设计。

### 3.3 跨域引用关系矩阵

| 来源领域 | 目标领域 | 引用目的 | 是否改变目标权威 |
|---|---|---|---|
| Analysis | KB | 分析输入资料 | 否 |
| Analysis | Project Cognition | 项目技术上下文 | 否 |
| Analysis | PC | 产品上下文 | 否 |
| PC | Analysis | 认知证据及分析依据 | 否 |
| Project Cognition | Analysis | 技术证据及报告 | 否 |
| KB | Analysis | 知识来源追溯 | 否 |
| KB | PC | 知识来源或业务关联 | 否 |
| KB | Project Cognition | 项目技术资料关联 | 否 |

## 4. 四项公共业务契约

公共契约不是新的统一业务领域，也不要求统一数据库结构。

### 4.1 Run Contract

```
RunContract:
  run_id / run_type / owning_domain / execution_source
  subject_refs[] / input_refs[] / output_refs[]
  initiated_by / status
  started_at? / completed_at?
```

规则：Run 代表执行活动不代表业务工作流；Intake Task 不因接入而消失；Report 不因接入而变成执行实体；各领域保留自己的 Run 实现；经 Adapter 映射已有执行实体；外部统一状态映射领域内部状态。

### 4.2 Finding Contract

```
FindingContract:
  finding_id / owning_domain / finding_type
  source_refs[] / evidence_refs[]
  severity? / subject_refs[]
  status / created_at / updated_at
```

适用对象：Discovery Finding（Project Cognition / Discovery 相关业务）、Design Review Finding（Design）、Delivery Review Finding（Delivery）、Analysis Finding（Analysis）、Project Cognition Item（依业务语义决定是否适配——**不能因名称相近就把 Cognition Item 改造成 Finding**）。统一的是外部表达、身份与证据引用规则，不是内部状态机。

### 4.3 Evidence Reference

```
EvidenceReference:
  evidence_id / source_domain / source_type / source_object_id
  source_version_id? / locator? / captured_at? / integrity_hash?
```

证据必须区分：原始来源/来源对象/来源版本/内容定位/获取时间/完整性校验。证据引用不意味着复制或拥有原始内容；原始文件与转换产物归 Object Storage，正式业务内容归对应领域。

### 4.4 Human Decision Contract

```
HumanDecision:
  decision_id / owning_domain / target_ref: ObjectReference
  decision_type / decision_result / decided_by / decided_at
  reason? / evidence_refs[] / policy_ref?
```

统一要求：明确人工主体/决策对象/决策结果/理由/当时证据/可审计；**决策不能跨领域借用**（PC 对产品认知的确认不能作为 Project Cognition 对技术事实的确认）。AI 可提供建议、证据和候选决策，但不得伪造人工决策主体。

## 5. 权威分工与版本管理模型

### 5.1 四类基础设施职责

| 基础设施 | 权威职责 | 不负责 |
|---|---|---|
| Git / Markdown | 正式内容正文及历史版本 | 业务审批状态 |
| PostgreSQL | 对象身份、状态、版本引用、审批、审计、事件 | 已迁移领域的正式正文权威 |
| Object Storage | 原始文件、附件、转换产物 | 正式知识与认知生命周期 |
| Index / PageIndex | 检索、定位、索引及派生数据 | 正式内容权威 |

**PostgreSQL 不是所有业务内容的最高权威**——它是业务身份、状态和治理记录的权威。

### 5.2 分领域内容权威矩阵

| 领域 | 当前权威 | 最终目标权威 | 迁移状态 |
|---|---|---|---|
| KB Document | PostgreSQL（迁移前） | Git / Markdown | 待实施 |
| KB Memory | PostgreSQL（迁移前） | Git / Markdown | 待实施 |
| Analysis Report | Git | Git | 按 Q2 裁决 |
| Project Cognition | 待进一步确认 | 待正式确认 | 不得假定已 Git 化 |
| PC Cognition | 当前实现需核实 | Git / Markdown（PCGIT 目标） | P4 兑现 |

必须区分：Analysis Report 的 Git 正文权威已通过 Q2 裁决；PCGIT 目标已通过 Q7 裁决，但 git_committed 不可达的现状不能描述为已实现。

### 5.3 Git Commit 与 DB Revision 的一致性

```
Domain Object → Domain Revision
  (revision_id / object_id / content_commit / content_hash / lifecycle_state)
     → Git Commit → Markdown / Content
```

强制规则：
1. 每个正式 Revision 必须能定位唯一、可验证的内容版本。
2. Git Commit 与 Revision 双向可追溯。
3. DB 记录不能指向不存在或不可访问的 Git Commit。
4. Git 写入失败不能导致 DB 显示已提交。
5. DB 写入失败必须有可恢复的 Git 提交处理机制。
6. 必须设计失败补偿、幂等处理和恢复检查。
7. 迁移过程中不得长期双写或双主。

建议提交过程：准备 Revision → 生成内容 → 写入 Git → 验证 Commit → 提交 DB Revision 引用 → Revision 正式生效 → 记录审计事件。Git 成功而 DB 失败 → 进入可识别待恢复状态，不静默丢失不伪造成功。事务协调/补偿队列/异常状态机留各领域实施设计。

## 6. 生命周期与状态管理

### 6.1 状态归属原则

内部状态机由所属领域维护；公共 API/Event 表达建议词汇（pending/reviewing/confirmed/rejected/frozen/retired），**不要求批量修改历史数据库状态**。

### 6.2 Proposal 与 Formal Object 的区别

所有具有知识/认知治理能力的领域必须区分：Proposal / Formal Object / Revision-Version / Decision / Evidence 五类概念，但不强迫四域用相同表结构或生命周期。

### 6.3 领域生命周期矩阵

| 能力 | KB | Analysis | PC | Project Cognition |
|---|---|---|---|---|
| 创建 | 支持 | 支持 | 支持 | 支持 |
| 提案 | 支持 | 按业务需要 | 支持 | 支持 |
| 人工审核 | 支持 | 按业务操作定义 | 必须 | 必须 |
| 正式确认 | KB 自主管理 | 报告按业务规则管理 | 必须 | 必须 |
| 版本管理 | 必须 | Report Revision | 必须 | 必须 |
| 冲突治理 | 按知识规则 | 按分析业务规则 | 必须 | 待补完整设计 |
| 冻结 | 按领域规则 | 按领域规则 | 必须 | 必须明确 |
| 退役 | 按领域规则 | 按领域规则 | 按领域规则 | 按领域规则 |
| 审计 | 必须 | 必须 | 必须 | 必须 |

注意：Analysis Finding 的确认不自动等于 PC 或 Project Cognition 的确认。

## 7. 权限与租户隔离契约（P1 安全约束）

### 7.1 权限原则

默认拒绝 + 权限交集 + 领域所有权校验 + 租户隔离。一次跨域访问至少通过七层：身份验证 → Team/Tenant 成员关系 → 目标对象租户 → 项目归属（如适用）→ 来源领域访问权限 → 目标领域对象访问权限 → 具体操作权限。

### 7.2 跨域访问责任

| 操作 | 权限责任主体 |
|---|---|
| 创建对象 | 对象所属领域 |
| 查看对象 | 目标领域及其访问策略 |
| 修改对象 | 对象所属领域 |
| 人工确认 | 对象所属领域的授权人工主体 |
| 建立跨域引用 | 引用发起领域 + 目标领域 |
| 转换为其他领域对象 | 目标领域 |
| 退役对象 | 对象所属领域 |

### 7.3 禁止行为

跨域引用绕权限 / 后台任务绕租户隔离 / MCP 绕项目归属 / 历史对象 ID 跨租户访问 / 直连 DB 绕业务权限 / AI Agent 冒充人工审批主体。

安全审计升级规则：发现真实跨租户访问可能性 → 立即将 Q11 从 P1 升级 P0。

## 8. 跨域转换与知识沉淀规则

引用（Reference）、转换（Proposal）、确认（Confirmation）、发布（Publication）是四种不同业务动作。

- **Analysis → Project Cognition**：Analysis Finding → Evidence Reference → Project Knowledge Proposal → 人工审核 → 人工确认 → Formal Project Knowledge → Version。禁止直接把 Finding 写成正式 Project Knowledge。
- **Analysis → PC**：Analysis Finding/Conclusion → Evidence Reference → PC Cognition Proposal → PC Review → Human Confirmation → Formal Cognition Object。Analysis 提供依据但不替代 PC 认知管理。
- **Analysis / Project Cognition → KB**：跨项目复用必须经独立知识沉淀流程（Candidate Knowledge → KB Proposal → KB Knowledge Review → Human Confirmation → KB Memory）。绝对禁止：自动把 Finding 变 Memory；自动把 Project Knowledge 复制成 Memory；项目特定技术事实直接认定为通用知识；通过复制内容改变权威归属。
- **PC → Project Cognition**：PC 定义产品业务能力，Project Cognition 记录其技术实现。可建立产品能力引用/技术实现关联/实现状态证据/技术约束映射；产品业务规则不得复制为项目技术规则，技术实现事实不得反向成为产品正式业务定义。

## 9. 目标对象注册表（业务对象清单，非数据库表清单）

| Domain | Object | Identity Owner | Content Owner |
|---|---|---|---|
| KB | Source | KB | KB |
| KB | Document | KB | KB |
| KB | Document Revision | KB | KB |
| KB | Knowledge Proposal | KB | KB |
| KB | Memory | KB | KB |
| KB | Memory Revision | KB | KB |
| KB | Knowledge Review | KB | KB |
| Analysis | Analysis Run | Analysis | Analysis |
| Analysis | Intake Task | Intake / 导入业务 | 对应业务领域 |
| Analysis | Analysis Report | Analysis | Analysis |
| Analysis | Report Revision | Analysis | Analysis |
| Analysis | Analysis Finding | Analysis | Analysis |
| Analysis | Analysis Conclusion | Analysis | Analysis |
| Analysis | Analysis Recommendation | Analysis | Analysis |
| PC | Cognition Proposal | PC | PC |
| PC | Cognition Object | PC | PC |
| PC | Cognition Version | PC | PC |
| PC | Cognition Conflict | PC | PC |
| PC | Cognition Evidence | PC | PC |
| Project Cognition | Knowledge Proposal | Project Cognition | Project Cognition |
| Project Cognition | Project Knowledge | Project Cognition | Project Cognition |
| Project Cognition | Knowledge Version | Project Cognition | Project Cognition |
| Project Cognition | Project Evidence | Project Cognition | Project Cognition |
| Project Cognition | Project Decision | Project Cognition | Project Cognition |

Intake Task 暂按业务工作流实体独立登记，不因参与 Analysis 流程就认定为 Analysis Run 子对象。

## 10. 对象关系基数与约束（目标业务关系，不代表现有 DB 已满足）

| 关系 | 基数 | 约束 |
|---|---|---|
| Source → Document | 1:N | 一个来源可形成多个文档 |
| Document → Document Revision | 1:N | 每次正式修订独立版本 |
| Memory → Memory Revision | 1:N | 知识身份与版本身份分离 |
| Analysis Run → Report | 1:N 或 0:N | 一个执行活动可产生多个分析结果 |
| Intake Task → Analysis Run | 1:N 或 0:N | 导入任务可触发多次执行 |
| Analysis Report → Report Revision | 1:N | 每个报告必须有独立修订历史 |
| Analysis Report → Finding / Conclusion / Recommendation | 各 1:N | 报告包含多个发现/结论/建议 |
| Proposal → Evidence | N:M | 提案可引用多份证据 |
| Cognition Object → Version | 1:N | 正式对象具有版本历史 |
| Cognition Object → Conflict | 1:N | 一个认知对象可涉及多个冲突 |
| Cross-domain Object → Evidence | N:M | 证据可支持多个业务对象 |

### 10.1 需要特别保留的关系开放项

1. Intake Task 与 Analysis Run 是否允许脱离关系。
2. Analysis Run 是否可产生多个 Report。
3. Report 重新分析采用新 Report 还是同一 Report 的新分析轮次。
4. Finding 是 Report 的独立子对象还是具有脱离 Report 的独立生命周期。
5. Report Revision 与 Findings 是否必须绑定具体版本。
6. Evidence 采用统一独立对象身份还是以 Evidence Reference 为主。
7. Project Cognition 是否需要与 PC 类似的 Proposal/Formal Object/Version 三层模型。

以上需结合真实业务链及现有数据库证据确认。

## 11. 事件与审计模型

### 11.1 事件原则

公共事件契约负责跨领域通知，不代替领域状态机。统一事件信封：

```
DomainEvent:
  event_id / event_type / event_version / owning_domain
  tenant_id / project_id?
  subject_ref: ObjectReference
  actor: { actor_type, actor_id }
  occurred_at / correlation_id? / causation_id?
  payload: object
```

### 11.2 事件分类

| 类别 | 典型事件 | 责任领域 |
|---|---|---|
| Object Lifecycle | ObjectCreated / ObjectRetired | 对象所属领域 |
| Proposal | ProposalCreated / ProposalReviewed | 提案所属领域 |
| Decision | DecisionApproved / DecisionRejected | 决策所属领域 |
| Version | RevisionCreated / RevisionPublished | 内容所属领域 |
| Analysis | RunStarted / RunCompleted | Analysis |
| Report | ReportCreated / ReportRevised | Analysis |
| Finding | FindingCreated / FindingResolved | Finding 所属领域 |
| Conflict | ConflictDetected / ConflictResolved | 冲突所属领域 |
| Cross-domain | ReferenceCreated / ReferenceRevoked | 引用发起领域 |

### 11.3 审计必须回答的问题

任何一个正式业务对象，都应能够回答：
- 谁创建了它？谁修改了它？谁确认了它？
- 确认依据是什么？使用了哪些证据？确认时对应哪个版本？
- 后续发生了什么变化？是否被其他领域引用？是否发生过冻结、撤销或退役？

审计记录必须保留历史事实，不能因为对象状态变化而覆盖过去的决策。

## 12. 现有数据库映射审计要求

本文件定义目标模型，下一步需与真实数据库逐项对照。**不能仅凭 Schema 名称或表名判断领域归属。**

### 12.1 审计对象

| 审计范围 | 代表对象 |
|---|---|
| KB | kb.memory、Document、Revision、Review |
| Analysis | repo_analysis_report、ai.analysis_run、Findings |
| PC | pc.cognition_object、Proposal、Version、Conflict |
| Project Cognition | ai.project_knowledge、相关 Proposal、Evidence、Version |
| 跨域基础 | 引用关系、事件、人工决策、权限、租户隔离 |

### 12.2 每个数据库对象必须完成七项检查

1. 业务归属：属于哪个领域？
2. 身份性质：是否拥有独立业务身份？
3. 内容权威：正文目前由哪里维护？
4. 状态权威：谁负责生命周期？
5. 引用关系：是否正确指向其他领域？
6. 权限边界：是否存在跨租户或跨项目访问风险？
7. 实现状态：已实现、部分实现、占位或未实现？

### 12.3 审计结果分类

| 结论 | 处理 |
|---|---|
| MATCH | 符合目标模型，保持 |
| PARTIAL | 部分符合，补齐缺失能力 |
| MISMATCH | 领域或对象语义错误，需要设计修正 |
| MISSING | 缺少必要业务能力 |
| STUB | 只有占位或不可达实现 |
| UNKNOWN | 证据不足，不得推测 |

每项审计结论必须包含：数据库对象名称 / 当前业务语义 / 目标业务对象 / 实证依据 / 差异说明 / 风险等级 / 推荐处理方式 / 是否需要 Migration。

**禁止在审计过程中顺手修改 Schema。**

### 12.4 Migration 前置门槛（须同时满足）

目标对象身份明确 / 对象归属确认 / 对象关系确认 / 内容权威确认 / 版本关系确认 / 权限边界确认 / 历史数据兼容方案确认 / 失败恢复与回滚方案确认 / **真实业务链路已完成 E2E 验证**。

## 13. 尚未裁决与待详细设计事项

本设计基线不能通过设计者自行补充来替代尚未作出的架构裁决。

### 13.1 必须继续保留的开放事项

| ID | 待决策问题 | 建议处理阶段 |
|---|---|---|
| O-01 | Run 与 Intake Task 的精确关联基数 | Analysis 详细设计 |
| O-02 | Report 重新分析时的身份与轮次规则 | Analysis 详细设计 |
| O-03 | Finding 的独立生命周期及版本绑定 | Finding Contract 设计 |
| O-04 | Evidence 的独立身份与存储实现 | 公共契约详细设计 |
| O-05 | Project Cognition 正式内容权威 | Project Cognition 详细设计 |
| O-06 | PC 与 Project Cognition 的对象关联词汇 | 跨域关系设计 |
| O-07 | 四域 Proposal / Review 的具体状态机 | 各领域详细设计 |
| O-08 | Human Decision 是否需要独立公共决策记录实体 | 审计与审批设计 |
| O-09 | Git 与 DB Revision 的提交一致性及恢复机制 | 各领域实施设计 |
| O-10 | 四域事件发布及消费可靠性 | 事件架构设计 |
| O-11 | Project Cognition 人工确认主体门 | Project Cognition 审计 |
| O-12 | 跨租户引用及旧 ai 栈安全问题 | P1 安全复核 |

### 13.2 原裁决中的实施事项继续有效

Q1~Q12 全部约束继续有效（Q1 先 Intake 关联不加 Run FK / Q2 Report+Revision 独立+Git 正文 / Q3 三知识对象独立 / Q4 KB 排期 Git 化禁双主 / Q5 Finding 领域独立+公共契约 / Q6 Conflict 五项闭环 / Q7 PCGIT 真兑现 / Q8 KB 接入审阅中心保审核语义 / Q9 501 不冒充已实现 / Q10 修正 owner_domain='kb' / Q11 安全复核必要时升 P0 / Q12 先真实 E2E 再重构）。

## 14. 后续实施路线

不改变原裁决的六阶段实施顺序：
1. **P0** 证据核实与设计冻结（完成 Q1~Q12 实证核查、目标模型确认及 E2E 测试设计；禁止 Schema 重构）
2. **P1** 真实业务链打通（仓库分析→报告→KB 确认→PC Proposal→人工审批→发布→Close；至少一条真实完整业务链成功）
3. **P2** Analysis 对象模型落地（Run/Intake/Report/Revision 整体设计 + Finding Contract）
4. **P3** KB 内容权威迁移（Git 正文权威、Revision 一致性、失败恢复）
5. **P4** PCGIT 与冲突治理（PC 内容权威迁移、Conflict 五项闭环、审阅中心接入）
6. **P5** 旧 ai 栈收敛（按安全审计与调用证据逐步退役旧写入通道、租户隔离加固、历史状态清理）

## 15. 设计基线验收标准

本文件只有满足以下 12 项条件，才能作为后续实施设计的正式输入：

| # | 验收项 | 要求 |
|---|---|---|
| 1 | 四域边界 | KB、Analysis、PC、Project Cognition 的业务职责与对象归属明确 |
| 2 | 对象身份 | 正式对象与版本对象身份分离 |
| 3 | Analysis 模型 | Run、Intake、Report、Report Revision 语义分离 |
| 4 | 跨域引用 | 具备稳定身份、版本引用及权限校验规则 |
| 5 | 公共契约 | Run、Finding、Evidence、Human Decision 四项契约完整 |
| 6 | 人工决策 | 决策主体、对象、证据和审计可追溯 |
| 7 | 内容权威 | 每个领域的当前权威、目标权威及迁移状态明确 |
| 8 | 生命周期 | Proposal、Formal Object、Revision 的边界明确 |
| 9 | 租户安全 | 跨域引用不得绕过租户与项目隔离 |
| 10 | 开放事项 | 未裁决内容保留，不擅自推断 |
| 11 | 数据库映射 | 具备逐对象实证审计方法 |
| 12 | 迁移门槛 | E2E、兼容、恢复及回滚要求明确 |

### 15.1 本仓逐项核对（2026-11-15，11/12 通过 + 1 项条件通过）

| # | 结论 | 依据 |
|---|---|---|
| 1 | ✅ | §1.1/1.2 权威边界表 + §9 注册表逐对象归属 |
| 2 | ✅ | §2.1 规则3（Document Revision 与 Memory Revision 不共享版本身份）+ §6.2 + §9 Identity Owner 列 |
| 3 | ✅ | §2.2.1 三对象语义分离 + §2.2.3 五行操作表 |
| 4 | ✅ | §3.2 ObjectReference（稳定身份/version_id/权限规则5）+ §3.3 矩阵 |
| 5 | ✅ | §4.1~4.4 四契约字段+规则完整 |
| 6 | ✅ | §4.4 七项统一要求 + 决策不跨域借用 + AI 不得伪造主体 |
| 7 | ⚠️ 条件通过 | 结构明确（矩阵含当前/目标/迁移状态三列），但 **2 处值与审计实证不符**：Analysis Report 当前权威误标 Git（实=借住 kb/PG）、PC 当前权威留空（实=PG 权威+PCGIT 已登记）。修正后达标 |
| 8 | ✅ | §6.2 五概念区分 + §6.3 生命周期矩阵 |
| 9 | ✅ | §7 七层校验 + 禁止行为清单 + Q11 升级规则 |
| 10 | ✅ | §10.1 七条 + §13.1 O-01~O-12 全部保留未推断 |
| 11 | ✅ | §12 方法完备；且本仓已完成同方法实证审计（docs/audit/kb-analysis-pc-2026-11-15/ 9 分册），词汇映射 GAP→MISSING / CONFLICT→MISMATCH / 桩→STUB |
| 12 | ✅ | §12.4 九项前置门槛（E2E/兼容/恢复/回滚全含）+ §14 |

**结论**：结构上 12/12 达标；第 7 项修正 2 处事实值后即全绿，文档可冻结为正式实施输入。冻结前置动作 = 反馈修正 2 处（另建议补 §9 注册表 pc.relationship/pc.source，属完整性建议不阻塞验收）。

> 专家补充说明：此清单用于**设计审查**，不代表数据库实现或 E2E 已经通过。

## 16. 最终架构结论

1. **四域独立**：KB、Analysis、PC、Project Cognition 分别拥有自己的业务对象及权威边界。
2. **对象身份独立**：跨域引用不改变对象身份和所属领域。
3. **Analysis 三层分离**：Run、Intake Task、Report 是不同业务实体。
4. **报告版本独立**：Report Revision 归 Analysis 管理，正文以 Git 为权威。
5. **知识对象不合并**：ai.project_knowledge、kb.memory、pc.cognition_object 各自独立。
6. **公共契约不等于公共表**：四项契约只解决跨域协作。
7. **人工决策不可代替**：每个领域必须独立完成自己的人工确认。
8. **Git 与 PostgreSQL 分工明确**：正文与业务状态分别管理，不允许长期双主。
9. **安全优先**：跨域引用必须满足租户、项目和领域权限交集。
10. **先业务后迁移**：真实 E2E 成功是大规模结构迁移的前置条件。

### 16.1 下一阶段正式交付物（按序）

| 顺序 | 文档 | 目的 |
|---|---|---|
| 01 | 四域对象注册表与现有数据库映射审计任务书 | 核实现状与目标差异 |
| 02 | Analysis 对象模型详细设计 | 闭合 Run / Intake / Report 关系 |
| 03 | 四项公共业务契约详细规范 | 明确接口和事件表达 |
| 04 | 四域内容权威迁移设计 | 明确 Git / DB 一致性 |
| 05 | E2E 业务链测试方案 | 验证真实业务闭环 |
| 06 | Migration 任务书 | 根据审计结果确定实际数据库变更 |

### 16.2 V1.0 基线的使用边界

本文可作为四域对象注册表、数据库映射审计、Analysis 详细设计及跨域契约设计的上游依据。**但在 O-01~O-12 相关问题完成裁决之前，不得将本文直接转换为完整数据库 Migration 指令。**

---

## 附录 A：本仓审计事实核对结论（2026-11-15，ai-pm 侧）

核对基准：docs/audit/kb-analysis-pc-2026-11-15/（9 分册）+ architecture-ruling-v1.md。

**一致项**：Q1~Q7 裁决全部正确承接（Intake/Run/Report 三分、Report Revision+Git、三知识对象不合并、Finding Contract 不统一表、Conflict 五要件、PCGIT 不可达不得标已实现）；三条不可变更边界全部体现；§8 转换流程与现有代码防线（kb_pc_analysis 只写 draft+proposal、真人门、MCP 无确认工具）吻合。

**与审计事实不符 / 需向专家反馈修正**：
1. **§5.2 矩阵"Analysis Report 当前权威=Git"有误**——审计实证：当前报告正文借住 kb.document_revision.content（PG），报告行仅存裸指针 kb_revision_id（无 FK、唯一样本为 NULL）。应改为"当前=借住 KB（PG）→迁移中"，否则掩盖 P0-2 现状。Q2 裁的是目标态。
2. §5.2 "PC Cognition 当前实现需核实"——审计已核实：PG content 唯一权威、git_commit_sha 恒 NULL（PCGIT 已登记 pc/mod.rs:25-27），可直接填入。

**登记缺口（建议下一版补）**：
3. §9 注册表缺 PC 的 **Cognition Relationship（+Relationship Revision/Relationship Type 字典）** 与 **Source**（跨域来源登记，唯一有真实数据的 PC 表）；KB 侧缺 Source↔Document 的显式关系对象（docproc source_ref）。
4. §4.2 "Discovery Finding 归属 Project Cognition / Discovery 相关业务"表述含糊——审计中 discovery.* 是独立 schema、discovery_finding 有独立生命周期，归属须明确单一领域。

**开放项（§10.1）与审计 P0 的映射**：#3/#4 即审计 P0-1/P0-2 的裁决落点，须按六 Phase 的 P1（真实 E2E）产出证据后冻结；#7 与 ai.project_knowledge 现状（有 version/status/supersedes、无 proposal/approval 实体）对应。

**补充核对（§11.3~§15 尾部）**：
5. §12 审计要求与本次只读审计高度重合，可直接复用：12.1 五类审计对象 = 已有 9 分册覆盖；12.2 七项检查已隐含完成。分类词汇映射：本审计 GAP→MISSING、CONFLICT→MISMATCH、PC conflict/501/git_committed→STUB，其余 MATCH/PARTIAL/UNKNOWN 同义。**§12.1 说"Project Cognition 相关 Proposal/Evidence/Version"——审计实证：ai 域当前没有独立 Proposal/Evidence/Version 实体**（cognition_item 即提案形态、evidence 是 jsonb 列、version 只是整数列），O-11 确为实缺，非文档臆断。
6. O-01~O-12 与既有裁决/审计全部对齐：O-02=裁决 Q2 补充"轮次策略待定"、O-11=四域审计 P0「ProjCognition 确认无主体门」、O-12=Q11 升级规则。无新增冲突。
7. §12.4 九项 Migration 前置门槛与裁决六 Phase P0/P1 完成标准一致；其中"真实业务链路已完成 E2E 验证"即 Q12。
8. **§15 验收标准仅到标题，清单未随文提供**——文档尚不能视为完整冻结态，需专家补齐 §15 后才能作为正式实施输入。
