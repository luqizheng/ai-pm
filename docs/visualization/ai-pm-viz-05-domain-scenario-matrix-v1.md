# 六大业务领域可视化场景与需求矩阵 V1.0

> 状态：V1.1 已修订（并入 2026-11-12 专家评审；决策基线见 ADR V1.0） · 日期：2026-11-12 · 列说明：[数据源]=已核实的既有表/字段；[生成]=规则拼装(R)/AI 起草(A)/混合(H)；[通道]=Live(L)/Artifact(V)；优先级对齐立项 P0–P3。MVP 范围以 ★ 标注。

## 4.1 Product Cognition

| 场景 | view.kind | 数据源 | 生成 | 通道 | 优先级 | 备注 |
|---|---|---|---|---|---|---|
| 产品能力地图 ★(P1) | capability-map | `pc.cognition_object` + `pc.relationship` | H | L | P1 | 按 object kind 分组；关系密度高需 lens |
| 产品模块关系图 | capability-map | `pc.relationship(_type)` | R | L | P1 | 与上图同数据不同 lens |
| 业务领域关系图 | dependency-graph | `pc.cognition_object`(domain 类) | R | L | P2 | |
| 业务对象关系图 | dependency-graph | `pc.relationship` 全量 | R | L | P2 | |
| 角色-场景-流程图 | sequence / flowchart | `pc.document(_revision)`（AI 从文档起草） | A | V | P3 | 文档内容非结构化，必须走 Artifact+审批 |

## 4.2 Project Cognition

| 场景 | view.kind | 数据源 | 生成 | 通道 | 优先级 | 备注 |
|---|---|---|---|---|---|---|
| 系统技术架构图 ★(P1) | architecture-graph | `ai.cognition_item`(architecture/repository/domain) + baseline | H | L+V | P1 | Live 为主；AI 生成版走 Artifact |
| 服务与模块依赖图 | dependency-graph | `ai.cognition_item` | R | L | P1 | |
| API 关系图 | architecture-graph | `ai.cognition_item`(convention) / 文档 | A | V | P2 | API 清单无专表，P2 评估 |
| 数据模型关系图 | data-model | `ai.cognition_item`(domain) / 迁移文件 | A | V | P2 | |
| Repository 结构与模块分布 | tree | `ai.cognition_item`(repository) | R | L | P2 | |

## 4.3 Requirement / Design / ADR

| 场景 | view.kind | 数据源 | 生成 | 通道 | 优先级 | 备注 |
|---|---|---|---|---|---|---|
| Req→Design→ADR→Task 追踪图 ★(P1) | trace-graph | `design.design_requirement` + `design.adr_requirement` + `design.adr_design_revision` + `delivery.task_requirement` | R | L | P1 | 四表现成，纯读拼装；Direct/Indirect 影响显式区分（DV-8） |
| 需求覆盖关系图（含反向缺口） | impact-tree | 同上，按 requirement 为根 | R | L | P1 | "无 Design 覆盖"即覆盖缺口，天然产出 |
| 设计依赖关系图 | dag | `design.design` 间 depends-on（若无此表则经 ADR 桥接，见缺口 G-3） | R | L | P2 | |
| 架构变更影响分析图 ★(P1) | impact-tree | 追踪关系反向展开 | R | L | P1 | 选中任一节点看下游；Direct/Indirect 两类分开呈现（DV-8） |
| 需求变更传播路径 | trace-graph | revision 链 + 追踪边 | R | L | P2 | 依赖 revision 历史完整 |

## 4.4 Task Management

| 场景 | view.kind | 数据源 | 生成 | 通道 | 优先级 | 备注 |
|---|---|---|---|---|---|---|
| Task Dependency Graph ★(P1) | dag | `delivery.task_dependency` + `delivery.task`(13 态) | R | L | P1 | 单表单图，最简可测 |
| 项目执行路径 / 关键路径 ★(P1) | dag + 路径高亮 | 同上 + task_run 状态 | R | L | P1 | 关键路径计算纯前端/SQL 均可 |
| 任务执行泳道图 | sequence | `plan.task_plan` + `delivery.task` | R | L | P2 | |
| 风险传播关系图 | dependency-graph | task_dependency + blocked/awaiting_decision 态 | R | L | P3 | 风险定义需领域评审 |

## 4.5 AI Collaboration

| 场景 | view.kind | 数据源 | 生成 | 通道 | 优先级 | 备注 |
|---|---|---|---|---|---|---|
| 对话内 AI 生成图（任意 kind） ★(P0·实验) | 按 request | VizSpec（AI 起草，nature=business/explanatory） | A | V | P0 | envelope `viz_artifact` 通道；SeeCode 实验接入非核心依赖（DV-11） |
| Multi-Agent 协作图 | sequence | `ai.conversation` + `delivery.task_run_event` | R | L | P2 | 事件表追加式，回放友好 |
| Agent 执行时序图 ★(P2) | sequence | `delivery.task_run_event`(幂等白名单事件) + `task_checkpoint` | R | L | P2 | 0013 幂等设计保证事件可信 |
| Agent 上下文传递路径 | trace-graph | `ai.conversation_context_item` 注入链 | R | L | P3 | |
| ACP 会话恢复路径 | sequence | `task_checkpoint.session_id` + worker_command | R | L | P3 | |
| 人工审批/权限检查/回传流程图 | flowchart | 流程为静态语义（文档化） | A | V | P3 | 教学向，Artifact 即可 |

## 4.6 Repository / Code Cognition

| 场景 | view.kind | 数据源 | 生成 | 通道 | 优先级 | 备注 |
|---|---|---|---|---|---|---|
| 代码模块依赖图 | dependency-graph | repo 扫描（SeeCode `SC scan` 同思路）/ `ai.cognition_item` | H | L+V | P2 | SeeCode evidence 理念复用 |
| Commit 影响分析 ★(P3) | impact-tree | commit → 文件 → requirement 追溯（`kb.source` / docproc） | R | L | P3 | 需 commit↔task↔requirement 链路核验 |
| 多 Repository 依赖关系 | dependency-graph | `ai.cognition_item`(repository) | R | L | P3 | |
| Code→Requirement→Design 追踪 | trace-graph | 组合链 | R | L | P3 | 依赖上游缺口补齐 |

## 缺口清单（进入任务拆解）

| # | 缺口 | 影响 | 处置 |
|---|---|---|---|
| G-1 | 后端无图/边聚合读端点 | 所有 Live 场景 | P0 建 `viz_query` 聚合层（先覆盖追踪图/DAG 两类） |
| G-2 | 前端零图形库 | 所有场景 | P0 引入 sandbox iframe（零新依赖）；P1 引入 G6 |
| G-3 | Task↔Design/ADR 无直接关联表（二跳拼接） | 追踪图 D→T 边语义 | V1 用二跳拼接（经 requirement），不新增业务表；若评审认为必须直连，另立迁移评审 |
| G-4 | API/commit 等对象无结构化承载 | 4.2/4.6 部分场景 | 对应场景降 P2/P3，等认知域对象建模成熟 |
| G-5 | SeeCode 渲染器依赖 Node 20 | worker 部署 | 镜像加 Node 运行时（D4 §9） |

## MVP 范围（并入专家评审后修订，DV-14）

专家裁决调整了实施顺序：**P0 不含 Live 图**，Live 追踪图/Task DAG 移至 P1。

- **P0（Foundation & Security & First Renderer）**：VizSpec 核心 + 校验器 + Relation Registry + Artifact 元数据/provenance + Adapter Contract Tests + Spec Golden Tests + 权限投影过滤（Q11）+ 一致性标记（Q12）+ 安全 Viewer（独立 Origin/CSP/postMessage 协议）+ SeeCode 实验接入（HTML/SVG/PNG、中文用例、失败降级，**非核心运行依赖**）。
- **P1（Live Graph）**：G6 5 Adapter（Q2 验证后冻结）+ 追踪图 + Task DAG + 架构图 + Direct/Indirect 影响。
- 原"三链路互不阻塞可并行"的判断保留，作为 P1 内部的并行拆分依据。
