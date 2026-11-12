# AI-PM Visualization User Interaction UX Specification V1.1

> **文档类型**：Product UX Specification · **版本**：V1.1（基于专家 V1.0 + 安全方向修订） · **状态**：Proposed → **待产品确认（§15 已给出裁决建议）**
> **适用范围**：AI-PM Visualization Engine · **架构基线**：[ADR V1.0.1](ai-pm-viz-09-architecture-decision-record-v1.md) + [V1.0.2 待签收修订](ai-pm-viz-09-architecture-decision-record-v1.md)（附录 A）· [15 号安全审查](ai-pm-viz-15-v0-6-security-review-v1.md)
> **核心技术组件**：VizSpec / SeeCode（服务端出图）/ AntV G6 / Artifact Viewer（静态 SVG）
> **配套**：[16 号 Viewer 技术设计](ai-pm-viz-16-v0-8-viewer-design-v1.md)（前端落点）· [17 号词表会签](ai-pm-viz-17-relation-registry-signoff-v1.md)

---

## 0. V1.1 修订说明（相对专家 V1.0）

专家 V1.0 写于 **ADR V1.0 基线**（iframe 时代）。此后 [15 号安全审查](ai-pm-viz-15-v0-6-security-review-v1.md) 已裁决**放弃 iframe、改为产物静态化 + 零脚本呈现**（实测 SeeCode v0.6.3：SVG 纯 Node 出图、产物 0 脚本/0 外链/字体内嵌、转义有效）。因此 V1.1 **只改与该裁决冲突的部分**，其余交互设计全部保留并采纳。

| 编号 | 专家 V1.0 原文位置 | 修订 | 依据 |
|---|---|---|---|
| UX-R01 | §3.2「Artifact Viewer = 在 AI-PM 中安全展示 HTML 产物」 | → 「展示**服务端产出的静态 SVG**」 | 15 号 §3.3 L1/L2 |
| UX-R02 | §4.1 B「P0 使用 SeeCode 生成的 HTML 产物，通过 Artifact Viewer 安全展示」+「(Sandboxed Artifact)」 | → 「P0 使用 SeeCode 服务端导出的 **SVG**，以 `<img>` 静态呈现；P0 **无交互**」 | 15 号 §3、§5 |
| UX-R03 | §4.1 B「安全的图形内交互（必须经过安全验证）」 | → 移除该项；交互能力归 P1 的 G6 通道 | 15 号 §5.2 |
| UX-R04 | §5.1 流程 A 第 7~9 步「调用 SeeCode Adapter → 生成 HTML 产物 → Artifact Viewer 展示」 | → 「调用 SeeCode Adapter → 服务端导出 **SVG** → Artifact Viewer 以 `<img>` 展示」 | 同上 |
| UX-R05 | §7.2 节点交互「3. 宿主检查来源和会话标识」 | → 移除 postMessage 校验（G6 同文档渲染，无跨帧通信）；保留「宿主执行权限检查 + 宿主查询业务对象」 | 15 号 §3.3 L3 |
| UX-R06 | §9.2「Artifact 安全：独立产物 Origin / Sandbox iframe / CSP / iframe 零凭证 / 零 API / 白名单 postMessage」 | → 整节替换为**三层防线**（静态化 / 零外网 / 自绘交互） | 15 号 §3.3 |
| UX-R07 | §10.2「iframe 通信异常」行 | → 删除该行 | 同上 |
| UX-R08 | §11 表「HTML 产物 = 实验接入 / 不作为主要模式」 | → 「**SVG = P0 主模式**；HTML = 不向浏览器投递（仅可选下载且须静态化检查）；PNG = 条件支持」 | 15 号 §4、ADR-VIZ-007 |
| UX-R09 | §12「ArtifactViewer = 安全展示 HTML 产物」「ExportMenu = HTML / SVG / PNG」 | → 「展示静态 SVG」「SVG 主 / PNG 条件 / HTML 可选下载」 | 同上 |
| UX-R10 | §13.1 UX-P0-09「HTML 产物不能绕过 AI-PM 的身份、权限和业务接口」 | → 「**产物不执行任何脚本**（SVG 静态化断言）；交互通道不加载任何产物代码」 | AC-7′/AC-18′ |
| UX-R11 | §11「节点点击 / 边点击 = 待 SeeCode 验证」 | → P0 明确为**不支持**（静态图无节点级交互），节点级信息经表格通道访问；P1 支持 | 16 号 §3.2、§4.2 |
| UX-R12 | §4.1 B「图形缩放和滚动（以实际渲染能力为准）」 | → 缩放归 P1（G6）；P0 静态图按容器自适应宽度 | 16 号 §3.1 |

**未修订且全部采纳**：§1.1 产品目标 · §1.2 六条交互原则 · §2 角色与场景 · §3.1 交互链路 · §4.1 A/C/D/E 四面板 · §5.2~§5.5 流程 B~E · §6 生命周期与发布交互 · §7.2~§7.6 P1 交互 · §8 AI 联动 · §9.1/9.3/9.4 权限与一致性 · §10 降级 · §13 验收标准 · §15 实施建议 · §16 最终产品定义。

**专家 spec 与既有设计的两处互补吸收（已并入 16 号）**：AI Explanation Panel（§4.1 D）与 Feedback Area（§4.1 E）此前 16 号未细化 → 16 号 §12 补齐。

---

# 1. 产品定位

## 1.1 产品目标

Visualization 是 AI-PM 的第三种信息表达方式，与文本和结构化数据并列。

它将 AI-PM 已掌握的项目事实、业务对象、对象关系、执行状态和 AI 分析结果转换为可视化内容，帮助用户：

1. 理解复杂业务关系。
2. 查看项目对象之间的依赖和追踪关系。
3. 理解 AI 分析结论及其依据。
4. 识别直接关系和间接推导关系。
5. 发现风险、缺失关系和潜在影响。
6. 通过对话要求 AI 调整分析和图形。
7. 保存、查看和导出可视化产物。

## 1.2 核心交互原则

**P1：AI-PM 是唯一用户交互入口**
用户通过 AI-PM 对话、业务页面和 Visualization Viewer 使用可视化能力，不需要直接接触 SeeCode。

**P2：Visualization 不具有业务事实权威性**
Visualization 可以读取、解释、关联和呈现业务事实，但不能创造、修改或批准业务事实。

**P3：图形与业务对象分离**
图形节点、边、布局和视觉样式属于呈现层。业务对象的正式修改必须回到对应业务领域。

**P4：AI 生成内容必须明确标识**
正式业务事实与 AI 推断内容必须区分，不能让用户误认为 AI 假设已经成为正式认知。

**P5：用户必须能够追溯图形依据**
用户应能够理解图形来自哪些业务对象、哪些版本，以及图形是否仍然有效。

**P6：渲染器对用户透明**
SeeCode、G6 或未来其他渲染 Adapter 的替换，不应改变用户的基本交互方式。

---

# 2. 用户角色与使用场景

## 2.1 用户角色

| 角色 | 主要行为 |
|---|---|
| Project Owner | 查看项目整体关系、进度、风险和影响 |
| Project Manager | 查看任务依赖、设计与需求追踪 |
| Product / Requirement Owner | 查看需求关联、覆盖情况和设计关系 |
| Architect | 查看架构图、模块关系、ADR 和设计追踪 |
| Developer | 查看任务依赖、实施依据和技术关系 |
| Reviewer / Approver | 查看 AI 分析结果和可视化产物，作出正式审批 |

具体权限必须继承所属业务对象和 Artifact 的权限规则，不能因为用户能够打开图形就获得图中全部对象的访问权。

## 2.2 主要使用场景

| 场景 | 用户目标 | 典型可视化 |
|---|---|---|
| 项目全景 | 理解项目当前结构 | Project Overview |
| 需求追踪 | 查看需求与设计、任务的关联 | Requirement Traceability |
| 任务依赖 | 理解任务执行顺序 | Task DAG |
| 架构理解 | 理解系统模块与技术关系 | Architecture Graph |
| 影响分析 | 识别某项变更影响哪些对象 | Impact Analysis |
| AI 解释 | 理解 AI 的分析和假设 | Explanatory Diagram |
| 知识审查 | 识别关系缺失和认知不一致 | Knowledge Relationship |

---

# 3. 整体交互架构

## 3.1 用户交互链路

```text
User
  |
  v
AI-PM Conversation / Business Page
  |
  v
Visualization Request
  |
  v
AI-PM Analysis
  |
  v
Business Data Query + Permission Check
  |
  v
VizSpec Generation
  |
  v
VizSpec Validation
  |
  v
Visualization Adapter
  |
  +----------------------+
  |                      |
  v                      v
SeeCode Adapter       Fallback Adapter
(P0 Artifact: SVG)    (Table / Mermaid)
  |                      |
  +----------+-----------+
             |
             v
       Artifact Viewer
       (static <img>)
             |
             v
        User Feedback
             |
             v
        AI-PM Conversation
```

P1 增加在线交互图：

```text
AI-PM Live Visualization
          |
          v
       VizSpec
          |
          v
      G6 Adapter
          |
          v
     Interactive Graph
          |
          v
  Node / Edge Selection
          |
          v
  AI-PM Host Permission Check
          |
          v
  Context Panel / AI Conversation
```

> **V1.1 变化**：P0 链路中的 SeeCode Adapter 产出 **SVG**（纯 Node 服务端导出），Artifact Viewer 以 `<img>` 静态呈现；P0 无跨帧通信环节。

## 3.2 组件职责

| 组件 | 职责 |
|---|---|
| AI-PM Conversation | 用户提出可视化请求、反馈和修改意见 |
| Visualization Service | 查询、分析、生成 VizSpec |
| VizSpec Validator | 检查数据结构、关系和安全规则 |
| SeeCode Adapter | 生成 P0 解释性可视化产物（**服务端 SVG**） |
| G6 Adapter | P1 在线交互图 |
| Artifact Viewer | 在 AI-PM 中**安全展示静态 SVG 产物**（零脚本执行） |
| Context Panel | 显示选中对象的业务上下文 |
| Artifact Management | 保存、发布、查看版本和状态 |
| Fallback Adapter | 渲染失败时提供表格或 Mermaid 降级 |

SeeCode 不直接承担业务查询、业务审批和业务对象修改。

---

# 4. P0 用户交互规格：Artifact Visualization

P0 是 V1 最先交付的用户体验。它以 AI 生成的解释性可视化产物为核心，支持查看、反馈、重新生成、保存、发布和导出。P0 不要求实现完整的在线关系图编辑器。

## 4.1 P0 页面结构

```text
+-------------------------------------------------------------+
| Visualization Title                 Status       Actions    |
+-------------------------------------------------------------+
| Description / AI Explanation                                |
+-------------------------------------------------------------+
|                                                             |
|                                                             |
|                  Visualization Canvas                       |
|                                                             |
|                (Static SVG Artifact, <img>)                 |
|                                                             |
|                                                             |
+-------------------------------------------------------------+
| Data Sources | Nature | Revision | Generated At | Freshness |
+-------------------------------------------------------------+
| AI Explanation / Selected Information                       |
+-------------------------------------------------------------+
| User Feedback / Ask AI to Modify                             |
+-------------------------------------------------------------+
```

> **V1.1**：Canvas 内容为静态 SVG（`<img>` 引用 Blob objectURL），**无节点级交互**；节点级信息经同页的"表格视图 / 溯源面板"访问（P0 表格兼作节点导航通道，见 16 号 §3.2）。

### A. Header

显示：Visualization 标题、图形类型、Artifact 状态、生成时间、当前版本、操作入口。

| 操作 | 说明 |
|---|---|
| Regenerate | 要求 AI 重新生成 |
| Export | 导出支持的格式（SVG 主 / PNG 条件 / HTML 可选下载） |
| View Sources | 查看来源 |
| View History | 查看历史版本 |
| Publish | 进入正式发布流程 |
| Close | 关闭 Viewer |

操作权限必须由 AI-PM 服务端检查。

### B. Visualization Canvas

主要展示区域。**P0 使用 SeeCode 服务端导出的 SVG 产物，以 `<img>` 静态展示**（零脚本执行，见 §9.2）。

支持：适配容器宽度（`width:100%; height:auto`）、图形说明信息（随图或经 AI 解释面板）、节点与关系的视觉区分（由 SeeCode 出图能力决定）。

> **P0 明确不支持**：节点点击、悬停、Focus 面板、图形内缩放（→ P1 G6 通道）。P0 的节点级信息访问经「表格视图」与「来源面板」。
> **P0 交互缺口提示**：界面需明示「交互探索在 P1 上线」，避免被理解为功能缺失（UX-D11 裁决）。

### C. Provenance Panel

显示图形的数据依据：

- 数据来源领域。
- 来源对象名称及 ID。
- 来源对象版本。
- 来源更新时间。
- 图形生成时间。
- 数据一致性状态（Current / Mixed Revision / Stale）。
- 是否存在 AI 推断内容。

### D. AI Explanation Panel

显示 AI 对图形的解释：图形表达了什么、主要关系、关键发现、**Direct Relationships**、**Indirect Relationships**、潜在风险、AI 推断和待确认事项。

### E. Feedback Area

用户通过自然语言告诉 AI-PM 如何调整。例如：

- "只显示已确认的需求。"
- "突出显示阻塞任务。"
- "把间接关系单独显示。"
- "解释为什么 Task-102 与 Design-003 有关系。"
- "去掉不相关的历史节点。"
- "把这张图改成更适合架构评审的布局。"

反馈产生新一轮 AI 分析和 VizSpec，不直接修改业务对象。

---

# 5. P0 核心交互流程

## 5.1 流程 A：用户首次生成可视化

1. 用户在 AI-PM 对话中提出需求。
2. AI-PM 识别图形类型和业务范围。
3. AI-PM 查询相关业务对象。
4. 执行权限过滤。
5. 生成 VizSpec。
6. 执行 Spec 校验和安全检查。
7. 调用 SeeCode Adapter → **服务端导出 SVG**。
8. Artifact Viewer 以 `<img>` 展示结果。
9. AI-PM 返回简要解释。

用户体验要求：生成中显示 Processing 状态；明确显示正在生成；**不允许把未经验证的半成品显示为正式产物**；生成失败给出可读错误说明与降级内容；**渲染失败不得阻断正常 AI 对话**。

## 5.2 流程 B：用户要求修改图形

示例："这张图太复杂了，只保留需求、设计和任务之间的关系。"

AI-PM：识别修改意图 → 分析要求 → 生成新 VizSpec → 重新校验 → 重新渲染（服务端出 SVG）→ 生成新草稿版本 → 展示新旧结果。

```text
Original Visualization
        |
        v
User Feedback
        |
        v
AI Generates New Version
        |
        v
New Visualization Preview
        |
        +---- Accept
        |
        +---- Request Changes
        |
        +---- Compare Versions
```

重要规则：不覆盖已发布的 Artifact；重新生成不代表正式发布；用户可继续要求修改；新版本必须保留生成依据；**不得因为调整布局而改变业务关系**。

## 5.3 流程 C：用户接受或拒绝 AI 结果

Accept / Reject 需区分两种含义：

**第一种：接受图形呈现结果** —— 用户认可当前图形表达方式，可保存或进入发布流程。**不意味着业务事实得到确认。**

**第二种：接受 AI 提出的业务建议** —— 若 AI 在图形中发现潜在关系或提出业务建议，必须进入对应业务领域的确认流程，不能由 Artifact 的 Accept 按钮完成。

| 操作 | 系统行为 |
|---|---|
| Accept Visualization | 接受当前图形呈现 |
| Request Changes | 重新生成 |
| Reject Visualization | 拒绝当前呈现结果（reason 必填） |
| Confirm Business Proposal | 跳转对应业务确认流程 |
| Dismiss AI Hypothesis | 忽略 AI 假设 |

## 5.4 流程 D：用户查看图形来源

用户点击 View Sources，展示：来源对象名称、对象类型、对象 ID、Revision、来源领域、来源定位信息、来源权限状态。

对于无权访问的对象：**不显示对象名称、不显示对象 ID、不显示 Tooltip、不通过边存在性或关系计数泄露信息**。

## 5.5 流程 E：用户导出图形

V1 支持范围以 ADR-VIZ-007 为准：

- **SVG**（P0 承诺，服务端纯 Node 出图）
- **PNG**（条件支持，需路径验证通过，见 R-03）
- **HTML**（可选下载，不向浏览器投递；下载后由用户本地打开）

GIF / MP4 不属于 V1 必须交付范围。

导出前检查：① 当前用户是否具有导出权限；② Artifact 是否允许导出；③ 当前格式是否支持；④ 导出内容是否包含无权访问的信息；⑤ 导出链路是否满足安全要求。

---

# 6. Artifact 生命周期与交互

## 6.1 状态

遵循 ADR-VIZ-006：

```text
draft ──► published ──► stale
  │
  └──────► rejected
```

Artifact 状态和 Approval 状态必须分开管理。

## 6.2 状态对应的用户体验

| Artifact 状态 | 用户看到的内容 | 可用操作 |
|---|---|---|
| Draft | 草稿图形 | Preview、Edit Request、Publish Request |
| Published | 已发布图形 | View、Export、History |
| Stale | 来源数据已变化 | View Old Version、Regenerate |
| Rejected | 已拒绝的产物 | View History、Regenerate |

Stale 不自动重渲染。用户需要明确看到：**"此图形所依据的业务数据已经发生变化，当前展示的是历史版本。"** 并可：查看旧版本、查看发生变化的来源、请求重新生成。

## 6.3 发布交互

1. 用户点击 Publish。
2. 检查 Artifact 是否完整。
3. 检查 provenance。
4. 检查 nature。
5. 检查权限。
6. 创建 Approval。
7. 授权人员审批。
8. 审批通过后发布。
9. 发布版本不可变。

对于 nature=explanatory 的内容：必须始终显示"AI 生成 · 非正式认知"；不允许发布到正式业务槽位；**不能通过用户点击 Accept 将其转换为正式业务事实**。

---

# 7. P1 用户交互规格：Live Interactive Graph

P1 引入 AntV G6 5。其目标不是简单地让用户点击图形，而是让图形成为 AI-PM 的关系探索入口。

P1 的交互重点：节点选择、关系选择、关系展开、路径追踪、影响分析、Direct / Indirect 区分、图形与 AI 对话上下文联动。

## 7.1 P1 页面结构

```text
+---------------------------------------------------------------+
| Graph Title             Filters          Layout       Actions |
+-------------------------------+-------------------------------+
|                               |                               |
|                               |       Context Panel           |
|                               |                               |
|       Interactive Graph       |  Selected Object              |
|                               |  Properties                    |
|                               |  Relationships                 |
|                               |  Sources                       |
|                               |  AI Explanation                |
+-------------------------------+-------------------------------+
| AI Conversation / Ask About Selection                          |
+---------------------------------------------------------------+
```

图形区与 Context Panel 分离。**用户点击图形时 Context Panel 更新，而不是直接打开业务编辑页面。**

## 7.2 节点交互

用户点击节点：

1. G6 返回 Node ID。
2. **宿主执行权限检查**（G6 与宿主同文档，无跨帧通信、无会话标识校验环节）。
3. **宿主查询业务对象**。
4. Context Panel 展示信息。

> **V1.1**：原「宿主检查来源和会话标识」（postMessage 校验）已移除 —— G6 渲染的是 VizSpec 数据而非第三方产物代码，不存在跨帧消息面。

节点信息字段：Name / Type / ID / Status / Revision / Nature / Source / Updated At。

对于 explanatory 节点，必须持续显示：**AI 生成 · 非正式认知**。

## 7.3 边交互

点击边后显示：关系名称、起点、终点、关系来源、关系确认状态、是否为直接关系、是否为推导关系、推导路径、语义可信度。

```text
Task-102
   |
   | derived_trace
   | direct = false
   |
Requirement-012
   |
   | implements
   |
Design-003
```

用户必须能够区分：**Direct**（业务数据中存在直接关系）/ **Indirect**（经中间对象推导）。推导关系不能伪装成正式的直接业务关系。

## 7.4 关系展开

用户可选择展开上游关系、下游关系、相关需求、关联设计、关联 ADR、相关任务。

采用渐进式展开：初始只展示核心对象，用户主动展开后再加载。**展开操作只能改变图形视图，不得创建业务关系。**

## 7.5 路径追踪

用户选择两个节点（如 Requirement-012 → Task-102），系统展示：直接路径、间接路径、路径中的对象与关系、缺失的关系、权限限制提示、数据一致性状态。

**如果没有可靠路径，应明确显示"未发现可验证关系"，不能让 AI 自动补造业务关系。**

## 7.6 影响分析

用户选择某个 Requirement / Design / ADR 或其他对象，点击 Impact Analysis，系统展示：Direct Impact、Indirect Impact、Affected Requirements、Affected Designs、Affected ADRs、Affected Tasks、Potential Risks。

Direct 与 Indirect 必须视觉区分。AI 可进一步解释"为什么这个对象会影响这些任务"，**但解释不能改变实际追踪关系**。

---

# 8. AI Conversation 与 Visualization 联动

## 8.1 交互原则

用户可以围绕当前图形持续与 AI-PM 对话。对话应自动携带当前可视化上下文：Artifact ID 或 Live Graph Context、当前 VizSpec Revision、当前选中节点、当前选中边、用户筛选条件、数据来源版本、当前 consistency 状态。

## 8.2 典型交互

| 用户输入 | AI-PM 行为 |
|---|---|
| 解释这个节点 | 解释选中对象 |
| 为什么这两个对象有关联？ | 解释关系及来源 |
| 显示所有受影响任务 | 生成影响分析 |
| 只显示直接关系 | 更新图形过滤条件 |
| 展开这个模块 | 查询相关关系并扩展图形 |
| 哪些关系是 AI 推断的？ | 标出 explanatory 内容 |
| 这张图的数据是否最新？ | 检查 source_revision 和 stale |
| 找出没有关联设计的需求 | 执行查询并生成结果 |

## 8.3 AI 输出的呈现

AI 输出可以包含：Text Explanation、Visualization Artifact、Selected Object Card、Relationship Card、Impact Analysis Card、Source Reference、Warning / Risk Card。

**图形是 AI 输出的一种呈现形式，不应强制所有 AI 回复都生成图形。**

---

# 9. 权限、安全与一致性 UX

## 9.1 权限规则

权限过滤必须在业务查询和图形投影两层执行。**前端隐藏节点不能代替服务端权限过滤。**

无权限数据不能通过以下方式泄露：节点、边、Tooltip、Focus Panel、关系数量、图形布局、导出文件。

## 9.2 Artifact 安全（V1.1 整节替换）

**原 V1.0 的五层 iframe 隔离已废止**（依据 [15 号安全审查](ai-pm-viz-15-v0-6-security-review-v1.md)）。新安全模型为**三层**：

| 层 | 措施 | 对用户的表现 |
|---|---|---|
| **L1 静态化** | 产物只产出 SVG；入库前断言无 `<script>` / `foreignObject` / 可执行 `on*` 属性 / 外部引用 | 用户打开图**不会有任何脚本运行**；比沙箱更强（沙箱是限制运行，不执行是消除运行） |
| **L2 零外网** | 字体 base64 内嵌（实测已内嵌）；SVG 无真实外链；服务端导出走本地 vendor 编码器 | 用户看图不产生任何对外请求，私有数据不出域 |
| **L3 自绘交互** | P1 交互由 G6 渲染 **VizSpec 数据**（自有代码 + 自有数据），不加载任何产物脚本 | 用户点节点时，宿主执行权限检查后再展示 |

补充约束：

- Artifact Viewer 以 `<img>` 引用产物（**不使用** `<object>`/`<iframe>`——`<object>` 可执行 SVG 内脚本）。
- 交互通道的前端执行路径不得引入任何 SeeCode 产物脚本（构建产物 + 源码双断言，AC-18′）。
- **重新引入浏览器内 HTML 产物默认禁止**；如将来必须引入，须重走 ADR-008 全套（独立 Origin + sandbox + CSP + postMessage）。

P1 的图形事件也不能绕过宿主权限检查。

## 9.3 一致性状态

| 状态 | 用户说明 |
|---|---|
| Current | 当前图形依据的数据版本一致 |
| Mixed Revision | 图形包含不同版本的数据 |
| Stale | 来源数据已变化，图形需要重新生成 |

**不能将 mixed-revision 数据伪装成同一时点的完整图形。**

## 9.4 Nature 区分

| Nature | 含义 | UI 标识 |
|---|---|---|
| business | 真实业务对象 | 正式业务对象 |
| explanatory | AI 假设或解释内容 | AI 生成 · 非正式认知 |

explanatory 节点采用独立视觉样式，**但不能只依靠颜色区分，还应提供文字标签**。

---

# 10. 异常处理与降级体验

## 10.1 渲染失败

当 SeeCode 不可用或渲染失败：① 不阻断 AI-PM 对话；② 显示图形生成失败说明；③ 保留 AI 分析结果；④ 使用 Fallback Adapter；⑤ 按图形类型降级为结构化表格或 Mermaid；⑥ 允许用户重新尝试。

## 10.2 异常状态

| 异常 | 用户体验 | 系统行为 |
|---|---|---|
| Spec 校验失败 | 显示数据结构错误 | 拒绝渲染 |
| SeeCode 不可用 | 提示降级 | 使用 Fallback |
| 权限不足 | 显示权限提示 | 不返回受限数据 |
| 来源数据缺失 | 标记数据不完整 | 不虚构节点或关系 |
| Mixed Revision | 显示一致性警告 | 保留版本信息 |
| Artifact Stale | 显示过期提示 | 不自动重渲染 |
| Export 失败 | 显示导出失败 | 保留当前图形 |
| 产物静态化断言失败 | 显示"图形不可用" | **拒绝入库与渲染**（安全失败路径） |
| 注入检测失败 | 拒绝渲染 | 进入安全失败路径 |

> **V1.1**：删除原「iframe 通信异常」行 —— 不再有 iframe 与跨帧通信。

## 10.3 Fallback 能力矩阵

遵循 ADR-VIZ-002 的降级方向：

| 图形类型 | Fallback |
|---|---|
| Flowchart | Mermaid |
| Sequence | Mermaid |
| Trace | G6 或结构化表格 |
| DAG | G6 或结构化表格 |
| Animation | 不保证 |
| GIF / MP4 | V1 不保证 |
| Isometric | V1 不保证 |

---

# 11. P0 与 P1 功能边界

| 功能 | P0（SeeCode·SVG） | P1（G6） |
|---|---|---|
| AI 生成图形 | 支持 | 支持 |
| Artifact Viewer | 支持（静态 SVG） | 可复用 |
| **SVG 产物** | **主模式**（服务端纯 Node 出图） | 按实际能力 |
| **HTML 产物** | **不向浏览器投递**（仅可选下载） | 不作为主要模式 |
| PNG 导出 | 条件支持，需路径验证 | 按实际能力 |
| 查看来源 | 支持 | 支持 |
| AI 解释 | 支持 | 支持 |
| 用户对话修改 | 支持 | 支持 |
| Artifact 保存 | 支持 | 视业务场景 |
| Artifact 发布 | 支持 | 视业务场景 |
| 节点点击 | **不支持**（静态图；经表格/来源面板访问） | 支持目标 |
| 边点击 | **不支持** | 支持目标 |
| 关系展开 | 非 P0 必须项 | 支持目标 |
| 路径追踪 | 非 P0 必须项 | 支持目标 |
| 影响分析 | 以解释性产物 + AI 解释为主 | Live Graph |
| 节点编辑业务关系 | 禁止 | 禁止 |
| GIF / MP4 | 不支持承诺 | 后续条件评估 |

> **V1.1**：P0 的 SVG 能力已由实测确认（纯 Node、无浏览器、零外链）；PNG 仍须路径验证（R-03），**不应仅凭格式名称认为已可用**。

---

# 12. 关键 UI 组件清单

| Component | 职责 | 优先级 | 前端落点（16 号） |
|---|---|---|---|
| VisualizationHeader | 标题、状态、操作 | P0 | `VizVersionBar.vue` + 视图头 |
| ArtifactViewer | 安全展示**静态 SVG** 产物 | P0 | `VizArtifactFrame.vue`（`<img>` + Blob） |
| VisualizationFallback | 表格 / Mermaid 降级（**兼作 P0 节点导航通道**） | P0 | `VizSpecTable.vue` |
| VisualizationStatus | Draft / Published / Stale | P0 | `VizStaleBar.vue` + `Badge.vue` |
| ProvenancePanel | 来源和版本 | P0 | `VizTracePanel.vue`（Sheet 范式） |
| NatureBadge | Business / Explanatory | P0 | `Badge.vue`（**须含文字标签**） |
| ConsistencyIndicator | Current / Mixed Revision | P0 | `VizVersionBar.vue` 内联 |
| AIExplanationPanel | AI 图形解释（Direct / Indirect / 风险 / 待确认） | P0 | **V0-8.12 新增**（专家 §4.1 D） |
| VisualizationFeedback | 用户反馈与重新生成 | P0 | **V0-8.13 新增**（专家 §4.1 E） |
| ExportMenu | **SVG 主 / PNG 条件 / HTML 可选下载** | P0 | `VizArtifactFrame.vue` 操作槽 |
| ArtifactHistory | 历史版本 | P0 | `VizVersionBar.vue` |
| PublishDialog | 发布审批入口 | P0 | 复用既有 Dialog/Sheet 范式 |
| GraphCanvas | 在线图形画布 | P1 | G6 Adapter |
| GraphNode / GraphEdge | 图形节点 / 关系 | P1 | G6 Adapter |
| GraphToolbar | 缩放、布局、筛选 | P1 | G6 Adapter |
| GraphContextPanel | 选中对象上下文 | P1 | 右栏 `ContextPanel.vue`（Graph Tab） |
| RelationshipExplorer | 关系展开 | P1 | G6 Adapter + 后端查询 |
| PathExplorer | 路径追踪 | P1 | G6 Adapter + 路径算法 |
| ImpactAnalysisPanel | 影响分析 | P1 | G6 Adapter + 后端聚合 |

---

# 13. UX 验收标准

## 13.1 P0 验收

**UX-P0-01：生成** —— 用户能够通过 AI-PM 对话生成可视化产物。
**UX-P0-02：查看** —— 用户能够在 AI-PM 内安全查看图形。
**UX-P0-03：来源追溯** —— 用户能够查看图形的来源对象和版本。
**UX-P0-04：性质识别** —— 用户能够区分正式业务事实与 AI 推断。
**UX-P0-05：反馈修改** —— 用户能够通过对话要求修改图形，并获得新的图形版本。
**UX-P0-06：Artifact 生命周期** —— 用户能够识别 Draft、Published、Stale、Rejected 状态。
**UX-P0-07：发布隔离** —— 接受图形不能直接修改业务事实；正式发布必须经过对应流程。
**UX-P0-08：导出** —— 用户能够使用已验证的导出格式。
**UX-P0-09：安全（V1.1 改写）** —— 产物**不执行任何脚本**（SVG 静态化断言通过）；交互通道不加载任何产物代码。
**UX-P0-10：降级** —— SeeCode 不可用时，用户仍能获得可理解的结构化结果。
**UX-P0-11（新增）** —— 界面明示"交互探索在 P1 上线"，P0 的节点级信息可经表格/来源面板访问。

## 13.2 P1 验收

**UX-P1-01：节点选择** —— 点击节点后能够在 Context Panel 查看对象信息。
**UX-P1-02：边选择** —— 点击边后能够查看关系类型、来源和关系性质。
**UX-P1-03：关系展开** —— 用户可以主动展开上下游关系。
**UX-P1-04：路径追踪** —— 用户可以查看两个对象之间的可验证关系路径。
**UX-P1-05：影响分析** —— 系统能够区分 Direct Impact 和 Indirect Impact。
**UX-P1-06：AI 上下文联动** —— 用户可以围绕当前节点或关系继续向 AI 提问。
**UX-P1-07：权限安全** —— 无权限对象不能通过图形或关系信息泄露。
**UX-P1-08：一致性** —— 用户能够识别图形所使用的数据版本及其一致性状态。

---

# 14. 待产品确认事项 —— 裁决

专家 V1.0 列出 10 项待确认，均为 UX 建议而非架构事实。**产品经理裁决如下**（与既有 ADR/设计不冲突者直接采纳）：

| 编号 | 决策问题 | 专家建议 | **产品裁决** | 依据/备注 |
|---|---|---|---|---|
| UX-D01 | P0 Artifact Viewer 是否提供独立详情页？ | A：支持独立 Viewer，同时允许嵌入 AI 对话 | **采纳 A** | 对话卡内嵌 + 独立详情页并存；独立页承载长会话与版本历史 |
| UX-D02 | 用户修改图形是否生成新 Revision？ | A：生成新草稿版本，不覆盖已发布版本 | **采纳 A** | 与 13 号 §4.1 Artifact 状态机一致 |
| UX-D03 | Accept 是否区分图形接受与业务确认？ | A：必须区分 | **采纳 A** | ADR-VIZ-004/005 零业务写路径；UX §5.3 双义表已落 |
| UX-D04 | 来源信息放在哪里？ | A：独立 Sources Panel | **采纳 A** | 16 号 §3.3 `VizTracePanel`（Sheet 范式） |
| UX-D05 | P1 点击节点后如何展示详情？ | A：右侧 Context Panel | **采纳 A** | 右栏已有四 Tab 范式（`ContextPanel.vue`）；新增 Graph Tab |
| UX-D06 | 图形筛选是否需要保存？ | A：可以保存视图条件，但不改变业务事实 | **采纳 A（限 P1）** | 筛选条件属视图状态；P0 无筛选能力 |
| UX-D07 | 是否允许用户直接编辑图形节点和关系？ | A：不允许直接修改业务关系 | **采纳 A** | ADR-VIZ-005；代码层由 `scripts/viz-no-write-path.sh` 强制 |
| UX-D08 | Stale 图形如何处理？ | A：明确提示，由用户请求重新生成 | **采纳 A** | 不自动重渲染（ADR-VIZ-006） |
| UX-D09 | explanatory 内容是否允许发布？ | A：不允许发布到正式业务槽位 | **采纳 A（加强）** | 13 号 §3.2 DB CHECK 强制（`nature_publish_check`），非仅 UI 约束 |
| UX-D10 | AI 图形修改是否支持版本比较？ | A：支持新旧版本对比 | **采纳 A** | `VizVersionBar` + spec diff（DV-6 统一数据模型支持结构化 Diff） |
| **UX-D11（新增）** | P0 产物无交互，界面如何处理该缺口？ | — | **明示而非隐藏**：卡片/页脚标注"交互探索在 P1 上线，当前提供表格视图"；表格作为节点导航通道 | 避免被理解为功能缺失或缺陷 |
| **UX-D12（新增）** | P0 节点级信息访问路径？ | — | **双通道**：① 同页表格视图（排序/分页 + 点击跳转原文档）；② Provenance Panel | 16 号 §3.2 |

---

# 15. 实施建议

## Phase 1：P0 基础体验

Artifact Viewer（静态 SVG）· AI 生成与展示链路 · Provenance Panel · Nature 标识 · Consistency Indicator · AI Explanation · 用户反馈与重新生成 · Fallback · 产物静态化与零外网验证。

## Phase 2：P0 Artifact 管理

Artifact History · Draft / Published / Stale · Publish / Approval · SVG 导出（PNG 条件，HTML 可选下载）· 版本比较。

## Phase 3：P1 Live Graph

G6 Graph Canvas · Node / Edge Selection · Context Panel · Relationship Explorer · Path Explorer · Direct / Indirect Impact · AI Conversation Context Integration。

---

# 16. 最终产品定义

AI-PM Visualization 的最终用户体验应当是：

**用户通过 AI-PM 提出问题，AI-PM 读取和分析项目知识，生成可视化内容，用户通过图形理解业务关系，再通过 AI-PM 对话进一步探索和提出修改意见。**

SeeCode 是 P0 的内部渲染 Adapter（**服务端出图**），不是用户直接操作的产品界面。AntV G6 是 P1 的在线图形交互 Adapter。两者都必须遵守统一 VizSpec、业务权限、数据来源和业务事实权威边界。

最终，Visualization 应成为 AI-PM 的第三表达层，而不是独立的业务系统或第二知识权威。

**核心原则：图形帮助用户理解业务，AI-PM 负责组织知识，业务领域负责维护正式事实。**
