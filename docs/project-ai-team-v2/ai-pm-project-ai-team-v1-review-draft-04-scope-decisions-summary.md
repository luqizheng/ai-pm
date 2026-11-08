# Project AI Team V1.0 产品需求与架构设计基线（评审稿）· 分篇 4/5：实施范围、三个待裁决问题与设计状态总结（§13–§15）

> 状态：评审稿转录 · 未冻结 · Q2–Q4 待确认
> 记录日期：2026-11-08 · 来源与拆分说明见 [00-index](ai-pm-project-ai-team-v1-review-draft-00-index.md)
> **编号警示：本篇 Q2/Q3/Q4 是本次 AI Team 设计评审的问题编号，不是此前已确认的 Client 调度 Q2，也与 `ai-pm-v2-architecture-baseline-v1.md` 已裁决的 Q1/Q2/Q3 不是同一组问题。**

---

## 13. V1.0 实施优先级与交付范围

建议分四个实施阶段。

| 阶段 | 交付内容 | 优先级 |
| --- | --- | --- |
| S1 | AI Team、AI Project Manager、Role Registry、Role Definition | P0 |
| S2 | Client Registry、Project Authorization、Model Policy | P0 |
| S3 | Task Dispatch、Task Execution、并行调度、结果协调 | P0 |
| S4 | Team Management UI、Execution Monitor、权限管理 | P1 |

### 13.1 V1.0 必须具备的能力

- [ ] 每个 Project 自动拥有 AI Project Manager。
- [ ] 无外部 Client 仍能使用 Native AI-PM。
- [ ] 平台预置 Role Catalog。
- [ ] Role Definition 可版本化。
- [ ] Role 与 Client 解耦。
- [ ] Client 个人注册与 Project 授权。
- [ ] Platform 自动调度。
- [ ] 支持 Task 并行与依赖。
- [ ] Task 与 TaskExecution 分离。
- [ ] 支持暂停、恢复、失败和人工介入。
- [ ] AI Team 知识写入统一协调。
- [ ] 支持项目级模型策略。
- [ ] 支持权限检查与审计。
- [ ] 提供 AI Team 管理页面。

### 13.2 V1.0 暂不建设

- Agent 自主创建和修改 Role。
- 自主修改团队组织结构。
- 多 Agent 自主无限递归拆解。
- 多项目之间的 AI Team 自治协作。
- 独立建设通用 Agent Runtime。
- 自动静默切换 Provider 或模型。
- AI 自动进行正式业务审批。

---

## 14. 最终需要裁决的三个问题

目前核心架构已经比较完整。真正需要用户进一步确认的是三个产品选择。

> 注意：以下 Q2、Q3、Q4 是本次 AI Team 设计评审的问题编号，不是此前已确认的 Client 调度 Q2。

### Q2：专业 Role 的执行方式（待裁决）

专业 Role 是否必须由独立 Agent 执行？

**A. 职责独立、执行资源共享（推荐）**

- 每个 Role 有独立职责、Instructions、Capability 和输出契约。
- AI Project Manager 可以直接处理普通问答和简单工作。
- 专业任务通过对应 Role 执行。
- 不要求每个 Role 都有独立 Agent 实例。
- 可以通过同一个 Native AI 模型承担不同 Role 的工作。
- 需要工程能力时，调度外部 Client。

**B. 专业工作必须独立 Agent 执行**

- 每个专业 Role 都需要独立 Agent 执行过程。
- AI Project Manager 只负责协调。
- 不允许 AI Project Manager 直接承担专业任务。

原稿建议 A：符合此前 Platform 不建设内部多 Agent Runtime 的架构方向，同时保留专业职责独立性。

### Q3：是否允许项目自定义 Role？（待裁决）

**A. V1.0 只允许使用平台预置 Role（推荐）**

- 启用和停用 Role。
- 调整项目级配置。
- 调整部分 Instructions 和执行约束。
- 不允许创建全新 Role。
- Role 的基础职责和安全边界由平台管理。

**B. 允许项目创建自定义 Role**

- 用户可以定义名称、职责、能力和输出。
- 可以配置自定义 Instructions。
- Platform 负责验证和调度。

原稿建议 A：先稳定 Role Contract 和调度体系，再考虑开放扩展。

### Q4：是否需要独立的 AI Team 管理界面？（待裁决）

**A. 提供完整管理页面（推荐）**

入口：Project Settings → AI Team

包括：

- Overview
- Roles
- Agent Clients
- Model Policies
- Execution Policies
- Execution Monitor

**B. V1.0 只提供基本配置**

暂时只管理 Role、Client 和模型配置，执行记录放在其他页面。

原稿建议 A：AI Team 是项目的重要基础设施，必须具有可观察性和管理能力。

---

## 15. 当前设计状态总结

| 状态 | 内容 |
| --- | --- |
| 已有正式裁决 | 12 项架构原则 |
| 本次新增确认 | AI PM = 独立管理角色 |
| 待确认 | Q2 / Q3 / Q4 |

设计覆盖范围：

- 团队组织与 Role Catalog
- Role Definition 与版本管理
- 权限矩阵
- Task Dispatch 与多任务协作
- Client Registry 与项目授权
- Work Session / AI Task / TaskExecution
- 知识协调与人工治理
- Project Settings 管理界面
- V1.0 实施优先级

原稿总体判断：现在不需要继续扩大 AI Team 的功能范围，而应该先把这三个问题裁决，随后形成正式冻结版设计基线。

原稿请求用户直接回复「Q2=A，Q3=A，Q4=A」或修改其中任意一项；并承诺根据裁决形成正式的《Project AI Team V1.0 产品需求与架构设计基线》，明确标记哪些内容已经冻结、哪些属于后续实施设计，避免在 Backend 和 Figma 阶段重新解释架构。
