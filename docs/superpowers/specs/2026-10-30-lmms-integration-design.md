# ai-pm ↔ lmms 基座对接设计（鉴权 / 错误基座 / 运行时）

- 日期：2026-10-30
- 状态：draft（brainstorming 产出，用户已裁决范围，待批准）
- 仓库：ai-pm（本仓）+ low-memory-micro-services（../low-memory-micro-services，基座）
- 前置事实：基座 libs 1.6.0 已合并 main；**2026-10-30 晚 WireGuard 已连、kellnr 可达（200）**——crate 通道改为 **kellnr registry 版本依赖**（用户二次裁决）；compose 走 lmms 发布的基础模板复制

## 一、意图与背景

用户裁决（2026-10-30）：对接范围 **A+B+C 全量**（鉴权 + 错误基座 + 运行时融合），通道 **kellnr 版本依赖 + compose 模板复制**（2026-10-30 二次裁决，取代早先的 path 先行）。

两边现状：

- **ai-pm**：Rust workspace（pm-domain/application/infrastructure + pm-ai + pm-mcp；apps: pm-api/pm-worker/ai-pm-worker）。PG 纯血（sqlx 0.8 + migrate + uuid/json，no TLS，`query!` 禁用），自带 thiserror 错误，**无任何鉴权**（MCP 服务挂在 pm-api `/mcp`，裸奔），自带 job queue 与 worker，监督闭环（veto/freeze/redline）迭代中。
- **lmms 基座**：libs 17 crate；token-service 已支持 **client_credentials（user 绑定）+ API key（user+expiry）**（分支 feat/token-client-credentials 的工作，已随合并进 main）；libs/authz 已含 **PM_MCP_READ / PM_MCP_DOCUMENT_WRITE / PM_TASK_UPDATE** 权限码——当初就是为本对接开的。
- **鉴权机制**：`libs/common-jwt` 提供 `JWKSCache`（从 token-service `/.well-known/jwks.json` 拉公钥、本地验签）与 `service_jwt`（服务间 token）；token-service 同时暴露 `/tokens/.well-known/jwks.json` 别名。
- **DB 层不对接**（裁决）：ai-pm 的 PG 栈（migrate/uuid/json）比 common-db 通用层更贴合，保留自家实现；基座 PG 方言能力留给未来其他消费方。

## 二、Phase 1：鉴权对接（最先做）

**信任模型**：调用方（Java pm 平台的 AI agent、运维脚本）向 lmms token-service 获取 token（client_credentials 或 API key），持 `Authorization: Bearer` 调用 ai-pm；**ai-pm 用 common-jwt 的 JWKSCache 本地验签**（信任 lmms token-service 的 JWKS），不逐请求外呼。

1. **依赖**（kellnr）：`digua_common_jwt`、`digua_authz`（权限码常量）。`.cargo/config.toml` 定义 `[registries.kellnr]`，Cargo.toml 以 `{ version = "1.6.0", registry = "kellnr" }` 引用；升级=改版本号（kellnr 上游发版由 lmms `coder release publish rust` 负责）。
2. **挂点**：`pm_api::routes::router` 加 `middleware::from_fn_with_state` 鉴权层——`Authorization: Bearer <jwt>` 验签 + 过期校验；`/health`、`/.well-known/jwks.json`（如透传）、Token 校验自用端点豁免；`/mcp`（streamable-http）在同一层校验（rmcp 挂载前的 HTTP 中间件，不进 MCP 协议层）。
3. **权限执行**：token claims 中的 scope/permission 字段对照 `authz::perms` 的 `PM_MCP_READ`（只读工具）、`PM_MCP_DOCUMENT_WRITE`（文档写入类工具）、`PM_TASK_UPDATE`（任务状态类工具）映射到 pm-mcp 各 tool 与对应 REST 路由；具体 claim 字段名以 token-service 实际签发为准（建造者核对 `services/token-service` 的 claims 构造）。
4. **配置与降级**：`AI_PM_AUTH_MODE = jwks | off`（默认 **jwks**，fail-closed；`off` 仅限本地开发显式设置）；`AI_PM_JWKS_URL`（默认 lmms token-service 地址）、`AI_PM_JWKS_ISSUER`。jwks 拉取失败 → 401 + 明确错误码（不静默放行）。
5. **客户端配套**：对接说明写清调用方如何取 token（token-service 的 client_credentials 端点，含 user 绑定参数），Java pm 侧适配不在本仓范围。

## 三、Phase 2：错误与可观测基座

1. **common-error 边界转换**（不深改 domain）：pm-domain/application 保留 thiserror；在 pm-infrastructure / API handler 边界统一转 `digua_common_error::AppError`（错误码对齐 lmms 体系），响应体错误结构两边一致。
2. **可观测对齐**：ai-pm 已用 tracing；补 OTLP 导出配置（指向 lmms 观测栈的 collector），log 格式对齐 lmms 的 JSON tracing 约定。`digua_common_log` 是否引入由建造者按「能对齐且不拖依赖」判断，不强求。

## 四、Phase 3：运行时融合

1. **编排**：新增 `docker-compose.ai-pm.yml`，把 ai-pm（pm-api/pm-worker/ai-pm-worker + aipm-pg）接入 lmms 基础设施网络（共享 RabbitMQ/观测栈；PG 保持 ai-pm 独立实例）；不动 lmms 主 compose 文件。
2. **事件总线**：盘点 ai-pm-worker 现有 inbound event 通路后决定——若接入 RabbitMQ 则经 lmms `common-event` 约定（outbox/交换机命名），若现通路已满足则仅做连通性验证。**不重构 ai-pm 的监督闭环**。
3. **文档**：对接 runbook（密钥/JWKS 配置、本地起全栈、调用方取 token 示例）。

## 五、约束与非目标

- **ai-pm 并行会话风险**：本仓有其他会话活跃开发。对策——动手前 `git status` 体检（本轮已做：clean、master==origin）；对接代码全部落在**新增文件/新增模块**（auth 中间件模块、新 compose 文件、新增 error 转换层），避免改现役文件；每完成一小步立即 commit。
- **内网**：WireGuard 已连、kellnr/npm/maven registry 可达；docker registry（192.168.2.182）仍故障，镜像推送仍挂起。
- 非目标：DB 层替换（裁决不对接）；Java pm 侧改造；ai-pm 进 lmms 的 dist 分发体系；Rust 之外的语言集成。
- lmms 基座为 **path 引用**：两仓必须同机共存于 `D:\kudi-project\templates\` 下；基座改动即时生效是特性也是风险（对接期间基座不动 main 之外的分支）。

## 六、已裁决事项

| # | 裁决 | 内容 |
|---|---|---|
| 1 | 范围 | A+B+C 全量（鉴权 + 错误基座 + 运行时），一次设计分三阶段实施 |
| 2 | 通道 | ~~path 先行~~ → **kellnr 版本依赖**（2026-10-30 晚二次裁决，WireGuard 连通后） |
| 5 | compose | 不手写：消费 lmms 发布的 compose 基础模板（`pnpm run compose:template`）复制适配；同名项目并入 superpowers 网络 |
| 3 | DB 层 | 不对接，ai-pm 保留自家 PG 栈 |
| 4 | 安全默认 | 鉴权默认 fail-closed（jwks 模式），off 仅显式开发用 |

## 七、留给建造者的决策

- token claims 的具体字段（scope/permissions/sub）与解析实现——以 `services/token-service` 实际签发为准。
- rmcp streamable-http 的鉴权挂点实现细节（axum middleware 与 rmcp service 的层级关系）。
- Phase 2 中 `digua_common_log` 取舍；AppError 转换层的模块位置。
- Phase 3 ai-pm-worker 事件通路盘点后的接法；compose 网络命名与端口规划。
- 各新模块的文件布局与测试策略（path 依赖下 cargo test 直接可用）。

## 八、验收判据

1. 无 token 访问 pm-api 任意业务路由与 /mcp → 401；scope 不足 → 403；`PM_MCP_READ` token 可调只读工具、不可调写入工具。
2. `AI_PM_AUTH_MODE=off` 时行为与现状一致（本地开发可用）。
3. jwks 不可达时启动/请求 fail-closed，错误信息明确。
4. 错误响应结构两边同构；OTLP 导出在 lmms 观测栈可见 ai-pm 的 traces。
5. `docker compose -f infrastructure/docker-compose.yml up -d aipm-pg pm-api ...` 起通 ai-pm 服务（编排=模板复制；2026-11-03 起 ai-pm 服务并入 `infrastructure/docker-compose-services.yml`，登录验证栈同文件，见 infrastructure/README.md），与 lmms 基础设施网络互通。
6. ai-pm 现有测试全绿；新增鉴权/转换层测试覆盖每个验收点。
