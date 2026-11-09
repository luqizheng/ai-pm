# KB/PC 知识层模式选型：PageIndex 模式 vs llm_wiki 模式（V1）

> **状态**：裁决稿，待用户确认后进实施设计。
> **输入面（用户定版 2026-11-09）**：只考虑 `txt / pdf / image / xlsx / docx / markdown` 六格式；**旧版 Office（.doc/.xls）不支持**。
> **内容面貌（用户核过真实文档）**：最多的是**版本链修改文档**（同一份制度的 v1.2、v1.3…系列文件）；其次客户收集的**制度**、**流程图**、**财务套表 / 打印版式的 xlsx 套表**。
> **关联**：`ai-pm-markdown-knowledge-space.md`（PageIndex 定位）· `ai-pm-t060-knowledge-index-v1.md`（db-section/provider 端口）· llm_wiki 参考研究（2026-11-09）· `kb-legacy-import-pilot-plan-v1.md`（搁置，本稿接管其 D3/D5 裁决职能）

---

## 一、结论：分层采用，不是二选一

| 层 | 采用模式 | ai-pm 现状 | 缺口 |
| --- | --- | --- | --- |
| **检索层**（找得到） | **PageIndex 模式** | **已建成**：`db-section` v1 就是 PageIndex 模式的自研实现（章节树 + 锚点 + `document_index_state` 有效性判定 + provider 端口预留） | ①kb/pc 域没有索引表（KP-3 如实登记"知识检索未建"）②六格式 → markdown 的解析层为零 |
| **知识层**（读得懂、可累积） | **llm_wiki 模式**（借操作规程，不借直写） | 骨架已画好：kb.source(不可变来源) / kb.document+pc.cognition(知识层) / 质量门+真人确认(schema 纪律) | ①ingest/lint/index 三个**操作规程**没建②目录层没建③增量维护循环没建 |

**裁决一句话：检索层照 PageIndex 走（已建成，补解析即可）；知识生产照 llm_wiki 的三操作规程走（ingest/index/lint），但一切产物落 KB/PC 既有草案→真人确认链，绝不学它 LLM 直写文件。**

两个模式不是竞争关系，而是 llm_wiki 自己就是这么分层的——它的 "Raw sources → Wiki → Schema" 映射到 ai-pm：

| llm_wiki | ai-pm 对应物 | 谁更强 |
| --- | --- | --- |
| Raw sources（不可变） | `kb.source`（触发器锁死 + hash 去重） | 平 |
| Wiki（LLM 生成的页） | `kb.document_revision` + `pc.cognition_object` | **ai-pm**（有修订链/版本发布/提案审批） |
| `[[wikilink]]` 图 | `pc.cognition_relationship`（类型化四元组跨域） | **ai-pm** |
| 矛盾（ingest 时 LLM 顺带发现） | `pc.conflict`（8 类领域对象，含 `revision_conflict`/`rule_contradiction`） | **ai-pm** |
| index.md 目录 | ❌ 无 | **llm_wiki**（必须补建） |
| lint 巡检操作 | ❌ 无巡检面 | **llm_wiki**（必须补建） |
| ingest 编译操作 | 分析链只覆盖代码仓库 | **llm_wiki**（须扩到文档） |
| **版本链** | `revision_no` + status + superseded | **ai-pm 完胜（llm_wiki 为零）** |
| 租户/审计/门禁/回滚 | 有 | **ai-pm 完胜（llm_wiki 为零）** |

---

## 二、决定性论据：版本链——你最多的文档形态，恰恰是 llm_wiki 的盲区

真实文档面貌里占最大比重的是"同一份东西的 v1.2、v1.3……系列修改文档"。两个模式对此的处理能力天差地别：

**llm_wiki 模式裸用会碾平版本链**：v1.2 和 v1.3 是两个独立 source 文件，编译层会把两者的内容**混在同一个 wiki 页里**——没有 family 概念、没有 superseded 语义、没有"现行版本"。对个人读书笔记无妨，对**制度文档是致命的**：检索命中 v1.2 的已废止条款并当成现行规范，比检索不到更糟。

**KB/PC 修订模型就是为这个造的**，版本链直接映射（零新表）：

```
版本族（family）= kb.document（doc_key 稳定，doc_type=knowledge/architecture）
  系列文件 v1.2 → kb.document_revision（revision_no 按版本序）
  系列文件 v1.3 → kb.document_revision（最新 published = 现行）
  旧修订 = 历史（可 diff「v1.2→v1.3 改了什么」），检索永不命中
  跨制度矛盾（财务制度 vs 采购制度）→ pc.conflict（rule_contradiction）
  版本打架 → pc.conflict（revision_conflict，枚举里现成）
```

**衍生裁决：检索索引只索引"现行修订"**——这是版本链面貌对检索层的硬约束。llm_wiki / PageIndex 都没有这条（它们索引一切文件），必须作为 kb 域索引表的第一原则（对齐 `ai.document_index_state` 的有效性语义：只认 published）。

### 2.1 版本族解析（本选型唯一新增的硬需求件）

- **文件名规则**：`<名称> v1.3.docx` → family key + version no（正则提取；`V1.3`/`v1.3`/`（1.3）`变体归一）。
- **兜底**：文件名无版本 → 读 front page 标题/修订记录表；再无 → 单文件自成 family（revision_no=1）。
- **家族级标注（D4 最小版）**：`现行 / 废止` 二值，挂 family 级；权威部门/生效日期后置。
- **"修改说明"类文档**（若系列里混有 delta 备忘而非全文重发）：作为对应全文修订的 `change_summary` 附着，不独立成 family。*（待裁：需看真实文件名样本确认两种形态的占比，见 §六）*

---

## 三、六格式处理矩阵

| 格式 | 提取（L0） | 检索入口 | 编译处理（ingest） | 硬指标/坑 |
| --- | --- | --- | --- | --- |
| **markdown** | 直读 | `db-section` 现成（章节树） | 直编 | 无 |
| **txt** | 直读 | 无大纲 → 整篇单节 | 直编（价值密度低多走 archive） | 无 |
| **docx** | 解析 → markdown，**Word 大纲级别 → `#` 层级** | 章节树 | **版本族归并 → 修订链**；高价值制度编译进 PC 对象 | **大纲级别还原率 = 解析器硬指标**；批注/修订痕迹剥离 |
| **pdf** | pdfium 文本层（llm_wiki 同款选型）→ markdown | 章节树 | 同 docx | 扫描版 pdf → **转 image 路径**（见下行） |
| **image** | vision caption：**有界事实描述**（"采购审批流程：申请→部门审批→财务复核→付款"），一次 vision 调用/图 | caption 文本（锚回原图路径） | 流程图 caption → **PC 流程对象候选**（人工确认后成 cognition_object） | caption 只写"图里有什么"不写推断；成本=每图一次 vision 调用（待裁预算） |
| **xlsx** | 逐 sheet → markdown 表格；表头规范化、合并单元格拍平、**版式噪声剥离**（打印套表的装饰列/行） | 表格行文本（V1 够用：套表行数有限） | 套表编译 = **sheet 清单 + 指标口径摘要**（"本表 A 列=应发工资，合计行=E列求和"） | **真·财务查询（算数/勾稽）是结构化查询，不是检索——明确后置，V1 不做** |

**旧版 Office 出局的影响**：`.doc/.xls` 不解析（登记 `unsupported_format`）；来源端纪律 = 收集时统一转存 docx/xlsx（客户侧一次性转换，工具不限）。

---

## 四、四内容类型的落点

| 类型 | 主形态 | 落点 | 编译价值 |
| --- | --- | --- | --- |
| **版本链制度** | docx 系列 | family→kb.document 修订链；**现行版进检索**；关键条款 → PC 约束对象（constraint） | 高——条款级编译是 PC cognition 的主粮 |
| **客户制度（非版本链）** | docx/pdf | 单文件单 family；同上 | 高 |
| **流程图** | image（或 docx 内嵌图，提取出来同路径） | 原图=kb.source；caption=可检索文本；确认后 → PC 流程对象 | 中——流程结构化对需求分析直接有用 |
| **财务套表 / 打印 xlsx 套表** | xlsx | 原件=kb.source；sheet 提取物=kb.document（doc_type=knowledge）；口径摘要编译 | 中——V1 做"口径可查"（这列是什么），不做"数字会算" |

---

## 五、落点接线（不动任何红线）

1. **索引表**：kb 域**自建**索引表（章节树同构，复用 `knowledge_index.rs` 纯函数解析）——**不碰** `ai.document_section_index`，**不碰** `guard_ai_container_users`（guard 只禁 kb 域用 `ai.document*`，kb 域建自己的表合法）。对齐 T-060 原则：索引产物绑解析规则版本号，正文变则重算。
2. **只索引现行**：索引有效性判定 = content_hash + 解析规则版本 + **revision 是 family 当前 published**（三要素，比 ai 栈多第三条——版本链带来的）。
3. **编译落点**：制度条款 → PC cognition 对象（提案链，AI 恒 draft）；套表口径/制度摘要 → kb.document（draft → 真人 published）。analysis_mode 用现枚举（`derived`/`llm`）。
4. **目录层**：聚合 family（kb.document）+ PC 对象 → 有界目录视图，进注入新一节（预算 3~5k，挂 `discovery_context` 的 CAP/DROP_ORDER 口径）——同时服务 Agent（知道该读什么）和人（浏览面）。
5. **lint 巡检**（后置但登记）：矛盾（rule_contradiction）/ 过期（revision 链上旧版仍被引用）/ 孤儿（零引用 family）三类起步，产 `pc.conflict` 候选行，真人裁决。

## 六、待裁清单

| # | 事项 | 需要什么 |
| --- | --- | --- |
| Q1 | 版本族文件名规则是否成立；全文重发 vs 修改说明的占比 | **真实文件名样本 50 个**（用户提供或授权扫目录） |
| Q2 | image caption 的预算与模型 | 每图一次 vision 调用——预算红线 + provider 选型 |
| Q3 | 财务套表 V1"口径可查"是否够用 | 用户确认使用场景（查口径 vs 查数字） |
| Q4 | 索引表落 kb 还是 pc schema（制度条款编译后两者都有正文） | 实施设计时定，倾向 kb（原文级）+ pc 对象走结构化查询 |
| Q5 | docx 解析器终选 | 用 20~30 份真实 docx 跑大纲还原率实测（pdfium 只管 pdf，docx 用纯 Rust XML 解或 LibreOffice 兜底） |
