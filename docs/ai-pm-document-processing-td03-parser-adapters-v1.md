# Document Processing · TD-03 Parser Adapter 技术设计 — V1

**上游：** 规格 §4/§7.2 + 仓库实证（knowledge_parse.rs 已有 DOCX 解析基座）
**选型总裁决：** **纯 Rust（zip + quick-xml），零外部进程、零 LibreOffice/Pandoc**——沿用 knowledge_parse.rs 裁决 Q-B 的先例（「纯 Rust ZIP+XML，零 LibreOffice」）。规格 §7.2 的 Pandoc/MarkItDown/Docling 候选**否决**：引入 Python/外部二进制违背仓内部署形态（容器镜像只有 Rust 产物），且 spec 自己要求 Adapter 隔离具体技术——我们隔离的方式就是不用外部技术。中文评测策略见 §5。

---

## 1. 统一输出契约

所有 Parser 实现同一 trait（`pm-application/src/docproc_parse.rs`）：

```rust
pub struct ParseOutcome {
    pub markdown: String,                 // 规范化 Markdown（规格 §4.3）
    pub images: Vec<ExtractedImage>,      // 二进制已提取，待 worker 落 file-service
    pub tables: Vec<TableInfo>,           // 表格计数与保真方式（md / html_fallback / attachment）
    pub mapping: Vec<SourceMapping>,      // source_location ↔ target_location（TD-01 §⑤）
    pub warnings: Vec<ParseWarning>,      // 不可解析内容 → 警告，不静默丢弃（规格 §4.2）
    pub report: ParseReport,              // 章节数/表格数/图片数/OCR页数/完整性
}
pub struct ExtractedImage { pub data: Vec<u8>, pub ext: String, pub source_location: Value, pub alt: Option<String> }

pub trait ParserAdapter: Send + Sync {
    fn name(&self) -> &'static str;             // docx / pdf / pptx / xlsx / html / txt / csv / md
    fn version(&self) -> &'static str;          // adapter 自身版本（写 job.parser_version）
    fn supports(&self, ext: &str, magic: &[u8]) -> bool;   // 扩展名 + 魔数双检
    fn parse(&self, ctx: ParseCtx) -> Result<ParseOutcome, ParseError>;
}
```

**Markdown 规范化规则（全 adapter 一致）：**
- 标题统一 ATX（`#`），层级 = 原文大纲层级（不因跳级重排，保真优先）。
- 表格 → GFM 管道表；合并单元格不可表达 → 整表 HTML `<table>` fallback 或截断警告（规格 §4.5）。
- 图片 → `assets/image-NNN.ext` 相对引用（下载路径经 docproc API 解析）；提取失败 → 警告 + 占位引用。
- 不改写语义、不删重、超链接保留目标（规格 §4.3 八条全落）。

## 2. 各格式解析策略

### 2.1 DOCX（复用并升级 knowledge_parse 基座）
- 容器：`word/document.xml`（主体）+ `word/_rels/document.xml.rels`（关系）+ `word/styles.xml`（样式→大纲级别）+ `word/media/*`（图片）。
- 结构：body 顺序遍历 `w:p`/`w:tbl`；标题 = pStyle→HeadingN 映射（knowledge_parse 已有 `style_to_heading`，抽公共）；编号列表识别 `numPr` → 有序/无序列表。
- 表格：`w:tbl`→行/单元格文本；`gridSpan/vMerge` 合并单元格 → 单元格留空 + 警告（V1 拍平，P1 HTML fallback）；表格前后文保序输出。
- 图片：`w:drawing`→`a:blip r:embed`→rels→media 文件字节提取；记录段落位置。
- 页码：DOCX 无固定分页 → source_location 用「段落序号」（`{paragraph: 42}`），mapping_type=`paragraph_to_section`。

### 2.2 XLSX
- 容器：`xl/sharedStrings.xml` + `xl/workbook.xml`（sheet 清单）+ `xl/worksheets/sheetN.xml` + `xl/worksheets/_rels`（超链接）。
- 策略：每个 worksheet → 一个 `## Sheet: <名>` 章节；`row`→管道表行；首行按表头样式（有值且后续行引用其列）→ 表头行；`t="s"` 单元格经 sharedStrings 解索引。
- 公式：单元格存公式时读缓存值 `<v>`，公式文本记 warning（规格「公式信息」）。
- 合并单元格 `mergeCells` → 首格取值、余格空 + 警告。
- 映射：`{sheet: "名称", row: 12}` → `{heading: "## Sheet: X", table_ref}`。

### 2.3 PPTX
- 容器：`ppt/presentation.xml`（幻灯片顺序）+ `ppt/slides/slideN.xml` + `ppt/slides/_rels` + `ppt/media/*`。
- 策略：每页 → `## Slide N: <标题占位文本>`；`p:sp` 内 `a:t` 文本按 shape 顺序合并段落；`p:pic`→图片提取（rels→media）；备注 `notesSlide` → 引用块（`> `）。
- 映射：`{slide: 3}` → `{heading: "## Slide 3"}`（规格 §6.5 示例）。

### 2.4 PDF（自研最小提取器，零新依赖）
- 依赖面：flate2（已在依赖树：zip→deflate）+ 自写对象/流解析。**不引 pdf-extract/lopdf**（依赖树重、Windows 工具链风险；U-03 评测后若自研不达标再立切片换库）。
- 三级降级（D-04 的 V1 落地）：
  1. **结构扫描**：trailer/xref（含 xref stream）→ page tree → 每页 Content Stream inflate。
  2. **文本提取**：`BT..ET` 块内 `Tj/TJ/'/"` 字符串，处理 `Td/TD/Tm` 行向量近似分行；编码三层：WinAnsi/Standard 直映射、`ToUnicode` CMap（`beginbfchar/beginbfrange` 逐条解析，覆盖中文常见单/双字节映射）、Type0+Identity-H 逐 UTF-16BE 码点。**提取率不达标的字体 → 字符降级 U+FFFD + warning（不静默丢字）**。
  3. **页面判定**：每页文本长度≈0 且含图片 XObject → `ocr_required: true` 页清单（规格 D-04 V1 五项能力）；全文档无文本 → PARTIAL_SUCCESS + `needs_ocr` warning + 页清单进 report。
- 页码映射天然具备：`{page: 5}` → `{heading: "## 5"}`；Markdown 按页分节（`## Page N`）。
- 加密 PDF（`/Encrypt`）→ 409 `encrypted_document`。

### 2.5 HTML / TXT / CSV / MD
- HTML：quick-xml 流式；`h1..h6/p/ul/ol/li/table/img/a`；内联样式丢弃，`<script/style>` 跳过；外链资源不抓取（安全边界）。`srcdoc` 外部资源加载天然不存在（无浏览器）。
- TXT：按空行分段。
- CSV：RFC4180（引号转义、CRLF）；首行表头 → 管道表；大文件（>10k 行）→ 截断 + 警告。
- MD：原样读取 + 结构解析（标题树提取供 mapping）。

## 3. 共享工具层

| 工具 | 位置 | 说明 |
|---|---|---|
| zip 容器打开 + 大小守卫 | docproc_parse.rs | 单文件解压比 ≤100x、总解压 ≤512MiB（ZIP Bomb，TD-04 §4） |
| OOXML 共享：XML 文本清洗/转义还原 | 同上 | `&amp;` 实体、`xml:space`、空白归一 |
| knowledge_parse 的 docx 走读逻辑 | 抽取复用 | 现有 207 行基座升级为 docproc adapter，**knowledge_parse 保持原 API**（KB 导入不迁库，避免动既有测试基线） |
| Markdown 分节索引 | 同上 | 产 `target_location` 行区间（mapping 用） |

## 4. 评测与验收（U-03 的 V1 落法）

**合成语料基准**（tests 仓库夹具，中英混合）：
| 用例 | 断言 |
|---|---|
| docx：三级标题 + 中英混排 + 三行表格 + 2 图 + 编号列表 | 标题层级 100%、表格行列数一致、图片字节 sha256 与源一致、列表类型正确 |
| docx：中文样式名（标题 1/标题 2，WPS 导出） | 依据 styles.xml 的 outlineLvl 兜底（不依赖样式名拼写） |
| xlsx：2 sheet + 合并单元格 + 公式缓存值 + 中文表头 | sheet 章节、合并首格取值、公式取缓存值 + warning |
| pptx：5 页含标题/正文/图/备注 | 页序、每页标题、备注引用块 |
| pdf：WinAnsi 英文 / ToUnicode 中文 / 扫描件 | 英文全提、中文 ≥95% 码点还原（U+FFFD 计数<5%）、扫描件报 needs_ocr |
| csv/html/md | 行列保真、标题树 |
| zip bomb（高压缩比 docx） | 解压守卫拒绝 → FAILED `archive_bomb` |
| 加密 docx（ECMA-376 Standard 加密容器） | 409 `encrypted_document` |

**真实中文文档评测（U-03）**：WPS/Word 各导出一份真实中文需求文档 + 扫描 PDF，人工比对标题/表格/正文完整率，结果记入本文档 §6 评测记录（P0 验收前补）；不达标项 → P1 adapter 升级，版本号 +1 重转（D-06 历史产物保留）。

## 5. 不做清单（V1）
- 旧版二进制 DOC/XLS/PPT（规格 §4.1.1 允许：外部转换属用户侧责任）。
- PDF 图片提取/OCR 引擎（P1，ocr_required 清单已产出）。
- 脚注/尾注（DOCX `footnotes.xml` → 整体警告占位）。
- 复杂合并单元格 HTML 保真（拍平 + 警告）。
