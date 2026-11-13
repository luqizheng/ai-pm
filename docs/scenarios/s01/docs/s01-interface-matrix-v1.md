# S01 Interface Matrix V1.0

> 文档性质：S01-0 退出产物（总体计划 §3.1 W04、§11）
> 上位：`docs/s01-全局主菜单与信息架构设计基线/docs/s01-implementation-plan-v1.md`
> 代码基线：`master 2933db8`
> 核验范围：`apps/pm-api/src/`（全部 HTTP 路由）、`crates/pm-mcp/src/`、`apps/pm-worker`、`apps/ai-pm-worker`、`crates/pm-application/src/`
> 版本：V1.0　日期：2026-11-03

---

## 1. 前缀与装配事实

| 前缀 | 用途 | 定义位置 | 鉴权 |
|---|---|---|---|
| `/health` | 就绪探针 | `apps/pm-api/src/routes.rs:146` | 公开 |
| `/api/v1/...` | 事件摄取 + 监管 + 通知流 | `routes.rs:147-158` | 需鉴权 |
| `/v1/pm/...` | 业务端点（前端经典页契约） | 各模块 `router()` | 需鉴权 |
| `/install`、`/install/{tool}`、`/install/packages/*` | 执行端接入引导 / 技能包分发 | `onboarding.rs:27-28`、`skills_package.rs:57-63` | 公开 |
| `/mcp` | MCP streamable HTTP（18 工具） | `routes.rs:248`（`nest`） | 鉴权 + 权限码 + 真人成员门 |

**路由注册唯一入口**：`apps/pm-api/src/routes.rs:103-250` `build_router()`。
`crates/pm-application` / `pm-infrastructure` / `pm-mcp` **零 HTTP 路由**。
全仓**无 `unimplemented!()` / `todo!()` / 501 stub**——所有已注册端点都有实体 handler。

**鉴权链（外→内）**：`require_bearer`(auth.rs:190) → `resolve_principal`(auth.rs:225) → `require_active`(auth.rs:268) → handler；`/mcp` 内层另有 `attach_device_claim`(device_claim.rs:39) 与 `require_mcp_permission`(auth.rs:572)。

**放行路径**：`is_public_path` = `/health`、`/install`、`/install/*`（auth.rs:288-294）；self-service = `/v1/pm/me`；SSE 特例 = `/v1/pm/events/stream`（token-in-URL 仅限此一端点，auth.rs:551/628）。

**业务级可见性**：`project_visible`/`require_project_visible`(support.rs:126/146) 跨租户或非 `core.project_member` → **404**（不泄露存在性）；`require_task_project`(support.rs:161)；`customer_visible_to`(customers.rs:81)。

---

## 2. REST 端点全清单（98 个）

### 2.1 平台 / 摄取 / 监管（7）

| 方法 | 路径 | handler | 位置 | 领域 |
|---|---|---|---|---|
| GET | `/health` | `health_handler` | routes.rs:353 | platform |
| POST | `/api/v1/events` | `submit_event_handler` | routes.rs:357 | ingestion |
| POST | `/api/v1/supervision/veto` | `veto_handler` | routes.rs:399 | supervision |
| POST | `/api/v1/supervision/approvals/{id}/decision` | `decide_handler` | routes.rs:447 | supervision |
| POST | `/api/v1/supervision/freeze` | `freeze_handler` | routes.rs:495 | supervision |
| POST | `/api/v1/supervision/unfreeze` | `unfreeze_handler` | routes.rs:512 | supervision |
| GET | `/api/v1/projects/{project_id}/notifications` | `notifications_handler` | routes.rs:537 | notification |

### 2.2 Tenant / Principal / IAM（9）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/tenants/{tenant_id}/members` | `list_handler` | tenant_members.rs:53 |
| POST | `/v1/pm/tenants/{tenant_id}/members` | `add_handler` | tenant_members.rs:141 |
| GET | `/v1/pm/tenants/{tenant_id}/members/{principal_id}` | `detail_handler` | tenant_members.rs:96 |
| DELETE | `/v1/pm/tenants/{tenant_id}/members/{principal_id}` | `remove_handler` | tenant_members.rs:188 |
| PATCH | `/v1/pm/tenants/{tenant_id}/members/{principal_id}/role` | `set_role_handler` | tenant_members.rs:167 |
| POST | `/v1/pm/tenants/{tenant_id}/owner-transfer` | `owner_transfer_handler` | tenant_members.rs:210 |
| GET | `/v1/pm/me` | `me_handler` | principals.rs:27 |
| PATCH | `/v1/pm/principals/{principal_id}` | `update_handler` | principals.rs:47 |
| GET | `/v1/pm/principals` | `principals` | pm_lists.rs:55 |

### 2.3 Customer / Project / Task / Requirement（18）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/customers` | `list_customers` | customers.rs:96 |
| POST | `/v1/pm/customers` | `create_customer` | customers.rs:206 |
| GET | `/v1/pm/customers/{customer_id}` | `get_customer` | customers.rs:156 |
| PATCH | `/v1/pm/customers/{customer_id}` | `update_customer` | customers.rs:277 |
| GET | `/v1/pm/projects` | `projects` | pm_lists.rs:83 |
| POST | `/v1/pm/projects` | `create_project_handler` | pm_lists.rs:138 |
| GET | `/v1/pm/requirements` | `requirements` | pm_lists.rs:242 |
| GET | `/v1/pm/requirements/{id}/revisions` | `revisions` | pm_lists.rs:504 |
| POST | `/v1/pm/requirements/{id}/revisions` | `create_revision_handler` | pm_writes.rs:223 |
| POST | `/v1/pm/requirements/{id}/revisions/{rid}/approve` | `approve_revision_handler` | pm_writes.rs:235 |
| GET | `/v1/pm/tasks` | `tasks` | pm_lists.rs:285 |
| POST | `/v1/pm/tasks` | `create_task_handler` | pm_writes.rs:196 |
| PATCH | `/v1/pm/tasks/{task_id}` | `update_task_handler` | pm_writes.rs:209 |
| GET | `/v1/pm/milestones` | `milestones` | pm_lists.rs:328 |
| GET | `/v1/pm/approvals` | `approvals` | pm_lists.rs:370 |
| POST | `/v1/pm/approvals/{id}/decide` | `decide_handler` | pm_writes.rs:78 |
| GET | `/v1/pm/decisions` | `decisions` | pm_lists.rs:414 |
| GET | `/v1/pm/audit-logs` | `audit_logs` | pm_lists.rs:461 |

### 2.4 Task 依赖 / Run / Delivery / Acceptance（12）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/tasks/{task_id}/dependencies` | `list_handler` | task_dependencies.rs:82 |
| POST | `/v1/pm/tasks/{task_id}/dependencies` | `add_handler` | task_dependencies.rs:93 |
| DELETE | `/v1/pm/tasks/{task_id}/dependencies/{depends_on}` | `remove_handler` | task_dependencies.rs:111 |
| GET | `/v1/pm/task-runs` | `task_runs` | pm_runs.rs:63 |
| GET | `/v1/pm/task-runs/{run_id}` | `task_run_detail` | pm_runs.rs:171 |
| GET | `/v1/pm/tasks/{task_id}/delivery` | `list_handler` | pm_delivery.rs:27 |
| GET | `/v1/pm/acceptance-overview` | `overview_handler` | pm_acceptance.rs:43 |
| GET | `/v1/pm/tasks/{task_id}/review-findings` | `review_findings_handler` | pm_acceptance.rs:81 |
| GET | `/v1/pm/tasks/{task_id}/criterion-verdicts` | `criterion_verdicts_handler` | pm_acceptance.rs:130 |
| GET | `/v1/pm/tasks/{task_id}/review-approvals` | `review_approvals_handler` | pm_acceptance.rs:169 |
| GET | `/v1/pm/review-approvals` | `list_handler` | pm_reviews.rs:54 |
| POST | `/v1/pm/review-approvals/{id}/decision` | `decision_handler` | pm_reviews.rs:99 |
| GET | `/v1/pm/artifacts` | `artifacts` | pm_artifacts.rs:36 |

### 2.5 Repository / SSH Key / Repo Import（16）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/projects/{project_id}/repositories` | `list_handler` | pm_repositories.rs:137 |
| POST | `/v1/pm/projects/{project_id}/repositories` | `create_handler` | pm_repositories.rs:154 |
| PATCH | `/v1/pm/repositories/{repository_id}` | `update_handler` | pm_repositories.rs:170 |
| DELETE | `/v1/pm/repositories/{repository_id}` | `delete_handler` | pm_repositories.rs:186 |
| POST | `/v1/pm/repositories/{repository_id}/verify` | `verify_handler` | pm_repositories.rs:201 |
| POST | `/v1/pm/repositories/{repository_id}/analyze` | `analyze_handler` | pm_repositories.rs | **新增（2026-11-13 S1 手动增量分析）**：可选 `idempotency_key`；202 受理 / 409 进行中 / 422 无锚点；幂等键 `repo_analyze:{rid}:{commit}:{trigger}` |
| POST | `/v1/pm/repositories/{repository_id}/archive` | `archive_handler` | pm_repositories.rs:216 |
| GET | `/v1/pm/projects/{project_id}/ssh-key` | `get_key_handler` | pm_repositories.rs:231 |
| POST | `/v1/pm/projects/{project_id}/ssh-key` | `generate_key_handler` | pm_repositories.rs:246 |
| DELETE | `/v1/pm/projects/{project_id}/ssh-key` | `delete_key_handler` | pm_repositories.rs:261 |
| GET | `/v1/pm/projects/{project_id}/ssh-keys` | `list_keys_handler` | pm_repositories.rs:276 |
| POST | `/v1/pm/projects/{project_id}/ssh-keys` | `create_key_handler` | pm_repositories.rs:299 |
| DELETE | `/v1/pm/projects/{project_id}/ssh-keys` | `revoke_key_handler` | pm_repositories.rs:316 |
| GET | `/v1/pm/projects/{project_id}/imports` | `list_handler` | pm_imports.rs:119 |
| POST | `/v1/pm/projects/{project_id}/imports` | `start_handler` | pm_imports.rs | **请求体必填 `repository_ids`（N-3）** + 可选 `idempotency_key`（N-4） |
| GET | `/v1/pm/projects/{project_id}/imports/{import_id}` | `get_handler` | pm_imports.rs:151 |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/cancel` | `cancel_handler` | pm_imports.rs | 支持 `proposed` 态；body 可带 `reason`（写入审计） |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/terminate` | `terminate_handler` | pm_imports.rs | **新增（I6-B-05）**：`proposed` 态显式终止，释放 active 名额 |
| GET | `/v1/pm/projects/{project_id}/imports/{import_id}/proposals` | `proposals_handler` | pm_imports.rs:183 |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/apply` | `apply_handler` | pm_imports.rs:197 |

### 2.6 Discovery（8）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/projects/{project_id}/discovery/sessions` | `list_handler` | pm_discovery.rs:148 |
| POST | `/v1/pm/projects/{project_id}/discovery/sessions` | `start_handler` | pm_discovery.rs:167 |
| GET | `/v1/pm/projects/{project_id}/discovery/sessions/{session_id}` | `get_handler` | pm_discovery.rs:201 |
| POST | `.../sessions/{session_id}/status` | `status_handler` | pm_discovery.rs:222 |
| POST | `.../sessions/{session_id}/findings` | `add_finding_handler` | pm_discovery.rs:251 |
| POST | `.../findings/{finding_id}/verification` | `verification_handler` | pm_discovery.rs:291 |
| POST | `.../sessions/{session_id}/questions` | `add_question_handler` | pm_discovery.rs:328 |
| POST | `.../questions/{question_id}/answers` | `answer_handler` | pm_discovery.rs:359 |
| POST | `.../questions/{question_id}/defer` | `defer_handler` | pm_discovery.rs:387 |

### 2.7 Knowledge（13）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/documents` | `list_handler` | documents.rs:82 |
| GET | `/v1/pm/documents/{document_id}` | `get_handler` | documents.rs:96 |
| GET | `/v1/pm/documents/search` | `search_handler` | documents.rs:62 |
| GET | `/v1/pm/documents/proposals` | `list_proposals_handler` | documents.rs:212 |
| POST | `/v1/pm/documents/proposals` | `submit_proposal_handler` | documents.rs:177 |
| POST | `/v1/pm/documents/proposals/{proposal_id}/confirm` | `confirm_redline_handler` | documents.rs:232 |
| POST | `/v1/pm/documents/revisions/{revision_id}/veto` | `veto_revision_handler` | documents.rs:257 |
| POST | `/v1/pm/documents/revisions/{revision_id}/reindex` | `reindex_handler` | documents.rs:359 |
| POST | `/v1/pm/documents/reindex` | `reindex_project_handler` | documents.rs:394 |
| GET | `/v1/pm/documents/{document_id}/references` | `list_references_handler` | documents.rs:283 |
| POST | `/v1/pm/documents/{document_id}/references` | `add_reference_handler` | documents.rs:304 |
| GET | `/v1/pm/documents/{document_id}/index-state` | `index_state_handler` | documents.rs:338 |
| POST | `/v1/pm/requirements/revisions/{revision_id}/migrate` | `migrate_requirement_handler` | documents.rs:456 |

### 2.8 Devenv / Install / LLM / Outbox / SSE（12）

| 方法 | 路径 | handler | 位置 |
|---|---|---|---|
| GET | `/v1/pm/devices` | `list_handler` | pm_devices.rs:108 |
| POST | `/v1/pm/devices` | `register_handler` | pm_devices.rs:80 |
| GET | `/v1/pm/devices/{device_id}` | `detail_handler` | pm_devices.rs:122 |
| POST | `/v1/pm/devices/{device_id}/bindings` | `bind_handler` | pm_devices.rs:133 |
| DELETE | `/v1/pm/devices/{device_id}/bindings/{binding_id}` | `unbind_handler` | pm_devices.rs:153 |
| POST | `/v1/pm/devices/{device_id}/checks` | `checks_handler` | pm_devices.rs:164 |
| POST | `/v1/pm/devices/{device_id}/revoke` | `revoke_handler` | pm_devices.rs:183 |
| GET | `/install` | `catalog_handler` | onboarding.rs:108 |
| GET | `/install/{tool}` | `guide_handler` | onboarding.rs:136 |
| GET | `/install/packages/release-manifest.json` | `manifest_handler` | skills_package.rs:67 |
| GET | `/install/packages/{file}` | `package_handler` | skills_package.rs:78 |
| GET | `/v1/pm/llm/providers` | `list_handler` | llm_configs.rs:52 |
| PUT | `/v1/pm/llm/providers/{provider}` | `update_handler` | llm_configs.rs:66 |
| POST | `/v1/pm/llm/providers/{provider}/test` | `test_handler` | llm_configs.rs:84 |
| GET | `/v1/pm/outbox/dead-letters` | `dead_letters_handler` | outbox_ops.rs:57 |
| POST | `/v1/pm/outbox/events/{event_id}/replay` | `replay_handler` | outbox_ops.rs:74 |
| GET | `/v1/pm/outbox/stats` | `stats_handler` | outbox_ops.rs:120 |
| GET | `/v1/pm/events/stream` | `stream_handler` | audit_stream.rs:119 |

---

## 3. MCP 工具全清单（18 个）

注册处：`crates/pm-mcp/src/server.rs:73-436`；函数层 `crates/pm-mcp/src/tools.rs`（1343 行）。挂载：`apps/pm-api/src/routes.rs:248`。

### 3.1 执行族

| # | 工具 | 参数 | 作用 | 注册 | 实现 |
|---|---|---|---|---|---|
| 1 | `pm_submit_technical_design` | project_id, task_ref, fixture, summary, idempotency_key? | 幂等提交技术设计事件，排队 AI 分析 | server.rs:73 | tools.rs:173 |
| 2 | `pm_get_submission_status` | project_id, event_id | 事件状态 + proposal + 分析 Run 状态 | server.rs:94 | tools.rs:241 |
| 3 | `pm_get_task_context` | project_id, task_ref | 任务字段 + 需求修订 + 里程碑 | server.rs:114 | tools.rs:298 |
| 4 | `pm_list_tasks` | project_id | 列 ready 任务 + 最新 queued run | server.rs:134 | tools.rs:337 |
| 5 | `pm_claim_task` | project_id, task_id, client_name, client_version? | Model A 显式领取，签发一次性 lease token，锁定每仓基线 | server.rs:361 | tools.rs:664 |
| 6 | `pm_get_run_context` | project_id, run_id, lease_token, lease_generation | 读完整执行契约（**不续租**） | server.rs:384 | tools.rs:696 |
| 7 | `pm_report_progress` | project_id, run_id, lease_token, lease_generation, stage, note?, blocked | 心跳 + 续租（严格向前） | server.rs:406 | tools.rs:715 |
| 8 | `pm_submit_result` | …, integration_status, repositories[], summary?, knowledge_proposals[] | 交付：commit 校验 + 落交付清单 + 任务 submitted | server.rs:428 | tools.rs:759 |

### 3.2 知识族（均需 `PM_MCP_DOCUMENT_WRITE`）

| # | 工具 | 作用 | 注册 | 实现 |
|---|---|---|---|---|
| 9 | `pm_submit_document` | 建 Git 支撑的知识文档 + revision | server.rs:156 | tools.rs:394 |
| 10 | `pm_propose_document` | 走质量门提案；红线提案挂起待人工确认，**不写 Git** | server.rs:177 | tools.rs:462 |
| 11 | `pm_confirm_document_proposal` | 人工确认红线提案 → commit Git | server.rs:199 | tools.rs:495 |
| 12 | `pm_veto_document_revision` | 否决窗口内回滚：Git revert + 指针回退 + 索引重建 | server.rs:219 | tools.rs:519 |

### 3.3 Discovery 族（6 个）

| # | 工具 | 作用 | 注册 | 实现 |
|---|---|---|---|---|
| 13 | `pm_discovery_get` | 读 session（findings/questions/proposals） | server.rs:240 | tools.rs:1027 |
| 14 | `pm_discovery_get_context` | 组装 `/pm-learn` 工作上下文 | server.rs:260 | tools.rs:1044 |
| 15 | `pm_discovery_submit_findings` | 提交 AI 发现 + 证据 + 仓库关联 | server.rs:280 | tools.rs:1102 |
| 16 | `pm_discovery_submit_answers` | 提交澄清问题 / 回传用户回答 | server.rs:300 | tools.rs:1138 |
| 17 | `pm_discovery_submit_proposal` | 知识变更提案（镜像到 `discovery_proposal`） | server.rs:319 | tools.rs:1196 |
| 18 | `pm_discovery_complete` | 一轮采集标为 COMPLETED / REVIEW_REQUIRED | server.rs:341 | tools.rs:1319 |

### 3.4 MCP 权限门禁

| 层 | 位置 | 内容 |
|---|---|---|
| ① HTTP 权限码 | `apps/pm-api/src/auth.rs:556-616` | 全部 `/mcp` 需 `PM_MCP_READ`；5 个写工具另需 `PM_MCP_DOCUMENT_WRITE`。**`PM_MCP_TASK_UPDATE` 尚未分配** |
| ② 应用层成员门 | `crates/pm-mcp/src/caller.rs:31-111` | `ensure_member`（真人成员 → `require_project_member`，非成员 → NotFound）；`authorize`（成员 + `resolve_project_executor`） |
| ③ Host 白名单 | `apps/pm-api/src/mcp_hosts.rs:18-37` | DNS rebinding 防护 |

**权限码表覆盖缺口（缺陷 D-11）**：`MCP_TOOL_PERMISSIONS`（auth.rs:562-568）只列 5 个工具；`pm_discovery_submit_findings`、`pm_claim_task`、`pm_submit_result`、`pm_report_progress`、`pm_discovery_complete` **仅受 `PM_MCP_READ` 约束**——写能力与读能力同码。S01 须按工具重划权限码。

### 3.5 MCP 直写权威状态的路径（须保持经受守卫状态机）

| 工具 | 写入对象 | 状态机 |
|---|---|---|
| `pm_claim_task` | `task_run` 租约/基线 + `task.status → claimed` | ✅ `apply_transition`（claim.rs:395、605） |
| `pm_report_progress` | `task_run` 续租/心跳 + `task.status → running` | ✅ `apply_task_transition`（run_context.rs:637-647） |
| `pm_submit_result` | 交付清单 + `task.status → submitted` | ✅ `apply_task_transition`（delivery/mod.rs:738-742） |
| `pm_discovery_complete` | `discovery_session.status` | ⚠️ **自建状态机**（session.rs:161-176），不共用 Task 状态机（缺陷 D-10） |
| `pm_confirm_document_proposal` / `pm_veto_document_revision` | `ai.document_proposal` / `document_revision` | ✅ 知识域闸门 + 项目级 advisory lock |

---

## 4. Worker 职责清单

两个进程共享同一 outbox 表：`apps/pm-worker/src/main.rs`（41 行，只跑 outbox）、`apps/ai-pm-worker/src/main.rs`（286 行，6 条循环）。Handler 装配入口 `crates/pm-infrastructure/src/outbox_handlers.rs:283-291`；启动时目录一致性校验 `outbox_worker.rs:92-98`（不一致即 panic）。

| 职责 | 位置 | 要点 |
|---|---|---|
| Outbox 分发 | pm-worker main.rs:33-40；ai-pm-worker main.rs:106-121 | Handler：`TaskReady` / `DeliverySubmitted` / `RunCompleted`；目录 `pm-application/src/outbox.rs:101-130` |
| **原子领取** | `run_dispatch/claim.rs:258+`（`claim_core`） | 候选查询 `FOR UPDATE OF r SKIP LOCKED`(claim.rs:272-300)；任务行锁 `FOR UPDATE`(346-353)，锁序 run→task。`claim_once`(无人值守，受 `AI_PM_RUN_DISPATCH` 开关) 与 `pm_claim_task`(Model A) 共用 `claim_core` |
| Run 启动编排 | `run_executor.rs:43-190` | session 前置持久化 → spawn → claimed→running |
| Run 轮询/兜底续租 | `run_executor.rs:198+`、`poll_active_runs` | 四信号判定 + 兜底续租 |
| 孤儿 reconcile | `run_executor.rs:395` | 三重证据判活（pid 文件/目录痕迹/Job·pgid）；无法确证返回 `Uncertain`，**不静默回收** |
| 终态僵尸清理 | `run_executor.rs:451` | — |
| 失败判定作业 | ai-pm-worker main.rs:152-167 → `review/failure.rs` | 谓词强制 `run_kind='delivery'` |
| Review 领取 | ai-pm-worker main.rs:139-146 → `review/worker.rs:421/75/129` | `FOR UPDATE SKIP LOCKED` 条件 UPDATE |
| 后台作业 `ai_analysis` | ai-pm-worker main.rs:224-231、252-286 | `job_queue::claim` 用 `FOR UPDATE OF job SKIP LOCKED` + 原子租约 |
| 后台作业 `repo_import` | ai-pm-worker main.rs:80-99、196-249 | clone → 事实采集 → AI 草案 → 落 `repo_import_source` |
| Review 卡死回收 | `review/worker.rs:89-127` | 超预算 → `timed_out` + 排重试 |
| 租约过期回收 | `run_dispatch/lease.rs:61-91`（`isolate_started_run`）、`94-114`（`requeue_with_backoff`） | 领取与回收同一路径，无独立 reaper |

### 4.1 租约事实

| 项 | 内容 | 位置 |
|---|---|---|
| DB 字段 | `lease_owner`、`lease_expires_at`、`heartbeat_at`(0011:33-35)、`executor_session`(0011:40)、`lease_generation`、`deadline_at`、`last_agent_heartbeat_at`(0023:8-17) | — |
| Token | 64 hex；sha256 小写 hash；**明文只返回一次** | run_context.rs:25-75 |
| 过期判定 | `lease_live = (lease_expires_at IS NOT NULL AND lease_expires_at > now())` | run_context.rs:101 |
| 校验链 | 行存在 → hash 严格相等 → `lease_generation` 相等 → `lease_live` | run_context.rs:157-175 |
| 续租 | 严格向前（`now() + interval`） | run_context.rs:470-500 |
| 下发 | `AI_PM_LEASE_TOKEN` / `AI_PM_LEASE_GENERATION` 仅经环境变量，禁落 pid 文件/日志 | codex_executor.rs:344-350；port.rs:141 |

### 4.2 Codex 执行适配器

| 项 | 状态 |
|---|---|
| 适配器 | `crates/pm-infrastructure/src/codex_executor.rs`（约 1050 行）`CodexExecutor`(:42) —— **已实现** |
| 环境装配 | `AI_PM_CODEX_BIN`/`AI_PM_CODEX_ARGS`/`AI_PM_RUN_KILL_GRACE_SECS`/`AI_PM_RUN_LOG_MAX_BYTES`(:89-101, :39) |
| Worker 接线 | `AI_PM_RUN_EXECUTOR=codex|mock`；缺省 `NoopExecutor`（port.rs:243） |
| CLI 调用 | `tokio::process::Command::new(executable).args(arguments)`，**不注入内置子命令**(:469-490) |
| 启动剖面 | `CodexLaunchProfile`(:297-309)：executable/arguments/working_dir/env/mcp_config/instruction_file/approval_policy/sandbox_policy/timeout |
| MCP 配置 | `AI_PM_MCP_URL` → `<run_root>/mcp.json`，凭据走环境变量不进文件(:353-364) |
| Instruction 文件 | `<run_root>/instruction.md` —— **仅 3 步**：`pm_get_run_context` / `pm_report_progress` / `pm_submit_result`(:367-380) |
| 工作区 | 逐仓 checkout + `validate_workspace_path` 二次校验(port.rs:73)；镜像仓跨进程锁(:124-140) |
| 进程隔离 | Windows Job Object(`KILL_ON_JOB_CLOSE`) / Unix process group(:495-510) |
| **bootstrap** | ❌ **无**。仓库内唯一 `bootstrap` 语义是"项目初始化"（与执行器无关）；执行端只有 `skills/pm-ai-bootstrap/SKILL.md`（技能包，非代码）。无子命令封装、无参数契约校验、无版本/能力探测 |

### 4.3 Knowledge Coordinator 位置（**不在 Worker 内**）

`crates/pm-application/src/knowledge_write_coordinator.rs`（280 行，在 **pm-application** crate）。Worker 路径**不触碰**知识写入协调。

| 接口 | 位置 | 作用 |
|---|---|---|
| `acquire(pool, project_id, timeout)` | :74 | project 级会话级 advisory lock（键 `ai.project_knowledge_write:{project_id}`，:68）；超时 → `ProjectWriteBusy`（默认 10s，:27）；连接 detach 不放回池(:96-98) |
| `ProjectWriteGuard` + `Drop` | :33-67 | 显式 `pg_advisory_unlock` + 丢弃连接 |
| `claim_operation` / `complete_operation` / `find_operation` | :153 / :202 / :225 | 幂等账本（`ai.knowledge_write_operation`） |
| `current_head` / `check_base_revision` | :241 / :268 | 乐观并发校验（不一致 → 409） |

调用点：`knowledge.rs:297-394`、`knowledge_proposal/submit.rs:53/281`、`knowledge_proposal/veto.rs:19`。

---

## 5. REST / MCP / Worker 职责矩阵（总体计划 §11 的现实映射）

| 领域 | REST（实际） | MCP（实际） | Worker（实际） | 与总体计划 §11 的差异 |
|---|---|---|---|---|
| Tenant | ✅ 成员 CRUD + owner-transfer（9 端点） | — | — | 相符 |
| Principal | ✅ `/v1/pm/me`、`/v1/pm/principals` | 上下文注入 `caller.rs` | — | 相符 |
| Project | ⚠️ 仅列表 + 创建；**无单项目 GET、无归档端点** | 只读上下文 | 后台作业 | **缺归档入口（I1-D 依赖）** |
| Repository | ✅ 6 端点（列表/创建/更新/删除/验证/归档）；**缺单仓 GET** | 授权仓库上下文（`pm_get_run_context`） | 验证、分析 | 相符（补单仓 GET） |
| Task | ✅ 列表/创建/更新/依赖 | ✅ 读上下文 + 写进度/结果 | 调度 | 相符 |
| Run | ⚠️ **纯只读 2 个 GET** | ✅ claim/context/progress/result | ✅ 生命周期管理 | 相符（写侧只在 MCP，是设计而非缺口） |
| Discovery | ✅ 9 端点 | ✅ 6 工具 | ❌ **无分析作业** | **Worker 侧缺 Discovery 分析执行** |
| Evidence | ❌ **无实体、无端点** | ⚠️ 仅内嵌于载荷 | ⚠️ 仅交付清单 | **缺口（缺陷 D-5/D-12）** |
| Knowledge | ✅ 13 端点 | ✅ 4 写工具 | ❌ 不参与 | 相符（Coordinator 在 application 层） |
| Delivery | ⚠️ 任务级 + `acceptance-overview`；**无项目级聚合** | 提交交付证据 | 聚合处理 | **缺项目级（缺陷 D-13）** |
| Policy | ❌ 无管理端点（YAML 静态加载） | — | — | **缺口，但列为延后 C-1** |
| Git 仓库访问 | ✅ **无独立端点**（V1.1 简化：访问由 Task 范围 + 仓库状态决定） | ❌ 无（随 Run 契约下发仓库清单） | ✅ 执行器物化仓库前按快照过滤 | **已闭合（2026-11-04，V1.1 简化裁决）** |

---

## 6. 缺口清单（S01 必补 vs 延后）

### 6.1 S01 必补

| # | 缺口 | 事实 | 归属 |
|---|---|---|---|
| ① | **Project 成员授权无写入路径** | `core.project_member` 全仓零 INSERT（仅测试与 `bootstrap.rs:414`）；`support.rs:135`/`pm_lists.rs:99`/`pm_runs.rs:87`/`pm_artifacts.rs:54` 只读 | S01-W13（+0030） |
| ② | ~~**Git 能力授权零基础**~~ **已闭合（简化）** | 原缺口：无权限枚举、无权限字段。**2026-11-04 用户简化裁决**（「不区分，都可以读写」）→ 不再引入能力枚举与授权表；访问改由 Task 仓库范围 + 仓库 `verified` 决定，执行期以 Run 快照强校验 | I3-B~I3-E（+0032/0033） |
| ③ | **无 Evidence 实体与回传通道** | `discovery_finding.evidence` 为 JSONB 数组（0025:61）；MCP 侧 `pm_submit_result.repositories[].tests[]`（tools.rs:599-605）与 `pm_discovery_submit_findings.findings[].evidence`（868-883）内嵌 | I6-B-04（+0031） |
| ④ | **无项目级 Delivery 聚合** | `pm_delivery.rs:21-25` 仅任务级；`pm_acceptance.rs:43` 的 `acceptance-overview` 已是项目级聚合但只含汇总计数 | S01-W60（+0034 或视图） |
| ⑤ | **MCP 权限码表覆盖不足** | `auth.rs:562-568` 仅 5 工具；写工具与读工具同码 | I6-B-05 |
| ⑥ | **单对象 GET 缺失** | `GET /v1/pm/projects/{id}`、`/tasks/{id}`、`/repositories/{id}` 均不存在（列表端点存在） | S01-W20/W21/S01-W61 |
| ⑦ | **Project 归档/Project 成员端点缺失** | I1-D 依赖归档入口；`core.project.status` 的**消费者为零**（`smark-pm-s01-v1-3-revision-and-i1-verification-已经过审.md` §二） | I1-D（+I1-A） |
| ⑧ | **Discovery 无 Worker 分析作业** | 唯一 clone/采集链在 Import 域（`lifecycle.rs:356/384`），facts 落 `repo_import_source`，**不落 Discovery** | I6-B-02 |
| ⑨ | **Codex bootstrap 层缺失** | 无子命令封装/参数契约/版本探测；instruction.md 仅 3 步，无「回传证据」步骤 | T-090 |
| ⑩ | **Discovery Session 与 Task 两套状态机并行** | `discovery/session.rs:161-176` vs Task 13 态，无交叉校验 | I6-B-05 |
| ⑪ | **MCP 权限码只覆盖 5 个写工具**（D-11） | lmms 侧只有 `PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE` / `PM_MCP_TASK_UPDATE` 三个码，无发现族专用码；单仓收紧会打断现网执行链路 | **跨仓协同**（lmms 发码 → ai-pm 收紧 → 回归），非单仓可决 |

### 6.2 明确延后

| # | 缺口 | 理由 |
|---|---|---|
| ① | Policy 管理 REST 端点 | `delivery.policy_scope` 表存在但策略仅从 `policies/ai-pm-ai-lead-v2.yaml` 加载（`routes.rs:110`）；S01 不要求在线治理 |
| ② | Notification 写侧（已读/订阅） | 与 S01 主链路无关 |
| ③ | Tenant 资源自身端点（`GET /v1/pm/tenants/{id}`、租户列表） | 当前调用方从 JWT `tid` 取租户，无需列举 |
| ④ | `GET /v1/pm/repositories`（全局仓库列表） | 仓库天然按 Project 归属 |

---

## 7. 依赖关系核对结论（总体计划 §12.2 串行项的真实性）

| 串行项 | 核验结论 |
|---|---|
| 身份上下文 → 授权接口 | ✅ 真实。`resolve_principal`/`require_active`/`ensure_member` 三层已就位，缺的是 Project 级成员数据（缺口①） |
| Project/Repository 数据模型 → 多仓库执行 | ✅ 真实。`project_repository`/`task_repository`/`task_run_repository` 三表已成型，缺的是 Git 能力维度（缺口②） |
| Task/Run 状态机 → 调度器 | ✅ 真实且已遵守。`claim_core` 锁序 run→task，MCP 写侧全经 `apply_task_transition` |
| MCP 契约 → 执行端接入 | ✅ 真实且已遵守。18 工具已固定，`instruction.md` 三步即契约 |
| Source/Attempt 模型 → Evidence 复合外键 | ✅ 真实且**硬阻塞**（缺陷 D-2：source 无组合唯一键，组合 FK 无法建立） |
| Knowledge Proposal 规则 → 发布集成 | ✅ 真实且已遵守。`knowledge_write_coordinator` + `base_revision` 乐观并发已就位 |
| Migration 审查 → 依赖它的代码合并 | ✅ 真实。0031 是唯一多任务共享变更（I6-B-02/04/07/08） |

**无「无法解释的关键接口依赖」**（实施计划 §10 退出条件之一，成立）。
