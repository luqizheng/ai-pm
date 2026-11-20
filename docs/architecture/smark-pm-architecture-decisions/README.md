# smark-pm 架构决策文档

**文档集版本：** 1.2\
**基线日期：** 2026-11-20\
**范围：** 共同设计原则、PA-03、PA-04、PA-05、PA-06、PA-07

## 文档结构

| 文档 | 内容 |
|---|---|
| [00-设计原则与架构边界](00-design-principles.md) | 产品定位、小团队协作原则、Product 与 Project 的职责边界 |
| [PA-03-Product 信息分发与人类决策路由](PA-03-product-information-routing.md) | 事项归属、通知接收者、决策路由和决策完成规则 |
| [PA-04-共享 Project 的研发协作与事项归属](PA-04-shared-project-collaboration.md) | 需求、任务、技术设计、多 Product 冲突、优先级与生命周期影响 |
| [PA-05-统一事项模型与信息流](PA-05-work-item-and-activity-feed.md) | 统一事项基础模型、领域对象、关联、Activity History、Feed 和已读状态 |
| [PA-06-AI Agent 与人类的协作边界](PA-06-agent-human-collaboration.md) | 自主权限、事项管理、正式知识修改、任务调度、交付验收和异常处理 |
| [PA-07-Task Execution Orchestration 与跨 Product 协作 V1.0](PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md) | 新执行架构、Workflow、Task/Run、Task Dependency V1、Task Decision Review、Human Consultation、Product 生命周期与跨 Product 协作 |
| [PA-07 开发实施规范 V1.0](PA-07-developer-implementation-specification-v1.0.md) | PA-07 的 schema 映射、数据模型、事务/API/事件契约、实施任务（IMP-001~018）与测试矩阵 |

## 基线状态说明

本文件集整理截至 2026-11-20 的架构决策。PA-03 至 PA-06 按各自文档状态执行。PA-07 V1.0 为正式架构基线：其中用户明确确认的决策与助手依据用户授权形成的技术裁决在文档内分别标注；正式需求、Product 目标、重大业务取舍等决定权仍属于人类。PA-07 的 V0.1/V0.3 讨论稿已被 V1.0 取代，不再单独归档；实施差距与代码验证事项以配套开发实施规范为准。

本文档集不代表数据库表结构、API、事件 Schema、权限矩阵或 UI 交互已经全部设计完成。后续实施设计应遵循已确认决策，并将新增技术细节单独记录。

## 变更原则

1. 不得静默改写已确认决策。
2. 决策发生变化时，应新增或更新明确的架构决策记录，说明变更原因、影响范围和生效版本。
3. 建议与待确认事项必须与已确认基线分开标注。
4. 本文件集应纳入 Git 版本管理。
5. PA-07 的旧 Agent Work Unit 方案保留为历史记录；与新执行架构冲突的内容不得继续作为有效设计依据。
