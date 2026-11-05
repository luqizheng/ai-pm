# S01-04 · AI-PM Cognition UX 详细设计稿 V1.1

> **所属阶段**：S01 — Project Initialization & Management Foundation
> **切片编号**：S01-04
> **切片名称**：AI-PM Cognition
> **文档性质**：UX 产品行为与交互规则设计
> **版本**：V1.1
> **状态**：用户裁决已收编，待设计冻结
> **前置切片**：S01-01、S01-02、S01-03
> **关联阶段**：S02 Requirement Governance、S03 Analysis & Design
> **管理模式**：人类负责项目，AI Agent 协助执行

> **⚠️ 2026-11-05 修订稿到达（已记录，未收编，待基线确认）**：专家《S01-04 AI-PM Cognition 详细设计稿 V1.2》见
> [s01-04-ai-pm-cognition-design-revision-v1-2.md](s01-04-ai-pm-cognition-design-revision-v1-2.md)。与本稿相关：① Q1~Q5 裁决映射全一致（本稿 ：56/:241/:380/:438/:493 与 ：891-895 收编表为裁决正文；V1.2 中 Q1 未显式标注，其 §1.2 即 Q1=B 落点）；② V1.2 扩展为长期认知治理（六领域/变化治理/混合 Context/统一认知治理），范围大于本稿与已实施 CognitionPage V1（迁移 0036），演进路径待基线确认；③ V1.2 §15 十项技术核验勿复制 Discovery/Knowledge/Document 三套既有提案状态机（已并入[统一核验清单](s01-technical-verification-unified-checklist-v1.md)）。读本稿时须对照修订记录。

---

## 1. 产品定位

### 1.1 产品目标

AI-PM Cognition 是 Project 级别的长期认知能力。

它帮助 AI Agent 理解一个 Project 的：

- 项目目标和产品定位
- 业务领域及业务概念
- Repository 及其代码结构
- 技术栈及系统架构
- 研发规范和工程约束
- 项目演进历史

系统通过持续分析项目资料建立认知，并由人类确认重要认知，逐步形成可供研发 Agent 使用的正式项目知识。

### 1.2 核心原则

1. AI 可以自主收集和分析，但不能自行决定项目正式事实。
2. AI 的推断不自动成为正式 Knowledge。
3. Knowledge 必须具有来源和确认记录。
4. 已确认 Knowledge 的变化必须通过人工确认。
5. 每次认知分析必须保留独立 Run 和历史结果。
6. Project 认知必须支持多个 Repository。
7. Agent 使用项目认知时，必须遵循上下文授权及范围控制。

### 1.3 产品边界

S01-04 负责项目认知，不负责：

- 正式需求的生命周期管理（S02）。
- 技术决策的完整评审和批准流程（S03）。
- 具体 Task 的执行调度。
- Agent 的代码修改和交付验收。
- 替代项目知识空间的正式发布机制。

---

## 2. 核心对象模型

用户裁决 Q1：B。

采用 Cognition + Knowledge 双对象体系。

### 2.1 Cognition

Cognition 是 AI 对 Project 进行分析后形成的认知结果。

它主要记录：

- AI 识别出的事实。
- AI 推断出的业务概念。
- AI 对架构和模块的理解。
- 技术栈及研发规范分析。
- 认知证据和来源。
- 置信程度。
- 不确定性和冲突。
- 对现有 Knowledge 的变化建议。

Cognition 具有分析属性，不天然具有正式知识效力。

### 2.2 Knowledge

Knowledge 是已经经过人类确认的正式项目知识。

例如：

- 项目的正式产品定位。
- 已确认的业务术语。
- 已确认的架构约束。
- 已确认的模块职责。
- 已确认的研发规范。
- 已确认的 Repository 关系。

Knowledge 应当具有：

| 属性 | 说明 |
| --- | --- |
| Knowledge ID | 唯一标识 |
| Project ID | 所属项目 |
| Category | 业务、架构、技术、研发规范等 |
| Content | 正式知识内容 |
| Source References | 证据来源 |
| Confirmation Record | 确认人和确认时间 |
| Version | 知识版本 |
| Status | 当前有效、已被替代等 |
| Supersedes | 被替代的旧知识（如适用） |

### 2.3 两个对象的关系

Cognition 可以：

1. 提出新的 Knowledge。
2. 提议修改已有 Knowledge。
3. 提议废止已有 Knowledge。
4. 提出多个 Knowledge 之间的关系。
5. 发现 Knowledge 之间的冲突。

但 Cognition 不能直接修改正式 Knowledge。

必须经过人类确认，才能完成正式知识变更。

### 2.4 Cognition Run

每次正式认知分析形成一个独立 Cognition Run。

Run 是一次认知执行的历史记录，不是 Knowledge 本身。

Run 应记录：

- Run ID
- Project ID
- 执行时间
- 执行触发来源
- 分析范围
- Repository 基准版本
- 输入资料
- 使用的分析能力及版本
- 执行状态
- 产生的认知条目
- 失败及异常信息

同一个 Project 可以拥有多个 Cognition Run。

历史 Run 不因重新分析而删除。

---

## 3. 认知领域划分

Project Cognition 分为六个领域。

### 3.1 Project Identity

项目身份与目标：

- Project Purpose
- Product Vision
- Target Users
- Business Domain
- Project Scope
- Key Constraints

主要来源：Project 创建信息、人类输入和正式项目文档。

### 3.2 Repository Understanding

仓库认知：

- Repository Identity
- Technology Stack
- Directory Structure
- Application Entry
- Module Architecture
- Build & Test
- Development Convention

每个 Repository 独立建立认知。

多个 Repository 之间可以建立项目级关联，但不得未经证实推断其职责关系。

### 3.3 Domain Understanding

业务领域认知：

- Domain Concepts
- Business Terminology
- Domain Entities
- Business Rules
- Business Processes
- Entity Relationships

识别业务概念不代表正式定义业务概念。

正式定义需要人类确认。

正式需求及需求状态仍由 S02 管理。

### 3.4 Architecture Understanding

架构认知：

- System Architecture
- Module Boundaries
- Module Dependencies
- Data Flow
- External Dependencies
- Technology Decisions
- Architecture Constraints
- Technical Debt

证据类型必须明确区分代码事实、文档事实和 AI 推断。

### 3.5 Development Convention

研发规范：

- Git Branch Convention
- Commit Convention
- Coding Standards
- Testing Requirements
- API Convention
- Database Migration Convention
- Error Handling
- Security Constraints

已有正式规范优先于 AI 推断。

### 3.6 Project Evolution

项目演进：

- Repository 变化
- 技术栈变化
- 架构变化
- 业务规则变化
- 文档变化
- 已确认知识与新证据之间的差异

只负责发现、分析和提出更新，不自动改变 Knowledge。

---

## 4. 首次认知建立流程

用户裁决 Q2：C。

采用自动准备 + 人工启动正式分析。

### 4.1 阶段一：自动准备

Project 创建后，系统自动开始收集基础资料。

准备内容：

1. Project 基础信息。
2. 已授权 Repository 信息。
3. Repository 基本元数据。
4. 可以访问的 README 和项目文档。
5. 技术栈识别所需的基础配置。
6. 可用的项目知识来源。

此阶段不启动正式的 Cognition Run。

系统产生的是认知准备资料，不是正式认知结论。

### 4.2 阶段二：展示准备结果

向用户展示：

- Project 基础信息是否完整。
- Repository 是否已授权。
- Repository 是否可访问。
- 已收集资料数量。
- 缺少哪些重要资料。
- 哪些资料需要用户补充。

允许用户：

- 补充 Project 描述。
- 添加 Repository。
- 补充项目文档。
- 修正基础信息。
- 查看资料收集结果。

没有 Repository 时，仍然允许进行基础认知分析。

### 4.3 阶段三：人工启动

用户明确启动正式认知分析。

启动之前，系统展示：

- 本次分析范围。
- 将使用的 Repository。
- 已收集资料。
- 当前缺失资料。
- 分析能力说明。

用户确认后创建 Cognition Run。

### 4.4 阶段四：AI 分析

AI 按六个认知领域进行分析。

允许部分领域分析成功、部分领域失败。

不要求所有领域同时完成。

### 4.5 阶段五：产生认知结果

每条认知独立形成 Cognition Item。

每条认知必须包含：

- 认知内容。
- 所属领域。
- 证据来源。
- 证据类型。
- 置信程度。
- 是否存在冲突。
- 对应的 Knowledge 变更建议（如适用）。

### 4.6 阶段六：人类逐项审核

用户对每条认知独立进行处理：

- Confirm：确认并形成 Knowledge。
- Edit & Confirm：修改后确认。
- Reject：拒绝该认知。
- Request Investigation：要求进一步调查。
- Defer：暂缓处理。

确认其中一条，不代表确认其他条目。

---

## 5. Cognition 状态管理

### 5.1 Cognition Run 状态

| 状态 | 含义 |
| --- | --- |
| Pending | 等待执行 |
| Collecting | 收集分析资料 |
| Analyzing | 正在分析 |
| Completed | 分析完成 |
| Partially Completed | 部分分析完成 |
| Failed | 执行失败 |
| Cancelled | 人工取消 |

### 5.2 Cognition Item 状态

| 状态 | 含义 |
| --- | --- |
| Proposed | AI 新提出的认知 |
| Confirmed | 已确认并形成 Knowledge |
| Rejected | 已被人类拒绝 |
| Investigation Required | 需要进一步调查 |
| Deferred | 暂缓处理 |
| Superseded | 已被新认知提案替代 |

Run 的完成状态与 Item 的确认状态相互独立。

例如：

Run 已经 Completed，但仍可能有大量 Proposed 条目。

不能把 Completed 解释成全部认知已经确认。

### 5.3 Knowledge 状态

建议区分：

- Active：当前有效。
- Superseded：已经被新版本替代。
- Retired：已经正式废止。

Knowledge 的确认状态不与 Cognition Run 状态绑定。

---

## 6. 逐项确认机制

用户裁决 Q3：B。

这是本切片的核心交互规则。

### 6.1 确认粒度

每个 Cognition Item 独立确认。

用户可以：

- 确认单条。
- 修改单条后确认。
- 拒绝单条。
- 暂缓单条。
- 要求调查单条。

不要求整个 Run 一次性确认。

### 6.2 批量操作

允许提供批量确认能力，但必须满足：

1. 每条记录独立展示。
2. 每条记录具有独立的证据和内容。
3. 用户明确选择了需要批量处理的条目。
4. 不得默认全选。
5. 不得把存在冲突或重大不确定性的条目混入普通批量确认。

批量操作只是交互效率优化，不改变逐项确认的业务语义。

### 6.3 确认记录

每条确认操作记录：

- 操作人。
- 操作时间。
- 操作类型。
- 确认前内容。
- 确认后内容。
- 对应 Cognition Item。
- 对应 Knowledge 版本。

AI 不具有代替人类确认的权限。

### 6.4 拒绝记录

拒绝的 Cognition Item 必须保留历史。

拒绝不能直接删除证据。

后续再次发现相似认知时，系统应能参考历史拒绝记录，避免重复提出已经明确否定的内容。

但如果出现新的证据，可以重新提出，并明确说明与历史拒绝记录的关系。

---

## 7. Knowledge 更新机制

用户裁决 Q4：C。

所有已确认认知的变化均通过变更提案，由人类决定。

### 7.1 变化类型

AI 可以提出：

| 变化类型 | 说明 |
| --- | --- |
| Add | 新增知识 |
| Modify | 修改已有知识 |
| Retire | 废止已有知识 |
| Relationship Change | 修改知识之间的关系 |
| Conflict | 发现知识冲突 |

### 7.2 变化处理流程

1. AI 发现变化。
2. 生成 Cognition Item。
3. 关联原有 Knowledge。
4. 形成 Knowledge Change Proposal。
5. 展示变化前后内容。
6. 展示证据及影响范围。
7. 人类确认、修改、拒绝或暂缓。
8. 确认后产生新的 Knowledge 版本。
9. 保留旧版本及完整变更记录。

### 7.3 禁止行为

AI 不得：

- 直接覆盖已确认 Knowledge。
- 自动废止已有 Knowledge。
- 自动改变正式项目目标。
- 因为新 Run 成功而自动发布知识。
- 将低置信度推断作为已确认事实。
- 删除已经形成的历史版本。

### 7.4 更新通知

发现变化后，只向用户提示：

- 发现了什么变化。
- 涉及哪些 Knowledge。
- 为什么认为需要更新。
- 有哪些证据。
- 可能影响哪些模块或 Agent。

是否接受变化由人类决定。

---

## 8. Agent Context 使用机制

用户裁决 Q5：C。

采用混合模式。

### 8.1 核心认知默认提供

每个 Project 可以维护一组核心 Knowledge。

例如：

- 项目目标。
- 产品定位。
- 重要架构约束。
- 核心技术栈。
- 全局研发规范。
- 关键业务边界。

这些内容默认进入该 Project 下 Agent 的基础 Context。

核心 Knowledge 的范围需要由人类明确确认，不能由 AI 自行决定。

### 8.2 其他知识按任务检索

非核心 Knowledge 不全部注入 Agent。

系统根据：

- Task 内容。
- Task 类型。
- 关联 Repository。
- 关联模块。
- Agent 职责。
- 当前执行阶段。

动态检索相关知识。

### 8.3 混合 Context 结构

建议采用以下结构：

**Base Context**

Project 核心知识、全局约束、通用研发规范。

**Task Context**

当前 Task 的相关业务知识、模块信息、技术约束。

**Repository Context**

当前执行仓库的结构、代码事实和研发规范。

**Execution Context**

当前 Run、任务执行状态和必要的历史信息。

### 8.4 Context 可追溯性

每次 Agent 执行应能够追溯：

- 使用了哪些 Knowledge。
- 使用了哪些版本。
- 哪些内容来自 Repository。
- 哪些内容属于当前任务输入。
- 是否存在未确认信息。
- Context 生成时间。

未经确认的 Cognition 不得默认作为正式 Knowledge 注入。

如确有必要使用待确认信息，必须明确标注其状态和来源，并遵循相应授权。

### 8.5 多 Agent 规则

一个 Project 可以运行多个 Agent。

多个 Agent 可以使用同一批已确认 Knowledge。

Agent 不得直接修改 Knowledge。

如果 Agent 发现新的业务事实、架构变化或知识冲突，应提交认知提案，由统一的知识管理机制处理。

---

## 9. 多 Repository 认知机制

继承 S01-03 的裁决。

### 9.1 独立认知

每个 Repository 独立识别：

- 技术栈。
- 目录结构。
- 模块职责。
- 构建方式。
- 测试方式。
- 工程规范。

### 9.2 Project 级认知

系统可以在多个 Repository 之间识别：

- 调用关系。
- 服务依赖。
- 共享模块。
- 数据交换关系。
- 前后端关系。
- 共同的业务领域。

这些关系必须具有证据。

不能仅因为多个仓库属于同一 Project，就推断它们之间存在依赖关系。

### 9.3 Primary Repository

允许 Project 暂时没有 Primary Repository。

没有 Primary Repository 时：

- 仍可建立 Project 级 Cognition。
- 各 Repository 独立分析。
- 不将任何 Repository 自动认定为项目主仓库。

### 9.4 Repository 变化

新增、移除或变更 Repository 时：

1. 检测变化。
2. 提示用户。
3. 用户确认后更新 Repository 管理关系。
4. 识别受影响的 Cognition。
5. 生成重新分析建议。
6. 产生新的 Cognition Run。

不直接覆盖原有 Knowledge。

---

## 10. 重新执行与历史管理

### 10.1 支持重复执行

Cognition 可以重复执行，不限制为 Project 初始化的一次性操作。

触发方式：

- 人工主动执行。
- 新增 Repository 后。
- 新增正式项目文档后。
- 重大架构变化后。
- 技术栈变化后。
- 人工提出调查要求。

### 10.2 每次执行保留独立 Run

重新执行时：

- 不删除旧 Run。
- 不覆盖旧认知结果。
- 记录本次输入资料。
- 记录 Repository 基准版本。
- 产生新的认知差异。
- 对比已有 Knowledge。

### 10.3 差异识别

对比结果至少包括：

- 新增认知。
- 已有认知保持一致。
- 内容发生变化。
- 原认知不再成立。
- 新旧认知存在冲突。
- 证据不足，无法判断。

### 10.4 执行失败

Run 失败不得影响：

- 已确认 Knowledge。
- 旧 Run。
- 历史确认记录。
- 已有 Agent Context 的历史追溯能力。

---

## 11. 用户操作权限

| 操作 | AI | 人类 |
| --- | --- | --- |
| 收集 Project 信息 | 执行 | 提供、补充 |
| 收集 Repository 信息 | 执行 | 授权 |
| 启动正式认知 | 不得自行启动 | 决定启动 |
| 分析项目结构 | 执行 | 可要求重做 |
| 识别业务概念 | 提出建议 | 确认、修改 |
| 识别架构约束 | 提出建议 | 确认、修改 |
| 创建 Knowledge | 提出变更 | 确认 |
| 修改 Knowledge | 提出变更 | 确认 |
| 废止 Knowledge | 提出变更 | 确认 |
| 确认认知 | 无权代替 | 负责 |
| 检索 Agent Context | 执行 | 配置核心范围及权限 |
| 更新 Agent Context | 按授权执行 | 管理授权范围 |

---

## 12. 异常场景

| 异常 | 处理原则 |
| --- | --- |
| 没有 Repository | 允许建立基础认知 |
| Repository 无法访问 | 标记缺失，不阻断其他来源 |
| 多仓库信息冲突 | 保留不同来源及冲突 |
| 文档与代码不一致 | 不自动选择其中一方 |
| AI 无法判断业务定义 | 标记待调查 |
| 用户拒绝认知 | 保留拒绝记录 |
| 重新分析失败 | 不影响已有 Knowledge |
| Repository 删除 | 提示受影响知识，不删除历史 |
| Knowledge 已被其他 Run 更新 | 检查版本冲突，要求重新评估 |
| 多 Agent 同时提交认知 | 统一进入认知提案处理流程 |

---

## 13. UX 页面与交互要求

本切片定义页面职责，不冻结具体视觉布局。

### 13.1 Project Cognition Overview

展示：

- Cognition 整体状态。
- 当前有效 Knowledge 数量。
- 待确认认知数量。
- 待处理变更数量。
- Repository 认知覆盖情况。
- 最近一次 Cognition Run。
- 启动或重新执行入口。

### 13.2 Cognition Preparation

展示自动准备结果：

- Project 信息。
- Repository 授权情况。
- 资料收集情况。
- 缺失资料。
- 分析范围。

主要操作：补充资料、调整范围、启动正式分析。

### 13.3 Cognition Runs

展示全部历史 Run：

- Run 时间。
- 执行范围。
- 执行状态。
- 认知条目数量。
- 确认进度。
- 与现有 Knowledge 的变化情况。

### 13.4 Cognition Review

逐条审核认知：

- 认知内容。
- 领域分类。
- 证据来源。
- 置信程度。
- 与已有 Knowledge 的关系。
- AI 分析理由。
- 修改、确认、拒绝、调查和暂缓操作。

### 13.5 Project Knowledge

管理已确认 Knowledge：

- 按领域分类。
- 查看知识详情。
- 查看版本。
- 查看确认历史。
- 查看来源。
- 设置核心 Knowledge。
- 查看被替代或废止的知识。

### 13.6 Knowledge Change Review

展示变更提案：

- 原有内容。
- 新建议内容。
- 差异对比。
- 证据及原因。
- 影响范围。
- 确认、修改、拒绝和暂缓操作。

### 13.7 Agent Context Inspection

展示 Agent 实际使用的项目上下文：

- 默认核心 Knowledge。
- 任务动态检索 Knowledge。
- Repository 相关信息。
- 知识版本。
- 来源及确认状态。

要求能够追溯 Agent 为什么获得这些信息。

---

## 14. 验收标准

S01-04 至少满足以下验收要求。

### A. 认知建立

- [ ] Project 创建后能够自动准备基础资料。
- [ ] 自动准备不等于正式认知分析。
- [ ] 人类可以决定何时启动正式分析。
- [ ] 无 Repository 时仍可建立基础认知。
- [ ] 支持多个 Repository 独立分析。

### B. Cognition 与 Knowledge

- [ ] Cognition 与 Knowledge 是独立管理对象。
- [ ] Cognition 可以提出 Knowledge 变更。
- [ ] 未确认 Cognition 不自动成为正式 Knowledge。
- [ ] Knowledge 有独立版本及来源。
- [ ] Knowledge 可以追溯确认人和确认时间。

### C. 逐项确认

- [ ] 每个认知条目独立确认。
- [ ] 支持修改后确认。
- [ ] 支持拒绝、调查和暂缓。
- [ ] 支持有条件的批量操作。
- [ ] 保留完整确认历史。

### D. 知识更新

- [ ] 所有 Knowledge 变化必须产生变更提案。
- [ ] 不允许 AI 直接覆盖已确认知识。
- [ ] 支持新增、修改、废止及关系变化。
- [ ] 人类可以决定是否接受。
- [ ] 更新后保留旧版本。

### E. Agent Context

- [ ] 支持 Project 核心 Knowledge 默认注入。
- [ ] 支持 Task 相关 Knowledge 动态检索。
- [ ] 支持 Repository 级上下文。
- [ ] 支持多 Agent 共享已确认 Knowledge。
- [ ] Agent 不得直接修改 Knowledge。
- [ ] 能够追溯实际使用的 Knowledge 版本。

### F. 历史与可靠性

- [ ] Cognition 支持重复执行。
- [ ] 每次执行产生独立 Run。
- [ ] 失败 Run 不影响已确认 Knowledge。
- [ ] 支持新旧认知差异对比。
- [ ] 支持认知冲突处理。
- [ ] 支持多 Agent 认知提案统一协调。

---

## 15. 与其他切片的职责边界

### S01

S01-04 建立 Project 的长期认知能力，为后续项目管理和研发执行提供基础。

### S02

S02 负责正式需求实体和需求治理。

Cognition 可以识别需求相关内容，但不能绕过 S02 的需求确认和治理流程。

### S03

S03 负责正式分析、设计和技术决策。

Cognition 可以识别架构信息，但不能把 AI 的架构推断直接当成正式技术决策。

### Knowledge Space

Knowledge Space 是正式知识的管理与发布体系。

S01-04 负责产生认知结果和知识变更提案，正式 Knowledge 的管理与发布应遵循知识空间既有规则。

不得在两个体系中重复建设正式知识发布流程。

---

## 16. 用户裁决记录

| 裁决编号 | 最终裁决 | 状态 |
| --- | --- | --- |
| Q1 | B：Cognition + Knowledge 双对象 | 已收编 |
| Q2 | C：自动准备 + 人工启动 | 已收编 |
| Q3 | B：逐项确认 | 已收编 |
| Q4 | C：所有变化均产生提案，由人类决定 | 已收编 |
| Q5 | C：核心认知默认注入，其余动态检索 | 已收编 |

补充意见：无。

---

## 17. V1.1 修订说明

相对于 V1.0，本版本完成以下修订：

1. 将 Cognition 与 Knowledge 明确为两个独立管理对象。
2. 增加 Cognition Run 与 Cognition Item 的职责定义。
3. 将首次认知流程调整为自动准备、人工启动。
4. 将认知审核调整为逐项确认。
5. 明确所有 Knowledge 变更必须由人类决定。
6. 增加知识版本及变更提案机制。
7. 将 Agent Context 调整为核心认知默认注入、其余按需检索。
8. 补充多 Agent 共享知识及统一协调机制。
9. 补充异常处理、页面职责及验收标准。
10. 明确与 S02、S03 和 Knowledge Space 的边界。

**最终产品原则：**

AI-PM Cognition 不是一次性的项目扫描工具，而是 Project 的长期认知引擎。

AI 持续理解项目并提出认知，Knowledge 通过人类确认形成正式项目依据，Agent Context 根据具体研发任务提供适当的信息。

人类始终掌握项目事实和知识变更的最终决定权。

---

**文档状态：V1.1 用户裁决收编完成，待评审与设计冻结。**