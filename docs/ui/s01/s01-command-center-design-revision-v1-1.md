# S01 设计修订 · 第一份：S01-01 Project Command Center 设计修订稿 V1.1（专家供稿记录）

> **转交日期**：2026-11-05。本文件是对用户转交的专家设计修订稿的**忠实记录**（以转交文本为准）。
> 稿面状态（原稿自带）：**产品定位已裁决 · UX 设计修订 · 待基线确认**。
> 原稿自述：当前没有检索到原始 S01-01 完整设计文件，本稿是基于现有讨论和已确认裁决的**重建修订稿，不冒充原文恢复**。
> 按既定协作约定：供稿阶段只记录不评审；评审须用户明确要求。
> 文件名说明：据本稿 §九 自我更正（S01-01 工作包仍为 Initialization Overview），本稿承载的是 **Project Command Center 的整体设计约束**，故文件名不带工作包编号，避免与既有两个 S01-01 稿撞号。

## 〇、修订依据（原稿自带）

- S01-01 原有初始化事项设计脉络。
- 2026-11-04 至 2026-11-05 的 Project Dashboard 讨论。
- 用户最新裁决：AI-PM 采用**方案 A**（R-S01-01-01，见 [s01-01-native-ai-pm-independence-ruling-v1.md](s01-01-native-ai-pm-independence-ruling-v1.md)）。
- 已确认的 InitItem 读模型与六态展示规则。

---

## 1. 产品定位

### 1.1 定义

Project Command Center 是用户进入一个 Project 后的**默认首页**，也是项目生命周期中的**持续工作控制中心**。

它负责帮助用户：

1. 了解项目目前的整体情况。
2. 查看项目正在进行的工作。
3. 管理需求、知识、决策和任务。
4. 使用产品内置 AI-PM 开展项目分析和管理。
5. 发现需要人工处理的事项。
6. 了解项目初始化和外部工具接入情况。

### 1.2 核心设计原则

**Create Project = Ready to Work**

创建项目成功后，项目立即具备开展工作的能力。

- 不需要先接入代码仓库。
- 不需要先配置 MCP。
- 不需要先接入 Codex 或 Claude Code。
- 不需要等待初始化全部完成。
- 不需要先完成项目基线确认才能管理需求。

### 1.3 产品边界

| 概念 | 定位 |
| --- | --- |
| Project | 核心业务对象 |
| Project Command Center | 项目工作控制中心 |
| Project Initialization | 项目能力准备情况 |
| Built-in AI-PM | 产品原生 AI 能力 |
| Repository | 可选工程集成 |
| MCP | 可选外部工具接入 |
| Project Baseline | 经过正式确认的项目基线 |

不引入 Workspace 作为额外业务实体。

---

## 2. 页面整体信息架构

Project Command Center **不是传统的数据统计 Dashboard**。

它应该围绕**项目工作的优先级**组织页面。

```
PROJECT HEADER
  Project Name
  项目描述 · 项目状态 · 项目类型
  [Project Active] [AI-PM Available] [Repository Not Connected]

01 · NEEDS ATTENTION —— 需要处理的事项
  - 待人工确认的需求
  - 等待人工决策的事项
  - 需要处理的项目风险
  - AI 分析等待人工审核的结果

02 · PROJECT OVERVIEW —— 项目概况
  - Requirements / Tasks / Decisions / Knowledge / Baseline

03 · PROJECT EXECUTION —— 项目工作进展
  - Active Work
  - AI-PM Activities
  - Agent Runs（接入后显示）
  - Recent Deliverables

04 · PROJECT INITIALIZATION —— 项目初始化进度
  - AI-PM 能力准备
  - Repository 接入
  - MCP 接入
  - 工程执行环境
  - 项目基线确认

05 · RECENT ACTIVITY —— 最近活动
  项目操作、需求变更、AI 分析、人工决策和交付记录。
```

**重要的视觉原则**：Needs Attention 必须优先于 Initialization。

> 因为用户进入项目是为了工作，而不是为了完成配置。

---

## 3. 页面各区域详细设计

### 3.1 Project Header

展示项目身份信息。

| 元素 | 说明 |
| --- | --- |
| Project Name | 项目名称 |
| Description | 项目描述 |
| Project Type | 项目类型 |
| Project Status | 项目业务状态 |
| AI-PM | 产品内置 AI 能力状态 |
| Integrations | 外部集成概况 |

Repository 未接入时，只显示**中性的 Not Connected**，不能显示为项目异常。

### 3.2 Needs Attention

这是页面**最高优先级**区域。事项应该按照实际需要人工处理的紧迫程度排序。

示例：

```
Needs Attention · 3 items

  Requirement Confirmation —— 2 项需求等待人工正式确认（Requirements · Review）
  Requirement Conflict —— 发现 1 处需求定义冲突，等待处理（AI-PM · Analysis）
  Baseline Confirmation —— 项目基线尚未正式确认（Project Governance）
```

注意：

- 只有真正需要人工处理的事项才进入这里。
- 普通任务不应该全部塞入 Needs Attention。
- 等待外部工具接入**不自动构成阻塞**。
- AI 生成的建议**不能自动视为正式决策**。
- 用户完成处理后，事项应从此区域消失或更新。

### 3.3 Project Overview

以简洁的业务统计卡片展示：

| 模块 | 统计内容 |
| --- | --- |
| Requirements | 总量、待确认、已确认 |
| Tasks | 未开始、进行中、已完成 |
| Decisions | 待决策、已确认 |
| Knowledge | 文档数量、待审核 |
| Baseline | 当前版本、确认状态 |

不应该将 AI Agent 数量、Repository 数量作为项目是否健康的核心指标。

### 3.4 Project Execution

展示正在发生的项目工作。包括：

- 人工创建的工作。
- AI-PM 正在进行的分析。
- AI 生成的建议。
- 外部 Agent 执行记录（有接入才显示）。
- 已完成的交付物。

未接入 Agent 时，**不应该出现一个空洞的 Agent 监控大面板**。

### 3.5 Project Initialization

初始化是**辅助信息**，而不是首页的主要内容。建议默认使用紧凑的进度摘要：

```
Project Readiness —— In Progress
项目工作已可正常开展，部分扩展能力尚未配置。

  Built-in AI-PM    Available
  Repository        Not Connected
  MCP Tools         Not Connected
  Project Baseline  Pending Confirmation

  查看全部初始化事项 →
```

完成初始化后，这个模块**自动收缩**；当出现新的待处理初始化事项时，再提供相应提示。

---

## 4. InitItem 数据与状态展示规则

> 这一部分必须保留此前已经确认的代码事实，**不能因为 UI 修订而重新定义后端领域模型**。

### 4.1 InitItem 的定位

InitItem 是初始化事项的**展示读模型**，不是数据库实体。它由七个不同领域实体的状态派生而来。

因此：

- 六态是 InitItem 的**展示状态**。
- 六态不是 Project 的状态。
- 六态不是 Repository 的状态。
- 六态不是 AI-PM 的状态。
- 六态不能被直接写入某个领域实体。
- 初始化事项的完成情况不能代替 Project 的工作能力判断。

### 4.2 前端展示原则

前端只消费初始化读模型提供的结果。**不允许**：

1. 根据前端自己的逻辑重新计算六态。
2. 把初始化状态反向写入领域实体。
3. 将等待外部工具接入解释为项目不可工作。
4. 把 InitItem 数量直接当作项目完成率。

### 4.3 初始化与项目工作能力分离

需要明确两个不同的判断：

| 判断 | 含义 |
| --- | --- |
| **Project Work Availability** | 项目是否能够开展原生项目工作 |
| **Initialization Readiness** | 项目各项初始化能力的准备程度 |

两者不能合并为一个状态。

例如：一个 Project 可以处于初始化尚未全部完成的状态，但已经可以完整使用需求管理、内置 AI-PM 和人工决策能力。

---

## 5. 页面交互规则

| 用户操作 | 系统响应 |
| --- | --- |
| 点击 Requirement 统计卡 | 进入需求列表 |
| 点击 Needs Attention 事项 | 进入对应处理页面 |
| 点击 AI-PM 分析 | 进入对应分析任务 |
| 点击 Baseline | 进入基线管理 |
| 点击 Repository 状态 | 进入 Repository Management |
| 点击 MCP 状态 | 进入外部工具管理 |
| 点击 Initialization | 展开完整初始化事项 |
| 点击 Recent Activity | 进入对应业务记录 |

所有跳转必须落到**真实业务功能**，不能只做展示性按钮。

---

## 6. 空状态设计

S01-01 最容易出现的错误，就是把刚创建的项目设计成一个充满空白统计卡片的 Dashboard。应该使用**主动引导**：

```
Project Ready
Your project is ready to work.
Start by defining what you want to build. Your AI-PM is ready to help.

[ Start with AI-PM ]
Describe your project goals, existing ideas, or initial requirements.
AI-PM can help organize and analyze them.        —— Available · Native AI capability

Suggested starting points
  - Create your first requirement
  - Start a requirement analysis
  - Organize project knowledge
  - Define project goals
```

以上是设计示意，**不代表要求将所有操作都集中在首页**。每个入口应进入对应的业务模块。

---

## 7. 不同项目阶段的页面表现

| 项目阶段 | 页面重点 |
| --- | --- |
| 刚创建 | 引导开始需求工作、AI-PM 原生能力 |
| 需求建设期 | Requirements、Analysis、Decisions |
| 基线建立期 | Baseline、待人工确认事项 |
| 工程接入期 | Repository、MCP、Agent |
| 执行期 | Tasks、Execution、Deliverables |
| 持续维护期 | Risks、Changes、Knowledge、Activity |

Project Command Center 的**整体框架保持稳定**，内容和优先级根据项目实际数据变化。不应该为每个阶段设计一套完全不同的 Dashboard。

---

## 8. 验收标准

本次修订需要满足以下验收条件。

- [ ] 创建 Project 后立即进入 Project Command Center。
- [ ] 不接入 Repository 也可以正常工作。
- [ ] 不接入 MCP 也可以使用内置 AI-PM。
- [ ] 不接入 Codex / Claude Code 也可以开展需求分析。
- [ ] 可以独立创建、编辑和管理需求。
- [ ] 可以开展 AI 辅助需求分析。
- [ ] AI 生成的建议与正式人工决策有明确区别。
- [ ] 可以开展项目知识管理。
- [ ] 可以开展人工评审和决策。
- [ ] Initialization 不阻止项目原生工作。
- [ ] InitItem 六态仍然只属于初始化事项展示读模型。
- [ ] 不引入 Workspace 业务实体。
- [ ] 接入外部工具后，原有工作能力不发生退化。
- [ ] 首页重点始终是项目工作，而不是初始化完成率。

---

## 9. 与 S01 工作包的关系：必须纠正一个编号问题

重新核对了此前的 S01 工作包顺序，发现上一稿有一个需要纠正的地方：

**S01-01 原本是 Initialization Overview，不是 Project Command Center。**

两者应该这样区分：

- **Project Command Center**：创建项目后进入的默认工作首页，是项目持续工作的总入口。
- **S01-01 Initialization Overview**：负责展示项目初始化事项及其准备进度，是 Command Center 中的一个功能模块，也可以进入完整详情页。

因此，**上一稿的第 2～8 节应作为 Project Command Center 的整体设计约束，不能直接取代 S01-01 的原始工作包定义**。

### 修订后的 S01 工作包顺序

| 顺序 | 工作包 | 本次修订重点 |
| --- | --- | --- |
| S01-01 | Initialization Overview | 初始化不再是项目工作的门槛 |
| S01-02 | Project Creation | 创建后立即具备工作能力 |
| S01-03 | Repository Management | 仓库接入为可选增强 |
| S01-04 | AI-PM Cognition | 内置 AI-PM 独立工作 |
| S01-05 | Project Baseline | 保留人工正式确认机制 |
| S01-06 | Codex / MCP Bootstrap | 外部工程工具独立接入 |
| S01-07 | Initialization Handover | 完成后回到 Project Command Center，不引入 Workspace 实体 |

---

## 附 A · 记录者附注：与既有文档的关系登记（事实登记，非评审）

| 既有文档 | 关系登记 |
| --- | --- |
| [裁决记录 R-S01-01-01](s01-01-native-ai-pm-independence-ruling-v1.md) | 本稿是方案 A 在 Command Center UI 层的展开（§1.2 / §3 / §8 与裁决 §一~§三同向）；本稿 §九 对裁决记录 §四「结构预告」的编号表述做了**更正**（S01-01 仍为 Initialization Overview），裁决记录 §四 已加更正注 |
| I5 总控台定稿 V1.1（`../smark-pm-s01-i5-project-command-center-v1-1.md`） | **同向**：默认工作入口定义（§1.1）、初始化进度常驻+收缩（§3.5 ≈ I5 §4.1/4.6）、InitItem 读模型与六态（§4 ≈ I5 §4.4）、零依赖可用（§1.2 ≈ I5 §4.7 增补）。**口径差（待基线确认收敛，登记不改稿）**：① 区块结构——I5 六区块（含独立的「风险与阻塞」「项目目标与进展」）vs 本稿五区（风险并入 Needs Attention；统计含 Baseline 不含 Goal）；② 头部状态 chips（Project Active / AI-PM Available / Repository Not Connected）为本稿新增要求 |
| [S01-01 初始化总览稿](s01-01-project-initialization-overview-ux-v1.md) | §九 确认该稿工作包定位不变；§4.3「Work Availability vs Initialization Readiness」分离与其 §9.5 增补（2026-11-05）同向；§8 验收「创建后立即进入 Command Center」与其 §3.6 H4「允许进入总控台」一致；初始化总览页=「Command Center 内模块 + 可进完整详情页」的双形态须在基线确认后落实到该稿 §2 |
| README「编号冲突（待裁决）」注 | 本稿 §九 修订后顺序（05=基线确认/06=Bootstrap/07=交接）与 README 既有建议顺序同向；**稿件改名与交叉引用仍须统一执行**（本稿状态=待基线确认，未执行改名）；Personal Home 编号（建议 S01-08）在本稿表中未出现 |

## 附 B · 待基线确认后的执行挂账

1. I5 六区块 vs 本稿五区的结构收敛（风险/目标归属、区块合拆）——基线确认后统一修订两稿。
2. README 索引按 §九 顺序执行改名与交叉引用统一修订。
3. §4.2「前端不允许重算六态」在 ai-frontend 实施侧的核对项（对齐 S01-01 总览稿 §14.3 既有缺口清单）。
4. §3.3 统计卡与 I5「项目目标与进展」区块的数据口径（Goal 是否入统计）待收敛。
