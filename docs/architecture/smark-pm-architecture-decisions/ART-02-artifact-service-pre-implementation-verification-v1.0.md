# ART-02：Artifact Service 实施前置核验报告（第一轮）

> **状态：** 仓库静态核验已完成；运行环境与上游 token/file-service 契约仍开放  
> **核验日期：** 2026-11-24  
> **核验基准：** `master` = `3a50c7cdee3b2ef0d7e19b241b64764a52515fb8`  
> **关联：** ADR-ART-001、ART-00、ART-01  
> **范围说明：** 本报告核验当前 AI-PM 仓库中的代码、迁移、工作流和既有审计记录。它不声称连接了生产数据库、token-service 或 file-service 实例。

## 0. 结论摘要

第一轮代码核验支持 ART-01 的总体方向，但发现两个必须在独立服务实施前解决的契约缺口：

1. **项目授权委托尚未形成可直接复用的 Artifact Service 能力。** 当前 pm-api 的用户 JWT 验证与项目成员授权是两层：JWT 经 JWKS 验签，业务 handler 再通过本地 PostgreSQL 的 `require_project_visible` 查询项目成员。Artifact Service 不能直接复制或调用该数据库逻辑。现有代码没有证明 token-service 已能签发 `aud=artifact-service` 且带 project/action scope 的短时能力凭证。
2. **file-service 已有上传、下载、信息查询、删除和预签下载基础，但未从 AI-PM 当前调用代码证明具备 ART-01 所需的通用 upload-intent / checksum-attestation / finalize 协议。** 不能把已有 `upload_file` 和 `presign` 误认为这些能力已经存在。

其余结论：

- 复用 file-service/RustFS 有明确代码依据，AI-PM 当前没有必要另建 S3 控制面。
- SeeCode 的子进程环境隔离、超时强杀、输出大小限制和 SVG 静态化检查已有实现基础；网络隔离、CPU/内存硬限额由部署层负责。
- 当前 Mermaid 证据只确认 SeeCode 内含 Mermaid importer；未发现独立服务端 Mermaid Generator 的实现证据。
- 当前 Monorepo 的 Rust CI 对 `crates/**`、`apps/**` 变更运行 workspace 级检查/测试，不能满足 Artifact Service 真正独立构建和发布的长期目标。
- 一份 2026-11-15 历史数据库盘点快照中 `viz.*` 四张表行数均为 0；该材料未证明其对应当前生产库，不能据此宣称生产数据为空。

**当前建议：** 保留 ART-01 的独立 Artifact Service + 复用 file-service + AI-PM 保留业务授权和审批的方向。下一步先完成 A02-01~A02-05 的真实 IAM / file-service 能力核验，并据结果确定委托凭证签发方案和文件上传协议；在此之前不创建生产迁移、不迁移 SeeCode、不开放生产流量。

## 1. 证据基准与可信等级

| 标记 | 含义 |
|---|---|
| VERIFIED-CODE | 在指定仓库 commit 的源代码/迁移中直接观察到 |
| VERIFIED-DOC | 在版本化设计/审计文档中观察到；不等同于运行态 |
| HISTORICAL-SNAPSHOT | 有日期的历史环境盘点快照；环境归属/当前性须再次确认 |
| NOT-VERIFIED | 本轮未取得足够证据 |
| BLOCKED | 缺少上游源码、运行环境或运维权限，不能继续推断 |

所有源代码引用均指向本报告顶部所列的 master commit；分支 ART-01 文档变更不在该审计基准中。

## 2. IAM 与项目授权核验

### 2.1 当前 pm-api 身份验证链

**VERIFIED-CODE：** `apps/pm-api/src/auth.rs`：

- token 来源在模块说明中标记为 lmms token-service RS256 JWT。
- pm-api 使用 `digua_common_jwt::JWKSCache` 和 `verify_token` 本地验签。
- 当前调用为 `verify_token(token, jwks, None)`；调用点没有传入预期 audience。
- 验签后要求 `tid`、`sub`，解析 `roles`、`permissions`、`aud`、`client_id`、`purpose`、`ct`。
- 服务身份与跨租户 token 被业务中间件拒绝；`AuthedPrincipal` 包含内部 tenant UUID、principal ID、auth mode 等。
- 身份解析与项目授权不是同一件事。

**结论：** 现有 JWKS 验签可作为受信签名体系的基础，但 ART-01 的 Artifact 专用 audience 必须显式落实。不能仅因当前 token 可通过 pm-api 验签，就认为它可安全地用于 Artifact Service。

### 2.2 当前项目可见性授权

**VERIFIED-CODE：** `apps/pm-api/src/support.rs` 的 `require_project_visible(pool, project_id, tenant_id, principal_id)` 查询 AI-PM 本地授权数据；不可见项目返回 404，避免存在性泄露。Visualization handler 使用 `AuthedPrincipal` 和该守卫。

**结论：**
- 项目成员授权事实仍由 AI-PM 业务侧掌握。
- Artifact Service 不可复制 `project_visible` SQL 或直接访问 AI-PM 数据库。
- 必须由 AI-PM 在操作发生前完成授权，再向 Artifact Service 提供受限、短时、可审计的委托证明。

### 2.3 token-service 能力状态

**VERIFIED-CODE（消费者侧）：** `auth.rs` 已解析 `purpose`，代码注释列出 token-service v1.7.0 起的用途值：`user_login`、`api_key`、`auth_code`、`service`、`resource`。现有服务 token 被业务面拒绝。

**NOT-VERIFIED：**
- 本轮未取得 lmms 1.8.1 token-service issuer 源码和运行态配置。
- 未证明 `resource` token 能绑定 Artifact Service audience、project ID、action allowlist、resource ID 和短 TTL。
- 未证明 token-service 有按 delegation_id / jti 撤销能力或相应密钥策略。
- 未核验服务主体申请 service JWT 的 allowlist 是否允许未来独立的 artifact-service caller。

**实施阻断：** ART-01 的委托凭证不能直接套用现有普通用户 JWT。ART-02 必须在以下两种实现中选定并实测一种：
1. token-service 增加/启用 resource-scoped token 能力，由 AI-PM 授权边界请求签发，audience 固定为 Artifact Service；
2. AI-PM 专用授权 Broker 签发 Artifact Service 可独立验证的短时能力凭证，密钥轮换、audience、scope、撤销和审计需有正式运维契约。

不能由 Artifact Service 自行信任请求体中的 project/action，也不能让 pm-api 长期共享全局签名秘密而不做密钥轮换与审计设计。

## 3. file-service / RustFS 核验

### 3.1 已存在的集成方式

**VERIFIED-CODE：** `apps/pm-api/src/files.rs`：

- 使用 `digua_file_sdk::FileClient`。
- 从环境读取 `SERVICE_BOOTSTRAP_TOKEN`、`AI_PM_TOKEN_SERVICE_URL`、`FILE_SERVICE_URL`。
- 使用 `ServiceJwtCache::get_for_tenant("file-service", tid, false)` 获取 per-tenant 服务 JWT。
- 每请求按调用者租户绑定 token；客户端提交的 tenant_id 不作为可信归属来源。
- 已接入 upload、list、info、download、delete 和 presign 路径。
- 代理上传上限配置为 100 MiB，multipart 文件在 pm-api handler 中读取到内存后再调用 SDK 上传。

**VERIFIED-CODE：** `crates/pm-infrastructure/src/docproc_store.rs`：

- 已有 `ArtifactStore` 抽象与 `FileServiceStore` 实现。
- `put/get/presign` 通过 `digua_file_sdk` 调用 file-service。
- 预签下载 TTL 为 600 秒。
- per-tenant 服务 JWT 通过 `ServiceJwtCache` 获取。
- 缺少 `SERVICE_BOOTSTRAP_TOKEN` 时明确禁用并返回 unavailable，不假成功。

**结论：** file-service 是平台已采用的对象存储控制面，RustFS 由其托管；Artifact Service 应继续通过 SDK/API 复用。

### 3.2 当前调用代码尚未证明的能力

**NOT-VERIFIED：** 本轮 AI-PM 调用代码未证明以下能力已存在：

- 通用 upload intent（一次性、限对象、限方法、限大小）；
- 可信 checksum 返回与算法保证；
- multipart 大文件直传与 finalize/commit；
- 对象复制/原子 finalize 语义；
- 病毒扫描/quarantine 状态查询；
- Artifact 独立 namespace / bucket-prefix 配额；
- 生命周期保留策略与延迟物理删除；
- file-service 对新服务主体 artifact-service 的授权 allowlist。

注意：这不等于上游 file-service 一定没有这些 API，只表示本轮没有从 AI-PM 消费端代码取得证据。ART-02 必须读取 lmms 1.8.1 的 file-service 实际 API/OpenAPI/SDK 并运行契约测试。

### 3.3 上传协议建议修正

在 file-service 未证明支持 upload intent/finalize 之前，ART-01 中“两阶段上传”应作为目标契约，而非已具备能力。

实施顺序：
1. 优先检查 file-service 1.8.1 是否已有直接上传 / 预签上传 / checksum / 对象信息 API。
2. 若已有，Artifact Service 通过 SDK 使用并补 Consumer Contract Tests。
3. 若缺失，优先在 file-service 增加通用能力，再由 Artifact Service 使用。
4. 禁止 Artifact Service 直接连接 RustFS 或复制长期 S3 凭据。
5. V1 如必须先用服务端代理上传，应明确最大文件大小、流式处理、超时、内存和重试策略；不得把当前 pm-api 100 MiB 缓冲实现直接复制成长期方案。

## 4. Visualization 数据与迁移边界

### 4.1 现有数据结构

**VERIFIED-CODE：** `migrations/0020_visualization_artifact.sql`：

- `viz.artifact` 保存 tenant_id、project_id、slot_key、version_no、state、nature、完整 spec JSONB、spec_hash、source_hash、provenance、renderer 元数据。
- 已有已发布版本不可变触发器；published 只能转 stale，stale 为终态。
- `viz.approval` 独立于 Artifact 状态机，保存 target_hash、expected_version_no、requested_by、decided_by、decision_reason。
- `viz.render_output` 当前把 HTML/SVG 文本或 PNG bytea 存在 PostgreSQL；每 Artifact+format 唯一。
- `viz.slot_config` 保存 require_approval、auto_stale、allowed_nature、max_versions、retention_days 等 Visualization 专属策略。
- 迁移文件注释明确：业务对象引用不建跨域 FK，业务语义仍归业务域。

### 4.2 迁移映射确认

| 对象 | 处理 |
|---|---|
| viz.artifact | 映射为通用 Artifact + ArtifactVersion；保留 VizSpec/provenance 业务快照 |
| viz.render_output | 文件内容迁入 file-service；输出元数据/hash 进入 ArtifactOutput |
| viz.approval | 留在 AI-PM Visualization，不迁移 |
| viz.slot_config | 留在 AI-PM Visualization，不迁移 |
| source_hash / stale 判定 | Artifact 保存输入 hash/文件引用；业务来源解析和 stale 判定留在 AI-PM |

### 4.3 历史行数快照

**HISTORICAL-SNAPSHOT：** `docs/audit/four-domain-2026-11-15/raw/inv_rowcounts.tsv` 中 `viz.artifact`、`viz.approval`、`viz.render_output`、`viz.slot_config` 均为 0。

**限制：** 文件路径标注日期为 2026-11-15，但该文件未在本轮证据中标明对应生产/测试/开发环境，也不是当前连接数据库的实时查询。它只能说明该历史盘点样本中这些表为 0 行，不能证明当前生产数据为空。

**迁移前置：** 必须重新连接实际目标环境，获取 row count、大小、外键、索引、触发器、所有写入 API 和 UI 依赖，并进行备份/恢复演练。

## 5. SeeCode 与 Mermaid Generator 核验

### 5.1 SeeCode 已有安全资产

**VERIFIED-CODE：**
- `crates/pm-infrastructure/src/viz_render_sandbox.rs`：子进程输入通过 stdin/受限临时文件；`env_clear()` + 白名单环境变量；执行超时 + kill_on_drop；stdout 输出字节上限；临时目录清理。
- `crates/pm-infrastructure/src/viz_seecode_adapter.rs`：SeeCode 两步 CLI 流程，最终只产出 SVG。
- `artifact_safety::inspect_svg` 在渲染后、入库前静态化检查；失败不落 `viz.render_output`。
- SeeCode M-2 签收文档记录了 vendor、版本、许可证、真实端到端与安全测试结果。

**重要部署责任：**
- 沙箱模块本身不建立网络 namespace 或 egress deny；零外网依赖容器/网络策略。
- CPU/内存硬限额依赖容器/cgroup；模块自身主要限制执行时间和输出量。
- 独立 Artifact Worker 必须在部署层补齐 network deny、CPU/memory/PID、只读根文件系统、临时目录配额等约束。

### 5.2 Mermaid 现状

**VERIFIED-CODE：** 搜索命中 `tools/seecode/scripts/lib/importers/mermaid.mjs`，它是 SeeCode 的 Mermaid importer。

**NOT-VERIFIED / 未发现：** 本轮未发现独立服务端 Mermaid CLI/Renderer Adapter 的实现、Golden Test 或生产运行配置。Importer 只证明可以解析/导入 Mermaid，不等于可把 .mmd 直接渲染为 SVG 的正式 Generator。

**结论：** ART-01 将 Mermaid 定为 V1 正式 Generator 是目标设计；需要独立实现与安全验收，不得把现有 importer 当作已完成。

## 6. CI / Repository 隔离核验

### 6.1 当前 workspace

**VERIFIED-CODE：** 根 `Cargo.toml` 使用 workspace resolver 2，members 为 `crates/*` 和 `apps/*`。

### 6.2 当前 CI 触发与执行范围

**VERIFIED-CODE：** `.github/workflows/pa08-registry-portability.yml`：
- pull_request / push 的 paths 包含 `Cargo.toml`、`Cargo.lock`、`crates/**`、`apps/**` 等。
- Linux job 执行 `cargo check --workspace`、`cargo clippy --workspace --all-targets`。
- nextest archive 使用 `cargo nextest archive --workspace`，后续运行完整 workspace 测试分片。

**VERIFIED-CODE：** `.github/workflows/pa07-blast-radius.yml` 有更窄的路径触发，但仅覆盖 pm-domain / pm-ai 特定回归，不构成 Artifact Service 的独立 workspace、依赖锁文件、镜像和发布。

**结论：** 在当前 Monorepo 中新增 `apps/artifact-service` 仍会进入 workspace 全量构建/测试路径；单靠目录命名或新增一个 workflow 不足以获得真正独立发布边界。ART-00 的独立 Repository 推荐继续成立。

## 7. ART-02 核验矩阵

| 编号 | 项目 | 本轮状态 | 结论 / 下一步 |
|---|---|---|---|
| A02-01 | pm-api JWT / JWKS 验签链 | VERIFIED-CODE | 已知使用 JWKS 本地验签；当前调用未指定预期 audience |
| A02-02 | Artifact 专用 project/action delegation | BLOCKED（上游代码已核验） | lmms v1.8.1 的 resource token 路由无鉴权挂载，且签发 claims 缺 aud/tid；不可复用，需先修复并补安全测试 |
| A02-03 | file-service per-tenant service JWT | VERIFIED-CODE | AI-PM 已使用 ServiceJwtCache；新 artifact-service caller 是否允许仍待核验 |
| A02-04 | upload/download/info/delete/presign | VERIFIED-CODE | AI-PM 已通过 digua_file_sdk 使用；当前调用路径存在 |
| A02-05 | upload intent/checksum/finalize/multipart | PARTIAL（上游代码已核验） | multipart / presign 存在；hash 未计算（None），未发现 intent/finalize/checksum-attestation；P0 协议仍未关闭 |
| A02-06 | viz.* 表结构/状态机 | VERIFIED-CODE | 迁移 0020 可确认；真实当前行数未核验 |
| A02-07 | viz.* 历史数据行数 | HISTORICAL-SNAPSHOT | 2026-11-15 样本 0 行；环境归属未知，不能作生产结论 |
| A02-08 | SeeCode 子进程与 SVG 安全 | VERIFIED-CODE + VERIFIED-DOC | 既有资产可复用；部署层网络/CPU/内存隔离需单独配置 |
| A02-09 | Mermaid 独立 Renderer | NOT-VERIFIED | 只发现 importer；V1 Generator 需实现 |
| A02-10 | Rust CI 独立隔离 | VERIFIED-CODE | 当前 workspace workflow 执行全 workspace；新仓库是长期方案 |
| A02-11 | RustFS 生产 bucket/备份/恢复 | NOT-VERIFIED | 需要运维配置与恢复演练 |
| A02-12 | lmms v1.8.1 上游源码与 API/SDK | VERIFIED-CODE（仅静态源码） | 已读取 Tag v1.8.1 的 token-service、file-service、Rust SDK 与 OpenAPI；运行态 claims、服务主体权限和存储部署仍未验证 |

## 8. 实施前必须关闭的阻断项

### P0 — 授权委托

- 取得 lmms 1.8.1 token-service 的 resource token 签发契约和实际 claims。
- 确定谁负责 project/action authorization（必须是 AI-PM 可信授权边界）。
- 实现 Artifact Service 专用 audience 验证和短时 scope 检查。
- 验收跨租户、跨项目、动作越权、资源越权、过期、撤销、重放负例。

### P0 — file-service 上传与对象一致性

- 取得 file-service 1.8.1 OpenAPI / SDK。
- 验证可信 checksum、对象元数据、大小、MIME、multipart 和 finalize 能力。
- 验证新服务主体获取 per-tenant service JWT 的权限。
- 若缺失 upload intent/finalize，在 file-service 侧先补通用能力。

### P1 — Worker 安全运行环境

- 明确独立 Worker 镜像与部署策略。
- 以网络策略阻断外网和内部非必要网段。
- 以 cgroup/容器限制 CPU、内存、PID、文件系统与执行时间。
- 将 SeeCode 已有沙箱和 SVG 检查抽离为可独立测试的模块。
- Mermaid 从零建立独立 Renderer、Golden、安全和零外网测试。

### P1 — 迁移与运维

- 对真实目标环境执行 viz.* 行数/容量盘点。
- 确认 file-service 对象生命周期与数据库元数据保留期一致。
- 执行数据库 + 对象存储联合恢复演练。
- 完成单写权威切换与回滚脚本设计。

## 9. 下一步执行顺序

1. **ART-02A（优先）**：取得 lmms 1.8.1 token-service / file-service 的 OpenAPI、SDK 或源代码，冻结真实委托 token 与文件 API 契约。
2. **ART-02B**：连接实际开发/测试环境，验证 JWT claims、服务身份签发、file-service 对象操作和租户隔离。
3. **ART-02C**：取得目标生产/内网部署配置，核验 RustFS bucket、备份、生命周期与恢复。
4. **ART-02D**：对当前目标数据库执行 viz.* 实时数据盘点和所有写入方搜索。
5. 完成以上步骤后，才能冻结 ART-02 最终版并开始 ART-03 创建独立 Repository。

## 10. 审计结论

ART-02 第一轮证明：Artifact Service 可以复用现有 file-service 身份与对象存储集成模式，也可以复用 SeeCode 的安全实现基础；但不能直接复用 AI-PM 当前的项目授权 SQL，也不能把现有文件上传接口视为已具备完整版本提交协议。

**当前不批准进入生产实现/迁移。** 首先关闭授权委托与 file-service 上传协议两个 P0。其余待核验项应在 ART-02A~D 中取得真实证据，不得用历史快照、设计文档或代码注释替代。


---

## 11. ART-02A：lmms v1.8.1 上游源码核验（2026-11-24）

> **核验对象：** `luqizheng/lmms`，Git Tag `v1.8.1`。  
> **版本证据：** 根 `Cargo.toml` 的 `workspace.package.version = "1.8.1"`；Tag 下可读取。  
> **范围：** GitHub 源码、OpenAPI 与 Rust SDK 静态核验；没有连接运行中的 token-service/file-service，也没有读取密钥、令牌或业务数据。  
> **审计性质：** VERIFIED-CODE 不等于 VERIFIED-RUNTIME。

### 11.1 Token Service：普通 Service JWT

**VERIFIED-CODE：** `services/token-service/src/routes/service_jwt.rs`

- `POST /tokens/service-jwt` 使用 bootstrap bearer token 查找 active owner。
- 请求字段为 `aud`、`tid`、可选 `ct`；签发 claims 包含 `iss`、`sub`、`aud`、`tid`、`iat`、`exp`、`roles=["service"]`、`purpose="service"`。
- TTL 在该路由中固定为 3600 秒。
- owner 由 bootstrap token 数据库记录决定，不是请求体任意指定。
- token-service 支持 keystore active key/kid 与 JWKS；本轮只确认源码路径，不验证生产轮换状态。

**结论：** 现有 Service JWT 能满足服务到服务的 audience + tenant 基础隔离，但没有 project ID、action allowlist、resource ID、delegation ID 等 Artifact 操作级授权约束；1 小时固定 TTL 也不是 ART-01 预期的短时能力凭证。可作为服务身份调用 file-service 的基础，不可替代项目授权委托。

证据：
- [service_jwt.rs](https://github.com/luqizheng/lmms/blob/v1.8.1/services/token-service/src/routes/service_jwt.rs)
- [token-service main.rs](https://github.com/luqizheng/lmms/blob/v1.8.1/services/token-service/src/main.rs)
- [token-service lib.rs](https://github.com/luqizheng/lmms/blob/v1.8.1/services/token-service/src/lib.rs)

### 11.2 Token Service：Resource Token 当前不可用于 Artifact Service

**VERIFIED-CODE：** `services/token-service/src/main.rs` 将 `POST /tokens/resource` 直接挂载在主 Router；该路由没有附加 `verify_aud_principal_middleware` 或其他身份验证层。

**VERIFIED-CODE：** `services/token-service/src/routes/resource.rs`：

- 请求体包含 `resource_type`、`resource_id`、`permission`、可选 `ttl_sec`。
- handler 没有提取或验证调用者身份，也没有验证调用者是否拥有该资源权限。
- handler 调用 `issue_resource_token("resource-service", ...)`。

**VERIFIED-CODE：** `services/token-service/src/issuer.rs` 的 `issue_resource_token`：

- 写入 `scope="resource"`、`resource_type`、`resource_id`、`permission`、`purpose="resource"`，以及标准 subject/jti/issuer/expiry。
- 没有写入 `aud` 和 `tid`。
- 因此该 Token 既缺少 Artifact Service 需要的 audience/tenant 绑定，也没有证明资源授权来自可信 AI-PM 授权边界。

**P0 安全发现：** 当前 resource token endpoint 的挂载方式存在未鉴权签发风险。即使服务网络在部署层限制访问，也不能将其作为安全边界。必须先修复路由鉴权与签发授权，再考虑扩展 claims；修复前禁止 Artifact Service 使用该 endpoint。

**建议修复验收：**
1. 签发接口必须要求可信调用者身份（至少固定 audience 的 service JWT），并验证调用者具有签发权限。
2. 资源权限必须由可信授权边界确认；不能仅根据请求体中的 resource_type/resource_id/permission 直接签发。
3. 新凭证必须绑定 `aud=artifact-service`、tenant、project、action allowlist、resource（如适用）、短 TTL、jti/delegation ID。
4. 明确撤销/过期策略和签发审计。
5. 增加未认证、错误 audience、跨 tenant、跨 project、越权 action、越权 resource、过期、重放等负例测试。
6. 不应让 Artifact Service 直接调用当前未鉴权的 resource endpoint。

证据：
- [resource.rs](https://github.com/luqizheng/lmms/blob/v1.8.1/services/token-service/src/routes/resource.rs)
- [token-service main.rs](https://github.com/luqizheng/lmms/blob/v1.8.1/services/token-service/src/main.rs)
- [issuer.rs](https://github.com/luqizheng/lmms/blob/v1.8.1/services/token-service/src/issuer.rs)

### 11.3 File Service：已存在的能力

**VERIFIED-CODE：**

- `services/file-service/src/routes/file.rs`：`POST /file/upload` multipart 上传；`GET /file/list`；`GET /file/{id}` 下载；`DELETE /file/{id}`；`GET /file/{id}/info`。
- `services/file-service/src/routes/presign.rs`：`POST /file/presign`。
- `services/file-service/src/routes/mod.rs`：`/file/*` 路由要求 `aud=file-service` 的有效 JWT。
- `services/file-service/src/services/file.rs`：租户范围来自已验证的 `TenantScope`；上传时 tenant_id 从 token scope 派生，而非信任 multipart metadata。
- `services/file-service/src/storage/object_store.rs` 与 `storage/s3.rs`：抽象支持 put/get/get_stream/delete/exists/size 和 S3 预签名；S3-compatible driver 可连接 RustFS。
- `sdk/rust/file-service-sdk`：提供 multipart upload、presign、list、info、download 等客户端能力。

证据：
- [file routes](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/routes/file.rs)
- [presign routes](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/routes/presign.rs)
- [route auth](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/routes/mod.rs)
- [file service implementation](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/services/file.rs)
- [object store abstraction](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/storage/object_store.rs)
- [Rust SDK](https://github.com/luqizheng/lmms/tree/v1.8.1/sdk/rust/file-service-sdk)

### 11.4 File Service：Artifact Version 提交协议缺口

**VERIFIED-CODE：**

- `FileService::upload` 将文件写入对象存储，再写入元数据；`FileInfo.hash` 明确赋值为 `None`，未观察到服务端 checksum 计算。
- 当前 multipart handler 通过 `field.bytes()` 将完整文件读入内存；不是流式上传路径。
- `create_presign_url` 先通过 `file_id` 查询既有元数据，再为该记录的 storage_path 生成 GET/PUT URL；它不是先创建受限 upload intent 再 finalize 的两阶段提交。
- 未观察到 finalize/commit endpoint、上传后 checksum attestation、对象状态校验或不可变对象提交机制。
- SDK 的 `UploadFileMeta` 不包含 tenant_id，且注释明确租户归属来自 service JWT 的 tid；这是正确的信任边界方向。

**结论：** File Service 可继续作为通用对象存储和文件元数据服务，但当前公开接口不足以直接承担 ART-01 所要求的不可变 Artifact Version 原子提交协议。P0 上传协议仍然开放。

**建议：** 优先在 file-service 增加通用、非 Artifact 专属的 upload-intent / bounded presign / checksum / finalize 能力；Artifact Service 只消费该通用契约。若短期沿用 multipart 服务端上传，必须先定义大小上限、流式处理、内存/超时/重试和 hash 计算，不能将当前全量缓冲路径视为长期方案。

证据：
- [file service implementation](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/services/file.rs)
- [file routes](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/routes/file.rs)
- [file models](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/src/models/file_info.rs)
- [file-service OpenAPI](https://github.com/luqizheng/lmms/blob/v1.8.1/services/file-service/openapi.yaml)
- [Rust SDK client](https://github.com/luqizheng/lmms/blob/v1.8.1/sdk/rust/file-service-sdk/src/client.rs)

### 11.5 上游身份与租户验证机制

**VERIFIED-CODE：**

- `libs/authz/src/verify_aud.rs` 通过 `Principal::from_token(..., Some(expected_aud))` 强制 audience 匹配。
- File Service 对业务路由采用 `verify_aud_principal_middleware("file-service")`。
- `libs/jwt-ctx/src/scope.rs` 从已验证 Principal 派生 Single tenant 或 CrossTenant scope。
- 因此 Artifact Service 应采用相同的明确 audience 验证模式，但还需自行校验 Artifact 专用 project/action claims。

证据：
- [audience middleware](https://github.com/luqizheng/lmms/blob/v1.8.1/libs/authz/src/verify_aud.rs)
- [JWT principal](https://github.com/luqizheng/lmms/blob/v1.8.1/libs/jwt-ctx/src/principal.rs)
- [tenant scope](https://github.com/luqizheng/lmms/blob/v1.8.1/libs/jwt-ctx/src/scope.rs)

### 11.6 ART-02A 结论与更新后的阻断项

| 项目 | 状态 | 结论 |
|---|---|---|
| lmms v1.8.1 Tag / 版本基准 | VERIFIED-CODE | Tag 可读取，workspace version 为 1.8.1 |
| Service JWT | VERIFIED-CODE | aud + tid + ct；固定 3600 秒；没有 project/action delegation |
| Resource Token | VERIFIED-CODE / P0 SECURITY BLOCKER | endpoint 无鉴权挂载；claims 无 aud/tid；不可复用 |
| File Service tenant isolation | VERIFIED-CODE | file routes 强制 aud=file-service；TenantScope 从已验证 token 派生 |
| Multipart upload | VERIFIED-CODE | 存在，但全量缓冲；hash=None |
| Presigned URL | VERIFIED-CODE | 已有 GET/PUT presign；PUT 依赖既有 file record |
| Upload intent / finalize / checksum | NOT FOUND IN REVIEWED V1.8.1 PATHS | 未发现对应 API/SDK/handler；不能据此宣称全仓绝对不存在 |
| 运行态 JWT claims / bootstrap allowlist | NOT VERIFIED | 需连接实际部署环境做无秘密暴露的契约验证 |
| RustFS bucket / retention / backup / restore | NOT VERIFIED | 需部署配置与恢复演练 |

**ART-02A 决定：** 保留“复用 token-service 信任根、复用 file-service/RustFS”的总体方向，但不复用当前 resource token endpoint。两个 P0 继续阻断 Artifact Service 生产实现：  
1. 修复并验证受限 Artifact delegation 签发与验证；  
2. 建立可验证 checksum 和 finalize 的通用文件提交协议。

下一步 ART-02B 应在真实测试环境验证 Service JWT 的签发身份、`aud=file-service`、tenant scope、bootstrap owner allowlist，以及 File Service 上传/预签名/下载/删除的跨租户负例。ART-02C/02D 仍分别核验 RustFS 运维与目标数据库实时数据。


---

## 12. ART-02B：File Service 契约与测试证据复核（2026-11-24）

> **核验对象：** `luqizheng/lmms`，Tag `v1.8.1`，peeled commit `7c382118e267ad7b9f7d3457c8648e78e662da41`。  
> **范围：** File Service routes/service/storage、OpenAPI、Rust SDK、迁移与测试源码静态复核。  
> **重要限制：** 本阶段没有连接用户正在运行的 token-service、file-service、RustFS 或数据库；没有使用生产凭据，也没有执行真实上传/下载/删除。因此本节的 VERIFIED 均指 VERIFIED-CODE / VERIFIED-TEST-SOURCE，不代表 VERIFIED-RUNTIME。

### 12.1 API 与 SDK 能力矩阵

| 能力 | v1.8.1 代码证据 | 结论 |
|---|---|---|
| 服务端 multipart 上传 | `routes/file.rs::upload_file` → `FileService::upload` | VERIFIED-CODE |
| 文件元数据查询 | `GET /file/{id}/info` | VERIFIED-CODE |
| 文件下载 | `GET /file/{id}` | VERIFIED-CODE |
| 文件删除 | `DELETE /file/{id}` | VERIFIED-CODE |
| 列表查询 | `GET /file/list` | VERIFIED-CODE；查询过滤参数的实现仍需独立核验 |
| GET/PUT 预签名 | `POST /file/presign` → `ObjectStore::presign_get/presign_put` | VERIFIED-CODE |
| Rust SDK multipart / presign / list / info / download | `sdk/rust/file-service-sdk/src/client.rs` | VERIFIED-CODE |
| Rust SDK delete 方法 | 当前检查的 `FileClient` 未发现 | NOT FOUND IN REVIEWED SDK |
| Upload Intent / Finalize / Commit | OpenAPI、routes、service 与 SDK 检查范围内未发现 | NOT FOUND IN REVIEWED V1.8.1 PATHS |
| 服务端 SHA-256 / checksum attestation | 上传流程未实现；`FileInfo.hash` 被设为 `None` | VERIFIED-CODE：缺失 |
| Multipart 流式写入 | handler 使用 `field.bytes()`，service 接收完整 `Bytes` | VERIFIED-CODE：当前路径全量缓冲 |
| S3/RustFS 流式读取 | ObjectStore 存在 `get_stream` | VERIFIED-CODE；不等于上传支持流式 |

### 12.2 上传、对象提交与一致性

**VERIFIED-CODE：**

1. `upload_file` 读取 multipart 的 metadata 与 file 字段；文件字段通过 `field.bytes()` 读成完整内存缓冲。
2. `FileService::upload` 先调用 `ObjectStore::put(storage_path, data)`，再调用 `save_file_info` 插入 `file_files`。
3. `storage_path` 使用服务端 UUID，不再拼接客户端 filename；这是正确的路径隔离方向。
4. `FileInfo.hash` 当前明确写入 `None`，没有服务端 checksum 计算或上传后对象校验。
5. 对象写入与元数据 INSERT 不在同一事务，也未看到失败补偿删除对象的逻辑。对象写成功、数据库写失败时可能留下孤儿对象。
6. 删除流程先删除对象，再删除元数据；若元数据删除失败，会留下指向已删除对象的记录。
7. 当前 presign handler 先按 `file_id` 查询既有 FileInfo，再对其 `storage_path` 生成 URL。PUT 预签名不是创建上传意图的接口；没有独立 pending 状态、一次性 upload token、完成回调或 finalize 校验。
8. PUT 预签名对既有 storage_path 授权写入；在当前实现下，它不证明上传内容与元数据 size/hash 一致，也不具备 Artifact Version 的不可变提交语义。

**ART-01 影响：** 不能直接把当前 PUT presign 当作 Artifact Version 上传协议。若未来复用 PUT，必须新增通用的 upload intent、短 TTL/单对象/单操作约束、内容长度与 checksum 约束、finalize 时服务端对象 HEAD/校验、状态转换与失败清理；Artifact Service 不应自行绕过 file-service 写 RustFS。

### 12.3 预签名与 API 契约风险

**VERIFIED-CODE：**

- `CreatePresignUrl.operation` 在 OpenAPI 中是普通 string；service 仅对精确值 `put` 走 PUT，其余所有值都走 GET，没有严格枚举校验。
- `expires_in_seconds` 默认 3600 秒；当前路径将负值传给存储层时以 `max(0)` 转换，但没有发现明确的业务 TTL 上限策略。
- 生成的 presigned URL 会被写入 `file_presign_urls.url`；应评估数据库日志、审计与备份对短时签名 URL 的暴露面，并考虑仅存摘要/元数据而非完整 bearer-like URL。
- OpenAPI 文件 `info.version` 标记为 `1.8.0`，但本次核验的 Git Tag 是 `v1.8.1`。需要确认该 OpenAPI 是生成产物未更新，还是服务契约版本独立管理；不要仅凭文件内版本号判断实际服务版本。

**建议：** 对 operation 使用严格 enum（get/put），拒绝未知值；对 TTL 设置服务端最大值；对 PUT presign 限定专用 intent 记录与一次性状态；避免长期保存完整签名 URL。

### 12.4 租户身份与隔离测试

**VERIFIED-CODE：**

- `routes/mod.rs` 对 `/file/*` 使用 `verify_aud_principal_middleware("file-service")`，要求固定 audience 的 service JWT。
- `upload_file` 从 `TenantScope` 传入 service；`FileService::upload` 的 tenant_id 从 scope 派生，而不是信任 multipart 中的 tenant_id。
- `get_file_info`、download、delete 与 presign 均通过 `get_file_info(id, scope)` 路径取得文件记录；查询构造器使用 `and_tenant(scope)`。
- `sdk/rust/file-service-sdk/src/model.rs` 的 `UploadFileMeta` 不包含 tenant_id，并明确注释租户来自 service JWT 的 `tid` claim。这是正确的信任边界。
- `tenant_isolation_test.rs` 构造 `aud=file-service` 的 service JWT，验证租户 A 不能读取租户 B 的 file info、跨租户主体可读取两边，并验证 user JWT（aud=coder-micro-services）被拒绝。

**测试覆盖边界：** 当前专门的 tenant isolation test 主要验证 file info 读取和 user-JWT audience 拒绝；没有在该测试中逐项验证跨租户 upload/list/download/delete/presign。不能把单个 info 读取负例扩大解释为所有端点均已通过运行态测试。

### 12.5 生命周期、过期与存储配置

**VERIFIED-CODE：**

- `S3Storage` 通过统一 S3-compatible driver 支持 RustFS path-style；启动配置从环境变量读取 endpoint、bucket、region 与凭据。
- bucket 自动创建是异步 best-effort；失败只记录 warning，不阻止服务启动。部署必须显式检查 bucket 已存在且权限正确，不能把服务启动成功当作 bucket 可用证明。
- 文件下载会检查 `expires_at` 并将过期文件表现为 not found。
- 本次检查的 file-service `main.rs` 未发现过期文件后台清理 worker；README 对“临时文件自动清理”的描述需要在运行环境/其他调度组件中确认，不能仅凭 README 认定服务本身会清理对象。
- 配置源码默认 backend 为 local；生产必须明确设置 `FILE_STORAGE_BACKEND=rustfs`、endpoint、bucket、path-style 和凭据来源。没有读取任何实际凭据值。

### 12.6 ART-02B 结论

| 检查项 | 状态 | 结论 |
|---|---|---|
| File Service API 基础能力 | VERIFIED-CODE | 上传、下载、删除、info、list、presign 已存在 |
| service JWT audience gate | VERIFIED-CODE | 路由要求 `aud=file-service` |
| TenantScope 代码隔离 | VERIFIED-CODE | 核心读写通过 scope 派生租户条件 |
| 租户隔离测试 | VERIFIED-TEST-SOURCE / PARTIAL | 已验证 info 读隔离与 user JWT 拒绝；端点覆盖不完整 |
| SHA-256 / checksum attestation | VERIFIED-CODE：缺失 | 当前 hash=NULL |
| Upload Intent / Finalize | NOT FOUND IN REVIEWED PATHS | 不能作为 Artifact 上传提交协议 |
| 上传流式处理 | VERIFIED-CODE：缺失 | multipart 当前全量缓冲 |
| 对象与元数据原子提交/补偿 | VERIFIED-CODE：缺失 | 存在孤儿对象/元数据不一致窗口 |
| Presign operation/TTL 限制 | VERIFIED-CODE：不足 | operation 非枚举；未发现明确 TTL 上限 |
| RustFS 运行态、bucket 权限与恢复 | NOT VERIFIED | 需部署环境验证 |
| 真实跨租户 API 负例 | NOT VERIFIED | 本轮没有连接实际部署环境 |

**ART-02B 决定：**

1. 保留 file-service 作为 Artifact Service 的唯一对象存储控制面；Artifact Service 不直接持有 RustFS 长期凭据，也不绕过 file-service。
2. 当前 file-service 可用于受控的普通文件存取，但**不得直接作为 Artifact Version 的最终提交协议**。
3. 继续将“通用 upload-intent + checksum + finalize + 对象状态验证 + 清理补偿”列为 P0 存储契约阻断项。该能力应在 file-service 通用层实现，不在 Artifact Service 内重复实现。
4. 先完成运行态验证，再冻结 ART-01 的 file-service adapter contract；不能用源码单测代替真实 token-service/file-service/RustFS 联调。

### 12.7 运行态验证清单（需要部署环境）

以下项目必须由连接到目标测试环境的执行者完成；只需提供脱敏结果，不要提交 token、Authorization header、签名 URL、密钥或用户文件内容：

- [ ] 使用受控 bootstrap owner 签发 service JWT；确认调用者身份、`aud=file-service`、`tid`、`ct` 与有效期。
- [ ] 用 tenant-A token 上传测试文件；确认返回 tenant_id 来自 token，而不是请求 metadata。
- [ ] tenant-B token 对 tenant-A 文件执行 info、download、presign、delete，全部应返回统一的 not-found/deny 结果；并验证 tenant-A 文件仍完整可读。
- [ ] 普通 user JWT（错误 audience）访问所有 file routes，应在路由层被拒绝。
- [ ] 验证 GET/PUT presign 的实际 TTL、operation 行为、签名 URL 过期后不可用；不要把 URL 本身写入报告。
- [ ] 模拟对象写成功但 metadata insert 失败、对象删除成功但 metadata 删除失败，确认补偿/告警/修复策略。
- [ ] 验证 RustFS bucket、service access policy、持久卷、备份和恢复；至少完成一次独立恢复演练。
- [ ] 记录部署镜像 digest、服务版本、测试时间、测试用例 ID 与脱敏结果。

**当前状态：ART-02B 源码/测试证据复核已完成；真实运行态验证仍 OPEN。** 在拿到上述脱敏测试证据前，ART-02B 不得标记为 runtime PASS，ART-03 也不得据此解除 P0。


---

## 13. ART-02C：RustFS 部署、持久化与备份恢复核验（2026-11-24）

> **核验对象：** `luqizheng/lmms`，Tag `v1.8.1`，peeled commit `7c382118e267ad7b9f7d3457c8648e78e662da41`。  
> **范围：** 标准 Docker Compose、prod/remote override、file-service 配置、ObjectStore 实现、备份 Runbook 与部署文件树。  
> **核验限制：** 只读静态仓库核验。未连接用户实际部署主机、Docker daemon、RustFS endpoint、bucket、PostgreSQL 或备份介质；没有读取任何密钥或环境变量实际值。不能将仓库配置推断为用户当前运行态。

### 13.1 关键发现：标准 Compose 默认不是 RustFS

**VERIFIED-CODE：**

- `services/file-service/src/config.rs::default_storage_backend`：`FILE_STORAGE_BACKEND` 未设置时默认 `local`。
- `examples/docker-deploy-full/docker-compose-services.yml` 中 File Service 设置 `STORAGE_PATH=/var/lib/file-service/storage`，挂载 named volume `file-storage:/var/lib/file-service/storage`。
- 同一 File Service service block 未设置 `FILE_STORAGE_BACKEND`、`S3_ENDPOINT`、`S3_BUCKET`、`S3_ACCESS_KEY`、`S3_SECRET_KEY`。
- `examples/docker-deploy-full/docker-compose-services.prod.yml` 对 File Service 只调整 host ports 与 restart policy，没有切换 storage backend。
- `examples/docker-deploy-full/docker-compose-services.remote.yml` 对 File Service 只增加 restart policy，没有清空 named volume，也没有注入 RustFS 配置。
- Compose 文件底部将 volume 命名为 `superpowers_file-storage`。

**结论：** 按 v1.8.1 仓库提供的标准部署 Compose，File Service 默认使用 LocalStorage，数据落在 Docker named volume `superpowers_file-storage`。README 虽说明 RustFS 是私有化部署推荐后端，但“代码支持 RustFS”与“标准 Compose 已启用 RustFS”是两件不同的事。

**边界：** 用户可以在仓库外部通过环境注入或自行维护的 Compose override 配置 RustFS；本次仓库静态审计不能证明用户当前环境一定运行 LocalStorage。需要实际部署配置确认。

### 13.2 RustFS 配置与对象存储实现

**VERIFIED-CODE：**

- `StorageBackend::parse` 接受 `local`、`rustfs`、`s3` 等值。
- RustFS backend 要求 `S3_ENDPOINT`、`S3_ACCESS_KEY`、`S3_SECRET_KEY`；bucket/region/path-style 等从配置读取。
- RustFS 默认使用 path-style；S3-compatible driver 使用统一实现，不为 RustFS 建独立业务 API。
- `S3Storage::put/get/get_stream/delete/exists/size/presign_get/presign_put` 已实现。
- `S3_AUTO_CREATE_BUCKET` 默认 true；启动时异步 best-effort 检查/创建 bucket，失败只记录 warning，不阻断服务启动。
- 创建 bucket 需要 `CreateBucket` 权限；如果采用最小权限生产策略，建议由运维预建 bucket 并关闭自动创建，而不是给 file-service 运行身份永久授予建桶权限。
- RustFS 相关配置由 File Service 读取；Artifact Service 不应持有这些长期 S3 凭据，也不应直接访问 bucket。

**部署安全建议：**

- File Service 的 RustFS 凭据应仅具备指定 bucket/prefix 的必要读写/删除权限，不应具备账户级管理权限。
- Artifact Service 只获得调用 file-service 的 service identity；不要向它注入 `S3_ACCESS_KEY` / `S3_SECRET_KEY`。
- 生产 bucket 应预建、禁止公共访问、限制网络入口，并将服务端点与管理控制台分离。
- 需确认 presigned URL 的外部可达域名、TLS、反向代理、DNS 和生命周期；内网 endpoint 生成的签名 URL 不能默认认为浏览器/外部客户端可访问。
- bucket versioning、object lock、生命周期规则、服务端加密、跨故障域复制等能力，本次没有在 lmms 仓库中找到可证明的 RustFS 运行配置，状态为 NOT VERIFIED。

### 13.3 标准部署的文件持久卷

**VERIFIED-CODE：**

- 标准 Compose 使用 named volume `superpowers_file-storage` 保存 LocalStorage 文件。
- prod override 未替换该卷；remote override 也保留 base volume。
- 本次检查的 Compose 片段未为该卷配置宿主机备份、快照、异地复制或自动清理策略。
- Docker named volume 解决容器重建后的本机持久化，不等于备份，也不等于跨主机灾备。

**结论：** 如果目标环境沿用标准 Compose 且没有外部 override，则 File Service 文件内容依赖 `superpowers_file-storage`。只备份 PostgreSQL 元数据而不备份这个卷，会产生 metadata 存在但对象内容不可恢复的风险；只备份文件卷而不备份 file-service DB，也会丢失文件 ID、tenant、size、expiry 等元数据关联。

### 13.4 备份 Runbook 与部署拓扑不一致

**VERIFIED-DOC：**

- `docs-site/runbooks/backup.md` 明确称为“4 项基础设施组件”备份 SOP，内容覆盖 MySQL、Redis、RabbitMQ、Consul。
- Runbook 自述“没有自动备份”，当前是手动 SOP。
- Runbook 没有 File Service `superpowers_file-storage` named volume 的备份/恢复步骤，也没有 RustFS bucket/object 数据的备份/恢复步骤。
- 本次检查的部署文件使用 `infrastructure/infra-postgres`，其 Compose 配置 PostgreSQL named volume `postgres_data`；上述 Runbook 的数据库章节却写 MySQL。不能把该 MySQL Runbook 当作当前 PostgreSQL 部署已具备的数据库备份证明。
- `docs/superpowers/specs/2026-07-29-p0.4-backup-runbook-design.md` 将备份范围限定为手动 SOP，并明确把自动化、异地备份/对象存储上传排除在外。

**结论：** 当前仓库中可见的备份文档不足以证明 File Service 数据或 RustFS 对象有可恢复备份；而且数据库类型与当前 PostgreSQL 部署文件存在文档漂移。需要单独补齐与目标部署拓扑一致的 PostgreSQL + 对象存储联合备份/恢复方案。

### 13.5 生命周期与清理

**VERIFIED-CODE：**

- File Service 元数据模型包含 `is_temporary` 与 `expires_at`；下载路径会检查过期时间。
- 本次审阅的 `services/file-service/src/main.rs` 启动流程没有发现过期对象清理 worker。
- File Service README 提到临时文件自动清理，但在本次审阅的启动路径中没有找到可证明该功能实际运行的调度任务。
- S3-compatible ObjectStore 提供 `delete`，但对象删除操作能力不等于生命周期策略已经配置。
- 本次没有找到可确认的 RustFS bucket lifecycle policy、retention policy 或跨服务协调删除实现。

**结论：** 过期访问限制与物理对象清理是两种不同语义。Artifact Service 需要不可变版本保留策略；不能把 `expires_at` 检查或 README 描述视为已经实现可靠的对象生命周期管理。

### 13.6 联合备份与恢复必须满足的要求

对 Artifact Service 而言，数据库元数据与对象字节是一个逻辑交付单元。目标运行环境至少应具备：

1. **一致性时间点：** 备份期间暂停/协调写入，或采用可证明的一致性快照机制，确保数据库记录与对象集能互相对应。
2. **数据库备份：** 对 file-service PostgreSQL/MySQL 实际使用的数据库执行加密、校验和、保留策略明确的备份；备份恢复后需验证 schema、文件记录、tenant scope 与索引。
3. **对象备份：** 对 LocalStorage named volume 或 RustFS bucket/prefix 做独立备份/复制；仅备份 bucket 配置不等于备份对象字节。
4. **版本/删除语义：** 明确对象版本控制、误删保护、保留期限、软删除窗口、生命周期与最终物理删除的责任边界。
5. **恢复顺序：** 恢复数据库与对象存储到同一一致性点；随后抽样逐条校验 FileInfo 对应对象存在、size 一致、checksum 一致、可下载。
6. **恢复演练：** 至少在隔离环境完成一次完整恢复，记录 RPO/RTO、对象数量、字节数、抽样 hash 与失败处理。
7. **异地保护：** 生产备份不能只留在同一 Docker host 或同一个 RustFS 故障域；需要独立凭据、独立故障域和定期恢复验证。

### 13.7 ART-02C 核验矩阵

| 检查项 | 状态 | 结论 |
|---|---|---|
| File Service 支持 RustFS/S3 | VERIFIED-CODE | RustFS backend 与 S3-compatible driver 已实现 |
| 标准 Compose 启用 RustFS | VERIFIED-CODE：否 | 默认 backend=local，Compose 配置 named volume |
| 标准 Compose 文件持久化 | VERIFIED-CODE | `superpowers_file-storage` named volume |
| prod/remote override 切换 RustFS | VERIFIED-CODE：未发现 | override 未配置 backend/endpoint/credentials |
| RustFS bucket 权限与配置 | NOT VERIFIED | 需目标环境核对；不读取或输出密钥 |
| bucket versioning / lifecycle / object lock | NOT VERIFIED | 仓库没有可确认的实际 bucket policy |
| file-storage volume 备份 | NOT FOUND IN REVIEWED RUNBOOK | 没有对应备份/恢复步骤 |
| RustFS 对象备份 | NOT FOUND IN REVIEWED RUNBOOK | 没有对象数据备份/恢复证据 |
| PostgreSQL 备份流程 | PARTIAL / DOCUMENTATION DRIFT | 当前部署文件使用 PostgreSQL；被审阅 Runbook 数据库章节为 MySQL |
| 自动备份 | VERIFIED-DOC：无 | Runbook 明确是手动 SOP |
| 对象生命周期清理 worker | NOT FOUND IN REVIEWED STARTUP PATH | README 有描述，启动路径未找到 worker |
| 联合恢复演练 / RPO / RTO | NOT VERIFIED | 必须在实际目标环境执行 |

### 13.8 ART-02C 决定

1. **保留 file-service 作为唯一对象存储控制面**，继续复用 RustFS/S3-compatible driver；不在 Artifact Service 中增加直连对象存储的代码路径。
2. **把“实际部署启用 RustFS”从默认假设改为部署前显式核验项。** 标准 v1.8.1 Compose 默认是 LocalStorage + named volume；如果生产要用 RustFS，必须在受版本控制的部署 override/环境契约中明确设置 backend、endpoint、bucket 和凭据引用。
3. 如果继续使用 LocalStorage，必须将 `superpowers_file-storage` 卷纳入备份、异地复制和恢复演练；如果使用 RustFS，则必须把 bucket/object 数据纳入独立备份和恢复演练。
4. PostgreSQL 元数据与对象数据必须形成联合备份/恢复策略；禁止只备份数据库或只备份对象。
5. 修正 Runbook 与 PostgreSQL 部署拓扑的漂移，并补充 file-service 对象数据备份章节；不得把手动 SOP 误称为自动备份保障。
6. 在目标环境确认 backend、endpoint、bucket policy、对象数量/容量、保留策略、备份位置和恢复证据之前，ART-02C 运行态仍为 OPEN。

**当前状态：ART-02C 仓库配置/文档静态核验已完成；用户实际环境 backend、RustFS 配置、备份与恢复状态仍 NOT VERIFIED。**


## 14. ART-02D — 现有 viz.* 数据、读写路径与迁移依赖核验

### 14.1 核验范围与证据基线

本节只核验 AI-PM 仓库中的迁移、Rust 应用层、pm-api handler、测试和历史数据库盘点；不连接当前数据库，不读取真实业务行、密钥或环境变量值。

| 项 | 基准 / 结果 |
|---|---|
| 当前代码基线 | `master` commit `3a50c7cdee3b2ef0d7e19b241b64764a52515fb8` |
| 本文档分支 | `docs/artifact-service-baseline-v1`，写入前 HEAD `bbd8a0db81800567ac6f496da310f045ddc317f9` |
| 历史数据库快照 | `full-dev`：Docker 容器 `superpowers-full-postgres-1`，database `postgres`，user `postgres` |
| 历史快照对应代码 | commit `05ce33d8dfbd70a672b27c1a6ce20a1ee5d405c6`（2026-11-15 19:18 +0800） |
| 历史盘点时间 | 2026-11-15 19:41 起；当时记录 migrations 1~24 已执行 |
| 当前生产/目标环境实时行数 | **NOT VERIFIED** |

历史快照来源：`docs/audit/four-domain-2026-11-15/00_evidence_baseline.md` 与 `raw/inv_rowcounts.tsv`。该证据明确指向 full-dev，而非生产数据库；其数据不能外推为当前环境状态。

### 14.2 Schema、约束与关系

四张表均由 `migrations/0020_visualization_artifact.sql` 创建：

| 表 | 当前职责与关键结构 | 主要约束 |
|---|---|---|
| `viz.artifact` | 图形 Spec、来源指纹、版本号、状态、生成者与 renderer 元数据 | 状态 CHECK；`(project_id, slot_key, version_no)` 唯一；published Spec 不可改写，published 只能转 stale，stale 为终态 |
| `viz.approval` | 独立发布审批记录、目标 hash、预期版本、裁决人/原因 | pending/approved/rejected/cancelled 状态；每项目槽位最多一条 pending；artifact_id FK |
| `viz.render_output` | 按 artifact + format 保存 HTML/SVG 文本或 PNG bytes、hash、大小及 renderer 信息 | format/content 组合 CHECK；`(artifact_id, format)` 唯一；artifact_id FK |
| `viz.slot_config` | 每项目槽位的审批、允许 nature、版本上限和保留天数策略 | slot_key、nature、max_versions、retention_days CHECK；`(project_id, slot_key)` 唯一 |

数据库约束核验结果：

- `viz.approval` 与 `viz.render_output` 只建立了指向 `viz.artifact(id)` 的外键。
- `viz.artifact`、`viz.slot_config` 没有指向 `core.project` / `core.tenant` 的数据库外键；viz 表也没有对 `core.principal` 的创建者/裁决者外键。
- approval/render_output 的外键只保证 artifact 存在，未用复合外键强制子记录的 tenant_id/project_id 与父 artifact 一致。当前主要依靠应用层项目可见性校验和写入路径维持该关系。
- 业务对象来源主要保存在 Spec / provenance JSONB 内，不存在可由数据库外键自动发现的业务对象依赖。迁移前必须扫描这些引用并核验引用有效性。
- `render_output` 对同一 artifact+format 采用唯一键；应用代码的 UPSERT 会覆盖该组合的内容与 renderer 元数据。因此它不是独立不可变版本历史，不能直接等同于 ART-01 的 immutable Artifact Version。

### 14.3 应用层读写路径矩阵

基于当前仓库源码搜索与文件复核：

| 表 | 读路径 | 写路径 | 当前接线状态 |
|---|---|---|---|
| `viz.artifact` | `artifact_service::get_artifact`；`queries::{list_slot_summaries,count_artifacts,list_artifacts_page}`；`viz_handlers` 详情/列表/槽位接口 | `artifact_service::create_draft` 插入；`decide_approval` 更新 published/rejected；`mark_stale` 更新 stale | 已接入 pm-api REST 路由 |
| `viz.approval` | `queries::approval_project`；`decide_approval` 行锁读取 | `request_approval` 插入；`decide_approval` 更新裁决状态 | 已接入审批请求与裁决 REST 路由 |
| `viz.render_output` | 本次未发现 pm-api 生产读取 SQL；集成测试会读取验证渲染结果 | `render_job::render_artifact` 在传入 PgPool 时 INSERT，并按 artifact+format UPSERT | 函数与测试存在；全仓调用点搜索只发现函数定义和测试，未发现 pm-api route 或 worker 的生产调用链 |
| `viz.slot_config` | 未发现应用层读取 SQL | 未发现应用层 INSERT/UPDATE/DELETE | 当前只有 migration/schema 与设计文档，未发现运行时消费者 |

补充观察：

1. `apps/pm-api/src/viz_handlers.rs` 当前路由包含槽位摘要、Artifact 详情/列表/创建、审批请求/裁决、refresh 和 Live Graph；没有发现 render_output 的读取/下载路由。
2. 当前 `artifact_refresh` 只调用 `mark_stale`，源码注释明确源对象 hash 重算仍待 V0-3 接线；不能把 refresh 路由视为已完成的 stale 检测 worker。
3. `render_job` 的持久化函数本身可写 render_output，但没有发现当前生产调用方；这与“表结构存在”或“测试通过”是不同层级的证据。
4. `slot_config` 的审批开关、允许 nature、max_versions 和 retention_days 尚未发现应用层消费路径；不能假定这些配置当前已经控制生产行为。
5. pm-api handler 对项目执行 `require_project_visible`；写操作另执行 `ensure_trusted_context`。这说明当前业务权限仍在 AI-PM，迁移后不应把这些规则隐式复制到 Artifact Service。

### 14.4 历史行数证据

历史 full-dev 精确行数快照记录如下：

| 表 | 2026-11-15 full-dev 行数 |
|---|---:|
| `viz.artifact` | 0 |
| `viz.approval` | 0 |
| `viz.render_output` | 0 |
| `viz.slot_config` | 0 |

**解释边界：** 这是有明确容器/database/user 标识的 full-dev 历史快照，优于没有环境标识的裸 rowcount，但仍不是生产数据，也不是当前实时状态。不能据此直接 DROP、TRUNCATE、跳过数据映射或宣布迁移零风险。

### 14.5 对 Artifact Service 迁移的影响

- **Artifact / Version：** `viz.artifact` 是最直接的候选数据来源。迁移时需保留每条 Spec、slot/version、state、source_hash、provenance、renderer 信息与时间戳；必须先确定 draft/rejected/stale 的历史保留策略，以及目标 Artifact Version 的映射。
- **Render Output：** 只有在确认真实数据和实际读取者后，才决定将 HTML/SVG/PNG 内容迁移为 Artifact Version Output 或可重建产物。旧表按 artifact+format UPSERT 的覆盖语义必须显式处理，不能假设存在多次渲染历史。
- **Approval：** 继续由 AI-PM 保持业务审批权威，不搬入通用 Artifact Service。迁移后应以稳定的 artifact/version 引用关联审批，保留原审批记录与裁决审计。
- **Slot Config：** 继续属于 AI-PM 业务策略，不迁移为 Artifact Service 通用配置。由于当前未发现应用层消费者，应先确认其目标语义和未来消费者，再决定保留、补齐实现或废弃。
- **业务引用：** JSONB Spec/provenance 内可能含 requirement/design/task/knowledge 等对象引用；必须做结构化提取、有效性校验和迁移映射，数据库外键清单不足以覆盖这些依赖。
- **切换策略：** 先建立旧表到 Artifact Service 的只读映射与校验工具，再分阶段切换写入和读取；不得长期双写。旧表保留到回滚窗口关闭、引用审计完成、对象 hash/数量核对通过后，才可另行批准退役。

### 14.6 ART-02D 核验矩阵与决定

| 检查项 | 状态 | 结论 |
|---|---|---|
| 四张表的 DDL / CHECK / UNIQUE / FK / trigger | VERIFIED-CODE | 已从 migration 与历史 catalog 导出核对 |
| 当前 Rust 应用层 Artifact / Approval 读写路径 | VERIFIED-CODE | 已定位 service、query 与 handler |
| render_output 生产调用链 | NOT FOUND IN REVIEWED TREE | 写入函数存在，但未发现生产调用方或 pm-api 读取路由 |
| slot_config 应用层消费者 | NOT FOUND IN REVIEWED TREE | 当前仅确认 schema 与设计定义 |
| 2026-11-15 full-dev 行数 | VERIFIED-HISTORICAL | 四表均为 0；仅适用于该历史 full-dev 快照 |
| 当前目标环境 / 生产实时行数 | NOT VERIFIED | 本次未连接数据库 |
| JSONB 业务引用的实际数据分布 | NOT VERIFIED | 未读取业务行内容 |
| 外部消费者、运维脚本或人工 SQL 依赖 | NOT VERIFIED | 仓库静态搜索不能证明仓库外不存在调用者 |

**ART-02D 决定：**

1. 当前代码证据足以确认 Artifact 与 Approval 的应用层读写边界；render_output 与 slot_config 目前存在“Schema 已建、生产接线未证实”的实现缺口。
2. 历史 full-dev 快照中四表为 0 行，但不能将其作为生产清空证明或当前迁移批准依据。
3. 继续冻结真实数据迁移与旧表退役；下一步必须取得目标环境脱敏只读 rowcount、表结构/约束快照、应用版本/迁移版本，以及仓库外消费者确认。
4. ART-01 的职责分工维持：Artifact/Version/Output 归通用 Artifact Service；Approval、Slot Config、项目授权和业务引用语义继续由 AI-PM 持有。
5. ART-02D 完成的是仓库静态核验与历史证据复核，不代表生产数据核验、迁移演练或退役批准已完成。

**当前状态：ART-02D 静态核验完成；目标环境实时数据、仓库外消费者与迁移演练仍 OPEN。**
