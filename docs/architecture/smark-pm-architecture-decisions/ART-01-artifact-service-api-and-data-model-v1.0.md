# ART-01：Artifact Service API、数据模型与授权契约 V1.0

> **状态：** 技术设计基线（供实施遵循；生产态接口和 IAM claims 仍须环境核验）  
> **日期：** 2026-11-24  
> **上位决策：** [ADR-ART-001](ADR-ART-001-artifact-service-v1.0.md)  
> **前置审计：** [ART-00](ART-00-artifact-service-current-state-audit-v1.0.md)  
> **配套边界：** [Visualization 集成边界](../../visualization/ai-pm-viz-27-artifact-service-integration-v1.md)

## 0. 文档效力与决策标记

本文冻结 Artifact Service V1 的目标 API、数据模型、授权模式、存储适配和迁移约束。它是实施契约，不代表服务、数据库、SDK、迁移或生产 IAM 已经实现。

- **[基线]**：由 ADR-ART-001 和已确认产品边界直接确定，实施不得擅自改变。
- **[技术裁决]**：在用户已授权继续设计的范围内，为获得可实施方案而作出的默认技术选择；如实现发现硬性冲突，须先提交变更说明。
- **[待核验]**：必须从真实环境、现有 API 或运维配置取得证据，不能用本文假设替代事实。
- **[后续决策]**：不阻塞 V1 契约，但必须在对应阶段明确处理。

## 1. V1 总体契约

### 1.1 服务边界

Artifact Service 是独立 Web Server，拥有独立 GitHub Repository、Rust workspace、Cargo.lock、数据库迁移、CI、容器镜像、版本和发布流程。

Artifact Service 是通用产物基础设施，不是 Visualization Service，也不是 AI-PM 业务服务。

| 能力 | 权威归属 |
|---|---|
| Artifact、Artifact Version、Reference、Generation Job 元数据 | Artifact Service PostgreSQL |
| 文件内容与对象存储生命周期 | file-service / RustFS；Artifact Service 管理引用和内容元数据 |
| Requirement / Design / ADR / Task 等业务对象 | AI-PM |
| 业务对象存在性、项目成员权限、业务审批、关系语义 | AI-PM 业务侧 |
| VizSpec、Relation Registry、Live Graph、业务投影 | AI-PM Visualization |
| Mermaid / SeeCode 执行与输出技术校验 | Artifact Service Generator Adapter / Worker |
| 用户身份真实性 | 平台 token-service / JWKS 信任体系 |
| Artifact API 的租户/项目/动作授权决策 | Artifact Service 验证短时委托凭证；委托由可信 AI-PM 授权边界签发 |

### 1.2 不变量

1. Version 一经提交为 READY 即不可变；更新内容必须创建新 Version。
2. Artifact 的 current_version_id 只是可变指针，不改变任何历史 Version。
3. 业务审批不进入 Artifact Service；READY 只表示技术产物可用。
4. Artifact Service 不读取 AI-PM 业务数据库，不调用内部 SQL，不依赖 pm-api crate。
5. 客户端传入 tenant_id / project_id 只是资源定位输入，不构成授权证明。
6. 文件正文不进入 PostgreSQL；Artifact Service 不另建 S3 管理控制面。
7. Worker 无数据库 DSN、用户 JWT、AI-PM 服务凭据和长期对象存储密钥。
8. 不允许长期双写旧 viz.* 与新 Artifact Service。
9. 普通 AI-PM 业务变更不触发 Artifact Service 全量 CI 或自动升级 Artifact Service。
10. API / SDK 的兼容性必须由契约测试证明，不能依赖共享内部类型。

## 2. 资源模型

### 2.1 关系总览

```text
Tenant
  └── Project
       ├── Artifact (logical resource)
       │    ├── ArtifactVersion 1 (immutable)
       │    │    ├── Source Object
       │    │    ├── Output Objects*
       │    │    └── Manifest
       │    ├── ArtifactVersion 2 (immutable)
       │    ├── Reference*  ───────> external AI-PM business object
       │    └── GenerationJob*
       └── ...
```

Artifact Service 只保存 tenant/project 范围和外部引用标识，不维护 Tenant、Project 或业务对象的主数据。

### 2.2 Artifact

Artifact 是逻辑产物容器，不等于单个文件。

| 字段 | 类型/约束 | 说明 |
|---|---|---|
| id | UUID PK | 服务生成，不接受调用方指定 |
| tenant_id | UUID NOT NULL | 可信委托范围内的租户 |
| project_id | UUID NOT NULL | 可信委托范围内的项目 |
| artifact_key | VARCHAR(120) | 项目范围内可选业务稳定键；唯一约束为 tenant+project+key（非空时） |
| name | VARCHAR(240) | 展示名称 |
| description | TEXT nullable | 简介，不作为业务权威 |
| artifact_type | ENUM-like VARCHAR | document / report / image / diagram / visualization / presentation / dataset / media / file |
| category | VARCHAR(80) nullable | 业务侧提供的分类标签，不解释其业务含义 |
| lifecycle_state | active / archived / deleted | 逻辑生命周期 |
| current_version_id | UUID nullable | 指向 READY Version；创建版本时事务内更新 |
| created_by | actor subject | 由授权委托提供 |
| created_at / updated_at | timestamptz | 服务端时间 |
| deleted_at | timestamptz nullable | 逻辑删除时间 |

**约束：**
- UNIQUE(id, tenant_id, project_id)，供复合外键确保同项目关联。
- UNIQUE(tenant_id, project_id, artifact_key) WHERE artifact_key IS NOT NULL。
- 所有业务查询都必须显式限定 tenant_id 和 project_id。
- deleted Artifact 不接受新 Reference、Version 或 Generation Job；历史记录在保留期内仍可审计。
- 不提供任意修改历史 Version 的接口。

### 2.3 ArtifactVersion

每个 Version 表示一个完整、可审计、不可变的产物快照。

| 字段 | 类型/约束 | 说明 |
|---|---|---|
| id | UUID PK | Version ID |
| tenant_id / project_id | UUID NOT NULL | 冗余范围键，保证隔离 |
| artifact_id | UUID NOT NULL | 所属 Artifact |
| version_number | BIGINT > 0 | Artifact 内单调递增 |
| state | pending / ready / failed / quarantined | 创建与安全检查状态 |
| source_format | VARCHAR(80) nullable | 例如 mmd、vizspec、md |
| source_object_id | VARCHAR(200) nullable | file-service 文件标识 |
| source_sha256 | CHAR(64) nullable | 源文件摘要 |
| generator_id / generator_version | nullable | 生成器与精确版本 |
| adapter_version | nullable | Adapter 版本 |
| manifest | JSONB NOT NULL | 输出清单、格式、对象 ID、hash、大小和生成元信息 |
| created_by / created_at | actor / timestamptz | 创建来源 |
| ready_at | timestamptz nullable | 技术校验通过时间 |
| failure_code | VARCHAR(100) nullable | 稳定错误码 |

**约束：**
- UNIQUE(artifact_id, version_number)。
- UNIQUE(id, artifact_id, tenant_id, project_id)。
- 通过复合外键保证 Version 与 Artifact 的 tenant/project 一致。
- Version READY 后，source、manifest、generator metadata 和对象引用均不可更新；可增加独立审计事件，但不能覆写快照。
- failed / quarantined Version 不得成为 current_version_id。
- current_version_id 必须引用同 Artifact 且 state=ready 的版本；若数据库无法用普通 FK 表达状态约束，则在事务服务层校验并以约束触发器或延迟一致性测试补强。
- 对象存储内容本身使用新对象 ID / 不可复用对象路径；不依赖文件名实现版本隔离。

### 2.4 ArtifactOutput

一个 Version 可有多个派生输出（例如 SVG、PNG、PDF）。为避免将多输出塞入不可查询的 JSONB，V1 建议建立独立关系表：

- id、version_id、tenant_id、project_id；
- output_role（primary / preview / attachment / source-map）；
- format、mime_type、size_bytes、sha256；
- file_object_id（file-service 标识）；
- safety_status、created_at。

唯一约束建议为 (version_id, output_role, format)。ArtifactOutput 在 Version READY 后不可更新；重新生成输出必须创建新 Version。Manifest 保存完整生成参数和工具版本，但不替代可查询的输出表。

### 2.5 ArtifactReference

Reference 是 Artifact 与外部业务对象之间的通用关联，不建立跨服务外键。

字段：id、tenant_id、project_id、artifact_id、owner_type、owner_id、relation_type、created_by、created_at、removed_at。

- owner_type 使用受控命名空间字符串，例如 requirement、design、adr、task、knowledge、project；Artifact Service 不解析其含义。
- owner_id 为外部稳定 ID；禁止仅凭该字段访问 AI-PM 对象。
- relation_type 初始允许 generated_from、illustrates、supports、references、evidence_for、delivered_by。
- UNIQUE(tenant_id, project_id, artifact_id, owner_type, owner_id, relation_type) WHERE removed_at IS NULL。
- 建立/删除 Reference 前，AI-PM 必须完成业务对象存在性和业务授权校验；Artifact Service 仍需验证调用方委托范围和 reference:write 权限。
- V1 不提供“反向查询后即可读取业务对象”的能力。

### 2.6 GenerationJob

字段：id、tenant_id、project_id、artifact_id、target_version_id、generator_id、generator_version、adapter_version、status、idempotency_key、request_hash、attempt_count、max_attempts、created_at、started_at、heartbeat_at、completed_at、error_code、error_message、trace_id。

状态机：

```text
queued -> running -> succeeded
                  -> failed
                  -> cancelled
                  -> timed_out
queued -> cancelled
running -> retry_wait -> queued
```

- Job 状态和 Artifact 生命周期分离。
- 重试仅在明确可重试错误上进行；参数错误、安全拒绝和不支持格式不可自动重试。
- 同一 idempotency_key + tenant/project + endpoint 的相同 request_hash 返回同一 Job；相同 key 不同 request_hash 返回 409 IDEMPOTENCY_KEY_REUSED。
- Worker 至少一次投递语义；生成结果必须以 job_id + attempt_id 幂等提交，禁止重复创建相同 Version 或覆盖已提交内容。
- 超时由服务端调度器判定；Worker 失联通过 heartbeat 过期进入可恢复状态。

## 3. PostgreSQL 逻辑 Schema 与隔离

### 3.1 初始表集合

V1 建议独立数据库和 schema（例如 artifact），至少包括：

- artifact
- artifact_version
- artifact_output
- artifact_reference
- generation_job
- generator_registry
- idempotency_record
- audit_event
- outbox_event（若启用事件通知）

不复用 viz.* 或 docproc.* 表。

### 3.2 关键索引

- artifact(tenant_id, project_id, lifecycle_state, updated_at DESC)
- artifact(tenant_id, project_id, artifact_type, created_at DESC)
- artifact_version(artifact_id, version_number DESC)
- artifact_reference(tenant_id, project_id, owner_type, owner_id)
- generation_job(tenant_id, project_id, status, created_at)
- generation_job(status, heartbeat_at) 用于 Worker 调度恢复
- idempotency_record(tenant_id, project_id, endpoint, idempotency_key) UNIQUE
- audit_event(tenant_id, project_id, created_at DESC)

对所有业务表建立 tenant/project 复合约束；RLS 可作为纵深防御，但不能替代应用层显式授权。数据库连接使用服务内部最小权限角色；迁移角色与运行角色分离。

### 3.3 并发与事务

- Artifact Version 编号通过对 Artifact 行执行事务级锁或原子计数分配，禁止先读 max(version_number) 再无锁插入。
- current_version_id 更新使用乐观并发：请求可携带 If-Match / expected_current_version_id；不匹配返回 409 VERSION_CONFLICT。
- Version READY、ArtifactOutput 写入、current_version_id 切换和 outbox 事件必须在同一数据库事务中提交。
- 对象文件在事务外准备；事务失败时保留可清理的孤儿对象记录，不能让数据库指向未验证文件。
- 所有状态机转移都必须校验允许的前置状态，并写入审计记录。

## 4. HTTP API V1

### 4.1 通用规则

- Base path：`/v1`。
- Content-Type：JSON API 使用 `application/json`；文件内容通过 file-service 受控上传/下载，不通过 JSON Base64。
- ID：UUID 字符串；时间统一 RFC 3339 UTC。
- 分页：cursor-based，返回 next_cursor；不承诺跨请求快照。
- 幂等：所有创建类写请求支持 `Idempotency-Key`，有效期和保留策略由服务配置，最低覆盖 24 小时。
- 并发：资源更新使用 ETag / If-Match；冲突返回 409。
- 错误：统一 `{ "error": { "code", "message", "request_id", "retryable", "details" } }`；details 不得泄漏内部路径、凭据或堆栈。
- API 兼容：新增可选字段向后兼容；移除/重命名字段、改变语义或缩窄允许值均须新 API major 或迁移窗口。
- OpenAPI 3.1 是唯一 API 描述来源；Rust DTO、SDK 和契约测试从规范生成或验证，不手工维护互相漂移的三套定义。

### 4.2 Endpoint 清单

| Method | Path | 用途 |
|---|---|---|
| POST | /v1/artifacts | 创建逻辑 Artifact |
| GET | /v1/artifacts | 按项目/类型/状态分页查询 |
| GET | /v1/artifacts/{artifact_id} | 查询元数据和当前版本 |
| PATCH | /v1/artifacts/{artifact_id} | 修改名称、描述、分类等非版本元数据 |
| POST | /v1/artifacts/{artifact_id}/versions | 创建上传版本会话 / 注册源文件 |
| GET | /v1/artifacts/{artifact_id}/versions | 列出版本 |
| GET | /v1/artifacts/{artifact_id}/versions/{version_id} | 查询版本和输出清单 |
| POST | /v1/artifacts/{artifact_id}/versions/{version_id}/commit | 完成上传、校验并提交 Version |
| POST | /v1/artifacts/{artifact_id}/current-version | 切换当前版本（If-Match） |
| POST | /v1/artifacts/{artifact_id}/references | 建立外部业务引用 |
| DELETE | /v1/artifacts/{artifact_id}/references/{reference_id} | 解除引用 |
| POST | /v1/artifacts/{artifact_id}/generation-jobs | 请求生成 |
| GET | /v1/generation-jobs/{job_id} | 查询 Job 状态 |
| POST | /v1/generation-jobs/{job_id}/cancel | 取消 Job |
| GET | /v1/generators | 查询可用 Generator 和能力 |
| GET | /v1/health | 进程存活检查 |
| GET | /v1/ready | 依赖就绪检查 |
| GET | /metrics | 内部监控指标（网络访问受限） |

V1 不提供通用 SQL、任意对象 Key 读写、业务对象 CRUD、业务审批、跨项目搜索和任意 URL 抓取接口。

### 4.3 创建 Artifact 示例

```http
POST /v1/artifacts
Authorization: Bearer <delegated-capability-token>
Idempotency-Key: 2c4c...
Content-Type: application/json

{
  "project_id": "project-uuid",
  "name": "System Architecture",
  "artifact_type": "visualization",
  "category": "architecture"
}
```

project_id 仅用于资源定位；服务端必须从委托凭证提取 tenant/project/action 范围并核对一致性。调用方不得通过请求体扩展凭证权限。

### 4.4 文件上传协议

采用两阶段上传，文件流走 file-service：

1. AI-PM 调用 Artifact Service 创建 Version Upload Session。
2. Artifact Service 校验授权、创建 PENDING Version 和 upload intent，并通过 file-service adapter 获取短时、单对象、限定方法和大小的上传授权。
3. 客户端或受控后端向 file-service 上传文件。
4. 调用 commit；Artifact Service 通过 file-service 读取可信元数据/校验 hash、大小、MIME 和安全状态。
5. 校验通过后事务提交 Version READY；校验失败转 QUARANTINED / FAILED，不更新 current_version_id。
6. 上传中断或会话过期由清理任务回收，不留下 READY 的空版本。

**重要：** ART-00 仅确认现有 docproc 已通过 SDK 调用 file-service，不证明 file-service 当前已支持上述通用能力。ART-02 必须核验并补齐接口；若缺少能力，应先扩展 file-service SDK/API，不允许 Artifact Service 直接绕过控制面访问 RustFS。

### 4.5 生成请求

```http
POST /v1/artifacts/{artifact_id}/generation-jobs
Idempotency-Key: <stable-key>

{
  "generator_id": "mermaid",
  "input": {
    "source_version_id": "uuid"
  },
  "options": {
    "formats": ["svg"]
  },
  "expected_current_version_id": "uuid-or-null"
}
```

响应为 202 Accepted，包含 job_id、status、poll_url、created_at。V1 采用轮询为必选机制；Webhook/Event 可后续增加，不能作为唯一交付机制。

生成成功后创建新的 ArtifactVersion 和 ArtifactOutput；只有满足调用方声明的 current-version 条件时才切换 current_version_id。若版本竞争失败，产物仍可作为非 current 历史版本保留，并返回明确状态，不得静默覆盖其他请求的当前版本。

## 5. IAM：身份验证与项目授权委托

### 5.1 目标模式

采用 **平台用户身份 + 短时能力委托凭证**：

- 用户登录身份仍由现有 token-service 签发并由受信 JWKS 验证。
- AI-PM 业务授权边界负责确认用户/Agent 是否能在指定项目执行指定操作。
- 授权边界签发面向 Artifact Service audience 的短时委托凭证。
- Artifact Service 独立验证委托凭证并执行 scope 校验，不调用 pm-api 内部权限函数，不读取 AI-PM 数据库。
- Worker 不持有用户凭证；只接收由 Artifact Service 签发的单次、短时任务能力。

该设计是目标契约；实际 issuer、audience、claims、签名算法和服务身份流程必须通过 ART-02 环境核验后确定。

### 5.2 委托凭证最低 claims

| Claim | 含义 |
|---|---|
| iss | 受信任签发方 |
| aud | 固定为 Artifact Service |
| sub | 发起操作的用户或受控 Agent 主体 |
| tenant_id | 唯一租户范围 |
| project_id | 唯一项目范围 |
| actions | 允许动作白名单 |
| resource | 可选资源类型/ID 限定 |
| delegation_id | 可审计委托 ID |
| jti | 唯一凭证 ID，支持撤销/重放控制 |
| iat / nbf / exp | 签发、生效、到期时间 |
| actor_type | user / agent / service |
| parent_jti | 可选父委托链，用于追踪 |

建议 TTL 2–5 分钟；具体值须结合当前 token-service 能力和调用链延迟实测。委托必须 audience-specific、project-scoped、action-scoped、短时有效。不能使用一个长期全项目万能 token。

### 5.3 动作权限集合

V1 采用细粒度动作，不使用单一 artifact:admin：

- artifact:read
- artifact:create
- artifact:update_metadata
- artifact:archive
- artifact:delete
- version:create
- version:read
- version:commit
- version:set_current
- reference:write
- reference:remove
- generation:request
- generation:read
- generation:cancel
- output:download

读取、写入、下载和删除权限必须分开。签名下载授权还须绑定单个 file object、HTTP 方法和短期有效期。

### 5.4 服务间与 Worker 授权

- AI-PM 服务端到 Artifact Service：短时委托凭证或受控服务身份 + 用户/Agent 授权上下文；具体采用哪一种由 ART-02 根据 token-service 实际能力核验。
- Artifact Service 到 file-service：复用现有服务身份换取 per-tenant 短期 JWT 的模式；不将 bootstrap token 传递给 Worker。
- Artifact Service 到 Worker：一次性 job capability，只包含 job_id、输入对象的临时读取授权、输出提交通道、允许的 generator/options、资源预算和过期时间。
- Worker 不得自行请求更宽 scope，不得访问任意 tenant/project 对象。
- 所有拒绝必须记录稳定错误码和审计事件，但日志不得记录 bearer token 原文。

### 5.5 明确拒绝的模式

- 仅校验 JWT 签名而不校验 tenant/project/action。
- 信任请求体 tenant_id / project_id。
- 只校验“调用者是 pm-api”便授予全项目权限。
- Artifact Service 复制 pm-api 的 project_visible 业务逻辑。
- Artifact Service 直接访问 AI-PM 数据库查询项目成员。
- Worker 继承用户 JWT、数据库 DSN 或长期 S3/file-service 密钥。

## 6. File-service Adapter Contract

Artifact Service 对象存储边界使用内部 StoragePort 抽象，生产实现通过现有 file-service API/SDK；不直接连接 RustFS。

最低能力：

| 能力 | 目的 |
|---|---|
| create_upload_intent | 创建单对象、短时、限方法/大小的上传授权 |
| inspect_object | 返回可信 object ID、size、checksum、detected MIME、状态 |
| create_download_intent | 创建单对象短时下载授权 |
| copy_or_finalize_object | 将临时对象安全登记为不可变正式对象（如 file-service 支持） |
| delete_object | 经保留期/引用检查后的对象删除 |
| head_object | 检查对象存在性和元数据 |
| service_auth | 使用受控服务主体获取短时 per-tenant 授权 |

对象 ID 与对象 Key 由 file-service 管理；Artifact Service 只保存 object ID、hash、size、mime 和用途。若 file-service 不提供原子 finalize/copy，使用临时对象 + 不可变正式对象的补偿流程，不能假定 rename 是原子操作。

**ART-02 待核验：** checksum 算法和可信来源、上传大小上限、multipart、直传 CORS、短时授权 TTL、租户隔离、删除/保留策略、病毒扫描状态、对象复制、审计、服务主体权限、SDK 版本兼容。

## 7. Generator Contract：Mermaid 与 SeeCode

### 7.1 统一 Adapter 接口

逻辑契约：

```rust
trait GeneratorAdapter {
    fn descriptor(&self) -> GeneratorDescriptor;
    async fn validate(&self, input: GeneratorInput) -> ValidationResult;
    async fn generate(&self, input: GeneratorInput, limits: ResourceLimits)
        -> Result<GeneratorOutput, GeneratorError>;
    async fn inspect(&self, output: GeneratorOutput) -> InspectionResult;
}
```

这段是接口意图，不是已存在的 Rust 源码。具体 crate/API 由独立仓库实施时定义。

GeneratorDescriptor 至少包含 generator_id、version、supported_input_formats、supported_output_formats、capabilities、resource_profile 和 security_profile。

### 7.2 Mermaid V1

- 输入：UTF-8 Mermaid DSL，保存原始 .mmd。
- V1 默认输出 SVG；PNG 仅在 ART-02 运行环境、字体、渲染依赖和零外网验证通过后开放。
- 固定并记录 Mermaid 引擎版本、Adapter 版本和渲染镜像 digest。
- 禁止远程资源、CDN、外网请求、未授权文件读取和任意 URL 导入。
- 限制输入字节、节点/边规模、渲染时长、内存、CPU、输出大小。
- SVG 必须经统一静态化与安全扫描；未通过不得发布为 READY。
- Golden Tests 覆盖代表性语法、字体、布局、错误输入和安全负例。

### 7.3 SeeCode V1

- 输入为经 AI-PM 业务侧验证的 VizSpec 快照及必要编译输入；Artifact Service 不解释 VizSpec 业务语义。
- 输出服务端 SVG only，继续遵守 ADR-VIZ-002 M-2 安全签收。
- 锁定 SeeCode 上游版本、commit、许可证、Node 运行时和 Adapter 版本。
- 复用已有 compiler / sandbox / SVG safety 规则和测试夹具时，必须拆除对 AI-PM crate、DB actor 和业务服务的依赖。
- 零外网、零业务凭据、零数据库凭据、零长期存储凭据；输入通过受限文件/标准输入传递，资源配额由宿主强制执行。
- 不重新引入 HTML/iframe/browser-execution 服务端输出路径。

### 7.4 Worker 隔离与资源预算

每个 Job 使用隔离进程或等效隔离单元；至少具备：
- 非 root 用户、只读根文件系统、临时目录限额；
- 网络默认 deny；无 DNS/公网 egress；
- CPU、内存、PID、执行时间和输出字节上限；
- 清理环境变量、禁止继承宿主凭据；
- 输入只读、输出目录独立；
- Worker 崩溃、超时、OOM 可被服务端识别；
- 输出进入 quarantine，完成 MIME/hash/SVG 检查后才能提交。

资源数值属于部署 profile，应在 ART-02 基准测试后配置，不在本文虚构固定值。

## 8. 错误、重试、审计和可观测性

### 8.1 稳定错误码

| HTTP | Code | 含义 |
|---|---|---|
| 400 | INVALID_REQUEST | 请求格式/字段无效 |
| 401 | UNAUTHENTICATED | 缺少或无效凭证 |
| 403 | SCOPE_DENIED | 委托凭证不包含所需范围 |
| 404 | RESOURCE_NOT_FOUND | 资源不存在或不在可见范围（避免 ID 探测） |
| 409 | VERSION_CONFLICT | 乐观并发冲突 |
| 409 | IDEMPOTENCY_KEY_REUSED | 同 key 不同请求体 |
| 413 | PAYLOAD_TOO_LARGE | 超出文件/输入上限 |
| 415 | UNSUPPORTED_MEDIA_TYPE | MIME/格式不支持 |
| 422 | GENERATOR_VALIDATION_FAILED | Generator 输入验证失败 |
| 429 | RATE_LIMITED | 限流 |
| 503 | STORAGE_UNAVAILABLE | file-service 不可用 |
| 503 | GENERATOR_UNAVAILABLE | 无可用 Worker/Generator |
| 504 | GENERATION_TIMEOUT | 生成超时 |

### 8.2 重试策略

- 客户端只对明确 retryable=true 的错误重试，并复用原 Idempotency-Key。
- 服务端 Job 重试使用指数退避 + jitter，最大次数可按 Generator profile 配置。
- 不重试 4xx 输入/授权/安全拒绝。
- file-service 短暂不可用时 Job 可进入 retry_wait；不得将半成品标为 READY。
- 取消是尽力而为；若 Worker 已完成且提交成功，取消请求返回当前终态，不回滚已提交的不可变 Version。

### 8.3 审计与指标

审计事件至少包括：Artifact 创建/修改/归档/删除、Version 创建/提交/失败、Reference 建立/解除、Job 请求/开始/重试/终止、输出下载、授权拒绝和管理操作。

Metrics 至少包括 API latency/error rate、Job queue depth、Job duration/failure/timeout、Worker saturation、Storage error/latency、孤儿对象数量、quarantine 数量和版本提交失败数。日志以 request_id、trace_id、tenant/project、artifact_id、job_id 关联；不得记录凭证或完整敏感源文件。

## 9. Visualization 旧模型职责承接与清理

### 9.1 新旧职责边界

本项目没有真实用户和历史业务数据（产品负责人于 2026-11-24 确认）。因此本节定义的是**新实现的职责承接与旧结构清理**，不是数据迁移方案。

| 现有对象 | 新实现归属 | 处理原则 |
|---|---|---|
| `viz.artifact` | Artifact Service 的 Artifact + ArtifactVersion | 不迁移旧行；新产物直接写入新服务 |
| `viz.render_output` | Artifact Service 的 ArtifactOutput + file-service object | 不迁移旧文件/行；新输出按新协议生成 |
| `viz.approval` | AI-PM Visualization 业务模型 | 审批主体、状态机、业务含义继续归 AI-PM；在新模型中实现，不迁移旧记录 |
| `viz.slot_config` | AI-PM Visualization 业务策略模型 | 槽位配置和策略继续归 AI-PM；在新模型中实现，不迁移旧记录 |
| source_hash / provenance | 双侧分工 | Artifact 保存输入快照/hash 元数据；AI-PM 负责业务来源解析和 stale 判定 |
| SeeCode render job 状态 | GenerationJob | 新生成任务使用 Artifact Service 生命周期；不迁移旧任务记录 |

### 9.2 切换与旧表清理顺序

1. **Build new authority**：先实现 Artifact Service 新 schema/API，以及 AI-PM 侧 Approval、Slot Config 等业务模型。
2. **Cut consumers over**：将 AI-PM 的 Artifact/Output 调用切换到 Artifact Service Client；业务审批与项目授权仍由 AI-PM 执行。
3. **Remove legacy dependencies**：删除旧 `viz.artifact` / `viz.approval` 读写 handler、service、query、repository、worker、旧 SQL 和不再需要的测试 fixture；确认 `render_output`、`slot_config` 没有残留运行时引用。
4. **Verify**：全仓搜索旧表名和旧 SQL；运行相关 crate 测试、API/集成测试、migration 测试；确认应用启动和请求路径不再依赖旧表。
5. **Drop legacy schema**：在代码已不访问旧表之后，通过 AI-PM 后续 migration 删除 `viz.artifact`、`viz.approval`、`viz.render_output`、`viz.slot_config` 及仅服务于这些旧表的索引、函数、触发器。清理 migration 必须与移除代码依赖的版本顺序匹配。
6. **No backfill / no dual-write**：不执行历史数据 backfill，不建立旧新表双写，不构建旧 ID 映射或历史兼容窗口。若未来产品状态发生变化，必须先修订架构决策再实施。

### 9.3 清理红线

- 不直接改名或复用 `viz.*` 表作为 Artifact Service 新表。
- 不把 Approval、Slot Config、VizSpec、Relation Registry、Live Graph 或项目授权搬入 Artifact Service。
- 不因删除旧表而丢失 Approval / Slot Config 的业务职责；先实现新的 AI-PM 业务模型。
- 不允许 DROP 表后仍保留 handler、SQL、worker 或测试代码依赖。
- 不做历史数据迁移、长期兼容或新旧双写。
- Artifact Service 的新数据权威和 file-service 对象提交协议必须先实现并通过测试；旧表清理不是两个 P0 的替代项。


## 10. SDK、部署、健康检查和发布

### 10.1 SDK

- 提供 AI-PM Rust Client SDK，独立 crate/version；对外只暴露稳定 API DTO 和错误类型。
- SDK 不依赖 pm-domain、pm-application 或 pm-infrastructure。
- SDK 支持 base URL、超时、重试策略、request_id、Idempotency-Key、ETag 和凭证注入。
- AI-PM 锁定兼容 SDK 版本；升级需跑 Consumer Contract Tests。
- 生成的 OpenAPI client 可作为其他语言集成基础，但 V1 首要消费者为 Rust AI-PM。

### 10.2 部署拓扑

- 独立 artifact-api 服务、独立 PostgreSQL 数据库/schema、独立 Worker 部署单元。
- file-service / RustFS 作为共享平台存储基础设施；Artifact Service 拥有独立逻辑 namespace/bucket-prefix 和配额策略（由 file-service 能力落实）。
- Worker 网络策略与 API 服务隔离；Worker 仅能访问任务输入/输出通道。
- API 与 Worker 可独立水平扩展；Job queue 机制由实施阶段根据现有平台设施选型，不能把消息中间件假定为已存在。
- readiness 检查 DB、file-service 授权/元数据 API 和必要队列；liveness 不因外部依赖短暂故障而失败。

### 10.3 CI 与测试责任

Artifact Service 自己运行完整 CI：fmt、clippy、unit、DB integration、file-service contract、API OpenAPI compatibility、Generator golden、SVG security、worker isolation、idempotency/concurrency、migration、recovery 和 container scan。

AI-PM 自己运行消费者契约与集成测试：SDK 兼容、授权委托、跨项目隔离、引用映射、Viewer 和业务审批边界。

普通 AI-PM 业务提交不运行 Artifact Service 全量 CI；Artifact Service 发布或 API 兼容性变化时运行受影响消费者契约测试和平台集成测试。

### 10.4 健康与恢复

- /health：进程存活，不探测所有外部依赖。
- /ready：DB、存储授权和 Job 调度关键依赖可用。
- 备份覆盖 Artifact PostgreSQL、file-service 对象和 ID 映射；必须定期演练恢复并校验数据库引用对象存在。
- 备份恢复目标 RPO/RTO 由平台 SLO 阶段确定；本文不虚构数字。
- 删除和保留策略必须与 file-service 生命周期策略一致，防止 DB 仍引用已被底层生命周期清除的对象。

## 11. ART-02 实施前置核验清单

以下均为必须取证，不得以 ART-00 静态审计替代：

| 编号 | 核验项 | 通过证据 |
|---|---|---|
| A02-01 | token-service issuer / audience / claims / JWKS / 服务身份能力 | 脱敏配置、实际签发/验签集成测试 |
| A02-02 | AI-PM 可否签发 project/action scoped 短时委托 | 授权流程图、接口契约和跨项目负例测试 |
| A02-03 | file-service upload/download intent、对象元数据和 hash | SDK/API 实测与契约测试 |
| A02-04 | file-service 租户隔离、大小限制、multipart、删除、保留 | 实测报告及权限负例 |
| A02-05 | RustFS bucket/prefix、备份、恢复与生命周期 | 运维配置和恢复演练 |
| A02-06 | viz.* 真实数据量、所有写入方、读写依赖 | SQL 统计、代码搜索、运行日志/查询证据 |
| A02-07 | SeeCode 现有安全测试、Node Worker 资源边界 | 独立运行测试及安全负例 |
| A02-08 | Mermaid 引擎、字体、语法覆盖和零外网 | 锁定版本、Golden Tests、网络隔离证明 |
| A02-09 | GitHub Actions path filter、独立镜像/SDK 发布 | workflow 实际运行记录 |
| A02-10 | 独立 Repo 名称、部署环境、域名/端口、SLO | 平台运维确认 |

## 12. 实施拆分与完成门禁

| 阶段 | 交付 | 完成条件 |
|---|---|---|
| ART-02 | 环境核验与 API/OpenAPI 细化 | A02-01~10 证据齐全；所有待核验项有结论 |
| ART-03 | 创建独立 Repository / Rust workspace | 独立 CI、Docker、版本和发布管线通过 |
| ART-04 | 核心 API + PostgreSQL 模型 | Artifact/Version/Reference/Job、幂等、并发和隔离测试通过 |
| ART-05 | file-service Adapter | 不直连 RustFS；上传/下载/hash/删除权限测试通过 |
| ART-06 | Mermaid Generator | Golden、安全、零外网和资源限制测试通过 |
| ART-07 | SeeCode Generator 迁移 | SVG-only、安全签收复用、无业务 crate 耦合 |
| ART-08 | AI-PM SDK 与授权委托 | 跨项目/跨租户/动作越权负例全通过 |
| ART-09 | Visualization 数据迁移演练 | 完整映射、hash 对账、回滚演练通过 |
| ART-10 | 分阶段切换与旧路径退役 | 单一写权威、无长期双写、业务验收完成 |

## 13. 本文尚未决定的事项

以下事项不影响当前契约边界，但不得由实现人员默默猜定：

1. 独立仓库最终命名（建议 `artifact-service`）。
2. 生产 JWT 的具体 claim 名称、签名算法、issuer/audience 和委托签发端点。
3. file-service 是否已有完整 upload intent / checksum / finalize / retention API。
4. Job queue 使用现有平台设施还是 Artifact Service 自带轻量 PostgreSQL outbox/worker polling。
5. PNG、PDF 等额外 Generator 输出何时开放。
6. 数据保留期、软删除恢复期、备份 RPO/RTO 和服务 SLO。
7. 迁移窗口与旧 viz.* 表最终退役时间。

这些是 ART-02 的取证和部署决策，不是重新讨论 Artifact Service 是否独立、是否复用 file-service、是否保留 Mermaid/SeeCode 的理由。

## 14. 验收结论

ART-01 冻结了 V1 的目标服务边界、资源模型、API 目录、授权委托原则、file-service 适配契约、Generator 安全契约、Visualization 数据迁移方向和实施门禁。

**ART-01 完成不等于 Artifact Service 已实现。** 在 ART-02 取得真实 IAM、file-service、RustFS、CI 和 viz 数据证据之前，不得创建生产迁移或开放生产流量。后续实施必须逐项满足本文契约，并将实现证据以 ART-03 及后续记录归档。
