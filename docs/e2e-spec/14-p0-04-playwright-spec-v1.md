# AI-PM · P0-04 Requirement 与 Knowledge 权威性 · Playwright E2E 规格 V1.0

> **性质**：P0-04 套件的**可执行**自动化规格（4 条案例：P0-011 ~ P0-014 + 综合一致性）。
> **上位文件**：`docs/e2e-spec/10-p0-test-specification-v1.md`（P0 总规格）、`docs/e2e-spec/13-p0-03-playwright-spec-v1.md`（AI 人工确认闸，本套件复用其口径）。
> **设计权威**：`docs/ai-pm-markdown-knowledge-space.md`（§四 one authority per fact、§八 Git/DB sync）、T-060（知识索引）。
> **基线**：master @ `2b3f509e`；迁移基线 `migrations/0001_baseline.sql` + 0006（doc_type CHECK）。
> **判定纪律**：三存储独立验证（Git 文件 / Git 命令 / SQL 直查），API 返回不作为唯一证据。

---

## 0. 评审结论速览（先读这节）

**规格的三层权威模型在本仓全部真实落地**，且实现比规格表述更强。以下 6 条核验结论把规格断言锚定到真实代码：

| # | 级别 | 规格假设 | 本仓事实 | 处置 |
|---|---|---|---|---|
| **R27** | ✅+🟠 | 创建 Requirement 即产生 Markdown + Git 提交 | **绑定点 = `submit_revision`，不是 create**：draft 只落 PG，投影文档**随首次 submit 诞生**（`crates/pm-application/src/requirement_projection.rs` §6.6「导入产生的 draft 不绑定」；`requirement.rs:696` submit 签名带 `KnowledgeRoot`） | P0-011 的「正式创建」= **draft → submit → confirm 全链**（D18）；步骤表加 submit 步 |
| **R28** | ✅ | 版本引用 = `current_revision_ref` | 真实字段：`delivery.requirement.current_revision_id` + `ai.document_revision.git_commit_sha`（`markdown_version_ref`，`requirement_projection.rs:7`）+ `content_hash`（DB/Git 同源 sha256，`knowledge::content_hash` 单点收敛） | 断言字段名改真实列 |
| **R29** | ✅+🟠 | 命名 `REQ-E2E-001` / `KB-E2E-001` | `REQ-E2E-001` 是**业务编号** = `delivery.requirement.code`；投影文档 `doc_key = req-{requirement_id(uuid)}`（0006 CHECK `^[A-Za-z0-9._-]+$`）——测试按 **code** 查需求、按 **doc_key** 查投影；知识文档无编号字段 → doc_key 自拟 `kb-e2e-001` | §2.2 数据表按双键命名 |
| **R30** | ✅ | PageIndex 可删除重建 | **真实实现**：索引表 = `ai.document_section_index` + `ai.document_index_state`（kb 镜像同构）；重建入口 ×2：`POST /v1/pm/documents/revisions/{revision_id}/reindex`（单文档 `rebuild_document`）+ `POST /v1/pm/documents/reindex`（项目级全量，T-060 §5.5）；检索 = `GET /v1/pm/documents/search`；索引缺失降级 = `knowledge_search.rs:51` `IndexStateMissing`；质量标志 = `knowledge_quality.rs:30` `reindex_required` | 用例全部可落地 |
| **R31** | ✅ | 「不能从 DB/PageIndex 反向恢复正式知识」 | **代码里已有防线的正面工具**：`requirement_projection.rs` 提供 `rebuild_projection`（**Git→content 反向投影**）——恢复源是 **Git**，不是 DB/索引；读侧 content jsonb 是「读投影」（read.rs），损坏可重建 | P0-014「错误实现」红线与代码裁决一致，写入断言 |
| **R32** | ⚠️ | 「Git 仓库可用于保存权威内容」 | 知识空间 root = `KnowledgeRoot::from_env()`（**本地 Git 仓库目录**，env 注入），与项目关联的 `integration.project_repository`（SSH 远端）**无关** | Fixture 需准备本地 Git 仓库目录 + env 指向（D17） |

**三层职责 → 代码事实对照**

| 层 | 规格职责 | 本仓实现 |
|---|---|---|
| **Markdown + Git** | 内容权威 | `crates/pm-application/src/knowledge.rs:1-6`："Markdown + Git is the authoritative body"；一份文档 = 一个文件 + 一个 commit + 两行（`ai.project_document` + `ai.document_revision`）；Git 提交**先于** PG 事务（§6.7） |
| **PostgreSQL** | 状态权威 | 文档身份 / 规范路径 / revision 序号 / commit sha / content_hash / 提案与红线 / 审计（`governance.audit_log`）/ 事件 |
| **PageIndex** | 派生索引 | `ai.document_section_index`（章节锚点）+ `ai.document_index_state`（索引状态机）；retrieval fusion 中 PageIndex 是 KB 通道内部定位加成（`retrieval/fusion.rs:5-6`），可删可重建 |

---

## 1. Requirement 与普通描述的区别（规格 §2 → 本仓实体表）

| 内容 | 性质 | 本仓落点 |
|---|---|---|
| **正式 Requirement** | 有独立 ID、状态、版本及审批记录 | `delivery.requirement`（uuid 主键 + `code` 业务编号）+ `delivery.requirement_revision`（5 态状态机）+ 确认账 `delivery.requirement_confirmation` |
| 正式 Requirement 的**权威正文** | Markdown + Git | 投影文档 `doc_key=req-{uuid}`（`ai.project_document`，doc_type=`'requirement'`），版本 = `ai.document_revision.git_commit_sha` |
| 普通项目描述 | 普通描述性内容 | `core.project.description`（无状态机） |
| AI 需求分析结果 | 分析产物 | `ai.analysis_run.result` + `ai.change_proposal` |
| AI 修改建议 | 待人工确认的建议 | `ai.change_proposal(status='proposed')` / `governance.approval_request(pending)` / 修订 `pending_confirmation` |
| Requirement Revision | 正式需求的历史版本 | `delivery.requirement_revision` 行（不覆盖）+ 对应 Git commit（不覆盖） |

> 「不能因为某段 Markdown 出现需求文字就自动成为正式 Requirement」在本仓 = 投影绑定只发生在 `submit_revision`（唯一绑定点），且 confirm 需真人 + 哈希门 —— **没有人工链路就没有正式需求**。

---

## 2. 测试环境与 Fixture

### 2.1 测试数据（按 R29 修正为双键命名）

| 对象 | 名称 | 说明 |
|---|---|---|
| Team | `e2e-knowledge-team` | 独立 tid（92xx 段，与 P0-01~03 不重叠） |
| Project | `e2e-knowledge-project` | Key `E2EKB92xx` |
| User A | `e2e-human-owner` | human 主体（jwks 自签），Owner |
| AI Agent | `e2e-ai-lead` | **不手工创建** —— 建项目时 bootstrap 自动产生 `kind='ai_pm'` 主体（`bootstrap.rs:250,289`），测试按 kind 查询取用 |
| Requirement | code `REQ-E2E-001` | 投影 doc_key = `req-{uuid}`（自动） |
| Knowledge 文档 | doc_key `kb-e2e-001` | doc_type=`'knowledge'` |

### 2.2 环境要求（逐条给锚点）

| # | 要求 | 本仓落实 |
|---|---|---|
| 1 | Project 已完成基础初始化 | `POST /v1/pm/projects` 201；Command Center 200 |
| 2 | Markdown 知识目录可读写 | `KnowledgeRoot::from_env()`（env 注入 root 路径）；fixture 建临时目录 + `git init` |
| 3 | Git 可提交 | root 目录为**本地 Git 仓库**（`knowledge.rs:161` 检查 `.git`）；与 SSH 远端无关（R32） |
| 4 | PG 可查元数据 | 一次性库（`aipm-teamqa-pg` :16499） |
| 5 | PageIndex 可索引/重建 | `knowledge_indexer.rs`（`rebuild_document` / `index_states`）；索引表在 PG |
| 6 | AI 与人工不同身份 | bootstrap 的 ai_pm 主体 vs User A（human）—— kind 列区分（`core.principal.kind`） |

### 2.3 Fixture 模块（映射真实路径）

```
ai-frontend/e2e/
├── p0-04-requirement-knowledge.spec.ts
└── p0/
    ├── fixtures.ts               # 复用 buildOwner / createProject
    ├── knowledge-root.ts         # 临时 Git 仓库目录 + env 注入（D17）
    ├── requirement-api.ts        # 创建/修订/提交/确认（复用 13 号 approval-api）
    ├── markdown-store.ts         # 直接读 root 下文件（权威正文）
    ├── git-version.ts            # git log / git show <sha>:<path>（权威历史）
    └── pageindex-api.ts          # reindex 路由 + index_state 直查
```

> 三种独立验证方式（规格 §十）→ 本仓落点：Playwright（页面）；Git/Markdown（`git -C <root>` + 文件读取，**绕过应用**）；PostgreSQL（psql 直查一次性库，fixture 有 psql 通道 `global-setup.ts:26-28`）。

---

## 3. P0-011 · 正式 Requirement 创建

**测试目标**：人工正式创建 Requirement 时，Markdown 内容、Git 版本、PG 元数据三者正确关联。

> **R27 关键修正**：`create` 只产生 PG draft 行；**Markdown 文件与 Git commit 在 `submit` 时诞生**（投影绑定点）。规格的 Step 3/4「检查 Markdown / Git」必须排在 submit 之后。

前置：Project 已创建；User A 为 Owner；KnowledgeRoot 已初始化；`REQ-E2E-001` 未存在。

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 人工创建 Requirement（draft） | 201 | `POST /v1/pm/projects/{pid}/requirements {code:"REQ-E2E-001", title, markdown}`（`CreateRequirementRequest`：code+title 必填） |
| 2 | 检查 Requirement ID | 生成 uuid；`code='REQ-E2E-001'` | `delivery.requirement` |
| 3 | **人工提交**（submit） | 成功；**投影文档诞生**：文件落盘 + Git commit + `ai.document_revision` 绑定 sha | `submit_revision`（唯一绑定点，`requirement.rs:696` + `requirement_projection.rs`）；幂等键 `req-submit:{revision_id}` |
| 4 | 检查 Markdown 文件 | 正式内容正确写入 | root 下 `doc_key=req-{uuid}` 对应文件；含 title / 验收条件 |
| 5 | 检查 Git | 存在对应 commit | `git log`；`git show <sha>:<path>` 含正文 |
| 6 | 检查 PG | 元数据及版本引用正确 | `ai.document_revision.git_commit_sha` 非空 = `markdown_version_ref`；`content_hash` = sha256(markdown)（Git 侧同源） |
| 7 | 检查 Requirement 状态 | 符合正式创建流程 | 修订 `draft→pending_confirmation`（submit）；人工 confirm → `approved`；`current_revision_id` 指向它 |
| 8 | 刷新页面 / 重读 Markdown | 仍在且一致 | `GET /v1/pm/documents/{document_id}` + 文件重读 |

**三存储断言（代码）**

```ts
// ① Markdown 权威（绕过应用直读文件）
const doc = await readKnowledgeMarkdown(root, `req-${reqUuid}`);
expect(doc).toContain("用户任务创建");
expect(doc).toContain("系统支持团队成员创建任务");

// ② Git 权威（绕过应用直读历史）
const commit = await gitHeadFor(root, docPath);
const atRef = await gitShow(root, commit, docPath);
expect(atRef).toContain("用户任务创建");

// ③ PG 状态权威（psql 直查）
const meta = await sqlOne(`
  SELECT r.id, r.code, r.project_id, r.current_revision_id,
         rv.git_commit_sha, rv.content_hash, rv.state
  FROM delivery.requirement r
  JOIN delivery.requirement_revision rv ON rv.id = r.current_revision_id
  WHERE r.code = 'REQ-E2E-001'`);
expect(meta.git_commit_sha).toBe(commit);            // 版本引用指向真实 Git 版本
expect(meta.content_hash).toBe(sha256(doc));         // DB 哈希 = 正文哈希（同源）
expect(meta.state).toBe("approved");
```

**特别检查（规格原文 → 本仓口径）**

- DB 保存的是**版本引用**（uuid + sha + hash），不是第二份可独立修改的权威正文 —— content jsonb 是「读投影」，损坏可由 `rebuild_projection` 从 Git 重建（R31）。
- `git_commit_sha` 必须能定位到真实 Git 版本 —— `git show` 验证。
- **不允许 DB 有记录而 Markdown 不存在** —— 反向断言：删文件属于破坏权威（属故障注入域，不在 P0 正向用例）。
- 状态符合创建审批规则 —— draft → submit（绑定）→ confirm（真人 + `expected_content_hash`）→ approved。

---

## 4. P0-012 · Requirement Revision 与并发保护

**测试目标**：修改走新 Revision（不覆盖历史）；当前版本引用同步；并发修改无静默丢失。

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 读取 Revision 1（approved） | 内容正确 | `current_revision_id` → `git_commit_sha` → `git show` |
| 2 | 人工创建修订草稿 | 新 draft 修订 | `POST .../revisions`（create_draft，`pm_requirements.rs:539-542`） |
| 3 | 提交 + 确认（带 `expected_content_hash`） | Revision 2 诞生 | submit（第二次投影绑定，新 commit）+ confirm（CAS 双令牌） |
| 4 | 检查 Git 历史 | 两个 commit 均保留 | `git log` 计数 ≥ 2 |
| 5 | 读 Revision 1 | 仍是原始内容 | `git show <sha1>:<path>` + PG 旧行 `state='superseded'` |
| 6 | 读 Revision 2 | 含修改内容 | `git show <sha2>:<path>` |
| 7 | 检查 DB 当前版本引用 | 指向 Revision 2 | `current_revision_id` 原子迁移（`confirm_revision` 注释「current_revision_id 原子迁移；row_revision+1」） |
| 8 | 检查历史版本引用 | Revision 1 仍可追溯 | 旧行在 + `read_at_ref`（按 ref 读 Git，`requirement_projection.rs:281`） |
| 9 | 刷新页面 | 最新版本正确 | `GET /v1/pm/requirements/{id}` |

**并发保护断言（规格 §四附加 → 本仓 DEF-03 回归）**

> **DEF-03 历史与现状**（`docs/s02-02-requirement-runtime-verification-v1.md:171`）：旧 `pm_write/revision.rs` REST 路径并发定稿双双 200、静默丢失更新（无 `FOR UPDATE`、无 `expected_revision`）。**现行统一入口已带三重防护**：`lock_requirement` = `SELECT ... FOR UPDATE`（`requirement.rs:270-278`）+ `row_revision` CAS 双令牌 + `expected_content_hash` 哈希门（FLOW-05 已证：不符 → 409 且终态不变）。**回归必须打统一入口**（DEF-04 教训：双入口语义分叉是缺陷本体，D19）。

| 操作 | 预期 | 锚点 |
|---|---|---|
| User A 基于 Rev1 提交并确认 | 成功 → Revision 2 | 常规链 |
| User B 仍持 Rev1 的 `content_hash` 提交确认 | **409**（陈旧哈希），终态不变 | `expected_content_hash` 不符 → `PmWriteError` 409 |
| 检查当前版本 | 仍指向 Revision 2 | `current_revision_id` |
| 检查 Git 历史 | User A 的 commit 不丢 | `git log` |

```ts
test("P0-012 并发陈旧版本更新被拒（DEF-03 回归 · 统一入口）", async () => {
  const rev1Hash = await getCurrentHash(a, reqId);

  // User A：基于 Rev1 正常定稿
  const draftA = await createDraft(a, reqId, "系统支持团队成员创建任务，并允许管理员分配任务。");
  await submitRevision(a, reqId, draftA.revision_id);
  await confirmRevision(a, reqId, draftA.revision_id, draftA.content_hash);

  // User B：仍持 Rev1 的陈旧哈希 → 409
  const stale = await confirmRevision(a, reqId, draftA.revision_id, rev1Hash);
  expect(stale.status).toBe(409);

  // 终态不变：仍指向 Revision 2，A 的 commit 在 Git 历史里
  const meta = await getRequirementMetadata(a, reqId);
  expect(meta.current_revision_id).toBe(draftA.revision_id);
  expect(await gitCommitCount(root)).toBeGreaterThanOrEqual(2);
});
```

**通过标准**：Revision 不可覆盖；历史可追溯；引用正确；并发陈旧更新被拒且无静默丢失。

---

## 5. P0-013 · AI AutoApply 禁止（知识域扩展）

**测试目标**：AI 不能直接修改正式 Requirement / Knowledge，不能代人工审批 —— 与 13 号文档 P0-010 同构，本套件**扩展到知识域 V2 提案面**并加 Git 断言。

**知识域 V2 真实面**（`routes.rs:327-345`）：

| 能力 | 路由 | 闸 |
|---|---|---|
| 提案 | `POST /v1/pm/documents/proposals` | AI 可提（建议 ≠ 正式） |
| **红线确认** | `POST /v1/pm/documents/proposals/{id}/confirm` | 人工（与 `confirm_revision` 同族真人门） |
| **否决回滚** | `POST /v1/pm/documents/revisions/{revision_id}/veto` | 人工 |
| AI 写入白名单 | `AI_WRITABLE_COMMANDS` / `AI_FORBIDDEN_COMMANDS`（`discovery_writeback.rs:57-77`） | confirm 类被禁 |

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | AI 生成修改建议 | 建议产生 | `ai.change_proposal(status='proposed')` 或修订 `pending_confirmation` |
| 2 | 检查建议状态 | Pending | 同上 |
| 3 | 检查 Markdown | 正式内容未变 | 文件内容 = Rev2 |
| 4 | 检查 Git | **无未经确认的正式修改提交** | HEAD 未变（Git 提交只发生在 submit/绑定路径，AI 无该入口） |
| 5 | 检查 PG | 当前 Revision 不变 | `current_revision_id` / `git_commit_sha` 不变 |
| 6 | **AI 尝试直接应用** | **403** | ai_pm 主体令牌调 confirm → `require_human` → 403（13 号 §5.3 同款） |
| 7 | **AI 尝试正式审批（知识域）** | **403** | ai_pm 主体令牌调 `proposals/{id}/confirm` → 拒绝 |
| 8 | 检查审计 | 拒绝操作留痕 | `governance.audit_log` |
| 9 | 人工确认建议 | 产生正式 Revision | human 令牌 + 哈希 → Rev3 |
| 10 | 检查最新版本 | 内容正确 | `git show` 新 sha |

**安全断言 6 项（规格原文 → 本仓锚点）**

| # | 断言 | 锚点 |
|---|---|---|
| 1 | AI 只能创建建议，不能直接修改正式 Markdown | AI 写入面 = `AI_WRITABLE_COMMANDS` 白名单；confirm 类在 `AI_FORBIDDEN_COMMANDS` |
| 2 | AI 不能创建正式 Revision | `submit_revision` 产生的修订只落 `pending_confirmation`（`requirement.rs:716`）；正式 = confirm（human） |
| 3 | AI 不能伪造人工用户 ID | `core.principal.kind` 判定来自 DB 行，不来自 token 声明（`require_human_conn` 查库） |
| 4 | AI 不能调用人工审批接口绕过 | ai_pm 令牌直调 confirm / proposals confirm → 403 |
| 5 | AI 不能借助其他项目权限上下文 | `require_project_visible` + 租户组合 FK（12 号 §6.3 口径） |
| 6 | 未确认建议不当作正式 Requirement | 投影绑定只在 submit；建议态无 `git_commit_sha` 绑定（draft 不投影） |

> Git 断言的独特价值：本仓「Git 提交先于 PG 事务 + 幂等账本 `req-submit:{revision_id}`」（`requirement_projection.rs` §6.7）意味着**任何正式内容变化必然留下 commit**——HEAD 不变即权威未动，这是比 UI/API 更硬的证据。

---

## 6. P0-014 · Knowledge 索引重建

**测试目标**：PageIndex 只是派生索引；删索引不伤 Markdown/Git/PG 权威；可重建、可恢复检索。

**测试对象**：知识文档 doc_key `kb-e2e-001`（doc_type=`'knowledge'`，内容 = 规格原文 4 条任务管理规则）。

| Step | 操作 | 预期结果 | 真实锚点 |
|---|---|---|---|
| 1 | 检索「任务必须有明确负责人」 | 命中 | `GET /v1/pm/documents/search?...`（FTS/CJK 通道，`retrieval/cjk.rs`；离线可用，D20） |
| 2 | 检查索引存在 | 索引行在 | `ai.document_section_index` + `ai.document_index_state` |
| 3 | 记录权威快照 | — | Git HEAD + 文件全文 + PG 元数据（document 行 / revision / sha / hash） |
| 4 | **只删派生索引** | 索引数据清空 | psql `DELETE FROM ai.document_section_index WHERE ...` + `ai.document_index_state`（一次性库内，允许 fixture 直删**派生**表；正式内容表禁碰） |
| 5 | 检查 Markdown | 仍在 | 文件逐字节比对 |
| 6 | 检查 Git | 历史仍在 | HEAD 不变 |
| 7 | 检查 PG | 元数据与版本引用不变 | document / revision 行 diff 为空 |
| 8 | 重新索引 | 成功 | `POST /v1/pm/documents/revisions/{revision_id}/reindex`（单文档 `rebuild_document`）或 `POST /v1/pm/documents/reindex`（项目级，T-060 §5.5） |
| 9 | 再次检索 | 命中原文档 | search API |
| 10 | 对比检索结果 | 定位到正确 document + 版本 | 命中项 id / revision 与快照一致 |

**附加断言（规格 §六 → 全部可落地）**

| # | 断言 | 方式 |
|---|---|---|
| 1 | 删索引不删 Markdown 文件 | 文件存在 + 内容一致 |
| 2 | 删索引不改 Git HEAD | `git rev-parse HEAD` 前后相等 |
| 3 | 删索引不改正式状态 | PG diff |
| 4 | 重建不生成重复 Knowledge | `ai.project_document` 计数不变（doc_key 唯一） |
| 5 | 重建不修改正式内容 | 文件 + Git diff 为空 |
| 6 | 重建后索引版本与权威版本一致 | `ai.document_index_state` 指向当前 revision |
| 7 | 检索定位正确 ID 与版本 | search 结果含 document_id + revision |
| 8 | **索引缺失不静默** | `knowledge_search.rs:51` `IndexStateMissing` 降级路径返回可观测 fallback 原因；`knowledge_quality.rs:30` `reindex_required` 标志置位 |

**「特别需要防止的错误实现」（规格 §六）→ 本仓已有正确实现**

| 规格红线 | 本仓事实 |
|---|---|
| 不能从 PG JSONB / PageIndex 反向恢复正式知识 | 恢复工具 = `rebuild_projection`（**Git→content**，`requirement_projection.rs` §6.5）；恢复源是 Git，不是 DB/索引 |
| 权威文件缺失必须报数据完整性故障 | 读侧走 Git（`read_at_ref`）；投影损坏走反向投影工具重建，不冒充恢复成功 |

---

## 7. 综合一致性测试（P0-04 最终验收）

以 `REQ-E2E-001` 走完整生命周期（复用前 4 案例的链路，不另起数据）：

```
1. 人工创建 + submit + confirm   → Revision 1（approved）
   Markdown 落盘 + Git commit + PG 状态/版本引用（投影绑定）
2. 人工修改 → Rev2（draft→submit→confirm）
   Rev1 保留（superseded + 旧 commit），current_revision_id → Rev2
3. AI 提修改建议 → Pending
   ai.change_proposal(proposed)；Rev2 不变；Git HEAD 不变
4. 人工确认建议 → Revision 3
   新 commit + current_revision_id → Rev3
5. 删除并重建 PageIndex → Rebuilt
   Rev3 仍是正式内容；索引恢复到 Rev3
```

**综合断言（规格 §七 → 本仓口径）**

| 检查项 | 最终预期 |
|---|---|
| Markdown | Rev1/2/3 历史完整（`git log` ≥ 3 个内容 commit；`git show` 各 sha 可读） |
| Git | 正式版本历史完整（HEAD = Rev3 的 sha） |
| PostgreSQL | `current_revision_id` = Rev3；`git_commit_sha` = HEAD；`content_hash` = sha256(Rev3 正文) |
| 审批记录 | `delivery.requirement_confirmation` 3 行（人工确认人 = User A） |
| AI 建议 | `ai.change_proposal` 有独立行及 `proposed` 状态（未混入正式） |
| PageIndex | `ai.document_index_state` 指向 Rev3 |
| 历史版本读取 | Rev1/2 经 `read_at_ref` / `git show` 仍可读 |

---

## 8. 最终验收清单（17 项）

**P0-011**
- [ ] 正式 Requirement 创建成功（draft→submit→confirm 全链，D18）
- [ ] Markdown 正式内容正确（submit 绑定后落盘）
- [ ] Git 权威版本正确（commit 存在且 `git show` 内容一致）
- [ ] PG 元数据和版本引用正确（`git_commit_sha`/`content_hash` 同源）

**P0-012**
- [ ] 新 Revision 正确创建
- [ ] 历史 Revision 不可覆盖（superseded + 旧 commit 可读）
- [ ] DB 当前版本引用正确（原子迁移）
- [ ] 并发陈旧版本更新被拒（409，统一入口，DEF-03 回归）

**P0-013**
- [ ] AI 建议不能直接修改 Markdown（Git HEAD 不变）
- [ ] AI 不能创建正式 Revision（submit 只落 pending_confirmation）
- [ ] AI 不能代替人工审批（ai_pm 令牌 → 403，REST + 知识域 confirm 双路）
- [ ] 人工确认后正式 Revision 正确产生（Rev3 + 新 commit）

**P0-014**
- [ ] 删除 PageIndex 不影响 Markdown
- [ ] 删除 PageIndex 不影响 Git 和 DB 元数据
- [ ] PageIndex 可以根据权威内容重建（reindex 路由）
- [ ] 重建后检索结果与权威版本一致（document_id + revision 定位）

**综合**
- [ ] Revision 1、2、3 历史完整（Git + PG + 确认账三面一致）

---

## 9. 待裁决项

| # | 议题 | 选项 | 建议 |
|---|---|---|---|
| **D17** | KnowledgeRoot fixture | ① 测试内建临时 Git 仓库 + env 注入 root ② 复用全局固定目录 | **①**（每套件独立 root，`git init` + 首提交；`TEAM_QA_KEEP_DB=1` 同口径保留排查） |
| **D18** | 「正式创建」定义 | 创建即断言 / 全链（submit+confirm）后断言 | **全链**（权威在 submit 绑定、生效在 confirm；draft 无 Markdown 是设计而非缺陷） |
| **D19** | 并发用例入口 | 统一入口（`requirement.rs confirm_revision`）/ 旧 `pm_write` 路径 | **统一入口**；旧路径 DEF-03/DEF-04 若仍暴露端点，另立缺陷复核（不在 P0 正向用例里喂旧入口） |
| **D20** | 检索通道范围 | FTS（CJK，离线）/ +embedding（BGE-M3） | **P0 必过 = FTS 通道**；embedding 通道依赖外部 Ollama，归 P1/专项（`docs/runbook-ollama-bge-m3.md`） |
| **D21** | fixture 直删派生索引表 | psql DELETE（仅 `*_section_index` / `*_index_state`）/ 提供“危险”测试端点 | **psql DELETE**（一次性库内允许；正式内容表 `ai.project_document` / `ai.document_revision` / Git 目录禁碰） |

---

## 10. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/13-p0-03-playwright-spec-v1.md` | AI 人工确认三道闸（§5 与本套件 P0-013 同构，REST 侧断言复用） |
| `docs/e2e-spec/12-p0-02-playwright-spec-v1.md` | 项目创建 / 隔离口径（本套件前置） |
| `docs/ai-pm-markdown-knowledge-space.md` | 知识空间设计权威（§四/§八） |
| `crates/pm-application/src/knowledge.rs` | Markdown+Git 权威实现（create_document：一文件一 commit 两行） |
| `crates/pm-application/src/requirement_projection.rs` | 需求投影协议（绑定 / 幂等 / 反向投影 / Git 先行） |
| `crates/pm-application/src/knowledge_indexer.rs` | 索引器（`rebuild_document` / `index_states`） |
| `apps/pm-api/src/documents.rs` + `routes.rs:305-345` | 知识空间 V2 REST（search / proposals / veto / reindex） |
| `docs/s02-02-requirement-runtime-verification-v1.md` | DEF-01~04 运行时缺陷档案（P0-012 并发回归背景） |
