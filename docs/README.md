# AI-PM 文档索引（docs/）

本目录收录 AI-PM 项目的设计、验收、运维与场景（Slice）文档。2026-11-13 完成整体归置：散落根目录的文档按主题移入子目录，S01–S05 场景设计统一归入 `scenarios/`。

## 导航约定

- 文档间相互引用统一使用**仓库根相对**的绝对形式：`docs/<子目录>/<文件>.md`（例如 `docs/scenarios/s01/docs/s01-implementation-plan-v1.md`）。这样无论从哪个层级引用都能解析，请勿写相对路径（`./`、`../`）或裸文件名。
- 配图统一放在 `figures/`（跨场景共享）或各场景的 `figures/` 子目录（场景专属）。
- 本 `README.md` 仅做目录级导航；如某目录内容变化较大，请同步更新本节。

## 顶层目录索引

| 目录 | 用途 | 文件数 | 直接子目录 |
|---|---|---|---|
| `adr/` | 架构决策记录（ADR-0001 多租户/Agent/Executor 身份边界；ADR-0002 Executor 凭据分离） | 2 | — |
| `governance/` | 治理与跨域策略（客户主数据、主体身份、租户基础设施、鉴权-通知集成、D2-1 策略组合） | 5 | — |
| `executor/` | 执行端接入与运营（Codex bootstrap、onboarding runbook、t031/t032/t080/t081/t088 设计稿） | 10 | — |
| `tasks/` | 任务域设计（估算 PRD、任务依赖、任务详情页/追踪、13 态状态机、交付验收工作台、Outbox） | 11 | — |
| `slice-d/` | Slice D 执行管线（D2 上下文 / D3 Executor 适配 / D4 交付评审 / D5 评审 的设计与评审稿） | 9 | — |
| `acceptance/` | 验收记录（checkpoint1/2、kp1/kp2/kp3、dk-d 回写验收） | 7 | — |
| `runbooks/` | 运维手册（登录链、ollama + bge-m3 嵌入、测试服部署） | 3 | — |
| `reference/` | 技术参考（迁移台账、后端重构任务书、项目发现、测试性能、并行开发纪律、测试环境） | 6 | — |
| `architecture/` | 架构设计（架构原则、五阶段业务架构、混合工作区、MCP Executor 定位、多仓模型、对象存储） | 40 | `knowledge-services/`、`product-cognition/` |
| `ui/` | 通用 UI / 设计系统（控制台、设计系统、页面、unslop 审计、figma prompt、smark-pm 系列） | 29 | `ai-collab/` |
| `product/` | 产品需求与 PRD（腾讯文档发布 PRD 等） | 17 | — |
| `product-cognition-knowledge-base/` | 产品认知知识库（详细需求 v1.1 等） | 14 | `product-cognition-detailed-requirements-v1.1/` |
| `project-ai-team-v2/` | AI 团队 v2 项目文档 | 25 | — |
| `e2e-spec/` | 端到端规格 | 13 | — |
| `evaluation/` | 评估（基线 / 数据集 / 报告） | 11 | `baselines/`、`datasets/`、`reports/` |
| `design/` | 设计稿 | 2 | — |
| `figures/` | 共享配图 / 图表（PNG 等） | 44 | — |
| `superpowers/` | superpowers 计划与规格 | 12 | `plans/`、`specs/` |
| `team-onboarding/` | 团队入职 | 3 | — |
| `visualization/` | 可视化 | 28 | — |
| `scenarios/` | 场景设计 S01–S05（见下） | 134 | `s01/` `s02/` `s03/` `s04/` `s05/` |

## 场景设计 S01–S05（`scenarios/`）

每个场景目录归集该阶段的设计、UI、QA 与配图，结构大致为 `<场景>/docs`、`/<场景>/ui`、`/<场景>/qa`、`/<场景>/figures`。

| 场景 | 文件数 | 子目录 | 主题 |
|---|---|---|---|
| `scenarios/s01/` | 58 | `docs/` `qa/` `ui/` | 项目初始化（原 `s01-项目初始化`；含成员授权、后端 API 需求、实现计划、QA 验收、UI 设计） |
| `scenarios/s02/` | 17 | `ui/` | 需求域（运行时校验、源可溯、鉴权隔离、权限基础、smark-pm 系列） |
| `scenarios/s03/` | 35 | `figures/` `ui/` | 领域资产盘点与技术设计（含 `ui/acp/` ACP 切片设计、smark-pm 系列） |
| `scenarios/s04/` | 11 | `ui/` | 任务视图（概览 / 拆分 / 详情 /  backlog / 执行计划依赖图 / 时间线 / 执行就绪） |
| `scenarios/s05/` | 13 | `ui/` | 执行中心（执行详情 / 多 Agent 协作 / 人工干预 / 执行授权 / 交付评审 / 执行历史） |

> S01–S05 的 UI 文档（原散在 `docs/ui/s0x`、`docs/ui/s03-acp`、`docs/ui/smark-pm-s0x-*`）已并入对应场景的 `ui/` 子目录；S03 的 ACP 切片设计位于 `scenarios/s03/ui/acp/`。

## 架构子目录（`architecture/`）

| 子目录 | 用途 |
|---|---|
| `architecture/knowledge-services/` | 知识服务（文档处理 td01–td05、知识 v2 实现、Markdown 知识空间、t060 知识索引） |
| `architecture/product-cognition/` | 产品认知架构设计 |
| `architecture/smark-pm-architecture-decisions/` | smark-pm 架构决策文档集 V1.2（00 设计原则 + PA-03~PA-06 + PA-07 V1.0 及其开发实施规范：信息路由/共享 Project 协作/统一事项与信息流/Agent 协作边界/Task 执行编排与跨 Product 协作，基线 2026-11-20） |
| `architecture/pa-07-implementation/` | PA-07 开发实施产物（IMP-001 schema 映射报告等） |

## 维护提示

- 新增文档时，按主题放入对应子目录；若无合适目录，先在 `reference/` 或对应主题目录落地，再考虑扩展。
- 移动或重命名文档后，必须同步更新全仓引用（搜索 `docs/<旧路径>` 并改写为新绝对路径）。
- 本索引与文档移动史参见项目记忆 `.workbuddy/memory/2026-11-13.md`。
