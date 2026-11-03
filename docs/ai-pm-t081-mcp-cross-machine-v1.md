# T-081 `/mcp` 跨机暴露 —— 设计口径与实施记录 V1

> 任务：`todo.md` §10.5 T-081（runbook G1；T-087 定稿后并入 M10 V1 前置，第二轮评审升级为**真实部署验收门禁 C166**）。
> 上位依据：`docs/ai-pm-codex-bootstrap-v1.md` V1.3（§〇-A 裁清点 1 / §13.2 评审 P0-3）、`docs/ai-pm-executor-onboarding-runbook-v1.md` §十 G1 / §十一 R2。
> 本稿只记**本条落地的那一层**（服务端 Host 白名单 + 部署侧接线）；验收矩阵与两机实测门禁沿用设计稿，不重写。

---

## 一、范围

**做**：`/mcp` 的 Host 白名单从「代码写死回环」改为**部署可配置**；部署侧（测试机 overlay + env）把放行名单接线到位。

**不做**（沿用 T-081 条目原文）：不做 MCP OAuth；不动鉴权三层链（bearer → resolve → active）；不把 Host 白名单当成授权——**它只是 rmcp 的 DNS-rebinding 防护**，真正放行仍由真人 API Key 与成员校验决定。

---

## 二、三项裁清

任务书只写「`allowed_hosts` 从代码默认改为可配置」，落地前有三个必须选的点。按 §0.1 的判据（「不需要在两个以上方案之间选择就不必 brainstorm」），这三项各自存在实质分叉，故在此显式记录口径与理由。

### 2.1 白名单口径：**只增不减**（不是替换）

- 方案 A（**采用**）：配置的 Host 与 rmcp 默认回环名单**取并集**。
- 方案 B（未采用）：配置值**替换**默认名单。

**理由**：pm-api 的探针与本地自检（`/health`、`/v1/pm/me`、容器内自检脚本）走回环地址；若采用替换语义，运维一旦配了跨机 Host，本机探针会被自己的配置挡掉。回环放行不扩大攻击面（攻击者必须已经在机器上），因此保留。

### 2.2 配置形态：**单个环境变量**，不做配置文件、不做管理面

`AI_PM_MCP_ALLOWED_HOSTS`（逗号分隔）。

- 理由一：与本仓既有部署口径一致——`AI_PM_PUBLIC_BASE_URL` / `AI_PM_JWKS_URL` / `AI_PM_AUTH_MODE` 都是环境变量；新增一份「白名单配置文件」只会多出解析与分发两条路径。
- 理由二：本仓硬规则 1（新项目，不做兼容层）——不做「配置项 A + 兼容旧行为 B」的过渡层。
- 理由三：白名单是**部署事实**（这台机器对外叫什么地址），不是业务数据，不入 `core.*` 表、不进审计。

条目语义沿用 rmcp：**只写 host = 不限端口**；写 `host:port` = 端口须精确匹配。

### 2.3 空名单禁令：**不给「放行全部」的开关**

rmcp 的 `allowed_hosts` 为空即等于放行任何 Host（其源码注释直言 "not recommended"）。因此：

- 环境变量未设置 / 解析后为空 → 视为「没有额外放行」，**回落到回环默认**，绝不产生空名单；
- 本仓不提供 `*` 或 `disable_allowed_hosts()` 之类的等价开关；
- 代码层用一条断言钉住：允许名单**永不为空**（`crates/pm-mcp/tests/hosts.rs`）。

---

## 三、接线

| 层 | 落点 | 内容 |
|---|---|---|
| 传输层 | `crates/pm-mcp/src/server.rs` | `PmMcpServer::streamable_http_service(pool, extra_allowed_hosts: Option<Vec<String>>)`；`default_allowed_hosts()` 合并回环默认 + 额外名单（去空白、去重、**永不为空**） |
| 应用层 | `apps/pm-api/src/mcp_hosts.rs`（新） | `McpHosts { extra_allowed }` + `McpHosts::from_env()` + 纯函数 `parse_allowed_hosts()` |
| 装配层 | `apps/pm-api/src/routes.rs` | `build_router(..., mcp_hosts)`；生产构造器读 `McpHosts::from_env()`，测试构造器用 `McpHosts::default()`（**环境不变量污染测试**） |
| 部署层 | `infrastructure/docker-compose.test-server.yml` | 注入 `AI_PM_MCP_ALLOWED_HOSTS` 与 `AI_PM_PUBLIC_BASE_URL`，均用 `${VAR:?…}` 强制非空（忘配就让 compose 直接失败，而不是跨机 403 时才排查） |
| 引导层 | `infrastructure/scripts/setup-test-server.sh` | 提示这两个**机器相关、非密钥**变量必须按本机对外地址填写（口令类才自动生成） |
| 引导文案 | `crates/pm-application/src/onboarding/mod.rs` | 接入指引的 `known_constraints` 改为点名环境变量（原来只说「需先配置服务端允许的 Host」） |

> 生产叠加 `docker-compose.prod.yml` 时宿主端口被清空（外部走 `aipm-web:8091` 反代），本条**不改动生产叠加层**：生产拓扑下 `/mcp` 的对外形态由反向代理决定，Host 白名单随反代的域名在部署侧声明。

---

## 四、验收证据

### 4.1 自动化用例

| 用例 | 位置 | 断言 |
|---|---|---|
| 默认白名单拒跨机 Host | `crates/pm-mcp/tests/hosts.rs` | `Host: 192.168.2.10:8090` → **403** |
| 默认白名单仍接回环 | 同上 | `Host: 127.0.0.1:8090` → **200** |
| 声明的额外 Host 放行 | 同上 | `Host: 192.168.2.10:8090` → **200** |
| 未声明的 Host 继续被拒 | 同上 | `Host: evil.example:8090` → **403** |
| `host:port` 精确匹配 | 同上 | `:8443` → 200；`:9090` → 403 |
| 名单永不为空 | 同上 | `default_allowed_hosts(Some(&[]))` 非空 |
| 去空白/去重/保留回环 | 同上 | 名单内容逐项断言 |
| env 解析（纯函数） | `apps/pm-api/src/mcp_hosts.rs` | 逗号分隔 + trim；全空白 → `None`；`host:port` 原样保留 |
| API 层装配 | `apps/pm-api/tests/mcp_hosts.rs` | 经 `router_with_feed_and_hosts`：未声明 → 403；声明 → 200；回环不受影响 |

### 4.2 实测

- `cargo test -p pm-mcp --test hosts` → 7 passed / 0 failed。
- `cargo test -p pm-api --test mcp_hosts` → 3 passed / 0 failed（真实 PG，独立临时库）。
- 全量 `cargo test --workspace` 与 `cargo check --workspace --all-targets` 结果见本稿末尾「实施记录」。

---

## 五、遗留：C166 七项身份链路门禁（**必须从真实 Windows 开发机发起**）

服务端这一层做完了，但 T-081 的**验收**是七项端到端门禁（设计稿 §13.2），禁止用服务器内部自测充当：

1. Windows 开发机可访问服务器 `/mcp`；
2. 认证链路可用现有真人 API Key；
3. MCP 工具获得正确的真人 Principal 上下文；
4. T-051 的 Project Member 校验仍然有效；
5. T-059 的审计归因仍然有效；
6. 长连接/协议交互不被反向代理错误截断；
7. 未认证与越权请求被正确拒绝。

**当前状态**：①的**前置**（Host 白名单）已就位；①~⑦ 的**实测**待两机环境就绪后执行（执行端侧的环境事实见 `todo.md` T-090 条目）。

**执行端缺口（2026-11-03 用户反馈）**：Windows 测试机 `192.168.2.43` 已装 Z-Code，**但未装 MCP**——即该机目前没有可用的 MCP 客户端。因此 §5 的门禁需要先在 `192.168.2.43` 上补一个 MCP 客户端（Trae / Codex CLI / 其他 MCP-capable 执行端），否则「真实 Windows 发起」无法成立。选型决定后，本条的验收步骤按该执行端的配置形态填写。

---

## 六、实施记录

- **提交链**：`feat(t081)`（代码 + 用例 + 部署接线）→ `docs(t081)`（本稿 + `todo.md` 回填）→ `chore(memory)`，三者分开提交（§0.2 第 5 条）。
- **测试库**：`postgres://aipm:aipm@127.0.0.1:5435/aipm`（本机 `aipm-pg` 容器宿主端口 5435；`scripts/cargo-test.sh` 里的 5433 是旧口径，本机未起该端口）。
- **关键结论**：
  1. **Host 白名单不是授权**——它只挡 DNS rebinding；跨机真正放行的是真人 API Key + 成员校验（T-088 已落地）。
  2. **默认零变更**——不设环境变量时行为与改造前逐字相同（回环默认），因此存量部署不需要改配置。
  3. **只增不减**——配置跨机 Host 不会反噬本机探针。
  4. **空名单 = 放行全部**——rmcp 的语义陷阱，本仓用「解析后为空即回落」+ 代码断言双重堵死。
