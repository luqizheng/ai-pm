# AI-PM Visualization Engine 立项文档包 · 索引与裁决摘要 V1.0

> 状态：待产品评审 · 日期：2026-11-12 · 文档共 9 份（含本索引），仅规划与评估，未进入开发。

## 1. 文档导航

| # | 文档 | 回答 |
|---|---|---|
| 01 | [产品需求说明书](ai-pm-viz-01-product-requirements-v1.md) | 立项价值（文本/Card → 认知交互系统）、原则 P1~P5、FR-1~8、NFR、范围界定 |
| 02 | [SeeCode 技术调研](ai-pm-viz-02-seecode-research-v1.md) | 事实档案（原始文件核实）、四层关系、10 问逐答、替代方案对比、集成边界 |
| 03 | [Visualization Spec V1.0](ai-pm-viz-03-visualization-spec-v1.md) | Spec/Artifact/Slot 三概念、四段式结构、E/W/I 校验、词表、防绑定契约 |
| 04 | [系统架构设计](ai-pm-viz-04-system-architecture-v1.md) | 拓扑、基础设施定位论证、三条数据流、0020 迁移、权限安全、降级、envelope 融合 |
| 05 | [六领域场景矩阵](ai-pm-viz-05-domain-scenario-matrix-v1.md) | 30 个场景逐条（数据源/生成/通道/优先级）、缺口 G-1~G-5、MVP 三选 |
| 06 | [Viewer UI/UX 方案](ai-pm-viz-06-viewer-uiux-v1.md) | 三入口、布局、交互规范、PRISM/V2 深色、组件组织、状态一览 |
| 07 | [路线图与任务拆解](ai-pm-viz-07-roadmap-v1.md) | P0 八任务三轨、P1~P3、里程碑门槛、评审前置条件 |
| 08 | [风险与验收](ai-pm-viz-08-risks-acceptance-v1.md) | 12 项风险登记（S0~S3）、16 条可测验收、回归节奏 |

## 2. 立项核心判断（一段话版）

AI-PM 的关系数据已在库中显式建模，但 UI 看不见——可视化引擎把**关系升级为第三表达层**（文本叙事/结构化记录/**关系图形**），使系统从"记录认知"升级为"看见并验证自身认知结构"。工程形态为**共享基础设施**（非独立领域、非独立服务）：自有 Spec + 双渲染 Adapter + Artifact 版本审批 + 权限继承 + 可拔降级。

## 3. 八个最终问题的回答（决策摘要）

**Q1 SeeCode 是否适合作为第一代渲染技术基础？**
作为 **AI 生成图形产物的渲染器：适合**（MIT、npm 零依赖、spec 驱动天然匹配 LLM、evidence 溯源理念、中文支持、产物自包含可沙箱）。作为**在线交互图引擎：不适合**（静态产物模型、硬预算 ≤9 节点/12 边 faithful ≤24、无 API 渲染模式、无法与宿主 UI 联动）。→ 采用为 Adapter B，不做全局基座。

**Q2 哪些复用、哪些自建？**
复用 SeeCode：产物渲染/导出链、编辑级排版美学、纠错回路（render→fix→patch）、复杂度预算理念。自建：Spec 权威、溯源体系（source_ref）、版本与审批、权限继承、图查询聚合层、在线交互 Viewer、降级机制。

**Q3 Spec 如何防第三方绑定？**
Spec 零渲染器字段（四段式：view/nodes/edges/meta）+ 受控词表 + 三道契约门禁：双 Adapter 出图测试、字段纯净性 lint（禁 SeeCode/G6 保留字）、降级表格达标（D3 §7）。渲染器专属参数只存在于各 Adapter 编译器内部。

**Q4 如何确保每个业务关系可追溯？**
三层强制：① 生成侧——节点/边 source_ref 为必填，引用必须存在于权限内上下文，否则 E 级拒绝（编造在入口即失败）；② 存储侧——source_ref 与 kb.memory_ref 同构，带 revision/content_hash，驱动 stale；③ 展示侧——无溯源显式 `unlinked` 灰标，点击必达原文档；AC-16 断言接受图形不产生任何业务变更。

**Q5 如何与 AI Conversation / Context Panel / Response Envelope 融合？**
envelope 白名单新增第 9 类 `viz_artifact`（fail-closed 不变）；对话卡 Accept/Reject 复用 `conversation_interaction` 终态语义；生成时可用对象集合 = `conversation_context_item`；图上选中节点可反向注入为 Context Item；右栏新增 Graph Tab。详见 D4 §8、D6 §1。

**Q6 MVP 选哪些场景？**
三选（D5）：① Req→Design→ADR→Task 追踪图（Live，数据四表现成，纯读）；② Task Dependency DAG（Live，单表最简）；③ 对话内 AI 生成架构/流程图（Artifact 全链路：校验→审批→SeeCode→沙箱）。三条分别验证 Live 数据链、Live 交互链、AI 生成链，互不阻塞可并行。

**Q7 独立 Visualization Domain 还是基础设施？**
**基础设施**。图形没有自己的业务事实（权威全在原领域），独立领域=制造第二权威，违反 P1；引擎自有状态仅 artifact 簿记，与 PageIndex 同级。落点：`viz` 路由分区 + 迁移 0020 起 + `crates/pm-viz`（D4 §2，DV-1 裁决）。

**Q8（隐含）现在做还是晚点做？**
现在立项、评审后即动 P0：晚做的代价是各领域自造画图方案导致溯源/权限无法统一收口（返工 > 新建）；而 P0 的三轨任务全部是只读投影 + 展示层簿记，对既有业务零侵入（AC-12 可拔测试托底）。

## 4. 裁决登记（本包产生）

| 编号 | 裁决 | 位置 |
|---|---|---|
| DV-1 | 引擎定位=共享基础设施，非独立业务域 | D4 §2 |
| DV-2 | 双 Adapter 架构：Adapter B=SeeCode（P0 产物），Adapter A=G6 5（P1 在线，Vue Flow 为备选） | D2 §5、D4 §1 |
| DV-3 | Spec 为 AI-PM 自有四段式格式；SeeCode spec 仅是编译目标 | D3 |
| DV-4 | 引擎无任何业务写路径；接受图形不生效业务变更 | D1 FR-5、AC-16 |
| DV-5 | 迁移自 0020 取号；暂用二跳拼接补 Task↔Design 语义（G-3） | D4 §4、D5 |

**待评审裁决**：D3 §9（Live 复用 Spec？spec Git 化？词表会签）、D5 G-3（是否直连表）、D4 §5（内网导出策略）。

## 5. 事实核查声明

- SeeCode 数据取自 2026-11-12 读取的 README / SKILL.md / spec.md / package.json（main 分支），star 数为第三方统计未经官方页核实；转义行为、贡献者数量等未承诺项已列入 P0 验收测试而非假设。
- AI-PM 侧所有表名/路径/行号引用来自本次代码摸底（migrations/0001~0019、apps/pm-api、ai-frontend/package.json 等），关键缺口（无图查询端点、前端零图形库、Task↔Design 二跳）已进入 G-1~G-5。
