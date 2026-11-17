# Phase 4 — Product 资产归属演进规划基线 V1.0

**阶段：** Phase 4 — Product Asset Ownership Evolution
**版本：** V1.0
**状态：** Draft — 待确认
**规划日期：** 2026-11-16
**项目：** smark-pm
**前置阶段：** Phase 3 — 认知领域建设
**实施策略：** 与 Phase 3 并行规划，Phase 3 正常执行，不提前启动 Phase 4 实施。

---

# 1. 阶段背景

随着 KB、Product Cognition（PC）、Project Cognition 等认知领域逐步建立，系统需要进一步明确 Product 作为独立业务主体的身份，以及 Product 与 Project、知识资产、需求、设计和规划之间的关系。

当前系统已经形成若干重要的领域边界：

- KB 负责正式知识资产管理。
- Product Cognition 负责产品正式认知。
- Project Cognition 负责项目技术及实施认知。
- Product Plan 属于产品规划与项目管理领域。
- Project 是产品建设、交付和演进的实施载体。

随着这些领域逐步成熟，需要解决一个更基础的问题：

**系统中的资产究竟属于谁？Product、Team、Project 之间是什么关系？不同领域如何引用同一资产，同时保持各自的权威边界？**

Phase 4 正是围绕这一问题展开。

# 2. 阶段目标

建立统一、清晰、可演进的 Product 资产归属与关联模型。

具体目标：

1. 明确 Product 的领域身份及生命周期。
2. 明确 Product 与 Project 的关系。
3. 明确 Product 与 Team 的归属关系。
4. 明确 Product Cognition 与 Project Cognition 的权威边界。
5. 明确知识、文档、需求、设计等资产与 Product 的关联机制。
6. 明确 Product Plan 对产品认知和研发资产的引用方式。
7. 识别现有系统的数据模型与目标模型之间的差异。
8. 制定兼容现有数据的演进和迁移方案。

# 3. 已有架构决策与继承约束

以下内容作为 Phase 4 的既有约束，不在 Phase 4 中无依据地重新定义。

## 3.1 KB 资产归属

继承 P3-KB-01：

- Source、Document、Memory 统一采用 Team 级资产所有权模型。
- Project 是资产的关联和使用上下文，不是独立资产所有者。
- Document 和 Memory 可以关联多个 Project。
- 不建立 Project 独立所有的 KB 资产模型。

Phase 4 需要研究 Product 与 KB 资产之间的关联方式，但不能改变上述所有权裁决。

## 3.2 Product Cognition

继承已有 Product Cognition 裁决：

- Product Cognition 负责产品正式认知。
- 产品定位、目标、业务领域、能力、模块、角色、场景、流程、业务对象和业务规则等属于产品认知范畴。
- Product Cognition 是产品认知的正式权威来源。
- AI 可以提出认知建议，但正式确认由人完成。

## 3.3 Project Cognition

继承已有领域划分：

- Project Cognition 负责项目技术及实施认知。
- 技术架构、技术约定、项目实施相关的技术事实由项目认知领域维护。
- 不将产品认知与项目技术认知合并。

## 3.4 Product Plan

继承 Q-PC-13：

- Product Plan 不属于 Product Cognition 的正式认知对象。
- Product Plan 属于产品规划与项目管理领域。
- Product Plan 引用 Product Cognition 正式认知，形成规划与交付安排。
- 不允许 Product Plan 反向成为产品正式认知的权威来源。

# 4. 核心设计原则

## P4-DP-01：领域权威唯一

每类正式资产必须有明确的权威维护领域。

关联和引用不意味着获得资产修改权。

## P4-DP-02：所有权与使用关系分离

必须区分：

- Asset Ownership：资产所有权。
- Association：资产关联关系。
- Usage Context：资产使用上下文。
- Reference：跨领域引用关系。

不能用 Project 关联关系代替资产所有权。

## P4-DP-03：Product 与 Project 独立

Product 是产品主体，Project 是建设、交付和演进载体。

不得将两者设计为一对一的固定关系。

## P4-DP-04：跨领域引用不复制权威资产

不同领域需要使用同一资产时，优先建立稳定引用关系，而不是复制一份独立资产。

## P4-DP-05：保持历史可追溯

资产关联、归属变更、引用关系变化必须具备必要的历史追踪能力。

## P4-DP-06：兼容既有系统

不得仅为了满足新的 Product 模型而破坏现有 Project、KB、PC、Requirement 和 Design 数据。

## P4-DP-07：控制自动化边界

AI 可以识别、建议和整理资产关联。

涉及正式资产归属、正式认知确认及受保护操作时，遵守对应领域的人工确认和权限规则。

# 5. Phase 4 工作范围

## P4-00：现状审计（Baseline Audit）

目标：

基于当前 master 代码，识别现有 Product、Project、Team、KB、PC、Requirement、Design、Product Plan 的实际实现情况。

审计内容：

- 现有数据库实体及关系。
- 现有 API 和领域服务。
- 现有资产所有权字段。
- 现有跨领域关联关系。
- 现有权限与生命周期机制。
- 现有 UI 和导航入口。
- 已实现能力与既有设计文档之间的差异。

交付物：

- Current Architecture Audit。
- Entity Relationship Inventory。
- Ownership Gap Analysis。
- Phase 4 Risk Register。

要求：

不能以旧设计文档代替代码事实；所有差异必须明确标注。

## P4-01：Product Domain Model

目标：

明确 Product 作为独立业务主体的领域模型。

待研究：

- Product 身份定义。
- Product 唯一标识。
- Product 生命周期。
- Product 创建与归档。
- Product 与 Team 的归属关系。
- Product 基础属性与扩展属性。
- Product 是否存在产品族、产品线等层级关系。

交付物：

- Product Domain Model。
- Product Lifecycle Specification。
- Product Identity Rules。

## P4-02：Product–Project Relationship

目标：

建立 Product 与 Project 的关系模型。

待研究：

- 一个 Product 是否可以关联多个 Project。
- 一个 Project 是否可以同时服务多个 Product。
- 产品建设、维护、重构、升级项目之间的关系。
- Project 与 Product 的主次关系是否需要区分。
- 产品关联变更的权限和历史记录。
- 不关联 Product 的独立 Project 是否允许存在。

交付物：

- Product–Project Relationship Model。
- Relationship Lifecycle。
- Association Rules。

## P4-03：Cognition Ownership Boundary

目标：

明确 Product Cognition 与 Project Cognition 在跨领域场景中的权威边界。

待研究：

- Product Cognition 的归属主体。
- Project Cognition 的归属主体。
- 一个 Product 对应多个 Project Cognition 时如何关联。
- 产品认知与项目技术认知冲突时如何处理。
- 产品认知变更如何影响关联项目。
- 项目实施发现与产品正式认知之间如何反馈。

交付物：

- Cognition Ownership Matrix。
- Cross-domain Reference Rules。
- Conflict Resolution Rules。

## P4-04：Product Asset Association

目标：

明确 Product 与各类资产的关联关系。

研究范围：

- KB Source。
- Document。
- Memory。
- Product Cognition。
- Requirement。
- Design。
- ADR。
- 其他需要纳入产品管理的资产。

需要明确：

- 哪个领域拥有资产。
- Product 是否可以直接关联。
- 是否支持多 Product 关联。
- 是否支持多 Project 关联。
- 是否需要版本化引用。
- 关联关系是否需要审批。
- 关联解除后如何保留历史记录。

交付物：

- Product Asset Association Model。
- Asset Ownership Matrix。
- Association Lifecycle Rules。

## P4-05：Product Plan Integration

目标：

明确 Product Plan 与 Product Cognition、Project、Requirement、Design 的协作关系。

研究内容：

- Product Plan 的归属主体。
- Product Plan 与 Product 的关系。
- Product Plan 如何引用正式产品认知。
- Product Plan 如何形成需求和交付安排。
- Product Plan 与多个 Project 的关系。
- 产品目标变更如何影响规划。
- 规划变更如何反馈产品认知。

交付物：

- Product Plan Domain Boundary。
- Planning Reference Model。
- Planning Change Propagation Rules。

## P4-06：Migration & Compatibility

目标：

制定从现有模型向目标模型演进的方案。

研究内容：

- 现有实体兼容性。
- 现有数据归属问题。
- 历史关联关系保留。
- 标识与引用迁移。
- 数据一致性校验。
- API 兼容策略。
- 分阶段迁移及回滚方案。

交付物：

- Migration Strategy。
- Compatibility Matrix。
- Data Integrity Rules。
- Rollback Strategy。

## P4-07：UX & Navigation Impact

目标：

将新的领域模型映射到产品交互体系。

研究内容：

- Product 管理入口。
- Product Overview。
- Product Cognition 入口。
- Product Assets。
- Product–Project 关联管理。
- Product Plan 入口。
- 跨领域资产引用体验。
- 搜索、Deep Link 和上下文导航。

要求：

不能因为增加 Product 就简单增加一套重复的 Project 页面。

必须先确定用户的工作场景，再设计页面和导航。

交付物：

- Product UX Information Architecture。
- Core User Flows。
- Figma UX/UI Design Prompts。
- Navigation Impact Analysis。

## P4-08：Architecture Review & Freeze

目标：

完成整体架构评审并冻结设计基线。

评审内容：

- 领域边界完整性。
- 实体与关系一致性。
- 资产权威归属。
- 权限与生命周期。
- 跨领域引用。
- 数据迁移风险。
- API 与 UI 兼容性。
- 测试及验收标准。

交付物：

- Phase 4 Architecture Baseline。
- Decision Register。
- Implementation Backlog。
- Acceptance Criteria。
- Implementation Readiness Report。

# 6. Phase 4 非目标

Phase 4 不负责：

1. 重做 KB 的资产归属架构。
2. 合并 Product Cognition 与 Project Cognition。
3. 将 Product Plan 纳入 Product Cognition。
4. 重构 Phase 3 已冻结的全部领域模型。
5. 无必要地重写现有 Project 管理功能。
6. 在架构裁决前实施数据库迁移。
7. 在没有完整 UX 评估前大规模调整导航。
8. 将所有 Team 级资产强制转换成 Product 级资产。

# 7. 与 Phase 3 的协作关系

Phase 3 继续按照当前实施计划执行。

Phase 4 与 Phase 3 并行开展规划工作，但必须遵循以下机制：

- P4-00 先审计当前 master。
- 对 Phase 3 已冻结的架构决策只做依赖分析，不擅自修改。
- 如果发现 Phase 3 的实现与既有裁决不一致，分别登记为 Phase 3 问题和 Phase 4 依赖问题。
- Phase 4 不直接修改 Phase 3 的实施范围。
- 涉及共同领域边界的重大变更，必须单独提交架构决策。

# 8. 阶段门禁

## Gate 1：现状审计完成

要求：

- 代码事实清晰。
- 核心实体及关系完整盘点。
- 关键差距和风险已登记。

## Gate 2：领域模型裁决完成

要求：

- Product 身份明确。
- Product–Project 关系明确。
- 资产所有权明确。
- 领域权威边界明确。

## Gate 3：跨领域关联设计完成

要求：

- Product Asset Association 明确。
- 引用与所有权分离。
- 生命周期及权限规则明确。
- Product Plan 边界明确。

## Gate 4：实施准备完成

要求：

- 迁移策略完成。
- UX 设计完成。
- API 和数据模型完成评审。
- 测试与验收标准完成。
- 所有 P0/P1 架构问题关闭或获得明确处理决定。

只有通过 Gate 4，才允许进入 Phase 4 实施阶段。

# 9. 规划与实施管理规则

1. 所有 P4 切片必须有独立的需求与设计文档。
2. 所有重要架构选择必须形成 Decision Record。
3. 未经人工确认的方案不能作为正式架构基线。
4. 设计文档必须区分已确认、待确认和建议事项。
5. 每个切片必须明确上下游依赖。
6. 不允许以 UI 实现反向决定领域模型。
7. 不允许开发自行改变既有领域权威边界。
8. 所有正式设计变更必须记录原因、影响和兼容策略。

# 10. Phase 4 最终验收标准

Phase 4 设计完成时，必须能够明确回答：

- Product 是什么？
- Product 属于谁？
- Product 与 Team 是什么关系？
- Product 与 Project 是什么关系？
- Product Cognition 属于哪个领域？
- Project Cognition 属于哪个领域？
- KB 资产如何关联 Product？
- Document 和 Memory 是否允许关联多个 Product？
- Requirement、Design、ADR 如何关联 Product？
- Product Plan 如何引用正式认知？
- 跨领域引用如何保证权威唯一？
- 资产归属和关联变化如何审计？
- 如何兼容当前系统？
- 如何实施和回滚？

上述问题必须有正式设计结论，不能只停留在概念描述。

# 11. 当前启动事项

当前仅启动：

**P4-00 — Current Architecture & Ownership Audit**

审计基准：

- 当前 master 分支。
- 当前实际数据库 Schema。
- 当前 API 和领域服务。
- Phase 3 已确认的架构裁决。
- 已有 Product、Project、KB、PC、Requirement、Design、Product Plan 设计资料。

本轮不要求开发修改代码，不要求提交数据库 Migration，不要求实施新的 Product 模块。

首轮输出：

1. 当前领域实体清单。
2. 当前资产归属矩阵。
3. 当前 Product–Project 关联现状。
4. 当前跨领域引用现状。
5. 已有设计与代码实现差异。
6. Phase 4 需要裁决的问题。
7. P4-00 审计结论。

---

**基线状态：Draft — 待人工确认**

本文件定义 Phase 4 的规划范围和工作方法，不代表所有 P4 设计决策已经批准。

最终架构以逐项人工裁决及 Phase 4 Architecture Baseline 为准。
