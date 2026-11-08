# R-S01-01-01 · Native AI-PM Independence（原生 AI-PM 独立性）裁决记录 V1.0

> **记录日期**：2026-11-05。本文件是对用户转交的专家裁决的**忠实记录**（以用户 2026-11-05 转交文本为准；
> 若专家原文与本转述存在出入，以原文为准并回改本记录）。
> 用户说明：「后面我会继续出」——专家将陆续产出配套稿件（见 §四 预告），到达前本文件先承载该裁决正文。
> 记录职责：文档会话。按既定协作约定，本文件只记录、不含评审意见。

## 一、裁决正文

**裁决编号**：R-S01-01-01 · Native AI-PM Independence
**裁决方案**：A

**即使项目没有接入任何外部 AI 工具，产品内置 AI-PM 仍然必须可以工作。**

### 允许工作的能力

- 需求收集、整理和分析
- 需求分类、拆分和建议
- 需求冲突及缺失识别
- 项目知识整理
- 业务规则梳理
- 项目规划辅助
- 人工评审和决策支持

### 不依赖外部工具的前提（明确不要求）

- 不要求 Repository。
- 不要求 MCP。
- 不要求 Codex / Claude Code。
- 不要求外部 Agent。

### 边界

产品内置 AI-PM 负责认知、分析、建议和项目治理；外部工程工具负责扩展代码仓库访问、代码分析和工程执行能力。

## 二、最终产品能力模型（三层）

| 层 | 名称 | 内容 | 性质 |
| --- | --- | --- | --- |
| L1 | **Project Native Core** | Project、Requirements、Knowledge、Tasks、Decisions、Baseline | 创建项目即可使用 |
| L2 | **Built-in AI-PM**（原生 AI 能力） | Cognition、Analysis、Recommendations、Governance、Planning | 产品自带，仅需产品自身可用的 AI 模型服务 |
| L3 | **External Engineering Integrations**（可选增强） | Repository、MCP、Codex、Claude Code、External Agents | 可选，接入前呈现「尚未接入」 |

> **裁决原文注**：这三个层次不是项目初始化的先后门槛，而是不同的能力层。

## 三、对 S01-01 UI/UX 的直接影响

**Project Command Center 必须在以下两种情况下都成立**：

| 功能 | 刚创建项目（无外部接入） | 已接入工程工具 |
| --- | --- | --- |
| 项目需求管理 | 可用 | 可用 |
| AI 需求分析 | 可用 | 可用 |
| AI 知识整理 | 可用 | 可用 |
| 人工决策 | 可用 | 可用 |
| 代码仓库分析 | 尚未接入 | 可用 |
| 外部 Agent 执行 | 尚未接入 | 可用 |

**技术边界**：产品自身需要具备可用的 AI 模型服务，但不需要用户先配置 MCP 或外部编码 Agent。

## 四、专家后续稿预告（用户转述）

专家将陆续产出，配套结构为：

- **S01-01：Project Command Center（项目总控台）**——总控台独立成稿；
- **Project Initialization**：作为总控台内部的初始化进度模块；
- **Project Native AI-PM**：项目原生 AI 能力；
- **External Integrations**：独立的外部工具接入能力。

> **结构影响（待后续稿到达后统一处理，本记录阶段不改既有稿编号）**：
> 现存 `s01-01-project-initialization-overview-ux-v1.md` 的定位可能从「独立页面」调整为「总控台内部的初始化进度模块」；
> 其与 I5 总控台稿（`smark-pm-s01-i5-project-command-center-v1-1.md`，该稿 §0.4 已有职责重叠冲突注）的关系须在专家新稿中收敛。

> **2026-11-05 更正（第一份设计修订稿 §九 到达）**：编号表述以修订稿 §九 为准——
> **S01-01 工作包仍为 Initialization Overview（不是 Project Command Center）**；Command Center 是默认工作首页，
> 其整体设计约束由修订稿承载（见 [s01-command-center-design-revision-v1-1.md](s01-command-center-design-revision-v1-1.md)），
> 初始化总览=Command Center 内的功能模块 + 可进完整详情页。上方预告中「S01-01：Project Command Center」的表述作废，以此更正为准。

## 五、对既有文档的落地影响（本次记录同步执行）

| 文档 | 位置 | 动作 |
| --- | --- | --- |
| `s01-01-project-initialization-overview-ux-v1.md` | §1.3 G-9、§9.5（均 2026-11-05 增补） | 修正并引本裁决：内置 AI 能力（AI 需求分析/AI 知识整理/AI 认知）在零外部接入时**照常可用**；呈现「尚未接入」的仅限代码仓库分析与外部 Agent 执行两类 |
| `smark-pm-s01-i5-project-command-center-v1-1.md` | §4.7 | 增补「零仓库、零设备 Project：独立工作模式」，引本裁决与 §三 可用性矩阵 |
| `README.md` | 文档索引 | 登记本裁决记录文件与后续稿预告 |

**待后续稿细化的挂账**（记录，不在本次处理）：

- Stage 3（AI-PM 认知）在**零仓库**下的材料来源与入口呈现——裁决方向「认知可从项目文档/信息起步」与既有 §9.2「②→③ 软依赖」一致，细化待专家「Project Native AI-PM」稿；
- 「内置 AI 需求分析」在 S02 需求域的落地形态，待专家后续稿；
- S01-01 编号归属（Command Center vs 初始化总览）与 `README.md` 既有「编号冲突（待裁决）」注的收敛，待专家 S01-01 新稿到达后统一修订。
