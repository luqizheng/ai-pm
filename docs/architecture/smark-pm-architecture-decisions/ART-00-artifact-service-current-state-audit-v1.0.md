# ART-00：Artifact Service 现状与边界审计报告

> **审计日期：** 2026-11-24  
> **审计基准：** `master` 分支代码与文档；文件 SHA 记录于 §10  
> **状态：** 静态仓库审计完成；部署态数据量、生产 IAM 配置和实际 CI 触发行为仍需环境核验  
> **关联：** [ADR-ART-001](../smark-pm-architecture-decisions/ADR-ART-001-artifact-service-v1.0.md)、[Visualization 集成边界](../../visualization/ai-pm-viz-27-artifact-service-integration-v1.md)

## 1. 执行摘要

**结论：建议建设独立 Artifact Service，并建议采用独立 GitHub Repository；现阶段不建议把现有 Visualization 模块整体搬入新服务，也不建议重新建设一套对象存储。**

仓库已经具备若干可复用基础：

- RustFS 已作为 S3 兼容对象存储部署，由独立的 `file-service` 托管；
- AI-PM 已通过 `file-service` SDK、per-tenant service JWT 完成文件上传、下载和预签名；
- Document Processing 已有 Document / Version / Job / Artifact 数据模型与异步处理范式；
- Visualization 已有专用 Artifact / Approval / Render Output / Slot Config 模型；
- SeeCode 已有固定 vendor、编译器、隔离执行、SVG 安全检查和真实产物测试。

但这些能力分散在 AI-PM 单体工作区、pm-api/worker、file-service 和专用业务表中。**目前没有发现已经独立部署、独立发布、独立测试的通用 Artifact Web Service。**

用户提出的关键目标是：普通 AI-PM 业务开发或测试，不应每次都重新执行 Artifact Service 的全量测试。仅把代码拆成 crate、但仍由同一个 Cargo workspace 和根 Dockerfile统一构建，并不能可靠达成这一目标。

### 1.1 核心建议

| 事项 | 审计建议 | 理由 |
|---|---|---|
| 服务边界 | 独立 Web Server | 满足独立运行、扩缩容、故障隔离和 API 稳定性 |
| Git 仓库 | **新建独立 Repository** | 独立 Cargo workspace、依赖锁、CI、镜像和版本发布；真正避免业务变更触发服务全量构建 |
| 对象存储 | 复用现有 file-service / RustFS | 仓库已有存储控制面和 S3 后端，不重复造轮子 |
| Artifact 元数据 | Artifact Service 自有 PostgreSQL schema / 数据库 | 避免直接依赖 AI-PM 的 viz/docproc 业务表 |
| 身份认证 | 复用平台 token-service 的受信任 JWT/JWKS 体系 | 不复制身份签发；服务独立验证 token |
| 项目授权 | ART-01 设计显式授权委托/权限适配契约 | 不能直接复制 pm-api 内部 `project_visible` 代码或默认信任调用方 |
| SeeCode | 迁移为独立 Generator Adapter | 保留现有 Node 沙箱和 SVG-only 约束 |
| Mermaid | 增加独立正式 Generator Adapter | 不能仅依赖 SeeCode 的 Mermaid importer 或前端 Markdown 渲染 |
| Visualization | 业务语义留在 AI-PM | VizSpec、Relation Registry、Live Graph 和业务审批依赖业务事实 |

## 2. 审计范围与证据口径

本次通过 GitHub 对 `master` 的代码和文档进行只读检查，覆盖：

1. 数据库迁移与 Artifact 相关表；
2. Visualization 应用服务、SeeCode Adapter 和渲染沙箱；
3. Document Processing 文件存储抽象；
4. RustFS / file-service 部署与配置；
5. JWT / 服务间认证线索；
6. Cargo workspace 与容器构建边界；
7. Mermaid / SeeCode 现状及相关安全签收文档。

证据分为：

- **Observed**：在仓库文件中直接观察到；
- **Inferred**：根据现有代码结构推断，尚未通过部署态验证；
- **Decision Required**：需要在 ART-01 或实施阶段进一步冻结。

本报告不声称已连接生产数据库、读取生产 Bucket、检查运行时对象数量或执行 GitHub Actions。

## 3. Artifact 相关实现盘点

### 3.1 Visualization 专用 Artifact

**Observed**

迁移 `migrations/0020_visualization_artifact.sql` 创建 `viz` schema，并包含：

- `viz.artifact`：tenant/project、slot、version、state、nature、完整 JSONB spec、spec_hash、source_hash、provenance、renderer 信息；
- `viz.approval`：独立审批状态机、目标 Hash、期望版本号和审批人；
- `viz.render_output`：渲染输出；
- `viz.slot_config`：槽位配置。

迁移中已经包含若干重要数据库约束：

- published Artifact 的 spec 不可改写；
- published 只能转为 stale；
- stale 为终态；
- explanatory Artifact 不得发布；
- 每个项目槽位最多一条 pending approval；
- 审批锚点不可变。

`crates/pm-application/src/visualization/artifact_service.rs` 直接操作 `viz.artifact`，负责创建 draft、槽位版本递增、审批请求和审计。查询服务位于 `crates/pm-application/src/visualization/queries.rs`。

**判断**

这是 Visualization 的专用产物与发布工作流，不是通用 Artifact Service。它包含 slot、nature、VizSpec、业务审批等 Visualization 语义，不能原样搬迁为平台通用模型。

**待核验**

- 各表生产/测试环境实际数据量；
- 全部读写路径及触发器依赖；
- `render_output` 的实际内容类型、大小和使用率；
- 现有历史版本与审批记录是否必须保留原 ID。

### 3.2 Document Processing Artifact 与文件存储

**Observed**

迁移 `migrations/0016_docproc_domain.sql` 创建：

- `docproc.document`；
- `docproc.document_version`；
- `docproc.processing_job`；
- `docproc.artifact`；
- `docproc.source_mapping`。

其中 `document_version.storage_key` 和 `docproc.artifact.storage_key` 保存 file-service 文件 ID；文档版本包含 file_size、checksum、source_metadata。Artifact 是处理 Job 的转换产物，且有 append-only 保护。

`crates/pm-application/src/docproc_service.rs` 定义 ArtifactStore 抽象；`crates/pm-infrastructure/src/docproc_store.rs` 实现 FileServiceStore，使用 `digua_file_sdk`，并通过 token-service 获取 per-tenant service JWT。存储不可用时明确返回错误，不假成功。

**判断**

Document Processing 已有可复用的文件存储访问范式、Hash/元数据记录和异步 Job 设计；但 `docproc.artifact` 是处理流水线产物，不应直接升级为通用 Artifact 表。

优先复用 file-service 的存储能力和访问模式，而不是复制一份 S3 SDK/密钥管理/预签名逻辑。

### 3.3 其他业务 Artifact 与 Deliverable

**Observed**

仓库还存在任务交付物路径、Review Artifact Probe、AI 消息/产物附件等概念。它们不一定共享同一数据模型。

**判断**

全仓术语 Artifact 至少存在三类含义：

1. 可长期引用、可版本化的通用产物；
2. Document Processing Job 的转换输出；
3. Task / Review 流程中的交付物证据或文件引用。

ART-01 必须明确统一的是“文件与产物资源管理能力”，而不是强迫所有业务语义共用同一个状态机。

## 4. 对象存储与 file-service

### 4.1 已存在能力

**Observed**

`docs/architecture/ai-pm-object-storage-rustfs-v1.md` 明确记录：

- RustFS 已作为 S3 兼容对象存储落地；
- file-service 使用 RustFS 后端；
- AI-PM 通过 pm-api 文件代理访问 file-service；
- 浏览器读取使用短期预签名 URL；
- 富文本编辑器已有上传/粘贴上传链路。

`infrastructure/full-dev/docker-compose-services.yml` 包含 RustFS 服务；`infrastructure/full-dev/docker-compose.ai-pm.yml` 中 pm-api 配置了 `FILE_SERVICE_URL`、`AI_PM_TOKEN_SERVICE_URL` 和 `SERVICE_BOOTSTRAP_TOKEN`。

### 4.2 建议复用方式

**Recommendation**

Artifact Service 不应在 V1 直接持有 RustFS 长期 S3 凭据，也不应再实现一套平行的对象存储管理面。

建议：

- Artifact Service 通过受限的 file-service API / SDK 上传、下载和签名；
- Artifact Service 自己拥有 Artifact 元数据、Version、Reference、Generator Job；
- file-service 继续负责物理文件字节、Bucket、对象存储后端和签名 URL；
- 通过 storage adapter 隔离 file-service，未来可替换后端而不改变 Artifact API。

这与“PostgreSQL + S3 兼容对象存储”目标一致：S3 是底层持久化，file-service 是平台现有的存储控制面。不能把“Artifact Service 自己直连 S3”误写成目标架构的必要条件。

### 4.3 需要确认的存储事项

- file-service 对象归属是否支持 Artifact Service 的服务主体；
- 是否支持按 tenant 隔离及跨项目引用授权；
- 删除、保留期、对象版本保护和孤儿对象清理的实际语义；
- 大文件直传、下载预签名和内容 Hash 校验能力；
- 内网/生产环境的 RustFS endpoint、Bucket、备份和恢复流程。

## 5. 身份认证与项目授权

### 5.1 已观察事实

**Observed**

- pm-api 配置使用 token-service JWKS URL 验证 JWT；
- file-service 出站调用使用 service bootstrap token 换取 per-tenant service JWT；
- docproc 的 FileServiceStore 复用该模式；
- pm-api 的 Visualization handler 使用 AI-PM 内部认证主体和项目可见性校验。

### 5.2 独立服务需要解决的问题

**Inferred / Decision Required**

独立 Artifact Service 可以验证平台签发的 JWT，但“JWT 验证成功”不等于“调用者可以访问某个 project 的任意 Artifact”。

必须在 ART-01 选择并冻结一种授权契约：

A. Artifact Service 自己验证用户身份，并通过平台授权服务查询 tenant/project 权限；

B. AI-PM 业务服务向 Artifact Service 传递可验证的短时授权委托/能力凭证，明确 tenant、project、artifact、操作和有效期；

C. 组合方式：普通用户请求独立验证身份，业务敏感操作使用短时授权委托。

建议优先评估 C。无论选哪种，都禁止：

- 仅信任请求体的 tenant_id / project_id；
- 仅凭内部网络来源放行；
- 让 Artifact Service 直接访问 AI-PM 数据库读取业务成员表；
- 把 pm-api 内部 `project_visible` 代码复制后长期维护第二套业务权限规则。

## 6. SeeCode 与 Mermaid 现状

### 6.1 SeeCode

**Observed**

当前仓库已有：

- `tools/seecode/`：vendored SeeCode 0.6.3，含上游 commit 和许可证记录；
- `crates/pm-application/src/visualization/seecode_compiler.rs`：VizSpec 到 SeeCode 输入的编译逻辑；
- `crates/pm-infrastructure/src/viz_seecode_adapter.rs`：Node CLI Adapter；
- `crates/pm-infrastructure/src/viz_render_sandbox.rs`：子进程隔离；
- `crates/pm-application/src/visualization/artifact_safety.rs`：SVG 静态化检查；
- `crates/pm-application/src/visualization/render_job.rs`：生成、检查与失败处理。

`docs/visualization/ai-pm-viz-22-adr-viz-002-m2-signoff-v1.md` 记录了 Node 20 Worker、SVG-only、资源限制和安全测试签收。既有签收文档也记录了真实产物端到端测试。

**判断**

可复用的是安全约束、编译规则、测试夹具、vendor 锁定和 Adapter 行为；不能直接复用的是对 AI-PM 内部 crate、数据库连接和业务 actor 的耦合。

迁出时应把 SeeCode Adapter 改为 Artifact Service 的 Generator Contract 实现。Render Worker 不得持有数据库 DSN、用户 JWT 或长期 S3 凭据。

### 6.2 Mermaid

**Observed**

仓库中的 `tools/seecode/scripts/lib/importers/mermaid.mjs` 能把 Mermaid 转换为 SeeCode 模型；SeeCode 文档也列出 Mermaid 导入支持。

现有 Visualization 研究文档曾将 Mermaid 描述为 Markdown 内的现状渲染或 Fallback，而没有发现独立、已签收的 Artifact Service Mermaid Generator。

**判断**

Mermaid importer 不等于独立 Mermaid Renderer。若 Mermaid 是 V1 正式 Generator，应独立保存 .mmd 源文件，记录引擎/Adapter 版本，并建立 SVG 输出、安全检查、Golden Tests 和零外网验证。不能把“SeeCode 能导入 Mermaid”作为 Mermaid Adapter 已完成的证据。

## 7. 构建、CI 与独立测试边界

### 7.1 当前耦合

**Observed**

根 `Cargo.toml` 使用：

```toml
[workspace]
resolver = "2"
members = [
    "crates/*",
    "apps/*",
]
```

根 `Dockerfile` 以仓库根为构建上下文，通过同一构建阶段生成 pm-api、pm-worker、ai-pm-worker 镜像。Visualization 的 domain/application/infrastructure 代码目前位于该工作区内。

**判断**

仅新增 `apps/artifact-service` 并不自动实现独立测试边界：

- Cargo workspace 仍共享解析依赖和锁文件；
- workspace 级构建/测试可能重新编译大量 crate；
- 根 Dockerfile 和构建上下文仍然绑定仓库；
- 代码依赖若继续引用 pm-domain / pm-application，物理服务即使独立进程也无法真正独立演进。

### 7.2 GitHub Actions 现状限制

本次静态检索未获得足够证据，不能确认现有 GitHub Actions 是否已经有可用的路径过滤、独立 Artifact 工作流和独立镜像发布门禁。因此该项标记为 **Not Verified**，不得声称当前 CI 已支持或不支持。

需在 ART-02 前核对 `.github/workflows/` 的完整文件清单、触发条件、缓存键、镜像发布、SDK 发布和实际运行记录。

### 7.3 仓库选择建议

**Recommendation：新建独立 Repository，而不是长期留在当前 Monorepo。**

原因：

1. 用户明确要求普通 AI-PM 测试不应重复运行 Artifact Service 全量测试；
2. 独立仓库天然拥有独立 Cargo workspace、Cargo.lock、Dockerfile、依赖升级和 CI；
3. Artifact Service 必须通过稳定 API / SDK 使用，不能编译依赖 AI-PM 内部业务 crate；
4. SeeCode / Mermaid Worker 可以独立升级、测试和发布；
5. Artifact Service 的安全补丁和版本发布不应被 AI-PM 业务迭代节奏绑住。

但这不是立即大规模复制代码。推荐迁移方式：

- 新建独立仓库，只迁移通用 Artifact 核心、API、存储 adapter、Generator Contract、Worker 和对应测试；
- 通过版本化 HTTP API / SDK 与 AI-PM 通信；
- file-service / token-service 继续作为平台外部依赖；
- AI-PM 保留 VizSpec、Relation Registry、Live Graph、业务审批和业务投影；
- 原仓库中的 Visualization 代码在消费者切换前保持可运行；
- 数据迁移与 API 切换单独分阶段，不做长期双写。

如果独立仓库创建或跨仓库发布暂时受限，可短期在 Monorepo 内用独立目录、独立 lockfile/工作区、独立 Docker build context 和路径过滤 CI 作为过渡，但必须设定迁出时间点。**这只是过渡方案，不满足最终隔离目标。**

## 8. 推荐目标拓扑

```text
AI-PM Repository
  pm-api / AI-PM workers / business domains
  VizSpec / Relation Registry / Live Graph
  Artifact Consumer SDK
          |
          | HTTPS + versioned API
          v
Artifact Service Repository
  artifact-api
  artifact-domain
  artifact-storage (file-service adapter)
  artifact-generator-contract
  generator-mermaid
  generator-seecode
  render-worker
  independent Cargo.lock / CI / image / release
          |
          +---- Artifact PostgreSQL
          |
          +---- file-service API
                    |
                    v
               RustFS (S3)
          |
          +---- token-service JWKS / service identity
```

Artifact Service 是独立 Web Server，但生成器进程可由其独立 Worker 承载；不要求 Mermaid/SeeCode 各自变成常驻微服务。

## 9. 风险与后续任务

| ID | 事项 | 级别 | 处理阶段 |
|---|---|---:|---|
| ART-AUD-01 | 现有 viz.* 实际数据量、所有读写路径及数据保留要求 | P0 | ART-00 环境核验 |
| ART-AUD-02 | file-service 对 Artifact Service 服务主体、租户隔离和生命周期能力 | P0 | ART-01 |
| ART-AUD-03 | JWT issuer/audience/claims 与项目授权委托契约 | P0 | ART-01 |
| ART-AUD-04 | 独立仓库创建、SDK 发布与跨仓 CI 依赖策略 | P0 | ART-02 前 |
| ART-AUD-05 | GitHub Actions 当前路径过滤和镜像发布能力实测 | P1 | ART-00 补充核验 |
| ART-AUD-06 | Mermaid 独立渲染器、字体、零外网和输出稳定性 | P1 | ART-03 |
| ART-AUD-07 | SeeCode Adapter 从 AI-PM crate 解耦的具体边界 | P1 | ART-04 |
| ART-AUD-08 | viz.artifact / render_output / approval / slot_config 字段映射与回滚 | P0 | ART-01 / ART-05 |
| ART-AUD-09 | file-service 文件 ID 与 Artifact Version 的绑定、删除和孤儿清理 | P0 | ART-01 |
| ART-AUD-10 | 生产/内网 RustFS Bucket、备份与恢复实测 | P1 | ART-02 |

## 10. 证据索引

以下 SHA 为本次读取 `master` 时返回的文件 Blob SHA，便于复核：

| 文件 | SHA | 证据用途 |
|---|---|---|
| `Cargo.toml` | `9213f75ff75dba9abdf307de719524d28a75f239` | Cargo workspace 边界 |
| `Dockerfile` | 本次搜索命中，未单独读取全文 | 根 Docker 多目标构建 |
| `migrations/0020_visualization_artifact.sql` | `0e11c160bd1b34a69de7c579174e90523e4dff4b` | Visualization 专用 Artifact 模型 |
| `crates/pm-application/src/visualization/artifact_service.rs` | `3aa6209e02ea47328dc86cde99266e6b52f6d7d5` | Artifact 应用服务 |
| `crates/pm-application/src/visualization/queries.rs` | `4a2b3de3747126bb214e13b7e6e388f9c5d92cd2` | Visualization 查询 |
| `migrations/0016_docproc_domain.sql` | `5a1691b3f3d52c18c619b8f70e2112f8f3d189a9` | Document / Version / Job / Artifact |
| `crates/pm-infrastructure/src/docproc_store.rs` | `92c6a44f64e582ad3a7a02a2423e1e8a2510b725` | file-service storage adapter |
| `apps/pm-api/src/docproc.rs` | `c1cc41ba5c23d08ffd6542a5e7209404020ac8d1` | 文件/文档 API 与项目可见性 |
| `docs/architecture/ai-pm-object-storage-rustfs-v1.md` | `76b015c31d4ee0df16aaf633e25e4280d126aa7e` | RustFS/file-service 现状 |
| `docs/visualization/ai-pm-viz-22-adr-viz-002-m2-signoff-v1.md` | `282f85c9e434a548361ff0901ebf1f64688ce2e7` | SeeCode 安全签收 |
| `infrastructure/full-dev/docker-compose.ai-pm.yml` | `78970e76729ab207ce9f518350d3f69e04b8716e` | JWT / file-service 环境配置 |

补充代码检索命中：`crates/pm-infrastructure/src/viz_seecode_adapter.rs`、`crates/pm-infrastructure/src/viz_render_sandbox.rs`、`tools/seecode/scripts/lib/importers/mermaid.mjs`、`apps/pm-api/src/viz_handlers.rs`。

## 11. ART-00 审计结论

1. **独立服务：确认必要。** Artifact Service 应是 AI-PM 的底层独立 Web Server。
2. **独立仓库：推荐采用。** 这是满足独立测试、独立依赖和独立发布的最直接方式；Monorepo 路径过滤只能作为过渡。
3. **存储：复用现有 RustFS/file-service。** Artifact Service 自有元数据和版本管理，不复制物理存储设施。
4. **身份：复用 token-service 的可信身份体系，重新设计服务授权契约。** 不依赖 pm-api 内部业务表。
5. **Visualization：拆职责，不整体迁移。** 通用 Artifact、Version、Reference、Job 和 Generator 迁出；VizSpec、Relation Registry、业务投影、Live Graph 和业务审批留在 AI-PM。
6. **SeeCode：复用现有安全实现并解耦。** 保持服务端 SVG-only。
7. **Mermaid：单独建设正式 Generator。** SeeCode importer 不等于 Mermaid Adapter。
8. **数据迁移：暂不实施。** 在 ART-01 字段映射、权限、数据量和回滚策略冻结前，禁止修改现有 viz.* 表或双写。

**ART-00 静态代码与文档审计完成；部署态核验项仍开放。** 下一阶段应进入 ART-01，冻结服务 API、授权契约、元数据模型、file-service 适配与旧数据迁移映射。
