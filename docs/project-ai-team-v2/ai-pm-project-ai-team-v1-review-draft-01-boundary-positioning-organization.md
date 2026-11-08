# Project AI Team V1.0 产品需求与架构设计基线（评审稿）· 分篇 1/5：设计边界、产品定位与组织架构（§0–§3）

> 状态：评审稿转录 · 未冻结 · Q2–Q4 待确认（见分篇 4）
> 记录日期：2026-11-08 · 来源与拆分说明见 [00-index](ai-pm-project-ai-team-v1-review-draft-00-index.md)

**原稿设计目标**：建立项目级 AI 团队的完整产品模型，明确团队组织、专业 Role、Agent Client、调度机制、执行协作、权限治理及管理界面，为后续 Backend Domain、API、数据库和 Figma 设计提供统一依据。

原稿开篇上下文核对结果：此前 Project AI Team 已经有一组正式裁决，不应该重新讨论或改变。

---

## 0. 已有裁决与本次设计边界

### 0.1 已经确认的架构裁决

以下内容直接继承。

| 编号 | 已确认事项 | 裁决 |
| --- | --- | --- |
| AT-D01 | AI Team 归属 | 每个 Project 拥有自己的 AI Team |
| AT-D02 | Client 注册 | 个人级注册，用户可以授权给多个 Project |
| AT-D03 | Client 调度 | Platform 按 Role、Capability、授权、可用性等自动调度 |
| AT-D04 | Role 与 Client | Role 不固定绑定 Client，按 Task 动态分配 |
| AT-D05 | Platform 定位 | AI-PM 是管理与调度控制平面 |
| AT-D06 | 工程执行 | 外部 Client 承担专业工程执行 |
| AT-D07 | Native AI | 不连接外部 Client，仍可使用内置 AI-PM 能力 |
| AT-D08 | 执行模型 | Task 与 TaskExecution 分离 |
| AT-D09 | 并行协作 | 支持多个 Task 并行和依赖关系 |
| AT-D10 | 知识协调 | 知识写入由 Platform 统一协调 |
| AT-D11 | 人工治理 | 正式业务确认和高风险审批由人类负责 |

其中有三个特别重要的选择：

- Client 个人注册与 Project 授权是两个独立过程。
- Role 是工作职责定义，不是某个固定 Agent 或 Client。
- 没有符合条件的 Client 时，任务应该等待或阻塞，不能伪造成功。

这些是此前用户已经确认的约束。

### 0.2 本次新增确认

| 编号 | 事项 | 状态 |
| --- | --- | --- |
| AT-D12 | AI Project Manager 是独立管理协调角色 | 已确认 A |

AI Project Manager 不属于普通专业 Role，也不替代 Requirement Analyst、Developer 等角色。

它负责整个 Project AI Team 的协调、调度、工作跟踪与治理。

### 0.3 需要注意的两个系统边界

**第一，Project AI Team 不等于 Team Domain。**

此前已经确定：

- Team 是用户、项目和团队级 AI Provider 资源的组织边界。
- Project AI Team 是某个项目内部的 AI 工作组织。
- Team 管理 Provider 凭证及授权资源。
- Project 在 Team 授权范围内选择模型和 AI Role 策略。

因此：

```text
Team
  ├── Team Members
  ├── Team LLM Providers
  └── Projects
        ├── Project A
        │     └── Project AI Team
        └── Project B
              └── Project AI Team
```

**第二，不能把 AI Team 做成一套新的执行 Runtime。**

Platform 负责：

- 项目上下文管理
- Role 定义
- Task 创建与分配
- Client 调度
- 执行状态管理
- 结果协调
- 权限治理
- 知识维护

外部 Client 负责实际工程执行。Platform Native AI 通过既有模型服务能力承担 AI-PM 的理解、分析、协调等工作。

---

## 1. Project AI Team 产品定位

### 1.1 产品定义

Project AI Team 是项目级 AI 工作组织。

它由一个独立的 AI Project Manager 和若干专业 Role 组成，通过 Platform 调度 Native AI 能力或外部 Agent Client，共同完成项目管理、产品分析、知识维护和工程交付工作。

### 1.2 产品目标

1. 用户只需要面对一个 AI Project Manager。
2. AI Project Manager 能理解整个项目。
3. 专业工作由对应 Role 负责。
4. 不要求用户手动选择 Agent Client。
5. 支持多任务并行、依赖、暂停和恢复。
6. 所有专业工作结果都能追踪和验收。
7. 正式业务知识仍然遵循各自的权威归属。
8. 人类始终掌握重要确认和审批权。

### 1.3 核心产品体验

用户不应该看到一个复杂的 Agent 管理系统，而应该看到：

> 一个懂项目、能够组织 AI 团队工作的项目负责人。

例如：

```text
用户：
帮我新增一个用户登录需求。

AI Project Manager：
1. 理解用户意图。
2. 建立需求分析工作。
3. 创建 Requirement Analysis Task。
4. 组织相关专业 Role。
5. 收集分析结果。
6. 展示需求澄清工作区。
7. 等待用户回答。
8. 形成需求草稿。
9. 请求人工正式确认。
```

用户不需要知道背后用了几个 Client、调用了哪个模型。

---

## 2. AI Team 组织架构

建议采用以下组织结构。（原文为图形化组织图，粘贴文本中图形丢失，以下按原稿文字项重建。）

```text
Project AI Team（Project-level AI Organization）
│
├── Management Role
│     └── AI Project Manager
│           唯一统一管理与协调入口（Role Assignment / Task Dispatch）
│
├── Professional Roles
│     ├── Requirement Analyst          需求分析
│     ├── Product Cognition Analyst    产品认知
│     ├── Project Cognition Analyst    项目技术认知
│     ├── Knowledge Manager            知识维护
│     ├── Solution Architect           技术方案与架构
│     ├── Design Analyst               设计分析
│     ├── Developer                    工程开发
│     ├── QA Engineer                  测试质量
│     └── Risk Analyst                 风险分析
│
└── Execution Resources
      └── Platform Native AI / External Agent Clients
```

> 注：上图是逻辑组织关系，不代表每个 Role 都需要常驻一个 Agent 实例。

AI Project Manager 是管理协调者，专业 Role 是职责单元，Client 是执行资源。

---

## 3. Role Catalog：专业角色目录

建议 V1.0 先建立平台预置 Role Catalog。

### 3.1 Role 分类

| 分类 | 说明 |
| --- | --- |
| Management | 管理、协调和调度 |
| Product | 产品需求与产品认知 |
| Knowledge | 项目知识管理 |
| Architecture | 技术架构与技术方案 |
| Design | 设计分析 |
| Engineering | 工程实施 |
| Quality | 测试与质量 |
| Governance | 风险、安全与治理 |

### 3.2 V1.0 Role Catalog

以下为建议的初始目录，实施优先级不代表组织层级。

| Role Key | Role Name | 优先级 | 主要职责 |
| --- | --- | --- | --- |
| project_manager | AI Project Manager | P0 | 项目管理与协调 |
| requirement_analyst | Requirement Analyst | P0 | 需求分析和澄清 |
| product_cognition | Product Cognition Analyst | P0 | 产品认知分析 |
| project_cognition | Project Cognition Analyst | P0 | 项目技术认知分析 |
| knowledge_manager | Knowledge Manager | P0 | 知识整理和维护 |
| solution_architect | Solution Architect | P1 | 技术方案和架构 |
| design_analyst | Design Analyst | P1 | 设计分析与评审 |
| developer | Developer | P1 | 工程实施 |
| qa_engineer | QA Engineer | P1 | 测试与质量 |
| risk_analyst | Risk Analyst | P1 | 风险识别与评估 |
| research_analyst | Research Analyst | P2 | 外部调研 |
| security_reviewer | Security Reviewer | P2 | 安全评审 |
| release_manager | Release Manager | P2 | 发布协调 |

这里特别强调三个边界：

- Product Cognition Analyst 维护产品侧认知，不能取代 Project Cognition。
- Project Cognition Analyst 负责技术侧认知，不能取代 Product Cognition。
- Knowledge Manager 负责知识维护协调，不拥有所有知识对象的最终确认权。

### 3.3 Role 是否必须全部启用？

建议平台预置模板，Project AI Team 按项目实际情况启用。

但 AI Project Manager 必须存在，不允许用户关闭。

专业 Role 可以有：

- Enabled
- Disabled
- Not Configured
- Unavailable

Disabled 的 Role 不参与正常调度，但历史 Task 仍保留其原始 Role 版本引用。
