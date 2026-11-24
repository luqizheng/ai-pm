# AI-PM 文档总索引

> 本索引对应 `master` 分支，整理基准：2026-11-24。当前统计为本次整理前后的目录快照；新增或移动文件后应重新核对。

本目录包含产品定义、架构决策、实施设计、场景规格、审查证据、验收记录、运维手册及 UI 设计。**目录归属不等于文档权威级别**：判断某项规则是否有效，必须先看文档状态和对应决策索引，不能仅凭文件名或版本号。

## 1. 从哪里开始

| 目的 | 首选入口 |
|---|---|
| 了解文档规则、状态和维护方法 | [文档治理规范](governance/documentation-governance.md) |
| 了解项目整体架构原则 | [架构设计原则](architecture/smark-pm-architecture-decisions/00-design-principles.md) |
| 查找已确认的 smark-pm 架构决策 | [架构决策总索引](architecture/smark-pm-architecture-decisions/README.md) |
| 了解 AI 与人类的协作边界 | [PA-06](architecture/smark-pm-architecture-decisions/PA-06-agent-human-collaboration.md) |
| 了解 Task / Run / Workflow / Product 执行编排 | [PA-07 执行编排基线](architecture/smark-pm-architecture-decisions/PA-07-task-execution-orchestration-cross-product-collaboration-v1.0.md) |
| 了解 Agent Team、专业 Agent 与 Runner Runtime | [PA-07 Agent Team 与 Runtime](architecture/smark-pm-architecture-decisions/PA-07-agent-team-and-execution-runtime-v1.0.md) |
| 查找 PA-07 工程落地映射与实施任务 | [PA-07 Developer Implementation Specification](architecture/smark-pm-architecture-decisions/PA-07-developer-implementation-specification-v1.0.md) |
| 查找 PA-07 已完成的代码核对 | [PA07-IMP-001 Schema Mapping](architecture/pa-07-implementation/PA07-IMP-001-schema-mapping.md) |
| 查找 PA-07 实施审计与当前状态 | [PA-07 实施产物索引](architecture/pa-07-implementation/README.md)（IMP-001~003 + 最终验收报告） |
| 查找 PA-08 执行可靠性实施计划 | [PA-08 实施索引](architecture/pa-08-implementation/README.md) |
| 查找 S01–S05 产品场景与 UI 规格 | [scenarios/](scenarios/) |
| 查找验收记录 | [acceptance/](acceptance/) |
| 查找历史审查、代码事实和差距证据 | [audit/](audit/) |

## 2. 顶层目录

截至本次整理，`docs/` 下共有 22 个主题目录；文件数为本次索引生成时的快照值，不应作为运行时契约。

| 目录 | 文件数 | 职责与边界 |
|---|---:|---|
| `acceptance/` | 7 | 验收记录、检查点和实施验收证据；记录结果，不单独定义架构 |
| `adr/` | 2 | 编号式架构决策记录；与架构决策总索引互相引用 |
| `architecture/` | 90 | 当前架构原则、领域模型、系统边界、正式架构基线及其实施映射 |
| `audit/` | 242 | 代码事实、数据库盘点、影响分析、专家审查和历史核验；证据材料，不自动成为规范 |
| `design/` | 2 | 跨主题设计资料 |
| `e2e-spec/` | 13 | 端到端行为规格和测试契约 |
| `evaluation/` | 11 | 评估基线、数据集、评估报告 |
| `executor/` | 10 | Executor / Runner 接入、Bootstrap、部署与运维实施资料 |
| `figures/` | 44 | 跨主题共享图表和配图；场景专属图仍放在场景目录 |
| `governance/` | 6 | 文档治理及平台治理、身份、授权和策略组合 |
| `product/` | 17 | 产品定位、产品需求和 PRD |
| `product-cognition-knowledge-base/` | 14 | 产品认知知识库及其详细需求材料 |
| `project-ai-team-v2/` | 25 | 早期 Project AI Team V2 设计、实施和验收证据；与 PA-07 新基线冲突时以当前 PA-07 为准，历史材料不静默删除 |
| `reference/` | 6 | 技术参考、盘点、迁移和开发协作资料 |
| `runbooks/` | 4 | 开发、部署、登录链和运维操作手册 |
| `scenarios/` | 135 | S01–S05 产品场景、后端规格、UI、QA 和场景配图 |
| `slice-d/` | 9 | 早期 Slice D 执行管线设计及评审；作为演进历史和实现证据 |
| `superpowers/` | 12 | Superpowers 规格与实施计划；计划不自动等于已实施功能 |
| `tasks/` | 11 | Task 域专项设计、状态机、依赖、Outbox 和实施规格 |
| `team-onboarding/` | 3 | 团队入职、团队管理与相关 UI 资料 |
| `ui/` | 33 | 通用 UI、设计系统、AI 协作交互及跨场景 UI 规格 |
| `visualization/` | 28 | 可视化产品、架构、交互和验收规格 |

## 3. 重要子目录

### 3.1 Architecture

- `architecture/smark-pm-architecture-decisions/`：当前 PA 决策文档集，是 smark-pm 业务与系统架构决策的首选入口。
- `architecture/pa-07-implementation/`：PA-07 工程核对和实施映射产物。
- `architecture/product-cognition/`：Product Cognition 领域模型与实施架构。
- `architecture/knowledge-services/`：知识服务、Markdown 知识空间、索引和检索。
- `architecture/four-domain/`：四域模型、权威边界、对象/关系注册及数据库映射。
- `architecture/phase4/`、`architecture/organization-accesss-v1/`：阶段性组织、权限和产品域设计资料；具体有效性以其决策状态为准。

### 3.2 Scenarios

场景目录按 `scenarios/s01/` 至 `scenarios/s05/` 组织。场景内部可以包含 `docs/`、`ui/`、`qa/`、`figures/` 等子目录。场景索引优先放在对应场景目录，不把场景细节重复堆到本文件。

### 3.3 Artifact Service 与 Visualization\n\n- `architecture/smark-pm-architecture-decisions/ADR-ART-001-artifact-service-v1.0.md`：通用 Artifact Service 的独立部署、数据存储、版本和 Generator 架构基线。\n- `visualization/ai-pm-viz-27-artifact-service-integration-v1.md`：现有 Visualization 与 Artifact Service 的职责拆分、迁移边界和 ART-00 审计清单。\n\n通用 Artifact Service 的部署、数据和 API 权威以 ADR-ART-001 为准；原 Visualization 文档中被 ADR-ART-001 明确修订的部署/存储条款不再作为现行决策。\n\n### 3.4 历史设计与当前基线

`project-ai-team-v2/`、`slice-d/`、部分早期 `audit/` 与 `superpowers/` 文件可能描述旧架构或已完成阶段。它们用于解释演进、提供代码/验收证据，不应覆盖当前 PA-06 / PA-07 基线。

## 4. 文档链接与配图约定

- 文档内部链接统一使用**仓库根相对路径**，例如 `docs/architecture/smark-pm-architecture-decisions/README.md`；禁止依赖当前文件目录的 `./`、`../` 或裸文件名。
- 共享配图放在 `docs/figures/`；场景专属配图放在对应场景的 `figures/` 子目录。
- 新增主题目录时，必须更新本索引；大规模目录内部变更时，更新对应子目录索引。
- 不因目录整理而删除审查证据、验收记录、原始盘点或尚有引用价值的历史版本。
- 文件移动或重命名后，必须全仓搜索并修复旧路径引用。

## 5. 维护纪律

1. 先判断文档类型和权威级别，再决定存放位置。
2. 正式架构决策写入对应基线文档，并更新其 README / 决策索引。
3. 代码现状、差距和验证结果写入 audit 或 implementation 文档，不把代码事实与目标架构混为一谈。
4. 实施计划和验收记录应链接对应的正式架构基线。
5. 文档冲突必须显式登记并裁决；不得通过简单改标题或移动文件制造“已解决”的假象。
6. 文档状态、变更规则和冲突处理详见 [文档治理规范](governance/documentation-governance.md)。
