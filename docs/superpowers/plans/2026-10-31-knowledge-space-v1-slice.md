# 知识空间 V1 竖切（Markdown 文档登记 + 读取）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 把设计稿 `docs/architecture/knowledge-services/ai-pm-markdown-knowledge-space.md` 的第一步落成可运行的竖切：**Git 知识仓库 + 文档登记表 + 只读 REST + 前端知识页真数据**。正文只存在 Markdown 文件，PostgreSQL 只存身份/路径/版本/哈希/Commit，不存正文。

**Architecture:** 新增 `crates/pm-application/src/knowledge.rs`（文件系统 + Git 提交 + 两表写入）与 `apps/pm-api/src/documents.rs`（REST）。知识根目录由 `AI_PM_KNOWLEDGE_ROOT` 指定（默认与 ai-pm 平级的 `ai-pm-knowledge/`，**独立 Git 仓库**，首次使用自动 `git init` + 铺模板）。PostgreSQL 落 `ai.project_document` / `ai.document_revision` 两表；REVISION 只存 `git_commit_sha` + `content_hash`，不存正文。

**Tech Stack:** Rust（sqlx / tokio / git CLI，不引入 git 库）、PostgreSQL、Vue 3 + 现有 workspace 知识页。

**Spec:** [docs/architecture/knowledge-services/ai-pm-markdown-knowledge-space.md](docs/architecture/knowledge-services/ai-pm-markdown-knowledge-space.md) §四（权威划分）、§七.2（表结构）、§八（保存与状态机）、§十二（实施顺序第 1 步）。

## Global Constraints

- **PostgreSQL 不存 Markdown 正文**；`document_revision` 只存 `git_commit_sha` / `content_hash` / 元数据。
- **一个信息一个权威源**：本竖切不迁移 `delivery.requirement_revision.content`，不动需求定稿链路。
- 知识仓库是**独立 Git 仓库**，不写进 ai-pm 代码仓；所有操作走 git CLI（`git -C <root> ...`），不引第三方 git crate。
- 每任务结束即 commit（并行会话对策），只落本地，push 由用户裁决。
- 新增 npm 依赖零；Rust 新增依赖零（git 用 `std::process::Command`）。
- 测试必须用临时目录 + 临时 Git 身份，禁止污染真实知识根目录。
- 绝对路径操作；迁移 forward-only（新增 `0006_knowledge_documents.sql`）。

## Review Focus

1. **正文权威**：`GET /v1/pm/documents/{id}` 返回的 markdown 必须来自文件 + 对应 revision 的 commit，而不是数据库快照 → 测试断言「改文件后读到的内容变化」。
2. **Git 与 DB 不一致**：先 commit 后写 PG；PG 写失败时允许重试并以 `content_hash` 识别已存在 commit，不得产生重复提交 → 任务 3 的失败注入测试（可选，至少保证 commit 幂等）。
3. **路径安全**：`doc_key` / `doc_type` 必须白名单校验（`^[A-Za-z0-9._-]+$`），禁止 `..` 与绝对路径，防目录穿越。
4. **项目隔离**：文档按 `project.code` 分目录；跨项目读取返回 404。
5. **只读边界**：本竖切不暴露写 REST（用 service 层测试证明写入），避免在质量门/红线未落地前开放外部写。

## File Structure

```
ai-pm/migrations/0006_knowledge_documents.sql     # ai.project_document + ai.document_revision（新增）
ai-pm/crates/pm-application/src/knowledge.rs     # 根目录/模板/路径/git/两表读写（新增）
ai-pm/crates/pm-application/src/lib.rs           # mod knowledge + re-export
ai-pm/apps/pm-api/src/documents.rs               # GET 列表 / GET 详情（新增）
ai-pm/apps/pm-api/src/routes.rs                  # 挂载 /v1/pm/documents
ai-pm/apps/pm-api/src/lib.rs                     # pub mod documents
ai-pm/apps/pm-api/tests/documents.rs             # 集成测试（新增）
ai-pm/frontend/src/api/documents.ts              # 真数据客户端（新增）
ai-pm/frontend/src/workspace/pages/KnowledgePage.vue  # 改为真数据（V1.2 三栏保留）
ai-pm/docs/runbooks/runbook-login-chain.md                # 新增「知识空间」小节（环境变量/初始化/验收）
```

## Tasks

### Task 1 — 迁移：文档身份与修订表

- [x] `0006_knowledge_documents.sql`：按设计稿 §7.2 落 `ai.project_document`（`doc_key`/`doc_type`/`canonical_path`/`lifecycle_state`，`UNIQUE(project_id, doc_key)`、`UNIQUE(project_id, canonical_path)`）与 `ai.document_revision`（`revision_no`/`git_commit_sha`/`content_hash`/`status`/`change_summary`/`created_by`，`UNIQUE(document_id, revision_no)`）。
- [x] `doc_type` 加 CHECK：`requirement|decision|architecture|knowledge`；`status` CHECK：`draft|published|rejected|rolled_back`。
- [x] 迁移随 `sqlx::migrate!` 应用；验证 `\d ai.project_document`。
- [x] Commit：`feat(knowledge): document identity and revision tables`

### Task 2 — knowledge 服务：根目录、模板、路径、Git 提交

- [x] `knowledge.rs`：`KnowledgeRoot`（`AI_PM_KNOWLEDGE_ROOT`，默认 `<ai-pm>/../ai-pm-knowledge`；`ensure_ready()` 建目录、`git init`、写入 `templates/{requirement,change-request,adr}.md`，幂等）。
- [x] `canonical_path(project_code, doc_type, doc_key)` → `projects/<code>/<doc_type>/<doc_key>.md`；`doc_key` 白名单 + `..` 拒绝。
- [x] `create_document(...)`：写文件（不存在才写，存在且内容相同视为幂等）、`git add` + `git commit`（固定 author `AI-PM <ai-pm@local>`）、插入 `project_document` + `document_revision`（`revision_no=1`，`status='published'`），返回 `{document_id, revision, commit_sha, content_hash}`。
- [x] 单测：路径生成、非法 key 拒绝、模板幂等初始化（tempdir）。
- [x] Commit：`feat(knowledge): markdown root, templates and git commits`

### Task 3 — 读取用例与 REST

- [x] `knowledge::list_documents(pool, project_id)`：返回 id/doc_key/doc_type/current revision/updated_at（跨项目隔离）。
- [x] `knowledge::read_document(pool, project_id, document_id)`：取最新 `published` revision → 读文件 → 返回 `{document, revision, markdown, content_hash}`；文件哈希与 `content_hash` 不一致时返回 `Stale`（不静默返回旧内容）。
- [x] `apps/pm-api/src/documents.rs`：`GET /v1/pm/documents?project_id=`、`GET /v1/pm/documents/{id}?project_id=`；无 project_id 时 400。
- [x] 集成测试（tempdir + 真 git）：创建 → 列表 → 读取内容；跨项目 404；改文件后 `Stale`。
- [x] Commit：`feat(pm-api): read knowledge documents`

### Task 4 — 前端知识页接真数据

- [x] `frontend/src/api/documents.ts`：`listDocuments(projectId)` / `getDocument(id, projectId)`（`@/api/client` 既有封装）。
- [x] `KnowledgePage.vue`：左树来自列表（按 doc_type 分组），中栏渲染 markdown 纯文本段落，右栏保留 references 占位（无数据时显示空态）；mock 模式仍走 fixtures。
- [x] 新增/补齐 i18n key（zh-CN / en 同步）；`pnpm check:i18n` 绿。
- [x] 单测：有数据渲染、空态、接口失败降级。
- [x] Commit：`feat(workspace): knowledge page reads real documents`

### Task 5 — 文档与端到端验收

- [x] runbook 新增「知识空间」小节：`AI_PM_KNOWLEDGE_ROOT`、首次初始化、目录约定、只读边界。
- [x] 起栈验证：service 层创建一篇文档 → 浏览器 `/workspace/<projectId>/knowledge` 能看到标题与正文。
- [x] 全量回归：`cargo test -p pm-api -p pm-application`、`pnpm test:unit`、`pnpm check:i18n`。
- [x] Commit：`docs(runbook): knowledge space v1`

## 明确不做（后续里程碑）

- 不迁移 `delivery.requirement_revision.content` 到 Markdown（需求定稿链路保持现状）。
- 不做 `document_proposal` / 质量门 / 红线事前确认 / 否决回滚。
- 不做 `document_index_state` 与 PageIndex / Markdown 树索引检索。
- 不开放写 REST（避免绕过质量门）。

## Done When

- 能在真实 Git 知识仓库创建文档：文件 + commit + 两表行三者一致，重复创建不产生重复 commit。
- `GET /v1/pm/documents` 与 `GET /v1/pm/documents/{id}` 返回文件正文，跨项目 404，文件被手改后返回 `Stale`。
- 前端知识页显示真文档，mock 模式不回归。
- 测试全绿，全部 commit 落本地 master。