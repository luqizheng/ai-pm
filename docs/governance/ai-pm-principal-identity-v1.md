# 主体身份 v1（G3 principal 映射）

> 状态：**已过稿并实施**（T-020 产出 → 用户过稿 → T-021a 落地；实施记录见 §12）
>
> 上位约束：`docs/adr/ai-pm-adr-0001-tenant-agent-executor-identity.md`（Accepted 2026-11-01）。**本文范围 = T-021a（β）**：身份映射 + 审计三列 + 鉴权链。Tenant 实体、Agent 模型、Run 身份列归 T-021b（α/γ/δ），本文不含。
>
> 一句话：把「JWT 里的调用者」变成「`core.principal` 里的真实主体」，让审计 actor、成员资格、可见性都建立在真身份上。
>
> 前提（AGENTS.md 规则 1）：**新项目、无存量数据**，本设计不含任何回填脚本、认领流程或兼容分支。

## 1. 问题

- `core.principal` 只有 `id/kind/display_name/is_active/created_at`，**没有任何外部身份列**；全库无 tenant 列。
- `require_bearer`（`apps/pm-api/src/auth.rs:36-54`）验签后把 claims 塞进 extensions 就结束，`sub`/`tid` 从不落库。
- 所有写路径的 actor 由 `pm_write::owner_actor`（`crates/pm-application/src/pm_write.rs:92`）三级回退猜出（项目 owner → 任意 human → 全库第一个 human）。
- 结果：`governance.audit_log.actor_id` **不是真实调用者**。D3/D4/D5 都要往审计写执行证据，越晚修脏数据越多。

## 2. 目标与非目标

**目标**

1. 每个 token 都能解析到一个 `core.principal`，绑定键为 `(provider, claims_tenant_key, subject)`。
2. 未知身份自动 provision，但**默认未激活**（方案 C，已裁定）。
3. 未激活 fail-closed；`/v1/pm/me` 是唯一自查入口。
4. 业务写路径的审计 `actor_id` = 真实调用者；系统后台动作 = `ai-pm-dispatch` service principal。
5. 管理端可激活/停用 principal。

**非目标**

- 不做 RBAC 角色体系（`roles` 只用来判定 `service` 与 `tenant_admin`）。
- 不做 per-project 调用方绑定（D2 待裁 #12 仍开放，见 todo T-051）。
- 不做多租户实体与 Agent 模型（ADR-0001 把它拆到 T-021b）：本文 `claims_tenant_key` 只是上游 `tid` 的落库，**不是**内部 `tenant_id`。
- 不给 Agent 发独立凭据（ADR-0001 D4：Agent 是归因主体，认证主体仍是持 key 的真人）。
- 不做身份绑定/解绑接口（无存量数据，YAGNI；需要时另开）。
- 不做「用户被禁用后即时失效」（只能等 token 过期，见 §10）。

## 3. 已裁定

| # | 决策点 | 结论 | 理由 |
|---|---|---|---|
| Q1 | 映射载体 | 独立表 `core.principal_identity`，`UNIQUE(provider, claims_tenant_key, subject)`，`principal_id` 一对多 | 一个真人可绑 lmms 与将来的 GitLab/SSO；`core.principal` 保持纯业务主体，`ai_pm`/`service` 行不被塞满 NULL 认证列 |
| Q2 | 未激活身份的读权限 | **全拒**，只留 `/health` 与 `GET /v1/pm/me` | T-022 的成员过滤尚未实现，任何「允许只读」在今天是等于全库可读（客户、审计、知识正文） |
| Q3 | `owner_actor` 去留 | **双轨**：请求内用真实 principal，请求外用 `ai-pm-dispatch`；`owner_actor` 降为 `cfg(test)` 夹具 | 「猜」的回退在生产代码里编译不到，审计 actor 100% 有来源 |
| Q4 | 存量数据 | **不做迁移**（新项目） | AGENTS.md 规则 1；也不提供身份绑定接口 |
| Q5 | 上游契约 | **不改上游**；按现有 claims 实现，把运行时核对列为实施前置 | 见 §4 |

## 4. 上游契约（已核对源码）

来源：`services/token-service/src/{issuer.rs,routes/access.rs}`、`services/iam-service/openapi.yaml`（`UserClaimsResponse`）。

| token 类型 | 入口 | `sub` | 关键 claims |
|---|---|---|---|
| 浏览器登录 | `/tokens/user-jwt` | iam 用户 id | `aud=coder-micro-services`、`client_id=credential-login-service`、`tid`、`user_code`、`roles`、`permissions`、`ct` |
| API Key / client_credentials | `/tokens/access` | **绑定的真实 user_id**（未绑定 400） | **无 `aud`**、`client_id`=该 Key、`permissions` = 用户 ∩ Key、`tid`、`username`、`roles`、`ct` |
| 服务间 | `/tokens/service-jwt` | 服务名 | `roles=["service"]` |

`/internal/users/{id}/claims`：`{ user_id, username, nickname, roles[], permissions[], tid(int64), ct(bool), email?, phone? }`。

**由此修正的两个早期假设**

1. 不能按「有没有 `username`」判定人/机——API Key 的 token 也带真人 `sub` 与 `username`（上游注释：「方案 A：AI 工具代表真实用户操作」）。
2. 因此**不存在「无用户的业务 token」**：服务间 token 用 `roles` 含 `service` 识别，且不进业务面。

**判别规则（实现口径）**

- `roles` 含 `"service"` → 服务 token → 业务端点 `403 IAM_PERM_3003`，不 provision。
- `ct == true`（跨租户）→ `403 IAM_PERM_3004`，不 provision。
- 其余 → 映射到 `sub` 对应的 human principal；`aud` 存在记为 `interactive`，无 `aud` 且有 `client_id` 记为 `api_key`（写入审计，见 §7）。

**实施前置（运行时核对）**：启动 lmms 栈，分别用浏览器登录 token 与 API Key token 抓一次真实 claims，确认上表三项（服务 token 的 `roles`、API Key 的 `aud` 缺失、`tid` 数值型）。核对不通过则回到本文档改判别口径，不改上游。

## 5. 模型与迁移（0018-β）

```sql
CREATE TABLE core.principal_identity (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider      TEXT NOT NULL,          -- V1 只写 'lmms'
    claims_tenant_key TEXT NOT NULL,      -- claims.tid 字符串化（int64 → text）
                                          -- **不叫 tenant_id**：内部 tenant_id 是 UUID 外键（T-021b 建）
                                          -- 上游 claim 原文与内部租户主键是两个概念（ADR-0001 §7.1）
    subject       TEXT NOT NULL,          -- claims.sub（iam user id）
    principal_id  UUID NOT NULL REFERENCES core.principal(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, claims_tenant_key, subject)
);
CREATE INDEX idx_principal_identity_principal ON core.principal_identity(principal_id);

-- 审计补两列：能回答「谁的身份 + 用什么形态的 token」
ALTER TABLE governance.audit_log
    ADD COLUMN actor_identity_id UUID REFERENCES core.principal_identity(id),
    ADD COLUMN actor_client      TEXT,
    ADD COLUMN actor_auth_mode   TEXT;   -- 'interactive' | 'api_key' | 'service'
```

`core.principal` **不改**：不加 provider/tenant/subject 列，不改 `kind` 取值集合。

**列名移交（ADR-0001 §7.1）**：T-021a 先落地时 `core.tenant` 尚不存在，故上游 `tid` 以 `claims_tenant_key TEXT` 落库；T-021b 建 `core.tenant` 后补 `tenant_id UUID` 外键，由 `(provider, claims_tenant_key)` 解析。**无回填脚本**（新项目）。

**provision 事务**（首次见到未知三元组）：

```
BEGIN
  SELECT identity                       -- 命中 → 直接复用
  -- 未命中：
  INSERT core.principal(kind='human', display_name = username ?? nickname ?? subject, is_active = is_first_admin)
  INSERT core.principal_identity(...) ON CONFLICT DO NOTHING RETURNING principal_id
  -- 若返回空（并发输家）→ DELETE 刚建的 principal（尚未被任何行引用），再重查
COMMIT
```

`is_first_admin` = 三元组命中 `AI_PM_FIRST_ADMIN_TENANT` + `AI_PM_FIRST_ADMIN_SUBJECT`（两者都配置才生效；未配置时全部建为 `is_active=false`，不做「第一个登录者自动成为管理员」）。

`last_seen_at` 节流更新：`UPDATE ... WHERE id=$1 AND last_seen_at < now() - interval '5 minutes'`。

## 6. 鉴权矩阵与中间件

中间件链：`require_bearer` → `resolve_principal` → `require_active`。

1. **`require_bearer`**（不变量）：验签 + 必带 `tid`；验签后不再塞裸 `Value`，改为 typed `AuthenticatedIdentity { provider, claims_tenant_key, subject, roles, permissions, aud, client_id, ct, username? }`。
2. **`resolve_principal`**：按 §4 判别 + §5 provision，产出 `AuthedPrincipal { principal_id, identity_id, is_active, is_admin, auth_mode }`。`is_admin` = `roles` 含 `tenant_admin`。
   **临时口径**：T-021b 起改为读 `core.tenant_member.role`（ADR-0001 D5），届时 token `roles` 降为提示、撤权即时生效。T-021a 阶段保留 token 判定，缺口记在 §10.1。
3. **`require_active`**：`is_active=false` → `403 IAM_PERM_3002`，响应体只有错误码。

| 入口 | 链 | 未激活 human | 服务 token | `ct=true` |
|---|---|---|---|---|
| `/health`、`GET /v1/pm/me` | bearer → resolve | **放行**（只读自查） | 403 | 403 |
| `/v1/pm/*`（业务读写、SSE） | bearer → resolve → require_active | **403** | 403 | 403 |
| `/mcp` | bearer → resolve → require_active → `require_mcp_permission` | **403** | 403 | 403 |
| `/api/v1/*`（平台事件/监督） | 同上 | 403 | 403 | 403 |

`AI_PM_AUTH_MODE=off` 保留，但改为注入固定的 `dev-off` human principal（`is_active=true`）并打启动警告；不允许「无 claims 就整段跳过门」。

## 7. 调用链与审计

**HTTP 写路径**：handler 从 typed extension 取 `AuthedPrincipal`，构造 `pm_application::actor::AuditActor` 显式传给应用层；应用层不再调用 `owner_actor`（已收进 `cfg(test)`）。

| 位置 | 现状 | 改法（已实施） |
|---|---|---|
| `apps/pm-api/src/pm_writes.rs`（审批决策/建改任务/修订） | `owner_actor` 猜 | handler 取 `Extension<AuthedPrincipal>` → `audit_actor()` |
| `pm_write.rs`（需求草稿/定稿、建任务/改任务） | 函数内 `owner_actor` | 函数签名增 `actor: &AuditActor` |
| `repository.rs`（仓库/SSH 密钥 9 个写函数） | `Option<Uuid>` + 回退 | 必填 `actor: &AuditActor`，删除 `resolve_actor` |
| `project_import.rs::start_import` | `Option<Uuid>` + 回退 | 必填 `actor: &AuditActor` |
| `project_import.rs::advance_import_after_decision` | `owner_actor` | 增 `actor: &AuditActor` 参数；监督链传决策人，worker 传 `ai-pm-dispatch` |
| `command_executor::system_context` | 三级回退 | 固定 `ai-pm-dispatch`（`0015` 幂等种子），无查询 |

> 更正：设计稿早期列的 `project_import_advance.rs:38` **不在模块树里**（`lib.rs` 未声明该模块，文件不参与编译）。实际被调用的推进函数是 `project_import.rs::advance_import_after_decision`，由 `supervision` 在人工决策后调用。

**MCP 工具**：actor **保持** 项目内 `dev_agent` / `ai_pm`（D2 契约，工具代表代理而非调用者）。**三列 `actor_identity_id` / `actor_client` / `actor_auth_mode` 当前为 NULL** —— 已尝试但受 rmcp 3.8 的 transport 限制：`StreamableHttpService::new` 的工厂闭包拿不到 `http::Request`，且每个会话在 `tokio::spawn` 的子任务里运行（task-local 不跨 spawn）。要落地需改 transport 集成，登记为 T-059（见 §12）。

**审计写入**：`governance.audit_log.actor_id` = 真实 principal；三列 `actor_*` 描述身份与 token 形态。前端审计页字段向后兼容（新列可暂不展示）。

## 8. 错误码

| 码 | HTTP | 含义 |
|---|---|---|
| `IAM_AUTH_1004` / `IAM_AUTH_1005` | 401 | 验签失败 / 缺 token（沿用） |
| `IAM_AUTH_1006` | 500 | 身份解析失败（DB/事务错误；fail closed，不泄露内部细节） |
| `IAM_PERM_3001` | 403 | 缺权限码（沿用，如 `/mcp` 无 `PM_MCP_READ`） |
| `IAM_PERM_3002` | 403 | 主体未激活 |
| `IAM_PERM_3003` | 403 | 拒绝服务间 token |
| `IAM_PERM_3004` | 403 | 拒绝跨租户 token |

统一原则：响应体只给码与短句，**不泄露身份是否存在**。

## 9. 接口

| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/v1/pm/me` | `{ principal_id, display_name, is_active, is_admin, provider, claims_tenant_key, subject, auth_mode }`；未激活也返回 200 |
| PATCH | `/v1/pm/principals/{id}` | 仅 `is_admin`；body `{ is_active: bool }`；停用即停用，不删行 |
| GET | `/v1/pm/principals` | 沿用（增加 `is_active` 过滤与身份摘要列） |

不做身份绑定/解绑接口（无存量数据）。

## 10. 遗留取舍

1. **用户被禁用不即时生效**：上游 token 15 分钟有效期内仍可用；只有 token 过期后的重新签发才拿不到（`/internal/users/{id}/claims` 对禁用用户的行为需在运行时核对里一并确认）。若需要即时失效，要请上游加事件或查询接口。
2. **一个 principal 多身份**：模型支持，但 V1 只有 lmms 一个 provider，所以实际是一对一。
3. **`claims_tenant_key` 存字符串**：claims 里 `tid` 是 int64，落库字符串化；它是上游 claim 原文，不是内部 `tenant_id`。内部租户实体（`core.tenant`）与业务表 `tenant_id` 由 T-021b 建成（ADR-0001）。
4. **MCP actor 仍是代理**：与「审计 actor = 真实调用者」的表述有张力，取舍见 §7——代理是执行主体，真人是授权/操作主体，两者都留在审计里。
5. **Agent 是归因主体，不是认证主体**（ADR-0001 D4）：MCP 执行端凭真人 API Key 入场，actor 保持项目内 `dev_agent`/`ai_pm`；V1 不给 Agent 签发凭据（上游 `roles=["service"]` 仍被业务面 403），因此「codex 属于谁」的答案是**使用那把 key 的真人**，而不是一个独立的 Agent 主体。
6. **一把 key 多人共用会使归因塌陷**：审计能追到 key 背后的真人，但不能区分是谁在用；运营规则要求每人一把 key。需要 key 级主体时另裁，不在 V1。

## 11. 验收矩阵（续 D2 的 C28 之后）

| 编号 | 场景 | 期望 |
|---|---|---|
| C29 | 无 token 打业务端点 | 401 `IAM_AUTH_1005` |
| C30 | 服务 token（`roles=["service"]`）打业务端点 | 403 `IAM_PERM_3003`，库内不新增 principal |
| C31 | `ct=true` 的 token | 403 `IAM_PERM_3004`，不 provision |
| C32 | 首见未知 identity 调 `GET /v1/pm/me` | 200，`is_active=false`；库内 1 条 principal + 1 条 identity |
| C33 | 未激活调 `/v1/pm/projects` | 403 `IAM_PERM_3002` |
| C34 | 未激活调 `/mcp` | 403 `IAM_PERM_3002` |
| C35 | admin 激活后同 token 重建任务 | 200；`audit_log.actor_id` = 该 principal |
| C36 | 同一三元组并发首登（两请求） | 只建 1 个 principal，无孤儿行 |
| C37 | 浏览器登录 token 与 API Key token（同一 user） | 解析到**同一个** principal；审计 `actor_auth_mode` 分别为 `interactive` / `api_key` |
| C38 | 项目 owner=A，用 B 的 token 写 | `actor_id` = B（不再是 owner 猜测） |
| C39 | worker 推进导入 | `actor_id` = `ai-pm-dispatch` |
| C40 | 连续请求 | `last_seen_at` 按 5 分钟节流更新 |
| C41 | 配置首管理员 env 后首次请求 | principal 直接 `is_active=true` |
| C42 | `AI_PM_AUTH_MODE=off` | 注入 `dev-off` principal，业务端点可用且审计有 actor |

**T-021b（α/γ/δ）验收不在本文矩阵内**：跨租户隔离、组合外键、撤权即时生效、Run 身份列见 `docs/adr/ai-pm-adr-0001-tenant-agent-executor-identity.md` §10（XT / FK / RV / AG / EX 系列）。

**运行时核对（实施前置）**：C37/C30 需要真实 lmms token；栈未起时先用单元测试构造 claims 覆盖，栈起后再补一次真 token 验证并记入 runbook。

**不回归**：D2 的 C1–C28 全绿；`pm_lists`/`customers` 现有用例不因 actor 改造而失败。

## 12. 实施记录（T-021a，2026-11-01）

**迁移**：`0018_principal_identity.sql`（`core.principal_identity` + 审计三列）。

**代码落点**

| 文件 | 改动 |
|---|---|
| `apps/pm-api/src/auth.rs` | `AuthenticatedIdentity`（typed claims）+ `AuthedPrincipal`；`resolve_principal`（provision/节流 `last_seen_at`）；`require_active`（`/health`、`GET /v1/pm/me` 放行）；`inject_dev_principal`（off 模式，写请求才落库） |
| `apps/pm-api/src/routes.rs` | 三层顺序 `require_bearer → resolve_principal → require_active`；新增 `router_with_auth_config`（测试注入首管理员）；off 模式不再拆门 |
| `apps/pm-api/src/principals.rs`（新） | `GET /v1/pm/me`、`PATCH /v1/pm/principals/{id}`（仅 `is_admin`，不删行） |
| `apps/pm-api/src/{pm_writes,pm_repositories,pm_imports}.rs` | handler 取 `Extension<AuthedPrincipal>` 并传 `AuditActor` |
| `crates/pm-application/src/actor.rs`（新） | `AuditActor { principal_id, identity_id, client, auth_mode }` |
| `crates/pm-application/src/{pm_write,repository,project_import,supervision,command_executor}.rs` | actor 改为显式入参；审计插三列；`owner_actor` / `resolve_actor` 回退删除（前者仅 `cfg(test)`） |
| `apps/pm-api/src/pm_lists.rs` | `GET /v1/pm/principals` 增加 `is_active` 过滤 |

**测试**：`apps/pm-api/tests/principal_identity.rs`（C30–C42，11 例，全绿）；既有用例按新语义修正（off 模式 actor = `dev-off`；SSE/MCP 用例先映射身份或配置首管理员）。`cargo test --workspace` 全绿。

**已发现并修正的设计偏差**：`project_import_advance.rs` 不在模块树（见 §7 更正）。

**仍未落地（T-059）**：MCP 审计三列受 rmcp transport 限制（见 §7）。
