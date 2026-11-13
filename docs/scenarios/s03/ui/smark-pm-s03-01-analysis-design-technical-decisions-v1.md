# S03 · 分析、设计与技术决策 总体设计 V1.0

- 日期：2026-11-04
- 项目：smark-pm
- 阶段：S03 — Analysis, Design & Technical Decisions
- 上游：S02 需求提出与需求治理
- 下游：S04 实施与开发执行

> 本稿已将用户给出的五项裁决作为正式设计输入，不再把它们当成待讨论选项。
> 本稿区分三类内容：**〔已裁决〕**（用户已明确选择的架构原则）、**〔设计建议〕**（本 V1.0 据此提出的领域模型和业务规则）、**〔待核验〕**（必须检查现有代码、数据库迁移和运行行为后才能确认的事实）。

## 1. 五项架构裁决正式收编

| 编号 | 用户裁决 | 收编结果 |
|---|---|---|
| Q1 | B | 一个技术设计可以关联多个需求，支持多对多 |
| Q2 | B | 每次设计修改产生新的 Revision，历史不可覆盖 |
| Q3 | A | ADR 独立实体，关联需求和技术设计 |
| Q4 | A | AI 负责分析与设计，人类负责正式确认 |
| Q5 | C | 设计批准、必要 ADR 已解决、实施范围与验收标准齐备后才能进入 S04 |

以上五项均为已确认决策。

由此得到五条 S03 不可违反的架构原则：

1. Requirement 与 Technical Design 是两个独立领域对象。
2. Technical Design 与 Requirement 是多对多关系。
3. Technical Design Revision 必须不可变，历史版本必须可追溯。
4. ADR 是独立的架构决策对象，不是设计文档中的普通章节。
5. AI 不能自行批准正式技术设计或替代人类完成技术决策确认。

特别是 Q5：**不能把「设计文档已经写完」等同于「可以开始开发」。** S03 必须建立一个明确的实施准入门禁。

## 2. S03 的业务定位与上下游边界

### 2.1 完整研发治理链

```
S01 项目初始化与知识空间
          │
          ▼
S02 需求提出与需求治理
          │
          │ 已确认 Requirement Revision
          ▼
S03 分析、设计与技术决策
          │
          ├── Technical Analysis
          │
          ├── Technical Design
          │      └── Design Revision
          │
          ├── ADR
          │      └── Decision History
          │
          ├── Design Review
          │
          └── Implementation Baseline
                    │
                    ▼
              S04 实施与开发
                    │
                    ├── Task
                    ├── Agent Run
                    ├── Code Changes
                    └── Delivery
```

责任分界：

| 对象 | 回答的问题 | 所属阶段 |
|---|---|---|
| Requirement | 系统需要实现什么？ | S02 |
| Technical Analysis | 当前问题、约束和影响是什么？ | S03 |
| Technical Design | 具体采用什么实现方案？ | S03 |
| ADR | 为什么选择这个技术方案？ | S03 |
| Implementation Baseline | 本次实施究竟以什么为准？ | S03 |
| Task | 具体要完成什么工作？ | S04 |

S03 不负责实际完成代码实施，也不负责把 AI 设计稿直接转换成已经批准的开发任务。

### 2.2 S02 与 S03 的边界

继承 S02 的正式基线：

- Requirement 是独立业务实体，不依附于 Markdown 文档、Task 或 Discovery Proposal。
- Requirement Revision 是需求版本。
- 非功能需求与功能需求共同纳入 Requirement，通过类型区分。
- 需求正文的唯一权威是 Markdown + Git。
- PostgreSQL 负责业务 ID、版本引用、审批、状态、审计和事件。
- PageIndex 是可重建的派生索引。
- 人类正式确认 Requirement Revision，AI 不得自动确认。
- 首阶段不额外创建项目级 Requirement Baseline 实体，而以各 Requirement 的 `current_revision_id` 集合表达当前有效需求集合。

因此，S03 不应重新发明一套需求管理机制。**S03 只引用已经正式确认的 Requirement Revision。**

特别重要：设计引用的必须是具体需求修订，而不能只引用 Requirement ID。

```
REQ-001
  ├── Revision 1
  ├── Revision 2
  └── Revision 3 ← 当前正式确认版本

TD-001 Revision 1
  └── REQ-001 Revision 2

TD-001 Revision 2
  └── REQ-001 Revision 3
```

这样才能回答：

- 最初的设计依据是什么？
- 需求后来发生了什么变化？
- 哪一个设计版本响应了哪一个需求版本？
- 已经批准的技术设计是否仍然覆盖当前有效需求？

## 3. S03 核心领域模型

建议 S03 采用五个核心领域对象：

```
1. Technical Analysis
   技术分析对象：记录问题、现状、约束、风险与分析结论。

2. Technical Design
   技术设计对象：承载一个或多个需求的具体实现设计。
   拥有独立且不可覆盖的 Design Revision。

3. ADR — Architecture Decision Record
   独立技术决策对象：记录决策问题、备选方案、最终选择和理由。

4. Design Review
   设计评审对象：承载专家意见、技术核验、问题、裁决和复审记录。

5. Implementation Baseline
   实施基线：冻结进入 S04 的需求版本、设计版本、决策版本、实施范围及验收条件。
```

> 注意：五个对象是建议的领域模型，不代表现有数据库已经存在这些表。

### 3.1 Technical Analysis — 技术分析

技术分析的主要任务是理解需求和现有系统，而不是直接给出最终设计。

| 分析内容 | 说明 |
|---|---|
| Problem Statement | 要解决的问题 |
| Requirement Scope | 关联的需求及具体 Revision |
| Existing Architecture | 现有架构 |
| Code Impact | 受影响的代码模块 |
| Data Impact | 数据库、迁移、读写模型影响 |
| API Impact | REST、MCP、内部服务接口影响 |
| Dependencies | 内外部依赖 |
| Risks | 技术风险 |
| Constraints | 技术约束 |
| Open Questions | 尚未解决的问题 |

技术分析可以由 AI 生成、修改和补充。但它不是技术设计批准，也不具有直接实施授权。

### 3.2 Technical Design — 技术设计

两层模型：

```
TechnicalDesign
    │
    ├── DesignRevision 1
    ├── DesignRevision 2
    └── DesignRevision 3
```

TechnicalDesign 是稳定身份；DesignRevision 是不可变的设计内容版本。

每个 Revision 应具有：唯一 ID、所属 Design ID、版本号、正文 Git 引用、内容 Hash、创建人、创建时间、审批人、审批时间、评审结果、所引用的 Requirement Revision 集合。

每次修改正式设计正文，都必须创建新的 Revision。禁止直接覆盖已批准的 Revision。**设计修改与设计审批是两件事。** AI 可以提交 Design Revision，但只有授权的人类可以正式批准。

### 3.3 ADR — 独立架构决策实体

ADR 不应只是 `TechnicalDesign.md` 的「第八章：技术决策」，而应该拥有独立身份：

```
ADR-001
  ├── Decision Question
  ├── Context
  ├── Options
  ├── Chosen Option
  ├── Rationale
  ├── Consequences
  ├── Related Requirements
  └── Related Technical Designs
```

为什么必须独立？因为一个技术决策可能：影响多个技术设计；关联多个需求；被其他设计重复引用；在未来被新的决策替代；在某个时间点失效或需要重新评估。

```
ADR-001
   采用 PostgreSQL 作为业务状态权威
       │
       ├── TD-001 需求治理设计
       ├── TD-002 技术决策设计
       └── TD-003 任务执行设计
```

ADR 必须可以独立检索、审计和追溯。

**ADR 生命周期建议**

| 状态 | 含义 |
|---|---|
| proposed | AI 或人类提出 |
| under_review | 等待评审 |
| accepted | 人类正式接受 |
| rejected | 人类否决 |
| superseded | 被新决策取代 |
| deprecated | 已废弃，不再适用 |

建议进一步采用不可变决策记录：已经接受的 ADR 不直接修改其历史内容。决策改变时，创建新决策并明确替代关系。

> ADR 的具体 Revision 机制和状态转换将在 S03-04 专项裁定，不在本稿中假定现有系统已经实现。

### 3.4 五个领域对象的关联关系

![S03 领域模型关联图](.docs/scenarios/s03/figures/s03-domain-model.png)

（mermaid 源图：`TECHNICAL_ANALYSIS` informs `TECHNICAL_DESIGN`；`TECHNICAL_DESIGN` has `DESIGN_REVISION`；`REQUIREMENT` has `REQUIREMENT_REVISION`；`ADR` has `ADR_REVISION`；`DESIGN_REVISION` reviewed_by `DESIGN_REVIEW`；`DESIGN_REVISION` frozen_into `IMPLEMENTATION_BASELINE`；`DESIGN_REVISION` covers `DESIGN_REQUIREMENT`；`REQUIREMENT_REVISION` referenced_by `DESIGN_REQUIREMENT`；`ADR_REVISION` affects `DESIGN_REVISION`；`ADR_REVISION` referenced_by `ADR_DESIGN_REVISION`；`ADR_REVISION` relates `ADR_REQUIREMENT`。）

关键设计约束：

**第一，需求关联必须绑定需求版本。** 不能只保存 `design_id → requirement_id`，而应当保存：

```
design_revision_id
requirement_id
requirement_revision_id
```

这样设计版本与需求版本之间形成明确的追溯关系。

**第二，ADR 必须能够关联具体设计版本。** 一个 ADR 可以被多个设计引用，但应该明确引用的是哪个 Design Revision，而不是模糊地关联整个设计。

**第三，关联关系本身也需要审计。** 不能只记录设计正文的修改，还要记录需求关联、决策关联和范围变化。

> 以上是推荐的数据关系，具体表名、外键和约束须经过现有数据库核验后才能冻结。

## 4. 版本与生命周期设计

### 4.1 Technical Design 生命周期

```
draft
  │
  ▼
analysis_ready
  │
  ▼
under_review
  │
  ├── revision_requested
  │       │
  │       ▼
  │   新 Design Revision
  │       │
  │       └── under_review
  │
  ▼
approved
  │
  ▼
superseded
```

另设 `rejected` 作为评审否决的终态。

需要区分两个维度：**Design**（整个设计对象的生命周期）与 **DesignRevision**（每个具体设计版本的生命周期）。

例如 TD-001 当前已经批准 Revision 2，但 Revision 3 正在修改——这时不能因为 Revision 3 处于草稿状态，就让已经批准的 Revision 2 失效。

| 设计对象 | 当前版本 | 状态 |
|---|---|---|
| TD-001 | Revision 2 | 已批准 |
| TD-001 | Revision 3 | 修改中 |

因此，Revision 的状态不能简单地等同于 Design 的状态。

### 4.2 Revision 不可变原则

设计版本一旦产生，正文不得原地覆盖。

```
TD-001
  ├── R1 (Approved)
  ├── R2 (Approved)
  └── R3 (Draft)
```

当 R3 被批准：

```
TD-001
  ├── R1 (Superseded)
  ├── R2 (Superseded)
  └── R3 (Approved)
```

这里是同一设计内部的版本关系。其他设计引用 R2 时，不能因为 R3 批准，就自动改变其历史引用。

### 4.3 设计修订的触发条件

以下情况必须创建新 Revision：

1. 实现架构发生变化。
2. 数据库模型发生变化。
3. API 或 MCP 接口发生变化。
4. 技术方案发生变化。
5. 设计覆盖的需求范围发生变化。
6. 验收标准发生实质变化。
7. ADR 决策变化导致设计需要调整。

纯粹的拼写、格式修正是否需要产生 Revision，可以在后续设计中定义文档修订策略，但绝不能借此覆盖已经批准的技术内容。

## 5. 人类与 AI 的职责和权限

Q4 已经确定：AI 分析和设计，人类确认。

| 操作 | AI Agent | 人类 |
|---|---|---|
| 技术分析 | 允许 | 允许 |
| 生成技术设计 | 允许 | 允许 |
| 提交设计评审 | 允许 | 允许 |
| 正式批准技术设计 | 禁止 | 允许 |
| 提出 ADR | 允许 | 允许 |
| 正式接受 ADR | 禁止 | 允许 |
| 创建新 Revision | 允许 | 允许 |
| 正式冻结实施基线 | 禁止 | 允许 |

重要区别：**AI 可以生成一个看起来完整、质量门全部通过的设计稿，但这不等于设计已经获得批准。质量门与人类正式确认是两种独立机制。**

### 5.1 权限控制建议

沿用 S02 已经确立的权限治理方式：

- 项目成员权限。
- 项目角色权限。
- 操作级权限码。
- Application Service 统一写入。
- REST 和 MCP 共用业务规则。
- 正式确认必须经过人类身份校验。
- 审批行为必须记录审计事件。

不应该出现：

```
REST：必须人类审批
MCP：AI 可以自动审批
```

也不应该出现：

```
AI：通过 AutoApply 自动批准
人类：通过独立接口批准
```

两条路径必须经过同一个业务规则和授权边界。

> 这一点要特别注意 S02 已经暴露的双入口语义分叉、跨项目越权和并发问题。S03 不应该重新制造这些缺陷。

## 6. 实施准入门禁（Implementation Readiness Gate）

Q5 的正式收编。建议 S03 建立一个独立的实施准入检查服务。

### 6.1 五项准入条件

```
Implementation Readiness Gate
1. 需求依据已确认
   所有关联需求都必须引用有效的正式 Requirement Revision。
2. 技术设计已批准
   本次实施所依据的 Design Revision 必须获得人类正式批准。
3. 必要 ADR 已解决
   所有标记为实施阻塞项的技术决策必须处于已接受或其他明确允许的终态。
4. 实施范围明确
   明确本次实施包含什么、不包含什么、涉及哪些模块、接口和数据结构。
5. 验收标准齐备
   必须有可执行的验收条件、验证方法和必要的证据要求。

最终结果：Ready for S04
```

五项全部满足，才允许正式生成或批准实施基线。

### 6.2 不允许的行为

以下情况必须阻止进入 S04：

- 需求 Revision 已经被新的版本取代，但设计仍引用旧版本且未完成影响评估。
- 技术设计尚未获得人类批准。
- 关键 ADR 仍处于待决状态。
- 设计范围不清楚。
- 验收标准只有自然语言描述，没有可验证条件。
- 设计依赖的外部条件尚未确认。

但要注意：**不是所有 ADR 都必须在实施前解决。** 例如一项 P2 的性能优化决策可以明确推迟；而数据库主键策略这样的关键决策可能直接阻塞实施。

因此，每个 ADR 应具备：

- 是否阻塞实施。
- 影响范围。
- 解决期限或阶段。
- 对应的实施基线。

### 6.3 Implementation Baseline 的定位

S02 首阶段不建立独立 Requirement Baseline 实体，而是通过需求当前修订指针集合表达当前需求状态。但 S03 的实施基线有不同目的：**Implementation Baseline 是一次实施授权所依据的不可变快照。**

| 内容 | 固定依据 |
|---|---|
| Requirements | Requirement ID + Revision ID |
| Technical Designs | Design ID + Revision ID |
| ADRs | ADR ID + Decision Revision ID |
| Scope | 本次实施范围 |
| Acceptance | 验收标准版本 |
| Dependencies | 已确认依赖 |
| Approval | 人类批准人及时间 |
| Repository Context | 实施所依据的仓库及基准版本 |

为什么必须冻结？假设 TD-001 Revision 2 已批准，S04 据此生成 TASK-100；后来 TD-001 Revision 3 又被批准——TASK-100 不应该悄悄切换到 Revision 3。它仍然以自己的 Implementation Baseline 为准。如果 Revision 3 改变了实施要求，就应该走影响分析，决定是否需要新的实施基线及任务调整。

这与 S02 已经确定的 Task 对同一 Requirement 默认采用单一执行依据的原则保持一致。

## 7. S03 六个实施切片

| 切片 | 名称 | 主要交付 |
|---|---|---|
| S03-01 | 需求技术分析 | 技术分析模型、影响分析、技术问题 |
| S03-02 | 架构与代码分析 | 现有架构、代码资产、依赖和复用分析 |
| S03-03 | 技术方案设计 | Technical Design、Revision、设计关联 |
| S03-04 | ADR 技术决策 | ADR 独立实体、决策生命周期 |
| S03-05 | 设计评审与批准 | Review、问题修订、正式审批 |
| S03-06 | 实施基线与交接 | Readiness Gate、Implementation Baseline、S04 交接 |

**S03-01 · 需求技术分析**：如何把已确认需求转化为技术问题；如何识别需求之间的冲突；如何识别技术约束；如何分析需求变化的影响；如何管理待澄清问题。

**S03-02 · 架构与代码分析**：现有代码是否支持；是否存在可复用模块；数据库是否需要迁移；是否影响已有 API、MCP；是否影响现有任务和执行模型；如何引用已有架构决策。这里需要继承 S01、S02 的现状分析能力，而不是让 AI 每次都从零开始分析仓库。

**S03-03 · 技术方案设计**：技术设计的创建；一个设计关联多个需求；设计 Revision；设计文档版本管理；需求与设计的双向追溯。

**S03-04 · ADR 技术决策**：ADR 创建；备选方案比较；人类正式决策；ADR 与 Requirement 的关系；ADR 与 Technical Design 的关系；决策被替代后的历史追溯。

**S03-05 · 设计评审与批准**：AI 自检；专家评审；代码事实核验；问题登记；用户裁决；Revision 修订；人类正式批准。这里应沿用 S01、S02 已形成的评审方式，不能另外建立一套孤立的专家工作流。

**S03-06 · 实施基线与交接**：Readiness Gate；实施范围冻结；验收标准冻结；设计和 ADR 版本锁定；Repository 基准版本锁定；S04 Task 创建依据；基线变更与重新批准。

## 8. S03 的页面与 UI 结构建议

不建议把六个切片全部变成六个一级菜单。S03 是一套设计治理工作流，不是六个互相独立的业务系统。

建议在项目工作空间下采用四个主要页面：

![S03 工作空间页面结构](.docs/scenarios/s03/figures/s03-workspace-ui.png)

另外，AI 上下文不应该在每个页面重复建立一套。建议使用项目级统一上下文，页面根据当前业务对象追加局部上下文：

```
Project Context
    ├── Repository Context
    ├── Architecture Knowledge
    ├── Confirmed Requirements
    ├── Existing ADRs
    └── Current Design

Local Context
    ├── Current Design Revision
    ├── Related Requirements
    ├── Review Findings
    └── Pending Decisions
```

这样既能保持一致性，也不会让每个 Agent 都重复加载整个项目。

## 9. 现有代码事实核验清单

目前能取得的 S02 代码盘点材料，其核验基线是：

```
worktree: D:\kudi-project\templates\ai-pm-s01
branch: feat/s01-implementation
HEAD: 807ddae
```

这是 S02-01 V1.1 所使用的历史代码核验基线，不代表 2026-11-04 当前工作区的最新 HEAD。因此，以下把已经有历史源码证据的内容与必须重新核验的内容分开，避免把旧代码事实误认为 S02 实施完成后的现状（S02-01-需求领域资产盘点-V1.1.md）。

### 9.1 第一组：需求领域及 S02 接口

**已有历史代码证据**

| 核验 ID | 核验对象 | 历史代码位置 | S03 核验重点 |
|---|---|---|---|
| VF-01 | Requirement 主表 | 0002_delivery.sql | 确认当前实体结构和稳定 ID |
| VF-02 | Requirement Revision | 0002_delivery.sql | 确认版本、正文引用、状态和审批字段 |
| VF-03 | 需求当前版本指针 | `delivery.requirement.current_revision_id` | 确认当前有效版本语义 |
| VF-04 | 需求正文权威 | Markdown + Git / PG | 确认 S02 最终实施结果 |
| VF-05 | 需求与 Task 关系 | `delivery.task_requirement` | 确认 Revision 级追溯 |
| VF-06 | 需求正式确认 | S02-I6-A | 确认人类确认权限是否真正强制 |
| VF-07 | 需求读写服务 | `pm_write/`、REST、MCP | 确认是否统一走 Application Service |

尤其注意 VF-04：早期 S02-01 V1.1 记录的是 `requirement_revision.content` JSONB 作为正文权威。但后续 S02 正式裁决已经将 Markdown + Git 确立为正文唯一权威，PG 负责 ID、版本引用、审批、状态、审计和事件。**S03 必须以最终 S02 基线为准，而不能直接照搬旧盘点中的 JSONB 正文方案。**

### 9.2 第二组：技术设计现有资产（必须重点核验）

| 核验 ID | 核验对象 | 需要回答的问题 |
|---|---|---|
| VF-08 | `pm_submit_technical_design` | 是否已经有技术设计提交入口？ |
| VF-09 | Design 文档类型 | 是否存在 `doc_type='technical_design'`？ |
| VF-10 | 技术设计正文 | 正文是否已经采用 Markdown + Git？ |
| VF-11 | 设计 Revision | 是否有独立版本身份和历史记录？ |
| VF-12 | 设计批准 | 是否存在独立的人类正式审批机制？ |
| VF-13 | 需求关联 | 是否存在 Design ↔ Requirement 关联表？ |
| VF-14 | 版本追溯 | 设计是否关联具体 Requirement Revision？ |
| VF-15 | 设计历史 | 是否可以查询已批准及已废弃版本？ |

`pm_submit_technical_design` 值得特别关注：从历史 S02 资产盘点可以确认，MCP 工具清单中已经存在这个工具，但不能仅凭工具名称就判断它已经具备 S03 所需的完整设计生命周期。需要检查：工具输入 Schema；对应的 Application Service；数据库写入位置；文档存储方式；Revision 机制；人类审批机制；与 Requirement 的关联；与 Task、Run、Delivery 的关系。

不要重复建设一个现有工具已经具备的能力，也不要把一个仅能提交文档的工具误认为完整的技术设计治理系统。

### 9.3 第三组：ADR 与技术决策（必须重点核验）

| 核验 ID | 核验对象 | 历史已知情况 | 核验目标 |
|---|---|---|---|
| VF-16 | `ai.project_document` | 存在 `doc_type='decision'` | 确认 ADR 是否仅为普通知识文档 |
| VF-17 | `ai.document_proposal` | 已有文档发布管线 | 确认 ADR 是否通过该管线发布 |
| VF-18 | `governance.approval_request` | 已有治理审批对象 | 确认能否复用审批机制 |
| VF-19 | `ai.change_proposal` | 已有变更提案 | 确认决策审批与普通变更提案的边界 |
| VF-20 | Requirement ↔ Decision | S02-01 历史全库扫描未发现关联 | 确认 S02 后续是否已经补入关联 |
| VF-21 | Design ↔ Decision | 历史材料未证明存在 | 确认是否已有具体设计版本追溯 |

这里需要区分两个东西：

- `doc_type='decision'`：知识文档分类。
- ADR：具有独立 ID、生命周期、决策责任和追溯关系的架构决策实体。

不能仅仅因为系统已经有 decision 类型文档，就认为 ADR 已经实现。

### 9.4 第四组：评审、权限与审计

| 核验 ID | 核验对象 | 核验重点 |
|---|---|---|
| VF-22 | Design Review | 是否有独立评审记录及问题生命周期 |
| VF-23 | Revision Review | 评审意见是否绑定具体设计 Revision |
| VF-24 | Human Approval | 是否真正限制 AI 自动批准 |
| VF-25 | Role Permission | 是否有设计批准、ADR 接受等权限 |
| VF-26 | Audit | 是否记录提交、评审、批准、拒绝和撤回 |
| VF-27 | Optimistic Concurrency | 是否存在并发双批准、版本覆盖 |
| VF-28 | REST/MCP | 两个入口是否共享业务规则 |

尤其要核验现有的 `ai.document_proposal` 八态管线：

```
draft
  → quality_gates_passed
  → redline_pending
  → confirmed
  → commit_pending
  → committed
```

以及 `rejected`、`rolled_back`。

**不能直接把它当作 Design Revision 的生命周期。** 文档发布成功，不等于技术设计批准；Git Commit 成功，也不等于技术决策正式接受。

### 9.5 第五组：S04 交接与实施依据

| 核验 ID | 核验对象 | 核验重点 |
|---|---|---|
| VF-29 | Task 创建 | 是否支持从设计基线生成任务 |
| VF-30 | Task Requirement | 是否保留具体需求 Revision |
| VF-31 | Run | 是否可以追溯实施基线 |
| VF-32 | Delivery Manifest | 是否能追溯设计和需求依据 |
| VF-33 | Repository Baseline | 能否冻结仓库基准版本 |
| VF-34 | Acceptance | 是否能从设计验收条件生成实施验收依据 |
| VF-35 | Design Change Impact | 设计修改后能否找到受影响任务和执行记录 |

这一组是 S03 最终能否真正交接 S04 的关键。

尤其需要关注已有的：Task、Run、Delivery Manifest、Repository、Acceptance。**不要在 S03 新建一套与这些对象平行的实施管理系统。**

## 10. 代码核验的具体执行要求

不建议直接让技术人员开始创建新表。先完成一次独立的 S03 现状调查。

### 10.1 核验目录

建议让实施方按以下目录提交报告：

```
docs/scenarios/s03/
  S03-01-current-state-inventory.md
  S03-02-domain-model-analysis.md
  S03-03-design-lifecycle-analysis.md
  S03-04-adr-analysis.md
  S03-05-review-and-approval-analysis.md
  S03-06-implementation-baseline-analysis.md
  S03-code-facts-report.md
```

### 10.2 每项核验必须提供的证据

不能只给出「技术设计已经实现。」必须提供：

| 证据类型 | 要求 |
|---|---|
| Migration | 文件名、表名、字段、约束 |
| Rust | 具体模块、结构体、方法 |
| REST | 路由和 Handler |
| MCP | 工具名称、Schema、调用路径 |
| Frontend | 页面、API 方法、交互 |
| Tests | 单元测试、集成测试 |
| Runtime | 必要的真实运行结果 |

所有核验结论必须标注：

- `[已实现]`
- `[部分实现]`
- `[仅设计]`
- `[不存在]`
- `[存在缺陷]`
- `[仅静态]`
- `[已实跑]`

### 10.3 必须验证的关键场景

至少跑下面八个场景：

1. AI 创建技术设计 Revision，人类批准。
2. 已批准设计修改，产生新 Revision，旧版本仍可查询。
3. 一个设计关联三个需求，需求分别使用不同 Revision。
4. 一个 ADR 关联多个需求和多个设计。
5. AI 尝试正式批准设计，系统拒绝。
6. 两个用户并发批准不同设计 Revision，不能产生错误的当前版本。
7. 关键 ADR 未解决，实施准入检查失败。
8. 设计批准后发生修改，原有 Implementation Baseline 不得被静默改变。

第 8 项尤其重要，它将决定 S03 和 S04 是否真正实现版本隔离。

## 11. 需要在 S03 后续切片裁决的事项

五项架构裁决已经解决总体方向，但还不能把所有数据库细节都冻结。保留以下专项问题：

| 编号 | 问题 | 计划归属 |
|---|---|---|
| D-01 | Technical Analysis 是否也需要独立 Revision？ | S03-01 |
| D-02 | Technical Design 与 Requirement Revision 的关联是否必须绑定具体版本？ | S03-03 |
| D-03 | ADR 是否拥有独立 Revision，以及已接受决策如何修订？ | S03-04 |
| D-04 | 一个 ADR 是否可以被多个设计 Revision 引用？ | S03-04 |
| D-05 | Review 是否需要独立实体，还是沿用现有评审机制？ | S03-05 |
| D-06 | 设计批准和 ADR 接受是否使用不同权限码？ | S03-05 |
| D-07 | 一个 Implementation Baseline 是否允许包含多个 Technical Design？ | S03-06 |
| D-08 | 设计变更如何识别受影响的 Task、Run 和 Delivery？ | S03-06 |
| D-09 | Implementation Baseline 与 Repository Commit 如何绑定？ | S03-06 |
| D-10 | 多个设计共同覆盖同一需求时，如何判断覆盖完整性？ | S03-03 / S03-06 |

这些问题不需要现在全部回答。应在对应切片结合代码事实和具体场景进行裁决。

## 12. S03 总体实施推进计划

```
S03 Delivery Roadmap
1. 阶段 A：现状核验（下一步）
   完成 VF-01 至 VF-35，确定哪些能力可以复用、哪些需要扩展、哪些需要新建。
2. 阶段 B：场景与总体基线
   编写 S03 用户场景、领域模型、生命周期、权限矩阵及接口边界，形成 S03 最终基线。
3. 阶段 C：六个切片设计
   S03-01 至 S03-06 逐个形成设计稿，完成专家评审、代码核验和用户裁决。
4. 阶段 D：实施计划
   将批准后的切片转换为实施 TASK，明确迁移、API、MCP、前端、测试和依赖。
5. 阶段 E：S03 验收与交接
   验证设计 Revision、ADR、Review、Readiness Gate、Implementation Baseline 全链路，正式交接 S04。
```

### 12.1 S03 完成标准

S03 不能以「六份设计文档已经写完」作为完成标准。必须至少满足：

- [ ] Requirement → Design Revision 可追溯。
- [ ] Design Revision 不可覆盖，历史完整。
- [ ] 一个设计支持关联多个需求。
- [ ] ADR 具有独立身份和生命周期。
- [ ] ADR 可以关联需求和设计版本。
- [ ] AI 无法绕过人类正式批准。
- [ ] 评审、审批和修订过程有完整审计。
- [ ] Implementation Readiness Gate 可执行。
- [ ] Implementation Baseline 可以冻结完整实施依据。
- [ ] S04 能够基于该基线创建和执行任务。
- [ ] 设计变更不会静默改变历史实施依据。

### 12.2 特别需要保护的现有架构

四个不应该在 S03 中重复建设的东西：

1. 不新建第二套 Requirement。
2. 不把 Technical Design 塞进 `ai.document_proposal`，使其与知识文档发布生命周期混为一谈。
3. 不重复建设 Task、Run、Delivery 管理系统。
4. 不把 ADR 当作普通 Markdown 文档，也不把人类审批等同于 AI 质量门通过。

另外，**数据库 Migration 编号必须在最新代码和迁移登记表核对后分配**。历史盘点提到的 0029、0031、0032 等不能直接作为 S03 的新迁移编号。

## 13. 本次设计结论

**S03 总体设计 V1.0 — 五项裁决已收编。**

正式确定的架构方向：

- Requirement 与 Technical Design 分离。
- Technical Design 多对多关联 Requirement。
- 设计采用不可覆盖的 Revision。
- ADR 是独立领域实体。
- AI 分析与设计，人类正式确认。
- Implementation Baseline 必须通过五项准入条件。

当前状态：总体设计已形成，尚未完成代码事实核验，因此不能作为最终实施基线。
