# S01-W13 Project 成员授权设计 V1.0

> 文档性质：S01-W13 实施设计稿（todo §10.8 M11 第二条）
> 上位：`docs/s01-全局主菜单与信息架构设计基线/docs/s01-implementation-plan-v1.md`（冻结基线）、`docs/s01-全局主菜单与信息架构设计基线/docs/s01-task-registry-v1.md` §2.2
> 设计输入：`docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-overall-implementation-plan-v1.md` §4；`docs/s01-全局主菜单与信息架构设计基线/docs/s01-interface-matrix-v1.md` §4-①
> 代码基线：`master 2933db8`　版本：V1.0　日期：2026-11-03　状态：待实施

---

## 0. 一句话

把 `core.project_member` 从**只读事实表**变成**可管理的授权关系**：项目侧有成员管理 API，租户侧撤权级联撤销项目访问，撤权即时生效，全程可审计。

---

## 1. 现状事实（代码核验）

| 事实 | 位置 |
|---|---|
| `core.project_member(tenant_id, project_id, principal_id, role)`，PK `(project_id, principal_id)`，FK `(tenant_id, project_id)`→`core.project` | `migrations/0001_core.sql:41-46`；`0019_tenant.sql:102,184,238-239` |
| `role TEXT NOT NULL`，**无 CHECK**、无枚举 | `0001_core.sql:44` |
| 实际写入的 role 值：`owner`（真人）、`ai_pm`、`architect`、`developer`、`tester`（Agent） | `crates/pm-application/src/bootstrap.rs:282-286` |
| **全仓零写入路径**（除 bootstrap 与测试） | `grep "INSERT INTO core.project_member"` 仅命中 `bootstrap.rs:414` 与测试 |
| 只读消费点：项目可见性过滤、成员门、执行主体解析、任务/交付/制品查询 | `support.rs:133-143`、`authz.rs:39-51`、`authz.rs:86-92`、`pm_write/audit.rs:8,20`、`delivery/mod.rs:849`、`run_dispatch/lease.rs:261,272` |
| `require_project_member` **只查 project_member**，不查 `core.tenant_member` | `crates/pm-application/src/authz.rs:39-51` |
| 租户成员删除只删 `core.tenant_member` 行，**不动 `core.project_member`** | `apps/pm-api/src/tenant_members.rs:368-376` |
| 租户成员模块的事务/审计/错误构造范式 | `apps/pm-api/src/tenant_members.rs:264-333`、`403-433`、`558-581`、`587-618` |
| 项目侧审计表 `governance.audit_log` 含 project_id + 审计四列 + `claimed_device_id` | `crates/pm-application/src/pm_write/audit.rs:40-68` |
| 租户侧审计表 `governance.tenant_audit_log` | `0019_tenant.sql:51` |

### 1.1 由此暴露的三个缺陷

| # | 缺陷 | 后果 |
|---|---|---|
| **X-1** | Project 成员**不可管理**（零写入路径） | 人类无法被授予/撤销项目访问；`project_member` 只能由 bootstrap 写死 5 行 |
| **X-2** | **租户撤权不级联**：移除 `core.tenant_member` 行后，该人仍是 `core.project_member` | 退出租户的人**仍可访问其所有项目**——租户边界失效 |
| **X-3** | 真人项目成员**不必是租户成员** | 授权可绕过租户边界（bootstrap 的 Agent 行例外，见 §2.2） |

---

## 2. 设计裁决

### 2.1 角色模型：真人 Project 角色只有两档

| role | 语义 | 谁能授予 |
|---|---|---|
| `owner` | 项目负责人：可管理项目成员、可转移所有权 | 只能通过所有权转移获得（不允许直接授予） |
| `member` | 普通成员：可读写项目内业务对象（任务、仓库、交付、知识） | owner 或租户 admin |

**裁决理由**：

1. **YAGNI**：Git 能力授权（`read/modify/commit/push/create_pr/merge`）属 I3 的第三层，不在本任务内。Project 角色只表达"能不能管这个项目"，不表达"能对仓库做什么"。三档（owner/maintainer/viewer）会把"仓库能力"和"项目角色"混成一层，与 I3 §2 的四层求交模型冲突。
2. **不新增列**：`core.project_member.role` 一列同时承载 Agent 职能与真人角色，**拆列会让 12 处既有读取点全部改写**；改用**值域收敛**（CHECK 白名单）+ **真人角色两档**的读侧判定，零 schema 演进负担。
3. **Agent 职能值保留**：`ai_pm` / `architect` / `developer` / `tester` 是**任务承接定位**（`resolve_assignee` 按 role 取 principal），不是访问等级。它们留在原列、由 bootstrap 维护、**本 API 不可触碰**。

**附带的表达力限制（明确接受）**：PK `(project_id, principal_id)` 意味"一人一 role"。同一人既是 owner 又要承接 developer 任务无法表达。当前无此需求（人类成员与 Agent 职能天然分离），I3 阶段若需要再拆表。

### 2.2 授权前提：真人项目成员必须是租户成员

- **真人**（`core.principal.kind='human'`）要成为项目成员，必须同时是本租户 `core.tenant_member` 的一行，否则 **409 `NOT_TENANT_MEMBER`**。
- **Agent**（`kind='ai_pm'` / `'dev_agent'`）豁免：ADR-0001 D3 的 Agent 是**租户级**实例，多个项目复用同一组 Agent，bootstrap 写 `project_member` 但不写 `tenant_member`（见 `crates/pm-application/tests/devenv.rs:7` 夹具说明）。豁免只针对 bootstrap 写入的职能行，**不由本 API 写入**。

### 2.3 谁能管理：项目 owner 或租户 admin

- 项目 `owner`（真人行）→ 可管理本项目成员。
- 租户 `owner` / `admin`（`core.tenant_member.role`）→ 可管理本租户任意项目成员。
- 其余 → **403**。跨租户 / 非可见项目 → **404**（不可见即不存在，沿用 T-022 口径）。

### 2.4 租户撤权级联（修 X-2）

移除 `core.tenant_member` 行时：

| 情形 | 行为 |
|---|---|
| 该人是某项目 `owner` | **拒绝移除**，409 `PROJECT_OWNER_DEPENDENCY`（须先在项目内转移所有权）——不静默产生无主项目 |
| 该人是若干项目 `member` | 同事务**级联删除**其全部真人 `project_member` 行，逐项目写审计行 |
| 该人无任何项目身份 | 正常移除 |

### 2.5 撤权即时生效

- `require_project_member` 与 `project_visible` **每次请求实时查库**，删除 `project_member` 行后**下一次请求即被拒**（与租户成员"降级/删除后旧 JWT 下一次请求即被拒"同一口径，`tenant_members.rs:5`）。
- **正在执行中的 Run 不因撤权而中止**：Run 的授权快照语义归 S01-W15 / I3-D（"历史 Run 保留执行时授权快照，但不得凭历史快照继续执行当前已撤销的操作"）。本任务只保证**新领取/新请求**立即失权。

### 2.6 幂等

- 重复授权同一 principal → **409 `ALREADY_MEMBER`**；PK 保证**不产生重复关系**。
- 撤销不存在的成员 → **404**。

---

## 3. 数据模型（迁移 `0030`）

```sql
-- S01-W13：Project 成员授权。
-- role 值域钉死：真人成员 owner/member + Agent 职能 ai_pm/architect/developer/tester。
-- 既有 5 种取值全部在白名单内，无需数据回填（AGENTS.md 规则 1）。
ALTER TABLE core.project_member
    ADD CONSTRAINT ck_project_member_role
    CHECK (role IN ('owner', 'member', 'ai_pm', 'architect', 'developer', 'tester'));

-- 租户撤权级联按 (tenant_id, principal_id) 定位真人项目成员行。
CREATE INDEX idx_project_member_tenant_principal
    ON core.project_member (tenant_id, principal_id);
```

**不新建表、不加列、不写回填脚本。**

| 字段 | migration_id | source_design | source_task | dependencies | upgrade | fresh_install | constraints | data_backfill | rollback | test | freeze_status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 角色 CHECK + 索引 | 0030 | 本稿 §2.1 | S01-W13 | 0019 | 0029→0030 | 全序 | CHECK + INDEX | 无 | DROP CONSTRAINT / DROP INDEX | 见 §6 | draft → reviewed |

---

## 4. 接口契约

| 方法 | 路径 | 权限 | 成功 | 失败 |
|---|---|---|---|---|
| GET | `/v1/pm/projects/{project_id}/members` | 项目可见（成员/owner/租户 admin） | 200 `{items:[{principal_id, display_name, role, kind, is_active, created_at}]}` | 404 项目不可见 |
| POST | `/v1/pm/projects/{project_id}/members` | 项目 owner 或租户 admin | 200 `{principal_id, role:"member"}` | 403 非管理者 / 409 `ALREADY_MEMBER` / 409 `NOT_TENANT_MEMBER` / 409 `AGENT_PRINCIPAL` / 404 |
| DELETE | `/v1/pm/projects/{project_id}/members/{principal_id}` | 项目 owner 或租户 admin | 200 `{principal_id, revoked:true}` | 403 / 404 成员不存在 / 409 `OWNER_LAST` / 409 `AGENT_ROW` |
| POST | `/v1/pm/projects/{project_id}/owner-transfer` | 项目 owner 或租户 admin | 200 `{previous_owner, new_owner}` | 403 / 404 / 409 `ALREADY_OWNER` / 409 `NOT_TENANT_MEMBER` |

**不提供 `PATCH .../role`**：真人角色只有两档，改权 = 撤销重授 或 所有权转移，两者已各有端点。提供 PATCH 只会造出"member→member"的无意义操作与第三种错误语义。

### 4.1 错误码

| code | HTTP | 含义 |
|---|---|---|
| `ALREADY_MEMBER` | 409 | 该 principal 已是项目成员 |
| `NOT_TENANT_MEMBER` | 409 | 目标不是本租户成员（授权前提不满足） |
| `AGENT_PRINCIPAL` | 409 | 目标是 Agent principal，不可由此 API 授权 |
| `AGENT_ROW` | 409 | 目标是 Agent 职能行（bootstrap 维护），不可撤销 |
| `OWNER_LAST` | 409 | 项目唯一 owner 不可被撤销或降级 |
| `USE_OWNER_TRANSFER` | 409 | 授予 owner 必须走所有权转移端点 |
| `ALREADY_OWNER` | 409 | 目标已是 owner |
| `PROJECT_OWNER_DEPENDENCY` | 409 | 该人是某项目 owner，移除租户成员前须先转移项目所有权 |

---

## 5. 事务、锁序与审计

### 5.1 锁序（全局固定，向 I1 的 `project → run → task` 同向）

```text
core.tenant (FOR UPDATE)  →  core.project (FOR UPDATE)  →  core.project_member 目标行 (FOR UPDATE)
```

- 锁 `core.tenant`：把**同一租户内跨项目**的成员变更串行化（授权前提校验与租户撤权级联必须在同一把锁下互斥）。
- 锁 `core.project`：把**同一项目内**的成员变更串行化（owner 唯一性、并发授予/撤销）。
- 三把锁的获取顺序**全局唯一**，避免与 I1（project→run→task）形成环。

### 5.2 事务边界

单事务完成一次变更：锁 → 校验调用者权限 → 校验目标归属 → 写 DML → 写审计 → 提交。**审计失败即整体回滚**（沿 `tenant_members.rs:12` 口径）。

### 5.3 审计

| 场景 | 审计表 | action | target |
|---|---|---|---|
| 授予项目成员 | `governance.audit_log`（项目域） | `project_member.granted` | `project_member` / principal_id |
| 撤销项目成员 | `governance.audit_log` | `project_member.revoked` | `project_member` / principal_id |
| 所有权转移 | `governance.audit_log` | `project_member.owner_transferred` | `project_member` / 双方各一行 |
| 租户撤权级联 | `governance.tenant_audit_log` + 每个受影响项目的 `governance.audit_log` | `tenant_member.removed`（after_data 含 `project_memberships_revoked`）+ `project_member.revoked` | — |

审计四列（`actor_id` / `actor_identity_id` / `actor_client` / `actor_auth_mode`）+ `claimed_device_id` 与既有 13 处审计写入点同口径（`pm_write/audit.rs:40-68`）。

---

## 6. 验收

### 6.1 S01-A001~A010（新增验收段，编号由本任务占用）

| # | 场景 | 类型 | 期望 |
|---|---|---|---|
| A001 | owner 授予成员 → 成员可访问该项目 | 正向 | 200；成员随即可见该项目 |
| A002 | 非成员访问该项目 | 负向 | 404 `project not found`；**零副作用** |
| A003 | 跨租户 principal 被授予 | 负向 | 409 `NOT_TENANT_MEMBER`；零副作用 |
| A004 | 非本租户成员调用管理端点 | 负向 | 404 |
| A005 | 普通成员（非 owner、非租户 admin）授予他人 | 负向 | 403；零副作用 |
| A006 | 重复授予同一 principal | 幂等 | 409 `ALREADY_MEMBER`；**成员关系仍只有 1 行** |
| A007 | 撤销成员 → 其下一次请求被拒 | 负向 + 即时 | 200 撤销；随后成员访问 → 404 |
| A008 | 撤销唯一 owner | 负向 | 409 `OWNER_LAST`；owner 行仍在 |
| A009 | 移除租户成员（其为 2 个项目 member） | 正向 + 负向 | 移除成功；其 2 个项目访问即刻失效；审计含 `project_memberships_revoked=2` |
| A010 | 移除租户成员（其为某项目 owner） | 负向 | 409 `PROJECT_OWNER_DEPENDENCY`；租户成员行仍在 |

### 6.2 并发

| # | 场景 | 期望 |
|---|---|---|
| A011 | 两个请求并发授予同一 principal | 恰好一个 200、另一个 409 `ALREADY_MEMBER`；成员关系 1 行 |
| A012 | 授予与撤销并发 | 终态唯一确定（要么成员在、要么不在），无中间悬空态 |
| A013 | 租户成员移除与项目授予并发 | 二者由 `core.tenant` 行锁串行化：要么授予失败（目标已非租户成员）、要么授予成功且随后被级联撤销 |

### 6.3 恢复

| # | 场景 | 期望 |
|---|---|---|
| A014 | 审计写入失败 | 整个变更回滚（无半截状态） |

### 6.4 数据

| # | 场景 | 期望 |
|---|---|---|
| A015 | 写入非法 role | CHECK 违例（`23514`） |
| A016 | 同一 (project, principal) 重复行 | PK 违例（`23505`） |

---

## 7. 不做

1. 不做 Git 能力等级（I3-B / 迁移 0031）。
2. 不做 Task 级仓库范围（I3-C）。
3. 不做 Project 归档入口（I1-D）——但 owner 语义已为其预留。
4. 不动 `support.rs` / `authz.rs` 等 12 处既有 `project_member` 读取点的判定口径。
5. 不改 `core.project_member` 表结构（不加列、不拆表）。
6. 不做 Agent 职能行的授权/撤销（bootstrap 独占维护）。
7. 不做在线权限码管理端点（`delivery.policy_scope` 管理延后，实施计划 §9-C-1）。

---

## 8. 实施清单

| # | 文件 | 动作 |
|---|---|---|
| 1 | `migrations/0030_project_member_role.sql` | 新建 |
| 2 | `crates/pm-domain/src/project_member.rs` | 新建：`ProjectMemberRole`（Owner/Member）+ `parse` / `as_str` / `is_manager` |
| 3 | `crates/pm-domain/src/lib.rs` | 导出 |
| 4 | `crates/pm-application/src/project_member.rs` | 新建：`apply_member_change` / `list_members` / `revoke_all_for_principal` + 错误枚举 + 纯规则 `authorize` |
| 5 | `crates/pm-application/src/lib.rs` | 模块与导出 |
| 6 | `apps/pm-api/src/project_members.rs` | 新建：4 端点 + 错误映射 |
| 7 | `apps/pm-api/src/routes.rs` | 挂载 router |
| 8 | `apps/pm-api/src/tenant_members.rs` | 修改：`Remove` 分支加级联 + `PROJECT_OWNER_DEPENDENCY` 前置校验 |
| 9 | `apps/pm-api/tests/project_members.rs` | 新建：A001~A016（15 例） |

---

## 9. 风险

| 风险 | 缓解 |
|---|---|
| `core.project_member.role` 被误当作访问等级使用（与 Agent 职能混淆） | 真人两档 + Agent 职能值在 CHECK 白名单中物理区分；`ProjectMemberRole::parse` 只认 owner/member，其余一律视为职能行 |
| 租户撤权级联误删 Agent 行 | 级联 SQL 限定 `role IN ('owner','member')` 且 `principal.kind='human'` |
| 锁序与 I1（project→run→task）形成环 | 本任务锁序 tenant→project→project_member，**不碰 run/task 行**；与 I1 无交叉 |
| 两档角色表达力不足 | 明确记入 §2.1 限制条款，I3 阶段复核 |
