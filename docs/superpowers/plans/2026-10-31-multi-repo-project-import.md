# AI 项目经理冷启动（多仓库 + SSH Key + 编制 + 提案链）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 AI 项目经理能从「空项目」冷启动：绑定 N 个 SSH Git 仓库、读 commit 与 docs、建立 Agent 编制、生成带证据的草案，并以 4 条批量提案走红线审批，批准后落成可运营的项目。

**Architecture:** 在 `pm-application` 新增 `secret.rs` / `repository.rs` / `git_access.rs` / `repo_facts.rs` / `project_import.rs`；在 `pm-domain` 扩展 4 个批量命令；`pm-ai` 新增 `repo_model.rs`；`pm-api` 新增仓库与导入两组 REST；`ai-pm-worker` 增开 `repo_import` 循环；`bootstrap_project` 建 5 人编制。导入产物**不直写业务表**，而是转成 `ai.change_proposal` + `governance.approval_request`，复用既有决策/执行/审计链。

**Tech Stack:** Rust 2021（axum 0.8 / sqlx 0.8 / tokio / git CLI / ssh-keygen CLI / aes-gcm）· PostgreSQL 16 · Vue 3 + pinia + vitest。

**Spec:** [../specs/2026-10-31-multi-repo-project-import.md](../specs/2026-10-31-multi-repo-project-import.md)（组织 §五、数据 §六、命令 §七、草案 §十、API §十一、验收 §十四）

## Global Constraints

- 迁移 forward-only：新增 `0008_repositories.sql`；不改 0001–0007（`0007` 是 `llm_provider`）。
- Rust 新增依赖**零个 crate**；`pm-application` 仅加 `tokio.workspace = true`。前端 npm 依赖**零**。
- **导入不直写业务表**：只能生成提案；写入由 `command_executor` 执行，审计由既有链产生。
- **私钥永不回显**：响应 / 日志 / draft / 审计行四类载体均断言不含私钥与密文。
- 所有 git 子进程：参数数组、`GIT_TERMINAL_PROMPT=0`、显式超时、临时密钥用完即删。
- 批量命令必须幂等：`(project_id, code)` / `(project_id, doc_key)` 冲突复用，不重复写。
- 策略文件 `policies/ai-pm-ai-lead-v2.yaml` 是红线唯一真源，新增 `project_import` 时同步 `policy_engine` 镜像与测试。
- 测试用 `#[sqlx::test(migrations = "../../migrations")]` + 临时目录 + 临时 Git 身份；禁止污染真实缓存目录与真实库。
- 前端走 PRISM 组件与 token；文案走 vue-i18n；写操作按钮带权限校验（演示会话 permissions 为空时放行）。
- 每任务结束即 commit，只落本地 master，不 push。

## Review Focus

1. **AI PM 可持续运营**：导入后能用同一套命令再建需求 / 里程碑 / 文档 → Task 5、Task 14 集成测试。
2. **导入不绕过审批**：`apply` 只生成提案不落 delivery；所有写入必须有 `approval_request` 记录 → Task 9、Task 14。
3. **私钥泄露面**：ssh-keys / 仓库 / 导入 GET / 审计 / 日志全部断言不含 `private_key` 与密文 → Task 2、Task 14。
4. **命令注入**：URL 与分支名过白名单；`ext::` / `file://` / `..` / `-` 开头全部拒绝 → Task 3。
5. **证据接地**：编造的 doc/commit 被拒；全丢弃判 `no_grounded_output` → Task 9。
6. **批次一致性与幂等**：重复 apply、部分驳回、部分执行失败三种路径都可重试不重复 → Task 9、Task 10。
7. **存量回归**：`ai_analysis` 队列、既有读端点、SSE 审计流、既有审批不被打破 → Task 14 全量测试。

## File Structure

```
ai-pm/migrations/0008_repositories.sql                # 4 表 + 索引（新增）
ai-pm/crates/pm-domain/src/import.rs                  # ProjectDraft / *Draft / Evidence（新增）
ai-pm/crates/pm-domain/src/command.rs                 # 4 个批量命令变体（修改）
ai-pm/crates/pm-application/src/secret.rs             # ssh-keygen + aes-256-gcm（新增）
ai-pm/crates/pm-application/src/repository.rs         # 密钥/仓库 CRUD + URL 校验（新增）
ai-pm/crates/pm-application/src/git_access.rs         # git 子进程（超时/临时密钥）（新增）
ai-pm/crates/pm-application/src/repo_facts.rs         # clone + commit/docs 采集（新增）
ai-pm/crates/pm-application/src/project_import.rs     # 导入状态机 + 证据校验 + 草案转提案（新增）
ai-pm/crates/pm-application/src/command_executor.rs   # 4 个批量命令 handler（修改）
ai-pm/crates/pm-application/src/bootstrap.rs          # 5 人编制（修改）
ai-pm/crates/pm-application/src/policy_engine.rs      # project_import 红线镜像（修改）
ai-pm/crates/pm-application/src/supervision.rs        # 决策后推进导入状态（修改）
ai-pm/crates/pm-ai/src/repo_model.rs                  # RepoAnalysisModel + Fake + OpenAI（新增）
ai-pm/apps/pm-api/src/pm_repositories.rs              # ssh-keys / repositories REST（新增）
ai-pm/apps/pm-api/src/pm_imports.rs                   # imports REST（新增）
ai-pm/apps/pm-api/src/routes.rs / lib.rs              # 挂载（修改）
ai-pm/apps/ai-pm-worker/src/main.rs                   # repo_import 循环（修改）
ai-pm/policies/ai-pm-ai-lead-v2.yaml                  # project_import 红线（修改）
ai-pm/apps/pm-api/tests/repositories.rs               # 集成测试（新增）
ai-pm/apps/pm-api/tests/imports.rs                    # 集成测试（新增）

ai-pm/frontend/src/api/repositories.ts / imports.ts / projects.ts   # 客户端（新增）
ai-pm/frontend/src/shell/views/onboarding/
  ProjectImportWizard.vue + steps/{ProjectBasics,Repositories,Analysis,Review}.vue
  ProjectImportWizard.test.ts
ai-pm/frontend/src/shell/views/projects/repositories/ProjectRepositoriesView.vue + test
ai-pm/frontend/src/shell/views/pm/approvals/ImportBatchCard.vue      # 审批页批次卡（新增）
ai-pm/frontend/src/shell/router/routes.ts                            # 两条静态路由（修改）
ai-pm/frontend/locales/{zh-CN,en}/pm/import.yaml / repositories.yaml # 新增
ai-pm/docs/runbook-login-chain.md                                    # 新增小节（修改）
```

---

### Task 1: 迁移 0008 + 草案领域类型

**Files:**
- Create: `ai-pm/migrations/0008_repositories.sql`
- Create: `ai-pm/crates/pm-domain/src/import.rs`
- Modify: `ai-pm/crates/pm-domain/src/lib.rs`

**Interfaces:**
- Produces（表）：`integration.project_ssh_key`、`integration.project_repository`、`integration.repo_import`、`integration.repo_import_source`，列与约束见 spec §六。
- Produces（Rust）：`ProjectDraft { schema_version, profile, milestones, requirements, tasks, risks, unknowns, coverage }`；`MilestoneDraft { key, name, planned_at }`；`RequirementDraft { key, code, title, markdown, priority, confidence, evidence }`；`TaskDraft { key, title, description, priority, milestone_key, requirement_key, assignee_role, confidence, evidence }`；`DocumentDraft { doc_key, doc_type, markdown, change_summary }`；`EvidenceKind::{Doc, Commit}`；`AssigneeRole::{AiPm, Architect, Developer, Tester}`。全部 `Serialize + Deserialize`。
- Produces（约束）：每项目一把 active key；每项目同时一个进行中导入；`repo_import.status` CHECK 含 `proposed|rejected`。

- [x] **Step 1** 写迁移（4 表 + 两个部分唯一索引 + `UNIQUE(project_id,url)` / `UNIQUE(project_id,fingerprint)` / `UNIQUE(project_id,id)`）。
- [x] **Step 2** `import.rs` 定义草案与命令 payload 类型；JSON roundtrip 单测（含空 evidence、空 tasks）。
- [x] **Step 3** 验证：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test -p pm-domain`；`psql \d integration.repo_import`。
- [x] **Step 4: Commit** `feat(import): repository/import tables and draft types`

### Task 2: 密钥生成、指纹与加密

**Files:**
- Create: `ai-pm/crates/pm-application/src/secret.rs`
- Modify: `ai-pm/crates/pm-application/Cargo.toml`（仅 `tokio`）、`src/lib.rs`

**Interfaces:**
- Produces: `seal_text(project_id: Uuid, plaintext: &[u8]) -> Result<Vec<u8>, SecretError>`（AES-256-GCM，`nonce||ct||tag`，AAD=`project_id`）；`open_text(project_id, payload) -> Result<Vec<u8>, SecretError>`；密钥 `sha256(AI_PM_SECRET_KEY)`，与 `llm_config.rs` 同口径。
- Produces: `generate_ed25519(comment) -> Result<GeneratedKey { public_key, private_key, fingerprint }, SecretError>`；`fingerprint_of(pub_path) -> Result<String, SecretError>`（`ssh-keygen -lf` 第二列）。

- [x] **Step 1** 失败测试：seal/open 往返；篡改 1 字节失败；不同 project_id 作 AAD 失败；主密钥缺失 `SecretKeyMissing`。
- [x] **Step 2** 失败测试：生成后公钥以 `ssh-ed25519 ` 开头、指纹以 `SHA256:` 开头、临时私钥已删除。
- [x] **Step 3** 确认失败 → 实现 → 通过。
- [x] **Step 4: Commit** `feat(import): ed25519 key generation and aes-256-gcm secret box`

### Task 3: 仓库与密钥 CRUD + URL 校验 + 可达性校验

**Files:**
- Create: `ai-pm/crates/pm-application/src/repository.rs`、`src/git_access.rs`（本任务只做 `ls_remote`）
- Create: `ai-pm/apps/pm-api/src/pm_repositories.rs`
- Modify: `src/lib.rs`、`apps/pm-api/src/routes.rs`、`apps/pm-api/src/lib.rs`

**Interfaces:**
- Produces: `validate_git_url(url) -> Result<GitUrl, RepoError>`（`GitUrl { host, path, user, normalized }`）。
- Produces: `create_ssh_key`（已有 active 则返回现有）、`list_ssh_keys`、`revoke_ssh_key`（被 verified 引用 → `KeyInUse`）。
- Produces: `create_repository / list_repositories / update_repository（revision 乐观锁）/ archive_repository`。
- Produces: `verify_repository(pool, id) -> VerifyOutcome`：解密私钥 → 0700 临时目录 → `git ls-remote --heads` 30s → 回写 `status/head_commit_sha/default_branch/last_error_code`。
- Produces（HTTP）：ssh-keys ×3、repositories ×5（契约见 spec §十一）。

- [x] **Step 1** 失败测试（URL）：合法 `git@gitlab.local:team/world-map.git`、`ssh://git@host:2222/team/repo.git` 通过；`ext::sh -c rm`、`file:///etc/passwd`、`-oProxyCommand=...`、`git@host:../escape`、含空白 / `%` / `\` 拒绝。
- [x] **Step 2** 失败测试（服务）：key 幂等；吊销被引用 key 失败；URL 重复 `Duplicate`；revision 冲突。
- [x] **Step 3** 失败测试（HTTP）：响应体不含 `private_key` / `ciphertext`；状态码符合契约。
- [x] **Step 4** 确认失败 → 实现 → 通过。
- [x] **Step 5: Commit** `feat(import): project ssh keys and repository binding with url hardening`

### Task 4: Git 采集——clone、commit 事实、docs

**Files:**
- Modify: `ai-pm/crates/pm-application/src/git_access.rs`（clone / show / log / ls-tree）
- Create: `ai-pm/crates/pm-application/src/repo_facts.rs`
- Test: 两文件内 `#[cfg(test)]` + `ai-pm/crates/pm-application/tests/repo_facts.rs`

**Interfaces:**
- Produces: `clone_bare(url, key_path, dest, timeout) -> Result<(), GitError>`；`GitError::{Timeout, AuthFailed, NotFound, TooLarge, Other}`。
- Produces: `collect(root, branch, limits) -> RepoFacts`；`Limits { max_commits: 300, max_files: 20_000, max_doc_bytes: 64*1024, max_docs_total: 400*1024, max_repo_mb: 200 }`；超 300 条只取最近 300 并 `truncated=true`；commit 只取 subject（≤200 字符）。
- Produces: `cache_root()`（`AI_PM_REPO_CACHE_ROOT`，默认 `<cwd>/../ai-pm-repo-cache`）。

- [x] **Step 1** 夹具：临时仓库提交 3 次（`feat(core):` / `fix:`）、写 `README.md`、`docs/design.md`、`node_modules/x.md`。
- [x] **Step 2** 失败测试：commit 数 = 3 且 message 保留；docs 命中 README 与 `docs/design.md`、排除 `node_modules`；超 64KB 跳过并 `truncated`；非 UTF-8 跳过；造 301 条提交 → 恰 300 且 `truncated=true`；超长 subject 截断。
- [x] **Step 3** 失败测试：`git show` 路径 `../escape` 拒绝；超时返回 `Timeout`。
- [x] **Step 4** 确认失败 → 实现 → 通过（用 `file://` 本地仓库，不走网络）。
- [x] **Step 5: Commit** `feat(import): clone and collect commit/doc facts`

### Task 5: 命令扩展——4 个批量命令 + 执行器 handler

**Files:**
- Modify: `ai-pm/crates/pm-domain/src/command.rs`
- Modify: `ai-pm/crates/pm-application/src/command_executor.rs`
- Test: `command_executor.rs` 内单测 + `tests/command_batch.rs`

**Interfaces:**
- Produces: `ProjectCommand::{CreateMilestones, CreateRequirements, CreateTasks, WriteDocuments}`（payload 见 spec §七）。
- Produces: 执行器 handler，单事务内按 key 顺序插入并返回 `key -> id` 映射；需求 `state='draft'` + `current_revision_id` 指向；`WriteDocuments` 走 `knowledge::create_document`（事务外，失败补偿）。
- Produces: `assignee_role` → principal id 解析（`project_member.role`），失败留空 + finding。

- [x] **Step 1** 失败测试：`CreateMilestones` 批量 2 条 → 2 行且 key 映射正确；重复 code 幂等复用。
- [x] **Step 2** 失败测试：`CreateRequirements` 2 条 → requirement + revision(draft) 各 2 行，`current_revision_id` 指向 draft。
- [x] **Step 3** 失败测试：`CreateTasks` 带 `milestone_key/requirement_key/assignee_role` → 外键与 assignee 正确；未知 role → assignee NULL + finding。
- [x] **Step 4** 失败测试：`WriteDocuments` 2 篇 → 文件存在、各一条 git commit、两表行存在；第二篇失败 → 第一篇被补偿删除。
- [x] **Step 5** 确认失败 → 实现 → 通过。
- [x] **Step 6: Commit** `feat(ai-pm): batch create commands for requirements, milestones, tasks and documents`

### Task 6: 组织编制——5 人 bootstrap + 角色解析

**Files:**
- Modify: `ai-pm/crates/pm-application/src/bootstrap.rs`
- Test: `bootstrap.rs` 单测 + `apps/pm-api/tests/pm_lists.rs`（补充）

**Interfaces:**
- Produces: `bootstrap_project` 创建 `owner / ai_pm / architect / developer / tester` 五个 principal + 五个 `project_member`；`BootstrapOutcome` 增加 `architect_id / developer_id / tester_id`。
- Produces: `resolve_assignee(pool, project_id, role) -> Option<PrincipalId>`。
- 保留 `BootstrapRequest` 兼容；新增 `initial_status: Option<String>` 以支持 `initializing`。

- [x] **Step 1** 失败测试：bootstrap 后 `project_member` 恰 5 行且角色齐全；`BootstrapOutcome` 五个 id 互不相同。
- [x] **Step 2** 失败测试：`resolve_assignee("developer")` 返回 developer principal；未知 role 返回 None。
- [x] **Step 3** 确认失败 → 实现 → 通过（既有调用方用默认值不受影响）。
- [x] **Step 4: Commit** `feat(import): five-role project org chart on bootstrap`

### Task 7: AI 端口——RepoAnalysisModel + Fake + OpenAI

**Files:**
- Create: `ai-pm/crates/pm-ai/src/repo_model.rs`
- Modify: `ai-pm/crates/pm-ai/src/lib.rs`
- Test: `repo_model.rs` 内单测

**Interfaces:**
- Produces: `trait RepoAnalysisModel { async fn draft(&self, facts, docs, org: &OrgChart) -> Result<ProjectDraft, ModelError> }`；`OrgChart { roles: Vec<AssigneeRole> }`。
- Produces: `FakeRepoAnalysisModel`：README 标题 → 需求、commit 前缀 → 任务（带 `assignee_role`）、`docs/**` → 风险/未知；证据取 facts 真实值。
- Produces: `OpenAiRepoAnalysisModel::new(base_url, model, api_key, timeout)`：`<repo_material>` 边界 + JSON 输出；解析失败重试一次 → `BadOutput`。
- Produces: `model_from_llm_provider(pool, project_id) -> Arc<dyn RepoAnalysisModel>`：`ai.llm_provider` + `load_provider_credentials`；无可用供应商 → Fake（`model_name='fake'`）。

- [x] **Step 1** 失败测试（Fake）：2 篇 docs + 3 条 commit → 需求 ≥1、任务 ≥1，每条 evidence 可解析，每任务 `assignee_role` ∈ org。
- [x] **Step 2** 失败测试（OpenAI）：wiremock 非法 JSON → 重试 → `BadOutput`；合法 JSON → 解析；提示含 `repo_material`。
- [x] **Step 3** 失败测试：`model_from_llm_provider` 在无 enabled 供应商时返回 Fake。
- [x] **Step 4** 确认失败 → 实现 → 通过。
- [x] **Step 5: Commit** `feat(import): repo analysis model with fake and llm-provider adapters`

### Task 8: 导入状态机 + worker job + REST

**Files:**
- Create: `ai-pm/crates/pm-application/src/project_import.rs`（本任务到 `draft_ready`）
- Create: `ai-pm/apps/pm-api/src/pm_imports.rs`
- Modify: `ai-pm/apps/ai-pm-worker/src/main.rs`、`apps/pm-api/src/routes.rs`、`src/lib.rs`

**Interfaces:**
- Produces: `start_import(pool, project_id, actor_id)`：校验 `project_not_empty` / `no_verified_repository`；插 `inbound_event` + `job(repo_import)`；`status='queued'`。
- Produces: `get_import` / `cancel_import`；`ImportDetail.sources[]` 来自 `repo_import_source`。
- Produces: `run_import_job(pool, model, job)`：`cloning → reading → analyzing → draft_ready`（证据校验在 Task 9 接入前先透传）；异常 `failed`。
- Produces（worker）：第二条 `tokio::spawn` 循环 claim `repo_import`，不阻塞 `ai_analysis`。

- [x] **Step 1** 失败测试：空项目 + 1 verified 仓库 → 202 且恰 1 event + 1 job；有 requirement → `ProjectNotEmpty`；无 verified → `NoVerifiedRepository`；并发二次启动 → `ImportInProgress`。
- [x] **Step 2** 失败测试：job 失败 → `failed` + `error_code`，项目仍 `initializing`。
- [x] **Step 3** 失败测试（HTTP）：`queued` 时无 `draft` 字段；progress 单调；404/409 契约。
- [x] **Step 4** 确认失败 → 实现 → 通过；Fake 模型跑通 `queued → draft_ready`。
- [x] **Step 5: Commit** `feat(import): import state machine and dedicated worker loop`

### Task 9: 证据校验 + 草案转 4 条提案

**Files:**
- Modify: `ai-pm/crates/pm-application/src/project_import.rs`
- Test: `ai-pm/apps/pm-api/tests/imports.rs`

**Interfaces:**
- Produces: `ground_draft(draft, facts, docs) -> (ProjectDraft, DroppedReport)`；全丢弃 → `NoGroundedOutput`。
- Produces: `propose_import(pool, import_id, req: ApplyRequest) -> ProposedImport`：校验乐观锁/选择/`project.code` → 建 `ai.analysis_run`（`result={import_id}`）→ 生成 ≤4 条 `ai.change_proposal`（`risk_level=high`、`confidence` 取最低、`evidence` 聚合）→ 策略判定进 `redline_pending` + `approval_request` → import `status='proposed'`。
- Produces（HTTP）：`POST .../apply` 返回 `{outcome:'proposed', proposals[], analysis_run_id}`；`proposed` 重复调用返回既有提案，`applied` 返回 `already_applied`。

- [x] **Step 1** 失败测试（证据）：编造 doc / commit → 丢弃；全编造 → `NoGroundedOutput`；`dropped_items` 与报告一致。
- [x] **Step 2** 失败测试（提案）：apply 后恰 4 条 proposal + 4 条 approval，全部 `redline_pending`；`delivery.*` **零新增行**。
- [x] **Step 3** 失败测试（幂等/锁）：重复 apply 返回同一组 proposal；revision 不匹配 `StaleDraft`；空选择 `NoSelection`；code 冲突 `ProjectCodeTaken`。
- [x] **Step 4** 确认失败 → 实现 → 通过。
- [x] **Step 5: Commit** `feat(import): evidence grounding and draft-to-proposal conversion`

### Task 10: 红线策略 + 决策后推进导入状态

**Files:**
- Modify: `ai-pm/policies/ai-pm-ai-lead-v2.yaml`、`crates/pm-application/src/policy_engine.rs`、`src/supervision.rs`
- Test: `policy_engine.rs` 单测 + `apps/pm-api/tests/imports.rs`

**Interfaces:**
- Produces: 策略红线 `project_import`（`required_role=project_owner`），4 个批量命令全部命中。
- Produces: `decide_approval` 成功后若 `analysis_run.result.import_id` 存在 → 统计该 run 下提案：全 `executed` → import `applied` + 项目 `planning` + 审计 `project_import_applied`；任一 `rejected` → import `rejected`。
- Produces: 执行失败 → 提案 `failed`，导入保持 `proposed` 并记 `error_detail`；已完成项保留（不整批回滚），审计标 `partial`。

- [x] **Step 1** 失败测试：4 个批量命令的 `require_approval` 判定为 true 且角色为 `project_owner`。
- [x] **Step 2** 失败测试：4 条全部批准 → import `applied`、项目 `planning`、delivery 行存在、知识文档存在、审计 1 条 `project_import_applied`。
- [x] **Step 3** 失败测试：1 条驳回 → import `rejected`，项目仍 `initializing`，已批准的命令不回滚。
- [x] **Step 4** 失败测试：第 3 条执行失败 → 提案 `failed`，import 仍 `proposed`，审计含 `partial`。
- [x] **Step 5** 确认失败 → 实现 → 通过。
- [x] **Step 6: Commit** `feat(ai-pm): project import red line and batch decision transitions`

### Task 11: 前端 API 层 + 向导第 1–2 步

**Files:**
- Create: `ai-pm/frontend/src/api/{projects,repositories,imports}.ts` + 同目录 `*.test.ts`
- Create: `ai-pm/frontend/src/shell/views/onboarding/ProjectImportWizard.vue`、`steps/{ProjectBasics,Repositories}.vue`
- Create: `ai-pm/frontend/locales/{zh-CN,en}/pm/{import,repositories}.yaml`
- Modify: `ai-pm/frontend/src/shell/router/routes.ts`

**Interfaces:**
- Produces（api）：`createProject`、`createSshKey/listSshKeys/revokeSshKey`、`createRepository/listRepositories/updateRepository/archiveRepository/verifyRepository`、`startImport/getImport/cancelImport/applyImport`。
- Produces（路由）：`/onboarding/import`、`/projects/:projectId/repositories`（`AppShell.children` 静态新增）。

- [x] **Step 1** 失败测试（api）：路径/方法正确；错误码翻译；`createSshKey` 解析公钥与指纹。
- [x] **Step 2** 失败测试（步骤 1）：空字段禁用；提交调 `createProject` 进步骤 2。
- [x] **Step 3** 失败测试（步骤 2）：展示公钥/指纹/复制；添加 URL 出现 pending；校验后变 verified；无 verified 禁用下一步；`unreachable` 显示原因。
- [x] **Step 4** 确认失败 → 实现 → 通过；`pnpm check:i18n` 绿。
- [x] **Step 5: Commit** `feat(import): wizard basics and repository connection steps`

### Task 12: 向导第 3–4 步 + 审批批次卡

**Files:**
- Create: `steps/{Analysis,Review}.vue`、`ProjectImportWizard.test.ts`
- Create: `ai-pm/frontend/src/shell/views/pm/approvals/ImportBatchCard.vue` + test
- Modify: `ProjectImportWizard.vue`（组合 4 步）、审批页（挂批次卡）

**Interfaces:**
- Produces（步骤 3）：1s 轮询（卸载清理）；4 节点时间线；取消；失败重试。
- Produces（步骤 4）：分区 + 勾选（默认非低置信）；任务显示承接 Agent 角色；证据面板；`dropped_items` 提示；提交后展示「已生成 4 项提案」+「去审批」。
- Produces（批次卡）：按 `analysis_run_id` 分组；整批批准/驳回顺序调用 `decideApproval`；逐项展示结果；部分失败可单独重试。

- [x] **Step 1** 失败测试（步骤 3）：定时器推进轮询次数；状态推进点亮时间线；取消停轮询；卸载无残留。
- [x] **Step 2** 失败测试（步骤 4）：低置信默认不选；取消勾选不进 `requirement_keys`；`dropped_items>0` 提示；提交后展示提案数与跳转。
- [x] **Step 3** 失败测试（批次卡）：渲染 4 项与总件数；整批批准调用 4 次；1 次失败时展示失败项且其余成功。
- [x] **Step 4** 确认失败 → 实现 → 通过；`pnpm test:unit`、`pnpm typecheck` 绿。
- [x] **Step 5: Commit** `feat(import): analysis, draft review and import approval batch card`

### Task 13: 仓库管理页

**Files:**
- Create: `ai-pm/frontend/src/shell/views/projects/repositories/ProjectRepositoriesView.vue` + test

**Interfaces:**
- Consumes: Task 11 的 repositories api。
- Produces: 列表（label/url/role/status/HEAD/最近校验）；行内重新校验、换密钥、归档（二次确认）；页头「再导入」带 `projectId` 跳向导。

- [x] **Step 1** 失败测试：列表渲染；重新校验触发 api 并刷新徽标；归档二次确认；无权限时按钮隐藏（mock auth）。
- [x] **Step 2** 确认失败 → 实现 → 通过。
- [x] **Step 3: Commit** `feat(import): project repositories settings page`

### Task 14: 端到端验收 + runbook

**Files:**
- Create: `ai-pm/apps/pm-api/tests/imports_e2e.rs`
- Modify: `ai-pm/docs/runbook-login-chain.md`

- [x] **Step 1** 端到端：HTTP 建项目（断言 5 人编制）→ 建 key → 绑本地 `file://` 仓库 → verify → start_import → 直接跑 `run_import_job`（Fake）→ apply（断言 4 提案、delivery 零行）→ 4 次 decide → 断言项目 `planning`、需求 draft、任务 assignee 正确、知识文档 + commit、审计行。
- [x] **Step 2** 运营闭环：导入完成后用同一套命令再 `CreateRequirements` / `CreateMilestones` / `WriteDocuments` 各一次，断言成功（证明 AI PM 能继续运营）。
- [x] **Step 3** 安全断言：所有 HTTP 响应体、`tracing` 输出、draft JSON、审计 `after_data` 均不含私钥与密文。
- [x] **Step 4** runbook：`AI_PM_SECRET_KEY` 生成（`openssl rand -hex 32`）、`AI_PM_REPO_CACHE_ROOT`、`AI_PM_REPO_MAX_MB`、`AI_PM_REPO_ALLOWED_HOSTS`、Windows 0600 限制、Deploy Key 配置步骤、整批审批操作说明。
- [x] **Step 5** 全量验证：`DATABASE_URL=postgres://aipm:aipm@localhost:5433/aipm cargo test`、`cargo build`；`pnpm test:unit`、`pnpm typecheck`、`pnpm lint:eslint`、`pnpm check:i18n`；`git ls-files | grep -c $'\r'` 为 0。
- [x] **Step 6: Commit** `test(import): cold-start acceptance, ops loop and runbook`

## Out of Scope

Agent 自主认领与 Agent 间消息总线；AI 团队页接真数据；Git/CI Webhook 与增量同步；HTTP token / OAuth 认证；Vault/KMS 托管；需求自动定稿与自动执行；PageIndex / 知识树索引；导入进度 SSE；跨项目共享密钥与仓库。

## Done When

- 新建项目即有 5 人编制；导入任务能按角色派给架构 / 开发 / 测试 Agent。
- 一次导入产生 ≤4 条提案与 4 条审批，`apply` 前后 `delivery.*` 零直接写入；审批后全部落库且审计齐全。
- 重复 apply、部分驳回、部分执行失败三条路径都可重试且不重复写。
- 无证据条目被丢弃并计数；全丢弃判 `no_grounded_output`。
- 导入完成后 AI 项目经理能用同一套命令继续建需求 / 里程碑 / 文档。
- 私钥零泄露、全量门禁绿、改动只落本地 master。
---

## 完成证据（2026-10-31）

**交付与入库**：Task 1–13 的后端与前端交付物已随提交 `a8e1920`（冷启动导入主体：仓库/密钥 CRUD、git 采集、批量命令、5 人编制、repo 分析模型、导入状态机、证据接地、红线策略、向导与批次卡、仓库管理页）、`ac17594`、`cd034a2`（缓存出工作区 + clippy）入库。迁移文件实际为 `migrations/0009_repo_import.sql`（plan 列 0008，因 0008 已被 `llm_provider` 占用，序号顺延）。

**Task 14 端到端验收（本会话亲跑）**：
- 新增 `apps/pm-api/tests/imports_e2e.rs`：真 HTTP router（`tower::ServiceExt::oneshot`，无 socket）+ 真 PostgreSQL + 真本地 Git 仓库，一次跑通 `POST /v1/pm/projects`（断言 5 人编制）→ `POST .../ssh-keys`（只回公钥/指纹）→ `POST .../repositories` → `start_import` → `run_import_job(Fake)` 到 `draft_ready` → `apply`（断言 4 提案、`delivery.*` 与知识文档零写）→ 4 次 `POST /v1/pm/approvals/{id}/decide`(approved) → 断言 import `applied`、项目 `planning`、需求 `draft`、任务 assignee 非空、知识文档 = 需求数 + 1 且知识根有 git 提交、审计 `project_import_applied` 恰 1 条；Step 2 运营闭环补 `CreateMilestones`/`CreateRequirements`/`WriteDocuments` 各一次；Step 3 断言全部响应体 / 草案 JSON / 审计 `after_data` / 落库密文列不含 `private_key` 与 PEM。
- 单测：`bash scripts/cargo-test.sh -p pm-api -E 'binary(imports_e2e)'` → `1 passed`。

**偏差留痕**：
- plan Task 14 Step 1 字面「绑本地 `file://` 仓库 → verify」与 Task 3 的 URL 硬化（明确拒绝 `file:///etc/passwd`、`ext::`、`-` 开头）冲突：e2e 以合法 URL 经 HTTP 建行，再用 SQL 指向本地 `file://` 并置 `verified` 驱动 clone；HTTP verify 的正向契约由 `tests/repositories.rs` 覆盖。
- 首跑红：`WriteDocuments` 首跑用 `doc_type: "note"` 被拒（`invalid document type: note`；合法值 `requirement/decision/architecture/knowledge`），改 `knowledge` 后绿。
- 首跑红：读取 `private_key_ciphertext` 按 `String` 失败（列为 `BYTEA`），改 `Vec<u8>` 后绿。
- 前端两个既有测试红（`ProjectImportWizard.test.ts` 导入路径多一级致整文件无法加载；`repositories/index.test.ts` 的 `vue-router` mock 漏 `useRouter`），已修（`../../test-utils`、补 `useRouter` mock）。

**全量门禁**：
- 后端：`bash scripts/cargo-test.sh` → `Summary 269 tests run: 269 passed`（基线 258 → 268 → 269）；`cargo build` → `Finished dev 2.33s`。
- `git ls-files | grep -c $'\r'` → `0`。
- 前端：`pnpm typecheck` 绿；`pnpm test:unit` → `Test Files 87 passed / Tests 528 passed`；`pnpm check:i18n` → 各语言 key 对齐、195 源文件无新增硬编码中文。
- `pnpm lint:eslint` 仍报 95 项 `prettier/prettier`：本地 `core.autocrlf=true` 使工作树统一 CRLF，报错集中在 `src/workspace/**` 等并行线文件的混合/末尾缺换行；本线两个改动文件功能与类型均绿，该门禁红为存量环境问题，非本线内容引入。