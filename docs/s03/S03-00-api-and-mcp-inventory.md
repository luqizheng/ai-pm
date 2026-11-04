# S03-00 · API 与 MCP 清单（API and MCP Inventory）V1.1

> V1.1：收编 R2（MCP 正式确认入口=A* 目标下不存在，现存 confirm/veto 入口标注为实现缺陷）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

基线 `feat/s01-implementation` @ `3ccb42f`。REST 注册处分散于 apps/pm-api/src 各模块 `.route(` 调用；MCP 注册处仅 crates/pm-mcp/src/server.rs。

## 1. REST 端点（90+ 条唯一路径）

方法标注说明：✅=本次核验直接确认方法；其余为路径字面量（方法见路由源码，未逐一展开标注）。写语义判定以 agent 追踪到的 handler 为准。

### 需求域（S03 直接相关）
| 路径 | 方法 | 写语义 | 权限现状 |
|---|---|---|---|
| /v1/pm/requirements | GET（✅） | 读 | 仅租户+bearer |
| /v1/pm/requirements/{requirement_id}/revisions | GET 🟡 | 读（版本历史 version DESC，pm_lists.rs:503-531） | 同上 |
| /v1/pm/requirements/{requirement_id}/revisions/{revision_id}/approve | POST（agent 确认 pm_writes.rs:235-245） | **写=定稿** | **仅租户校验；无成员/权限码/真人门；无并发防护** |
| /v1/pm/requirements/revisions/{revision_id}/migrate | 写=Git 单向镜像（knowledge_migrate.rs） | 项目内 | |

### 文档/知识域（S03 正文载体候选）
| 路径 | 写语义 | 权限现状 |
|---|---|---|
| /v1/pm/documents（GET ✅）、/{document_id}、/search、/{id}/index-state、/{id}/references | 读 | bearer |
| /v1/pm/documents/proposals | 写=提案（submit.rs:40-275；干净提案自动 commit） | 项目成员+质量门 |
| /v1/pm/documents/proposals/{proposal_id}/confirm | **写=红线人工确认** | require_human；**REST 身份取 body.confirmed_by（冒认面 R1）** |
| /v1/pm/documents/revisions/{revision_id}/veto | 写=veto（git revert+rolled_back） | require_human；body.vetoed_by（R1） |
| /v1/pm/documents/reindex、revisions/{id}/reindex | 写=索引重建（派生） | 项目内 |
| /v1/pm/projects/{project_id}/goal | Goal 读写（I4；提案强制红线 submit.rs:103-110） | 项目内 |

### 审批/监督域
| 路径 | 写语义 | 权限现状 |
|---|---|---|
| /v1/pm/approvals（GET ✅）、/v1/pm/approvals/{approval_id}/decide | 写=裁决（pm_writes.rs:74；approve→执行 ProjectCommand） | 仅租户；无真人门；身份=认证人 |
| /v1/pm/review-approvals、/{approval_id}/decision | 写=D5 复核裁决（decide.rs:508-610） | 项目成员；无真人门 |
| /v1/pm/tasks/{task_id}/review-approvals | 读 | 项目内 |
| /api/v1/supervision/approvals/{id}/decision、/veto、/freeze、/unfreeze | 写（遗留监督端点） | **无校验+body.actor_id 自报（P0 R2）** |
| /api/v1/projects/{project_id}/notifications、/api/v1/events | 通知读/事件提交 | bearer |
| /v1/pm/decisions（GET ✅） | 读=决策中心读模型（approval_request⋈change_proposal） | bearer |

### 任务/执行/交付域
| 路径 | 写语义 |
|---|---|
| /v1/pm/tasks（GET ✅、POST——**REST 建任务不支持需求关联** task.rs:6-51）、/{task_id}（PATCH=乐观锁编辑 pm_write/task.rs:100-109） | 任务 CRUD |
| /v1/pm/tasks/{task_id}/dependencies、/{depends_on_task_id} | 依赖维护 |
| /v1/pm/tasks/{task_id}/criterion-verdicts、/delivery、/review-findings | 读（验收裁决/交付/findings） |
| /v1/pm/task-runs（GET ✅）、/{run_id}（GET ✅）、/{run_id}/cancel | Run 读+取消（project_lifecycle.rs:113-158） |
| /v1/pm/projects/{project_id}/delivery | 项目级交付聚合（17ca032） |
| /v1/pm/projects/{project_id}/command-center | 总控台聚合（I5-C，0034 索引） |
| /v1/pm/acceptance-overview | 验收聚合（T-070） |
| /v1/pm/artifacts | 产物列表（Diff 后端无数据源 pm_artifacts.rs:1-5） |

### 项目/仓库/导入/Discovery/租户/其他
- /v1/pm/projects（GET ✅）、/{id}/archive（写=I1 归档）、/{id}/members（+{principal_id}）、/{id}/owner-transfer（写）、/{id}/repositories、/{id}/ssh-key(s)、/{id}/imports（+apply/cancel/terminate/proposals——写=冷启动导入链）、/{id}/discovery/sessions（+findings/verification/questions/answers/defer/status——写=采集链）
- /v1/pm/repositories/{repository_id}（+archive/verify——写）
- /v1/pm/tenants/{tenant_id}/members（+{principal_id}、role、owner-transfer——写）
- /v1/pm/me（GET ✅）、/v1/pm/principals（+{id} PATCH ✅）、/v1/pm/devices（+bindings/checks/revoke——写=T-088 链）、/v1/pm/audit-logs（读）、/v1/pm/milestones（GET ✅）、/v1/pm/customers、/v1/pm/llm/providers（GET/PUT/test ✅）、/v1/pm/outbox（dead-letters/stats/replay——ops）、/v1/pm/events/stream（SSE）
- 平台面：/install（GET ✅）、/install/{tool}、/install/packages/...、/health（GET ✅）

## 2. MCP 工具全量清单（19 个，注册处=crates/pm-mcp/src/server.rs，tools/list 契约由 tests/tools.rs:408-431 钉死）

| # | 工具 | 行 | 写 | 权限 | S03 相关 |
|---|---|---|---|---|---|
| 1 | pm_submit_technical_design | :72 | 写(事件) | READ+DOCUMENT_WRITE+真人成员 | **是（设计提交=唯一入口，无设计生命周期）** |
| 2 | pm_get_submission_status | :93 | 否 | READ | 是 |
| 3 | pm_get_task_context | :113 | 否 | READ | 是（含最新需求修订上下文） |
| 4 | pm_list_tasks | :133 | 否 | READ | 是 |
| 5 | pm_submit_document | :155 | 写(Git) | READ+DOCUMENT_WRITE | 是（需求/决策直创） |
| 6 | pm_propose_document | :176 | 写(Git/提案) | 同上 | **是（设计/决策提案主通道）** |
| 7 | pm_confirm_document_proposal | :198 | 写 | 同上+require_human；身份=认证人（客户端 confirmed_by 丢弃 tools.rs:504） | **是——现存实现缺陷【A* 目标（S02-I6-A）=MCP 不提供正式确认入口，本工具不得解释为目标能力】** |
| 8 | pm_veto_document_revision | :218 | 写 | 同上（tools.rs:527） | 是——同类缺陷标注【与 A* ②相悖，待按 S02 治理线修复】 |
| 9 | pm_discovery_get | :239 | 否 | READ | 弱 |
| 10 | pm_discovery_get_context | :259 | 否 | READ | 弱 |
| 11 | pm_discovery_submit_findings | :279 | 写 | READ+TASK_UPDATE?(未分配 auth.rs:561) | 弱 |
| 12 | pm_submit_evidence | :299 | 写 | 同上 | 弱 |
| 13 | pm_discovery_submit_answers | :320 | 写 | 同上 | 弱 |
| 14 | pm_discovery_submit_proposal | :339 | 写 | 同上 | 是 |
| 15 | pm_discovery_complete | :361 | 写 | 同上 | 弱 |
| 16 | pm_claim_task | :381 | 写 | READ+TASK_UPDATE? | 参考（锁每仓基线） |
| 17 | pm_get_run_context | :404 | 否 | READ | 参考（RunContract 无需求视图） |
| 18 | pm_report_progress | :426 | 写 | 同上 | 参考 |
| 19 | pm_submit_result | :448 | 写 | 同上 | 参考（D4 逐仓校验） |

**无独立"审批/评审 MCP 工具"**；知识红线确认是唯一 MCP 化人工确认——**按 S02-I6-A A* 治理裁决，MCP 本不应承载正式确认能力，该现状属待修缺陷而非目标能力（R2）**。

## 3. REST/MCP 服务层共享与分叉

| 能力 | 共享点 | 分叉点 |
|---|---|---|
| 文档红线确认/veto | 同一 application service（confirm_redline_proposal/veto_revision） | MCP 丢弃客户端 confirmed_by 用认证人（tools.rs:502-504）；REST 透传 body.confirmed_by/vetoed_by（documents.rs:286,311）；**按 A*，MCP 侧入口整体为现存实现缺陷（目标=MCP 无正式确认入口）** |
| 审批裁决 | decide_approval 服务共享 | /v1/pm 用认证人（pm_writes.rs:130）；/api/v1/supervision 用 body.actor_id（routes.rs:464）；后者无租户/成员校验 |
| 需求定稿 | 无 MCP 入口 | REST 与命令执行器双实现语义分叉（DEF-04：state 前置/status 回写/审计口径不一致） |
| 权限码 | MCP 有硬编码表（auth.rs:562-568） | REST 零权限码校验 |
