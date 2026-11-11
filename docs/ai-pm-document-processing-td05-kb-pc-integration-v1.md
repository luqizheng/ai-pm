# Document Processing · TD-05 KB/PC 集成契约 — V1

**上游：** 规格 D-05/§2.2/§6.6 + 仓内实证（kb_service::confirm_revision 的 require_human 真人门、KnowledgeRoot Git 权威、kb-pc-contracts 端口）
**落地状态：** V1 已实现（commit 6106129a）——迁移 0017（kb.document_revision 加 source_ref/source_checksum）+ `kb_service::ingest_from_docproc` + `confirm_revision` ④b checksum 复检；见本文 §7 实装记录。
**一条铁律（D-05）：** 转换成功 ≠ 正式知识。docproc 产物进入业务域时**永远以候选（draft/提案）形态**出现，确认动作只能由对应业务域的既有真人门完成。

---

## 1. 集成模型：候选引用 + 领域自有确认

```text
docproc.artifact (markdown, 权威=docproc)
        │  ① 引用（不复制内容进业务表）
        ▼
业务域候选行（kb.document_revision draft / pc 提案），source_ref = docproc:{artifact_id}
        │  ② 领域真人确认（kb: confirm_revision require_human；pc: 各自确认门）
        ▼
正式知识正文 → Markdown + Git 权威（KnowledgeRoot），路径由业务域裁决
        │  ③ 派生索引
        ▼
PageIndex（document_section_index）由业务域触发，docproc 不触碰
```

三条边界（规格 §2.2/§3.2）：
1. **引用不复制**：候选行只存 `source_ref`（artifact_id + checksum），正文按需从 docproc 服务接口拉取——checksum 校验保证「确认时看到的 = 转换产物本身」，防止确认与入库之间内容漂移。
2. **确认权在领域**：docproc 不写 kb/pc 的任何表；业务域也不直查 docproc 表，一律经 `docproc_service` 端口。
3. **Git 路径归领域**：确认后入 Git 的 canonical_path、目录组织由 KB/PC 既有规则决定（kb_service::canonical_path）；docproc 不参与 Git。

## 2. 服务端口（`pm_application::docproc_service` 暴露给业务域）

```rust
#[async_trait]
pub trait DocprocReader: Send + Sync {
    /// 候选行创建前/确认前的内容拉取（含 checksum 校验，不匹配即错）
    async fn artifact_markdown(&self, tenant: Uuid, artifact_id: Uuid)
        -> Result<ArtifactMarkdown, DocprocError>;   // {content, checksum, document_id, version_id}
    /// 列出某原始文档的可用 markdown 产物（供候选选择）
    async fn latest_markdown_artifact(&self, tenant: Uuid, document_version_id: Uuid)
        -> Result<Option<ArtifactRef>, DocprocError>;
}
```

- 实现在 pm-infrastructure（读 docproc 表，无 parser 依赖）——满足 TD-02 依赖边界。
- `source_ref` 统一格式：`docproc:artifact:{uuid}`（含 checksum 于候选行冗余列，确认时复检）。

## 3. KB 接入（V1 实施面）

| 步骤 | 动作 | 落点 |
|---|---|---|
| 候选创建 | 业务入口（KB 资料导入页/服务层）选 docproc 文档 → 建 `kb.document_revision` draft，`source_ref=docproc:artifact:{id}`，正文=拉取的 markdown | kb_service/document.rs 既有 draft 通道 |
| 确认 | `confirm_revision`（require_human）→ published → 写 Git（KnowledgeRoot） | **完全复用，零改动** |
| 确认时复检 | confirm 前比对候选行 checksum 与 docproc artifact checksum，不一致 → 拒绝（提示重拉） | kb_service 加一行校验 |
| PageIndex | published 后既有 indexer 流程 | 零改动 |

**不做**：docproc.job.completed 事件自动创建 KB draft（规格 §5.1 的「AI 分析/人工确认」属业务编排；自动入候选默认 off——与 discovery 编排器同款开关模式，P1 视需要再加）。

## 4. PC / Requirements / Design 接入（V1 边界）

- 四域（PC/Requirements/Design/PM）采用**同一候选引用模式**，但 V1 只实现 KB 一个消费端（规格 §11 阶段 C 的接入顺序）。
- 其余域的接入 = 各域建候选行时引用 `docproc:artifact:{id}`（模式已立，无需 docproc 侧任何改动）；本文档即契约，各域实施时不再另行裁决。

## 5. 事件与集成的衔接（U-06/TD-02 §4）

`docproc.job.completed` 事件 payload 已含 `artifact_ids + report_summary`：
- V1 handler（pm-worker）：标记 delivered（事件仅审计/通知语义，不触发自动业务写入）。
- P1 可选开关：handler → 按 document 元数据自动建候选提案（默认 off，规格 D-05 AI 只能「提出」不能「确认」）。

## 6. 验收对照（规格 §12）

| 规格验收项 | 本文契约的落点 |
|---|---|
| 知识隔离：转换成功不自动确认正式知识 | §1 铁律 + §3 不做清单（自动候选 off） |
| 来源追溯：转换内容可追溯到原始文档 | source_ref + TD-01 source_mapping（artifact→version→document 链） |
| 索引隔离：转换成功 ≠ PageIndex 完成 | §3 PageIndex 由 published 触发，docproc 零触碰 |
| 正式知识权威唯一（Git） | §1 边界 3：docproc 产物永存对象存储层，Git 权威由领域写入 |


## 7. V1 实装记录（commit 6106129a）

| 设计项 | 实装 | 位置 |
|---|---|---|
| 候选引用（引用不复制） | `kb.document_revision.source_ref = docproc:artifact:{uuid}` + `source_checksum` | 迁移 0017 |
| 候选创建 | `kb_service::ingest_from_docproc`（拉取 markdown + 校验 checksum → 建 draft） | pm-application/src/kb_service/document.rs |
| 确认时复检 | `confirm_revision` ④b：source_ref 解析 → 查 docproc.artifact.checksum → 与 source_checksum 不一致则 **Conflict（留在 draft）** | 同上 |
| 正式知识权威 | 未改：Git 路径/版本/确认仍走既有 `confirm_revision` 通道 | 零改动 |
| PageIndex | 未触碰（转换成功不触发索引） | — |

**租户边界实证**：`kb.document` 的 `(tenant_id, project_id)` 外键强制 KB 项目与 docproc 文档同租户——跨租户接入被数据库层拒绝（测试中先撞 FK 后修正夹具）。这不是额外约束，是既有租户模型在接入面的显形。

**D-05 铁律的结构性守卫**：用例 `docproc_failure_stays_draft_never_publishes_knowledge` 以源码面断言固化——`docproc_service` 与 `docproc_worker` 均不含任何 KB 写入口（`use crate::kb_service` / `kb.document_revision` 字面量不存在），确认权永远在业务域。
