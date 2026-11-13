# S05 后台批 3 · 执行上下文注入闭环 实施报告 V1.0

| 项目 | 内容 |
|---|---|
| 文档性质 | 实施验收记录（补 S05-02「执行上下文注入」真实缺口） |
| 日期 | 2026-11-10 |
| 分支 | `feat/s05-batch3-knowledge-injection`（worktree ai-pm-s05-b3，基线 master @ 0924bfb0） |
| 迁移 | `0014_task_knowledge_pin.sql` |
| 用户裁决 | 知识来源 = **显式锚点 + 人工确认**（2026-11-10） |
| 差异说明 | `docs/architecture/knowledge-services/kp-s05-knowledge-injection-contract-deviations-v1.md`（D-KP-S05-1） |

---

## 一、缺口与解法

**缺口**：`delivery.task_knowledge_ref` 锚点表早已存在，但生产链路从未接上——唯一写路径是 `update_task` 的全量替换而生产零调用 → 表恒空；契约只给锚点不给正文；「按 task 取知识文档」能力不存在。

**解法**（三层闭环）：

| 层 | 机制 | 落点 |
|---|---|---|
| 锚点**自动生成**（候选） | 任务绑定需求 → 同事务按 `doc_key='req-{id}'` 生成锚点（`is_required=true`）；需求投影后回填已关联任务 | `pm_write::link_task_requirement` · `requirement_projection::bind_projection` |
| **人工确认**（权威） | `update_task(knowledge_refs)` 全量替换：增删 / 改 required / 钉版 | 既有路径（`KnowledgeRefInput` 加 `pinned_revision_no`） |
| **正文注入** | `RunContract.knowledge_sections[]`（additive，开关默认 off） | `task_knowledge::resolve_sections` ← `ai.document_section_index` |

## 二、契约变更（additive，`contract_version` 仍为 1）

1. `knowledge_sections[]`：`#[serde(default, skip_serializing_if = "Vec::is_empty")]`；开关 `AI_PM_KNOWLEDGE_SECTION_INJECTION`（默认 off，拼写错误按 off + 告警）。单 section ≤ 800 字符 / 每文档 ≤ 6 节 / 全文 ≤ 12 节 / `is_required` 优先。
2. `knowledge_refs[].pinned_revision_no`（可选）：NULL=未钉版（取最新 published）；非空=钉版。
3. `knowledge_refs[].revision_no` 语义：**钉版优先，否则最新 published**（默认行为不变，钉版后执行可复现）。

D2 §4.2 两条硬约束仍满足：每字段可追溯库表（无拼装）、契约不含工具/prompt/model 字段。

## 三、迁移

`0014_task_knowledge_pin.sql`：只加一列 `delivery.task_knowledge_ref.pinned_revision_no integer NULL`（CHECK ≥1）。**不新建知识表**——锚点表早已存在。

## 四、验收

| 用例组 | 覆盖 | 结果 |
|---|---|---|
| `crates/pm-application/tests/knowledge_injection.rs` | 锚点自动生成（绑定即生成 + 幂等）/ 人工确认（增删 + 改 required + 钉版）/ 注入与开关纪律（off 不序列化、on 取最新 published、钉版优先）/ 回填时序（先绑任务后提交需求） | **4/4 绿** |
| `crates/pm-application/tests/run_context.rs` | 既有契约用例（受 `KnowledgeRefInput` 扩展影响，已同步） | 11/11 绿 |
| `crates/pm-mcp`（含 `contract.rs` 契约镜像） | `ContractMirror`（`deny_unknown_fields`）同步新增 `knowledge_sections` + `pinned_revision_no` | 全绿 |
| 全 workspace 回归 | `cargo test --workspace --no-fail-fast` | 见 §五 |
| `cargo fmt` / `cargo clippy` | 本批文件 | 见 §五 |

## 五、回归与门禁

- 全 workspace：见 §六。
- `cargo fmt`：仅本批文件格式化（master 既有 rustfmt 漂移已于 2026-11-10 归一收口）。
- `cargo clippy`：本批零新增告警。

## 六、明确不做

- 不做自由检索注入（用户裁决：违背「正确的执行依据」红线）。
- 不做「按 task 取 KB 文档」的通用检索能力。
- 不改任务 13 态 / Run 7 态 / MCP 四工具契约（`pm_get_run_context` 字段增加属 additive）。
- 不做锚点的执行快照固化（留待 D5 交付面）。

## 七、后续批次入口

`docs/scenarios/s05/S05-00-code-facts-and-gap-analysis-v1.md` §四：B2=ACP-09 真实栈回归补缺（e2e 五项零覆盖 + 两项空断言）；B4=Q-D2C-1~6 裁决 → 调度策略层。
另：本批把「按 task 取知识」从「不存在」变为「显式锚点路径已通」，检索型补充（若需要）留待其后独立裁决。

---

**文档结束**
