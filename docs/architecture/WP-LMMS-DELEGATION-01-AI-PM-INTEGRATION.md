# WP-LMMS-DELEGATION-01 — AI-PM 侧集成说明（R1 修订版，对应 Artifact Service 仓库的委托契约草案 v0.2）

| | |
|---|---|
| 状态 | **草案（DRAFT）/ WAITING FOR INDEPENDENT SECURITY REVIEW**——配套文档；完整审计/契约/安全评审/实施计划见 `smark-artifact-service` 仓库 `docs/architecture/WP-LMMS-DELEGATION-01-*.md`；**第二轮独立安全评审通过前不得实现** |
| 基线（R1 修正） | ai-pm **GitHub 权威 `master` @ `d0e72fa6`**（`origin` 为内部同步副本 `origin/master@6f317176`，已落后，不作基线；两基线间 `apps/`、`crates/` 代码逐字节一致）；lmms tag `v1.8.2` @ `93fc3a2e`；smark-artifact-service `origin/main` @ `b5095cf`（AS-1/AS-2 已合并） |
| 结论速览 | LMMS 1.8.2 判定 **B（部分具备）**：身份/JWKS/S2S/文件（有条件，F-24）已就绪，委托 Token 签发不可用 → 由 AI-PM 作为**唯一**签发方（候选 B），AS 以 AI-PM JWKS 验签 + 写路径在线撤销检查（Q1） |

## 1. AI-PM 现状（与委托相关的既有事实，R1 复核）

- 用户身份：LMMS 登录 → RS256 用户 JWT → AI-PM 本地 JWKS 验签（`apps/pm-api/src/auth.rs`，`AI_PM_AUTH_MODE=jwks`，启动 fail-closed）。用户 JWT 不落库、不转发下游。
- **⚠️ R1 关键缺口（P0-02）**：`require_bearer` 调用 `verify_token(token, jwks, None)`（`auth.rs:289`）——**用户 JWT 的 audience 不被校验**。上游事实：LMMS v1.8.2 用户 JWT `aud` 为签发侧硬编码 **`"coder-micro-services"`**（`services/token-service/src/issuer.rs:194`，全平台共享值，**无 AI-PM 专用 audience**，其他 token 类型不设标准 aud）。delegation 签发链路必须建立独立验证链（契约 §3.0）：aud 强制 `coder-micro-services` + `purpose=="user_login"` + `ct==false` + roles 排除 `service` + 仅人类身份（dev-off/cpk/Agent/Service 拒绝）。**"audience 隔离依赖 purpose 而非专用 aud"为实施阻断项，提交第二轮评审裁决；验证链落地并通过测试前 issuer 不得启用（F-25）。**
- 租户模型（P0-01）：上游数值 `tid` 经 `ensure_tenant`（`auth.rs:520-546`）1:1 镜像为 **AI-PM 内部 `core.tenant` UUID**；`AuthedPrincipal.tenant_id: Uuid`（`auth.rs:174`）。**delegation 的 `tenant_id` claim 只能取该内部 UUID**（AS `TenantScope.tenant_id` 即 `Uuid`，`scope.rs:10-13`）；LMMS 数值 `tid` **不得**直接成为 AS scope，仅可作为独立 `lmms_tid` 字符串追溯字段（AS 不用于授权）。AI-PM **不向 AS 提供**任何直接查询 AI-PM 数据库的能力。
- 项目授权：DB 验证成员资格（`support.rs::require_project_visible`，仅项目可见性检查——**不是** Artifact 动作授权，P0-03）；角色以 DB 为准，token roles 不授权（ADR-0001）。
- 出站凭证先例：`ServiceJwtCache::get_for_tenant("file-service", tid, false)` + `SERVICE_BOOTSTRAP_TOKEN` → aud 绑定短时 service-jwt（`apps/pm-api/src/files.rs:96-117`）——候选 B 签发器的模式雏形，仅受众不同。
- smark-artifact-service 客户端：**不存在**（全仓检索无外呼、compose 无容器）。
- 密钥：全 env 注入；`AI_PM_SECRET_KEY` 为**数据加密密钥**（AES-256-GCM，`secret.rs`）。**delegation 签名密钥必须独立**：仅通过**独立环境变量/安全注入**（建议 `AI_PM_DELEGATION_SIGNING_KEY`）提供，**不得复用 `AI_PM_SECRET_KEY`**，不入库、不入 git、不入文档、不入测试输出。
- ⚠️ 安全卫生 P0（F-22，R1 事实更正）：仓库根 `.test-env.md` 含明文 SSH/root、LLM API key、GitLab 凭据——**未进入 git 历史**（未被跟踪且被 `.gitignore:20` 忽略），但轮换要求维持 P0：立即轮换全部凭据并移出工作树。**任何凭据内容不得写入文档、代码、测试输出或 Git。**

## 2. AI-PM 需要新增什么（实施计划阶段 0 + 阶段 1 的 AI-PM 侧任务）

| # | 项 | 要点 |
|---|---|---|
| 0 | **§3.0 验证链（前置闸门，阶段 0）** | delegation 签发接口专属：`verify_token(…, Some("coder-micro-services"))` + purpose 强制 + ct/roles/身份类别过滤；七项逐项测试；**issuer 未过本项不得启用** |
| 1 | **意图接口（P0-03）** | 受控业务意图端点，服务端写死意图→精确 Action 映射（AS 侧 15 动作，`action.rs:6-22`）；**动作授权不能由客户端控制**：调用方不可提交 actions，未知动作/无映射/非人类身份/dev-off/cpk 默认拒发；签发决策依据 = 认证用户 + tenant + project（DB 成员关系）+ 目标资源 + 业务状态 |
| 2 | delegation 签发器 | 独立 RS256 密钥对（独立 env/安全注入，见 §1 密钥条）；claims 契约 §3.1.1：`iss=ai-pm, aud=artifact-service, tenant_id=内部UUID, (lmms_tid 追溯), project_id, actions(映射决定), operation_id/artifact_id/caller_request_id(按意图绑定), iat/exp ≤ +300s（建议 120s）, jti, delegation_id, purpose=delegation`；**签发即写 `pm_delegation` 记录（fail-closed）**；签发事件入审计；不缓存不重用 |
| 3 | `GET /.well-known/jwks.json` | 仅公钥；内网；kid 轮换双活窗口 ≥ 10 分钟 |
| 4 | **撤销能力（Q1 已裁决，P0 必做）** | 新迁移：`pm_delegation` + `pm_delegation_revocation` 表（jti 主键、状态、reason、7 天保留）；内部查询端点 `GET /internal/delegation-revocations/{jti}`（service 身份认证，AS 写路径每请求直查，**AI-PM 停机 = AS 写平面 fail-closed 503**——已知情接受）；移出项目/用户禁用/取消 Job 的事务内**批量撤销** active jti；管理员紧急撤销按用户/项目批量 |
| 5 | artifact-service 客户端 | base URL/超时/Idempotency/`caller_request_id` 注入；凭证=单次 delegation token；错误码 401/403/409/503 透传（503 含 `REVOCATION_UNAVAILABLE`） |

## 3. 与 AS / LMMS 的边界对齐点（红线，R1 更新）

1. AI-PM 是唯一委托签发方（Q6：V1 单 issuer）；**决策与签名同一服务**；不把 LMMS 用户 JWT 传给 AS（候选 D 否决：aud 全平台共享 + TTL 缺省 3600s + 无 project 语义）。
2. 不要求 AS 理解 AI-PM 角色体系；只传意图映射后的 `actions` 白名单（15 精确动作，禁止 wildcard）。
3. Worker/执行体永不接触 delegation token：Job 创建后由 AS-2 durable caller identity + 输入快照 + lease 机制接管。
4. LMMS 1.8.2 冻结不改：`/tokens/resource` 不可用（未鉴权、缺 aud/tid、**TTL 输入缺陷 F-28**、无消费者）——不要基于它做集成；file-service 走既有 service-jwt 模式（受 F-24 信任边界限制，链路未满足六条受信要求前保持 fail-closed）。
5. `X-Project-Id`（AS 侧过渡头）**仅资源定位**（Q2）：claims 是唯一授权依据；AI-PM 客户端必须保证 header 与 token `project_id` 同源（同一 DB 验证结果），不一致时 AS 返回 403。
6. `caller_request_id` 仅覆盖 AS Job 创建路径的幂等，**不是所有 Artifact API 的通用重放防护**（P0-04：其余路径靠操作绑定 + 各自幂等 + TTL/撤销）。

## 4. Q1–Q6 冻结裁决（R1，AI-PM 侧视角）

| # | 裁决 | AI-PM 侧含义 |
|---|---|---|
| Q1 | 写操作在线 jti 撤销检查；TTL 建议 120s、上限 300s | 撤销存储 + 查询端点为 **P0 必做**；写路径每次查询、不可用 fail-closed |
| Q2 | Header 仅定位，claims 唯一授权依据 | 客户端 header 与 token project 同源；不构造 scope |
| Q3 | 所有受保护 API（含 GET）都要求 delegation | AI-PM 每次调 AS（含读）都需持有效 delegation |
| Q4 | 15 精确动作，禁止 wildcard | 意图映射只允许映射到这 15 个 |
| Q5 | JWKS 缓存最大陈旧期限明确化（AS 侧 15min），过期或刷新失败 fail-closed | AI-PM JWKS 端点需稳定可用；轮换窗口 ≥10min |
| Q6 | V1 单 active issuer：AI-PM | AI-PM 是唯一签发方；LMMS 未来接入需复审 |

以上仍为**评审裁决建议**：契约保持 DRAFT，第二轮独立安全评审通过前不启动实施（阶段 1）。

## 5. 顺序与依赖

1. 阶段 0（验证链 0.1 + F-22 凭据轮换 0.3）可先行准备；audience 阻断项（0.2）以第二轮评审结论为准；
2. **第二轮独立安全评审通过** → 启动阶段 1（意图接口+签发器+撤销+JWKS+客户端 ↔ AS 验证器+缓存策略+issuer 配置），双侧可并行，联调在 dev compose；
3. 文件链路属阶段 3（AS 侧过渡适配器），AI-PM 仅需配置服务身份 service-jwt；
4. LMMS 后续需求（resource token 修复含 F-28、upload-intent/finalize、撤销链路、aud 白名单）不阻塞上述任何一步。

详细任务表/测试矩阵/回滚见 AS 仓库 `WP-LMMS-DELEGATION-01-IMPLEMENTATION-PLAN.md` §2/§5/§6；契约全文见 `WP-LMMS-DELEGATION-01-AUTHORIZATION-CONTRACT.md`（R1 草案 v0.2）。
