# ADR-ART-001：Artifact Service 独立基础服务架构基线 V1.0

> **状态：** 用户确认的架构决策基线（2026-11-24）  
> **适用范围：** AI-PM 全平台 Artifact 管理、存储、版本、引用、生成与交付  
> **关联：** ADR-VIZ-001～011、AI-PM Visualization 文档包、SeeCode 集成  
> **性质：** 明确修订既有 Visualization 的部署与存储边界；不推翻其业务语义、安全规则和渲染契约。

## 1. 决策摘要

AI-PM 建设统一的 **Artifact Service**，作为 AI-PM 依赖的底层基础 Web Server。Artifact Service 独立运行、部署、发布和测试，通过稳定、版本化 API 向 AI-PM 各业务模块提供通用产物能力。

用户确认的决策：

| 决策 | 确认值 | 规范 |
|---|---|---|
| Q1：管理范围 | A：统一管理所有 AI 生成/纳管产物 | Artifact 是平台级通用资源，不限定为 Visualization |
| Q2：存储 | B：PostgreSQL 元数据 + S3 兼容对象存储 | 数据库保存元数据；对象存储保存文件内容 |
| Q3：版本 | A：每次生成形成不可变版本 | 历史版本内容不变；新生成创建新版本 |

补充部署决定：Artifact Service 是单独 Web Server，是 AI-PM 的基础设施组成部分，不是 pm-api 内部模块，也不只是 SeeCode 渲染进程。

## 2. 核心架构原则

1. **独立服务**：独立进程、Web API、配置、健康检查、部署和回滚。
2. **独立质量责任**：拥有自己的单元、集成、安全、契约测试和发布门禁。
3. **稳定契约**：AI-PM 通过版本化 HTTP API / SDK 调用，不依赖其内部代码或数据库。
4. **业务无感知**：Artifact Service 不理解 Requirement、Design、ADR、Task 等业务语义，只保存外部引用标识。
5. **通用产物**：Artifact 不等同于 Visualization；Visualization 是 Artifact 的一种类型。
6. **不可变版本**：版本内容不原地覆盖；修改、重新生成或重新渲染均创建新版本。
7. **存储分离**：PostgreSQL 保存元数据；S3 兼容对象存储保存源文件和派生产物。
8. **生成器可替换**：Mermaid、SeeCode 及未来 Generator 通过 Adapter 接入，不能污染 Artifact 核心模型。
9. **不建立第二业务权威**：Artifact 不取代 Markdown + Git、Requirement Revision、Design Revision、ADR 或 Task 等业务权威。
10. **基础服务稳定、业务快速迭代**：pm-api 业务变更不触发 Artifact Service 全量测试或重新发布；仅执行必要的消费者契约验证和平台集成验证。

## 3. AI-PM 架构位置

Artifact Service 属于 AI-PM Infrastructure Services。

~~~text
AI-PM Business Applications
  Project / Requirement / Design / ADR / Task / Knowledge / AI-PM Agent
                         |
                         | Versioned API / SDK
                         v
AI-PM Core Services
  pm-api / Agent Runtime / MCP Gateway / Orchestration
                         |
                         | Stable Service Contract
                         v
AI-PM Infrastructure Services
  Artifact Service
      |          |
      |          +---- Generator Adapters
      |                  |-- Mermaid
      |                  |-- SeeCode
      |                  +-- Future Generators
      |
      +---- PostgreSQL (metadata)
      +---- S3-compatible Object Storage (content)
      +---- Isolated Render Worker(s)
~~~

### 3.1 服务依赖方向

- AI-PM 业务模块可以依赖 Artifact Service API。
- Artifact Service 不得依赖 pm-api、AI-PM 业务 Domain crate 或业务数据库内部表。
- Artifact Service 不得通过直接访问 AI-PM 数据库解析 Requirement、Design、ADR 或 Task。
- Artifact Service 可以依赖平台身份认证能力，但不能要求调用 pm-api 的业务写接口。
- Generator Adapter 只依赖 Artifact Service 的 Generator Contract，不反向依赖业务模块。

### 3.2 独立部署与独立测试

Artifact Service 至少具备：

- 独立 Web Server 和监听端口；
- 独立容器镜像及版本号；
- 独立数据库迁移；
- 独立 S3 Bucket / Prefix 与访问凭据；
- 独立 CI 工作流、测试命令和测试报告；
- 独立健康检查、就绪检查和可观测指标；
- 独立部署、回滚和数据恢复流程。

测试隔离规则：

| 变更 | 必须执行 | 默认不要求 |
|---|---|---|
| Artifact Service 自身代码、迁移、Adapter 或依赖变化 | Artifact Service 完整 CI | 无关 AI-PM 业务模块全量测试 |
| pm-api / Requirement / Design 等业务变化 | 变更模块测试；Artifact Consumer Contract Test | Artifact Service 完整 CI |
| Artifact API 不兼容变更 | Artifact Service 完整 CI + 所有消费者契约测试 + 受影响集成测试 | 与变更无关的业务测试可按风险裁剪 |
| 定期平台验证 / 发布候选 | 关键跨服务集成测试 | 每次业务提交重复运行所有基础服务全量测试 |

集成测试不能取消，但应按契约变更、发布、风险和定期计划触发。Artifact Service 已发布且兼容的版本作为外部依赖锁定；AI-PM 构建不得隐式拉取或编译 Artifact Service 最新源码。

代码是否立即迁出当前 AI-PM 仓库，由 ART-00 现状审计确定；无论仓库物理位置如何，独立构建、发布和测试边界都是强制要求。若暂留 Monorepo，必须采用独立工作流与路径过滤，保证普通 pm-api 变更不会触发 Artifact Service 全量 CI。

## 4. Artifact Service 职责边界

### 4.1 服务负责

- Artifact 元数据注册、查询和生命周期管理；
- 不可变 Artifact Version；
- 源文件、渲染文件及附件上传、下载、Hash 校验和对象存储；
- Artifact 与外部业务对象的多对多引用；
- Generator / Adapter 注册信息及生成任务；
- 异步生成 Job、超时、重试和失败记录；
- 内容类型、大小、格式和安全校验；
- 访问授权、审计、归档和保留策略；
- 版本历史和历史版本读取；
- 安全预览与下载交付。

### 4.2 服务不负责

- Requirement、Design、ADR、Task、Knowledge 等业务对象的创建和更新；
- 业务审批与业务状态机；
- AI Agent 决策、模型调用和业务工作流；
- 判断某个 Requirement 或 Design 是否正式批准；
- 将 Artifact 生成成功解释为业务验收通过；
- 代替 Markdown + Git 保存正式知识正文；
- 代替业务领域维护关系语义和来源事实。

业务审批仍由所属业务领域负责。Artifact Service 可以记录产物技术状态，但不能把它解释为业务对象审批。

## 5. 通用 Artifact 数据模型

以下为概念模型；具体字段、约束和索引由 ART-01 数据模型设计冻结。

### 5.1 Artifact

代表一个逻辑产物，例如系统架构图、需求分析报告或测试报告。

核心属性：id、tenant_id、project_id、artifact_key、name、artifact_type、category、status、current_version_id、created_by、created_at、updated_at。

建议初始 artifact_type：

| 类型 | 用途示例 |
|---|---|
| visualization | Mermaid / SeeCode / 其他图形 |
| document | Markdown、PDF、DOCX |
| report | 分析报告、测试报告、审计报告 |
| image | PNG、JPEG、WebP |
| diagram | 可编辑图形源文件及导出 |
| presentation | PPTX、演示文件 |
| dataset | CSV、JSON、数据快照 |
| media | 音频、视频等媒体 |
| file | 未归类文件 |

artifact_type 表示内容形态；category 表示用途语义，两者不能混用。

### 5.2 Artifact Version

每次创建、修改、重新生成或重新渲染均产生新 Version。

核心属性：id、artifact_id、version_number、source_type、source_format、source_object_key、source_hash、generator_id、generator_version、output_format、content_object_key、content_hash、mime_type、size_bytes、manifest、created_by、created_at。

规则：

1. Version 创建后内容不可变。
2. 新版本不得覆盖旧版本对象 Key。
3. current_version_id 是 Artifact 的可变指针，不改变历史 Version。
4. 回退通过更新当前版本引用完成，不复制或改写历史内容。
5. 失败 Job 不得覆盖当前可用版本。
6. 原始输入与生成产物都应保留足以复现和审计的版本信息。

### 5.3 Artifact Reference

一个 Artifact 可以被多个业务对象引用。

核心属性：id、tenant_id、project_id、artifact_id、owner_type、owner_id、relation_type、created_at。

初始 relation_type 建议：generated_from、illustrates、supports、references、evidence_for、delivered_by。

owner_type / owner_id 是外部业务引用，不建立跨服务数据库外键。业务语义和业务对象存在性由 AI-PM 业务侧校验；Artifact Service 负责引用记录与访问边界。

### 5.4 Generation Job

异步生成任务至少记录：id、artifact_id、target_version_id、generator_id、generator_version、status、idempotency_key、retry_count、error_code、error_message、started_at、completed_at。

Job 状态与 Artifact 生命周期分离。Job 失败不意味着已有 Artifact 内容不可访问。

## 6. PostgreSQL 与 S3 存储规范

### 6.1 PostgreSQL

负责 Artifact / Version / Reference 元数据、Generator 注册与版本信息、Generation Job 状态、内容 Hash、MIME、大小和对象 Key、权限相关审计与操作事件、当前版本指针及生命周期元数据。

PostgreSQL 不承担大型文件正文存储。

### 6.2 S3 兼容对象存储

负责原始源文件、Mermaid .mmd 源文件、SeeCode / 其他 Generator 输入快照、SVG / PNG / PDF / HTML 等派生产物，以及文档、报告、演示和媒体文件。

对象 Key 建议：

~~~text
artifacts/{tenant_id}/{project_id}/{artifact_id}/versions/{version_id}/
  source/{safe_filename}
  output/{format}/{safe_filename}
  manifest.json
~~~

要求：

- 数据库只保存对象 Key，不保存供应商专属的永久访问 URL。
- 下载通过短期签名 URL 或 Artifact Service 受控代理。
- 对象 Key 由服务端生成，不直接使用用户输入作为路径。
- 上传完成后校验内容 Hash、大小和声明的 MIME。
- 版本对象不可覆盖；对象存储应启用适当的版本保护/删除保护策略。
- Bucket、凭据、备份和生命周期策略由 Artifact Service 独立管理。
- 对象存储不可用时，服务返回可识别错误，不得静默丢失元数据或返回伪成功。

### 6.3 一致性与失败恢复

数据库和对象存储不具备跨系统原子事务。实现必须采用可恢复的上传协议：

1. 创建带幂等键的上传/生成记录；
2. 上传到版本专属临时对象 Key；
3. 校验 Hash、大小、格式和安全扫描结果；
4. 将对象提升/登记为不可变正式对象；
5. 在数据库事务中创建 Version 并更新当前指针；
6. 对未完成对象和孤儿对象执行可观测的补偿清理。

不能先把数据库版本标为 AVAILABLE，再异步尝试上传文件。

## 7. Generator Adapter：Mermaid 与 SeeCode

Artifact Service 提供稳定 Generator Contract。生成器是可替换实现，不决定 Artifact 通用存储模型。

建议契约：

~~~text
GeneratorAdapter
  id()
  version()
  capabilities()
  validate(input, options)
  generate(input, options)
  inspect(output, options)
~~~

生成器运行在隔离 Render Worker 中，不得获得 Artifact 数据库 DSN、AI-PM JWT、S3 长期凭据或业务权限上下文。Worker 通过最小权限任务输入和受限对象存储临时授权完成工作。

### 7.1 Mermaid Adapter（V1 正式支持）

Mermaid 是 V1 正式支持的 Generator，不仅作为 SeeCode 失败后的临时降级方案。

- 输入：Mermaid DSL 文本，保存原始 .mmd 源文件；
- 输出：至少 SVG；
- PNG：在选定渲染运行时、字体和内网零外网验证通过后启用；
- 版本：记录 Mermaid 引擎及 Adapter 版本；
- 安全：限制输入长度、图规模、渲染时间和资源；SVG 必须经过独立安全静态化检查；
- 网络：禁止渲染过程访问外网、CDN、远程字体和未授权资源；
- 失败：保留原始源文件和 Job 错误，不产生可用输出版本；
- 预览：通过统一 Artifact Viewer 安全展示静态产物。

V1 优先支持 Flowchart、Sequence Diagram、Class Diagram、State Diagram、ER Diagram、Gantt、Journey、Git Graph。具体语法范围以 Adapter Contract Tests 为准。

Mermaid 是文本 DSL 渲染器，不承担大型在线交互图查询和实时图状态管理。

### 7.2 SeeCode Adapter（V1 支持，保持既有安全约束）

SeeCode 是结构化可视化产物 Generator，不是 Artifact Service，也不是 Artifact 存储中心。

- 输入：AI-PM 自有 VizSpec，经 Adapter 编译为 SeeCode Spec；
- 输出：遵循既有 ADR-VIZ-002 M-2 签收口径，**服务端 SVG only**；
- 运行：独立隔离的 Node Render Worker 子进程；
- 版本：固定上游版本、commit 和许可证记录；
- 安全：不可信输入、stdin/临时文件传参、零凭据继承、零外网、资源限额、独立 SVG 静态化检查；检查失败拒绝入库；
- 失败：不影响 Artifact 已有版本、Mermaid Adapter、在线 Live Graph 或表格降级；
- 替换：SeeCode 上游停止维护或安全门禁失败时，可注销 Adapter，不影响 Artifact 核心数据和 API。

本基线不把 SeeCode HTML/浏览器执行模式重新引入服务端生成路径。未来若要支持可执行 HTML，必须另行完成安全架构决策，不能沿用当前 SVG 安全结论。

### 7.3 Visualization 与通用 Artifact 的边界

- Artifact Service 管理图形源文件、输出文件、版本、引用、存储和访问。
- AI-PM Visualization 业务模块继续负责 VizSpec 语义、Relation Registry、业务数据投影、权限过滤和 Live Graph。
- Live Graph（例如 G6）是 AI-PM 在线视图，不等于持久化 Artifact，也不要求每次查询都创建 Artifact。
- Mermaid 与 SeeCode 是静态/可交付图形 Generator；不得让业务领域直接依赖其私有格式。
- Artifact Viewer 由 AI-PM 前端统一接入，根据类型、MIME 和安全策略展示；不允许业务页面各自拼接对象存储 URL。

## 8. API 与 SDK 约束

建议首个 API namespace：

~~~text
/v1/artifacts
/v1/artifacts/{artifact_id}
/v1/artifacts/{artifact_id}/versions
/v1/artifacts/{artifact_id}/versions/{version_id}
/v1/artifacts/{artifact_id}/references
/v1/artifacts/{artifact_id}/jobs
/v1/generators
/v1/generation-jobs/{job_id}
/v1/health
/v1/ready
~~~

最低 API 能力：创建 Artifact；查询 Artifact 和版本；创建版本/上传源文件；获取安全下载或预览地址；建立/解除外部业务引用；请求 Generator 生成；查询 Job 状态；设置当前版本；归档和逻辑删除；查询 Generator 能力和版本。

要求：

- API 使用明确版本前缀；
- 请求支持幂等键；
- 错误响应具有稳定机器可读 code；
- 大文件采用直传对象存储或流式上传，避免经过 JSON；
- SDK 独立版本化，业务仓库锁定兼容版本；
- API 契约测试独立发布，消费者只需运行 Consumer Contract Tests。

具体认证、分页、并发控制和错误码在 ART-01 API Contract 中冻结。

## 9. 身份、授权与安全

Artifact Service 必须独立验证身份和授权，不能因为请求来自 pm-api 或前端就默认可信。

强制要求：

- 验证受信任身份签发者的令牌签名、受众、有效期和必要 claims；
- 每次读取、上传、下载、引用、归档、删除和版本切换都检查 tenant/project 范围及操作权限；
- 不信任客户端单独提交的 tenant_id / project_id；
- 不允许通过猜测 Artifact ID 跨项目访问；
- 对象存储不公开 Bucket；签名 URL 限时、限对象、限方法；
- Render Worker 零业务凭据、零数据库凭据、零长期对象存储凭据；
- 所有生成输入视为不可信内容；
- SVG、HTML、Markdown、Office、PDF 等按各自内容安全策略处理；
- 记录访问、下载、版本创建、引用变更、删除和权限拒绝审计事件。

认证与授权具体复用当前 AI-PM IAM 的方式（JWT claims、权限服务或短时授权委托）必须在 ART-00 审计现有 IAM 后确定；不得通过让 Artifact Service 直接依赖 pm-api 内部实现来解决。

## 10. 生命周期、不可变版本与删除

建议 Artifact 生命周期：active、archived、deleted（逻辑删除，待清理）。

版本内容始终不可变。来源变更产生的 stale 可作为版本/来源状态或派生标记，不能通过覆盖旧版本内容解决。

规则：

1. 新生成成功后才允许更新 Artifact 当前版本指针。
2. 失败生成不得破坏当前可用版本。
3. 归档不删除历史版本。
4. 逻辑删除前检查引用、保留策略和权限。
5. 物理删除必须经过保留期、引用检查和审计。
6. 删除业务对象不自动删除仍被其他业务对象引用的 Artifact。
7. 正式业务审批由业务领域负责；Artifact Service 不代行审批。

## 11. 与既有 Visualization 实现的兼容和修订边界

本仓库已经存在 Visualization Engine 的实现与决策，不能把它当作空白项目重建。

已观察到的资产包括：

- migrations/0020_visualization_artifact.sql：viz schema 下的 artifact、approval、render_output、slot_config；
- crates/pm-domain/src/visualization/：AI-PM 自有 VizSpec、来源引用和校验模型；
- crates/pm-application/src/visualization/：投影、校验和 Artifact 应用服务；
- crates/pm-infrastructure/src/viz_seecode_adapter.rs、viz_render_sandbox.rs：SeeCode Adapter 与隔离执行；
- tools/seecode/：锁定的 SeeCode vendor；
- ai-frontend/src/components/viz/：Viewer、版本、审批、Stale 等 UI；
- docs/visualization/ai-pm-viz-*：Visualization 设计与验收文档。

### 11.1 既有架构决策的修订范围

本 ADR 明确修订：

- **ADR-VIZ-001**：原“V1 同库同服务、V2 再评估拆分”的部署决策，被本 ADR 的“独立 Artifact Web Server、独立发布和测试”取代；Visualization 仍不是业务 Domain。
- **ADR-VIZ-006**：原“Visualization Artifact / Render Output 优先存 PostgreSQL”的存储策略，被“通用 Artifact 元数据进 PostgreSQL、文件正文进 S3 兼容对象存储”取代。
- **ADR-VIZ-002**：保留 SeeCode Adapter 与 SVG-only 安全签收约束；本 ADR 增加 Mermaid 作为 V1 正式 Generator。旧文档中 Mermaid 仅作为 Fallback 的表述，在 Artifact Service 的生成产物范围内由本 ADR 取代。
- **ADR-VIZ-003/004/005/007/008/009 及其余业务语义、安全、权限、Registry、零业务写路径约束**：继续有效，除非未来 ADR 明确修订。

### 11.2 命名、切换与旧表清理规则

当前代码库中 artifact 一词已有多个历史语义。禁止直接将现有 `viz.artifact` 表改名或复用为通用 Artifact Service 数据表。

**产品负责人已确认的项目状态（2026-11-24）：AI-PM 尚未向真实用户开放，没有真实用户使用，也没有产生真实业务数据。因此本项目不实施旧 Visualization 数据迁移，不为不存在的历史业务数据设计长期兼容机制。** 该信息属于产品负责人确认，不应伪装成数据库审计结论。ART-02E 的数据库证据仅证明其核验的 full-dev 环境四张 `viz.*` 表在核验时均为 0 行。

后续切换按以下规则执行：

1. 新 Artifact Service 使用全新 schema / migration，不复用或改名现有 `viz.*` 表。
2. AI-PM 继续拥有 Visualization 的审批、Slot Config、VizSpec、Relation Registry、项目授权和业务引用语义；这些职责在新实现中按 AI-PM 新模型实现，不迁入 Artifact Service。
3. `viz.artifact` 与 `viz.render_output` 不做历史数据回填；新实现切换后，历史开发数据不作为兼容负担。需要保留的设计样例应以受控测试 fixture 或重新生成的 Artifact 表达。
4. 先在 AI-PM 中移除旧表的所有 handler、service、query、repository、worker、测试 fixture 和 SQL 依赖，再通过后续数据库 migration 删除旧表及相关索引、函数和触发器。
5. `viz.approval` 与 `viz.slot_config` 的业务职责不得随旧表删除而丢失；应先由新的 AI-PM 业务模型承接。旧表本身不要求迁移或长期保留。
6. 清理前必须完成代码搜索、构建、测试和 migration 顺序验证；部署顺序必须保证旧代码不再访问旧表后才执行 DROP。不得仅 DROP 表而保留旧代码调用。
7. 不建立新旧表双写、不做历史 backfill、不建立旧 ID 映射或历史下载兼容层，除非未来出现新的、经产品负责人确认的真实数据需求并另行修订本 ADR。
8. 更新文档、代码和 API 中容易混淆的命名。

旧表删除属于新实现完成代码切换后的正常 schema 清理，不是历史数据迁移项目。
## 12. 测试与发布治理

### 12.1 Artifact Service 自有测试

- Domain / API 单元测试；
- PostgreSQL migration 与 repository 集成测试；
- S3 兼容对象存储集成测试；
- 上传中断、Hash 不匹配、孤儿对象和补偿恢复测试；
- 不可变版本、并发创建和幂等性测试；
- tenant/project 越权与 IDOR 测试；
- 签名 URL 过期、越权和对象隔离测试；
- Generator Contract Tests；
- Mermaid Golden Tests、语法错误、恶意输入、零外网和 SVG 静态化测试；
- SeeCode Adapter Contract Tests、真实产物安全测试、隔离与资源限制测试；
- 故障注入、超时、重试和旧版本可用性测试；
- 备份、恢复和数据一致性演练。

### 12.2 AI-PM 消费者测试

pm-api 和业务模块只需验证：

- SDK/API 使用方式正确；
- API schema 与锁定版本一致；
- 项目/租户上下文正确传递；
- 业务对象引用建立和读取正确；
- Artifact Service 不可用时，业务页面/任务按定义降级，不发生级联故障。

### 12.3 发布规则

- Artifact Service 独立版本号和镜像；
- API 兼容性策略明确；
- 数据库迁移由 Artifact Service 自己执行和负责；
- 发布前完成自有 CI；
- API 不兼容变更先发布兼容窗口和迁移计划；
- AI-PM 通过锁定 SDK/API 版本升级；
- 独立回滚不应要求同步回滚整个 AI-PM，数据库迁移必须采用向前兼容策略。

## 13. 分阶段实施路线

| 阶段 | 交付 | 说明 |
|---|---|---|
| ART-00 | 现状与边界审计 | 审计现有 Artifact/Visualization/文件/文档/AI Run 产物、IAM、S3/RustFS、CI 和部署 |
| ART-01 | 通用数据模型与 API Contract | 冻结 Artifact、Version、Reference、Job、Generator、错误码、授权与幂等 |
| ART-02 | Artifact Service 基础 Web Server | 独立服务、PostgreSQL、S3、API、SDK、独立 CI |
| ART-03 | Mermaid Adapter | .mmd 源文件、SVG 输出、校验、安全、Golden Tests |
| ART-04 | SeeCode Adapter 接入 | 复用既有 vendor 与安全约束，VizSpec 编译和 SVG 输出 |
| ART-05 | AI-PM 集成与旧数据迁移 | Viewer、业务引用、API 切换、旧 viz.* 数据迁移和回滚 |
| ART-06 | 运营与增强 | 生命周期、清理、备份恢复、指标、更多 Generator |

每个阶段都必须产出实现证据和验收结果。架构文档不代表功能已经实现。

## 14. ART-00 必须回答的审计问题

1. 当前仓库所有 artifact 概念分别代表什么？哪些已经运行、哪些仅预留？
2. viz.artifact、viz.render_output、viz.approval、viz.slot_config 的实际使用范围和数据量是什么？
3. 是否已有通用上传、附件、文档、对象存储或 AI Run 产物能力？
4. 当前 IAM 的 JWT 签发者、audience、tenant/project claims 和权限验证方式是什么？
5. 部署环境是否已有 RustFS / MinIO / S3？生产和内网对象存储如何配置？
6. 现有 CI 是否支持独立工作流、路径过滤、镜像发布和独立测试？
7. Artifact Service 应独立仓库，还是先在 Monorepo 内独立目录/流水线？比较维护成本与测试隔离效果后给出结论。
8. 当前 SeeCode Worker 能否平滑迁移到独立服务？Node 运行时、隔离、网络策略和安全门禁由谁负责？
9. Mermaid 渲染运行时如何在无外网环境输出稳定 SVG？PNG 是否需要浏览器运行时？
10. 旧 Visualization Artifact 的迁移是否需要保留 ID、版本号、审批历史和下载兼容 URL？

ART-00 不得预先假定对象存储、IAM 适配、迁移设施或独立 CI 已经存在；必须提供代码、配置、部署和测试证据。

## 15. 验收标准

| ID | 验收标准 | 级别 |
|---|---|---|
| ART-001 | Artifact Service 可独立启动、部署、健康检查 | P0 |
| ART-002 | Artifact 元数据独立存入 PostgreSQL | P0 |
| ART-003 | 源文件与产物存入 S3 兼容对象存储 | P0 |
| ART-004 | 每次生成形成不可变 Version | P0 |
| ART-005 | 一个 Artifact 支持多个外部业务引用 | P0 |
| ART-006 | 生成失败不影响当前可用版本 | P0 |
| ART-007 | API/SDK 版本化且具备契约测试 | P0 |
| ART-008 | Artifact Service CI 与 pm-api CI 独立 | P0 |
| ART-009 | 普通 AI-PM 业务变更不触发 Artifact Service 全量 CI | P0 |
| ART-010 | tenant/project 越权访问被拒绝 | P0 |
| ART-011 | Mermaid Adapter 保存 .mmd 源并输出通过安全检查的 SVG | P0 |
| ART-012 | SeeCode Adapter 遵守 SVG-only 与现有安全签收约束 | P0 |
| ART-013 | 对象上传、数据库登记和失败恢复具有一致性保障 | P0 |
| ART-014 | Artifact Viewer 统一、安全地预览与下载 | P1 |
| ART-015 | 旧 Visualization 数据迁移可审计、可验证、可回滚 | P1 |

## 16. 决策结论

AI-PM 采用 **Artifact Service First**：

- Artifact Service 是独立 Web Server，是 AI-PM 依赖的底层基础服务；
- 统一管理 AI-PM 所有类型的 Artifact；
- PostgreSQL 管理元数据，S3 兼容存储管理文件内容；
- 每次生成形成不可变版本；
- Mermaid 与 SeeCode 是正式 Generator Adapter；
- Artifact Service 独立测试、独立发布，AI-PM 通过稳定 API / SDK 使用；
- Visualization 保留业务投影、VizSpec、Relation Registry 和 Live Graph 能力，持久化产物管理迁移到 Artifact Service；
- 现有代码与数据必须先审计、再迁移，不允许另建重复设施后长期双轨运行。

本基线定义目标架构与边界；具体实施必须经过 ART-00 和 ART-01，不得将本文视为现有功能已经实现的证明。
