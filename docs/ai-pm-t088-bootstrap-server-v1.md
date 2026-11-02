# T-088 服务端 Bootstrap 域与 MCP 调用人上下文设计稿 V1.1

| 项 | 内容 |
| --- | --- |
| 文档性质 | 设计稿 + 实施记录——T-088 服务端实施基线 |
| 版本 | **V1.1**（2026-11-02，按 T-088 V1 评审意见修订：P0×5 + P1×3 + 新增 C177~C184；保留 V1 的 D1~D5 用户裁决与总体架构） |
| 状态 | **已过稿并实施**（2026-11-02 用户过稿；同日实施完成——实施记录见 §十五） |
| 上位约束 | `docs/ai-pm-codex-bootstrap-v1.md` **V1.3（已过稿，M10 实施基线）**；`docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`；`docs/ai-pm-adr-0002-executor-credential-separation.md`；`docs/ai-pm-principal-identity-v1.md` |
| 关联条目 | todo.md §10.7 T-088（服务端，已实施）／T-089（技能包）／T-081（跨机暴露）／§10.5 T-082；§11 T-051（**已关闭**）、T-059（**rmcp 阻塞已解除**） |
| 实施记录 | **§十五**（2026-11-02，提交链 `d855987` → `96cb392` → `33fc743` → `df83f01`，91 套件 / 662 passed / 0 FAILED） |

---

## 〇、冻结约束（V1.1 不可动摇的前提）

以下结论在本稿全部章节中必须成立；实施方若发现某处与之冲突，**先改设计再改代码**，不得在实现里悄悄让步。

| 设计事项 | 冻结结论 |
| --- | --- |
| MCP 上下文传递 | rmcp 原生 `RequestContext` |
| 上下文生产 | pm-api 鉴权层统一生产 |
| 上下文消费 | pm-mcp 只读，不自行构造真人身份 |
| 应用层传递 | 显式传递 `AuditActor` |
| 授权主体 | **真人 Principal** |
| MCP 业务执行主体 | **当前 Project 的 dev_agent** |
| 设备身份 | V1 仅是**不可信声明** |
| 设备绑定 | **不产生**项目或仓库权限 |
| 成员校验 | **全部 17 个 MCP 工具** |
| 非成员错误 | **404** |
| REST 可见性 | 不改变 T-022 已收口语义 |
| rmcp wire 契约 | 不改变工具名称、描述和 JSON Schema |

> **红线（评审原文）**：**不能因为设备已经绑定 Project，就认为使用该设备的真人拥有 Project 权限。**

---

## 一、目标与非目标

### 1.1 目标

T-088 是 M10 的服务端半壁，一次交付三件互相依赖的事：

1. **让 MCP 工具层知道「调用人是谁」**——今天 MCP 工具**根本不知道调用人**，工具参数里的 `project_id` 由客户端自称，无任何校验。
2. **设备目录域**（`devenv.device` + `devenv.device_project_binding`）与 Bootstrap 端点——登记、绑定、自检、撤销。
3. **审计归因补齐**——真人身份三列 + 设备声明列，落到**全部生产审计写入点**。

### 1.2 非目标（明确不做）

- **不做**设备凭据签发、不做授权码、不做设备心跳、不做设备级任务领取——全属 V2（Codex Bootstrap V1.3 §13.5 禁入清单）。
- **不做**无人值守调度、不做 Runner 进程管理、不做 Local Runner 安装器。
- **不改** MCP 工具的 **wire 契约**（工具名 / 描述 / 入参 JSON Schema 全部不动）——新增的 `ctx` 是服务端形参，不进 Schema。
- **不改**既有 REST 端点的可见性语义（T-022 已收口）。
- **不改**租户 / 项目 / principal 权限模型；不改任务状态机；不改 11 个 MCP 工具的业务语义。
- **不做**费用 / 配额。
- **不改**`core.principal` / `core.project` / `core.project_member` 的既有结构（**只引用，不改表**）。

### 1.3 反目标（做成什么样算失败）

- ❌ **半截上下文**：加了 `ctx` 但工具里没用——成员校验与审计仍靠客户端自称。
- ❌ **越权面从一处挪到另一处**：claim 堵住了，但读类 / 文档类 / discovery 类仍可跨项目读。
- ❌ **审计出现第二套真相**：`actor_id` 与新的真人列各写各的，两套口径并存。
- ❌ **成员校验用错主体**：拿 `dev_agent` 去校验成员资格——`dev_agent` 本身就是 `core.project_member` 的一行，校验恒真，形同虚设。
- ❌ **为了通过测试放宽语义**：非成员请求返回 403 而不是 404（泄露项目存在性）。

---

## 二、裁决汇总

### 2.1 D1~D5（用户裁决，V1.1 全部保留）

| # | 裁点 | 裁决 | 依据 / 实证 |
| --- | --- | --- | --- |
| **D1** | 调用人上下文用什么通道进 MCP 工具层 | **rmcp 原生 `RequestContext` 通道**——tool handler 增 `ctx: RequestContext<RoleServer>` 形参 | **spike 已实证**：axum 中间件插入的 extension 在 stateless + json_response + `fallback_service` 装配下确实到达 tool handler（§4.1） |
| **D2** | 上下文进应用层走显式还是隐式 | **显式传参**——沿 T-021a 既有约定 | `append_audit(tx, project_id, actor: &AuditActor, …)` 已收 `&AuditActor`；猜人回退 `owner_actor` 已退场为 `#[cfg(test)]` 即为此 |
| **D3** | 成员校验覆盖多少工具 | **全部 17 个 MCP 工具**；非成员 → **404** | 16 个参数结构中 15 个带 `project_id`；第 16 个 `SubmissionStatusArgs` 只有 `event_id` → 由 `integration.inbound_event` 反查（§5.3） |
| **D4** | 设备声明在哪校验 | **中间件校验一次**——`attach_device_claim` 是**唯一**校验点 | `claimed_device_id` 带 FK → 落库值必须真实；无效声明须「忽略 + 留痕 + 不影响授权结果」 |
| **D5** | 幂等靠什么识别同一台设备 | **id 优先**——客户端生成并持久化 UUID，服务端按 id 命中即复用 | 按 (owner, platform) 匹配会把同一人两台 Windows PC 混为一台；且并发下无法保证唯一 |

### 2.2 V1.1 修订引入的执行者裁定（可否决）

| # | 裁定 | 理由 |
| --- | --- | --- |
| **M1** | `devenv` 两表用**组合外键**保证跨租户完整性，引用目标为 `core.tenant_member(tenant_id, principal_id)`、`core.project(tenant_id, id)`、`devenv.device(tenant_id, id)` | `core.principal` 是 ADR-0001 §3.2 的**全局表，无 `tenant_id`**；「principal ∈ tenant」的唯一既有可引用键是 `core.tenant_member` 的 PK（§7.1） |
| **M2** | 设备 owner / binding 的 `bound_by` 的组合 FK **默认 `NO ACTION`**（不 CASCADE），并成文操作规则：「移除租户成员前须先撤销其设备」 | CASCADE 会连带删掉设备历史；RESTRICT 把顺序约束显式化 |
| **M3** | `attach_device_claim` **只对 `tools/call` 请求**做设备查询与忽略事件审计；`initialize` / `tools/list` / notification **不查库、不写归因审计** | 满足「每个符合处理条件的请求最多一次查询」，并消除非工具请求的重复归因审计（C184） |
| **M4** | 归因忽略事件的审计**失败不升级为拒绝**——记服务端错误日志 + 监控；业务审计失败仍沿用既有事务一致性规则 | 归因不参与授权，审计失败不应改变授权结果（§八） |
| **M5** | `devenv.device.id` 采用**客户端生成的 UUID** 作主键；服务端登记走 `INSERT … ON CONFLICT (id) DO NOTHING` + 回读 | 主键冲突即并发闸门；同一 UUID 并发登记天然收敛为一行（§8.3） |
| **M6** | 重绑（复活 Binding 行）时**在同一事务内**清空 `verified_repositories` 并更新 `declared_repository_ids` | 旧验证结果对新声明无效；两动作必须原子（§8.4） |
| **M7** | 项目可用性**不进 Device 的 Ready**——它是「绑定 × 当前调用人成员资格」的派生值，管理页按调用人实时计算 | 设备可能绑多个项目，真人对不同项目权限不同；否则同一设备的 Ready 会随调用人反复变（§7.4） |
| **M8** | `verified_repositories` 只表示**服务端已确认的仓库身份与项目归属**，**不表示**远程机器上的本地工作目录或 Git 工作区可用 | 两个事实分离：服务端验身份/归属，客户端验本地磁盘（§7.5） |

---

## 三、架构总览

```text
Codex / Trae（Windows PC）
   │  Authorization: Bearer <lmms 真人 key>
   │  X-AI-PM-Device-Id: <device uuid，可选，不可信声明>
   ▼
┌──────────────────────────── pm-api（axum） ────────────────────────────┐
│ require_bearer      验签 + tid/sub → AuthenticatedIdentity                │
│ resolve_principal   映射 core.principal（未知身份按 inactive 建档）       │
│                     → insert Extension<HumanActor>          ← 生产点 1   │
│ require_active      fail-closed（is_active）                              │
│   ▼                                                                     │
│ /mcp 路由（内层）                                                        │
│   require_mcp_permission   工具级权限（PM_MCP_READ / 写权限）             │
│   attach_device_claim      仅 tools/call：读头 → 一次查库 → 校验          │
│                            → insert Extension<DeviceClaim>   ← 生产点 2   │
│   ▼                                                                     │
│   rmcp StreamableHttpService（stateless + json_response）                 │
│     └─ tool handler(ctx: RequestContext<RoleServer>)                     │
│          ├─ caller_from_ctx：读 HumanActor + DeviceClaim → CallerContext │
│          │                    （★ 只读 extensions，不访问数据库）          │
│          ├─ require_project_member(caller.human.principal_id, project_id) │
│          ├─ resolve_project_dev_agent(project_id) → dev_agent（失败即报错）│
│          ├─ 构造 AuditActor{principal_id = dev_agent, 真人三列, 设备列}   │
│          └─ 显式传给应用层函数                                          │
│   ▼                                                                     │
│ pm-application（领域层）                                                  │
│   authz::require_project_member / devenv（设备域） / 全部审计写入点        │
│   ▼                                                                     │
│ PostgreSQL（devenv 两表 + governance.audit_log / tenant_audit_log）        │
└──────────────────────────────────────────────────────────────────────────┘
```

**层序**：`/mcp` 走 `fallback_service(rmcp_service)`；鉴权层是**外层**（`Router::layer`），设备层与工具权限层是**内层**（`mcp_router.layer`）。两层都拿得到 `Extension<HumanActor>`。

---

## 四、身份构造：三层对象（P0-4）

### 4.0 三个对象与流程

```text
HTTP Authentication
       │
       ▼
   HumanActor          （请求级：经过认证的真人）
       │
       ▼
   DeviceClaim         （请求级：已处理过的设备声明结果）
       │
       ▼
  CallerContext         （MCP 请求级：human + device）
       │
       ▼
require_project_member  （用真人 principal_id）
       │
       ▼
resolve_project_dev_agent（当前 Project 的 dev_agent；失败即报错，不回退真人）
       │
       ▼
    AuditActor          （项目级：执行审计身份）
       │
       ▼
  Application Layer     （显式传参）
```

**两条硬约束**：

1. `require_project_member` 使用 `HumanActor.principal_id`，**绝不能**使用 `dev_agent`——`dev_agent` 本身就是 `core.project_member` 的一行（`tools.rs:191` 就是这么查的），用它校验恒真。
2. `AuditActor.principal_id` **必须是当前 Project 的 dev_agent**，不能是全局固定的 dev_agent。

### 4.1 α 通道的实证

todo.md 与 Codex Bootstrap V1.3 一直记着「rmcp 3.8 的工厂闭包不接收 `http::Request`（拿不到 axum extensions），且会话在 `tokio::spawn` 子任务运行（task-local 不继承）」。

**源码核查结论：前提不成立。**

| 环节 | 位置 | 事实 |
| --- | --- | --- |
| 请求 parts 注入消息 | `rmcp/src/transport/streamable_http_server/tower.rs:1538 / 2069 / 2154 / 2275` | `req.request.extensions_mut().insert(part)`——`part` 即 `http::request::Parts`，**连同其 extensions** |
| 消息 extensions 进上下文 | `rmcp/src/service.rs:1579` | `std::mem::swap(&mut extensions, request.extensions_mut())` → 装进 `RequestContext.extensions` |
| 官方用法 | `rmcp/src/transport/streamable_http_server/tower.rs:1310-1330` | rmcp 自己的文档示例就是 `ctx.extensions.get::<http::request::Parts>()` → `.extensions.get::<AppState>()` |
| 宏支持 | `rmcp-macros-3.8.0/src/tool_handler.rs:49 / 69` | `RequestContext<RoleServer>` 是合法的 handler 形参 |

**spike 实证**（throwaway，2026-11-02，已删除）：按本仓真实装配复刻，tool handler 读到中间件插入的 marker：

```text
initialize status = 200 OK
tools/call body = {"result":{"content":[{"type":"text","text":"seen-marker"}]}}
SPIKE RESULT: extension REACHED the tool handler
```

**spike 的两条副产事实**（进 T-089/T-090 的实施注意）：

1. MCP 客户端必须发 `Accept: application/json, text/event-stream`，否则 rmcp 返回 **406**。
2. 本机测试的 reqwest 必须 `.no_proxy()`（默认代理 10808 会拦截 127.0.0.1）。

### 4.2 第一层：`HumanActor`（请求级，pm-api 生产）

```rust
/// 由 pm-api 鉴权层生成，代表**经过认证的真人**。
/// 生产点唯一：pm-api 的 resolve_principal。
#[derive(Debug, Clone)]
pub struct HumanActor {
    pub principal_id: Uuid,          // 真人 principal（授权主体）
    pub identity_id: Option<Uuid>,   // core.principal_identity.id
    pub client: Option<String>,      // 来自 key 的 client_id claim
    pub auth_mode: String,           // 'api_key' | 'interactive' | 'unknown'
}
```

**生产点唯一**：pm-api `resolve_principal`（`apps/pm-api/src/auth.rs`）在构造完 `AuthedPrincipal` 后 `insert(Extension<HumanActor>)`。值由 `AuthedPrincipal::audit_actor()`（该方法已存在，`auth.rs:166`）派生。

**pm-mcp 只读**——不自行构造真人身份（冻结约束）。

### 4.3 第二层：`DeviceClaim`（请求级，pm-api 生产，**唯一校验点**）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClaimIgnoredReason {
    Absent,     // 未提供该头
    Malformed,  // 不是合法 UUID
    NotFound,   // 设备不存在
    NotOwner,   // 设备 owner ≠ 当前调用人
    Revoked,    // 设备 status='revoked'
}

/// **已经处理过的结果**，不是待验证的请求。
#[derive(Debug, Clone)]
pub struct DeviceClaim {
    pub claimed_device_id: Option<Uuid>,
    pub ignored_reason: Option<DeviceClaimIgnoredReason>,
}
```

**`attach_device_claim` 是唯一的设备声明校验点**（P0-3）：

```text
pm-api
  attach_device_claim
       │
       ├── 仅当 JSON-RPC method == "tools/call" 才继续（M3）
       │      否则：注入 DeviceClaim{ None, Absent }，不查库、不审计
       ├── 读取 Header X-AI-PM-Device-Id
       ├── 一次数据库查询（devenv.device 主键）
       ├── 检查设备存在性        → NotFound
       ├── 检查 owner == 调用人   → NotOwner
       ├── 检查 status == active  → Revoked
       ├── 记录忽略原因（仅 tools/call）
       └── 注入 Extension<DeviceClaim>
                 │
                 ▼
pm-mcp
  caller_from_ctx
       │
       ├── 读取 HumanActor
       ├── 读取 DeviceClaim
       └── 组装 CallerContext      ← ★ 不允许访问数据库
```

**硬约束**：

- 每个**符合处理条件的** MCP HTTP 请求**最多执行一次**设备查询。
- `caller_from_ctx` **不允许访问数据库**。
- pm-mcp **不允许**重新判断设备 owner 或 status——那会把校验散成两处，正是 P0-3 要消除的东西。
- 设备声明**不得改变真人授权结果**。

### 4.4 第三层：`CallerContext`（MCP 请求级，pm-mcp 装配）

```rust
#[derive(Debug, Clone)]
pub struct CallerContext {
    pub human: HumanActor,
    pub device: DeviceClaim,
}
```

`caller_from_ctx(&ctx) -> CallerContext`：只做「读两个 extension + 组装」，**无 IO**。

### 4.5 第四层：`AuditActor`（项目级，工具层构造）

```rust
#[derive(Debug, Clone)]
pub struct AuditActor {
    pub principal_id: Uuid,          // ★ 当前 Project 的 dev_agent（执行主体）
    pub identity_id: Option<Uuid>,   // ★ 来自 HumanActor
    pub client: Option<String>,      // ★ 来自 HumanActor
    pub auth_mode: String,           // ★ 来自 HumanActor
    pub claimed_device_id: Option<Uuid>, // ★ 来自已校验的 DeviceClaim
}
```

**构造流程**（工具层，业务逻辑之前）：

1. `caller_from_ctx(&ctx)` → `CallerContext`；
2. `authz::require_project_member(pool, caller.human.principal_id, caller.human.tenant_id, project_id)` → 不通过即 **404**；
3. `authz::resolve_project_dev_agent(pool, project_id)` → 该 Project 的 `dev_agent` principal；
   - **找不到 → 报错**（工具返回错误结果），**不得悄悄退回真人 Principal**（会破 ADR-0001 D2 的执行主体口径）。
4. 构造 `AuditActor{ principal_id: dev_agent, identity_id: human.identity_id, client: human.client, auth_mode: human.auth_mode, claimed_device_id: device.claimed_device_id }`。

**执行主体的分类口径**（维持现状，不统一改口径）：

| 工具类别 | 执行主体（`AuditActor.principal_id`） | 依据 |
| --- | --- | --- |
| 任务 / 运行类（`pm_get_task_context` / `pm_claim_task` / `pm_get_run_context` / `pm_report_progress` / `pm_submit_result`） | Project 的 **dev_agent** | 现状 `tools.rs:191` |
| 文档 / 知识类（`pm_submit_technical_design` / `pm_get_submission_status` / `pm_submit_document` / `pm_propose_document` / `pm_confirm_document_proposal` / `pm_veto_document_revision`） | Project 的 **ai_pm** | 现状 `tools.rs:331 / 405` |
| discovery 类（6 个） | 沿用各自现状解析 | M9 已实现 |

> `resolve_project_dev_agent` 需支持两种 kind（`dev_agent` / `ai_pm`）——按工具类别取用，**不改变各自现状口径**。

**红线**：设备声明**绝不**写进 `principal_id` / `identity_id`——真人身份列不受设备头影响（评审 P0-1b）。

### 4.6 客户端自称的 `confirmed_by` / `vetoed_by`

`pm_confirm_document_proposal` 与 `pm_veto_document_revision` 的 `confirmed_by` / `vetoed_by` 当前是**客户端自称的 Uuid**（`tools.rs:118 / 129`）。

**裁定**：**服务端以认证调用人覆盖客户端值**；参数保留仅为 wire 兼容（冻结约束：不改 JSON Schema），但**不再作为审计依据**。

理由：评审 P0-1 的核心是「真人 principal 必须是唯一授权依据」——保留客户端自称等于允许审计行记录一个自称的第三人，与该原则直接冲突。

> 实施注意：这两处改动的既有断言需同步核对（知识修订链的作者口径从「自称」变为「认证调用人」）。

---

## 五、成员校验（裁决 D3）

### 5.1 洞的准确形状

现状实测：

- `crates/pm-application/src/run_dispatch/claim.rs` 中 `project_member` **零命中** → claim 从不校验调用人是否项目成员。
- `crates/pm-application/src/read.rs:36` 的 `task_context` 只按 `t.id = $1 AND t.project_id = $2` 过滤 → 无调用人概念。
- pm-mcp **17 个工具**；16 个参数结构中 **15 个带 `project_id`**，`SubmissionStatusArgs` 只有 `event_id`。

**结论**：持有效 key 的租户成员，拿到别项目的 `project_id + task_id` 即可读其任务上下文；文档类与 discovery 类同理。这是比 T-051 原措辞（只管领取）更大的洞。

### 5.2 授权流水线（评审 P0-2 成文）

```text
MCP 请求
  │
  ├── 1. 认证真人 Principal            ← 鉴权层（既有三层链）
  ├── 2. 校验 Principal 的 Tenant / Project 成员资格   ← 本条新增（真人主体）
  ├── 3. 校验目标 Repository 的访问权限   ← 现状：工具内按项目解析仓库，非成员已在第 2 步被拦
  ├── 4. 解析 Device ID（仅归因）        ← 本条新增（不改授权结果）
  ├── 5. 记录 Device / Project Binding 关系 ← devenv 域（**仅经显式 REST 端点**，工具调用不自动建绑定）
  └── 6. 执行 MCP Tool
```

**Device 解析可以在权限校验前后进行，但不能成为权限校验的替代品。**

### 5.3 helper 与错误语义

```rust
// crates/pm-application/src/authz.rs（新增）
/// 校验真人是否为目标项目的成员；口径照 pm-api 既有过滤
/// （`pm_lists.rs:99`：`p.tenant_id = $5 AND EXISTS(project_member)`）。
pub async fn require_project_member(
    pool: &PgPool,
    caller_principal_id: Uuid,
    caller_tenant_id: Uuid,
    project_id: Uuid,
) -> Result<(), AuthzError>;

/// 取该 Project 的执行主体（dev_agent / ai_pm）。
/// **找不到即报错**，不得回退真人 Principal。
pub async fn resolve_project_executor(
    pool: &PgPool,
    project_id: Uuid,
    kind: ExecutorKind,   // DevAgent | AiPm
) -> Result<Uuid, AuthzError>;
```

- 语义：**非成员 → 404**（与 T-022 一致：不可见即不存在，防 ID 探测）。**不用 403**。
- **租户维度必带**：`core.project_member` 自 0019 起有 `tenant_id`，且有 `fk_project_member_tenant_project FOREIGN KEY (tenant_id, project_id) → core.project(tenant_id, id)`——跨租户项目在库层即不可达。
- 落点：**pm-mcp 每个 tool handler 的第一业务动作**（解析完 `args.project_id` 之后、进业务逻辑之前）。
- pm-api 的 REST 侧**不动**（已有各自的成员过滤；本条只补 MCP 侧）。

**`pm_get_submission_status` 的特例**：其参数只有 `event_id`（冻结约束禁止改 JSON Schema）→ **由 `integration.inbound_event.project_id` 反查目标项目**，再走同一成员校验：

```sql
SELECT project_id FROM integration.inbound_event WHERE id = $1
```

事件不存在 → **404**；查得项目后按该项目的成员资格判定。**多一次查询，语义与其余 16 个工具一致。**

**评审 P0-2 的反例必须被拒**：用户 A 是 Project 成员，Project 关联 R1/R2，Device D 已绑定该项目，但 A 对 R2 无仓库权——A 通过 D 调工具时，第 2 步按 **A 自己的成员资格**判定，D 的绑定**不参与**授权（审计里 `claimed_device_id` 记 D、`actor_identity_id` 记 A，两者不互相覆盖）。

### 5.4 C173 测试矩阵

| 调用人 | 目标项目 | 设备绑定 | 预期 |
| --- | --- | --- | --- |
| A | A 是成员的项目 | 无 | 允许进入业务校验 |
| A | A 不是成员的项目 | 无 | **404** |
| A | A 不是成员的项目 | **有** | **404** |
| A | 跨租户项目 | 无 | **404** |
| inactive A | A 是成员的项目 | 有 | 鉴权阶段拒绝（`require_active`） |
| A | A 是成员的项目，但**无目标仓库权限** | 有 | **仓库权限校验拒绝**（成员身份不自动扩大到所有 Repository） |

---

## 六、审计归因升级（P0-2）

### 6.1 生产审计写入点清单（实际扫描结果）

**扫描范围**：全仓 `INSERT INTO governance.audit_log` / `governance.tenant_audit_log`；区分**生产 SQL / 测试夹具 / 非审计的 governance 表**。

| # | 位置 | 表 | 三列现状 | actor 来源 | 本条动作 |
| --- | --- | --- | --- | --- | --- |
| A1 | `pm-application/src/pm_write/audit.rs:49` | audit_log | ✅ 已有 | `AuditActor`（调用方显式传入） | 加 `claimed_device_id` |
| A2 | `pm-application/src/pm_write/audit.rs:81` | audit_log | ✅ 已有 | 同上 | 加 `claimed_device_id` |
| A3 | `pm-application/src/repository/support.rs:21` | audit_log | ✅ 已有 | `AuditActor` | 加 `claimed_device_id` |
| A4 | `apps/pm-api/src/tenant_members.rs:565` | tenant_audit_log | ✅ 已有 | 租户成员操作人 | 加 `claimed_device_id` |
| A5 | `pm-application/src/command_executor/audit.rs:24` | audit_log | ❌ 只写 actor_id | `ctx.actor_id`（**执行主体**，非真人） | 补四列 + actor 升级为 `AuditActor` |
| A6 | `pm-application/src/delivery/mod.rs:846` | audit_log | ❌ | 事务内解析的项目 human 成员 / service principal | 补四列 + actor 升级 |
| A7 | `pm-application/src/knowledge_proposal/support.rs:30` | audit_log | ❌ | 形参 `actor_id` | 补四列 + 形参升级为 `AuditActor` |
| A8 | `pm-application/src/project_import/advance.rs:79` | audit_log | ❌ | 导入流程 actor | 补四列 + actor 升级 |
| A9 | `pm-application/src/review/queue.rs:350` | audit_log | ❌ | review 流程 actor | 补四列 + actor 升级 |
| A10 | `pm-application/src/run_dispatch/lease.rs:295` | audit_log | ❌ | 租约流程 actor | 补四列 + actor 升级 |
| A11 | `pm-infrastructure/src/outbox_handlers.rs:222` | audit_log | ❌ | outbox handler actor | 补四列（**跨 crate**） |
| A12 | `apps/pm-api/src/outbox_ops.rs:146` | audit_log | ❌ | 运维操作人 | 补四列（**跨 crate**） |

**合计 12 处生产 INSERT**（11 处 `audit_log` + 1 处 `tenant_audit_log`），其中 **4 处已写三列**、**8 处待升级**。

**辅助调用点**（不新增 INSERT，但需随 actor 升级改签名）：`pm-application/src/command_executor/executor.rs` 有 3 处 `insert_audit` 调用 → 汇入 A5。

**测试夹具**（造数据用，**不在升级范围**）：`apps/pm-api/tests/routes.rs:125`、`apps/pm-api/tests/audit_stream.rs:199`、`pm-application/tests/outbox_ops.rs:211`。

**非审计的 governance 表**（业务表，**不在本次范围**）：`governance.approval_request`、`governance.review_approval`、`governance.human_intervention`。

**动态 SQL**：无——全部为字面量 `sqlx::query`。

### 6.2 每处的绑定规则

统一规则（各点按此执行）：

| 列 | 值 | 来源 |
| --- | --- | --- |
| `actor_id` | 执行主体（dev_agent / ai_pm / service） | 维持各自现状口径（§4.5） |
| `actor_identity_id` | 真人 `core.principal_identity.id` | `HumanActor.identity_id` |
| `actor_client` | 执行端名 | `HumanActor.client` |
| `actor_auth_mode` | `api_key` / `interactive` / `unknown` | `HumanActor.auth_mode` |
| `claimed_device_id` | 已校验的设备 id，或 NULL | `DeviceClaim.claimed_device_id` |

**不在 MCP 路径的写入点**（A5~A12 的部分由 worker / 运维 / 后台流程触发）：真人三列与设备列**允许为 NULL**——没有真人调用者时写 NULL，**不猜**。

### 6.3 审计表分工

| 表 | 装什么 | `claimed_device_id` |
| --- | --- | --- |
| `governance.audit_log`（`project_id` NOT NULL） | 工具调用审计、**绑定类**事件（bind / unbind） | 增列 |
| `governance.tenant_audit_log`（租户级） | **设备生命周期**事件（登记 / 复用 / 撤销）、**「设备声明被忽略」**事件（发生时可能无项目上下文） | 增列 |

---

## 七、devenv 领域模型与端点

### 7.1 迁移 `0027_devenv.sql`（P0-1 完整 DDL）

**前置事实核对**（实施方必须先核，实施前已核过一次）：

| 表 | 现状 | 可引用的组合键 |
| --- | --- | --- |
| `core.principal` | **无 `tenant_id`**（ADR-0001 §3.2 全局表，租户归属由 `principal_identity` / `tenant_member` 表达） | ❌ 无 `(tenant_id, id)` |
| `core.tenant_member` | `tenant_id` + `principal_id` + `role` | ✅ **PK `(tenant_id, principal_id)`** |
| `core.project` | 0019 加了 `tenant_id` | ✅ `uq_project_tenant_id UNIQUE (tenant_id, id)` |
| `core.project_member` | 0019 加了 `tenant_id` + `fk_project_member_tenant_project` | ✅ `(tenant_id, project_id)` → `core.project(tenant_id, id)` |
| `core.principal` 被引用时的删行语义 | `core.tenant_member.principal_id → core.principal(id)` **无 CASCADE**（默认 NO ACTION） | — |

**结论**：「principal ∈ tenant」在库层的唯一可引用键是 **`core.tenant_member(tenant_id, principal_id)`**（PK）。本条**不修改任何核心表**——直接引用既有键。

```sql
-- ============================================================
-- T-088：开发环境目录域（devenv）
-- 上位：docs/ai-pm-t088-bootstrap-server-v1.md V1.1 §7.1
-- 0019-0026 已应用，不得回改；本迁移为 forward-only。
-- ============================================================
CREATE SCHEMA IF NOT EXISTS devenv;

-- ── 设备目录 ───────────────────────────────────────────────
-- 生命周期（status）与就绪快照（readiness）正交两列（评审 P1-1）。
-- id 由**客户端生成**并持久化在本地 device.json（M5）：
--   主键冲突即并发登记的天然闸门，见设计稿 §8.3。
CREATE TABLE devenv.device (
    id                  UUID PRIMARY KEY,               -- ★ 客户端生成的 UUID（不是 gen_random_uuid()）
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    owner_principal_id  UUID NOT NULL,
    name                TEXT NOT NULL,
    platform            TEXT NOT NULL,          -- 'windows' | 'macos' | 'linux'
    os_version          TEXT,
    executor_kind       TEXT NOT NULL DEFAULT 'codex',
    executor_version    TEXT,
    -- 生命周期：能否被接受为「有效登记设备」。V1 只有 active / revoked
    -- （评审明示：不需要 Disabled 就不提前引入）
    status              TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active','revoked')),
    -- 就绪快照：最近一次自检的结论（设备自身就绪，与调用人无关，见 §7.4）
    readiness           TEXT
                        CHECK (readiness IN ('installed','connected','authenticated',
                                             'ready','degraded','failed')),
    last_check_at       TIMESTAMPTZ,
    last_check          JSONB,
    revoked_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    -- ★ 跨租户完整性 #1：设备 owner 必须属于该 tenant
    --   引用 core.tenant_member 的 PK（core.principal 无 tenant_id，
    --   tenant_member 是「principal ∈ tenant」的唯一既有可引用键）。
    --   删行语义 NO ACTION（不 CASCADE）：移除租户成员前须先撤销其设备。
    CONSTRAINT fk_device_tenant_owner
        FOREIGN KEY (tenant_id, owner_principal_id)
        REFERENCES core.tenant_member (tenant_id, principal_id)
);

CREATE INDEX idx_device_owner ON devenv.device (tenant_id, owner_principal_id);

-- ── 设备 ↔ 项目绑定 ─────────────────────────────────────────
CREATE TABLE devenv.device_project_binding (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id               UUID NOT NULL,
    device_id               UUID NOT NULL,
    project_id              UUID NOT NULL,
    -- 用户声明的仓库 id 集（⊆ 该项目的 integration.project_repository）
    declared_repository_ids JSONB NOT NULL DEFAULT '[]',
    -- 服务端已确认的仓库身份与项目归属（⊆ declared；★ 不代表远程磁盘可用，见 §7.5）
    verified_repositories    JSONB NOT NULL DEFAULT '[]',
    bound_by                UUID NOT NULL,
    bound_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    unbound_at              TIMESTAMPTZ,       -- 软删除：解绑后行保留，供审计与历史
    FOREIGN KEY (tenant_id, device_id) REFERENCES devenv.device (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, bound_by) REFERENCES core.tenant_member (tenant_id, principal_id),
    UNIQUE (tenant_id, id),
    UNIQUE (device_id, project_id)             -- 幂等：重复绑定不产生重复行
);

CREATE INDEX idx_binding_device ON devenv.device_project_binding (device_id);

-- ── 审计：真人身份与设备声明分离（评审 P0-1b）────────────────
-- actor_client / actor_auth_mode 已在 0018 / 0019 存在，本迁移只加设备声明列。
-- 与 T-059 的 execution_mode 分属不同列，不重叠。
ALTER TABLE governance.audit_log
    ADD COLUMN IF NOT EXISTS claimed_device_id UUID REFERENCES devenv.device(id);
CREATE INDEX idx_audit_claimed_device ON governance.audit_log (claimed_device_id)
    WHERE claimed_device_id IS NOT NULL;

ALTER TABLE governance.tenant_audit_log
    ADD COLUMN IF NOT EXISTS claimed_device_id UUID REFERENCES devenv.device(id);
CREATE INDEX idx_tenant_audit_claimed_device ON governance.tenant_audit_log (claimed_device_id)
    WHERE claimed_device_id IS NOT NULL;
```

**跨租户完整性四条**（P0-1 要求，逐条对应上面的约束）：

| # | 要求 | 实现 |
| --- | --- | --- |
| 1 | Device 的 owner 必须属于 Device 的 tenant | `fk_device_tenant_owner` → `core.tenant_member(tenant_id, principal_id)` |
| 2 | Binding 的 device 必须属于 Binding 的 tenant | `FOREIGN KEY (tenant_id, device_id) → devenv.device(tenant_id, id)` |
| 3 | Binding 的 bound_by 必须属于 Binding 的 tenant | `FOREIGN KEY (tenant_id, bound_by) → core.tenant_member(tenant_id, principal_id)` |
| 4 | Binding 的 project 必须属于 Binding 的 tenant | `FOREIGN KEY (tenant_id, project_id) → core.project(tenant_id, id)` |

**验收（数据库拒绝，跨租户四例）**：Tenant A 的 tenant_id + Tenant B 的 principal_id 建 Device → **拒绝**；Tenant A + Tenant B 的 device_id 建 Binding → **拒绝**；Tenant A + Tenant B 的 bound_by 建 Binding → **拒绝**；跨租户 project 绑定 → **拒绝**。

**M2 的操作规则成文**：组合 FK 为 `NO ACTION`——**移除租户成员前必须先撤销其设备**；成员管理端点在检测到该成员仍持有设备时应返回明确指引（不是 500 裸 FK 错误）。

**迁移同步事项**（T-088 做）：

- `tenant_schema` 测试的租户业务表期望 **53 → 55**（当前断言为 53，含 M9 discovery 的 7 张）。
- `information_schema` 断言 `devenv.device` 与 `devenv.device_project_binding` 均带 `tenant_id`，且四条组合 FK 存在。
- 新表带 `tenant_id` → 纳入租户业务表的机械断言范围。

### 7.2 端点面

| 端点 | 作用 | 写者 | 可见范围 |
| --- | --- | --- | --- |
| `POST /v1/pm/devices` | 登记 / 复用设备 | 调用人 | 本人 |
| `GET /v1/pm/devices` | 列表 | — | member 见自己的；tenant owner/admin 见全租户 |
| `GET /v1/pm/devices/{id}` | 详情（含绑定、最近自检、**按调用人计算的项目可用性**） | — | 同上 |
| `POST /v1/pm/devices/{id}/bindings` | 绑定 Project（幂等） | 调用人 | 本人设备 |
| `DELETE /v1/pm/devices/{id}/bindings/{binding_id}` | 解绑（软删除） | 调用人 | 本人设备 |
| `POST /v1/pm/devices/{id}/checks` | 上报自检 → 写 `readiness` | 调用人 | 本人设备 |
| `POST /v1/pm/devices/{id}/revoke` | 撤销设备 | 调用人 / tenant admin | 本人设备 / 任意（admin） |

**可见范围口径**：沿 `core.tenant_member.role` 唯一事实源（ADR-0001 R5 的 `role` 列，V1 只复用现有列，不引入 human/agent 授权状态机）——

- `member`：只见 `owner_principal_id = 自己` 的设备，可撤销自己的；
- `tenant owner` / `admin`：见全租户设备，可撤销任意设备；
- 操作他人设备（member 对非本人设备）→ **404**（不泄露存在性）。

### 7.3 就绪四层

| 层 | 归属 | 含义 |
| --- | --- | --- |
| `installed` | Device | 技能包已安装 |
| `connected` | Device | 服务端连接检查通过 |
| `authenticated` | Device / 最近检查 | **最近一次检查使用的真人认证有效** |
| `ready` | Device | 开发环境自身满足就绪条件 |
| `degraded` | Device | 环境存在可用性问题 |
| `failed` | Device | 检查失败 |

### 7.4 项目可用性不进 Device 的 Ready（P1-1）

**设备可能绑定多个 Project，而真人对不同 Project 的权限不同**——若把项目权限折进设备 Ready，同一台设备会因为不同调用人的成员资格反复改变自身状态。

```text
Device
  │
  ├── Binding A → Project A → accessible
  ├── Binding B → Project B → inaccessible
  └── Binding C → Project C → accessible
```

因此：

- **Device 的 Ready 只描述设备自身**（技能包 / 连接 / 最近认证 / 声明仓库的自检结果）。
- **项目可用性**是「绑定 × 当前调用人成员资格」的**派生值**，由 `GET /v1/pm/devices/{id}` 按调用人实时计算（`bindings[].project_accessible`），**不落库**。
- **Device 的 Ready 不因调用人对某个 Project 无权而改变**；成员校验仍在每次工具调用时执行（§5）。

**退化**：认证过期、连接断开、声明仓库不可用 → `readiness` 可退化。退化由**下一次 `/checks`** 或管理端读取时判定；V1 无心跳，**不能主动发现**远端失联（属 V2）。

### 7.5 `verified_repositories` 的准确定义（P1-2）

> **`verified_repositories` 是服务端根据客户端提交的 Repository 声明，经过项目归属和仓库访问权限校验后确认的 Repository ID 集合。它不代表服务端已经验证远程机器上的本地工作目录或 Git 工作区真实可用。**

两个事实必须分开：

| 事实 | 由谁验证 | 落在哪 |
| --- | --- | --- |
| 仓库身份 + 项目归属 + 访问权限 | **服务端** | `verified_repositories` |
| 本地工作目录存在 / Git 状态可用 | **客户端** | 客户端自检报告（`last_check` 的客户端项） |

服务端校验链：`declared_repository_ids` ⊆ 该项目的 `integration.project_repository` → 每条仓库 id 的项目归属 → 校验通过者写入 `verified_repositories`；**未通过者剔除并回传给客户端**（不得静默丢弃）。

### 7.6 幂等与并发（P1-3）

**首次登记**：

1. 客户端生成 UUID（v4），写入本地 `device.json`（持久化，登记成功后再发请求亦可）；
2. 后续请求**始终**使用同一个 UUID；
3. **无 UUID 只作为首次登记**；
4. 首次登记**必须支持请求重试**；
5. **同一 UUID 的并发请求不得创建多条设备记录**。

服务端实现（M5）：`devenv.device.id` 即客户端 UUID（主键）；登记走

```sql
INSERT INTO devenv.device (id, tenant_id, owner_principal_id, ...) VALUES (...)
ON CONFLICT (id) DO NOTHING;
```

随后回读该行并按 §8.5 的复用规则判定；**并发请求天然收敛为一行**。

**Binding**：

- 绑定 / 解绑 / 重绑均使用**事务**；
- 对已有 Binding 行做适当的并发控制（`SELECT … FOR UPDATE` 或条件 UPDATE）；
- **解绑后保留原记录**（置 `unbound_at`）；
- **重绑复活原记录**（清 `unbound_at`）；
- **重绑时清空旧的 `verified_repositories`，并把「更新声明 + 清验证结果」放在同一事务**（M6）。

---

## 八、失败路径、并发与错误语义

### 8.1 常规失败

| 场景 | 响应 | 留痕 |
| --- | --- | --- |
| 非项目成员调用工具 | **404** | 不留痕（未授权请求不产生业务审计） |
| 设备不属于调用人 | **404** `device_not_found` | 不留痕 |
| 设备声明非法 | 请求**正常进行** | `tenant_audit_log` `device_attribution_ignored`（仅 tools/call） |
| 重复登记 | 复用既有行，**200** + 同一 device_id | 复用事件写 `tenant_audit_log` |
| 已撤销设备再登记 | 新建，**201** + 新 device_id | 旧行保持 `revoked` |
| 自检缺项 | **200** + `readiness='degraded'` + 逐项结果 | `last_check` 落库 |
| 撤销已撤销的设备 | **200**（幂等） | 审计记录 |

### 8.2 归因审计失败策略（M4）

- 设备声明**仅影响归因，不参与授权**；
- **归因审计写入失败，不得把请求升级为拒绝授权**——失败必须进入服务端错误日志与监控；
- **业务审计与归因忽略事件分开处理**：业务操作自身要求的审计若失败，仍遵循现有事务一致性规则（审计与业务同事务，失败即回滚业务）；
- 「设备声明被忽略」事件**只在 `tools/call` 触发**（M3），避免 `initialize` / `tools/list` / notification 各写一条重复事件（C184）。

### 8.3 首次登记的并发与重试

| 并发形态 | 期望 | 实现 |
| --- | --- | --- |
| 同一 UUID 两次并发首次登记 | **一条记录** | 主键冲突 + `ON CONFLICT DO NOTHING` + 回读 |
| 同一 UUID 首次登记请求重试 | **一条记录**，两次响应同 device_id | 同上 |
| 不同 UUID 并发登记（用户误开两个实例） | 两条记录 | 正常——UUID 是设备身份，不做 (owner, platform) 推测 |
| 提交已存在的他人 UUID | **拒绝**（404），不新建 | 回读后 owner/tenant 不匹配 → 拒绝 |
| 提交已撤销的 UUID | **新建**（新 id），旧行留历史 | 已撤销设备**不得自动恢复** |

### 8.4 Binding 的并发与重绑

| 形态 | 期望 | 实现 |
| --- | --- | --- |
| 并发绑定同一 (device, project) | **一条行** | `UNIQUE(device_id, project_id)` + 事务内条件写入 |
| 绑定与解绑并发 | 终态确定（按提交顺序） | 事务 + 行锁 |
| 重绑 | **复活原行**；`unbound_at` 清空；`declared_repository_ids` 更新；**`verified_repositories` 在同一事务内清空** | 单事务（M6） |
| 重绑后未重新自检 | `verified_repositories` 为空 → 该绑定不算「仓库已验证」 | 清空即失效 |

### 8.5 复用判定（D5）

`POST /v1/pm/devices` 携带可选 `device_id`：

| 情形 | 服务端行为 |
| --- | --- |
| id 存在 + owner 匹配 + `status='active'` | **复用**该行（更新 name / platform / executor_version） |
| id 存在 + owner / tenant 不匹配 | **拒绝**（404）——不新建，避免 id 探测 |
| id 存在 + `status='revoked'` | **新建**（新 id），旧行留历史——**已撤销设备不得自动恢复** |
| id 缺失 / 未命中 | 新建 |

---

## 九、测试与验收

### 9.1 验收编号映射

| C 编号 | 验收内容 | 归属 |
| --- | --- | --- |
| C145 / C146 / C147 / C149 / C151 / C152 / C156 / C157 / C161 | BOOT-08/09/10/12/14/15/19/20/24 在服务端的落地 | T-088 |
| C163 | Device ID 仅不可信声明，不影响 Principal 授权结果 | T-088 |
| C164 | 审计三分（真人 / `claimed_device_id` / run） | T-088 |
| C165 | Binding 不授予权限 | T-088 |
| C167 | 生命周期与就绪两列、撤销语义 | T-088 |
| C168 | 幂等三态 | T-088 |
| C171 | Principal 权限与 Device 归因完全分离 | T-088 |
| C173 | 成员校验覆盖全部 17 个 MCP 工具（非成员 → 404；含 §5.4 六行矩阵） | T-088 |
| C174 | **所有生产审计写入点均完成统一身份字段升级；实际生产 INSERT 数量与审计清单一致，不能遗留旧口径**（**不把数量固定为 8 或 9**） | T-088 |
| C175 | α 通道端到端：调用人上下文从 axum 中间件到达 tool handler（把 spike 转正为正式回归测试） | T-088 |
| C176 | 设备声明四类非法值（malformed / not_found / not_owner / revoked）均忽略 + 留痕 + 不改变授权结果 | T-088 |
| **C177** | **Device、Binding 跨租户 FK 完整性**（P0） | T-088 |
| **C178** | **HumanActor 正确映射当前 Project 的 dev_agent**（P0） | T-088 |
| **C179** | **DeviceClaim 不发生二次数据库校验**（P0） | T-088 |
| **C180** | **全部生产审计写入点覆盖率 100%**（P0） | T-088 |
| **C181** | **无 device_id 首次登记与并发重试幂等**（P1） | T-088 |
| **C182** | **多 Project Binding 下 Ready 语义**（P1） | T-088 |
| **C183** | **解绑、重绑并发与验证结果失效**（P1） | T-088 |
| **C184** | **非工具 MCP 请求不产生重复归因审计**（P1） | T-088 |

### 9.2 测试清单

**pm-mcp 集成测试**（`crates/pm-mcp/tests/`）：

1. **C175**：`resolve_principal` 插入的 `HumanActor` 与 `attach_device_claim` 插入的 `DeviceClaim` 在 tool handler 内可读（spike 转正）。
2. **C173**：参数化跑全部 17 个工具——非成员 → 404 且**不产生业务副作用**；成员 → 正常路径。矩阵见 §5.4（含 `pm_get_submission_status` 的反查路径）。
3. **C176 / C179**：四种非法设备声明 → `claimed_device_id` 为 NULL、审计有 `device_attribution_ignored`、工具仍正常执行；**并断言 pm-mcp 侧未发起任何设备查询**（用查询计数或 mock 验证）。
4. **C178**：断言审计行的 `actor_id` 是**当前 Project 的 dev_agent**（任务/运行类）与 **ai_pm**（文档/知识类），`actor_identity_id` 是真人 identity；**找不到 dev_agent 时报错而非回退真人**。
5. 合法设备声明 → `task_run.executor_id` 落该 id。

**pm-api 集成测试**（`apps/pm-api/tests/`）：

6. devenv 端点 CRUD + 可见范围（member 只见自己的 / admin 见全租户）+ 404 语义。
7. `/checks` 四层判定 + 退化路径。
8. **C181**：无 id 首次登记 → 生成 id；同 id 并发登记 → 一行；同 id 重试 → 同 id；他人 id → 404；已撤销 id → 新建。
9. **C184**：`initialize` / `tools/list` 不产生 `device_attribution_ignored` 审计；`tools/call` 产生。

**pm-application 测试**（`crates/pm-application/tests/`）：

10. `require_project_member` 的命中 / 未命中 / 跨租户 / inactive 四态。
11. `resolve_project_executor` 的命中 / 缺失（缺失即报错）。
12. **C177**：跨租户四例由**数据库拒绝**（直接执行跨租户 INSERT，断言 FK 违规）。
13. **C182**：多 Project Binding 下 Device 的 Ready 不随调用人变化；`project_accessible` 按调用人计算。
14. **C183**：绑定/解绑/重绑并发；重绑后 `verified_repositories` 已清空。
15. 迁移测试：`information_schema` 断言两新表 `tenant_id` + 四条组合 FK；两张审计表 `claimed_device_id` 列存在；**租户业务表期望 53 → 55**。

**审计覆盖测试**（**C174 / C180**）：

16. 以**清单驱动**：遍历 §6.1 的 12 处生产 INSERT 位置，断言每处的列清单都含 `actor_identity_id` / `actor_client` / `actor_auth_mode` / `claimed_device_id`；**清单与实际 INSERT 数量不一致即测试失败**（防止新增写入点漏改）。测试夹具与非审计的 governance 表不在此列。

**回归**：`cargo test --workspace` 全绿；`cargo check --workspace --all-targets` 0 error / 0 warning；**T-081 跨机闭环与 rmcp 契约测试不回归**（薄层不碰 transport）。

---

## 十、实施拆分（建议提交顺序）

| 步 | 内容 | 依赖 | 提交纪律 |
| --- | --- | --- | --- |
| 1 | `pm-application`：迁移 `0027` + `devenv` 模块 + `authz`（成员校验 + 执行主体解析）+ `HumanActor` / `DeviceClaim` / `CallerContext` / `AuditActor` 四类型 | — | `feat(devenv)` |
| 2 | **审计升级**：12 处生产 INSERT 补列（8 处待升级）+ actor 升级为 `AuditActor` + 清单驱动测试 | 步 1 | `feat(audit-actor)`（**与步 1 分开提交**，避免大 diff 不可审） |
| 3 | `pm-api`：`attach_device_claim` 层 + `resolve_principal` 插 `HumanActor` + devenv 八个端点 | 步 1 | `feat(api-devenv)` |
| 4 | `pm-mcp`：工具层接入 `ctx` + `caller_from_ctx` + 成员校验 + `AuditActor` 构造 | 步 2、3 | `feat(mcp-caller)` |
| 5 | 测试与验收（§9.2 全部） | 步 4 | `test(...)` |
| 6 | 设计稿实施记录回填 + todo.md | 步 5 | `docs(t088)` → `chore(todo)` → `chore(memory)` |

**跨会话注意**：步 2 会改 `pm-application` / `pm-infrastructure` / `pm-api` 多个既有文件——**与 M9 已提交的 discovery 改动同仓**，开工前须核对工作区与他人改动不重叠。

---

## 十一、迁移责任与历史核对（P0-5）

### 11.1 当前迁移历史核对结果（实施方须先复核，本稿已核过一次）

| 字段 | 归属 | 现状 | 落点 |
| --- | --- | --- | --- |
| `governance.audit_log.actor_client` | T-059（实为 **0018** 已建） | **已存在** | 不动 |
| `governance.audit_log.actor_auth_mode` | T-059（实为 **0018** 已建） | **已存在** | 不动 |
| `governance.audit_log.actor_identity_id` | T-021a（0018 建、0019 重建 FK） | **已存在** | 不动 |
| `governance.tenant_audit_log.actor_*` 三列 | 0019 | **已存在** | 不动 |
| `delivery.task_run.execution_mode` | **T-059**（ADR-0002 移交） | **未落库**（全仓迁移无此列） | **归 T-059**，本条不碰 |
| `governance.audit_log.claimed_device_id` | **T-088 / 0027** | 新增 | 本条 |
| `governance.tenant_audit_log.claimed_device_id` | **T-088 / 0027** | 新增 | 本条 |
| `devenv.device` / `devenv.device_project_binding` | **T-088 / 0027** | 新增 | 本条 |

**结论**：**T-059 与 T-088 不重复修改同一审计表结构**——T-059 剩下的只有 `execution_mode` 一列（普通列迁移，不受 rmcp transport 阻塞，可先行），落在 `task_run` 而非审计表。

**若 T-059 先落地**：其 `execution_mode` 应作为 0026 之后的 forward-only 迁移（如 `0028_task_run_execution_mode.sql`）；0027 不依赖它，两者互不冲突。**不允许两个任务分别重复修改相同的审计表结构。**

### 11.2 迁移验收必查项

- 实际 migration 文件内容；
- migration 执行顺序（0026 → 0027）；
- 两张审计表最终字段（`claimed_device_id` 存在且类型正确）；
- 所有 FK（含四条跨租户组合 FK）与索引；
- **失败恢复 / rollback 策略**——按仓库既有迁移约定（forward-only、不回改已应用迁移、失败即整个事务回滚；`sqlx::migrate` 单事务执行，无部分应用）。

---

## 十二、遗留与风险

| 项 | 说明 | 处置 |
| --- | --- | --- |
| **`execution_mode` 列** | T-059 的普通列迁移，本条不含 | 独立任务；可搭 0027 之后 |
| **成员移除与设备持有的顺序** | 组合 FK `NO ACTION` → 持有设备的成员无法直接从 `core.tenant_member` 移除 | 成文规则：先撤销设备再移除成员；成员管理端点给明确指引（M2） |
| **`actor_client` 可能为空** | 纯用户签发的 key 无 `client_id` claim | 空则写 NULL，不猜 |
| **跨机 `Host` 头与反代** | 本条不碰 transport | T-081 负责；T-088 只在 T-081 之上加扩展 |
| **审计表两处不同构** | `tenant_audit_log` 无 `target_type` / `target_id` | 「设备声明被忽略」事件用 `after_data` 记原因，不强塞 |
| **`/checks` 的客户端声明不可全信** | installed / connected / tool 三项由客户端报 | 服务端只对可验证事实负责；`ready` 结论由服务端算，并保留逐项证据供人工核对 |
| **V1 无心跳** | 远端失联不能主动发现 | 退化在下次自检或管理端读取时判定（§7.4） |
| **M9 并行会话** | `pm-mcp/src/tools.rs`、`server.rs` 已含 discovery 六工具 | 步 4 开工前先 `git status` 核对，不覆盖他人改动 |

---

## 十三、本轮改动声明与一致性检查

### 13.1 本轮改动范围

**本轮只修改本设计稿一个文件**：未修改任何产品代码、数据库迁移、测试或其他实施文件；未提交任何 commit（提交待设计稿过稿后进行）。

### 13.2 与既有 ADR / 上位稿的一致性检查

| 检查项 | 结论 |
| --- | --- |
| ADR-0001 D2（MCP 工具 actor = 项目内 dev_agent / ai_pm） | ✅ 一致——`AuditActor.principal_id` 维持各工具现状口径（§4.5），真人信息走 `actor_identity_id` |
| ADR-0001 D6（`executor_id` 无 FK，Registry 归 T-030） | ✅ 一致——V1 只写值、不据其鉴权或计费 |
| ADR-0001 R5（`project_member` 复用现有 `role` 列） | ✅ 一致——可见范围沿 `tenant_member.role`，不引入 human/agent 授权状态机 |
| ADR-0001 §3.2（`core.principal` 是全局共享表） | ✅ 一致——**未给 `core.principal` 加 `tenant_id`**，改用 `core.tenant_member` 的既有 PK 表达租户归属（M1） |
| ADR-0002 D1（不新增 Executor Credential，Run 级 = lease_token） | ✅ 一致——本条不涉凭据 |
| ADR-0002 D5（T-051 为跨机上线安全前置） | ✅ 一致——本条关闭 T-051 的核心机制（真人成员校验） |
| T-021a（actor 必须由调用方显式传入） | ✅ 一致——沿用 `AuditActor` 显式传参，**不引入隐式全局状态** |
| Codex Bootstrap V1.3 §5.1.1（三层身份） | ✅ 一致——§六 |
| Codex Bootstrap V1.3 §5.4.1（授权流水线六步） | ✅ 一致——§5.2 |
| Codex Bootstrap V1.3 §5.6（DDL 草案） | ✅ 扩展——补四条跨租户组合 FK 与 `tenant_audit_log` 增列；`device.id` 改客户端生成 |
| Codex Bootstrap V1.3 §5.7（审计三分） | ✅ 一致——§六 |
| Codex Bootstrap V1.3 §6（Ready 四层与退化） | ✅ 修订——Ready 明确为**设备自身就绪**，项目可用性分离（M7） |
| Codex Bootstrap V1.3 §8.1（幂等三态） | ✅ 扩展——补首次登记并发语义与重绑事务语义 |
| Codex Bootstrap V1.3 §13.5（V1 禁入清单） | ✅ 一致——未新增任何 V2 能力 |

### 13.3 仍未解决的问题（阻塞等级）

| # | 问题 | 阻塞等级 | 说明 |
| --- | --- | --- | --- |
| 1 | `pm_get_submission_status` 由 `integration.inbound_event` 反查 project_id | **无** | 已核：`inbound_event.id` 是 **PRIMARY KEY**（`0003_platform.sql:7`），`WHERE id = $1` 命中唯一行，反查无歧义；事件不存在 → 404 |
| 2 | `execution_mode` 列仍未落库 | **无**（不属本条） | 归 T-059 |
| 3 | `core.tenant_member` 的 `NO ACTION` 语义使「成员持有设备时无法移除」 | **低** | 已成文操作规则（M2）；成员管理端点的指引文案属 T-088 步 3 的实现细节 |
| 4 | 文档类工具 `confirmed_by` / `vetoed_by` 改为认证调用人后，既有断言与知识链口径需核对 | **低** | 属步 2 的实现细节，已在 §4.6 标注 |
| 5 | V1 无心跳 → 远端失联不可主动发现 | **无** | 设计取舍，V2 解决 |

---

## 十四、V1 → V1.1 修改记录与评审逐项处理结果

### 14.1 V1 → V1.1 修改记录

| 位置 | V1 | V1.1 | 触发 |
| --- | --- | --- | --- |
| 文档头 | V1 / 待过稿 | **V1.1** / 待过稿；加「本轮改动：仅本设计稿」声明 | 评审交付要求 |
| **§〇** | 无 | **新增「冻结约束」表**（12 条不可动摇前提 + 红线） | 评审 §一 |
| §2.2 | M1~M10（10 条裁定） | 改为 **M1~M8**（合并去重，标注评审对应项）；新增跨租户 FK、审计点清单、单次校验、身份三层、迁移责任等裁定 | 评审 P0×5 + P1×3 |
| §4 | `AuditActor` 单层；上下文从 `Extension<AuditActor>` 直取 | **三层对象** `HumanActor` → `CallerContext` → `AuditActor`；`DeviceClaim` 改枚举 + 唯一校验点；`caller_from_ctx` 禁访问数据库；新增 `confirmed_by`/`vetoed_by` 裁定 | P0-3 / P0-4 |
| §5.3 | helper 签名 `(pool, caller_principal_id, project_id)` | 增 `caller_tenant_id`；增 `resolve_project_executor`（缺失即报错）；补 `pm_get_submission_status` 反查路径；补 §5.4 六行测试矩阵 | P0-4 / 评审 §四 |
| §6 | 「8 处（7 个文件）」 | **12 处生产 INSERT 的完整清单**（4 已合规 / 8 待升级）+ 3 处 helper 调用点 + 3 处测试夹具 + 非审计 governance 表排除；C174 改为 100% 覆盖 | P0-2 |
| §7.1 | 两表 + 单列 `claimed_device_id`（仅 audit_log） | **四条跨租户组合 FK**（owner / device / bound_by / project）+ `tenant_audit_log` 增列 + `device.id` 改客户端生成 + 删行语义成文 + 表数 53→55 | P0-1 / P0-5 / P1-3 |
| §7.3~7.5 | Ready 含项目权限；`verified_repositories` 表述含混 | **Ready 限定为设备自身**；项目可用性分离为按调用人计算的派生值；**`verified_repositories` 定义为「服务端确认的仓库身份与项目归属」** | P1-1 / P1-2 |
| §8 | 常规失败语义 | 增 **§8.2 归因审计失败策略**、**§8.3 首次登记并发**、**§8.4 Binding 并发与重绑**、§8.5 复用判定 | 评审 §五 + P1-3 |
| §9.1 | C173~C176 | 保留 + **C174 措辞改写** + **新增 C177~C184** | 评审 §四 |
| §9.2 | 测试清单 | 增跨租户 DB 拒绝、dev_agent 映射、无二次查询断言、首次登记并发、多 Project Ready、Binding 并发、审计清单驱动测试 | P0×5 / P1×3 |
| §十一 | 迁移口径「43+7+2」 | 改为 **§十一 迁移责任与历史核对**：T-059 与 T-088 的字段归属表 + 迁移验收必查项 | P0-5 |
| §十三 | 无 | 新增：改动声明 + **ADR/上位稿一致性检查（12 项）** + 未解决问题与阻塞等级 | 交付要求 |

### 14.2 P0 逐项处理结果

| 评审项 | 处理 | 落点 |
| --- | --- | --- |
| **P0-1 跨租户数据完整性** | ✅ 已核 `core.principal` **无 `tenant_id`**（ADR-0001 全局表）→ 改用 `core.tenant_member(tenant_id, principal_id)`（PK）表达「principal ∈ tenant」；四条组合 FK 全部落到 DDL；**未修改任何核心表**；跨租户四例验收进 §9.2 第 12 项 | §7.1 |
| **P0-2 审计写入点重盘** | ✅ 实际扫描得 **12 处生产 INSERT**（原稿「8 处」与明细「9 处」的数量矛盾已订正）；区分生产 / 测试夹具 / 非审计 governance 表 / 动态 SQL；逐点列出 actor 来源与绑定规则；**C174 改为「实际生产 INSERT 数量与清单一致，不能遗留旧口径」，不固定为 8 或 9** | §6.1、§6.2、§9.1、§9.2 第 16 项 |
| **P0-3 DeviceClaim 二次校验** | ✅ `attach_device_claim` 为**唯一**校验点；`caller_from_ctx` **禁访问数据库**；每请求**最多一次**查询；忽略原因改**枚举**；`DeviceClaim` 明确为「已处理过的结果」；并加「仅 `tools/call` 触发查询与忽略审计」 | §4.3、M3、§8.2 |
| **P0-4 身份构造流程** | ✅ 明确三层对象 `HumanActor` / `CallerContext` / `AuditActor` 与完整流程图；两条硬约束（成员校验用真人；`AuditActor.principal_id` 必须是当前 Project 的 dev_agent）；`resolve_project_dev_agent` 缺失即报错**不回退真人**；补执行主体分类口径（dev_agent vs ai_pm） | §4.0、§4.2~§4.5 |
| **P0-5 迁移责任** | ✅ 核对 T-059 实际状态：`actor_client` / `actor_auth_mode` **0018/0019 已存在**（不属 T-059 待办）；`execution_mode` **全仓迁移无此列** → 仍归 T-059；0027 只加两表 + 两张审计表的 `claimed_device_id` → **两任务不重复改同一审计表结构**；迁移验收必查项成文 | §十一 |

### 14.3 P1 逐项处理结果

| 评审项 | 处理 | 落点 |
| --- | --- | --- |
| **P1-1 重新定义 Ready** | ✅ Ready 六态**归属设备**；**项目权限不纳入 Device 的 Ready**；项目可用性改为「绑定 × 当前调用人成员资格」的派生值，管理页按调用人实时计算、不落库 | §7.3、§7.4、M7 |
| **P1-2 重新定义 verified_repositories** | ✅ 定义为「服务端确认的仓库身份与项目归属集合」，**不代表远程磁盘可用**；两个事实分表列明（服务端验身份/归属，客户端验本地磁盘） | §7.5、M8 |
| **P1-3 首次登记与 Binding 并发** | ✅ 首次登记：客户端生成 UUID → 本地持久化 → 后续同 UUID → 无 UUID 仅首次 → 支持重试 → 同 UUID 并发不产生多行（主键冲突 + `ON CONFLICT DO NOTHING` + 回读）；Binding：事务 + 并发控制 + 解绑保留行 + 重绑复活 + **重绑清空 `verified_repositories` 与更新声明同事务** | §7.6、§8.3、§8.4、M5、M6 |

### 14.4 评审未点名但一并处理的项

| 项 | 处理 | 落点 |
| --- | --- | --- |
| `pm_get_submission_status` 无 `project_id` | 由 `integration.inbound_event` 反查（`id` 是 PK，命中唯一）再走同一成员校验——**不破 wire 契约、不留洞** | §5.3、§13.3-1 |
| `confirmed_by` / `vetoed_by` 客户端自称 | 服务端以认证调用人覆盖（参数保留仅为 wire 兼容） | §4.6 |
| 归因审计失败策略 | 失败不升级为拒绝；进错误日志与监控；业务审计失败仍走事务一致性 | §8.2、M4 |
| 非工具请求的归因审计噪声 | 仅 `tools/call` 触发查询与忽略审计 | §4.3、M3 |

---

## 十五、实施记录（2026-11-02）〔已实施〕

### 15.1 提交链

`d855987` feat(devenv) 步 1 → `96cb392` feat(audit-actor) 步 2 → `33fc743` feat(api-devenv) 步 3 → `df83f01` feat(mcp-caller) 步 4/5 → docs(t088) 本节 → chore(todo) → chore(memory)。

**验收总况**：`cargo test --workspace` 退出 0——**91 套件 ok / 662 passed / 0 FAILED**；`cargo check --workspace --all-targets` 0 error / 0 warning（M9 discovery 测试存量 1 条 unused warning 与本条无关）。

### 15.2 落地物（按步）

| 步 | 提交 | 内容 |
| --- | --- | --- |
| 1 | `d855987` | 迁移 `0027_devenv.sql`（devenv 两表 + 四条跨租户组合 FK + 两张审计表 `claimed_device_id`）；`caller.rs`（HumanActor/DeviceClaim/CallerContext 三层）；`authz.rs`（require_project_member 真人+租户维度；resolve_project_executor dev_agent/ai_pm 缺失即报错）；`devenv/`（register/bind/unbind/checks/revoke/list/get，D5 id 优先 + ON CONFLICT 并发闸 + 重绑清 verified 同事务 + 服务端算 readiness）；`AuditActor` 增 `claimed_device_id`；`tests/devenv.rs` 14 用例（C177 跨租户四例 DB 拒绝 / C181 并发收敛 / C182 Ready 不随调用人变 / C183 解绑重绑 / 迁移形状） |
| 2 | `96cb392` | **12 处生产审计 INSERT 统一四列**（8 处升级 + 4 处补 device 列，跨三 crate）；`ExecutionContext.actor_id → actor: AuditActor`（+`actor_id()` 访问器）；`DecideRequest`/`VetoRequest` 增 `#[serde(skip)] actor`（pm-api 填、legacy 回退仅执行主体）；`audit_run_as` 显式入口（audit_run 猜人回退仅存 worker 路径）；outbox handler 的 claimed_device_id 与 `task_run.executor_id` 同源同值；`tests/audit_actor_coverage.rs` 清单驱动两道闸（C174/C180） |
| 3 | `33fc743` | `auth.rs`：`AuthedPrincipal::human_actor()` + resolve_principal / inject_dev_principal 同点插 `Extension<HumanActor>`（唯一生产点）；`device_claim.rs`：attach_device_claim 唯一校验点（仅 tools/call 查询与忽略审计；四类非法 → ignored(reason) 不拒绝；忽略事件写 tenant_audit_log；写失败不升级 M4）；`pm_devices.rs` 八端点；`tests/pm_devices.rs` 5 用例（C176 HTTP 侧 / C184 仅 tools/call） |
| 4/5 | `df83f01` | `pm-mcp/caller.rs`：caller_from_ctx 只读 extensions（无 IO，C179）+ authorize（真人成员门 → 执行主体 → 项目级 AuditActor）+ ensure_member；server.rs 17 handler 增 `ctx: RequestContext<RoleServer>`；tools.rs 16 参数结构全接（get_submission_status 经 inbound_event 反查）；confirmed_by/vetoed_by 以认证调用人覆盖（§4.6）；Run 路径：ClaimOptions.audit_actor → executor_id 落列 + audit_run_opt、report_progress_as、SubmitResultRequest.caller 全管线透传；`tests/caller_ctx.rs` 6 用例（C173 家族覆盖+零副作用 / C176 同源同值 / C178 映射+不回退）；tools.rs smoke 加模拟鉴权层（**C175 端到端注入证据**）；既有 3 文件 39 调用点 + doc_write.rs 补测试调用人 |

### 15.3 实施期裁定与发现

1. **T-059 的「rmcp 拿不到请求」阻塞前提不成立**（实施前 spike 实证）：rmcp 3.8 `tower.rs:1538/2069/2154/2275` 注入 `http::request::Parts` → `service.rs:1579` swap 进 `RequestContext.extensions`，官方文档即此用法。**候选①/②之争消解为「α：原生通道」**，零 rmcp 改动、零 transport 风险、T-081 契约测试不受影响。T-059 审计列随之解阻（本条已落列，剩 `execution_mode` 归 T-059）。
2. **审计点盘点修正**：V1.1 稿 12 处 → 实施中新增 `device_claim.rs` 忽略事件 1 处 = **13 处**；清单闸门（C180）在步 3 后实测逮到该新增点并要求登记——机制按设计工作。
3. **`no_executor_assignee` 政策**：claim 政策要求任务 assignee = dev_agent + 至少一可写仓——测试夹具照 contract.rs 同构（assignee + verified repo + task_repository 行）。
4. **MCP 客户端须发 `Accept: application/json, text/event-stream`**（rmcp 否则 406）与 `Host` 头（stateless 直答要求）——进 T-089/T-090 的指引与适配层。
5. **reqwest 测试 `.no_proxy()`**（本机代理 10808 拦截 loopback）——沿用仓内既有配方。
6. **`audit_log.actor_identity_id` / `claimed_device_id` 都有 FK**——测试必须种真实 `principal_identity` / `devenv.device` 行，不能伪造 id；这也实证了「设备声明不可能指向不存在的设备」的库层保证。
7. **bootstrap 不建 `tenant_member`**（生产由登录链建）——涉及设备登记的测试夹具需补成员行；`fk_device_tenant_owner` 的 NO ACTION 语义与 M2 成文规则一致。

### 15.4 验收对照（§9.1 → 实测）

| 编号 | 结果 | 落点 |
| --- | --- | --- |
| C145/C146/C147/C149/C151/C157/C161 | ✅ | devenv 端点 + checks + 审计（devenv.rs / pm_devices.rs） |
| C152/C156 | ✅ | 身份对齐与 Binding 事实语义（devenv.rs / caller_ctx.rs） |
| C163/C164/C165/C167/C168/C171 | ✅ | pm_devices 5 + caller_ctx 6 + devenv 14 |
| C173 | ✅ | caller_ctx（家族覆盖 + 零副作用 + inbound_event 反查）+ pm-api 六行矩阵部分 |
| C174/C180 | ✅ | audit_actor_coverage 清单两道闸（13 处生产 INSERT 全四列、无清单外写入点） |
| C175 | ✅ | tools.rs smoke（真实 HTTP→rmcp→handler 的 extension 注入证据） |
| C176 | ✅ | pm_devices HTTP 侧 + caller_ctx 四类声明不落归因 |
| C177/C178/C179/C181/C182/C183 | ✅ | devenv.rs（DB 拒绝四例 / dev_agent 映射说明 / 并发收敛 / 多 Project Ready / 解绑重绑） |
| C184 | ✅ | pm_devices（initialize/tools_list 不写忽略审计） |
| **未验（非本条范围）** | — | C141/C143/C150/C159 等 T-089 侧；C148 升级场景；T-081 七项身份门禁（C166）跨机实施时补 |
