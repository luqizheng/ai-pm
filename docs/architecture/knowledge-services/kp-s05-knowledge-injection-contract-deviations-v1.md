# KP-S05 · S05-02 执行上下文注入 —— 契约差异说明 V1.0

| 项目 | 内容 |
|---|---|
| 文档性质 | Run 契约字段扩展的**差异说明**（照 D-KP3-7 模板：先落差异说明，再动契约） |
| 日期 | 2026-11-10 |
| 领域 | S05 · AI Agent Execution & Collaboration（批 3 · 执行上下文注入） |
| 用户裁决 | 2026-11-10：知识来源 = **显式锚点 + 人工确认**（不做自由检索） |
| 关联 | `docs/slice-d/ai-pm-slice-d2-mcp-context-v1.md` §4.2（字段→来源映射）· `docs/scenarios/s05/S05-00-code-facts-and-gap-analysis-v1.md` 缺口 S05-02 · 迁移 0014 |

---

## 一、背景与缺口

S05 蓝图 S05-02 要求 Agent 获得「相关知识库内容」，且必须「保证 Agent 使用的是**正确的执行依据**，而不是随意读取过时信息」。

代码事实（批 3 开工前）：

| 事实 | 证据 |
|---|---|
| 锚点表 `delivery.task_knowledge_ref` 早已存在（指向 `ai.project_document` 文档身份） | `migrations/0001_baseline.sql:1802`（源自 0015） |
| 但**生产链路从未接上**：唯一写路径是 `pm_write::update_task` 的全量替换，而生产代码**零调用** | `pm_write/task.rs:400`（`replace_knowledge_refs`）；`UpdateTaskRequest.knowledge_refs` 的构造只出现在测试里 |
| → 表几乎恒空，锚点从不下发 | — |
| 契约只给**锚点**（document_id / doc_key / canonical_path / revision_no），**不给正文** | `run_context.rs`（`knowledge_refs` 装配） |
| 「按 task 取知识」的能力只有 `discussion_memory_for_task`（L1 讨论记忆摘要，默认 off），**取 KB 文档内容的能力不存在** | `discussion_memory.rs:273` |
| 正文其实**在库**：`ai.document_section_index`（provider=`db-section`）按修订存 section 内容 | `migrations/0001_baseline.sql:932` |

## 二、用户裁决

**知识来源 = 显式锚点 + 人工确认。**

- 不做「按 task 派生 query 自由检索」——那正是「随意读取」：检索结果随语料漂移，无法保证是本任务被确认过的执行依据。
- 采用：任务绑定需求时**自动生成候选锚点** → **人工确认**（增删 / 改 required / 钉版）→ 执行时按锚点注入**正文**。
- 双通道方案（锚点为准 + 检索补充）被否：增量价值不足，且会稀释「锚点是权威依据」的语义。

## 三、契约变更（additive，3 处）

### 3.1 `RunContract.knowledge_sections[]`（新增，可选）

| 项 | 值 |
|---|---|
| 形态 | `#[serde(default, skip_serializing_if = "Vec::is_empty")]` |
| 开关 | `AI_PM_KNOWLEDGE_SECTION_INJECTION`（`off|on`，**默认 off**；拼写错误按 off + 告警） |
| `contract_version` | **不跳**（仍为 1）——additive 字段，off 或无锚点时契约输出与本字段引入前**逐字节一致** |
| 内容 | 锚定文档的 section 正文（`document_id` / `doc_key` / `revision_no` / `section_no` / `title` / `anchor` / `content` / `is_required`） |
| 截断 | 单 section ≤ 800 字符；每文档 ≤ 6 节；全文 ≤ 12 节；`is_required` 优先 |

### 3.2 `knowledge_refs[].pinned_revision_no`（新增，可选）

`#[serde(default, skip_serializing_if = "Option::is_none")]`。`NULL` = 未钉版（取该文档最新 published 修订）；非空 = 人工确认时钉定的版本。

### 3.3 `knowledge_refs[].revision_no` 语义微调

原语义为「live 解析：每次读取取最新 published」（D2 §4.2 R-F11）。现改为 **钉版优先、否则最新 published**：

- 默认（未钉版）行为**不变**（仍取最新 published，不落后）；
- 人工钉版后以钉版为准——**这是「被确认的执行依据」**，且使执行可复现（事后能回答「这次执行用的是哪一版」）。

## 四、字段 → 来源映射（D2 §4.2 补行）

| 契约字段 | 库内来源 | 可空 |
|---|---|---|
| `knowledge_sections[]` | **S05-02 增补（2026-11-10，用户裁决「显式锚点+人工确认」/ 差异说明 D-KP-S05-1）**：任务锚点（`delivery.task_knowledge_ref`）→ 文档身份与 published 修订（钉版优先，由 `run_context.rs` 既有白名单使用方解析）→ 章节正文（`ai.document_section_index`，provider='db-section'）——**正文级**；开关 `AI_PM_KNOWLEDGE_SECTION_INJECTION` 默认 off，off 或无锚点时**字段不序列化**（additive，`contract_version` 不跳） | 是（开关 off / 无锚点时缺省） |
| `knowledge_refs[].pinned_revision_no` | `delivery.task_knowledge_ref.pinned_revision_no`（迁移 0014 新增列） | 是（NULL=未钉版） |

> D2 §4.2 的两条硬约束仍然成立且被满足：① 每个字段可追溯到库表/列（无拼装字段）；② 契约中不含任何工具相关字段（tool 名 / prompt 模板 / model 名）。知识章节正文是**知识内容**，不是工具指令。

## 五、迁移与写入面

### 5.1 迁移 0014（`0014_task_knowledge_pin.sql`）

只加一列：`delivery.task_knowledge_ref.pinned_revision_no integer NULL`（CHECK ≥ 1）。**不新建知识表**——锚点表早已存在。

### 5.2 ADR-06a 合规（跨领域冻结的共享容器）

文档身份表与文档修订表是**跨领域共享容器**（KB / Goal / Discovery / Task Contract / Requirement Projection 五方使用），过渡期红线 = **禁止新增使用方**。机械化守卫：`crates/kb-pc-contracts/tests/guard_ai_container_users.rs`——扫描生产代码与迁移面，表名出现在 **SQL 字符串或注释**中即算使用证据。

因此本批的实现分布是：

| 关注点 | 落点 | 理由 |
|---|---|---|
| 文档身份解析（doc_key → document_id） | `pm_write/task.rs`（既有白名单使用方）· `requirement_projection.rs`（既有白名单使用方） | 新增文件直接访问受限表会被守卫拒绝 |
| 章节正文读取 | `task_knowledge.rs`（新文件，只查章节索引表——不在冻结集合内） | 正文表未冻结 |
| 契约装配（锚点 → 章节合并） | `run_context.rs`（既有白名单使用方） | 修订解析只在此处 |
| 迁移 | 0014 **零命中**受限表名（连注释也不出现） | 迁移面唯一合法落点是基线迁移 |

**结论**：使用方集合**未增长**（仍是原五方），迁移面零命中——守卫 3/3 绿。

### 5.3 写入面（锚点从恒空到有生产链路）

| 时序 | 写入点 | 机制 |
|---|---|---|
| 任务绑定需求 | `pm_write::link_task_requirement`（同事务） | 文档身份解析（`doc_key = "req-{requirement_id}"`）→ 插入锚点（`is_required=true`、未钉版）。文档尚不存在则跳过 |
| 需求投影完成 | `requirement_projection::bind_projection`（同事务） | 回填该需求**已关联的所有任务**的锚点（覆盖「先绑任务、后提交需求」时序） |
| 人工确认 | `pm_write::update_task`（既有路径，`knowledge_refs` 全量替换） | 增删锚点 / 改 `is_required` / 设 `pinned_revision_no`；**人工确认是权威**——全量替换会覆盖自动生成的候选 |

两种时序全覆盖：先提交需求后绑任务 → 5.2 第 1 行命中；先绑任务后提交需求 → 5.2 第 2 行回填。

## 六、配套与验证

- `crates/pm-application/src/task_knowledge.rs`：锚点绑定 / 回填 / 章节解析的唯一实现。
- `crates/pm-mcp/tests/contract.rs`：`ContractMirror`（`deny_unknown_fields`）同步新增 `knowledge_sections` 镜像与 `pinned_revision_no`。
- 集成测试 `crates/pm-application/tests/knowledge_injection.rs`：4 用例——锚点自动生成（+幂等）/ 人工确认（增删改钉版）/ 注入与开关纪律（off 不序列化、on 取最新 published、钉版优先）/ 回填时序覆盖。
- 回归：既有 `run_context.rs`（11）、`pm-mcp` 契约测试（7）全绿。

## 七、明确不做

- 不做自由检索注入（用户裁决）。
- 不做「按 task 取 KB 文档」的通用检索能力（属批 3 之后的增量，且需先定语义）。
- 不给锚点加「钉版快照列」之外的固化机制（执行快照留待 D5 交付面）。
- 不改任务 13 态 / Run 7 态 / MCP 四工具契约（`pm_get_run_context` 契约字段增加属 additive，不改工具契约）。

---

**文档结束**
