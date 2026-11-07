# Product Cognition 详细产品需求文档 V1.1 — 02 业务领域边界与权威归属

**文档状态：** 产品裁决整合稿（V1.1 拆分版）
**版本：** V1.1
**日期：** 2026-11-06
**所属产品：** smark-pm / PM-AI
**分册：** 02 / 09
**章节范围：** §3 业务领域边界与权威归属
**完整索引：** [00-index.md](00-index.md)

---

# 3. 业务领域边界与权威归属

## 3.1 Product Cognition 与 S01-04 Project Cognition

**正式裁决 Q-PC-01：采用按内容划分权威归属的方案 A。**

PC 与 S01-04 不整体合并，也不互相取代，而是分别管理产品侧认知和技术侧认知。

| 内容 | 唯一正式权威 |
|---|---|
| 产品定位、产品愿景、长期产品目标 | Product Cognition |
| 业务领域、业务能力 | Product Cognition |
| 功能模块、功能点 | Product Cognition |
| 用户角色、产品场景 | Product Cognition |
| 业务流程、业务对象、业务规则 | Product Cognition |
| 技术架构、技术约定 | S01-04 Project Cognition |
| 仓库结构、技术演进 | S01-04 Project Cognition |
| 原始资料、文档解析、知识提取 | Knowledge Base |

### 3.1.1 现有数据迁移

现有 `ai.project_knowledge` 中已确认的产品侧认知：

- 迁移为 Product Cognition 初始草稿；
- 不因历史上已确认而自动变成 PC 正式认知；
- 保留原始来源、历史版本及迁移记录；
- 按 PC 的正式治理规则完成确认。

迁移完成后，产品侧认知由 PC 唯一维护，不允许 S01-04 与 PC 双重写入。

S01-04 保留技术侧认知，并可以向 PC 提供技术背景、技术约束和证据。

## 3.2 Product Cognition 与 Knowledge Base

**正式裁决 Q-PC-12、既有 PC-G20：两者是并列的独立一级导航能力。**

| 比较维度 | Product Cognition | Knowledge Base |
|---|---|---|
| 核心目标 | 建立正式产品认知 | 管理资料和知识 |
| 管理对象 | 产品认知对象及关系 | 原始资料、解析内容、结构化记忆 |
| 核心职责 | 产品建模、关系、一致性、正式变更 | 资料接收、解析、提取、来源、检索 |
| 权威内容 | 正式产品认知 | 原始资料和经审核的知识记录 |
| AI 主要工作 | 认知分析、影响分析、变更建议 | 文档解析、知识提取、记忆整理 |
| 人工确认 | PC 正式认知确认 | 浅层记忆审核 |

两者共享 Knowledge Coordination，但不共享正式内容的写入权。

### 3.2.1 知识流转

正式流程：

`原始资料 → KB 解析与提取 → 人工审核浅层记忆 → PC 分析 → 认知草稿/变更提议 → PC 治理 → 正式认知`

Knowledge Base 审核通过不等于 Product Cognition 正式确认。

PC 必须独立判断：

- 是否属于产品认知；
- 是否已有对应对象；
- 是新增、修改、补充关系还是存在冲突；
- 是否需要人工确认；
- 适用哪个变更审批级别。

## 3.3 Product Cognition 与 Requirements

**正式裁决 Q-PC-11：Requirements 统一服务负责需求确认门。**

- Requirement 的正文、修订和正式确认由 Requirements 管理。
- PC 维护正式业务规则及其他产品认知。
- PC 提供需求确认前的认知状态检查。
- Requirements 统一服务负责调用检查并决定是否允许需求正式确认。
- PC 不建立独立需求审批入口。

## 3.4 Product Cognition 与 Design / Architecture

- 设计内容由 Design 管理。
- ADR 由 Design / Architecture 管理和确认。
- PC 可以引用 Design、ADR 并建立关联。
- PC 不负责 ADR 正文、状态和审批。
- 设计变化可以触发 PC 影响分析。

## 3.5 Product Cognition 与 Project Goal

**正式裁决 Q-PC-04：采用分层管理。**

| 对象 | 管理内容 | 权威 |
|---|---|---|
| Product Vision | 产品定位、愿景、长期产品目标、产品范围、产品边界及产品级约束 | Product Cognition |
| Project Goal | 当前项目目标、阶段目标、阶段性交付目标 | S01 项目管理领域 |

两者通过支撑关系连接。

规则：

1. Project Goal 不重复定义产品级长期目标。
2. Project Goal 可以随项目阶段调整。
3. Project Goal 不得擅自改变 Product Vision。
4. Product Vision 变更时，AI 分析受影响的 Project Goal，并提出建议。
5. 两者的关联变化保留历史记录。