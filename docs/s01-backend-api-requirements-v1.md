# S01 前端落地 · 后端接口需求清单 V1.0

> **文档编号**：S01-FE-BE-01
> **文档性质**：后端接口需求（给后端同事的实现依据）
> **上游依据**：`docs/ui/s01/s01-01~s01-04` 四份 UX 规范（已冻结）+ 本文 §0 代码事实核验
> **版本**：V1.0
> **日期**：2026-11-04
> **分支**：`feat/s01-frontend-design`（worktree `D:/kudi-project/templates/ai-pm-s01-frontend`）
> **前端契约基线**：`feat/s01-implementation` @ `63200c2`

---

## 0. 阅读指引与代码事实

本文只列**后端需新增或需改造**的部分。前端将按本文契约并行开发，**契约变更须先改本文再改实现**。

### 0.1 已有端点（**不要重复实现**）

以下能力后端已具备，前端直接复用：

| 能力 | 端点 | 位置 |
|---|---|---|
| 项目列表 | `GET /v1/pm/projects` | `pm_lists.rs:32` |
| 项目创建（基础） | `POST /v1/pm/projects` | `pm_lists.rs:138` |
| 仓库 CRUD | `GET/POST /v1/pm/projects/{pid}/repositories`、`PATCH/DELETE /v1/pm/repositories/{rid}` | `pm_repositories.rs:29` |
| 仓库核验 | `POST /v1/pm/repositories/{rid}/verify` | `pm_repositories.rs:45` |
| 仓库归档 | `POST /v1/pm/repositories/{rid}/archive` | `pm_repositories.rs:50` |
| 项目 SSH 密钥 | `GET/POST/DELETE /v1/pm/projects/{pid}/ssh-key(s)` | `pm_repositories.rs:52,57` |
| 项目成员授权 | `GET/POST /v1/pm/projects/{pid}/members`、`DELETE .../members/{principal_id}` | `project_members.rs:35` |
| Owner 转移 | `POST /v1/pm/projects/{pid}/owner-transfer` | `project_members.rs:43` |
| 项目 Goal | `GET /v1/pm/projects/{pid}/goal` | `routes.rs:216` |
| 文档提案/确认 | `GET/POST /v1/pm/documents/proposals`、`POST .../proposals/{id}/confirm` | `routes.rs:225,230` |
| 认知访谈（Discovery） | `/v1/pm/projects/{pid}/discovery/sessions/*` | `pm_discovery.rs:27` |
| 项目总控台聚合 | `GET /v1/pm/projects/{pid}/command-center` | `command_center.rs` |

### 0.2 关键既有约束（实现时必须遵守）

| 约束 | 事实 | 出处 |
|---|---|---|
| 建项自动落编制 | `bootstrap_project_owned` 写真人调用者为 `project_member(role='owner')`，并落 5 编制 | `bootstrap.rs:121,241` |
| `core.project.code` | `TEXT NOT NULL UNIQUE`（全局唯一，非项目内唯一） | `0001_core.sql:22` |
| `customer_id` | `core.project.customer_id` **NOT NULL** → 建项必须带客户 | `0001_core.sql:20` |
| Repository role | 现为 `main/docs/service/infra` CHECK | `0008:23-24` |
| `project_member.role` | 现为 `owner/member/ai_pm/architect/developer/tester` CHECK | `0030` |
| 幂等键惯例 | 字段名 `idempotency_key`，冲突返回 409 `idempotency_conflict` | `pm_imports.rs:158`、`routes.rs:389` |
| 错误体 | 业务错误体两种并存：`{message}`（pm_* 系）与 `{error}`（routes.rs 系） | `routes.rs:5-27` |
| MCP 权限码 | 仅 `PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE` / `PM_MCP_TASK_UPDATE` 三个 | `auth.rs:556,563-567` |
| 迁移号 | 已用到 `0035_s01_run_cleanup.sql`；`0029` 为空号 | `migrations/` |

### 0.3 权限码缺口（**需先裁决**）

S01-01 要求「正式确认必须具备独立 Approver 授权，不能由普通项目成员身份自动获得」，S01-02/03 要求 Owner 与授权成员分级。

**现有 `project_member.role` 只有 `owner/member` 两档真人角色**（`0030`），**无法表达「可读但不可确认基线」「可管仓库但不可确认基线」**。当前 lmms 只签发 3 个 MCP 权限码，REST 侧未设独立权限码体系。

**待你裁决（前端按最终结果实现，先给推荐）**：

| 方案 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A（推荐）** | REST 侧按 `project_member.role` 判定：`owner` 可确认基线，`member` 只读；**不新增权限码** | 零跨仓协同，今天就能做 | 粒度粗，无法表达「owner 之外的可信确认人」 |
| B | 新增 `core.project_member.role` 取值 `approver` | 表达力强 | 需改 CHECK 约束 + 前端角色下拉 + 授权流程 |
| C | 新增独立 `core.project_approver` 授权表 | 表达力最强，与 S02-I6-A 方向一致 | 表更多，且要与 S02 并行批的 Approver 设计合并（避免建两套） |

> ⚠️ **C 与 S02 冲突风险**：`docs/s02-i6a-permission-foundation-code-facts-v1.md` 已记录「项目级显式授权机制不存在，唯一先例 `git_capability_grant`」，S02 正在设计 Approver 授权表。**若你选 C，需与 S02 合并成同一张表**，否则会出现两套 Approver 机制。

**推荐 A 起步**：S01 首批把「确认权 = owner」作为可运行的最小闭环；S02 的 Approver 表落地后，S01-01 只需把判定函数从 `role == 'owner'` 换成查表，不动前端结构。

---

## 1. S01-02 项目创建 —— 后端需求

UX 依据：`docs/ui/s01/s01-02-project-creation-ux-v1-1.md`

### 1.1 B2-1 项目 Key 建议（新增）

```
GET /v1/pm/projects/suggest-code?name={project_name}
```

**响应** `200`：
```json
{ "code": "SMARK", "available": true, "conflict_code": null }
```

**规则**（UX §3.2）：
- 系统按名称生成建议（建议算法：取名称中字母/数字，全大写，截断 32 字符；中文名转拼音或取首字母——**算法由后端定，前端只展示结果**）。
- `available=false` 时 `conflict_code` 给出已占用的 code。
- 前端在用户输入 Name 后 debounce 调用，允许人工修改并重新校验。

### 1.2 B2-2 项目创建扩展（改造现有 `POST /v1/pm/projects`）

**现状**：请求体 `{code, name, description?, customer_id?}`；`code` 必填、`customer_id` 可选（不传走 bootstrap 隐式客户路径）；**无幂等**；**Owner 恒为调用者**。

**改造后请求体**：
```json
{
  "code": "SMARK",
  "name": "Smark PM",
  "description": "可选描述",
  "customer_id": "uuid",
  "owner_principal_id": "uuid",
  "project_type": "web_app",
  "business_domain": "研发管理",
  "expected_outcome": "可选",
  "initial_context": "可选",
  "idempotency_key": "client-generated-uuid"
}
```

**字段规则**：

| 字段 | 必填 | 规则 |
|---|---|---|
| `code` | ✅ | 沿用全局唯一；由 B2-1 建议 + 提交前再校验 |
| `name` | ✅ | 非空 |
| `customer_id` | ⚠️ 见下 | 现表 NOT NULL，若继续支持无客户建项须改列可空 |
| `owner_principal_id` | ❌ | 缺省 = 调用者；传值则须是同项目可授权成员 |
| `project_type` / `business_domain` / `expected_outcome` / `initial_context` | ❌ | 可选背景信息（UX §3.4） |

**必须实现**：
1. **幂等**：`idempotency_key` 重复 → 返回**首次创建结果**（同 `project_id`），HTTP 200，body 加 `"idempotent_replay": true`。冲突（同一 key 不同 payload）→ `409 {error:"idempotency_conflict"}`。
2. **Owner 显式化**：传 `owner_principal_id` 时，`project_member` 的 owner 角色给该人；调用者若非该人，仍需是成员，否则 `403`。
3. **Creator 与 Owner 分别记录**：调用者记为 creator（审计 `actor` 已覆盖），owner 记在 `project_member`。
4. **可选背景信息持久化**：落 `core.project` 新列 或 新表 `core.project_profile`（**建议新表**，避免 `core.project` 膨胀）。
5. **并发唯一性**：`code` 冲突 → `409 {error:"project_code_conflict"}`。

**响应** `201`：
```json
{
  "id": "uuid", "code": "SMARK", "name": "Smark PM",
  "customer_id": "uuid", "owner_principal_id": "uuid",
  "status": "planning", "revision": 1,
  "init_status": "not_started",
  "created_at": "...", "updated_at": "..."
}
```
> `init_status` 是 S01-02 §8 的 **Initialization 生命周期**，与 `project.status`（Active/Suspended/Archived）分离。

### 1.3 B2-3 初始化状态读写（新增）

```
GET  /v1/pm/projects/{project_id}/initialization
PATCH /v1/pm/projects/{project_id}/initialization
```

**GET 响应**：
```json
{
  "project_id": "uuid",
  "project_status": "planning",
  "init_status": "not_started",
  "current_baseline_id": null,
  "baseline_version": null,
  "repository_count": 0,
  "primary_repository_id": null,
  "cognition_run_count": 0,
  "pending_cognition_items": 0,
  "pending_baseline_proposals": 0,
  "updated_at": "..."
}
```
**`init_status` 五态**（UX §8.2）：`not_started` / `in_progress` / `waiting_for_human` / `blocked` / `completed`。

**PATCH 请求**：`{ "init_status": "in_progress", "blocked_reason": null }`
**约束**：只能由 `project_member` 写入；`completed` 需当前有生效基线，否则 `409 {error:"baseline_required"}`。

---

## 2. S01-03 仓库管理 —— 后端需求

UX 依据：`docs/ui/s01/s01-03-repository-management-ux-v1-1.md`

### 2.1 B3-1 Repository 角色与 Primary 语义（新增能力，**改造现有表**）

**冲突点**：UX 要求 role ∈ `{Primary, Supporting, Infrastructure, Test}` 且 **最多一个 Primary、可为零个**；现表 role ∈ `{main, docs, service, infra}`。

**推荐做法**（**需你确认**）：
- **方案 A（推荐）**：新建 `integration.project_repository_v2` 表，承载新语义；老表保留不动（历史数据零迁移，符合 AGENTS.md 规则 1 的"新项目直接改契约"精神——但老表有存量演示数据）。前端切到 v2 端点。
- **方案 B**：`ALTER TABLE` 改 CHECK 约束 + 加 `is_primary` 列 + `UPDATE` 回填映射（`main→Primary`、`docs→Supporting`、`service→Infrastructure`、`infra→Infrastructure`）。破坏性但表少。

> ⚠️ 若选 B，`main` 与 `infra` 都映射到 `Infrastructure` 会丢信息，**建议 `main→Primary`、`docs→Supporting`、`service→Supporting`、`infra→Infrastructure`**。**0029 是空号**，若走 B 建议占用 0029 还是 0036 请一并定。

**v2 表字段**（无论 A/B 都以此为准）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | UUID PK | |
| `project_id` | UUID FK → `core.project` | |
| `name` | TEXT | |
| `url` | TEXT | `UNIQUE (project_id, url)` |
| `provider` | TEXT | `github` / `gitlab` / `other` |
| `repo_owner` | TEXT | 组织/个人 |
| `role` | TEXT | `primary` / `supporting` / `infrastructure` / `test` |
| `default_branch` | TEXT | Git 仓库自身默认分支 |
| `selected_branch` | TEXT | **项目采用分支**（UX §6，与 default_branch 严格区分） |
| `selected_commit` | TEXT | **项目确认采用的 Commit SHA**（可空 = 未选定） |
| `connection_status` | TEXT | `pending`/`connected`/`failed`/`disconnected` |
| `verification_status` | TEXT | `pending`/`passed`/`warning`/`failed` |
| `last_verified_at` / `last_scanned_at` | TIMESTAMPTZ | |
| `revision` | BIGINT | 乐观锁 |
| `created_at` / `updated_at` | TIMESTAMPTZ | |

**约束**：
- **部分唯一索引**保证最多一个 Primary：
  `CREATE UNIQUE INDEX ... ON integration.project_repository_v2(project_id) WHERE role = 'primary'`
- `selected_commit` 非空时 `selected_branch` 必非空。

### 2.2 B3-2 仓库 CRUD 扩展（新增/改造）

```
GET    /v1/pm/projects/{project_id}/repositories        （改造：返回 v2 语义）
POST   /v1/pm/projects/{project_id}/repositories        （改造：v2 字段）
PATCH  /v1/pm/repositories/{repository_id}              （改造：role/分支/描述）
DELETE /v1/pm/repositories/{repository_id}              （改造：改逻辑断开）
POST   /v1/pm/repositories/{repository_id}/verify        （已有，扩展返回体）
POST   /v1/pm/repositories/{repository_id}/set-primary   （新增）
POST   /v1/pm/repositories/{repository_id}/disconnect    （新增）
POST   /v1/pm/repositories/{repository_id}/restore       （新增）
POST   /v1/pm/repositories/{repository_id}/update-version（新增：人类确认后更新采用版本）
```

**关键语义**：

| 端点 | 语义 | UX 依据 |
|---|---|---|
| `set-primary` | 将某仓库设为 Primary；**原 Primary 自动降为 supporting**；同事务 | §3.2 |
| `disconnect` | **逻辑断开**，不删行、不删历史 Run / Baseline 引用；`connection_status='disconnected'` | §8.3 |
| `restore` | 恢复关联，**必须重新核验权限与版本** | §8.1 |
| `update-version` | 请求体 `{selected_branch, selected_commit, reason}`；**只更新采用版本，不动正式基线** | §6.2 |
| `DELETE` | 语义等同 disconnect（保留历史） | §8.3 |

**`verify` 扩展返回体**（现状仅 `{repository_id, status, default_branch, head_commit_sha, error_code, error_detail}`）：
```json
{
  "repository_id": "uuid",
  "status": "verified",
  "verification_status": "passed",
  "default_branch": "main",
  "head_commit_sha": "abc123",
  "selected_branch": "main",
  "selected_commit": "abc123",
  "head_changed": true,
  "checks": [
    { "code": "repository_identity", "status": "passed" },
    { "code": "access_permission", "status": "passed" },
    { "code": "branch_availability", "status": "passed" },
    { "code": "commit_availability", "status": "warning" },
    { "code": "repository_metadata", "status": "passed" }
  ],
  "verified_at": "..."
}
```
**`head_changed` 是 UX §6.2 的核心**：Branch HEAD 变化**只提示，不自动更新** `selected_commit`。

**核验项**（UX §7）固定 5 项：`repository_identity` / `access_permission` / `branch_availability` / `commit_availability` / `repository_metadata`；状态 `pending`/`passed`/`warning`/`failed`。

### 2.3 B3-3 Provider 授权（新增）

```
GET  /v1/pm/projects/{project_id}/git-providers        （列出可授权的 provider 与当前授权状态）
POST /v1/pm/projects/{project_id}/git-providers        （发起授权，返回授权 URL）
POST /v1/pm/git-providers/{provider}/callback          （OAuth 回调）
DELETE /v1/pm/projects/{project_id}/git-providers/{provider}  （撤销授权）
```

**授权表** `integration.git_provider_authorization`：
| 字段 | 说明 |
|---|---|
| `project_id` / `provider` | 联合唯一 |
| `scopes` | TEXT[]，如 `["repo:read"]`；**读写权限必须分离** |
| `access_token` / `refresh_token` | **加密存储，永不回显**（前端只拿 `scopes` 与 `status`） |
| `status` | `active` / `expired` / `revoked` |
| `expires_at` | 授权失效时前端显示「重新授权」入口 |

**约束**：
- **凭证不明文回显**：`GET` 响应只含 `provider`/`scopes`/`status`/`expires_at`，**不含 token**。
- 仓库接入成功**不等于**授予 Agent 写权限——`scopes` 不含写权限时，AI 侧只能读。

---

## 3. S01-04 AI-PM 认知 —— 后端需求

UX 依据：`docs/ui/s01/s01-04-ai-pm-cognition-ux-v1-1.md`

### 3.1 B4-1 Cognition Run（新增）

```
GET  /v1/pm/projects/{project_id}/cognition/runs
GET  /v1/pm/projects/{project_id}/cognition/runs/{run_id}
POST /v1/pm/projects/{project_id}/cognition/runs            （人工启动正式分析）
POST /v1/pm/projects/{project_id}/cognition/runs/{run_id}/cancel
```

**Run 字段**（UX §2.4）：
`id` / `project_id` / `status` / `scope`（分析范围 JSON）/ `repository_baseline`（各仓 branch+commit）/ `inputs`（输入资料）/ `capability_version` / `item_count` / `started_at` / `completed_at` / `error_message`

**Run 状态**：`pending` / `collecting` / `analyzing` / `completed` / `partially_completed` / `failed` / `cancelled`

**关键约束**：
- **人工启动**：只有 REST `POST` 可创建 Run；**MCP 不暴露启动工具**（UX §11「AI 不得自行启动」）。
- 历史 Run 不删除、不覆盖（UX §10.2）。

### 3.2 B4-2 Cognition Item（新增）

```
GET  /v1/pm/projects/{project_id}/cognition/items?run_id=&status=&domain=&page=&size=
GET  /v1/pm/projects/{project_id}/cognition/items/{item_id}
PATCH /v1/pm/projects/{project_id}/cognition/items/{item_id}    （人类处理）
```

**Item 字段**（UX §4.5）：
`id` / `run_id` / `domain`（6 领域之一）/ `content` / `evidence`（数组）/ `evidence_type`（`code_fact`/`doc_fact`/`ai_inference`）/ `confidence` / `has_conflict` / `status` / `change_type`（`add`/`modify`/`retire`/`relationship`/`conflict`）/ `related_knowledge_id` / `ai_rationale` / `decided_by` / `decided_at` / `decision_note`

**6 个认知领域**（UX §3）：`project_identity` / `repository_understanding` / `domain_understanding` / `architecture_understanding` / `development_convention` / `project_evolution`

**Item 状态**：`proposed` / `confirmed` / `rejected` / `investigation_required` / `deferred` / `superseded`

**PATCH 动作**（UX §6.1 逐项确认，**无批量全选接口**）：
```json
{ "action": "confirm", "content": "修改后的正文", "note": "确认意见" }
```
`action` ∈ `confirm` / `edit_and_confirm` / `reject` / `request_investigation` / `defer`

**约束**：
- `confirm` → 生成 Knowledge 新版本；`edit_and_confirm` → 用提交的 `content` 生成。
- `reject` 保留记录与证据，**不删**（UX §6.4）。
- 每次操作记录操作人/时间/类型/确认前后内容/对应 Knowledge 版本（UX §6.3）。
- **批量操作**：UX §6.2 允许但约束严（不得默认全选、冲突条目不可混入）。**推荐不提供后端批量端点**，前端只做「用户显式勾选后逐条提交」。

### 3.3 B4-3 Project Knowledge（新增）

```
GET  /v1/pm/projects/{project_id}/knowledge?category=&status=&page=&size=
GET  /v1/pm/projects/{project_id}/knowledge/{knowledge_id}
GET  /v1/pm/projects/{project_id}/knowledge/{knowledge_id}/history
PATCH /v1/pm/projects/{project_id}/knowledge/{knowledge_id}/core    （设为/取消核心 Knowledge）
```

**Knowledge 字段**（UX §2.2）：
`id` / `project_id` / `category` / `content` / `source_references` / `confirmed_by` / `confirmed_at` / `version` / `status`（`active`/`superseded`/`retired`）/ `supersedes` / `is_core`

**约束**：
- 修改**只能通过变更提案**（UX §7），Knowledge 本身不可直接 PUT。
- 核心 Knowledge 的范围**必须人类确认**，AI 不得自行设置（UX §8.1）。

### 3.4 B4-4 认知准备（新增）

```
GET  /v1/pm/projects/{project_id}/cognition/preparation
```

**响应**：
```json
{
  "project_info_complete": true,
  "missing_project_fields": [],
  "repositories": [
    { "repository_id": "uuid", "authorized": true, "accessible": true,
      "collected_documents": 12, "missing": [] }
  ],
  "collected_count": 34,
  "missing_materials": [
    { "code": "no_repository", "message": "尚未接入仓库", "blocking": false }
  ],
  "analysis_scope": ["project_identity", "repository_understanding"]
}
```
**约束**：**无 Repository 时不阻断**（UX §12）；此端点只读，**不启动 Run**（UX §4.1「此阶段不启动正式的 Cognition Run」）。

### 3.5 B4-5 Agent Context 检查（新增，只读）

```
GET /v1/pm/projects/{project_id}/agent-context?task_id=&repository_id=
```

**响应**：UX §8.3 的四层结构
```json
{
  "base_context":      { "knowledge": [ {id, content, version, category} ] },
  "task_context":      { "knowledge": [] },
  "repository_context":{ "repository_id": "uuid", "branch": "main", "commit": "abc123", "knowledge": [] },
  "execution_context": { "run_id": null, "task_id": null, "state": "idle" },
  "generated_at": "..."
}
```
**约束**：每个条目带 `version` 与 `source`；**未经确认的 Cognition 不得进入**（UX §8.4）。

---

## 4. S01-01 基线确认 —— 后端需求

UX 依据：`docs/ui/s01/s01-01-project-baseline-confirmation-ux-v1.md`

### 4.1 B1-1 基线提案（新增）

```
GET  /v1/pm/projects/{project_id}/baseline/proposals
GET  /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}
POST /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}/revise    （要求修订）
POST /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}/reject    （拒绝）
POST /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}/regenerate （失败/过期后重新生成）
```

**Proposal 字段**：
`id` / `project_id` / `proposal_version` / `status` / `generation_reason` / `analysis_scope` / `sources[]` / `repositories[]`（含 `repository_name`/`repository_url`/`branch_name`/`commit_sha`）/ `content`（全文，按章节）/ `diff` / `risks[]` / `created_at`

**Proposal 状态**（UX §3.1）：`generating` / `pending_review` / `needs_revision` / `rejected` / `confirmed` / `expired` / `failed`

**约束**：
- `confirm` **只能由授权人类触发**，AI 不得自动置位。
- `expired` 不改变当前正式基线。
- `rejected` 保留记录不删除。
- 修订生成**新 Proposal Version**，历史版本保留。

### 4.2 B1-2 风险裁决（新增）

```
GET  /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}/risks
POST /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}/risks/{risk_id}/decide
```

**Risk 字段**：`risk_id` / `risk_level`（P0/P1/P2）/ `risk_title` / `risk_description` / `ai_status` / `human_decision` / `evidence_ref` / `decided_by` / `decided_at` / `decision_note`

**关键语义**（UX §4.3）：
- `ai_status`（AI 标记已解决）与 `human_decision`（人类已裁决）**是两个独立字段**，不可混用。
- **P0 未被人类裁决时，确认端点必须拒绝**（后端强制，前端只是禁用按钮）。
- 不提供「全部标记已解决」接口。

### 4.3 B1-3 正式确认（新增）

```
POST /v1/pm/projects/{project_id}/baseline/proposals/{proposal_id}/confirm
```

**请求体**（UX §4.4 + §7.2）：
```json
{
  "confirmation_note": "确认意见",
  "expected_current_baseline_id": "uuid-or-null",
  "expected_proposal_version": 3,
  "idempotency_key": "client-uuid"
}
```

**行为**：
1. 校验 P0 风险已全部人类裁决 → 否则 `409 {error:"unresolved_p0_risk", risks:[...]}`。
2. 校验 `expected_current_baseline_id` 与当前一致 → 否则 `409 {error:"baseline_conflict"}`（UX §7.2 明确错误码 `409 BASELINE_CONFLICT`）。
3. 校验 `expected_proposal_version` → 不一致 `409 {error:"proposal_version_conflict"}`。
4. 校验仓库基准未过期 → 过期 `409 {error:"baseline_expired"}`。
5. **同一事务**内：新建 `project_baseline` 版本 → 旧版本置 `superseded_at` → 更新 `project_init_state.current_baseline_id`。
6. 幂等：同 `idempotency_key` 重试返回**同一结果**（同 `baseline_id`），不重复建版本。

**响应** `201`：
```json
{
  "baseline_id": "uuid",
  "baseline_version": 1,
  "current_baseline_id": "uuid",
  "confirmed_by": "uuid",
  "confirmed_at": "...",
  "proposal_id": "uuid",
  "proposal_version": 3,
  "repository_baselines": [
    { "repository_id": "uuid", "branch": "main", "commit_sha": "abc123" }
  ],
  "confirmation_note": "..."
}
```

**失败反馈**（UX §4.4）：确认失败/冲突/过期/权限不足 **不得**返回「已生效」语义。HTTP 状态：409 冲突、403 权限不足、404 不存在。

### 4.4 B1-4 基线总览与历史（新增，只读）

```
GET /v1/pm/projects/{project_id}/baseline                        （当前生效基线总览）
GET /v1/pm/projects/{project_id}/baseline/history                （历史版本）
GET /v1/pm/projects/{project_id}/baseline/history/{baseline_id}  （版本详情 + 确认记录）
```

**总览响应**：
```json
{
  "current": {
    "baseline_id": "uuid", "version": 2, "confirmed_by": "uuid",
    "confirmed_at": "...", "content_summary": "...",
    "repositories": [ { "repository_id": "uuid", "branch": "main", "commit_sha": "abc123" } ]
  },
  "history": [ { "baseline_id": "uuid", "version": 1, "superseded_at": "..." } ],
  "pending_proposals": 1
}
```

**约束**：
- 当前生效基线**始终唯一**，全项目最多一个。
- 历史版本**只读、不可覆盖、不可删除**。
- 审核新提案时，当前基线仍有效且必须可见。

---

## 5. 新增/改动表汇总（迁移）

| 迁移号（待定） | 表 | 归属 |
|---|---|---|
| 0036 | `core.project_init_state` | Project↔Initialization 生命周期分离 |
| 0036 | `core.project_profile` | 可选背景信息（type/domain/outcome/context） |
| 0036 | `integration.project_repository_v2` | 多仓库 + Primary + Branch/Commit |
| 0036 | `integration.git_provider_authorization` | Provider OAuth 授权 |
| 0036 | `ai.project_baseline_proposal` | 基线提案 |
| 0036 | `ai.project_baseline` | 正式基线（版本化、不可覆盖） |
| 0036 | `ai.baseline_confirmation` | 确认记录（审计追溯） |
| 0036 | `ai.cognition_run` | 认知运行 |
| 0036 | `ai.cognition_item` | 认知条目 |
| 0036 | `ai.project_knowledge` | 正式知识 |
| 0037 | `core.project` 扩展列（若走 ALTER 方案） | `project_type` 等 |
| 0037 | `integration.project_repository` 扩展列（若走 ALTER 方案） | `is_primary`/`selected_branch`/... |

**建议**：全部新表集中到 **0036 一批**；仅当 §2.1 选方案 B（ALTER）时，破坏性变更单独占 **0037**。
> `0029` 是空号。若希望基线连续，也可把 0036 让给 ALTER、0037 给新表——**请一并定**。

---

## 6. 交付契约与验收

### 6.1 通用约定（沿用既有，前端已按此写）

- 路径前缀 `/v1/pm/*`（vite 代理剥 `/api`）。
- 列表响应 `{items, total}`（分页加 `page`/`size`）。
- 业务错误体：pm_* 系用 `{message}`；新端点**统一用 `{error}`**（与 `routes.rs` 系一致），前端两种都兼容。
- 时间字段 RFC3339 UTC。
- 乐观锁：写操作带 `revision`；冲突 `409`。

### 6.2 每端点验收要求

- [ ] 项目成员可见性已过滤（非成员读 → 404/空列表）
- [ ] 写操作服务端二次校验权限（前端隐藏按钮不算）
- [ ] 幂等键重试返回同一结果，不产生重复实体
- [ ] 并发冲突返回明确 409 错误码，不静默覆盖
- [ ] MCP 侧**不暴露**：项目创建、基线确认、Cognition 启动、认知逐项确认
- [ ] 有 vitest 或 Rust 测试覆盖

### 6.3 交付顺序建议（后端可并行）

| 批次 | 内容 | 阻塞前端哪些页面 |
|---|---|---|
| B1 | B2-1/B2-2/B2-3（项目创建 + 初始化状态） | S01-02 创建页、初始化工作空间 |
| B2 | B3-1/B3-2/B3-3（仓库多仓 + Primary + Provider） | S01-03 仓库管理页 |
| B3 | B4-1~B4-5（认知 Run/Item/Knowledge/准备/Context） | S01-04 认知总览与审核页 |
| B4 | B1-1~B1-4（基线提案/风险/确认/历史） | S01-01 基线确认页 |

> 前端按本文契约并行开发，各批次交付后逐个联调。

---

## 7. 待你裁决事项（前端等结论）

| # | 事项 | 推荐 | 影响 |
|---|---|---|---|
| Q1 | 确认权判定方式（A/B/C） | **A**（owner 可确认），S02 Approver 表落地后替换 | §0.3 |
| Q2 | Repository role 改造方式（新建 v2 表 / ALTER） | **A**（新建 v2 表） | §2.1 |
| Q3 | 迁移号排布（0029 / 0036 / 0037） | 0036 = 新表批；0037 备用 | §5 |
| Q4 | `core.project.customer_id` NOT NULL 是否放开 | **放开可空**（UX 允许无客户建项目） | §1.2 |
| Q5 | 批量认知确认是否提供后端端点 | **不提供**，前端逐条提交 | §3.2 |
| Q6 | Cognition 启动是否走 MCP | **不走**，仅 REST | §3.1 |

---

**文档状态**：V1.0 待后端同事评审 + 上述 6 项裁决。
