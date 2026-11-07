# Product Cognition 详细产品需求文档 V1.1 — 03 认知模型与外部引用

**文档状态：** 产品裁决整合稿（V1.1 拆分版）
**版本：** V1.1
**日期：** 2026-11-06
**所属产品：** smark-pm / PM-AI
**分册：** 03 / 09
**章节范围：** §4 多视角认知模型、§5 认知关系模型、§6 Requirement/Design/Decision 关联、§7 Product Vision 与 Project Goal
**完整索引：** [00-index.md](00-index.md)

---

# 4. Product Plan 多视角认知模型

## 4.1 八个产品视角

**正式产品模型：多视角关系模型，不设唯一中心。**

| 视角 | 内容 | 主要认知对象 |
|---|---|---|
| 1. Product Vision | 产品定位、目标、范围、边界 | Vision |
| 2. Business Capabilities | 产品要具备的业务能力 | Business Capability |
| 3. Feature Modules | 功能模块和重要功能点 | Feature Module、Feature |
| 4. Actors & Scenarios | 用户角色及使用场景 | Actor、Scenario |
| 5. Business Processes | 业务流程、状态流转 | Business Process |
| 6. Pages & Interactions | 页面、导航、交互 | Page |
| 7. Domain Objects & Rules | 业务对象、关系、规则 | Domain Object、Business Rule |
| 8. Requirements & Decisions | 需求、设计依据、历史决策 | Requirement Reference、Decision Reference |

八个视角是浏览和分析入口，不代表八套独立数据或八张互不相关的主表。

## 4.2 核心认知对象

### 4.2.1 Product Vision

包括：

- 产品定位；
- 产品愿景；
- 长期产品目标；
- 产品范围；
- 产品边界；
- 产品级约束。

### 4.2.2 Business Capability

描述产品需要具备的业务能力。

例如：

- 项目管理；
- 需求管理；
- 产品认知管理；
- 知识管理；
- 任务执行管理。

业务能力描述“产品能做什么”，不等同于功能模块或具体实现。

### 4.2.3 Feature Module

描述产品的功能模块。

例如：

- 项目初始化；
- 需求管理；
- 产品认知；
- Knowledge Base。

模块用于组织产品功能，但不意味着其内部所有功能点都需要独立建模。

### 4.2.4 Feature

**正式裁决 Q-PC-05：按关联和治理需要决定是否独立建模。**

普通功能点作为 Feature Module 的内部结构。

满足以下任一情况时，可以提升为独立 Feature：

- 需要独立关联 Requirement；
- 需要独立关联 Page；
- 需要独立关联 Scenario；
- 需要独立关联 Process；
- 需要独立追踪、版本管理或治理。

提升时保留原有信息及关联历史。

### 4.2.5 Actor

描述与产品交互的用户角色或外部参与者。

例如：

- 普通成员；
- 项目管理员；
- 团队管理员；
- 外部系统。

Actor 描述产品中的角色，不直接等同于平台用户权限配置。

### 4.2.6 Scenario

描述某个 Actor 在特定目标下如何使用产品。

包含：

- 场景名称；
- 目标；
- 参与角色；
- 前置条件；
- 触发条件；
- 主要交互过程；
- 预期结果；
- 异常路径；
- 关联能力、模块、页面、流程和需求。

### 4.2.7 Business Process

描述业务活动及其顺序、状态和转换。

流程内部节点、状态和转换随所属流程整体版本化与审批，不在 V1.1 中全部提升为独立认知对象。

### 4.2.8 Page

描述产品页面、导航位置、页面职责及主要交互。

页面区域和交互细节原则上属于 Page 内部结构，随 Page 版本管理。

### 4.2.9 Domain Object

描述产品业务领域中的核心对象。

可包括：

- 对象名称和定义；
- 业务职责；
- 属性概念；
- 与其他对象的关系；
- 生命周期；
- 业务约束。

对象属性和生命周期原则上随所属对象整体版本化。

### 4.2.10 Business Rule

描述产品必须遵守的正式业务规则。

例如：

- 某类用户可以执行什么操作；
- 某业务对象在什么条件下允许进入某状态；
- 某业务行为必须满足哪些前置条件。

业务规则必须由授权人员在 PC 中正式确认。

## 4.3 外部领域引用对象

Requirement、Design、ADR 不复制其正文作为 PC 的权威内容，而以引用节点参与 Product Map。

引用节点应包含：

- 外部对象稳定身份；
- 对象类型；
- 所属业务模块；
- 关联建立时的版本；
- 当前版本读取入口；
- 关联状态；
- 影响分析状态。

外部对象的正文和审批状态仍由原业务模块维护。

## 4.4 对象内部结构

以下内容默认属于所属对象内部结构：

- 流程节点、流程状态、状态转换；
- 页面区域、控件和交互细节；
- 业务对象属性、生命周期；
- 普通功能点；
- 简单展示顺序和描述。

只有满足独立关联、独立治理或独立追踪需要时，才提升为独立认知对象。

---

# 5. 认知关系模型

## 5.1 关系类型

Product Cognition 使用统一关系模型表达对象之间的业务联系。

| 关系 | 含义 |
|---|---|
| Supports | 支撑 |
| Realizes | 实现 |
| Triggers | 触发 |
| Involves | 涉及 |
| Displayed On | 展示于 |
| Depends On | 依赖 |
| Governed By | 受规则约束 |
| Derived From | 对象派生自另一个对象 |
| Supersedes | 一个对象取代另一个对象 |
| Conflicts With | 不作为普通关系直接维护，冲突由冲突实体管理 |

## 5.2 关系治理

每条正式关系应支持：

- 来源追溯；
- 关系建立时间；
- 关系确认状态；
- 有效版本；
- 关系变更历史；
- 关系失效或替代记录。

## 5.3 关系与来源的边界

来源追溯应使用 Source Evidence / Source Reference，不使用 Derived From 表示资料来源。

Derived From 只用于表达一个认知对象从另一个认知对象派生。

## 5.4 关系与版本的边界

Supersedes 只表示对象 A 取代对象 B，不用于替代对象自身的版本链或状态管理。

对象版本通过版本序列管理。

## 5.5 关系与冲突的边界

正式冲突由独立冲突记录管理。

冲突不是普通关系增删操作。

如需要在 Product Map 中展示冲突连线，应由冲突记录派生，不允许通过普通关系编辑器直接创建或删除冲突事实。

## 5.6 关系变更审批

- 普通关系增删：L1。
- Governed By 等影响正式规则约束的关系：L2。
- 涉及未解决重大冲突的关系：L3 冻结。

---

# 6. Requirement、Design 与 Decision 关联

## 6.1 Requirement 关联规则

**正式裁决 Q-PC-03：同时记录 Requirement 身份和建立关联时的 Revision。**

每条 PC 与 Requirement 的关联至少保留：

- Requirement 稳定身份；
- Requirement Code；
- 建立关联时的 Revision；
- 关联建立时间；
- 关联来源；
- 当前关联状态。

历史 Revision 是建立关联时的依据，不因 Requirement 后续修订而覆盖。

## 6.2 Requirement 后续修订

当 Requirement 产生新 Revision：

1. PC 能够识别关联需求发生变化。
2. AI 分析新旧 Revision 的差异。
3. 分析是否影响关联的产品能力、模块、场景、流程、业务规则或其他对象。
4. 形成影响分析结果。
5. 有实质影响时生成待处理事项或变更建议。
6. 不自动修改正式认知。
7. 是否更新关联依据及正式认知，按照对应治理机制处理。

Requirement 的新 Revision 不意味着历史关联自动切换到新 Revision。

## 6.3 Design 与 ADR

Design、ADR 作为外部领域引用节点参与产品认知关系图。

关联应能够追踪：

- 关联的 Design / ADR 身份；
- 关联建立时的版本；
- 当前版本；
- 对应的 Requirement；
- 对应的 Product Vision、Capability、Feature 或 Business Rule。

PC 不负责修改 Design 或 ADR 的正式内容。

## 6.4 统一决策记录模型

**正式裁决 Q-PC-02：采用统一决策记录模型，按类别划分业务归属。**

统一模型提供：

- 决策稳定身份；
- 决策类别；
- 决策来源；
- 决策理由；
- 备选方案；
- 决策结果；
- 状态；
- 版本历史；
- 关联对象；
- 决策时间及责任人。

业务归属：

| 决策类型 | 业务归属及确认权 |
|---|---|
| Product Decision | Product Cognition |
| Architecture Decision / ADR | Design / Architecture |
| 其他领域决策 | 对应业务模块 |

统一模型不意味着 PC 拥有所有决策的管理权或审批权。

现有 `ai.project_document.doc_type='decision'` 应纳入统一决策载体梳理，避免多个独立正式决策入口。

历史讨论、被否决方案和未决问题可以保留并关联正式决策，但不自动成为正式决策。

---

# 7. Product Vision 与 Project Goal

## 7.1 Product Vision 内容

- 产品定位；
- 产品愿景；
- 长期产品目标；
- 产品范围；
- 产品边界；
- 产品级约束。

## 7.2 Project Goal 内容

- 当前项目目标；
- 项目阶段目标；
- 阶段性交付目标；
- 项目目标完成状态。

## 7.3 支撑关系

Project Goal 可以关联一个或多个 Product Vision 目标，说明当前项目或阶段如何支撑产品长期目标。

产品愿景变更时，AI 对相关 Project Goal 进行影响分析。

PC 不直接替项目管理领域修改 Project Goal。