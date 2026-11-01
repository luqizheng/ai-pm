# AI-PM 知识空间 V2：实施记录（2026-11-01）

> 设计真源：`docs/ai-pm-markdown-knowledge-space.md`
> 状态：**已实现并通过真实 PG 集成测试**；提交/推送待用户确认
> 迁移：`0014_knowledge_v2`（`0011`=D1、`0012`=TaskReady 幂等、`0013`=R5 已占用）

---

## 〇、一句话结论

知识空间从「V1 只读 + 单文档写入」升级为 **V2 信任模型**：常规修订过**质量门**即自动提交 Git；**红线类**（对外承诺 / 删除历史 / 改写已交付版本）转**人类事前确认**；发布后**否决窗口**可 Git revert + 状态回退 + 指针回退 + 索引重跟进。检索走可替换的 `KnowledgeIndexer`（A 路径自建 Markdown 树），索引哈希过期即回退直读权威文件。

---

## 一、数据模型（迁移 `0014`）

| 对象 | 变更 |
| --- | --- |
| `ai.document_revision` | 补 `source_event_id`、`analysis_run_id`、`redline_confirmed_by`、`published_at` |
| `ai.document_proposal` | 新增：候选正文、质量门结果、红线/否决状态、结果修订 |
| `ai.document_index_state` | 新增：每修订 × provider 的派生索引状态 |
| `ai.knowledge_reference` | 新增：决策对文档/章节/版本的证据引用 |

**权威边界**：正文权威仍是独立 Git 知识根（`AI_PM_KNOWLEDGE_ROOT`）。`document_proposal.proposed_markdown` 只是提案生命周期内的候选内容，提交后不构成第二权威。

---

## 二、质量门 8 项（`knowledge_quality`）

| # | Gate | 拒绝码示例 |
| --- | --- | --- |
| 1 | 重复 | `duplicate_fact`（跨文档同内容）；同文档同内容 → `duplicate` 幂等无操作 |
| 2 | 来源 | `source_missing`（无 event / analysis run / source_ref） |
| 3 | 版本 | `version_conflict`（显式基线哈希 ≠ 当前 published） |
| 4 | 冲突 | `conflict_without_rationale`（改写正式需求/决策无 change_summary） |
| 5 | 归档 | `archive_informal`（空正文或临时标记） |
| 6 | 引用 | `reference_missing`（无 `source_events:` 或文档路径） |
| 7 | 红线 | `redline_unknown`；合法红线 → `redline_pending`（不写 Git） |
| 8 | 索引 | `index_no_heading`（无可解析标题） |

结果整体序列化进 `document_proposal.quality_gate_result`，供前端与审计复读。

---

## 三、提案 / 红线 / 否决（`knowledge_proposal`）

状态机：

```
quality_gates_passed → commit_pending → committed
                    ↘ redline_pending → confirmed → committed
                    ↘ rejected
published 修订 ──veto──→ rolled_back（Git revert + 指针回退 + 索引重跟进 + 审计）
```

- **常规提案**：质量门通过 → 写 Git → 落 `document_revision(published)` → `document_proposal(committed)` → 索引状态 `pending`。
- **红线提案**：落 `redline_pending`，**不写 Git**；人类确认（`core.principal.kind='human'`）后才提交，并记录 `redline_confirmed_by`。
- **否决窗口**：仅 `published` 修订可否决；默认窗口 86400s（`AI_PM_KNOWLEDGE_VETO_WINDOW_SECS` 可覆盖）。执行 `git revert`，修订置 `rolled_back`，关联提案置 `rolled_back`，索引置 `stale`，上一修订索引置 `pending`；读路径按 `status='published'` 取最新，天然回退到上一版本。
- 全程写 `governance.audit_log`（`source_kind='knowledge_v2'`）。

---

## 四、索引层（`knowledge_indexer`）

- `KnowledgeIndexer` trait：`provider` / `index_document` / `search`，可替换为 B/C 路径（外部 PageIndex 需另行评估数据出境）。
- A 路径 `MarkdownTreeIndexer`：解析标题树，检索时校验 `content_hash`。
- `index_revision`：哈希漂移 → `stale`；解析失败 → `failed`；成功 → `indexed`。
- `search_with_fallback`：索引哈希 ≠ 当前修订哈希时标记 `stale` 并**回退直读权威文件**，返回 `used_fallback` 与 `stale_revision_ids`。

---

## 五、证据引用（`knowledge_reference`）

同一（修订、章节锚点、来源）重复写入幂等复用；按文档倒序列出，供决策记录回溯到具体章节与版本。

---

## 六、需求正文迁移（`knowledge_migrate`）

- `migrate_requirement_revision`：把 `delivery.requirement_revision.content`（JSONB）镜像为 `requirement` 文档（front matter 带 `source_events`/`source_ref`），走同一质量门与提案流。
- **兼容**：PG 原行不删不改，旧读取路径保持不变。
- **回滚**：`rollback_requirement_migration` 仅当该文档全部修订均来自迁移时删除 Git 文件 + 两表行；对 PG 零副作用。

---

## 七、接口接线

### REST（`apps/pm-api`）

| 方法 | 路径 |
| --- | --- |
| GET/POST | `/v1/pm/documents/proposals` |
| POST | `/v1/pm/documents/proposals/{id}/confirm` |
| POST | `/v1/pm/documents/revisions/{id}/veto` |
| POST | `/v1/pm/documents/revisions/{id}/reindex` |
| GET/POST | `/v1/pm/documents/{id}/references` |
| GET | `/v1/pm/documents/{id}/index-state` |
| POST | `/v1/pm/requirements/revisions/{id}/migrate` |

### MCP（`crates/pm-mcp`）

新增 `pm_propose_document`、`pm_confirm_document_proposal`、`pm_veto_document_revision`，均要求 `PM_MCP_DOCUMENT_WRITE`；`tools/list` 精确快照已同步为 7 个工具。

### 前端（`KnowledgePage.vue`）

真实后端模式下展示所选文档的提案状态与质量门失败项；具备 `PM_MCP_DOCUMENT_WRITE` 时可对 `redline_pending` 提案确认、对 `committed` 提案发起否决。mock 模式不受影响。

---

## 八、验证证据（原始命令）

| 门禁 | 结果 |
| --- | --- |
| `bash scripts/cargo-test.sh`（workspace nextest + doctest） | `FULL_TEST_EXIT=0`；`Summary: 344 tests run: 344 passed, 0 skipped` |
| `cargo clippy -p pm-application -p pm-infrastructure -p pm-mcp -p pm-api --all-targets -- -D warnings` | `CLIPPY_EXIT=0` |
| `pnpm test:unit` | `UNIT_EXIT=0`；`Test Files 88 passed`、`Tests 533 passed` |
| `pnpm typecheck` | `TYPECHECK_EXIT=0` |
| `pnpm check:i18n` | `I18N_EXIT=0`（zh/en 728 key 对齐，无新增硬编码中文） |
| `node ../scripts/check-prism-tokens.js` | `TOKENS_EXIT=0` |

V2 专项测试：`crates/pm-application/tests/knowledge_v2.rs`（10 项：自动提交、幂等重复、来源门、索引门、红线挂起/确认、非人类拒绝、否决回滚、索引回退、引用幂等、需求迁移回滚）；`crates/pm-infrastructure/tests/migration_0014.rs`（3 项）。

**测试断言演进**：`pm-mcp` 的 `tools/list` 精确快照首跑红（4 → 7 个工具），更新为新的精确列表后复跑绿——属契约同步，非弱化。
**环境修复**：`frontend/src/styles/prism/tokens.test.ts` 的围栏正则原只认 `\n`，在 CRLF 检出下失败；改为 `\r?\n` 后复跑绿（存量跨平台缺陷，非本需求实现）。

---

## 九、已知边界 / 后续

| # | 边界 | 说明 |
| --- | --- | --- |
| 1 | 红线识别 | `external_commitment` / `rewrite_delivered` 依赖调用方声明；`delete_history` 可由空正文推断。语义识别（模型判定）留后续 |
| 2 | 旧修订未标 `superseded` | 读路径按 `status='published'` 取最新，否决即回退；未引入 `superseded` 状态以避免改 `0006` CHECK |
| 3 | 索引 A 路径 | 标题树 + 关键词检索，**不是**向量/语义检索；`KnowledgeIndexer` 已可替换 |
| 4 | 需求正文双轨 | PG `requirement_revision` 仍服务旧页面；Markdown 为镜像，切换读取源需单独裁决 |
| 5 | 否决窗口 | 仅后端校验，前端未显示剩余窗口时间 |