# KB-01 · Document → Memory 知识形成路径审计（Q1 补审）

> commit `05ce33d8` · 2026-11-15 · 只读 · 回答专家定向问题 Q1
> 结论：**知识形成链路在「文档发布 → 记忆形成」处整体断裂**。已有的是两侧独立成熟的半链（摄入-发布链 / 提案-审核链），中间无任何提取环节、无引用通道、无 AI 提案入口、无前端页面。

## Q1-1 文档发布后有无自动知识提取：**无**

- 事件：`kb.document_revision.published`（events.rs:44），confirm_revision 同事务写 outbox（kb_service/document.rs:279-283，幂等键 `kb_document_revision_published:{revision_id}`）。
- 全仓唯一消费者：`KbDocumentRevisionPublishedHandler`（kb_pc_handlers.rs:765-823，注册 :1281-1284），仅两件事：
  1. `index_published_revision`（kb_service/indexer.rs:180）建章节 FTS 索引；
  2. `notify_once` 平台通知留痕（:814-821）。
- 无 worker、无 AI 提取消费者、无 SSE 消费逻辑（全仓 grep 零其他命中）。

## Q1-2 从 kb.document/document_revision 创建 memory 的路径：**不存在**

- `INSERT INTO kb.memory` 全仓唯一生产入口 memory.rs:365（propose_create），唯一调用者 REST propose_memory（kb_memory_api.rs:313）。
- 入参 `ProposeMemoryBody`（kb_memory_api.rs:267-277）只有 memory_type/title/summary/body/change_reason/refs——**无 document/revision 引用字段**。
- 即使手动构造引用也会被双层白名单拒绝：resolve_refs 矩阵（discussion_memory.rs:116-217，:209-216 兜底拒）+ validate_refs（memory.rs:206-211，knowledge_base 域 target_type 只许 `source`）。

## Q1-3 从 docproc.artifact 创建 memory 的路径：**不存在**

- docproc 与 memory 唯一的桥是反向的：docproc.artifact → kb.document_revision draft（document.rs:292-375，source_ref=`docproc:artifact:{id}`，0017）。
- docproc_service/ocr/runner 中 grep `kb.memory|propose_memory` 零命中。

## Q1-4 AI 提取→人工审核→正式化流程现状

| 通道 | 提案（propose） | 审核（review） |
|---|---|---|
| REST | POST /v1/kb/memories（kb_memory_api.rs:45）——**契约允许 AI Key 提案**（:17-18「AI 提议」） | POST /v1/kb/memories/{id}/review（knowledge_services.rs:61,275，trusted context 门 AI 403 :572-573） |
| MCP | **无任何 kb/memory 工具**（pm-mcp tools 全集无命中） | — |
| 前端 | **无页面**（ai-frontend api/knowledge.ts 只封装 /v1/kb/documents*） | — |

即：AI 提案 memory 在契约上开放、通道上不存在；人工提案连 UI 都没有。当前 0 行数据与此完全一致。

## Q1-5 Memory 溯源能力

- memory_ref 四元组（BL:2914-2929）role∈{evidence,subject}；target_type 白名单（discussion_memory.rs:116-217）：
  - subject：requirement/task、requirement/requirement（可带 revision）
  - evidence：requirement/conversation_message（限提案人本人会话）、requirement/task_interaction、knowledge_base/source（memory.rs:226-245 验项目关联）
- **不能指向 kb.document / kb.document_revision / 章节对象**——矩阵不含，双层兜底拒。
- locator（BL:2925）为自由 jsonb，无任何结构化定位约定（无章节/行号 schema）。
- kb.memory 系无 evidence jsonb 列；证据完全由 ref_role='evidence' 行承载（≥1 强制 memory.rs:216-221）。pc.evidence 的 document_section 类型（target_domain='knowledge_base'+target_revision_id）是 PC 域列，与 kb.memory 不互通。

## Q1-6 已有能力 vs 缺失环节

**已有（可复用）**：
1. 摄入-发布链：docproc.artifact → draft（source_ref+checksum 复检）→ 真人 confirm → published 事件。
2. 发布后索引：章节 FTS 家族现行 + 失败台账。
3. Memory 治理链：propose_create/propose_change + K4 审核 CAS + CURRENT/SUPERSEDED 不可变版本 + 强制 evidence 引用。
4. 引用校验网关：resolve_refs 五类对象存在性/归属/属主校验。

**缺失（即知识形成链断裂点）**：
1. published 事件无知识提取消费者。
2. memory_ref 无法引用已发布文档/章节——**memory 与文档库溯源断链**（KB-02 裁决 KB-06 保留四元组时，需同步扩展白名单矩阵或建立 document 引用类型）。
3. locator 无结构化 schema。
4. MCP 无 memory 工具（AI 提案无通道）。
5. 前端无 memory 提案/列表页。

## 链路现状图

```
[文档侧]                                            [记忆侧]
docproc.artifact (0016)
   │ ingest（source_ref=docproc:artifact:{uuid}）
   ▼
kb.document_revision (draft)
   │ confirm_revision（真人 interactive 门）
   │ → outbox kb.document_revision.published
   ▼
KbDocumentRevisionPublishedHandler（唯一消费者）
   ├─ 章节索引 index_published_revision
   └─ notify_once 留痕
   ══════════ ✗ 断链：无提取消费者、无 doc→memory 路径 ══════════
                                                     ▲
REST POST /v1/kb/memories（仅手工直调，无 MCP/前端）  │
   └─ resolve_refs 白名单（subject: requirement/task │
      evidence: conversation_message/task_interaction│
      /kb.source）                                    │
   └─ propose_create → PENDING_REVIEW → K4 review ────┘
      → CURRENT 版本（不可变）
```

## 对裁决的输入

- 支持 KB-02（Document 与 Knowledge 分离，补知识形成关系）：断裂点证明当前「知识形成」纯靠人工在 REST 直插，U08 裁决 Knowledge Extraction 对象时，最小闭环 = ① memory_ref 白名单扩 document/revision 引用类型（或新引用通道）+ ② published 事件加提取消费者（受控自动候选）+ ③ MCP 工具/前端页（通道）。
- 支持 KB-07：审核端点已具备 trusted context 门，接入审阅中心只缺聚合与入口（见 KB-03 分册）。
- NO_PRODUCTION_DATA：以上全部为代码层结论，链路从未在真实数据走通。
