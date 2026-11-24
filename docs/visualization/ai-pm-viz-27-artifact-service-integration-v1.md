# ART-00 前置说明：Artifact Service 与现有 Visualization 的集成边界

> **状态：** 与 ADR-ART-001 配套的现状映射与实施约束  
> **日期：** 2026-11-24  
> **目的：** 防止把新 Artifact Service 当作空白项目，或误将现有 Visualization 全部搬迁为一个新的 Visualization Service。

## 1. 结论

本次架构调整的目标不是拆出独立 Visualization Domain，而是建设平台级通用 **Artifact Service**。

现有 Visualization 仍然有两类不同能力：

1. **业务投影与在线视图**：VizSpec、Relation Registry、业务关系查询、权限投影、Live Graph。它们依赖 AI-PM 业务数据和业务权限，应继续由 AI-PM 业务侧负责。
2. **产物管理与生成基础设施**：Artifact 元数据、版本、文件存储、Generator Job、渲染输出和交付。这部分应逐步迁移到独立 Artifact Service。

目标边界：

~~~text
AI-PM (pm-api / business modules)
  - VizSpec semantics
  - Relation Registry
  - business data projection
  - project/object permission filtering
  - Live Graph query and UI
  - business approval and workflow
            |
            | Artifact API / SDK
            v
Artifact Service (independent Web Server)
  - generic Artifact / Version / Reference
  - upload / download / S3 object management
  - generation jobs
  - Mermaid Adapter
  - SeeCode Adapter
  - output validation / integrity / audit
            |
            +-- PostgreSQL metadata
            +-- S3-compatible object storage
~~~

## 2. 当前仓库已有资产（2026-11-24 代码检索快照）

以下为已观察到的路径，不代表所有能力均已完成生产验收：

| 现有资产 | 路径 | 当前架构意义 |
|---|---|---|
| Visualization Artifact 数据模型 | migrations/0020_visualization_artifact.sql | viz schema 的专用图形产物、审批、渲染输出和槽位配置 |
| VizSpec 领域模型 | crates/pm-domain/src/visualization/ | AI-PM 自有图形语义模型，不应迁入通用 Artifact Service 作为业务事实 |
| Visualization 应用服务 | crates/pm-application/src/visualization/ | 图形校验、投影及现有 Artifact 逻辑，需逐项拆分职责 |
| SeeCode Adapter | crates/pm-infrastructure/src/viz_seecode_adapter.rs | 既有 SeeCode 编译/渲染接入 |
| Render Sandbox | crates/pm-infrastructure/src/viz_render_sandbox.rs | 子进程隔离执行基础 |
| SeeCode vendor | tools/seecode/ | 固定上游版本与许可材料 |
| Visualization 前端 | ai-frontend/src/components/viz/ | Viewer、版本、审批、Stale 等现有 UI |
| Visualization API | ai-frontend/src/api/visualizations.ts 及 pm-api 对应路由 | 当前 AI-PM 内部 API，未来经 Artifact Service SDK/HTTP 接入产物能力 |
| Visualization 架构决策 | docs/visualization/ai-pm-viz-09-architecture-decision-record-v1.md | 原部署与存储边界，部分条款由 ADR-ART-001 修订 |

## 3. 不允许直接搬迁的内容

以下能力不属于通用 Artifact Service：

- VizSpec 的业务语义与字段约束；
- Relation Registry 及其领域 Owner 会签；
- 业务对象关系查询与二跳推导；
- 业务查询层和图形投影层的双层权限过滤；
- Live Graph 的 current / mixed-revision 一致性语义；
- business / explanatory 图形性质判断；
- Requirement、Design、ADR、Task 的审批和业务工作流；
- 对话中 AI 生成图的业务确认与上下文协调。

这些能力继续留在 AI-PM。Artifact Service 只接收经过业务侧授权、校验和整理的生成输入，以及项目/租户范围和引用标识。

## 4. 应迁移到 Artifact Service 的能力

- Artifact / Version 元数据管理；
- Artifact 与外部业务对象的多对多 Reference；
- 源文件和生成输出文件的 S3 存储；
- 生成任务与 Worker 调度；
- Mermaid / SeeCode 等 Generator Adapter；
- Hash、MIME、文件大小和输出安全检查；
- 下载/预览授权、审计与生命周期管理。

迁移前须盘点当前 viz.artifact / viz.render_output / viz.approval / viz.slot_config 的字段、代码读写路径、数据量和测试。不能仅按表名把整个 viz schema 迁走。

## 5. Mermaid 与 SeeCode 的目标分工

| 项目 | Mermaid Adapter | SeeCode Adapter |
|---|---|---|
| 定位 | V1 正式 Generator | V1 正式 Generator |
| 输入 | Mermaid DSL 文本 | AI-PM VizSpec（先编译为 SeeCode Spec） |
| 原始源文件 | .mmd | AI-PM VizSpec 快照；必要时保存编译输入快照 |
| V1 输出 | SVG；PNG 经环境验证后启用 | SVG only |
| 优势 | 文本可读、易 diff、易由 AI 修改、常用图形覆盖广 | 编辑级静态图形表达与复杂视觉编排 |
| 版本记录 | Mermaid 引擎版本 + Adapter 版本 | SeeCode 上游版本/commit + Adapter 版本 |
| 安全要求 | 输入限额、零外网、SVG 静态化检查 | 零凭据、零外网、资源限额、SVG 静态化检查 |
| 失败降级 | 保留 .mmd 源文件和错误信息；可展示源码/结构化文本 | 表格或 Mermaid（仅当语义可无损映射时） |
| 不承担 | 大型实时在线图查询 | 在线 Live Graph / 业务关系查询 |

**关键区别：** Mermaid 是一等生成器，不只是 SeeCode 失败后的临时 Fallback。Mermaid 源码必须随 Artifact Version 保存；SVG 是派生产物。PNG 不能仅因 SVG 可用就默认承诺，须经过无外网和字体实测。

SeeCode 继续遵守既有 ADR-VIZ-002 M-2 签收：服务端 SVG only。不得将早期 HTML/iframe 方案重新引入。

## 6. 当前数据模型与通用 Artifact 的关系

现有 viz.artifact 是 Visualization 专用记录，不等同于新通用 Artifact。

| 现有对象 | 目标处理方向 |
|---|---|
| viz.artifact | 映射到通用 Artifact + Artifact Version；保留 VizSpec 语义快照 |
| viz.render_output | 输出文件迁入 S3；元数据和 Hash 进入 Artifact Version / Output Manifest |
| viz.approval | 先确认审批对象和业务含义；图形发布审批元数据可保留在 Visualization 业务侧，不得变成通用业务审批服务 |
| viz.slot_config | 槽位策略属于 Visualization 产品配置，原则上留在 AI-PM，不随通用文件存储迁移 |
| source_hash / provenance | 保留来源快照与 stale 判定所需信息；业务来源解析仍由 AI-PM 提供 |

以上是迁移方向，不是最终字段映射。ART-00 必须基于实际代码与数据核验后，ART-01 才能冻结映射。

## 7. 服务依赖与测试边界

- Artifact Service 的单元、数据库、S3、Generator、安全与恢复测试由 Artifact Service 自己负责。
- AI-PM 负责消费者契约测试：API/SDK 兼容、项目上下文、引用建立和业务页面集成。
- Artifact Service 变更时跑 Artifact Service 完整 CI，并跑受影响消费者契约/集成测试。
- 普通 Requirement、Design、Task 或前端业务变更，不应触发 Artifact Service 全量测试和重新发布。
- 平台发布前和定期执行关键跨服务集成测试，不能以独立部署为由取消。

## 8. ART-00 审计清单

1. 全仓搜索 Artifact / deliverable / attachment / document / render_output / file upload 的所有表、API、服务和 UI。
2. 标注每个 Artifact 术语的真实语义，禁止同名异义。
3. 检查 viz.* 表的当前读写路径、数据量、约束、触发器和迁移依赖。
4. 检查现有 SeeCode Adapter、Render Sandbox、Worker 调度和测试是否可独立抽取。
5. 检查 Mermaid 是否已有前端 Markdown 渲染依赖、版本及安全配置；明确它与服务端 Generator 的区别。
6. 审计 IAM token 的 issuer、audience、claims、tenant/project 授权和服务间认证。
7. 审计 RustFS / MinIO / S3 兼容对象存储在开发、内网、生产环境的现状。
8. 审计 GitHub Actions 的独立工作流、路径过滤、镜像发布和 SDK 发布能力。
9. 对比独立 Repository 与 Monorepo 独立流水线的测试隔离效果和维护成本。
10. 输出旧 viz.* 数据迁移、切换、回滚和停止双写方案。

审计报告必须区分 Observed（代码/配置/实测事实）、Inferred（推断）和 Decision Required（仍需决策），并提供文件路径或命令证据。

## 9. 权威关系

- 通用 Artifact Service 的目标架构以 [ADR-ART-001](../architecture/smark-pm-architecture-decisions/ADR-ART-001-artifact-service-v1.0.md) 为准。
- 原 Visualization 业务语义、VizSpec、Registry、权限投影和 Live Graph 规则继续以仍有效的 ADR-VIZ 条款为准。
- ADR-ART-001 仅在其明确列出的范围内修订 ADR-VIZ-001、ADR-VIZ-002、ADR-VIZ-006。
- 本文是现状映射和 ART-00 审计输入，不证明迁移或独立服务已经实施。
