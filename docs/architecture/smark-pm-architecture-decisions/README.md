# smark-pm 架构决策文档

**文档集版本：** 1.4  
**基线日期：** 2026-11-24  
**范围：** 共同设计原则、PA-03、PA-04、PA-05、PA-06、PA-07、ART-01

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
| [PA-07 开发实施规范 V1.0](PA-07-developer-implementation-specification-v1.0.md) | PA-07 的目标实施契约（schema 映射、数据模型、事务/API/事件契约、实施任务 IMP-001~019 与测试矩阵）；不代表对应功能已实现 |
| [PA-07 实施产物索引](../pa-07-implementation/README.md) | IMP-001~003 核对/审计/当前状态与最终验收报告（2026-11-22 终验） |

| [PA-07 开发实施规范 V1.0](PA-07-developer-implementation-specification-v1.0.md) | PA-07 的目标实施契约；不代表对应功能已实现 |

## PA-07 基线、实施规范与实施证据

PA-07 是一套执行架构，由两份互补基线共同构成，而不是两套独立架构：

- **Task Execution / Cross-Product Collaboration** 定义 Task、Revision、Dependency、Workflow、Orchestrator、Run、Product 门禁、验收、恢复和跨 Product 协作，回答任务如何被计划、编排、执行和验收。
- **Agent Team / Execution Runtime** 定义 Project AI-PM、专业 Agent、Workstream、逻辑 Agent Work Unit、Runner、Provider Adapter、Harness、Session、Job、Process、Workspace 与 MCP 边界，回答 AI-PM 如何组织工程执行、工程工作如何运行。
- Agent Team / Runtime 必须服从 Task Execution 的任务、授权、状态和验收契约；它不建立第二套 Task / Run 状态体系，也不扩大 Agent 权限。

**文档权威分工：**

| 文档 | 定位 | 能证明什么 |
|---|---|---|
| 两份 PA-07 架构基线 | 已批准架构规则 | 系统应遵循的业务与技术边界 |
| Developer Implementation Specification | 目标实施契约 | 实现应满足的 Schema、API、事务、事件、权限和测试要求 |
| `pa-07-implementation/PA07-IMP-*` | 实施核对与证据 | 特定 commit 下代码、migration、测试的实际状态 |

**实施规范不是完成报告，实施证据也不能擅自改变架构基线。** 早期审计必须连同其核对 HEAD 阅读；当前状态以最新复核报告及其明确的 HEAD 为准。实施证据入口见 [PA-07 实施产物索引](../pa-07-implementation/README.md)。

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


## ART-01 基线、实施规范与实施证据

ADR-ART-001 定义 AI-PM 通用 Artifact Service 的独立部署与数据权威边界。它修订既有 Visualization 决策中明确列出的部署与存储部分，但不改变 Visualization 的业务语义权威、VizSpec、Relation Registry、权限投影和 Live Graph 规则。

- Artifact Service 管理通用 Artifact、Version、Reference、文件存储和 Generator Job。
- AI-PM 业务侧负责业务对象、业务授权上下文、审批和关系语义。
- Mermaid 与 SeeCode 是 Artifact Service 的 Generator Adapter；SeeCode 继续遵守 SVG-only 安全签收。
- ART-00 现状审计、ART-01 API/数据模型设计和后续实施证据必须独立归档。

## 基线状态说明

本文件集整理截至 2026-11-24 的架构决策。PA-03 至 PA-07 按各自文档状态执行。ART-01 为本次新增的基础服务架构基线；具体数据模型、API、IAM 适配、对象存储部署和旧数据迁移仍须经过 ART-00 / ART-01 实施设计，不代表已经完成。


### ART-00 审计结论摘要

ART-00 已完成仓库静态代码与文档审计：现有 RustFS/file-service、docproc ArtifactStore、Visualization Artifact 和 SeeCode 沙箱可复用，但尚无通用独立 Artifact Web Service。建议新建独立 Repository 以获得独立 Cargo workspace、CI、镜像与发布；物理文件存储继续复用 file-service/RustFS。生产数据量、项目授权委托和 GitHub Actions 运行态仍需 ART-01/ART-02 前核验。详见 [ART-00 审计报告](ART-00-artifact-service-current-state-audit-v1.0.md)。


### ART-01 文档入口与实施状态

- [ADR-ART-001](ADR-ART-001-artifact-service-v1.0.md)：独立 Artifact Service 架构决策。
- [ART-00](ART-00-artifact-service-current-state-audit-v1.0.md)：现有代码、存储、IAM 与 Visualization 资产静态审计。
- [ART-01](ART-01-artifact-service-api-and-data-model-v1.0.md)：V1 API、数据模型、授权委托、file-service Adapter、Generator 和迁移契约。

ART-01 是目标实施契约，不代表独立服务、SDK、数据库或迁移已经实现。真实 token-service claims、file-service 能力、RustFS 运维状态和 viz.* 数据量必须在 ART-02 取得证据后才能进入实施。


### ART-02 第一轮核验

- [ART-02 实施前置核验报告](ART-02-artifact-service-pre-implementation-verification-v1.0.md)：核验 pm-api JWT/JWKS、项目授权、file-service、viz.*、SeeCode/Mermaid 和当前 CI；明确区分代码事实、历史快照和运行态待核验项。

ART-02 第一轮结论：独立服务与复用 file-service 的方向成立，但 Artifact 专用 project/action 委托凭证和 file-service upload-intent/checksum/finalize 能力尚未得到证实。两个 P0 项关闭前，不进入生产实现或迁移。
