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
