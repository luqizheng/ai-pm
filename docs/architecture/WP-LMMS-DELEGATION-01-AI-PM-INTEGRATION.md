# WP-LMMS-DELEGATION-01 — AI-PM 侧集成说明（对应 Artifact Service 仓库的委托契约草案）

| | |
|---|---|
| 状态 | 草案——配套文档；完整审计/契约/安全评审/实施计划见 `smark-artifact-service` 仓库 `docs/architecture/WP-LMMS-DELEGATION-01-*.md` |
| 基线 | ai-pm `origin/master` @ `6f317176`；lmms tag `v1.8.2`；smark-artifact-service `origin/main` @ `b5095cf`（AS-1/AS-2 已合并） |
| 结论速览 | LMMS 1.8.2 判定 **B（部分具备）**：身份/JWKS/S2S/文件已就绪，委托 Token 签发不可用 → 由 AI-PM 作为签发方（候选 B），AS 以 AI-PM JWKS 验签 |

## 1. AI-PM 现状（与委托相关的既有事实）

- 用户身份：LMMS 登录 → RS256 用户 JWT → AI-PM 本地 JWKS 验签（`apps/pm-api/src/auth.rs`，`AI_PM_AUTH_MODE=jwks`，启动 fail-closed）。用户 JWT 不落库、不转发下游。
- 项目授权：DB 验证成员资格（`support.rs::require_project_visible`、MCP `require_project_member`）；角色以 DB 为准，token roles 不授权（ADR-0001）。
- 出站凭证先例：`ServiceJwtCache::get_for_tenant("file-service", tid, false)` + `SERVICE_BOOTSTRAP_TOKEN` → aud 绑定短时 service-jwt（`apps/pm-api/src/files.rs:96-117`）——**这正是候选 B 签发器的模式雏形，仅受众不同**。
- smark-artifact-service 客户端：**不存在**（全仓检索无外呼、compose 无容器）。
- 密钥：全 env 注入；`AI_PM_SECRET_KEY` 为数据加密密钥（AES-GCM），**不可复用**为签名密钥。
- ⚠️ 安全卫生 P0：仓库根 `.test-env.md` 含明文 SSH/root/API key/GitLab 凭据——建议立即轮换并移出 git（详见 AS 仓库 SECURITY-REVIEW F-22）。

## 2. AI-PM 需要新增什么（实施计划阶段 1 的 AI-PM 侧任务）

| # | 项 | 要点 |
|---|---|---|
| 1 | delegation 签发器 | 独立 RS256 密钥对（env 注入）；`issue_delegation()`：`iss=ai-pm, aud=artifact-service, tid, project_id, actions(最小集), iat/exp≤+300s, jti, delegation_id, biz{task_run_id, caller_request_id}, purpose=delegation`；签发前必须完成既有项目授权判断；签发事件入审计；不缓存不重用 |
| 2 | `GET /.well-known/jwks.json` | 仅公钥；内网；kid 轮换双活窗口 ≥ 10 分钟 |
| 3 | artifact-service 客户端 | base URL/超时/Idempotency/`caller_request_id` 注入；凭证=单次 delegation token；错误码 401/403/409/503 透传 |
| 4 | 撤销能力（按裁决 Q1） | jti 黑名单表 + 查询端点；用户移出项目/封禁 → 停止签发（+可选撤销） |

## 3. 与 AS / LMMS 的边界对齐点（红线）

1. AI-PM 是唯一委托签发方；**决策与签名同一服务**；不把 LMMS 用户 JWT 传给 AS（候选 D 否决：aud 全域 + TTL 1h + 无 project 语义）。
2. 不要求 AS 理解 AI-PM 角色体系；只传 `actions` 白名单（AS 侧 15 动作枚举：`artifact:create/version:create/generation:request/generation:cancel/artifact:read/...`）。
3. Worker/执行体永不接触 delegation token：Job 创建后由 AS-2 的 durable caller identity + 输入快照 + lease 机制接管，长任务天然与 token 无关。
4. LMMS 1.8.2 冻结不改：`/tokens/resource` 不可用（未鉴权、缺 aud/tid、无消费者）——不要基于它做集成；file-service 走既有 service-jwt 模式即可。
5. `X-Project-Id`（AS 侧过渡头）与 token `project_id` 不一致时 AS 返回 403——AI-PM 客户端必须保证两者同源（同一 DB 验证结果）。

## 4. 顺序与依赖

1. P0 安全卫生（凭据轮换）可立即执行，无依赖；
2. 委托闭环（签发器+JWKS+客户端 ↔ AS 验证器+issuer 配置+AS-4 接线）双侧可并行，联调在 dev compose；
3. 文件链路属阶段 3（AS 侧过渡适配器），AI-PM 仅需配置服务身份 service-jwt；
4. LMMS 后续需求（resource token 修复、upload-intent/finalize、撤销链路）不阻塞上述任何一步。

详细任务表/测试矩阵/回滚见 AS 仓库 `WP-LMMS-DELEGATION-01-IMPLEMENTATION-PLAN.md` §2/§5/§6。
