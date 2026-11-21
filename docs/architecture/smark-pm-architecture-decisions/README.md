# smark-pm 架构决策文档

**文档集版本：** 1.3  
**基线日期：** 2026-11-21  
**范围：** 共同设计原则、PA-03、PA-04、PA-05、PA-06、PA-07

## 文档结构

| 文档 | 内容 |
|---|---|
| [00-设计原则与架构边界](00-design-principles.md) | 产品定位、小团队协作原则、Product 与 Project 的职责边界 |
| [PA-03-Product 信息分发与人类决策路由](PA-03-product-information-routing.md) | 事项归属、通知接收者、决策路由和决策完成规则 |
| [PA-04-共享 Project 的研发协作与事项归属](PA-04-shared-project-collaboration.md) | 需求、任务、技术设计、多 Product 冲突、优先级与生命周期影响 |
| [PA-05-统一事项模型与信息流](PA-05-work-item-and-activity-feed.md) | 统一事项基础模型、领域对象、关联、Activity History、Feed 和已读状态 |
| [PA-06-AI Agent 与人类的协作边界](PA-06-agent-human-collaboration.md) | 自主权限、事项管理、正式知识修改、任务调度、交付验收和异常处理 |
| [PA-07-Task Execution Orchestration 与跨 Product 协作 V1.0](PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md) | Task / Run / Workflow、Task Dependency、Task Decision Review、Human Consultation、Product 生命周期和跨 Product 协作 |
| [PA-07-Agent Team 与执行 Runtime V1.0](PA-07-agent-team-and-execution-runtime-v1.0.md) | 单一逻辑 Project AI-PM、按需专业 Agent、多 Agent 并行、Workstream / Agent Work Unit、Runner / Harness / Session / Job / Process、MCP 通信边界和知识写入协调 |
| [PA-07 开发实施规范 V1.0](PA-07-developer-implementation-specification-v1.0.md) | schema 映射、数据模型、事务/API/事件契约、实施任务（IMP-001~019）与测试矩阵 |
| [PA-07 实施产物索引](../pa-07-implementation/README.md) | IMP-001~003 核对/审计/当前状态与最终验收报告（2026-11-22 终验） |

## 基线状态说明

本文件集整理截至 2026-11-21 的架构决策。PA-03 至 PA-06 按各自文档状态执行。PA-07 由两份互补的架构基线组成：Task Execution / 跨 Product 协作，以及 Agent Team / 执行 Runtime。两者都必须遵循 PA-06 的人类授权边界。

PA-07 中用户明确确认的决策、用户授权范围内形成的技术裁决、提案和仍需人类决定的事项必须分别标注。正式业务目标、正式需求含义、重大业务取舍及授权边界仍由人类决定。

本文件集不代表数据库表结构、API、事件 Schema、权限矩阵或 UI 交互已经全部设计完成。实施设计应遵循当前架构基线，并将新增技术细节单独记录。

## 变更原则

1. 不得静默改写已确认决策。
2. 决策发生变化时，应新增或更新明确的架构决策记录，说明变更原因、影响范围和生效版本。
3. 建议与待确认事项必须与已确认基线分开标注。
4. 本文件集应纳入 Git 版本管理。
5. PA-07 旧 Agent Work Unit / Agent Team 设计仅在与当前基线一致的部分继续参考；冲突部分标记为历史，不得继续作为有效设计依据。
6. PA-07 Agent Team 文档中的 Agent Work Unit 是逻辑工作包，不是平台独立 Task / Run 生命周期或 Agent 身份体系。
