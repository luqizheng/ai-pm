# T-060 · 知识索引层可替换化与完整检索（P6 后半 · 设计稿 v1.3）

> 状态：**已实施（2026-11-02 落地，实施记录见 §13）**
> 版本：v1.0（2026-11-02 落盘，Q1–Q4 + A-1~A-6 已裁定）→ v1.1（2026-11-02 评审修订：并发一致性、版本边界、状态不变量、故障分级、检索正确性、性能验证、K-17~K-25）→ v1.2（2026-11-02 收录专家评审 v2 原文，补当前生效修订校验、检索服务职责分离、分页语义、索引表级故障回退）→ **v1.3（2026-11-02 裁定：Project 级 Knowledge Write Coordinator + 写入幂等 / base_revision 校验 / 跨实例协调；同日实施落地）**
> 评审结论：修改后过稿；**不推翻现有架构，不扩大本期范围**。
> 一句话：把「每次检索都逐篇读 Git + 全量扫描」换成「落库章节索引 + 可替换端口 + 发布即重建 + 索引不可用时回退直读」，不引入分词扩展、不引入向量库、不改检索响应契约。
> 上位依据：`todo.md` §9 M6（T-060）、`docs/ai-pm-markdown-knowledge-space.md` §九/§十一（A 路径与可替换适配层）、`docs/ai-pm-knowledge-v2-implementation.md` §3（索引现状）、`migrations/0014_knowledge_v2.sql:60-79`、`migrations/0019_tenant.sql`（租户纪律）、`AGENTS.md` 规则 1（新项目无存量数据，直接改契约）。

---

## 0. 裁定与评审修订摘要

### 0.1 用户裁定（v1.0）

| 编号 | 问题 | 裁定 | 落地位置 |
|---|---|---|---|
| Q1 | 索引产物落在哪里 | **落 DB 派生表**（`ai.document_section_index`），`document_index_state` 保留为状态机 | §4 |
| Q2 | 增量触发点 | **写入后同步重建，事务外、失败降级**（不新增 job / 事件） | §5 |
| A-1 | veto 回滚重建范围 | **只重建生效的新修订**（被否决修订只标 `stale`） | §5.1 |
| A-2 | `create_document` 是否纳入 | **纳入**，四个写入点一条口径 | §5.1 |
| Q3 | 排序与向量 | **关键词 + 结构化加权，零扩展依赖**；不引入向量 | §6.2 |
| A-3 | 引用加成口径 | 只认**锚点存在于当前 published 修订**的 `knowledge_reference` | §6.2 |
| A-4 | 分页 | 响应体**不变**；只新增可选 `limit`/`offset`（缺省 20/0） | §6.3 |
| Q4 | 端口边界与外部依赖 | **trait 扩到「重建 + 检索」两个语义，单一 DB 实现，零外部依赖**；`MarkdownTreeIndexer` 降级为回退直读器 | §3 |
| A-5 | 状态写入 | 同步路径**不写 `pending`**，直接落 `indexed` / `stale` / `failed` | §4.2 |
| A-6 | 索引表内容 | 存章节文本字段，**不存 tsvector** | §4.1 |

### 0.2 评审修订（v1.1，逐条对应）

| # | 评审意见 | 修订 |
|---|---|---|
| 1 | 并发一致性：`indexed`/`stale`/`failed` 转换须由同一并发控制保护，锁不只保护成功 INSERT | §5.3「唯一状态转换原语」（锁内**重算前置条件**，含过期操作 `Superseded`）；K-17 |
| 2 | `index_version` 必须参与有效性判断；版本升级需全量重建入口 | §4.2 有效性三要素、§6.1 查询条件新增 `index_version`、§5.5 `POST /documents/reindex?project_id=`；K-18 / K-25 |
| 3 | 统一 `indexed`/`stale`/`failed` 与章节产物的约束；读路径漂移同样遵守 | §5.3 INV-1 / INV-2；读路径只经同一原语；K-19 |
| 4 | 故障回退：区分单篇失效与整体不可用，不得伪装成正常空结果 | §6.5 故障分级、§7 错误映射（新增 `KNOWLEDGE_INDEX_UNAVAILABLE` → 503）；K-20 |
| 5 | 检索正确性：空锚点加成保护、租户查询约束、混合分页测试 | §6.1 SQL（空锚点 + `tenant_id` 条件）、§6.5、K-21 / K-22 / K-24 |
| 6 | 性能验证：不假定毫秒级，补大文档重建耗时与章节数测试 | §5.6（删除「毫秒级」论断，改为实测回填 + 上限断言）；K-23 |
| 7 | 补 K-17~K-25 | §8 测试矩阵 |

### 0.3 专家评审 v2 处置摘要（原文见 §12）

| 编号 | 问题 | 处置 | 落点 |
|---|---|---|---|
| P0-1 | 同步重建的跨修订并发：旧修订不得覆盖当前生效修订的检索状态 | **已采纳**：锁内增加「当前生效修订」校验，非最新 published → `Superseded`；本期只索引当前生效修订 | §5.3 |
| P0-2 | 状态与产物原子性、状态表唯一键核对、`index_version` 参与判定 | **已采纳**：核对 `0014:75` 唯一键为 `(document_revision_id, provider)`，与本稿 UPSERT 目标一致；INV-1/INV-2；`index_version` 已进 §4.2/§6.1 | §4.1、§4.2、§5.3 |
| P0-3 | 版本升级的失效与全量重建机制 | **已采纳**（v1.1）：四要素有效性 + 项目级全量重建入口 | §5.5、§6.1 |
| P1-1 | 回退路径的 stale 清理不完整 | **已采纳**（v1.1）：读路径只能经 `apply_transition(Stale)`，清空产物 | §5.3、§6.4 |
| P1-2 | 索引不可用的故障边界（单篇 / 整体 / 元数据不可用） | **已采纳并细化**：新增「仅索引表不可用 → 整项目回退」一档；元数据不可用才 503 | §6.5、§7 |
| P1-3 | 分页总数语义未定义 | **已采纳**：offset 分页、不承诺 total/has_more、全局统一排序、合并后分页、重复以 DB 优先 | §6.3、§6.4、K-24 |
| P1-4 | 引用加成的空锚点 | **已采纳**（v1.1）：双向非空约束 | §6.1、§6.2 |
| P1-5 | 多租户查询边界 | **已采纳**（v1.1）：SQL 显式 `tenant_id`；`tenant_id` 由修订行带出并由组合外键强制一致 | §6.1、§4.1 |
| P1-6 | 同步重建的性能假设缺证据 | **已采纳**（v1.1）：撤回毫秒级论断；K-23 记录章节数、重建与批量写入耗时 | §5.6、K-23 |
| P1-7 | `search_with_fallback` 职责边界 | **已采纳**：端口只做 provider 重建/检索；编排归独立 `KnowledgeSearchService` | §3.2 |
| P2-1 | 不为未来 provider 预设同一物理表 | **已采纳**：`document_index_state` 为通用状态；`document_section_index` 仅属 `db-section` | §4.1、§11 |
| P2-2 | 证据契约 | **已采纳**：`SearchHit` 内部携带 `revision_id + revision_hash`，证据绑定修订而非文档 | §3.3、§11 |
| P2-3 | 索引清理生命周期 | **已采纳**：revision 删除双级联；provider 废弃按 provider 手动清理；不新增 Job | §4.4 |

### 0.4 Project 级写入协调裁定（v1.3）

**原则**：Agent 执行并行化与知识库写入并行化是两个独立问题。允许多个 Agent 并行工作，**不意味着允许它们并发修改同一 Project 知识库**。

| # | 约束 | 落地 |
|---|---|---|
| 1 | 同一 Project 可有多个 Agent，在不同任务 / Repository 上并行执行 | 与写入模型解耦（本稿不改执行器） |
| 2 | 所有写入必须经过统一 Coordinator，不得绕过修改正文或索引数据 | §5.8：四个写入点 + 手动 reindex 全部经 `KnowledgeWriteCoordinator` |
| 3 | 同一 Project 的变更串行协调，范围覆盖文档变更、Revision 发布、同步重建的完成/失败判定 | §5.8：锁覆盖「业务事务 → 提交 → `rebuild_document` 结果落定」 |
| 4 | 不同 Project 并行，不设全局写锁 | §5.8：锁键 = `project_id` |
| 5 | 保留 Revision 级重建互斥作为防御性保护 | §5.3 原语不变，锁序 `project → revision` |
| 6 | 章节与状态仍由 DB 事务保证原子性；索引失败不回滚已发布的 Git Revision | §5.3 / §5.4 不变 |
| 7 | 写入命令幂等：同一 `operation_id` 不重复产生 Revision；并发修改校验 `base_revision` | §4.5 幂等账本 + §5.8 写入流程 |
| 8 | 多实例部署时协调跨实例有效，不能仅靠进程内 Mutex | §5.8：PG **会话级 advisory lock**（数据库级互斥） |
| 9 | 不引入新 Job / Outbox / Worker，保留同步重建 | §5.7 不变 |

**归属**：Coordinator 是**Project 级应用服务**，不属于任何 Agent，也不属于某个 Executor。调用方（REST、MCP 工具、未来的执行器）都只是它的客户端；Agent 角色增加或执行器更换时，一致性边界不随之改变。

---

## 1. 现状与缺口（实测证据）

| # | 事实 | 证据 | 后果 |
|---|---|---|---|
| 1 | 检索端点走内存扫描：`list_documents` → 逐篇 `read_document`（读 Git + 校验哈希）→ 整篇解析标题树 → 子串匹配 | `apps/pm-api/src/documents.rs:57-70`、`knowledge_index.rs:157-203` | 每次查询 O(文档数 × Git 读 × 全文扫描) |
| 2 | `KnowledgeIndexer` trait 已存在，但端点没用它；`search_with_fallback` 只有测试在调 | `knowledge_indexer.rs:33-47`、`apps/pm-api/tests/documents.rs:16` | 「可替换端口」只是名义存在 |
| 3 | 重建入口是游离函数 `index_revision`，不在 trait 里 | `knowledge_indexer.rs:179-243` | 「可替换」只有检索一半 |
| 4 | `ai.document_index_state` 只有状态、无索引产物；只有 `POST /reindex` 与 veto 写状态 | `migrations/0014_knowledge_v2.sql:60-79`、`documents.rs:339-360`、`knowledge_proposal/veto.rs:210-220` | 正常发布流程下大量修订「无状态行」 |
| 5 | `RetrievedSection.revision_hash` 恒为空串 | `knowledge_indexer.rs:105` | 证据回溯拿不到版本 |
| 6 | 打分只有标题 +2 / 正文 +1，硬截断 20 条，无分页、无引用加权 | `knowledge_index.rs:176-201` | 排序不可调、无法翻页、`knowledge_reference` 白建 |
| 7 | 迁移中 `CREATE EXTENSION` 只有 `pgcrypto` | `migrations/0001_core.sql:2` | 中文 `tsvector` / 向量扩展均不可用 |

---

## 2. 目标与非目标

**链路定位**（评审建议的架构表述，本稿以此为准）：

> 知识事实源（Git）→ 索引构建协议 → 可替换索引 Provider → 统一检索服务 → 可追溯证据

检索只是这条链路的出口；本切片建立的是**协议与边界**（产物形状、转换原语、有效性判定、回退契约），而不是「把 Markdown 搜索搬进 PostgreSQL」。

**目标**：索引可重建且幂等、状态与正文哈希及版本一致、哈希/版本漂移即失效、检索接口向后兼容、外部 provider 只需实现同一端口、证据可回溯；**同一 Project 的写入与同步索引重建由跨实例有效的 Coordinator 串行协调，写入命令按 `operation_id` 幂等并按 `base_revision` 拒绝过期修改**。

**非目标**：不引入向量库、不引入分词扩展、不重写 Git 事实源、不新增后台作业/事件（归 T-061）、不做监控页、不做读时惰性重建；**不设全局写锁、不改 Agent 并行执行模型、不引入额外分布式锁服务（协调只依赖 PostgreSQL）**。

---

## 3. 架构与端口

### 3.1 分层

```mermaid
flowchart LR
  subgraph 写入
    P1["create_document（MCP）"] --> CO
    P2["finalize_committed（自动提交）"] --> CO
    P3["confirm_redline_proposal（红线确认）"] --> CO
    P4["veto_revision（回滚）"] --> CO
    CO["KnowledgeWriteCoordinator（project 级串行 · 幂等 · base_revision）"] --> RB
    RB["DbSectionIndexer::rebuild_document"] --> TR["apply_transition（唯一转换原语 + revision 级锁）"]
    TR --> T1[("ai.document_section_index")]
    TR --> T2[("ai.document_index_state")]
  end
  subgraph 读取
    Q["GET /v1/pm/documents/search"] --> S["KnowledgeSearchService（编排）"]
    S --> DB["DbSectionIndexer.search：索引检索 + 打分 + 分页"]
    S --> FB["DirectReadFallback：读 Git + parse + 同一打分函数"]
    DB --> M["合并 / 去重（DB 优先） / 排序 / 分页"]
    FB --> M
    M --> R["{ items: [...] }（形状不变）"]
  end
```

- 写入侧一律进 **`KnowledgeWriteCoordinator`**（§5.8），锁按 `project_id` 串行，覆盖到同步重建结果落定；读取侧**不取该锁**（检索是只读，回退路径的漂移清理只取 revision 级锁）。
- **端口与实现均在 `pm-application`**（与现状一致，该 crate 已依赖 sqlx/tracing）。
- `knowledge_index.rs` 退化为**纯解析 + 打分纯函数 + 回退直读扫描**。
- 回退器保留 Git 权威读路径（`knowledge::read_document`，含 `content_hash` 校验）。

### 3.2 检索编排服务（评审 P1-7）

职责分离，端口只做「provider 的重建与检索」，**不承担 fallback 编排**：

| 职责 | 归属 |
|---|---|
| 取当前 published 修订、判定索引有效性（§4.2 四要素） | `KnowledgeSearchService`（应用服务，`knowledge_search.rs`） |
| 调用 DB 索引检索（`KnowledgeIndexer::search`） | `KnowledgeSearchService` → `DbSectionIndexer` |
| 识别未索引/失效文档，调用 `DirectReadFallback`，并触发 `apply_transition(Stale)` | `KnowledgeSearchService` |
| 合并去重（DB 优先）、全局排序、offset 分页 | `KnowledgeSearchService` |
| 索引重建（`rebuild_document`）与检索（`search`）本身 | `KnowledgeIndexer` 实现（`DbSectionIndexer`） |

这样未来接 PageIndex 时，只替换 `KnowledgeIndexer` 实现，回退机制与分页语义**不随 provider 绑定**。`search_with_fallback` 作为函数名退役，其逻辑全部收进 `KnowledgeSearchService::search`。

与读取侧并列的第二个应用服务是 **`KnowledgeWriteCoordinator`**（§5.8）：前者管「怎么读」，后者管「谁能写、按什么顺序写」。两者都按 **Project** 定位，都不属于某个 Agent，也不属于某个 provider 实现。

### 3.3 端口契约

```rust
pub const DB_SECTION_PROVIDER: &str = "db-section";
pub const DB_SECTION_INDEX_VERSION: &str = "v1";

#[derive(Debug, Clone, Copy)]
pub struct SearchPage { pub limit: i64, pub offset: i64 } // 缺省 20 / 0，limit ∈ 1..=100

#[derive(Debug, Clone, Serialize)]
pub enum IndexOutcome {
    Indexed { sections: i64 },
    Stale,                          // 磁盘正文哈希 ≠ 修订哈希
    Failed { message: String },     // 文件缺失 / 无标题 / 解析失败
    Superseded,                     // 锁内前置条件已变化，本次操作无任何写入
}

#[async_trait]
pub trait KnowledgeIndexer: Send + Sync {
    fn provider(&self) -> &'static str;
    fn index_version(&self) -> &'static str;
    async fn rebuild_document(&self, project_id: Uuid, revision_id: Uuid)
        -> Result<IndexOutcome, KnowledgeError>;
    async fn search(&self, project_id: Uuid, query: &str, page: SearchPage)
        -> Result<Vec<SearchHit>, KnowledgeError>;
}
```

- `search` 在**权威元数据或索引基础设施不可用**时必须返回 `Err`（新增 `KnowledgeError::IndexUnavailable`），**不得返回空 Vec**（见 §6.5）。
- 端口**不编排**：取当前修订、有效性判定、回退、合并、排序、分页均由 `KnowledgeSearchService` 负责（§3.2）。
- 删除的旧契约（`AGENTS.md` 规则 1，不留过渡层）：`IndexReference`、`RetrievedSection`、`index_document`、`index_revision`、`MARKDOWN_TREE_PROVIDER` / `MARKDOWN_TREE_INDEX_VERSION`。
- `SearchHit` 增 `revision_id: Uuid` 与 `revision_hash: String`，均 `#[serde(skip_serializing)]`——证据必须绑定**具体修订**而非文档（评审 P2-2），对 REST 响应**不可见**（守 A-4）。

---

## 4. 数据模型（迁移 `0024_knowledge_section_index.sql`）

> 编号说明：`0023` 已被 T-032 的未提交文件占用，本迁移取 `0024`。

### 4.1 DDL 草案

```sql
CREATE TABLE IF NOT EXISTS ai.document_section_index (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id            UUID NOT NULL,
    project_id           UUID NOT NULL,
    document_revision_id UUID NOT NULL,
    provider             TEXT NOT NULL,
    section_no           INT  NOT NULL,
    level                SMALLINT NOT NULL,
    title                TEXT NOT NULL,
    anchor               TEXT NOT NULL,
    content              TEXT NOT NULL,
    start_line           INT  NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_revision_id, provider, section_no)
);

CREATE INDEX IF NOT EXISTS idx_document_section_project
    ON ai.document_section_index (tenant_id, project_id, provider);
CREATE INDEX IF NOT EXISTS idx_document_section_revision
    ON ai.document_section_index (document_revision_id, provider);

ALTER TABLE ai.document_section_index
    ADD CONSTRAINT fk_document_section_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.document_section_index
    ADD CONSTRAINT fk_document_section_tenant_revision
        FOREIGN KEY (tenant_id, document_revision_id)
        REFERENCES ai.document_revision (tenant_id, id) ON DELETE CASCADE;
```

要点（A-6）：
- 只存章节文本，不存 `tsvector`、不存 embedding；`content` 与 `parse_markdown_tree` 输出完全一致（保证索引与回退同源）。
- `section_no` = 解析顺序（含 level 0 的 `preamble`），重建时连续重排。
- 租户纪律按 `0019`：`tenant_id NOT NULL` + 两条 `(tenant_id, *)` 组合外键（`uq_document_revision_tenant_id` 已存在）。
- `provider` 为维度列；`(document_revision_id, provider)` 唯一键在 `document_index_state` 上已存在。
- `tenant_schema.rs` 的 `TENANT_TABLES` 由 44 增至 **46**（本表 + §4.5 幂等账本）。
- **状态表唯一键核对（评审 P0-2）**：`ai.document_index_state` 的既有约束为 `UNIQUE (document_revision_id, provider)`
  （`0014:75`），本稿 UPSERT 的冲突目标与此**一致**；迁移 `0019` 已为其加 `tenant_id` 与 `(tenant_id, document_revision_id)`
  组合外键（`ON DELETE CASCADE`），无需改动。
- **`tenant_id` 的来源（评审 P1-5）**：章节行的 `tenant_id` 由修订行带出（不单独取参），并由
  `fk_document_section_tenant_revision (tenant_id, document_revision_id)` 强制与修订租户一致——数据库层保证
  「章节租户 = 修订租户」，不依赖应用层填充正确。
- **物理模型边界（评审 P2-1）**：`document_index_state` 是 **provider 通用状态表**；`document_section_index` 只属
  `db-section` 这一实现。未来 PageIndex（无 `section_no` / `level` / `start_line` 等字段）**自带派生表**，
  可替换的是端口契约，不是物理数据模型。

### 4.2 有效性判定（评审项 2）

一条章节行「可用于检索」当且仅当同时满足：

| 条件 | 字段 |
|---|---|
| provider 匹配 | `s.provider = st.provider = 当前 provider` |
| 状态有效 | `st.status = 'indexed'` |
| 版本匹配 | `st.index_version = 当前 index_version` |
| 哈希匹配 | `st.indexed_content_hash = revision.content_hash` |

任一不匹配 → 该修订**不可用索引**，进入回退路径（§6.5）。**不允许**「版本旧但哈希对」仍用旧产物。

### 4.3 状态机

| status | 含义 | 章节产物 | 写入点 |
|---|---|---|---|
| `indexed` | 产物齐全、哈希与版本均匹配 | 必须 > 0 行 | 重建成功（唯一原语） |
| `stale` | 磁盘正文与修订哈希不一致 | **必须为 0 行** | 重建遇漂移；读路径发现漂移（同一原语） |
| `failed` | 文件缺失 / 无标题 / 解析失败 | **必须为 0 行** | 重建失败（唯一原语） |
| `pending` | 同步路径**不再写入**（CHECK 保留，不删枚举） | — | 无 |

### 4.4 索引清理生命周期（评审 P2-3）

| 事件 | 行为 | 实现 |
|---|---|---|
| 修订删除 | 章节行与状态行**双双级联删除** | 两条 `(tenant_id, document_revision_id)` 组合外键均 `ON DELETE CASCADE`（状态表见 `0019:360-363`，章节表见本迁移） |
| 文档删除 | 沿用 `ai.document_revision` 既有外键行为，本期不改 | 不新增逻辑 |
| 修订被否决（veto） | 状态 `stale` + 章节清空（§5.1 #4） | `apply_transition(Stale)` |
| provider 废弃 | 按 `provider` 一次性手动清理（`DELETE … WHERE provider = $1`） | 不新增 Job、不做定期清理任务 |
| 历史修订 | **允许保留历史派生数据**（级联/人工清理），但**不参与检索**（§6.1 只取最新 published） | — |

### 4.5 写入幂等账本（`ai.knowledge_write_operation`）

写入命令的 `operation_id` 去重记录，同表写入业务事务内：

```sql
CREATE TABLE IF NOT EXISTS ai.knowledge_write_operation (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    UUID NOT NULL,
    project_id   UUID NOT NULL,
    operation_id TEXT NOT NULL,
    command      TEXT NOT NULL CHECK (
        command IN ('create_document', 'submit_proposal', 'confirm_redline', 'veto_revision')
    ),
    -- 本次操作产出的提案 / 修订（veto 可能两者皆空，用 NULL）
    proposal_id  UUID,
    revision_id  UUID,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, operation_id)
);

CREATE INDEX IF NOT EXISTS idx_knowledge_write_op_revision
    ON ai.knowledge_write_operation (revision_id)
    WHERE revision_id IS NOT NULL;

ALTER TABLE ai.knowledge_write_operation
    ADD CONSTRAINT fk_knowledge_write_op_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
```

- **不是 outbox、不是 job**：没有消费者、没有事件、没有后台扫描；只用于「同一 `operation_id` 重放时返回既有结果」。
- 插入与业务写入**同一事务**：唯一键冲突即代表重放，回滚本事务并读取既有行返回（不二次发布）。
- 生命周期：随 `core.project` 存在；不做定期清理（人工按 `created_at` 清理即可，同 §4.4 口径）。

---

## 5. 重建语义与触发点

### 5.1 四个触发点（Q2 / A-1 / A-2）

| # | 位置 | 现状 | 改动 |
|---|---|---|---|
| 1 | `knowledge::create_document`（`knowledge.rs:264-336`） | 返回 `CreatedDocument` 无 `revision_id` | `INSERT ... RETURNING id`；`tx.commit()` 后调 `rebuild_document` |
| 2 | `submit_proposal` 自动提交（`submit.rs:152-178`） | 经 `finalize_committed` 写库 | 删除 `document_index_state(pending)` 插入；调用方取 `revision_id` 后重建 |
| 3 | `confirm_redline_proposal`（`submit.rs:182-251`） | 同上 | 同上 |
| 4 | `veto_revision`（`veto.rs:5-121`） | tx 内：被否决修订标 stale，上一修订插 `pending` | tx 内仅改业务状态；提交后：被否决修订 `apply_transition(Stale)`（清空产物），生效修订 `rebuild_document`（A-1） |

`knowledge_migrate` 走 `submit_proposal`，自动获得索引。

**四个写入点全部经 `KnowledgeWriteCoordinator`（§5.8）**：取 project 锁 → 幂等预占 → `base_revision` 校验 → 业务事务提交（含 §4.5 账本行）→ 同步 `rebuild_document` → 释放锁。

### 5.2 重建流程（`rebuild_document`）

1. 按 `project_id + revision_id` 查 `canonical_path` / `content_hash` / `tenant_id` / `status` → 缺失 `NotFound`；非 `published` → `Conflict`（手动 reindex 同受此约束）。
2. 读文件（`safe_join`）、算哈希、解析标题树，构造 `Transition`（`Indexed{sections}` / `Stale` / `Failed`）。
3. 交给 `apply_transition`（§5.3）：锁内**重算**前置条件（可能降级为 `Stale`/`Failed`，或返回 `Superseded`）；`rebuild_document` 自身**不写**状态与产物。

### 5.3 唯一状态转换原语（评审项 1、3）

```rust
enum Transition {
    Indexed { sections: Vec<SectionRow> }, // 章节由解析结果给出
    Stale { reason: String },
    Failed { message: String },
}

/// 唯一写入索引产物与状态的入口；任何路径（发布 / 手动 reindex / 读路径漂移 / veto）都只能调它。
async fn apply_transition(
    pool: &PgPool, tenant_id: Uuid, project_id: Uuid,
    revision_id: Uuid, provider: &str, t: Transition,
) -> Result<IndexOutcome, KnowledgeError>
```

单事务内步骤：

1. **取得锁**：`SELECT pg_advisory_xact_lock(hashtextextended('ai.document_section_index:' || $revision_id || ':' || $provider, 0))`（与 `task_dependency.rs:97` 同一手法）。锁粒度 = `(revision, provider)`，成功重建、`stale` 清理、`failed` 清理**共用同一把锁**。
2. **锁内重算前置条件**（关键：不信任锁外观测，避免「旧操作删掉新产物」）：
   - 重读修订行：仍为 `published`、属于 `project_id` / `tenant_id`；
   - **当前生效修订校验（评审 P0-1）**：按 `document_id` 取 `ORDER BY revision_no DESC LIMIT 1` 的最新 published 修订；
     若它不是本次 `revision_id` → **`Superseded`**（历史修订不重建、不写状态、不参与检索）。本期只索引当前生效修订。
   - 重读磁盘文件并计算哈希 `h_disk`；
   - 若 `t = Indexed` 且 `h_disk ≠ revision.content_hash` → **就地降级**为 `Stale`，继续执行清理；
   - 若 `t = Indexed` 且解析无标题 → **就地降级**为 `Failed`；
   - 若 `t = Stale` 且 `h_disk = revision.content_hash`（别人刚修好）→ **`Superseded`**：不写状态、不删产物，直接返回；
   - 若 `t = Failed` 且当前状态已是 `indexed` 且哈希/版本匹配 → `Superseded`。
3. **按最终 transition 执行（同一事务）**：
   - `Indexed`：`DELETE` 该 `(revision, provider)` 旧产物 → 批量 `INSERT` 新产物 → `UPSERT state(indexed, indexed_content_hash = revision.content_hash, index_version = 当前版本, indexed_at = now(), last_error = NULL)`。
   - `Stale` / `Failed`：`DELETE` 该 `(revision, provider)` **全部**产物 → `UPSERT state(status = stale|failed, indexed_content_hash = NULL, last_error = reason)`。
4. 提交后返回 `IndexOutcome`；`tracing` 记录。

**不变量（K-19 全路径断言）**：
- **INV-1**：`status ≠ 'indexed'` ⇒ 该 `(revision, provider)` 章节行数 = 0。
- **INV-2**：`status = 'indexed'` ⇒ `indexed_content_hash = revision.content_hash` ∧ `index_version = 当前版本` ∧ 章节行数 > 0。
- 读路径发现哈希漂移时**必须**走 `apply_transition(Stale)`（清空产物），**禁止**直接 `UPDATE status`。
- 本表不引入触发器：不变量由「唯一原语 + 锁内重算」保证，并由 K-17/K-19 在全部路径验证（含并发交错）。

### 5.4 失败策略

- 四个调用点统一调 `rebuild_quietly(...)`：`Indexed` → `info!(knowledge_index_rebuilt)`；`Stale` / `Superseded` → `warn`/`debug`；`Failed` / DB 错误 → `warn!(knowledge_index_failed)`。
- **发布成功与否只由权威写入决定**：索引失败不阻断、不回滚、不重试；状态留痕，检索自动回退。
- 幂等：同一修订重复执行结果一致（删后插 + UPSERT + 锁内前置条件重算）。

### 5.5 版本升级与全量重建入口（评审项 2）

- 单修订：保留 `POST /v1/pm/documents/revisions/{revision_id}/reindex`（只允许 `published`）。
- **项目级全量**：新增 `POST /v1/pm/documents/reindex?project_id=` → 顺序遍历该项目**全部 published 修订**，逐个 `rebuild_document`；响应 `{ reindexed, stale, failed, superseded }`（只返回计数，不返回逐条明细）。
- `index_version` 上调（如 `v1` → `v2`）后的**既定动作**：改常量 → 全部行自动失效（§4.2）→ 调项目级全量重建 → 恢复直查。不做自动后台触发，不做双版本并存（`AGENTS.md` 规则 1）。
- 项目级入口带 `project_in_tenant` 边界校验，与其余端点一致。
- 两个 reindex 入口都**写索引数据**，因此同样经 `KnowledgeWriteCoordinator` 取 project 锁（§5.8）；项目级重建会长时间持锁，按项目逐个执行，不并发。

### 5.6 性能验证（评审项 6）

- 本稿**撤回 v1.0 中「毫秒级」的论断**：重建成本 = 解析 + 章节行插入 + 锁等待，随章节数线性增长，具体值必须实测。
- 验证方式（K-23）：构造 ≥ 200 个标题、≥ 200 KB 的 Markdown，在测试库执行一次重建，断言（a）章节行数与标题数一致、（b）`indexed` 状态与哈希一致、（c）耗时不超过 **10 s**（防病态退化的宽松上限，不是 SLA）；耗时同时 `info!` 输出并在实施后回填本节实测值。
- 回归保护：该用例随 `knowledge_index_v2.rs` 常态运行（不 `#[ignore]`），保证任何让重建退化为秒级以上的改动都会被看见。
- **实测值（2026-11-02，K-23）**：**221 章节 / 268,303 字节（262 KB）→ 重建 84 ms**（含解析 + 221 行落库 + 状态 upsert，本机 `#[sqlx::test]` 临时库）。结论：同步重建对发布请求不可见（< 0.1 s），§5.7「不改异步」成立；10 s 上限留作病态退化守卫。实测由用例内 `println!("K23_PERF ...")` 输出，可复跑取数。

### 5.7 为什么不做后台 job（被否方案记录）

`integration.job` 与 `OutboxWorker` 可用，但新增作业类型意味着（a）新建 worker 循环、（b）处理「已发布未索引」窗口、（c）与 T-061 事件语义重叠。发布点唯一的额外 IO 是刚写完的 Git 文件，失败已降级处理；按 YAGNI 否决。**若 K-23 实测显示发布请求不可接受**，再单独开切片改为异步——不在本稿内预留开关。

### 5.8 Project 级写入协调（`KnowledgeWriteCoordinator`，v1.3）

#### 5.8.1 执行模型

- **Agent 并行 ≠ 写入并行**：同一 Project 可有多个 Agent 在不同任务 / Repository 上并行执行；但对同一 Project 知识库的写入**串行**。
- 协调范围（写操作全集）：`create_document`、提案自动提交 / 红线确认、`veto_revision`、单修订 reindex、项目级全量 reindex。**任何绕过 Coordinator 直接改正文或索引数据的路径都不允许。**
- 读**不参与**协调：`GET /search`、`GET /documents/{id}` 只读；回退路径的漂移清理只走 revision 级原语（§5.3），不取 project 锁。
- 锁粒度 = `project_id`：不同 Project 完全并行，**无全局写锁**。

#### 5.8.2 锁实现（跨实例有效）

```rust
/// 持有期间独占该 Project 的写入权；连接不归还池，避免锁残留。
pub struct ProjectWriteGuard { conn: Option<sqlx::PgConnection> }

pub async fn acquire(
    pool: &PgPool, project_id: Uuid, timeout: Duration,
) -> Result<ProjectWriteGuard, KnowledgeError>;
```

| 项 | 设计 |
|---|---|
| 锁类型 | **会话级** `pg_try_advisory_lock(hashtextextended('ai.project_knowledge_write:' || $project_id, 0))`（不用进程内 Mutex，也不引入外部锁服务） |
| 为何不用事务级 | 协调范围跨「业务事务提交 → 同步重建结果落定」两个事务；`pg_advisory_xact_lock` 在 commit 时即释放，覆盖不到重建判定 |
| 获取 | 循环 `try` + 短退避，总等待 ≤ `KNOWLEDGE_WRITE_LOCK_TIMEOUT_MS`（默认 10 s）；超时 → `KnowledgeError::ProjectWriteBusy` → **409 `KNOWLEDGE_PROJECT_BUSY`** |
| 释放 | 显式 `pg_advisory_unlock` 后 `detach()`（连接关闭、不归还池）；异常 / panic 路径同样 `detach()`，会话结束由 PG 释放，**不留死锁** |
| 跨实例 | 锁在 PostgreSQL 内，多实例天然共享；实例崩溃时连接断开即自动释放 |
| 锁序 | 固定 `project → revision`（§5.3 的 revision 级锁在 project 锁内获取），任何路径不得反序，避免死锁 |

#### 5.8.3 一次写入的五个步骤

1. **取锁**：`acquire(project_id, timeout)`；失败 409（可重试，不是数据冲突）。
2. **`base_revision` 校验**（锁内读当前 head = 该文档最新 published 修订）：
   - 请求携带 `base_revision_id`（新建文档要求 `NULL`）；
   - 与当前 head 不一致 → **409 `KNOWLEDGE_REVISION_CONFLICT`**，不产生 Revision；
   - 取代原 `base_content_hash`（`AGENTS.md` 规则 1：不留两套基准字段）；
   - `veto_revision` 不按 head 比较（它作用于显式指定的 revision），但仍需 project 锁与幂等账本。
3. **幂等预占**（在业务事务内、**先于**正文写 Git）：`INSERT INTO ai.knowledge_write_operation (...) ON CONFLICT (project_id, operation_id) DO NOTHING RETURNING id`。
   - 无返回行 ⇒ 重放：读取既有行，按其 `proposal_id` / `revision_id` 返回既有结果，**不再写 Git、不再重建**；若既有行已提交但重建结果未落定，则补跑一次幂等重建（§5.3 保证重复执行无副作用）。
   - 有返回行 ⇒ 继续执行；若事务失败（含 Git 写入失败），账本行随事务回滚，重试可重新执行，**不会产生第二条 Revision**（Git 同内容重写哈希不变）。
4. **业务事务提交**：正文写 Git + revision 落库 + 回填账本 `revision_id` + 状态业务字段，一次提交。
5. **同步重建 → 释放锁**：`rebuild_document` 结果落定（`Indexed` / `Stale` / `Failed` / `Superseded`）后释放 project 锁；重建失败只记录，不阻断已提交的发布（§5.4）。

#### 5.8.4 与 revision 级锁的关系

- project 锁**消除同 Project 内的跨修订竞争**（P0-1 的两条重建交错在协调后不会发生）。
- revision 级锁**保留为防御层**：读路径的 `apply_transition(Stale)`、手动 reindex、未来若出现未走 Coordinator 的内部调用，仍由 §5.3 的锁内前置条件重算兜底（`Superseded`）。
- 因此本稿**同时保留**「project 串行」与「revision 互斥」，二者不是替代关系。

#### 5.8.5 明确的边界

- 不引入新的 Job / Outbox / Worker / 事件：协调是**请求内**的排队，没有后台补偿。
- 不改 Agent 执行模型：Agent 仍可并行跑任务，只是写知识库时必须排队。
- 协调服务是 **Project 级、与 Agent / Executor 无关**的组件，放在 `pm-application`。

---

## 6. 检索、排序与回退（Q3 / A-3 / A-4）

### 6.1 主路径：单条 SQL

```sql
-- $1 project_id, $2 归一化 needle(lower), $3 provider, $4 limit, $5 offset,
-- $6 当前 index_version, $7 tenant_id
WITH latest AS (
  SELECT d.id AS document_id, d.doc_key, d.doc_type, d.canonical_path,
         d.tenant_id, r.id AS revision_id, r.content_hash
    FROM ai.project_document d
    JOIN LATERAL (
      SELECT id, content_hash FROM ai.document_revision
       WHERE document_id = d.id AND status = 'published'
       ORDER BY revision_no DESC LIMIT 1
    ) r ON true
   WHERE d.project_id = $1 AND d.tenant_id = $7          -- 租户显式约束（评审项 5）
), base AS (
  SELECT l.*, s.section_no, s.level, s.title, s.anchor, s.content, s.start_line,
         (CASE WHEN lower(s.title) = $2 THEN 6
               WHEN strpos(lower(s.title), $2) > 0 THEN 3
               WHEN strpos(lower(s.anchor), $2) > 0 THEN 2
               ELSE 0 END)
       + LEAST(3, (length(lower(s.content)) - length(replace(lower(s.content), $2, '')))
               / greatest(length($2), 1)) AS raw
    FROM ai.document_section_index s
    JOIN latest l ON l.revision_id = s.document_revision_id
    JOIN ai.document_index_state st
      ON st.document_revision_id = l.revision_id
     AND st.provider = $3
     AND st.status = 'indexed'
     AND st.index_version = $6                            -- 版本参与有效性（评审项 2）
     AND st.indexed_content_hash = l.content_hash         -- 哈希一致
   WHERE s.provider = $3
), scored AS (
  SELECT base.*,
         ((raw * CASE doc_type WHEN 'architecture' THEN 12
                             WHEN 'decision'     THEN 12
                             WHEN 'requirement'  THEN 11
                             ELSE 10 END) + 5) / 10 AS score,
         CASE WHEN EXISTS (
           SELECT 1 FROM ai.knowledge_reference kr
            WHERE kr.document_revision_id = base.revision_id
              AND base.anchor <> ''                       -- 空锚点不参与加成（评审项 5）
              AND kr.section_anchor IS NOT NULL
              AND kr.section_anchor <> ''                 -- 空引用锚点不参与加成
              AND kr.section_anchor = base.anchor
         ) THEN 2 ELSE 0 END AS ref_boost
    FROM base
   WHERE raw > 0
)
SELECT *, score + ref_boost AS total FROM scored
 ORDER BY total DESC, canonical_path, start_line, section_no
 LIMIT $4 OFFSET $5;
```

### 6.2 打分口径

| 项 | 分值 |
|---|---|
| 标题全等（大小写不敏感） | 6 |
| 标题包含 | 3 |
| 锚点包含 | 2 |
| 正文出现次数 | `min(3, count)` |
| 文档类型权重 | `architecture`/`decision` ×1.2、`requirement` ×1.1、`knowledge` ×1.0（整数落为 `(base*w+5)/10`） |
| 引用加成 | +2；仅当 `document_revision_id` = 当前 published 修订、`section_anchor` 与章节 `anchor` 均**非空**且相等（A-3 + 评审项 5） |

- 同分次序：`canonical_path` → `start_line` → `section_no`。
- 回退路径用 `knowledge_index::score_section(...)` 纯函数，公式逐项对应（含空锚点保护）；K-14 钉住两路径等价。

### 6.3 分页与响应（A-4 / 评审 P1-3）

- `SearchQuery` 增 `limit`/`offset`（缺省 20 / 0；`limit` 夹在 `1..=100`，`offset` 负数按 0）。
- **本期只做 offset 分页，语义明确为**：
  - **不返回 `total`，不返回 `has_more`**（响应结构不扩展）；
  - **排序全局唯一**（§6.2 的 `score desc → canonical_path → start_line → section_no`），与数据来自 DB 还是回退无关；
  - **先合并去重、再排序、最后分页**（不各自分页后再拼，避免页边界错位）；
  - 不承诺跨页快照稳定（两次请求之间若有新发布，结果可能移动；这是可接受的）。
- 两路候选各取 `limit + offset` 条，足以覆盖合并后的前 `limit + offset` 条（§6.4 第 3～5 步）。
- 响应仍是 `{ "items": [ SearchHit… ] }`，字段集合与改造前**完全一致**；`revision_hash` 不进 JSON。
- 前端 `frontend/src/api/documents.ts` 无需改动（不传新参数即保持旧行为）。

### 6.4 回退合并（`KnowledgeSearchService::search`）

1. 取每篇文档最新 published 修订 + 该 provider 状态（一次查询，带 `tenant_id`）。
2. 分类：满足 §4.2 四要素 → DB 路径；否则 → 回退候选。
3. DB 路径：跑 §6.1（取 `limit + offset` 条）。
4. 回退候选：逐篇 `read_document`（哈希校验失败 → 该篇作废并 `apply_transition(Stale)`）→ `parse_markdown_tree` → `score_section`，取前 `limit + offset` 条。
5. 合并按 `(document_id, anchor)` 去重（**同键以 DB 行为准**）→ 统一排序 → 跳过 `offset` → 取 `limit`。
6. 返回 `FallbackSearchResult { used_fallback, stale_revision_ids, hits }`；handler 只序列化 `hits`。
7. **去重键与分页边界（评审 P1-3 边界用例，K-24）**：同一章节同时出现在 DB 与回退时只保留 DB 那条，且不产生重复行或跨页缺口。

### 6.5 故障分级（评审项 4 / 评审 P1-2）

故障按**影响范围**分三档处理，不允许「部分成功」冒充「完整结果」：

| 故障 | 判定 | 行为 |
|---|---|---|
| **单篇索引不可用**（`stale` / `failed` / 无状态 / 版本不符 / 哈希不符） | 该修订 | 仅该篇走回退直读；其余篇正常出结果；`used_fallback = true`（内部可见） |
| **单篇回退也失败**（文件缺失 / IO 错误） | 该篇 | 该篇缺席，`warn!` 记录；请求整体仍 `200` |
| **索引层整体不可用**（章节索引表不可读、索引 SQL 执行失败、`provider` 初始化失败）但**权威清单可读** | 全局 | **整项目**回退直读（所有文档走 `read_document` + `parse_markdown_tree`），`warn!(knowledge_search_fallback, infra=true, reason)`；结果仍 `200`，不返回任何 DB 索引残留 |
| **权威元数据不可用**（`project_document` / `document_revision` 查询失败、连接池不可用） | 全局 | 返回 `Err(KnowledgeError::IndexUnavailable)` → **HTTP 503** `KNOWLEDGE_INDEX_UNAVAILABLE`；**绝不**返回 `200 { items: [] }` |
| **Git 根不可读**（`KnowledgeRoot` 缺失 / 权限错误） | 全局 | 同上 `503`（此时既无索引也无直读，无法产出可信结果） |
| **确无匹配** | — | `200 { items: [] }`（唯一允许空结果的场景） |

判定原则（评审 P1-2 四条）：
1. 索引查询失败时**不返回部分成功结果**冒充完整结果（要么整项目回退，要么明确报错）。
2. 只要权威文档清单与修订元数据可读，就**整体回退直读**，保持可用性。
3. 权威元数据也不可用时，返回**明确错误**而非空列表（空结果与检索失败是两种业务含义）。
4. 每次回退都 `tracing` 记录**原因**（`reason` 字段：`index_state_missing` / `index_stale` / `index_failed` / `index_version_mismatch` / `index_infra_error`）。

---

## 7. 错误处理与可观测性

| 场景 | 错误 | HTTP |
|---|---|---|
| 文件不存在 / IO 失败（重建） | `IndexOutcome::Failed` → 状态 `failed` | 发布不受影响；手动 reindex 返回 500 `KNOWLEDGE_INTERNAL` |
| 磁盘哈希 ≠ 修订哈希（重建） | `IndexOutcome::Stale` → 状态 `stale`，产物清空 | 手动 reindex → 409 `KNOWLEDGE_DOCUMENT_STALE`（沿用既有映射） |
| 无 Markdown 标题 | `IndexOutcome::Failed` | reindex → 409 `KNOWLEDGE_CONFLICT` |
| 锁内前置条件已变化 | `IndexOutcome::Superseded` | reindex → 409（提示「状态已变化，请重试」） |
| 手动 reindex 指向非 published 修订 | `KnowledgeError::Conflict` | 409 |
| 同一修订并发转换 | advisory lock 串行化；`UNIQUE(document_revision_id, provider, section_no)` 兜底 | — |
| Project 写锁等待超时（§5.8） | `KnowledgeError::ProjectWriteBusy` | **409** `KNOWLEDGE_PROJECT_BUSY`（排队超时，可重试） |
| `base_revision` 与当前 head 不一致（§5.8） | `KnowledgeError::RevisionConflict` | **409** `KNOWLEDGE_REVISION_CONFLICT`（不产生 Revision） |
| 同一 `operation_id` 重复提交（§4.5） | 重放既有结果（内部 `Replayed` 标记） | **200**，响应与首次一致，不新增字段 |
| 索引层整体不可用（章节/状态查询失败），但权威清单可读 | 不报错 | **200**，整项目回退直读（§6.5）；`warn!` 带 `reason=index_infra_error` |
| 权威元数据 / 连接池不可用 | `KnowledgeError::IndexUnavailable` | **503** `KNOWLEDGE_INDEX_UNAVAILABLE` |
| Git 根不可读 | `KnowledgeError::IndexUnavailable` | **503** `KNOWLEDGE_INDEX_UNAVAILABLE` |
| 跨租户 | 组合外键拒绝；所有查询显式带 `tenant_id` | 404 / 403 由 `project_in_tenant` 处理 |

日志（本期只做日志；指标/监控页归 T-061）：
- `info!` `knowledge_index_rebuilt`（provider / revision_id / sections / elapsed_ms）
- `warn!` `knowledge_index_failed`（provider / revision_id / outcome / error）
- `warn!` `knowledge_index_superseded`（provider / revision_id）
- `info!` `knowledge_search_fallback`（project_id / stale_count / infra=false）
- `error!` `knowledge_search_unavailable`（project_id / error）
- `info!` `knowledge_write_locked`（project_id / wait_ms）
- `warn!` `knowledge_write_busy`（project_id / wait_ms）
- `info!` `knowledge_write_replayed`（project_id / operation_id / revision_id）

---

## 8. 测试矩阵

> 评审项 7：实现可合并测试，但每条不变量必须有断言。

| ID | 用例 | 断言 | 文件 |
|---|---|---|---|
| K-01 | 解析纯函数回归 | 现有 `parse_markdown_tree` 5 例全绿 | `knowledge_index.rs`（既有） |
| K-02 | 重建落库 + 幂等 | 章节行数 = 标题数（含 preamble）；连跑两次行数不变、`section_no` 连续；`indexed` 且哈希/版本一致 | `crates/pm-application/tests/knowledge_index_v2.rs` |
| K-03 | 哈希漂移即失效 | 改盘上文件 → 重建 → `stale` 且章节清零；检索不出旧内容 | 同上 |
| K-04 | 发布即索引：`create_document` | 返回后章节与 `indexed` 状态就位 | 同上 |
| K-05 | 发布即索引：`submit_proposal` 自动提交 | 提交后立即可检索 | 同上 |
| K-06 | 发布即索引：红线确认 | 确认后索引就位 | 同上 |
| K-07 | veto 回滚 | 被否决修订 `stale` 且章节删除；生效修订被重建；检索只出回滚后正文 | 同上 |
| K-08 | 打分次序 | 标题全等 > 标题包含 > 锚点 > 仅正文；同分按 path/line | 同上 |
| K-09 | 类型权重 | 同样命中下 `decision` 排在 `knowledge` 前 | 同上 |
| K-10 | 引用加成 | 当前修订被引用章节 +2；历史修订引用不加成 | 同上 |
| K-11 | 分页 | `limit=5&offset=5` 返回第 6–10 条；缺省 20；`limit>100` 夹紧 | 同上 |
| K-12 | 响应兼容 | REST JSON 字段集合与改造前一致（无 `revision_hash`） | `apps/pm-api/tests/documents.rs` |
| K-13 | 回退语义 | 状态缺失/`failed` 时回退直读仍能命中；`used_fallback` 只在内部 | `knowledge_index_v2.rs` |
| K-14 | 两路径打分一致 | 同一文档「索引态」与「强制回退态」排名与分值相同 | 同上 |
| K-15 | 租户结构不变量 | 新表进 `TENANT_TABLES`（**46** 张：章节索引 + 幂等账本），`tenant_id NOT NULL` + 组合外键齐备 | `crates/pm-infrastructure/tests/tenant_schema.rs` |
| K-16 | 迁移可重复 | `0024` 空库应用成功，表/索引/约束齐备 | `crates/pm-infrastructure/tests/migration_0024.rs` |
| **K-17** | **并发一致性（评审 1）** | 对同一 `(revision, provider)` 并发执行「重建」与「漂移清理」：两者串行化；后到旧操作返回 `Superseded` 且**不删除**新产物；结束后 INV-1/INV-2 成立 | `knowledge_index_v2.rs` |
| **K-18** | **版本参与有效性（评审 2）** | 把 state 的 `index_version` 改为 `v0` → 检索走回退、不返回旧索引内容；全量重建后恢复 DB 直查 | 同上 |
| **K-19** | **状态-产物不变量（评审 3）** | 遍历全部路径（成功 / 哈希漂移 / 无标题 / veto / 读路径漂移）：`status ≠ indexed ⇒ 章节=0`；`indexed ⇒ 章节>0 ∧ 哈希 ∧ 版本一致` | 同上 |
| **K-20** | **故障不伪装空结果（评审 4 / P1-2）** | 注入章节/状态查询失败（权威清单可读）→ 整项目回退直读且 `200`；再注入修订元数据查询失败 → `KNOWLEDGE_INDEX_UNAVAILABLE`（503）；两例均**不是** `200 {items:[]}`；真无匹配时才 200 空 | `apps/pm-api/tests/documents.rs` |
| **K-21** | **空锚点保护（评审 5 / P1-4）** | 章节 `anchor=''` 与 `section_anchor=NULL/''` 的引用均不产生 +2；非空相等才 +2 | `knowledge_index_v2.rs` |
| **K-22** | **租户约束（评审 5 / P1-5）** | 构造两租户同构数据，检索只出本租户结果；SQL 显式 `tenant_id` 条件生效 | 同上 |
| **K-23** | **大文档性能（评审 6 / P1-6）** | ≥200 标题、≥200 KB：记录**单文档章节数**、**重建总耗时**、**章节批量写入耗时**；章节行数正确、状态一致、重建耗时 ≤ 10 s（宽松上限），实测值回填 §5.6 | 同上 |
| **K-24** | **混合分页（评审 5 / P1-3）** | 部分文档走 DB、部分走回退且存在**同一章节两路都命中**：合并后 `limit/offset` 无重复、无缺口，**DB 行优先**，顺序稳定（复跑两次一致） | 同上 |
| **K-25** | **版本升级全量入口（评审 2 / P0-3）** | 改 `index_version` 常量后旧索引即时失效；`POST /documents/reindex?project_id=` 覆盖全部 published 修订，返回 `{reindexed, stale, failed, superseded}` 计数正确，重建后恢复 DB 直查 | 同上 |
| **K-26** | **同 Project 写串行（v1.3）** | 同一 project 并发发起两个写（不同文档）：整个过程串行（第二次在第一次重建落定后才开始），两篇索引互不交叉；`knowledge_write_locked` 记录 `wait_ms` | `knowledge_index_v2.rs` |
| **K-27** | **`operation_id` 幂等（v1.3）** | 同一 `operation_id` 连续提交两次：只产生 1 条 revision、1 条账本行；第二次返回与首次一致的结果，且不重写 Git、不重复重建 | 同上 |
| **K-28** | **`base_revision` 冲突（v1.3）** | 携带过期 `base_revision_id`（已被他人推进）→ 409 `KNOWLEDGE_REVISION_CONFLICT`，revision 计数不变 | 同上 |
| **K-29** | **跨实例协调与跨项目并行（v1.3）** | 用两个独立 pool 模拟两实例：同 project 写入串行（无并发重建）；不同 project 写入并行（总耗时明显小于串行累加） | 同上 |
| **K-30** | **锁释放兜底（v1.3）** | 持锁连接被强制关闭（模拟实例崩溃）→ 后续写入在超时内取得锁，无死锁残留；连接池未被耗尽的持锁连接污染 | 同上 |

> **编号对齐说明（评审 P0-P1 提供的 K-17～K-25 与本稿编号的差异）**：语义相同者已直接对齐（K-17 并发、K-18 版本失效、K-19 清理一致性、K-20 整体故障、K-21 空锚点、K-22 跨租户）。评审矩阵中「K-23 混合分页 → 本稿 K-24」「K-24 provider 版本升级 → 本稿 K-25」「K-25 大文档重建性能 → 本稿 K-23」（本稿在 v1.1 已先占了 K-23 给性能）。实施时以**本稿编号**为准。

T-060 验收四条 ↔ 用例：可重建且幂等 → K-02；状态与正文哈希一致 → K-02/K-03/K-19；接口向后兼容 → K-12（+K-11）；哈希漂移索引失效 → K-03/K-19。v1.1 评审七项 ↔ K-17~K-25；v2 评审矩阵 ↔ §12.7 编号映射；v1.3 写入协调 ↔ K-26~K-30（同 Project 串行 K-26、`operation_id` 幂等 K-27、`base_revision` K-28、跨实例 / 跨项目 K-29、锁释放 K-30）。

---

## 9. 不做与风险

**不做**：向量检索、中文分词扩展、后台索引作业、outbox 事件、监控页、自动修复索引失败（只在读路径标记 + 回退）、读时惰性重建、历史修订章节的历史查询视图、双版本索引并存；**不设全局写锁、不引入外部锁服务、不改 Agent 并行执行模型、不做按时长细分的锁降级**。

**风险与缓解**：
1. **打分公式两处实现（SQL / Rust）漂移** → K-14 钉住等价；公式只在 §6.2 维护，改动必须双改并跑 K-14/K-21。
2. **同步重建拉长发布请求** → 实测由 K-23 守卫（10 s 上限）；失败降级；若实测不可接受，另开切片改异步（本稿不预留开关）。
3. **回退路径仍是 O(文档数)** → 只在异常状态触发；正常路径全走 DB。
4. **`ILIKE` 级全表扫** → 单项目章节量级可接受；换 FTS 只替换端口实现，不动契约。
5. **版本升级需要全量重建** → 由 §5.5 项目级入口承接；旧版本产物自动失效（不会误用），退化为回退直读（不会丢结果）。
6. **迁移号冲突** → `0023` 属 T-032；本稿用 `0024`。
7. **写锁排队拉长发布请求**（§5.8）→ 仅同 Project 串行，超时 409 可重试；`wait_ms` 入日志，K-26/K-29 验证；若长期排队严重，再评估按文档细分锁（本稿不提前做）。
8. **项目级全量 reindex 长时间持锁**（§5.5）→ 按项目逐个执行、期间同项目写入排队；该入口是运维动作，不在常规发布路径上。

---

## 10. 实施清单（文件级）

| 文件 | 改动 |
|---|---|
| `migrations/0024_knowledge_section_index.sql` | 新建（§4.1 章节索引表 + §4.5 幂等账本表） |
| `crates/pm-application/src/knowledge_write_coordinator.rs` | **新建**：`KnowledgeWriteCoordinator` / `ProjectWriteGuard`（project 级会话 advisory lock、超时、释放与 `detach()` 兜底）；写入五步编排（§5.8.3） |
| `crates/pm-application/src/knowledge_indexer.rs` | 端口新契约 + `DbSectionIndexer`（`rebuild_document` / `search`）+ `apply_transition`（锁内重算前置条件）+ `DirectReadFallback` + `rebuild_quietly` |
| `crates/pm-application/src/knowledge_search.rs` | **新建**：`KnowledgeSearchService::search`（取当前修订 + 四要素判定 + DB/回退编排 + 合并去重 + 排序 + 分页 + 故障分档）；`SearchPage` / `FallbackSearchResult` |
| `crates/pm-application/src/knowledge_index.rs` | 保留解析；新增 `score_section`（含空锚点保护）；`search_documents` 更名 `scan_documents_direct`（仅回退用）；`SearchHit` 增 `revision_id`、`revision_hash`（均不序列化） |
| `crates/pm-application/src/knowledge.rs` | `CreatedDocument` 增 `revision_id`；入参增 `operation_id` / `base_revision_id`（替换 `base_content_hash`）；`create_document` 提交后重建；`KnowledgeError` 增 `IndexUnavailable` / `ProjectWriteBusy` / `RevisionConflict` |
| `crates/pm-application/src/knowledge_proposal/{submit,veto}.rs` | 删 `pending` 插入；经 Coordinator（幂等 + `base_revision` 校验）；提交后重建；veto 用 `apply_transition(Stale)` + 重建生效修订 |
| `apps/pm-api/src/documents.rs` | `search_handler` 走 `KnowledgeSearchService::search` + `tenant_id` + `limit`/`offset`；`reindex_handler` 改 `DbSectionIndexer` 并经 Coordinator；新增项目级 `reindex_project_handler`；`error_response` 增 503 / 409 映射 |
| `apps/pm-mcp`（`pm_submit_document`） | 透传 `operation_id` / `base_revision_id`（写入协调的唯一入口不变） |
| `apps/pm-api/src/routes.rs` | 新增 `POST /v1/pm/documents/reindex`（§5.5） |
| `crates/pm-application/src/lib.rs` | 重导出更新 |
| `crates/pm-infrastructure/tests/tenant_schema.rs` | 表清单加新表（44 → **46**） |
| `crates/pm-infrastructure/tests/migration_0024.rs` | 新建 |
| `crates/pm-application/tests/knowledge_v2.rs`、`apps/pm-api/tests/documents.rs` | 适配新契约 + K-12 / K-20 |
| `crates/pm-application/tests/knowledge_index_v2.rs` | 新建（K-02~K-11、K-13、K-14、K-17~K-30） |
| `docs/ai-pm-knowledge-v2-implementation.md` | 实施完成后回填「索引层 V2」章节与 K-23 实测耗时 |

**验收命令**：`cargo test --workspace`、`cargo check --workspace --all-targets`（`todo.md` §0.2 通用命令）。

---

## 11. 与后续切片的接口

- **T-061**：本稿不写 outbox 事件；`knowledge_index_failed` / `knowledge_search_unavailable` 日志与 `document_index_state` 可直接作为后续指标来源。
- **D5（T-042/T-043）**：`SearchHit.revision_id` / `revision_hash`（内部字段）与 `locator`（anchor/start_line）即证据回填所需的版本与定位信息。**证据必须绑定具体修订**，不能只绑 `document_id`——文档再次发布后，历史 Run 的证据不得被解释为最新正文。
- **未来 provider（PageIndex / 向量）**：实现同一 trait（`provider` / `index_version` / `rebuild_document` / `search`），并复用 §4.2 四要素有效性判定与 §5.3 转换原语；**自带派生表**，不共用 `document_section_index`；切换不改业务调用点。
- **Agent / Executor 扩展（平台层）**：`KnowledgeWriteCoordinator` 是 **Project 级公共边界**，不属于任何 Agent 或 Executor。新增 Agent 角色、更换执行器、增加并行度都不改变写入一致性模型；新写入方只需作为 Coordinator 的客户端接入（`operation_id` + `base_revision_id`）。

---

## 12. 评审记录（2026-11-02 · 专家评审 v2 全文）

> 本节为专家评审原文的格式化收录，保留其结论与论证，**不作改写**；逐条处置见 §0.3，测试编号对齐见 §8。

### 12.1 结论与总体评价

- **结论：有条件通过，修改后过稿**（P6 后半）。
- 方向正确，架构基本成立，但仍有几个影响**数据正确性**与**检索完整性**的 P0/P1 级问题。
- 认可的核心判断：**Git 是唯一权威正文，DB 是可重建的派生索引；索引不可用时回退直读，不让索引故障影响知识发布。**
- 认可的范围控制：**不引入向量库、不引入中文分词扩展、不新增后台 Job。**
- 不建议直接进入实施；问题不在整体架构，而在几个边界条件：**索引状态与索引产物的一致性、发布与索引之间的并发、回退检索的完整性、多租户约束**。

### 12.2 必须修改的问题（P0）

#### P0-1 · 同步重建存在并发覆盖旧状态的风险（§5.2、§5.3）

当前设计：发布修订 R1 → 事务提交 → 调用 `rebuild_document(R1)` → 重建读取 Git 文件 → 删除旧索引并写入新索引。问题在于 **Git 文件与 DB 修订之间存在并发窗口**：

- R1 发布成功并开始重建；R2 随后发布，Git 文件已更新；R1 的重建读到 R2 的文件；哈希不匹配，R1 被标记 `stale`；R2 开始重建并成功。这个例子本身可以正确收敛。
- 更危险的是**两个重建操作的执行顺序**：R2 重建先完成写入 `indexed`，R1 重建后完成写入 `stale`。虽然 R1/R2 是不同 revision，但必须确保 R1 的状态变化**不会破坏 R2 当前检索状态**。

现有 advisory lock 以 `revision_id` 为粒度，只能防止同一修订并发重建，**不能协调同一 document 的不同 revision**。

建议：
1. 保留 revision 级 advisory lock，增加 **document 当前生效修订校验**；
2. 写入索引状态前重新检查最新 published 修订（按 `revision_no DESC LIMIT 1`）；
3. 若当前 published 已不是本次重建的 revision：可保留历史修订的状态记录，但**不允许旧修订覆盖当前修订的状态**；检索只认最新 published revision；
4. 区分「历史修订是否可被索引」与「当前修订是否参与检索」两个概念；**本期只允许当前生效修订进入索引**，避免无意义的历史索引重建。

#### P0-2 · 索引产物与状态更新必须具有原子性（§5.2）

设计用一个事务完成 `DELETE` 旧章节 + `INSERT` 新章节 + `UPSERT document_index_state`，方向正确。但**状态表的唯一键、UPSERT 冲突目标和现有迁移定义必须核实**，尤其是 `UNIQUE (document_revision_id, provider)`；若实际约束不一致，重建会出现无法预期的冲突。

建议明确：
1. 章节数据与状态**同一事务提交**；
2. `indexed` 时章节数据**必须完整**；
3. `stale` / `failed` 时章节数据**必须为空**；
4. 不允许「`indexed` 但无章节」（除非明确支持空章节文档）；
5. **`index_version` 也作为有效性判定条件**：`status = indexed AND indexed_content_hash = revision.content_hash AND index_version = current_index_version AND provider = current_provider`。当前 §6.1 只检查 hash，未检查 `index_version`，会导致解析规则升级后旧索引仍被当作有效索引。

#### P0-3 · 索引版本升级没有明确的失效与重建机制（§3.2、§4.2、§6.1）

设计定义了 `DB_SECTION_INDEX_VERSION = "v1"` 并写入状态表，但**没有定义版本升级后的行为**。未来 v2 增加前言/代码块处理或内容规范化、v3 接入 PageIndex 时，**旧索引即使正文 hash 一致，也不代表索引结构符合当前解析规则**。

建议：
- `index_version` 必须参与有效性判断；
- `provider` 与 `index_version` 不匹配时，旧索引不能参与检索；
- 不允许把旧版本索引当作当前版本索引；
- 本期不做后台重建，但**必须有明确的全量重建入口**。

此项作为**过稿条件**。

### 12.3 重要修改（P1）

#### P1-1 · 回退路径的状态清理不完整（§4.2、§5.2、§6.4）

§4.2 规定 `stale` / `failed` 时「产物已清空」，而 §6.4 的读路径「仅标记 `stale`」，两者不一致：当前索引有 10 个章节 → 发现正文 hash 漂移 → 读路径把状态更新为 `stale` → **但没有删除原有 10 个章节**。虽然 SQL 主检索通过 `status = 'indexed'` 排除它们，数据库仍残留过期数据。

建议统一为：发现 hash 不一致 → 事务内 `DELETE` 该 `(revision, provider)` 章节 + `UPDATE state = stale` → 回退直读。并注意并发：**清理操作不能删除另一个已成功完成的重建结果**，用条件更新或同一 revision 的 advisory lock 保证状态转换安全。

#### P1-2 · 「索引不可用时回退」缺少完整故障边界（§3.1、§6.4、§7）

当前回退只覆盖：状态行不存在、`failed`、`stale`、hash 不一致；**未覆盖**：DB 索引查询 SQL 执行失败、索引表暂时不可访问、`provider` 初始化失败、索引版本不匹配。

建议把故障分两类：**单篇文档索引无效 → 该文档回退直读**；**索引整体不可用 → 整个项目回退直读**。但需明确前提：**DB 整体不可用时，不能假设还能从 DB 获取最新 revision 元数据**，因此整体回退的可用性取决于文档列表及 revision 元数据能否从其他路径取得。

本期至少明确：
1. 索引查询失败时**不返回部分成功结果**冒充完整结果；
2. 若能够获取权威文档清单，则**整体回退**；
3. 若权威元数据也不可用，则**返回明确错误**，而不是空列表；
4. 日志记录**回退原因**。

（对知识库检索尤其重要：**空结果与检索失败是两种完全不同的业务含义**。）

#### P1-3 · 分页总数语义没有定义（§6.3）

当前增加 `limit` / `offset`，但未定义：是否返回 `total`、是否需要 `has_more`、DB 与 fallback 合并后的分页是否一致、是否保证不同页之间稳定。

认可「保持现有 REST 响应结构不变，不为分页引入新响应字段」，但至少应明确：
- 本期采用 **offset 分页**；**不承诺 `total`**；排序必须**全局统一**；**DB 与 fallback 合并之后才分页**。

§6.4 第 3～5 步顺序正确；`limit + offset` 分别取两路候选再合并去重排序，理论上足以取得全局前 `limit + offset` 条。需补测试的边界：**同一章节同时出现在 DB 与 fallback 时，DB 结果优先，且不导致分页重复或遗漏**。

#### P1-4 · 引用加成需要排除空锚点（§6.1、§6.2）

当前 SQL 使用 `coalesce(kr.section_anchor, '') = base.anchor`，若前言章节 `anchor` 为空，可能出现**空锚点引用被错误加分**。

建议改为显式三条件：`kr.section_anchor IS NOT NULL AND kr.section_anchor <> '' AND kr.section_anchor = base.anchor`。另外，引用加成只认**当前 published revision 的有效锚点**——该裁定正确，不要扩大到历史修订。

#### P1-5 · 多租户查询边界还需要明确（§4.1、§6.1、§7）

DDL 对租户的约束做得不错（`tenant_id NOT NULL`、project 组合外键、revision 组合外键），但**检索 SQL 只通过 `project_id` 过滤**。

建议：
- 检索入口必须使用**经过租户授权校验的 `project_id`**，并尽可能在查询中同时带入 `tenant_id`（`WHERE d.project_id = $1 AND d.tenant_id = $tenant_id`）；
- **不能把「UUID 不易猜测」当成租户隔离机制**；
- 确认 `document_section_index.tenant_id` 与 revision 所属 `tenant_id` **永远一致**，而不仅依赖应用层填充。

#### P1-6 · 同步重建的性能假设缺少证据（§5.3、§5.4、§9）

否决后台 Job 的理由是「解析是纯内存操作、唯一 IO 是刚写完的 Git 文件，同步成本以毫秒计」，该判断**缺少实测依据**：文档可能包含大量 Markdown 章节、批量 `INSERT` 规模不确定、Git 读文件可能涉及磁盘或远程存储、发布请求还要承担 DB 写入及索引事务。

建议增加明确规模约束与性能验收：记录**单文档章节数**、测量**重建耗时**、测量**章节批量写入耗时**、验证较大文档不会明显拖慢发布请求。本期仍可不做后台 Job，但**不要把毫秒级当作已证明的事实**。

#### P1-7 · `search_with_fallback` 的职责边界不够清晰（§3.1、§3.2）

目前同时存在 `KnowledgeIndexer`（`rebuild_document` + `search`）、`search_with_fallback`、`DirectReadFallback`、`DbSectionIndexer`，**需要明确谁负责**：查询最新 published revision、判断索引状态、选择 DB 或 Git、去重、排序、分页。

建议：**`KnowledgeIndexer` 只负责索引 provider 的重建与检索，不负责整个系统的 fallback 编排**；由独立应用服务 `KnowledgeSearchService` 负责取当前 published revisions、调用 `KnowledgeIndexer.search`、识别未索引/失效文档、调用 `DirectReadFallback`、合并去重、统一排序、分页。这样以后替换 PageIndex 不会把回退机制绑定到 provider 实现。若项目已有明确端口组织方式可沿用现有命名，**重点是职责不能混淆**。

### 12.4 架构方向建议（P2）

#### P2-1 · `document_section_index` 不要过早承担所有未来 provider 的存储

当前 `UNIQUE (document_revision_id, provider, section_no)` 并通过 `provider` 区分实现，适合当前 DB Section Index；但未来 PageIndex / 向量索引未必具有 `section_no` / `level` / `title` / `anchor` / `content` / `start_line`，可能**是完全不同的索引产物结构**。

建议：本期保留 `document_section_index`；`document_index_state` 作为 **provider 通用状态表**；**不要提前规定未来所有 provider 必须共用 `document_section_index`**，未来 provider 可拥有自己的派生表。**可替换的是端口契约，不是所有 provider 必须共用一套物理数据模型。**

#### P2-2 · `SearchHit.revision_hash` 的位置合理，但建议补充证据契约

赞成 `revision_hash` 内部携带、不暴露 REST。后续 D5 需要证据回溯，建议明确原则：`SearchHit` 携带 `document_id` / `revision_id` / `revision_hash` / `anchor` / `start_line` / `content`；**证据必须绑定具体 revision，不能只绑定 `document_id`**——否则文档再次发布后，历史 Run 的证据可能被错误解释为最新正文。不需要提前改 D5 实现，只需保证 M6 检索结果保留足够信息。

#### P2-3 · 建议补充索引清理策略

目前只讨论当前修订索引、veto 被否决修订清理、重建幂等，**未明确**历史修订、被删除文档、孤儿索引的生命周期。本期至少确定：
1. revision 删除时**级联删除章节索引**；
2. document 删除时**清理关联索引状态**；
3. provider 废弃时**可按 provider 清理**；
4. **不做定期清理任务**（不为此增加新 Job）。

### 12.5 评审肯定的既有裁定

| 设计决策 | 评价 |
|---|---|
| Git 是唯一权威正文 | 保留 |
| DB 存派生章节索引 | 保留 |
| 写入后事务外同步重建 | 保留 |
| 索引失败不影响发布 | 保留 |
| 不引入 pgvector | 保留 |
| 不引入中文分词扩展 | 保留 |
| 关键词 + 结构化加权 | 保留 |
| 当前 published 修订引用加成 | 保留 |
| 响应 JSON 不变 | 保留 |
| provider 同时具备 rebuild / search | 保留 |
| 索引异常时回退直读 | 保留 |

> 尤其赞同 Q4：**一个可替换的索引器不应只有 `search()`，还应有 `rebuild_document()`**；否则未来换 provider 时，业务层仍需知道如何重建旧索引，这不是真正的可替换。

### 12.6 对并发问题的进一步澄清

需修正一个判断：不同 revision 的索引状态分别按 `revision_id` 隔离，§6.1 又只读最新 published revision，因此**旧 revision 的重建本身不会直接覆盖新 revision 的索引状态**。

真正需要防范的是**同一 revision 的成功重建与 stale/failed 清理并发竞争**：
1. 重建 A 读取文件，判断 hash 正确；
2. 重建 B 读取文件，发现 hash 不一致；
3. A 成功写入章节与 `indexed` 状态；
4. B 随后执行 `stale` 清理，删除 A 写入的章节。

因此 §5.2 的 advisory lock **必须覆盖所有状态转换**，而不仅是成功重建的 `INSERT` 事务。建议：
- 同一 `revision + provider` 的**重建、`stale`、`failed` 全部使用同一把锁**；
- 读文件、校验 hash、解析、写入状态的**并发边界必须明确**；
- **不允许较早的重建结果覆盖较新的状态判定**；
- 不同 revision 的旧索引可保留为历史派生数据，但**不能参与当前检索**。

### 12.7 评审建议的验收矩阵（含本稿编号映射）

| 评审 ID | 测试 | 核心断言 | 优先级 | 本稿 ID |
|---|---|---|---|---|
| K-17 | 同 revision 并发重建 | `indexed` / `stale` 不发生交叉覆盖 | P0 | K-17 |
| K-18 | `index_version` 不匹配 | 旧版本不能参与检索 | P0 | K-18 |
| K-19 | `stale` 清理一致性 | `stale` / `failed` 状态下无章节残留 | P1 | K-19 |
| K-20 | 整体索引故障 | 明确回退或明确报错，不伪造空结果 | P1 | K-20 |
| K-21 | 引用空锚点 | 空锚点不获得引用加成 | P1 | K-21 |
| K-22 | 跨租户检索 | 不能检索其他租户的文档 | P1 | K-22 |
| K-23 | 混合分页 | DB + fallback 合并后无重复、无遗漏 | P1 | **K-24** |
| K-24 | provider 版本升级 | 旧版本索引失效 | P0 | **K-25** |
| K-25 | 大文档重建性能 | 记录耗时和章节规模 | P1 | **K-23** |

### 12.8 最终裁决（修改后过稿）

- **不建议推翻现有架构，也不建议扩大 M6 的功能范围。**

**必须处理**：
1. 同 revision 并发状态转换安全；
2. `index_version` 参与有效性判断；
3. 旧版本索引失效规则。

**建议处理**：
4. `stale` / `failed` 清理原子性；
5. 整体索引故障的回退边界；
6. 分页、引用及租户约束；
7. 重建性能验收。

**不建议增加**：向量数据库、中文分词、后台 Job、新增 outbox 事件、监控管理页面、未来 PageIndex 的提前实现。

---

## 13. 实施记录（2026-11-02 · v1.3 落地）

> 实施依据：本稿 v1.3。**契约按 AGENTS.md 规则 1 直接改**——旧契约
> （`IndexReference` / `RetrievedSection` / `index_document` / `index_revision` /
> `MARKDOWN_TREE_PROVIDER` / `MARKDOWN_TREE_INDEX_VERSION` / `search_with_fallback` /
> `search_documents`）**已删除**，不留迁移脚本 / 双写 / 兼容分支 / 回退开关。

### 13.1 文件级落地对照（§10 清单）

| 文件 | 落地情况 |
|---|---|
| `migrations/0024_knowledge_section_index.sql` | ✅ 新建。`ai.document_section_index`（章节索引派生表）+ `ai.knowledge_write_operation`（写入幂等账本）；两条组合外键把 `tenant_id` 与 `core.project` / `ai.document_revision` 绑死 |
| `crates/pm-application/src/knowledge_write_coordinator.rs` | ✅ 新建。`ProjectWriteGuard`（会话级 `pg_try_advisory_lock` + `detach()` 不归还池 + `Drop` 兜底）、`claim_operation` / `complete_operation` / `find_operation`、`current_head`、`check_base_revision` |
| `crates/pm-application/src/knowledge_indexer.rs` | ✅ 端口新契约 `KnowledgeIndexer { provider, index_version, rebuild_document, search }` + `DbSectionIndexer` + **唯一原语** `apply_transition`（`pg_advisory_xact_lock` 锁内重算前置条件 → 章节与状态同事务）+ `rebuild_quietly` / `mark_stale` / `index_states` |
| `crates/pm-application/src/knowledge_search.rs` | ✅ 新建。`KnowledgeSearchService::search`（四要素判定 → DB/回退编排 → 合并去重 → 排序 → 分页）；`FallbackSearchResult` / `FallbackReason` / `SearchPage::normalized` |
| `crates/pm-application/src/knowledge_index.rs` | ✅ 保留纯函数解析 + 新增 `raw_score` / `score_section` / `doc_type_weight`；`search_documents` → `scan_documents_direct`（仅回退用）；`SearchHit` 增 `revision_id` / `revision_hash`（均 `#[serde(skip_serializing)]`） |
| `crates/pm-application/src/knowledge.rs` | ✅ `CreatedDocument` 增 `revision_id`；入参增 `operation_id` / `base_revision_id`（替换 `base_content_hash`）；`KnowledgeError` 增 `IndexUnavailable` / `ProjectWriteBusy` / `RevisionConflict` |
| `crates/pm-application/src/knowledge_proposal/{submit,veto}.rs` | ✅ 删 `pending` 插入；经 Coordinator（幂等 + `base_revision` 校验）；提交后重建；veto = `mark_stale(被否决)` + 重建生效修订 |
| `apps/pm-api/src/documents.rs` | ✅ `search_handler` 走 `KnowledgeSearchService` + `limit`/`offset`；`reindex_handler` 走 `DbSectionIndexer` 并经 Coordinator；新增 `reindex_project_handler`（`{reindexed, stale, failed, superseded}`）；`error_response` 增 503 / 409 映射 |
| `apps/pm-api/src/routes.rs` | ✅ 新增 `POST /v1/pm/documents/reindex` |
| `crates/pm-mcp/src/tools.rs` | ✅ 四个工具 args 增 `operation_id`；`ProposeDocumentArgs.base_content_hash` → `base_revision_id`；`map_knowledge_error` 处理三个新错误变体 |
| `frontend/src/api/documents.ts` + `KnowledgePage.vue` | ✅ `SubmitProposalPayload` 增必填 `operation_id`、改 `base_revision_id`；`confirmProposal` / `vetoRevision` 增 `operationId` 入参并透传（幂等键绑定被操作对象 id） |
| `crates/pm-infrastructure/tests/tenant_schema.rs` | ✅ 表清单 44 → **46** 张，新增 `document_section_index` + `knowledge_write_operation` |
| `crates/pm-infrastructure/tests/migration_0024.rs` | ✅ 新建，8 条（表存在 / 唯一键 / 章节号重复拒绝 / 账本幂等 / command 枚举封闭 / 级联删除 / 跨租户拒绝 / DDL forward-only） |
| `crates/pm-application/tests/knowledge_index_v2.rs` | ✅ 新建，**29 条**（K-02~K-30 全覆盖，另加账本记录、基线语义、状态视图三条） |
| `apps/pm-api/tests/knowledge_index_v2.rs` | ✅ 新建，1 条（K-12 响应契约 + K-20①整项目回退 200 / K-20②权威元数据不可用 503） |

### 13.2 关键实现决策（与本稿的偏差及原因）

| # | 偏差 | 原因 |
|---|---|---|
| 1 | `check_base_revision(None, _)` **放行**（只在显式声明基线时校验） | 原语义把「未声明基线」当「必须等于 head」，会把「放弃乐观并发」的合法调用误判为冲突。**新建文档另有一条硬规则**：携带 `base_revision_id` 直接 `RevisionConflict`（没有既有文档可作基线） |
| 2 | SQL 打分拆成「标题 CASE」+「锚点 CASE」**两个独立加法项** | 原写法把锚点命中放在标题 CASE 的 `ELSE` 分支里——标题命中则锚点不计分，与 §6.2 口径不一致。K-14 钉住 SQL / Rust 两路径等价 |
| 3 | `raw` / `score` / `ref_boost` 显式 `::bigint` | PG 表达式默认 `INT4`，与 Rust `i64` 不兼容 |
| 4 | 回退路径**只扫失效的那几篇**（按 `stale_revision_ids` 逐篇直读） | 原实现全量扫所有文档：浪费，且会把已索引文档重复计入 |
| 5 | 回退路径的引用加成改由 `reference_boosts()` 提供 | 与 SQL 路径同口径（只认当前修订 + 非空锚点），K-14 一致性 |
| 6 | `load_doc_states` / 回退直读失败 → `KnowledgeError::IndexUnavailable` | §6.5 故障分级：**权威元数据不可读必须 503，绝不退化成 200 空结果**（伪装成「无匹配」） |
| 7 | `KnowledgeSearchService.indexer` 类型为 `Arc<dyn KnowledgeIndexer>`，提供 `with_indexer()` | 端口可替换性落地（§3.3），测试可注入替身 |
| 8 | `scan_documents_direct` 单篇读失败 → `continue` + `warn!` | 单篇失效只该让**该篇**回退，不该让整次检索失败 |

### 13.3 验收证据（2026-11-02）

| 门禁 | 结果 |
|---|---|
| `DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm cargo test --workspace` | 退出码 0；**78 个套件 / 566 passed / 0 failed**（日志 `target/t060-run5.log`，连续两跑结果一致） |
| `cargo check --workspace --all-targets` | 退出码 0，**0 error / 0 warning** |
| K-23 性能实测 | 221 章节 / 268,303 B → **84 ms**（见 §5.6） |
| `pnpm typecheck`（frontend） | 退出码 0 |
| `pnpm test:unit`（frontend） | 退出码 0；**88 套件 / 533 passed** |

新增测试分布：`knowledge_index_v2.rs` 应用层 29 条 + 迁移 8 条 + API 契约 1 条 = **38 条新增**。

### 13.4 遗留

- **T-061**：`RunCompleted` 生产者、死信重放接口、事件监控指标——本稿**不写 outbox 事件**（§9 明确不做）；`knowledge_index_failed` / `knowledge_search_unavailable` 日志与 `document_index_state` 可直接作为指标来源。
- **未来 provider（PageIndex / 向量）**：实现同一 trait + 自带派生表，切换不改业务调用点（§11）。
- **§9 风险 1（打分公式双实现）**：公式只在 §6.2 维护；K-14 / K-21 为守门用例。
