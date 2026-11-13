# Document Processing V1.0
## 产品需求与技术架构设计

**所属系统：** PM-AI / smark-pm  
**文档版本：** V1.0  
**修订版本：** V1.0-R1  
**文档状态：** 架构裁决已完成，技术设计待细化  
**裁决状态：** D-01～D-07 全部通过  
**适用范围：** PM-AI 平台级基础设施

---

# 1. 产品定位

## 1.1 建设目标

Document Processing 是 PM-AI 的平台级文档处理与内容转换基础设施。

它负责将外部文件转换为系统可管理、可追溯、可供 AI 理解和处理的标准化内容，并为知识管理、产品认知、项目认知、需求管理、设计管理等业务模块提供统一的文档处理能力。

核心目标：

1. 支持常见办公文档和技术文档的解析与转换。
2. 尽可能保留原始文档的语义结构。
3. 建立原始文件、转换产物和业务知识之间的可追溯关系。
4. 支持 AI 对文档内容进行理解、分析、提取和比较。
5. 为 Markdown + Git 知识管理体系提供标准化输入。
6. 提供统一、可扩展、可复用的文档处理 API。

## 1.2 产品边界

Document Processing 负责：

- 文件接收和格式识别。
- 文档内容提取。
- 文档格式转换。
- 文档结构规范化。
- 图片、表格和附件处理。
- 文档版本比较。
- 转换任务管理。
- 转换结果及来源追溯。
- 文档导出。

Document Processing 不负责：

- 判断文档内容是否为正式需求。
- 判断内容是否属于 Product Cognition。
- 判断内容是否属于 Project Cognition。
- 自动确认正式知识。
- 维护业务知识的权威版本。
- 决定需求和设计的业务审批状态。
- 承担 PageIndex 的检索职责。

核心原则：

**Document Processing 处理文档，不裁决文档中的业务知识。**

# 2. 已确认的架构基线

以下七项已经完成正式架构裁决，后续产品、数据库、API 和实施设计必须遵守。

| 裁决 | 确认方案 | 架构约束 |
|---|---|---|
| D-01 | A | Team 级 Document，支持可选 Project 关联 |
| D-02 | A | 原始文件与转换产物统一使用对象存储 |
| D-03 | A | 独立 Document Processing Service，内部 Adapter + Worker |
| D-04 | A | V1 建立 OCR 接口，OCR 引擎放入 P1 |
| D-05 | A | 正式知识只能由对应业务领域完成确认 |
| D-06 | A | 原始文件版本与转换产物版本分别管理 |
| D-07 | A | 文档始终是不可信输入，解析隔离，AI 独立防护 |

## 2.1 D-01：Document 的归属范围

**正式裁决：A。**

Document 是 Team 级基础文档实体，支持可选 Project 关联。

具体要求：

1. Document 必须归属一个 Team。
2. Document 可以不关联任何 Project。
3. Document 可以关联一个 Project。
4. Team 级资料可以被项目初始化、KB、PC 等模块使用。
5. Project 关联必须经过 Team 一致性校验。
6. Document 的访问权限首先受 Team 边界约束，再受 Project 和业务资源权限约束。

不采用必须归属 Project 的设计。

这样可以支持团队共享资料、待分类资料以及尚未创建项目时导入的文档。

## 2.2 D-02：原始文件与转换产物存储

**正式裁决：A。**

原始文件与转换产物统一使用对象存储。

包括：

- 原始 DOCX、PDF、PPTX、XLSX。
- 转换后的 Markdown。
- 提取出的图片。
- OCR 结果。
- 表格数据。
- 转换报告。
- 导出文件。
- 文档差异报告。

PostgreSQL 管理元数据、状态、版本和关联关系。

正式 KB/PC 知识正文由对应知识领域自行管理，不归 Document Processing 管理。

特别约束：

转换生成的 Markdown 不自动进入正式知识 Git 仓库。

当业务模块决定将其作为正式知识时，由业务模块负责后续的 Git 路径、版本、确认和审计。

## 2.3 D-03：服务部署方式

**正式裁决：A。**

采用独立 Document Processing Service。

内部采用：

- Parser Adapter。
- Processing Dispatcher。
- Processing Worker。
- Artifact Manager。
- Conversion Engine。

业务模块通过统一 API 调用，不直接依赖具体解析器。

独立服务边界已经确定。

但具体部署拓扑、服务通信协议、队列选型仍属于后续技术细化事项。

## 2.4 D-04：OCR 的 V1 范围

**正式裁决：A。**

V1 建立 OCR 接口和扩展能力，但完整 OCR 引擎列入 P1。

V1 必须支持：

1. 判断 PDF 是否包含可提取文本。
2. 检测疑似扫描页面。
3. 识别需要 OCR 的页面。
4. 生成 OCR 处理任务所需的数据结构。
5. 记录 OCR 状态和处理结果。
6. 为后续 OCR Adapter 预留统一接口。

V1 不要求完整部署 OCR 引擎。

P1 增加 OCRmyPDF 或其他经过评估的 OCR 实现。

## 2.5 D-05：正式知识确认权限

**正式裁决：A。**

Document Processing 没有正式知识确认权。

正式知识必须由对应业务领域确认。

例如：

| 内容 | 确认责任方 |
|---|---|
| 通用项目知识 | KB |
| 产品定位、业务规则、产品能力 | Product Cognition |
| 技术架构、技术约定 | Project Cognition |
| 正式需求 | Requirements |
| 正式技术设计 | Design |

AI 可以：

- 分析文档。
- 提取候选知识。
- 识别潜在需求。
- 识别业务规则。
- 生成建议。
- 提出待确认问题。

但不能因为转换成功而自动确认正式知识。

## 2.6 D-06：原始文件版本与转换产物版本

**正式裁决：A。**

必须分别管理。

原始文件版本：

表示用户上传的文件内容发生了变化。

转换产物版本：

表示特定原始文件版本在特定转换器、转换配置下生成的产物。

例如：

```text
Document
  |
  +-- Source Version 1
  |      |
  |      +-- Markdown Artifact A
  |      +-- PDF Extraction Artifact B
  |
  +-- Source Version 2
         |
         +-- Markdown Artifact C
         +-- PDF Extraction Artifact D
```

转换器从 V1.0 升级到 V1.1，不代表原始文件出现了新版本。

历史转换产物应保留，以支持：

- 转换结果比较。
- 质量回归。
- 转换器升级验证。
- 问题追踪。
- 原始内容复现。

## 2.7 D-07：文档安全边界

**正式裁决：A。**

所有上传文档始终视为不可信输入。

解析阶段：

- 文件类型检查。
- 病毒扫描。
- Parser 隔离执行。
- CPU、内存和时间限制。
- 防止 ZIP Bomb。
- 禁止不必要的外部网络访问。
- 文件路径安全处理。

AI 分析阶段：

- 文档内容不具备系统指令权限。
- 文档内嵌的提示注入不能改变 Agent 的系统规则。
- 不允许仅凭文档内容自动执行外部工具操作。
- AI 分析使用独立的安全策略。
- 敏感内容和权限边界由业务系统控制。

文档解析安全与 AI 输入安全是两层独立防护，不能互相替代。

# 3. 与 PM-AI 现有架构的关系

## 3.1 平台定位

Document Processing 是横向基础设施能力，不隶属于任何单独的业务模块。

主要消费者：

| 消费模块 | 使用场景 |
|---|---|
| KB | 知识资料导入、转换、附件管理 |
| Product Cognition | 产品资料、业务文档导入 |
| Project Cognition | 项目技术资料、架构文档导入 |
| Requirements | Word/PDF 需求文档解析 |
| Design | 设计文档、架构方案导入 |
| Project Management | 项目初始化资料分析 |
| AI-PM | AI 分析材料、生成正式文档 |
| MCP | 外部工程工具提交文档和资料 |

## 3.2 权威关系

| 数据类型 | 权威归属 |
|---|---|
| 原始文件 | Document Processing 的原始文件管理体系 |
| 转换产物 | Document Processing 的派生产物管理体系 |
| 正式 KB 知识正文 | KB |
| 正式产品认知 | Product Cognition |
| 正式项目认知 | Project Cognition |
| 正式需求 | Requirements |
| 正式设计 | Design |
| 元数据和任务状态 | PostgreSQL |
| 检索索引 | PageIndex 等派生索引 |

Document Processing 不能成为 KB 或 PC 的第二套知识权威。

# 4. 功能需求

## 4.1 文件格式支持

### 4.1.1 导入格式

| 格式 | 扩展名 | V1 支持能力 |
|---|---|---|
| Word | DOCX | 正文、标题、列表、表格、图片 |
| PDF | PDF | 文本、页面、表格、图片 |
| PowerPoint | PPTX | 幻灯片文本、标题、表格、图片 |
| Excel | XLSX | 工作表、单元格、表格、公式信息 |
| Markdown | MD | 原样读取及结构解析 |
| HTML | HTML/HTM | 正文、标题、链接、表格 |
| 纯文本 | TXT | 文本提取 |
| CSV | CSV | 表格数据解析 |

V1 不要求支持旧版 DOC、XLS、PPT 二进制格式的完整解析。可以通过外部格式转换器先转为 OOXML 格式。

### 4.1.2 导出格式

V1：

- Markdown。
- DOCX。
- PDF。
- HTML。
- CSV。
- ZIP（包含 Markdown、图片及附件）。

P2：

- PPTX。
- 更复杂的 Office 排版和模板生成。

## 4.2 文档解析

系统需要提供统一的解析结果结构。

支持提取：

1. 文档标题和章节。
2. 正文段落。
3. 标题层级。
4. 有序及无序列表。
5. 表格。
6. 图片及其引用位置。
7. 超链接。
8. 文档元数据。
9. 页码及幻灯片编号。
10. 脚注和尾注（适用格式）。

对于暂时无法正确解析的内容，应当保留原始附件并产生解析警告，而不是静默丢弃。

## 4.3 Markdown 规范化

所有适合转换为结构化文本的文档，应支持生成规范化 Markdown。

规范：

- 标题统一使用 Markdown 标题语法。
- 保留合理的标题层级。
- 表格转换为 Markdown 表格。
- 图片以资源引用方式保存。
- 超链接保留目标地址。
- 保留必要的原始文档标识。
- 不擅自修改原文语义。
- 不擅自删除看似重复的业务内容。
- 无法可靠转换的内容应保留为附件或转换警告。

示例：

```markdown
# 产品需求

## 1. 用户管理

系统应支持多个团队成员。

### 1.1 用户邀请

支持邮件邀请和邀请链接。

## 2. 权限管理

| 角色 | 权限 |
|---|---|
| Admin | 管理团队 |
| Member | 使用项目 |
```

转换产物可以附带机器可读的来源元数据，但不得将 Document Processing 的内部元数据强制写入所有正式知识正文。

## 4.4 图片及附件处理

图片必须独立保存，不应直接丢弃。

要求：

- 保留图片原始二进制。
- 为图片分配唯一资源 ID。
- Markdown 使用相对路径引用。
- 记录图片在原文中的位置。
- 尽可能保留图片说明。
- 支持图片预览。
- 支持后续 OCR 和视觉分析。

建议产物结构：

```text
document/
  source.docx
  content.md
  assets/
    image-001.png
    image-002.jpg
  conversion-report.json
```

以上为逻辑产物结构，物理存储统一使用对象存储。

## 4.5 表格处理

表格是 PM-AI 文档转换的重点。

V1 支持：

- 普通表格。
- 多行、多列。
- 单元格文本。
- 表头识别。
- 合并单元格的尽可能保真处理。
- Excel 工作表及区域提取。

对于复杂合并单元格，Markdown 无法完整表达时，应采用 HTML 表格或保留原始表格附件。

不能为了生成 Markdown 而破坏表格语义。

## 4.6 OCR

V1 建立 OCR Adapter 接口，完整 OCR 引擎放入 P1。

OCR 处理结果需要：

- 记录页面编号。
- 记录识别文本。
- 记录识别质量。
- 记录识别引擎及版本。
- 保留原始页面关联。
- 支持后续重新处理。

OCR 识别结果不能视为绝对准确的原文。

## 4.7 文档比较

V1 建立统一比较接口，支持：

- Markdown 与 Markdown。
- DOCX 转换结果与 DOCX 转换结果。
- PDF 转换结果与 PDF 转换结果。
- 不同格式文档经过规范化后比较。

差异类型：

- 新增内容。
- 删除内容。
- 修改内容。
- 表格变化。
- 图片变化。
- 章节结构变化。

输出结构化差异报告。

文档比较只识别内容差异，不自动判断需求是否发生正式变更。

## 4.8 文档生成与导出

支持将系统中的 Markdown 内容导出为 DOCX、PDF 和 HTML。

导出时支持：

- 标题样式。
- 表格样式。
- 图片。
- 页眉页脚（基础模板）。
- 文档目录。
- 文档标题和版本信息。

V1 不追求完整复刻复杂 Word 排版。

# 5. 文档处理工作流

## 5.1 标准处理流程

```text
用户上传文件
      |
      v
文件安全检查
      |
      v
格式识别
      |
      v
创建处理任务
      |
      v
调用对应 Parser
      |
      v
内容提取
      |
      v
结构规范化
      |
      v
Markdown / 附件生成
      |
      v
转换质量检查
      |
      v
保存处理产物
      |
      v
通知业务模块
      |
      v
AI 分析 / 人工确认
      |
      v
进入相应业务流程
```

## 5.2 任务状态

| 状态 | 说明 |
|---|---|
| PENDING | 等待处理 |
| PROCESSING | 正在处理 |
| COMPLETED | 成功完成 |
| PARTIAL_SUCCESS | 部分成功 |
| FAILED | 处理失败 |
| CANCELLED | 已取消 |

任务状态由 PostgreSQL 管理。

转换产物和原始文件不应因任务失败而被错误删除。

## 5.3 异常处理

典型情况：

- 文件损坏。
- 文件格式不匹配。
- 文件加密。
- 文件超过大小限制。
- Parser 处理超时。
- OCR 失败。
- 图片提取失败。
- 表格转换质量不足。
- 存储服务异常。

要求：

1. 错误分类。
2. 保留错误详情。
3. 支持安全重试。
4. 避免重复生成相同产物。
5. 允许部分成功。
6. 不影响原始文件。

# 6. 数据模型设计

Document Processing 建立独立 Domain。

以下是逻辑模型，不代表最终数据库表名已经冻结。

## 6.1 Document

原始文档实体。

建议字段：

- id
- team_id
- project_id（可为空）
- filename
- mime_type
- file_extension
- file_size
- storage_key
- checksum
- uploaded_by
- created_at

约束：

- team_id 必填。
- project_id 可空。
- project_id 非空时，必须属于同一个 team_id。
- 文件下载必须执行 Team 和资源级权限检查。

## 6.2 Document Version

管理原始文档版本。

字段：

- id
- document_id
- version_number
- storage_key
- checksum
- source_metadata
- created_at

原始文件内容发生变化时产生新版本。

## 6.3 Document Processing Job

处理任务。

字段：

- id
- document_version_id
- processing_type
- status
- parser_name
- parser_version
- config_version
- started_at
- completed_at
- error_code
- error_detail
- created_at

## 6.4 Document Artifact

转换产物。

字段：

- id
- processing_job_id
- artifact_type
- storage_key
- mime_type
- checksum
- metadata
- created_at

产物类型包括：

- MARKDOWN
- EXTRACTED_TEXT
- IMAGE
- TABLE_DATA
- OCR_RESULT
- CONVERSION_REPORT
- EXPORT_FILE

转换器或配置升级可以产生新的 Artifact，不得覆盖历史产物。

## 6.5 Document Source Mapping

记录原文和转换内容之间的对应关系。

字段：

- id
- artifact_id
- source_location
- target_location
- mapping_type
- metadata

例如：

- PDF 第 5 页对应 Markdown 某个章节。
- PPTX 第 3 张幻灯片对应 Markdown 某个章节。
- DOCX 某个表格对应转换产物中的某个表格。

该模型为后续 AI 引用来源和知识追溯提供支持。

## 6.6 业务关联

Document Processing 不直接拥有 KB、PC、Requirement 等业务实体。

通过独立关联关系或者业务模块自身的关联表建立关系：

```text
Document
   |
   +-- KB Source
   |
   +-- Product Cognition Source
   |
   +-- Project Cognition Source
   |
   +-- Requirement Source
   |
   +-- Design Source
```

具体业务关联由对应领域负责。

# 7. 技术架构

## 7.1 总体架构

```text
             PM-AI Business Layer
    +--------------------------------+
    | KB | PC | Requirements | Design|
    +--------------------------------+
                   |
                   v
       Document Processing API
                   |
                   v
    +--------------------------------+
    | Document Processing Service    |
    |                                |
    | 1. File Detection              |
    | 2. Parser Dispatcher           |
    | 3. Content Extraction          |
    | 4. Normalization               |
    | 5. Conversion                  |
    | 6. Comparison                  |
    | 7. Export                      |
    +--------------------------------+
                   |
          +--------+--------+
          |                 |
          v                 v
     Parser Adapters    Processing Queue
          |                 |
          v                 v
    External Libraries   Workers
          |                 |
          +--------+--------+
                   |
                   v
             Object Storage
                   |
                   v
              PostgreSQL
```

Document Processing Service 是独立服务边界，内部组件可以按部署规模拆分，不要求 V1 将每个内部组件部署成独立微服务。

## 7.2 技术选型

采用成熟开源组件，通过 Adapter 模式隔离具体技术。

| 组件 | 用途 | 建议 |
|---|---|---|
| Pandoc | 文档格式互转 | 核心转换候选 |
| MarkItDown | Office/PDF 转 Markdown | 通用解析候选 |
| Docling | 复杂 PDF、表格和布局 | 增强解析候选 |
| Apache Tika | 文件识别、元数据 | 可选增强 |
| python-docx | DOCX 结构提取和生成 | Office 专项 |
| python-pptx | PPTX 解析 | Office 专项 |
| openpyxl | XLSX 解析 | Office 专项 |
| OCRmyPDF | 扫描 PDF OCR | P1 OCR 候选 |
| ClamAV | 文件病毒扫描 | 安全组件 |

推荐初始组合：

- Pandoc：格式转换。
- MarkItDown：通用文档解析。
- python-docx / python-pptx / openpyxl：复杂 Office 结构专项处理。
- Docling：复杂 PDF 解析。
- OCRmyPDF：P1 OCR 扩展。

以上组件不要求全部同时部署。最终选择需要通过样本质量测试和依赖评估。

业务模块不得直接依赖具体解析器。

## 7.3 存储设计

已裁决：

- PostgreSQL：元数据、任务、状态、关联关系。
- S3 兼容对象存储：原始文件和转换产物。
- Markdown + Git：正式知识正文权威。
- PageIndex：派生检索索引。

Document Processing 的 Markdown 转换产物存储于对象存储。

进入正式 KB/PC 后，由对应知识领域决定 Git 路径、版本和确认流程。

## 7.4 异步处理

文档解析属于可能耗时的任务，必须支持异步执行。

推荐：

- API 创建 Job。
- Worker 拉取任务。
- Parser Adapter 执行。
- 产物写入对象存储。
- PostgreSQL 更新任务状态。
- 业务模块接收完成事件。

V1 可以评估复用现有任务基础设施，但 Document Processing 必须保持独立服务边界。

不建议为了文档转换单独引入复杂的分布式工作流平台。

# 8. API 设计

统一 API 前缀：

`/api/v1/document-processing`

## 8.1 上传文件

`POST /documents`

创建 Document 和 Document Version。

## 8.2 创建转换任务

`POST /documents/{document_id}/process`

参数：

- target_format
- processing_profile
- options

返回 processing_job_id。

## 8.3 查询处理状态

`GET /jobs/{job_id}`

返回：

- status
- progress
- parser
- errors
- artifacts

## 8.4 获取转换产物

`GET /artifacts/{artifact_id}`

返回产物元数据及受控下载地址。

## 8.5 文档比较

`POST /compare`

输入两个文档版本或两个转换产物，返回差异任务。

## 8.6 导出文档

`POST /export`

输入 Markdown 产物、目标格式和导出配置，返回导出任务。

## 8.7 获取处理报告

`GET /jobs/{job_id}/report`

返回：

- 成功提取的章节数。
- 表格数量。
- 图片数量。
- OCR 页面数量。
- 警告数量。
- 失败项目。
- 来源映射完整性。

所有 API 必须经过 Team、Project 和资源级权限校验。

# 9. 安全与权限

V1 必须具备：

1. 文件类型白名单。
2. 文件大小限制。
3. 文件真实类型检查，不只依赖扩展名。
4. 病毒扫描。
5. 文件名及路径安全处理。
6. Parser 沙箱或隔离 Worker。
7. CPU、内存和执行时间限制。
8. 防止 ZIP Bomb。
9. 防止恶意 HTML 和外部资源加载。
10. Team / Project 数据隔离。
11. 原始文件和产物下载鉴权。
12. 操作审计。

文档内容属于不可信输入。

即使文档中出现要求 AI 忽略系统指令、调用工具或泄露信息的内容，也只能将其视为被分析的文档内容，不得提升为系统指令。

# 10. 质量要求

## 10.1 转换质量

重点验证：

- 标题层级正确性。
- 正文完整性。
- 表格内容完整性。
- 图片提取率。
- 页码映射准确性。
- 超链接保留情况。
- 非文本内容丢失情况。

不以“生成了 Markdown 文件”作为转换成功的唯一标准。

## 10.2 性能要求

V1 初步目标：

- 小型 DOCX/TXT：30 秒内完成。
- 一般 PDF：60 秒内完成。
- 大型文档：异步处理并提供状态。
- 任务支持超时和安全重试。
- 支持多个独立任务并行。

具体 SLA 应在实际测试数据基础上确定。

## 10.3 可观测性

记录：

- 文件类型。
- Parser 版本。
- 处理耗时。
- 任务成功率。
- 失败原因。
- 转换警告。
- 产物大小。
- 资源消耗。

不得将文档正文、密钥等敏感内容写入普通运行日志。

# 11. V1 交付范围

## P0：基础能力

1. 文件上传和原始文件管理。
2. DOCX 转 Markdown。
3. PDF 文本提取及 Markdown 转换。
4. PPTX 内容提取。
5. XLSX 工作表及表格提取。
6. HTML/TXT/CSV 解析。
7. 图片和附件管理。
8. Markdown 规范化。
9. 转换任务状态管理。
10. 转换报告。
11. 来源映射。
12. Team/Project 权限隔离。
13. 安全检查和错误处理。
14. 统一 API。
15. OCR Adapter 接口及页面检测能力。

## P1：增强能力

1. 完整 OCR 引擎。
2. 复杂 PDF 结构解析增强。
3. 文档版本比较。
4. 表格差异识别。
5. Markdown 导出 DOCX/PDF。
6. 批量转换。
7. 文档处理质量评估。

## P2：高级能力

1. PPTX 生成。
2. 复杂 Office 模板。
3. 高级排版。
4. 文档自动分类。
5. 文档语义结构识别增强。
6. 文档内容质量评估。

# 12. 验收标准

| 测试项 | 验收要求 |
|---|---|
| DOCX 转换 | 标题、段落、列表和普通表格完整 |
| PDF 转换 | 文本提取，尽可能保留页面映射 |
| PPTX 提取 | 保留幻灯片顺序和内容 |
| XLSX 提取 | 保留工作表和表格结构 |
| 图片提取 | 图片文件和 Markdown 引用对应 |
| 异常文件 | 产生明确错误，不影响原文件 |
| 重试 | 不产生错误的重复业务产物 |
| 多项目隔离 | 不允许跨项目读取和下载 |
| 来源追溯 | 转换内容可追溯到原始文档 |
| 知识隔离 | 转换成功不自动确认正式知识 |
| 索引隔离 | 转换成功不等于 PageIndex 已完成 |
| 版本隔离 | 转换器升级不创建原始文件新版本 |
| 安全隔离 | 恶意文档不能突破 Parser 和 AI 安全边界 |

# 13. 实施阶段

## 阶段 A：基础数据与存储

- Document Domain。
- Team 级 Document。
- 可选 Project 关联。
- 原始文件对象存储。
- Document Version。
- Processing Job。
- Artifact。

## 阶段 B：核心解析与转换

- DOCX。
- PDF。
- PPTX。
- XLSX。
- HTML/TXT/CSV。
- Markdown 规范化。
- Parser Adapter。
- Worker。

## 阶段 C：业务集成

- KB 接入。
- Product Cognition 接入。
- Project Cognition 接入。
- Requirements 和 Design 接入。
- AI 分析与候选知识流程。

## 阶段 D：质量与增强

- OCR。
- 文档比较。
- 导出。
- 质量评估。
- 批量转换。

# 14. 未裁决事项与后续技术闭合清单

以下事项尚未完成正式裁决，保留为后续技术设计工作，不得视为已批准。

| 编号 | 待确定事项 | 后续工作 |
|---|---|---|
| U-01 | 对象存储具体实现 | MinIO、S3 或现有存储设施评估 |
| U-02 | 异步队列选型 | 评估现有任务基础设施及消息队列 |
| U-03 | Parser 最终组合 | 使用真实中文文档样本进行质量对比 |
| U-04 | 文件大小和数量限制 | 明确 Team 配额、单文件限制 |
| U-05 | 原始文件保留策略 | 删除、归档、合规保留和生命周期 |
| U-06 | 业务事件协议 | 定义处理完成、失败及产物就绪事件 |
| U-07 | API 鉴权与授权细节 | 明确服务身份、用户身份和资源级授权 |
| U-08 | 转换质量评分 | 建立自动化评测指标和验收阈值 |
| U-09 | 转换产物清理策略 | 明确历史产物保留和垃圾回收机制 |
| U-10 | 文档比较实施范围 | 确定 V1 与 P1 的实际切分 |
| U-11 | 文档版本并发控制 | 明确同一 Document 的并发上传与版本分配 |
| U-12 | Git 正式知识接入契约 | 定义业务领域接收 Markdown 产物的接口 |
| U-13 | OCR 引擎选型 | P1 根据中文识别质量进行评估 |
| U-14 | 多租户隔离策略 | 明确对象存储路径、加密和访问隔离 |

以上事项在后续技术设计阶段逐项闭合。

# 15. 最终架构原则

Document Processing V1 必须遵守：

1. 平台级能力，而非业务模块专属能力。
2. Document 是 Team 级实体，支持可选 Project 关联。
3. 原始文件与转换产物统一使用对象存储。
4. 原始资料永久保留策略由生命周期和保留规则决定，不得因转换失败丢失。
5. 转换产物可重建，但历史版本需要按保留策略管理。
6. Markdown 是重要的标准化内容格式，但不是所有转换结果的唯一表达形式。
7. 文档转换与正式知识确认严格分离。
8. 文档处理与 PageIndex 检索严格分离。
9. Parser 采用 Adapter 架构，支持替换和扩展。
10. PostgreSQL 负责处理任务及状态，不能代替知识正文权威。
11. 所有跨 Team / Project 操作必须进行权限校验。
12. 文档始终是不可信输入，解析和 AI 分析分别建立安全边界。
13. 原始文件版本与转换产物版本分别管理。
14. Document Processing 通过独立服务 API 向各业务领域提供能力。

**最终产品定义：**

Document Processing 是 PM-AI 统一的文档接入、解析、转换、规范化和产物管理基础设施，为 AI 理解外部资料以及各业务领域建立正式知识提供标准化输入。

---

# 16. 架构裁决记录

| 编号 | 决策 | 结果 | 状态 |
|---|---|---|---|
| D-01 | Document 归属范围 | A：Team 级，可选 Project | 已确认 |
| D-02 | 原始文件及产物存储 | A：统一对象存储 | 已确认 |
| D-03 | 服务部署方式 | A：独立服务 + Adapter + Worker | 已确认 |
| D-04 | OCR V1 范围 | A：接口进入 V1，引擎进入 P1 | 已确认 |
| D-05 | 正式知识确认权 | A：业务领域独立确认 | 已确认 |
| D-06 | 版本管理 | A：原始版本与产物版本分离 | 已确认 |
| D-07 | 安全边界 | A：不可信文档，解析隔离，AI 独立防护 | 已确认 |

**当前结论：**

Document Processing V1 的七项核心产品与架构裁决已经完成。后续工作重点从方向选择转向数据模型细化、技术接口定义、存储与任务基础设施选型，以及实施范围闭合。

整体设计仍保留 U-01～U-14 技术待定事项，不将其提前视为已裁决。
