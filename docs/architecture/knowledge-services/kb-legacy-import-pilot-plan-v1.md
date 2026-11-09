# KB 存量导入试点方案 V1（草案）

> **状态**：草案（2026-11-09 搁置：存量文件导入专项暂不推进，优先裁决知识层模式选型，见 `kb-pc-knowledge-layer-model-selection-v1.md`；本稿保留作未来参考）。
> **背景**：首发目标场景 = 公司存量项目（约 2GB、数千份 Word 文档）。这不是"未来某天"的规模——**发布后第一批使用者就是存量项目**。
> **定位**：本稿不是终态设计。是用 **100~200 份真实文档**换**四个数字**的试点执行方案；四个数字反过来裁决 D1（导入模型）/ D3（检索对象）/ D5（解析器）。
> **关联**：KP-3 验收记录（知识检索未建）· `kb-pc-independent-services-architecture-v1.1.md` · `ai-pm-markdown-knowledge-space.md` · llm_wiki 参考研究（2026-11-09 会话）

---

## 一、试点要回答的四个问题

| # | 数字 | 裁决什么 | 测法 |
| --- | --- | --- | --- |
| N1 | **解析吞吐与失败率**（篇/分钟；失败原因分类） | D5 解析器选型；2GB 外推总解析时长 | 三条解析路线跑同一批样本 |
| N2 | **LLM 成本/篇**（triage + 编译两步 CoT，token 与金额） | D1 全量导入预算（3000 篇外推总价） | 调用记录按篇归集 |
| N3 | **编译墙钟/篇**（含并行度影响） | D1 全量导入时长外推（串行 vs 并行 2~4） | 分组计时 |
| N4 | **召回@5 基线**（50 个人工标注问题） | D3 检索对象；后续一切检索调优的起点 | 标注集 × 试点索引 |

附带产出（不设阈值，作为观察项）：L1 triage 与人工抽检的一致率；编译产物的人工"有用/没用"评价分布。

### 1.1 不变式（试点全程不可破）

1. **AI 只能提案，产物恒 draft，真人确认后才生效**（Q2-C / V-6 写入纪律）。试点的人工确认可以是脚本化低科技形态（见 §6.4），但门必须在。
2. **原始资料不可变**：`kb.source` 内容不可改写触发器已锁；解析产物写**独立试点目录**，永不回写源文件。
3. **租户隔离 + 审计**：一切入库走现有 fail-closed 中间件链；试点脚本用真实主体身份落审计。
4. **零正式写入面改动**：不新建正式迁移表（试点元数据走文件，见 §6.5）、不动 `guard_ai_container_users`、不动 `ai.document*`。

---

## 二、样本选取口径

**总量 100~200 份**，分层抽样，配额如下（按实际库调整，配齐为原则）：

| 维度 | 配额要求 |
| --- | --- |
| **格式** | docx ≥60%、doc（老格式）≤10%、pdf ≤15%、xlsx/ppt ≤10%、其他 ≤5%——配比贴近真实库分布 |
| **年代** | 近 1 年 ~40%、1~3 年 ~35%、3 年以上 ~25%——**老文档必须占四分之一**，时效性问题是试点必答题 |
| **类型** | 设计文档 / 规范制度 / 会议纪要 / 需求 / 总结汇报，各 ≥15 份 |
| **长度** | <5 页 ~30%、5~20 页 ~50%、>20 页 ~20%（含 1~2 份超长，测截断行为） |
| **脏样本** | 刻意含 3~5 份：加密/损坏文件、扫描版 PDF、GBK 编码、巨表——失败率数字必须含这类 |

**meta 清单**（试点全部元数据的第一张表，CSV/Excel）：

```
source_id, 文件名, 原始路径, 格式, 大小bytes, 文件mtime, 疑似类型, 疑似归属业务域, 备注
```

---

## 三、存储分层（试点内最小落地，D2 的试试点形态）

- **原始文件不动**：留在现有共享盘/NAS，试点脚本只读。`kb.source` 以 `uri` 登记（`file://` 或 UNC 路径），**不搬运、不复制入库**——对象存储选型归试点后（2GB 级别的正式落点另行裁决）。
- **登记即去重**：`kb.source` 的 `UNIQUE(tenant_id, content_hash)` 团队池天然吃掉重复文件，试点顺带测出**真实重复率**（第六个观察数字）。
- **闭合声明 hash 缺口**：KP-3 遗留"文件类来源走 uri+声明 hash，拉取重算比对归 Phase 4"——试点 L0 **解析前重算 sha256 并与登记值比对**，不一致记失败（分类：`hash_mismatch`）。这是试点对正式代码的第一个可反哺点。
- **`inline_content` 禁用**：`MAX_INLINE_BYTES=1MB` 本来就装不下正文级文件；试点全部走 uri。
- `source_kind` 枚举已覆盖：`docx`/`pdf`/`markdown`/`other`（xlsx/ppt 落 `other`，试点如实记）。

---

## 四、解析层选型（D5，试点裁决）

三条路线**全部**跑同一批样本，出对比表后裁决，不在纸面选：

| 路线 | 覆盖 | 优势 | 风险/成本 | 试点角色 |
| --- | --- | --- | --- | --- |
| **A. 纯 Rust**（docx=unzip+XML 手解或 docx-rs；pdf=pdfium，llm_wiki 同款） | docx 好、pdf 好、**doc/xls/ppt 不行** | 进程内、可嵌入 pm-api、无外部依赖 | docx 表格/公式/批注还原度需实测；pdfium 需带动态库 | 主候选 |
| **B. LibreOffice headless**（soffice --convert-to txt/html/md） | **格式覆盖最广**（doc/xls/ppt 全吃） | 老格式唯一现实解 | 重（进程外、并发差、容器镜像 +400MB）；样式还原一般 | 老格式兜底，与 A 组合 |
| **C. 云解析/MinerU 类** | 广 | 免运维 | 外发数据 = 红线（公司存量文档**不可外发**） | **否决，不出 now** |

**试点结论形态**：成功率先按格式分列（docx / doc / pdf / 扫描 pdf / 加密），再给组合建议（预期 = A 为主 + B 兜 doc 老格式）。扫描 PDF 的 OCR **明确出试点范围**（登记 `parse_failed:scanned_pdf` 即可，OCR 是否建归试点后）。

**解析产物规范**（L0 输出 = 试点目录内一个 normalized markdown + sidecar）：

```
{pilot_dir}/parsed/{source_id}.md          # 标题层级保留（# 对齐 Word 大纲级别）
{pilot_dir}/parsed/{source_id}.meta.json   # 解析器、耗时、页数、图片数、失败原因、警告
```

标题层级是硬要求：**下游章节索引（db-section 同款解析）完全依赖 `#` 层级**，Word 大纲级别丢失 = 检索退化成全文匹配。解析器必须报告"大纲级别还原率"。

---

## 五、L0/L1/L2 三层管线

### 5.1 L0：解析与登记（廉价、全自动）

```
扫共享盘 → sha256 → POST /v1/kb/sources（uri 模式，去重免费）
        → 解析（§四）→ normalized md + meta
        → 失败登记（失败也是产出：分类必须结构化）
```

失败分类枚举：`hash_mismatch` / `unsupported_format` / `encrypted` / `corrupted` / `scanned_pdf` / `timeout` / `outline_flat`（无大纲级别，降级警告不失败）。

### 5.2 L1：triage（每篇一次廉价 LLM 调用 + 规则先滤）

规则先滤（零成本）：字数 < 200 → `archive_only`；纯表格/图（文本抽出率 < 10%）→ `human_review`。

LLM triage 一次调用，输出结构化 JSON：

```json
{
  "doc_type_guess": "design|spec|meeting|requirement|report|other",
  "business_domain": "<疑似业务域>",
  "freshness_signal": "current|aging|stale|unknown",
  "compile_value": "high|medium|low",
  "key_topics": ["..."],
  "reason": "<一句话>",
  "suggested_action": "compile_l2|archive_only|human_review"
}
```

**判据草案**（试点验证用，阈值跑完再定）：
- `compile_value=high`：现行规范/活设计/独有信息（库中唯一讲这块的）
- `compile_value=low`：被新版覆盖的旧版、纯流程通知、重复内容
- `freshness_signal` 结合 meta 的 mtime 与文中自述日期
- 抽检：≥30 份人工复核 triage 结论，出一致率（观察项，无阈值）

### 5.3 L2：编译（两步 CoT → ai-pm 形态）

llm_wiki 的两步链收编为 ai-pm 语义：

| llm_wiki | ai-pm 试点形态 |
| --- | --- |
| Step1 分析（实体/矛盾/连接建议） | **分析草案**：结构化事实 + 与现有知识的张力点 |
| Step2 生成（改 wiki 页面） | **知识草案 markdown**：只写产物，不落正式表 |
| `sources[]` frontmatter | **source 锚**：`kb.source` id + 路径 + hash（溯源对齐 `pc.evidence` 精神） |
| LLM 直写 wiki | **草案目录**：`{pilot_dir}/drafts/{source_id}.md` + 状态字段，**不落任何正式表** |
| 无门禁 | **人工确认门**（§6.4），确认后才落 `kb.document_revision`（draft status，对齐 `analyze_repository_snapshot` 先例：分析草稿以 `kb.document_revision` 草稿落库、等待人工确认） |

并行度：**分组并行 2~4**。分组键 = 疑似业务域（meta 清单人工标注），同组内串行保序（后文档能看到先文档草案的"已编译知识"摘要，模拟增量累积语义），组间并行抢吞吐。试点实测串行 vs 并行 2 vs 并行 4 三档（N3）。

矛盾处理：Step1 发现与已有草案矛盾 → **只登记**进 `{pilot_dir}/conflicts.csv`（draft 级冲突，对齐 `pc.conflict` 类型枚举做首版分类），不触发正式冲突流程。试点结束统计矛盾密度，裁决 L2 编译时"跨文档一致性"要做到什么程度。

---

## 六、接口与落点（试点内新增面的形状）

### 6.1 L0 批量登记
直接循环调现有 `POST /v1/kb/sources`，**零新端点**。试点脚本（Rust bin 或 shell，放 `scripts/legacy-import-pilot/`）负责遍历、hash、比对、调 API。

### 6.2 解析 worker
试点独立进程（不进 pm-api）：读共享盘 → 解析 → 写 `{pilot_dir}/parsed/`。理由：解析器选型未定，先不把它焊进服务；试点裁决后再谈工程化（进程内 lib vs sidecar 容器）。

### 6.3 triage / 编译调用
走现有 team LLM provider 通道（`team_llm` 已有真实 Key 面(provider_key enabled 先例)），调用记录落 `ai.llm_call_record`（purpose 标 `legacy_pilot_triage` / `legacy_pilot_compile`）——**成本归集（N2）直接查这张表，零新表**。

### 6.4 人工确认门（低科技形态，红线不破）
- 草案 = `{pilot_dir}/drafts/` 里的 markdown 文件 + `status` 字段（`pending|approved|rejected|needs_revision`）。
- 评审人（项目 Lead 或指定 PM）直接读文件，把结论填进 `drafts/review.csv`。
- 确认脚本只做一件事：`approved` 的草案以 **draft status** 插入 `kb.document_revision`（复用 `kb_pc_analysis` 的落库路径与同事务 outbox 语义），insert 主体 = 评审人真实 principal（审计落真人）。
- **不建 UI**。UI 归试点后的正式提案面。

### 6.5 试点元数据全部走文件，零新迁移表
meta / triage / review / conflicts / 计时全部为 `{pilot_dir}` 下 CSV/JSON。理由：试点结论是"判据与阈值"，不是表结构；正式表（triage 结果落哪、权威标注列）等试点裁决后随正式迁移（自 **0003** 起，0001/0002 已占）一次设计对。唯一入库的正式数据 = `kb.source` 行 + 确认后的 `kb.document_revision` 草稿行 + `llm_call_record` 行。

### 6.6 检索面（试点自建，不动现有两栈）
- **不动** `ai.document_section_index`（guard 红线：新 KB 域禁用 `ai.document*`）。
- 试点索引 = `{pilot_dir}/index/` 下自建章节索引文件（**复用 `knowledge_index.rs` 纯函数**解析 Markdown → 章节），检索 = 试点脚本内存查询（CJK bigram + 标题加权，抄 llm_wiki 打分常量：TITLE_TOKEN_WEIGHT=5 / CONTENT_TOKEN_WEIGHT=1 / 文件名命中加成）。
- 这是对 T-060"未来 provider 自带派生表"原则的一次演练：**索引产物与解析规则绑定试点版本号**，正文变了重算。

---

## 七、评估集与召回基线（N4）

**50 个标注问题**，模板：

```
qid, 问题（自然语言）, 期望命中的 source_id 列表（1~5 篇）, 问题类型（事实查找|汇总|对比|定位）, 出题依据文档
```

出题规则：
- ≥20 题从样本里**选定一篇文档**出"这篇讲什么/某细节"（单文档命中）
- ≥15 题跨 2~3 篇（汇总/对比类）
- ≥10 题含时间限定（"现在的 X 是什么"——测时效性检索，预期暴露老文档污染）
- ≥5 题答案在**未入选样本**的文档里（测"知识缺口"行为，预期 miss）

测法：`db-section` 同款章节索引（§6.6 试点版）跑全量，输出 recall@1/@5 + 失败分类（`知识不对` / `找不到` / `找到没读`）。**N4 是基线不是及格线**——它的作用是让之后的 RRF/向量/图扩展有对照。

---

## 八、时间盒与验收门

| 周 | 内容 | 出口 |
| --- | --- | --- |
| W1 前半 | 样本抽取 + meta 清单 + 共享盘只读挂载 + 三解析器环境就绪 | meta 清单冻结 |
| W1 后半 | L0 全量跑（三路线 × 全样本）+ 失败分类表 | **N1 出数** |
| W2 前半 | L1 triage 全量 + 30 份人工抽检 + L2 试点编译（高价值子集，预计 30~60 篇） | **N2/N3 出数** |
| W2 后半 | 50 题标注 + 召回基线跑 + 报告 | **N4 出数** + 试点报告 v1 |

**验收门（go/no-go 判据，试点报告里逐项答）**：

| 门 | 通过 | 不通过的处理 |
| --- | --- | --- |
| 解析 | docx 成功率 ≥85%（不含脏样本刻意失败） | D5 重选型（B 上升为主） |
| 成本 | 3000 篇外推总价 < 预算红线（红线由用户定，建议先按 ¥500 档假设） | L1 判据收紧 compile_value=high 口径 |
| 时长 | 全量导入外推 ≤ 5 个工作日 | 并行度上调 / L2 范围收窄 |
| 召回 | **无阈值**——基线本身即交付 | —— |

**试点不做的事（红线）**：向量库 / pgvector、知识图谱、前端任何页面、正式迁移表、`guard`/`ai.document*` 改动、OCR、对象存储选型、K1/K3 解锁。全归试点后。

---

## 九、试点后裁决清单（试点报告的决策表）

| 决策 | 输入数字 | 选项 |
| --- | --- | --- |
| D1 导入模型 | N2/N3 + 重复率 | 全量编译 / triage 分层（L2 只编 high）/ 人工 curate |
| D3 检索对象 | N4 失败分类 | 纯编译层 / 纯原始层 / 混合（原始兜底 + 编译增值） |
| D5 解析器 | N1 | A / A+B 组合 / 其他 |
| D4 权威标注 | 50 题中时效题表现 + triage freshness 一致率 | `kb.source` 扩列（status/authority/dept）的正式迁移设计 |
| 检索索引归属 | 试点自建索引实测 | kb 域正式索引表设计（对齐 T-060 provider 端口） |
| 注入接线 | 编译产物质量评价 | 目录层 + `knowledge_digest`（S3，`S3_DOC_LIMIT=5`/CAP 12000）扩容方案 |

---

## 十、立即可反哺正式代码的三个点（试点中顺手做）

1. **hash 重算比对**（§三）：补 KP-3 挂账"拉取重算比对归 Phase 4"的第一半。
2. **`knowledge_index.rs` 纯函数复用验证**：试点索引直接 import 该模块，暴露其作为跨栈复用件的缺口（如有）。
3. **llm_wiki 增量跳过**：试点脚本天然按 sha256 跳过已解析文件——为正式 ingest 的增量缓存积累真实命中率数据。
