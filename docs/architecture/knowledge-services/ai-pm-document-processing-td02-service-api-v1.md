# Document Processing · TD-02 服务边界与 API 契约 — V1

**上游：** 规格 §3/§7/§8 + TD-01（docproc schema）
**状态：** 技术设计待评审（本文档即实施契约）

---

## 1. 部署形态裁决（D-03 在本仓的落地）

规格 §7.1 明确「内部组件不要求 V1 拆成独立微服务」。本仓为模块化单体（pm-api + pm-worker + ai-pm-worker），Document Processing 落地为**同进程内的独立限界上下文**，D-03 的「独立服务边界」以四个硬边界实现：

| 边界 | 实现 |
|---|---|
| 数据边界 | `docproc` schema 独立（TD-01）；业务域不直查 docproc 表 |
| API 边界 | 统一前缀 `/v1/document-processing`（网关侧为 `/api/v1/document-processing`，本仓路由惯例不带 `/api` 前缀——网关剥离），独立路由模块 `pm-api/src/docproc.rs` |
| 执行边界 | 解析在 **pm-worker** 的专用 job 循环执行（§3），pm-api 只创建/查询，不做解析 |
| 依赖边界 | 业务模块只经 `pm_application::docproc_service` 接口调用；**parser 适配器不可被业务模块 import**（仅 pm-worker 的 job 执行器可见） |

## 2. 调用关系

```text
浏览器/前端 ──multipart──► pm-api /v1/document-processing/* ──写 docproc.* + 创建 job(pending)
                                                                     │
pm-worker docproc job loop ◄──claim (SKIP LOCKED + lease)────────────┘
        │ ParserAdapter 执行 → 产物 upload file-service → 写 artifact/source_mapping
        ▼
  integration.outbox_event (docproc.job.completed / .failed)  ──► 业务模块 handler
        │
KB/PC/Requirements 服务层（同进程）按需主动拉取产物（TD-05）
```

- 同步路径（pm-api）：上传入库、创建任务、状态/产物/报告查询、下载预签。
- 异步路径（pm-worker）：解析执行、产物落存、事件外发。

## 3. API 契约（V1 全量，权限 = 登录态 + tenant 边界；project 资源另校验成员可见性）

所有请求经 `AuthedPrincipal`（`tenant_id` 强制投影）；`project_id` 参数存在时执行 `project_visible` 校验（同 documents.rs 模式），跨租户一律 404。

### 3.1 上传 `POST /v1/document-processing/documents`（multipart）
- 字段：`file`（必填）、`project_id`（可选）、`filename`（缺省取 multipart filename）。
- 行为：字节 → sha256 → file-service 上传 → 建 `docproc.document` + `document_version`(v1) → 返回 201。
- 幂等：同 document 重传（同 checksum）→ 返回既有 version（201 + `duplicate: true`），不产生新版本行。**新内容才产生新 version**（D-06）。
- 限制：大小 ≤100MiB（file-service 上限对齐）、类型白名单（规格 §4.1.1 八格式 + 扩展名/魔数双检）。
- 出参：`{document_id, version_id, version_number, checksum, duplicate}`。

### 3.2 创建任务 `POST /v1/document-processing/documents/{document_id}/process`
- 体：`{target_format: "markdown", processing_profile?: string, options?: object}`（V1 仅 markdown；compare/export 见 §3.6-3.7）。
- 幂等：`uq_job_active_config` 命中 → 200 返回既有 job（`existed: true`），不入队。
- 出参：`{job_id, status, existed}`。

### 3.3 任务状态 `GET /v1/document-processing/jobs/{job_id}`
- 出参：`{job_id, status, progress, parser_name, parser_version, error_code, error_detail, artifacts: [{artifact_id, artifact_type, mime_type, size, created_at}]}`。

### 3.4 产物元数据 `GET /v1/document-processing/artifacts/{artifact_id}`
- 出参含 `download_url`（现签 file-service 预签 URL，短时效；markdown 产物另含 `content` 内联——≤1MiB 时直返文本，免二次请求）。

### 3.5 处理报告 `GET /v1/document-processing/jobs/{job_id}/report`
- 出参（规格 §8.7 全字段）：`{sections, tables, images, ocr_pages, warnings[], failures[], mapping_completeness}`——来源：conversion_report artifact 的 metadata。

### 3.6 文档比较 `POST /v1/document-processing/compare`
- 体：`{left_artifact_id, right_artifact_id}`（两个 markdown 产物）→ 创建 compare job → 产出 `conversion_report`(type=comparison) 差异报告 artifact（行级 diff 分块：added/removed/changed）。P0 实现行级；表格/图片级差异列 warning（U-10 边界）。

### 3.7 导出 `POST /v1/document-processing/export`
- 体：`{artifact_id, target_format: "html"|"csv", options?}`。V1 支持 markdown→HTML（自有渲染，零新依赖）；DOCX/PDF 导出 P1（规格 §4.1.2 V1 列表按依赖成本裁剪，本文档显式记录该偏离，待规格评审追认）。

### 3.8 列表与下载
- `GET /v1/document-processing/documents?project_id=&limit=&cursor=`——文档列表（含最新版本号/时间）。
- `GET /v1/document-processing/documents/{document_id}/versions`——版本列表。
- 原始文件下载：走 §3.4 同款预签（version_id 也当 artifact 语义查）。

### 3.9 错误契约
| HTTP | 场景 |
|---|---|
| 400 | 类型白名单外 / 魔数不匹配 / 参数非法（error_code: `unsupported_format` / `magic_mismatch` / `empty_file`） |
| 404 | 文档/任务/产物不存在或跨租户（不泄露存在性） |
| 409 | 加密文件、损坏容器（`encrypted_document` / `corrupted_container`） |
| 503 | file-service 不可用（`storage_unavailable`） |

错误体统一 `{error: {code, message, detail?}}`；`error_detail` 同步落 job 行。

## 4. 事件契约（U-06 闭合）

**硬钉裁决（TD-01-4）**：`integration.outbox_event.project_id` 由 NOT NULL 改为**可空**（AGENTS 规则 1：直接改契约，不设哨兵假值；既有查询均为 `= project_id` 过滤，NULL 行天然不命中，零兼容分支）。`pm_infrastructure::outbox` 新增 `append_team(tx, tenant_id, event)` 显式租户变体；原 `append` 保留（project 事件）。

| 事件 | 触发 | payload |
|---|---|---|
| `docproc.job.completed` | job → completed/partial_success | `{job_id, document_id, version_id, status, artifact_ids, report_summary}` |
| `docproc.job.failed` | job → failed | `{job_id, document_id, version_id, error_code}` |
| idempotency_key | — | `docproc:job:{job_id}:{final_status}` |

事件消费者：pm-worker 注册 docproc handler（标记 delivered + 触发 TD-05 的 KB 候选钩子）；事件协议对跨服务消费者保持开放（MCP/外部经 outbox 轮询亦可读）。

## 5. 模块落点（实施清单）

| 层 | 文件 | 内容 |
|---|---|---|
| domain | `pm-domain/src/docproc.rs` | 实体类型、状态枚举、ParseError/JobError |
| application | `pm-application/src/docproc_service.rs` | 上传/建任务/查询用例 + `DocumentStore` 端口（trait，file-service 适配在 infra） |
| application | `pm-application/src/docproc_parse.rs` | ParserAdapter trait + 注册表（TD-03） |
| infrastructure | `pm-infrastructure/src/docproc_repo.rs` | docproc 表 CRUD + job claim/lease SQL |
| infrastructure | `pm-infrastructure/src/docproc_store.rs` | `DocumentStore` 的 file-service 适配（digua_file_sdk，复用 files.rs 的 per-tenant JWT 模式） |
| worker | `apps/pm-worker`（main 追加）+ `pm-infrastructure/src/docproc_worker.rs` | job 循环：claim → adapter → artifacts → outbox |
| api | `apps/pm-api/src/docproc.rs` + routes.rs merge | §3 全部端点 |
| migration | `migrations/0016_docproc_domain.sql` | TD-01 DDL + outbox project_id 可空 |

## 6. 安全要点承接（规格 §9）

- 文档=不可信输入：解析在 worker 进程内、纯 Rust 库（zip/quick-xml）——无外部进程、无网络访问；zip 解压配解压比与总量上限（TD-04 §4，ZIP Bomb 防护）。
- 魔数双检防扩展名伪造；AI 防护（提示注入不提权）属 AI 分析模块职责（D-07 两层独立），docproc 只保证解析层隔离。
