# T-080 · 执行端 URL 引导式接入（onboarding endpoint）· 设计稿 V1

| 项 | 内容 |
| --- | --- |
| 文档性质 | 设计稿（接口契约 + 指引内容契约 + 测试矩阵），已过稿 |
| 版本 | V1（2026-11-02） |
| 记录 | 司南（产品经理 Agent）按用户 11 问裁定落盘 |
| 状态 | **服务端已实施 + M-01（Trae）端到端通过**（2026-11-02）：§六.1 自动化用例全绿；Trae 从 URL 到 11 个工具全链路验证；**M-02（Codex）阻塞于执行端模型 500，待补测**（见 §11.6） |
| 上位约束 | `docs/ai-pm-executor-onboarding-runbook-v1.md`、`docs/ai-pm-mcp-executor-positioning-v1.md`、`docs/ai-pm-principal-identity-v1.md`、`docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`、`docs/ai-pm-codex-bootstrap-v1.md`（T-087 的直系前身）、`docs/ai-pm-architecture-principles.md` |
| 关联条目 | todo.md §10.5 M8：T-080（本条）/ T-081（`/mcp` 跨机暴露）/ T-082（`pm_list_tasks`）；T-051（预注册档位，保持开放）；T-059（MCP 审计三列） |
| 前置 | 无（不依赖 T-081；跨机场景在 T-081 完成后自然可用） |

---

## 〇、一句话结论

平台暴露一条**公开、稳定、agent 可读**的接入 URL（`/install/{tool}`）：用户把它粘给执行端 agent 并说「帮我安装」，agent 拉取该 URL 得到该工具的配置片段与验证步骤，写入 MCP 配置后自检，直到 `GET /v1/pm/me` 返回 200。全程用户**不手工编辑任何配置文件**，且**任何凭据都不出现在 URL 或指引内容里**。

**用户流程（原话口径）**：第一次安装工具 → 打开 trae / codex 等执行端 → 粘贴平台 URL → 对 agent 说「帮我安装」→ agent 按 URL 返回的安装方法完成安装（写 MCP 配置、引导填 key、验证连通）。

---

## 一、现状盘点（实测，非推断）

### 1.1 已有能力

| 能力 | 位置 | 状态 |
| --- | --- | --- |
| `/mcp` Streamable HTTP 服务 | `crates/pm-mcp/src/server.rs`（`streamable_http_service`） | ✅ 11 个工具，stateless JSON 模式 |
| `/mcp` 路由挂载 | `apps/pm-api/src/routes.rs:209`（`.nest("/mcp", mcp_router)`） | ✅ 鉴权时套 `require_mcp_permission` |
| 主体自查 | `GET /v1/pm/me`（`apps/pm-api/src/principals.rs:27`） | ✅ 未激活主体也可读（`is_active` 字段在此） |
| 鉴权三层链 | `require_bearer` → `resolve_principal` → `require_active` | ✅ `/v1/pm/me` 已在 `is_self_service_path` 放行名单内 |

### 1.2 三个真实缺口

| # | 缺口 | 影响 | 本条处置 |
| --- | --- | --- | --- |
| G1 | **接入 URL 不存在** | 用户没有任何「粘贴给 agent」的入口 | 本条新建 `/install` + `/install/{tool}` |
| G2 | **`/health` 路由不存在** | runbook §二 / §三 把「`GET /health` 200」当作部署校验与安装验证的第一步，实际请求会 404 | 本条新建 `GET /health` |
| G3 | 无「分工具配置片段」 | agent 拿到 URL 也不知道 trae 的 MCP 界面长什么样、codex 是 TOML 还是 JSON | 本条内置工具清单 |

> `apps/pm-api/src/auth.rs:259` 的 `is_self_service_path` 已经把 `/health` 写进放行名单——**放行名单存在、路由不存在**。这不是笔误，是历史遗留的半截实现。

### 1.3 一条不做的事（明确记录，避免误记）

`crates/pm-mcp/src/server.rs` 的 rmcp `StreamableHttpService` **本条不动**。它带来三类约束：Host 白名单默认回环（跨机 403）、强制 `Accept` 头含 `text/event-stream`（缺失 400）、工厂闭包拿不到 request extensions（T-059 审计三列被卡死）。这些分别归 **T-081** 与 **T-059**。

> 用户 2026-11-02 曾提出「不采用 MCP 标准」，最终裁定为**保留 MCP 协议与 rmcp 传输层**。因此本条是「在既有 MCP 实现之上补接入层」，不是「另立传输层」。

---

## 二、范围裁定（2026-11-02 用户 11 问逐条确认）

| # | 议题 | 裁定 |
| --- | --- | --- |
| 1 | URL 形态 | **双形态**：默认 Markdown（人看）+ `Accept: application/json` 或 `?format=json` → JSON（agent 读）；同一 URL 内容协商 |
| 2 | 分工具寻址 | **路径变体** `/install/{tool}`（与 Bootstrap 稿 §一 的 `/install/codex` 一致）；`/install` 返回清单 |
| 3 | 密钥入口 | **用户把 key 交给 agent，agent 写入配置**；URL 与指引内容**不含 key** |
| 4 | 验证闭环 | `/health` 200 + `/v1/pm/me` 200（`is_active=true`）；未激活给出明确下一步而非报错；**本条补建 `/health`** |
| 5 | 预注册档位（T-051） | **不登记**；`client_name` 仍由 `pm_claim_task` 时执行端上报 |
| 6 | 租约/心跳默认值 | **指引给建议值，不改服务端默认**（默认仍 `AI_PM_RUN_LEASE_SECS=300`） |
| 7 | 指引内容源 | **代码内置工具清单** + `AI_PM_PUBLIC_BASE_URL` env 注入平台地址 |
| 8 | 接入页鉴权 | **完全公开**（agent 在拿到 key 之前就要拉取它） |
| 9 | 前端范围 | **只做服务端**；前端「Codex 环境接入」卡片归 T-091 |
| 10 | 工具覆盖面 | **已核实清单 + 通用兜底**：`trae` / `codex` / `workbuddy` 已核实；`z-code` / `dsh` / `deepseek-harness` / `opencode` / `pi` 及任何未知 slug 落通用 MCP 条目 |
| 11 | 指引终点 | 验证到 `/v1/pm/me` 为止；**「开始干活」另起一节**（此刻执行端仍需从 Web 拿 `task_id`，`pm_list_tasks` 归 T-082） |

---

## 三、架构与组件

### 3.1 组件落位

| 件 | 位置 | 职责 |
| --- | --- | --- |
| 接入端点 | `apps/pm-api/src/onboarding.rs`（新） | `GET /install`、`GET /install/{tool}`；**公开**，不进 bearer 三层链 |
| 健康端点 | `apps/pm-api/src/routes.rs` | `GET /health` → 200 `{"status":"ok","service":"pm-api"}` |
| 公开路径判定 | `apps/pm-api/src/auth.rs` | `is_public_path()`：`/health`、`/install`、`/install/{tool}`；三层链各自放行 |
| 工具清单 | `crates/pm-application/src/onboarding/catalog.rs`（新） | 内置常量表；**无外部文件、无解析器** |
| 渲染 | `crates/pm-application/src/onboarding/render.rs`（新） | 同一份 `Guide` 结构 → Markdown / JSON |
| 应用层出口 | `crates/pm-application/src/onboarding/mod.rs`（新） | `pub fn install_guide(tool_slug: Option<&str>, base_url: &str) -> Guide` |

**为什么指引内容放 `pm-application` 而不是 `pm-api`**：它是纯数据 + 纯渲染（无状态、不碰库），放应用层可被单元测试直接断言，不需要起 HTTP 服务；同时 `pm-api` 已依赖 `pm-application`，不引入新依赖方向。

**零迁移**：本条不改任何表结构。

### 3.2 公开路径与鉴权链的交互

三层鉴权链（`apps/pm-api/src/auth.rs`）当前对**所有**路径都要求 bearer。接入页必须免鉴权，否则「粘贴 URL 给 agent」的第一步就走不通。

| 层 | 本条处置 |
| --- | --- |
| `require_bearer` | 公开路径直接 `next.run`（不验签、不插 `AuthenticatedIdentity`） |
| `resolve_principal` | 公开路径直接 `next.run`（无 principal 可解析） |
| `require_active` | 公开路径直接 `next.run`；`/v1/pm/me` **保持现状**（仍走三层，只是 `require_active` 放行） |

判定函数放在 `auth.rs`：

```
is_public_path(path):
  path == "/health" 或 path == "/install" 或 path.starts_with("/install/")
```

**`/v1/pm/me` 不是公开路径**——它是验证链的一环，必须带 bearer；但未激活主体可读（`require_active` 已在放行名单内），这正是引导页要读 `is_active` 的前提。

### 3.3 平台地址来源

| 项 | 值 |
| --- | --- |
| env | `AI_PM_PUBLIC_BASE_URL` |
| 缺省 | `http://127.0.0.1:8090` |
| 缺省时告知 | JSON 响应带 `base_url_source: "default"`，让 agent 知道地址可能是错的 |
| 规范化 | 去掉尾部 `/`；scheme 只允许 `http` / `https`；其余按原样返回并由日志告警 |

`AI_PM_PUBLIC_BASE_URL` 是本条**唯一新增环境变量**。

---

## 四、指引内容契约

### 4.1 工具清单条目

```
slug / display_name / verified(bool) / config_target / config_format / config_template
```

| slug | verified | config_target | config_format |
| --- | --- | --- | --- |
| `trae` | ✅ | Trae 设置 → MCP Servers → 添加 **HTTP** 类型服务器 | `json`（`mcpServers.<name>.url` + `.headers`） |
| `codex` | ✅ | `~/.codex/config.toml`（用户级）或 `<项目>/.codex/config.toml`（受信任项目） | `toml`（`mcp_servers.<name>.url` + `http_headers`） |
| `workbuddy` | ✅ | `mcp.json` | `json`（同 trae 形状） |
| `z-code` / `dsh` / `deepseek-harness` / `opencode` / `pi` | ❌ | 「先确认你这个工具的 MCP 配置文件在哪」 | `json`（通用 `mcpServers` 形状） |

> 已核实格式的三个来自各工具公开文档；其余五个本仓无任何格式记录，**不猜格式**——给通用 `mcpServers` JSON 片段 + 一句让 agent 先确认配置文件位置。实测后把 `verified` 翻成 `true` 并补精确片段即可，**无需改协议**。

**未知 slug 不 404**：落 `verified=false` 的通用条目（用户裁定：「支持 MCP 就能接入」）。

### 4.2 JSON 契约（agent 读，字段名定死）

```json
{
  "platform_base_url": "http://127.0.0.1:8090",
  "base_url_source": "default | env",
  "mcp_endpoint": "http://127.0.0.1:8090/mcp",
  "transport": "streamable-http",
  "tool": {
    "slug": "trae",
    "display_name": "Trae",
    "verified": true,
    "config_target": "Trae 设置 → MCP Servers → 添加 HTTP 类型服务器",
    "config_format": "json"
  },
  "config_template": { "...": "该工具的 MCP 配置片段，key 位置用 <API_KEY> 占位" },
  "requirements": [
    "一个支持 MCP（Streamable HTTP）的客户端",
    "平台签发的 API Key（已绑定真人）——请向平台管理员索取"
  ],
  "steps": [
    { "id": "read_guide", "title": "读取本指引" },
    { "id": "ask_api_key", "title": "向用户索取 API Key（不要臆造、不要写入本 URL）" },
    { "id": "write_config", "title": "把 config_template 写入 config_target，<API_KEY> 替换为用户给的 key" },
    { "id": "verify", "title": "按 verify 列表逐条自检" }
  ],
  "verify": [
    { "name": "health", "method": "GET", "url": "http://127.0.0.1:8090/health",
      "headers": {}, "expect_status": 200 },
    { "name": "identity", "method": "GET", "url": "http://127.0.0.1:8090/v1/pm/me",
      "headers": { "Authorization": "Bearer <API_KEY>" },
      "expect_status": 200, "expect_field": "is_active" }
  ],
  "activation": {
    "when": "identity 响应中 is_active == false",
    "next_step": "请平台管理员执行 PATCH /v1/pm/principals/{principal_id}，body {\"is_active\": true}，然后重试 identity 步骤。",
    "endpoint": "PATCH /v1/pm/principals/{principal_id}"
  },
  "next_steps": {
    "claim": "从 Web 控制台挑选 ready 任务拿到 task_id 与 project_id，再调用 pm_claim_task 领取（当前版本没有任务列表工具，执行端不能自行发现任务）。"
  },
  "lease_hint": {
    "lease_secs": 300,
    "recommendation": "人工档建议租约 ≥ 7200s、心跳 ≤ 1800s（需运维调整 AI_PM_RUN_LEASE_SECS；服务端默认 300s）。"
  },
  "known_constraints": [
    "/mcp 默认只接受回环 Host；跨机接入需先配置服务端允许的 Host（T-081）。"
  ]
}
```

**凭据边界（硬规则）**：URL、`config_template`、Markdown 全文中**不得出现任何真实 key**。key 只以占位符 `<API_KEY>` 出现在需要用户填的位置；`Authorization` 的值形如 `Bearer <API_KEY>`。

### 4.3 Markdown（人看）

同一份事实的渲染，顺序固定：

1. **平台地址**：`mcp_endpoint` + transport。
2. **配置片段**：代码块，`<API_KEY>` 显式标注「在此填入你的 API Key」。
3. **三步验证**：`/health` → `/v1/pm/me` → `is_active` 判定。
4. **未激活路径**：`PATCH /v1/pm/principals/{principal_id}`（管理员动作）。
5. **开始干活**：从 Web 拿 `task_id` → `pm_claim_task`。
6. **租约建议**。
7. **已知约束**（Host 白名单）。

### 4.4 Host 头：写，但值从部署地址推导

**更正 runbook §三 的「必带头 Host」为「建议显式带头 Host」**，理由：

- rmcp 的 `parse_host_header` 优先读客户端自带的 `Host`，缺失时才回落 `uri.authority()`；其源码注释明确写着「`axum::Router::nest` can drop the `Host` header hyper synthesizes from it」——**pm-api 恰恰是 `nest("/mcp", …)`**，所以显式带更稳。
- rmcp 默认 `allowed_hosts = ["localhost", "127.0.0.1", "::1"]`（**不限端口**），因此 `Host: 127.0.0.1` 或 `127.0.0.1:8090` 都通过。

因此模板里的 `Host` 值 = **部署地址的 host（带非默认端口）**，而不是写死 `127.0.0.1`。写死会在跨机部署下把请求指向错误的 authority。

---

## 五、数据流与错误处理

### 5.1 请求流

```
GET /install/{tool}（无 token）
  → require_bearer：公开路径 → 放行
  → resolve_principal：公开路径 → 放行
  → require_active：公开路径 → 放行
  → onboarding handler：
      slug = path param（缺省 → 清单）
      校验 slug 字符集（仅 [a-z0-9-]）
      guide = pm_application::onboarding::install_guide(slug, base_url)
      按 Accept / ?format 选 Markdown 或 JSON
```

### 5.2 错误处理

| 情形 | 响应 | 说明 |
| --- | --- | --- |
| 未知 slug（如 `/install/foo`） | 200，通用兜底条目 | 不 404（用户裁定：支持 MCP 就能接入） |
| slug 含非法字符 | 400 `{"message":"invalid tool slug"}` | 防拼接式路径穿越 |
| `/install`（无 slug） | 200，工具清单 | 人看是列表，agent 看是 JSON 数组 |
| `?format=json` 与 `Accept: application/json` 同时出现 | JSON | Accept 优先，二者不冲突 |
| `?format=` 非法值（非 json） | 400 `{"message":"unsupported format"}` | 不静默回落 |
| 缺 `AI_PM_PUBLIC_BASE_URL` | 200，`base_url_source="default"` | 不是错误，是提示 |

---

## 六、测试矩阵

### 6.1 自动化（`apps/pm-api/tests/onboarding.rs`，新）

| # | 用例 | 断言 |
| --- | --- | --- |
| O-01 | `/health` 无 token | 200，`status=ok` |
| O-02 | `/install` 无 token | 200；Markdown 含 `trae` / `codex` / `workbuddy` |
| O-03 | `/install/trae` 无 token | 200；含配置片段与 `<API_KEY>` 占位 |
| O-04 | `/install/{未知 slug}` | 200；`verified=false` 通用条目 |
| O-05 | `/v1/pm/me` 无 token | 401（**不被公开路径误放行**） |
| O-06 | `?format=json` | `Content-Type: application/json`；字段齐全（§4.2 全部键） |
| O-07 | `Accept: application/json` | 同 O-06 |
| O-08 | 默认（无 Accept） | `Content-Type: text/markdown` |
| O-09 | **凭据不外泄** | 全部响应体不含 `Bearer ` + 非占位符；不含 `sk-` 形态串；只允许 `<API_KEY>` |
| O-10 | 非法 slug | 400 |
| O-11 | `?format=xml` | 400 |
| O-12 | `AI_PM_PUBLIC_BASE_URL` 变更 | JSON 的 `mcp_endpoint` / `base_url_source` / 模板 Host 同步变化 |
| O-13 | `/v1/pm/me` 未激活主体 | 200 且 `is_active=false`（引导页据此给下一步） |
| O-14 | `/install/{tool}` 的 JSON 里 `steps`/`verify` 步骤 id 唯一且非空 | 防指引内容自相矛盾 |

### 6.2 手动（2026-11-02 实测，结果见 §11.6）

| # | 场景 | 期望 |
| --- | --- | --- |
| M-01 | Trae：粘贴 `http://127.0.0.1:8090/install/trae` → 「帮我安装」 | agent 拉指引 → 问 key → 写 MCP 配置 → 自检 → `/v1/pm/me` 200；**全程无手工编辑配置文件** |
| M-02 | Codex：粘贴 `http://127.0.0.1:8090/install/codex` → 「帮我安装」 | 同上（TOML 配置） |
| M-03 | 未激活主体 | 给出「请管理员激活」明确指引，而非报错 |
| M-04 | 换用 z-code / dsh / opencode / pi | 走通用条目也能装（若其 MCP 配置格式与 `mcpServers` 兼容） |

M-01/M-02 满足 todo.md T-080 的验收句「用 ≥2 个不同执行端实测『粘贴 URL → 帮我安装 → 配置就位 → `/v1/pm/me` 200』全程无手工编辑配置文件」。

---

## 七、不做（YAGNI）

| 不做 | 理由 / 归属 |
| --- | --- |
| 不在 URL 或指引中传递凭据 | 硬规则（§4.2） |
| 不安装执行端软件本身 | 用户自己的事；Worker/Runner 归 T-087~T-091 |
| 不改 11 个 MCP 工具契约 | 定位文档与 ADR-0002 的边界 |
| 不动 rmcp 传输层 | 用户裁定保留 MCP 标准；Host 白名单归 T-081，审计三列归 T-059 |
| 不登记执行端预注册档位 | T-051 保持开放；`client_name` 由 `pm_claim_task` 上报 |
| 不新增迁移 | 本条零表结构变更 |
| 不做前端入口页 | 归 T-091（Bootstrap 稿 图1） |
| 不改服务端租约默认值 | 指引只给建议值（§4.2 `lease_hint`） |
| 不做无人值守 Runner | T-092 延后 |
| 不做 MCP OAuth / 不动鉴权三层链语义 | T-081 范围除外 |

---

## 八、遗留与后续

| 项 | 归属 |
| --- | --- |
| `/mcp` 跨机暴露（Host 白名单可配置 + TLS + 反代） | T-081 |
| `pm_list_tasks`（执行端自行发现可领任务） | T-082 |
| MCP 审计三列（`actor_client` / `actor_auth_mode`） | T-059（受 rmcp transport 阻塞） |
| 执行端档位机制 | T-051（保持开放） |
| Worker / Skill / 设备授权 / 前端两页 | T-087~T-091 |
| `pm_submit_result` 之外的执行端能力扩展 | 后续切片 |

---

## 九、实施清单（文件级）

| 文件 | 动作 |
| --- | --- |
| `crates/pm-application/src/onboarding/mod.rs` | 新增：`Guide` 结构、`install_guide()`、`tool_catalog()` |
| `crates/pm-application/src/onboarding/catalog.rs` | 新增：工具条目常量表 |
| `crates/pm-application/src/onboarding/render.rs` | 新增：`to_markdown()` / `to_json()` |
| `crates/pm-application/src/lib.rs` | 新增 `pub mod onboarding;` |
| `apps/pm-api/src/onboarding.rs` | 新增：两个 GET 端点 + 内容协商 |
| `apps/pm-api/src/routes.rs` | 新增 `GET /health`；`merge(crate::onboarding::router())` |
| `apps/pm-api/src/auth.rs` | 新增 `is_public_path()`；三层链各自放行 |
| `apps/pm-api/tests/onboarding.rs` | 新增：O-01~O-14 |
| `docs/ai-pm-executor-onboarding-runbook-v1.md` | 补 §三 接入向导小节（指向 `/install/{tool}`） |
| `todo.md` | 状态原地改 + §13 归档 |

---

## 十、验收标准

1. §六.1 的 O-01~O-14 全绿；`cargo test --workspace` 退出码 0；`cargo check --workspace --all-targets` 0 error / 0 warning。
2. §六.2 的 M-01 / M-02 由用户实测通过（Trae + Codex 各一遍，全程无手工编辑配置文件）。
3. 全部响应体不含凭据（O-09 断言）。
4. 设计稿实施记录回填本稿 §十一。

---

## 十一、实施记录（2026-11-02，Trae）

### 11.1 落地文件

| 文件 | 动作 | 说明 |
| --- | --- | --- |
| `crates/pm-application/src/onboarding/mod.rs` | 新增 | `Guide` / `ToolInfo` / `Step` / `VerifyStep` / `Activation` / `NextSteps` / `LeaseHint` / `Catalog`；`install_guide()`、`tool_catalog()`；`host_header_for()`（Host 值从部署地址推导） |
| `crates/pm-application/src/onboarding/catalog.rs` | 新增 | 8 条内置工具条目（3 已核实 + 5 通用）+ `GENERIC` 兜底 + `is_valid_slug()` |
| `crates/pm-application/src/onboarding/render.rs` | 新增 | `to_markdown()` / `catalog_to_markdown()` |
| `crates/pm-application/src/lib.rs` | 改 | `pub mod onboarding;` + 关键类型再导出 |
| `apps/pm-api/src/onboarding.rs` | 新增 | `GET /install`、`GET /install/{tool}`；内容协商；`AI_PM_PUBLIC_BASE_URL` 读取 |
| `apps/pm-api/src/routes.rs` | 改 | 新增 `GET /health`；`merge(crate::onboarding::router())` |
| `apps/pm-api/src/auth.rs` | 改 | 新增 `is_public_path()`；`require_bearer` / `resolve_principal` / `require_active` 三层各自放行 |
| `apps/pm-api/src/lib.rs` | 改 | `pub mod onboarding;` |
| `apps/pm-api/tests/onboarding.rs` | 新增 | O-01~O-14 共 8 个用例 |

无迁移（零表结构变更）。

### 11.2 实现期做出的三个细节决定（设计稿未写死）

1. **JSON 响应带 `charset=utf-8`**。首版只发 `application/json`，实测窄接口客户端（PowerShell 5 的 `Invoke-WebRequest`）按 ISO-8859-1 解码，中文全成乱码。虽然 RFC 8259 规定 JSON 默认 UTF-8，但显式声明 charset 零成本且消除歧义，故加上。Markdown 分支本来就带 charset。
2. **`negotiate()` 的错误类型改为 `NegotiateError`**。原设计让它返回 `Result<Format, Response>`，clippy 报 `result_large_err`（Response ≥128 字节）。改为只携带原因的小枚举 + `IntoResponse`，调用处 `err.into_response()`。
3. **未知 slug 的 `tool.slug` 返回 `"generic"`**，而不是回显请求里的 slug。理由：通用条目在语义上就是一个条目（`display_name = "通用 MCP 客户端"`），回显任意 slug 会让 `tool` 字段自相矛盾。

### 11.3 验收证据

| 项 | 命令 | 结果 |
| --- | --- | --- |
| 用例 | `cargo test -p pm-api --test onboarding` | **8 passed / 0 failed** |
| 全量回归 | `DATABASE_URL=…:5433/aipm cargo test --workspace` | 退出码 **0**，无 `test result: FAILED` |
| 类型检查 | `cargo check --workspace --all-targets` | 退出码 **0**，0 error / 0 warning |
| Lint | `cargo clippy --workspace --all-targets` | 新增四个文件 **0 命中**（其余警告为仓内既有） |

**真实二进制冒烟**（临时库 `aipm_smoke`，`AI_PM_AUTH_MODE=off`、`AI_PM_PUBLIC_BASE_URL=http://192.168.1.50:8090`、`PM_API_PORT=8099`）：

- `GET /health` → `{"service":"pm-api","status":"ok"}`
- `GET /install/trae` → Markdown：平台地址 / 配置片段（`Host: 192.168.1.50:8090`，**非**写死回环）/ 三步验证 / 未激活下一步 / 开始干活 / 租约建议 / 已知约束
- `GET /install/codex?format=json` → `application/json; charset=utf-8`，TOML 片段：
  `[mcp_servers.ai-pm]` + `url` + `http_headers = { Authorization = "Bearer <API_KEY>", Host = "…" }`
- 冒烟结束后已 `DROP DATABASE aipm_smoke`，未触碰开发库 `aipm`。

### 11.4 M-01 / M-02 实测结果

见 §11.6（2026-11-02 已实测）。

### 11.5 发现的环境问题（与本条无关，记录备查）

本机开发库 `aipm` 的 `_sqlx_migrations` 在第 10 号迁移上校验和不匹配（`VersionMismatch(10)`），`cargo run -p pm-api` 会直接退出。这是既有的本地库漂移，`migrations/` 无未提交改动。按 AGENTS.md「不考虑迁移兼容」，重建开发库即可；本次冒烟改用临时库绕开，未动 `aipm`。

### 11.6 M-01 / M-02 端到端实测（2026-11-02）

**测试环境（真实鉴权，非 `AI_PM_AUTH_MODE=off`）**

| 件 | 配置 |
| --- | --- |
| 数据库 | 新建 `aipm_e2e`（避开 `aipm` 的迁移漂移，未改动 `aipm`） |
| pm-api | `PM_API_PORT=8090`、`AI_PM_PUBLIC_BASE_URL=http://127.0.0.1:8090`、`AI_PM_JWKS_URL=http://127.0.0.1:8088/jwks.json`、`AI_PM_FIRST_ADMIN_SUBJECT=usr-admin` |
| JWKS | 仓内 test-only RSA 密钥对（`apps/pm-api/tests/keys/`）生成 JWKS + 签发 RS256 JWT |

**前置闭环（真实 JWT 全链路）**

| 步 | 观测 |
| --- | --- |
| `GET /v1/pm/me`（`usr-e2e` 首次） | 200，`is_active=false`，principal 自动 provision |
| `GET /v1/pm/me`（`usr-admin`，首管理员） | 200，`is_active=true`，`is_admin=true` |
| `PATCH /v1/pm/principals/{id}`（管理员） | 200，`{"is_active":true}` |
| `GET /v1/pm/me`（`usr-e2e` 复检） | 200，`is_active=true` |
| `POST /mcp` initialize + tools/list（带该 key） | 200；`serverInfo=ai-pm-mcp 0.1.0`；**11 个工具** |

**M-01（Trae）— 通过（含工具展示）**

- 按 `/install/trae?format=json` 的 `config_template` 原样写入 `%APPDATA%\Trae CN\User\mcp.json`，新增 `ai-pm` 条目（`url` + `headers.Authorization` + `headers.Host`），既有 `context7` / `godot` 条目未改动。
- 用**落盘后的这份文件本身**读出 url 与 headers（4 个头：`content-type` / `accept` / `Authorization` / `Host`）发起 MCP 握手：`initialize` 200、`tools/list` **11 个工具**。
- **Trae 面板实测（用户确认）**：MCP Servers 列表出现 `ai-pm`，状态正常、开关开启，展开后 11 个工具全部列出（`pm_claim_task` / `pm_confirm_document_proposal` / `pm_get_run_context` / `pm_get_submission_status` / `pm_get_task_context` / `pm_propose_document` / `pm_report_progress` / `pm_submit_document` / `pm_submit_result` / `pm_submit_technical_design` / `pm_veto_document_revision`）→ **接入链路从 URL 到工具面完整打通**。
- 落盘踩坑：PowerShell 5 的 `Set-Content -Encoding utf8` 会写入 **BOM**，Trae 的 JSON 解析器会直接失败；已改用 `UTF8Encoding($false)` 重写，首字节 `123,10,32`（`{`）。

**M-02（Codex）— 阻塞（执行端模型不可用）**

- `codex exec` 两次尝试均在**调用上游模型**时失败：`https://api.minimax.cn/v1/responses` 连续返回 **HTTP 500**，codex 报 `We're currently experiencing high demand`。与接入链无关，是执行端自身的模型通道故障。
- 因此「codex 自主读 URL → 写 config.toml」这一段**未被执行**（非代码问题，非本稿问题）。
- 能证明的部分：把指引给出的 TOML 原样放进临时 `CODEX_HOME` 后，`codex mcp list` 正确识别为
  `ai-pm | http://127.0.0.1:8090/mcp | streamable_http | Bearer token`，`codex mcp get ai-pm` 回显
  `http_headers: Authorization=*****, Host=*****` → **指引里的 TOML 形状被 Codex 0.163 正确解析**。
- 隔离手段：用临时 `CODEX_HOME`（`C:\Users\leo\aipm-e2e-codex-home`）而非用户的 `~/.codex/config.toml`；
  另注：`CODEX_HOME` 放在系统临时目录下会被 codex 拒绝（`Refusing to create helper binaries under temporary dir`）。

**判定**

| 项 | 结论 |
| --- | --- |
| 接入 URL 内容正确性 | ✅ 两种形态、两个工具均正确 |
| 真实 JWT → `/v1/pm/me` → `/mcp` 全链路 | ✅ 通过（未激活→激活→11 工具） |
| M-01 Trae 配置落位与连通 | ✅ **通过（含 Trae 面板 11 个工具可见）** |
| M-02 Codex 自主安装 | ⛔ 阻塞于执行端模型 500，未执行 |

> 结论：T-080 的**服务端、配置契约与其中一个真实执行端（Trae）已端到端验证**；「两个不同执行端均由 agent 自主完成安装」这一验收句仍差 M-02，需在 Codex 模型通道恢复后补测。
