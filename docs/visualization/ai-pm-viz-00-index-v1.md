# AI-PM Visualization Engine 立项文档包 · 索引与裁决摘要 V1.1

> 状态：**实施前架构复审附条件通过（ADR V1.0.1）**；[V1.0.2 安全修订](ai-pm-viz-15-v0-6-security-review-v1.md)（放弃 iframe → 产物静态化）待 Architecture/Security Owner 签收。R-01 取号结论 0061（待签收）、R-02 已关闭、R-03（PNG）开放按 B2 处理。未进入完整实施。

## 1. 文档导航

| # | 文档 | 回答 |
|---|---|---|
| 01 | [产品需求说明书](ai-pm-viz-01-product-requirements-v1.md) | 立项价值（文本/Card → 认知交互系统）、原则 P1~P5、FR-1~9（新增图形性质区分）、NFR |
| 02 | [SeeCode 技术调研](ai-pm-viz-02-seecode-research-v1.md) | 事实档案、四层关系、10 问逐答、方案对比、集成边界（含降级能力矩阵） |
| 03 | [Visualization Spec V1.0](ai-pm-viz-03-visualization-spec-v1.md) | 四段式结构、provenance/nature、Relation Registry、derived_trace 合成边、防绑定契约 |
| 04 | [系统架构设计](ai-pm-viz-04-system-architecture-v1.md) | 拓扑、crate 布局与服务拆分时机、数据流（含一致性边界）、0020 迁移、安全增强、融合 |
| 05 | [六领域场景矩阵](ai-pm-viz-05-domain-scenario-matrix-v1.md) | 30 场景逐条、缺口 G-1~G-5、修订后 MVP 范围 |
| 06 | [Viewer UI/UX 方案](ai-pm-viz-06-viewer-uiux-v1.md) | 三入口、布局、交互规范（Direct/Indirect、解释图徽标、postMessage） |
| 07 | [路线图与任务拆解](ai-pm-viz-07-roadmap-v1.md) | **V1.1**：P0=Foundation+Security+SeeCode 实验，P1=Live Graph（G6） |
| 08 | [风险与验收](ai-pm-viz-08-risks-acceptance-v1.md) | 12 风险登记、**21 条可测验收**（AC-17~21 为评审新增） |
| 09 | [**ADR V1.0.1**](ai-pm-viz-09-architecture-decision-record-v1.md) | **现行决策基线（附条件通过）**：11 项决策 + R-01~R-10 风险登记 + P0 启动门槛 + 复审裁决记录（原 V1.0 的 11 决策/验证项/追踪矩阵被本版继承并闭合） |
| 10 | [**P0 验收矩阵 V1.0**](ai-pm-viz-10-p0-acceptance-matrix-v1.md) | AC-1~21 逐条：阶段归属（关闭 R-02）、实施任务、测试场景 T-01~T-34、验收证据、阻断等级（B1/B2/P1）、R-01 核查证据、M1 清单 |
| 11 | [**P0 签收材料包 V1.0**](ai-pm-viz-11-signoff-pack-v1.md) | 7 张角色单页签收表（S-1~S-7）+ 启动门槛待办追踪 + R-03 口径 + 回执归档表 + 专家再介入触发条件 |
| 12 | [**V0-1 技术设计**](ai-pm-viz-12-v0-1-spec-validation-design-v1.md) | **可直接进入实现**：仓库落点、Rust 类型草案、校验器（22 个问题码 + 纯函数形态）、Relation Registry 治理、source_hash 规范、Golden 手写比对方案、9 项实施任务、6 项偏差更正 |
| 13 | [**V0-2 数据模型设计**](ai-pm-viz-13-v0-2-data-model-design-v1.md) | **取号结论 0061**（0020/0055~0060 曾存在，sqlx 版本单调性）+ `viz` schema 四表 DDL + Artifact/Approval 状态机分离（R-04）+ 容量清理规则（R-05）+ API 端点 + 10 项实施任务 |
| 14 | [**V0-3 权限投影与一致性设计**](ai-pm-viz-14-v0-3-projection-consistency-design-v1.md) | **双层过滤**（查询层端点 project 校验 + 投影层整边丢弃/计数重算）、11 项泄露面清单、`consistency=current` 严格定义（R-07）、nature 粒度（R-06）、Resolver 落点、泄露矩阵测试 |
| 15 | [**V0-6 安全方向审查**](ai-pm-viz-15-v0-6-security-review-v1.md) | **放弃 iframe → 产物静态化 + 零脚本呈现**（实测 SeeCode v0.6.3：SVG 纯 Node 出图、0 脚本/0 外链/字体内嵌、转义有效）；三层新防线、交互归 G6、AC-6′/7′/18′ 重写、M-1~M-8 修订清单 |
| 16 | [**V0-8 Viewer 设计**](ai-pm-viz-16-v0-8-viewer-design-v1.md) | 产物通道（`<img>`+Blob，零图形依赖）+ 降级表格（P0 兼作节点导航）+ G6 交互通道（P1）；**第 9 类信封六处必改**、右栏 Graph Tab 三处、令牌/i18n 规则、11 项实施任务 |
| 17 | [**Registry 词表会签表 V1.2**](ai-pm-viz-17-relation-registry-signoff-v1.md) | **V0-1 唯一硬人工依赖**：架构预审修订（V-01~V-12）+ 三裁决（**R-05 方向分离 / R-06 PC 唯一权威 / R-08 反向视图≠新事实**）+ **R-07 定版 `kb.memory_supported_by_source`**（memory→source，一对多实证）+ 三层方向表述（DAG 无需翻转）+ 15 条 Lint + 会签记录表 |
| 19 | [**会签材料包（发送稿 V1.1）**](ai-pm-viz-19-owner-signoff-letters-v1.md) | 两封定向材料（经评审专家审校修订后定稿）：① 致专家增量说明（R-07 修正+命名、R-05 三层方向、复审定位收敛）② 致 Owner 会签材料（R-08 非独立关系措辞 + Q-1~Q-3）+ **发送前九项检查清单** |
| 18 | [**UX 规格 V1.1**](ai-pm-viz-18-v0-8-ux-spec-v1-1.md) | **交互规格权威**（专家 V1.0 + 安全修订 UX-R01~R12）：六原则、角色场景、P0/P1 交互规格、五流程、生命周期、AI 联动、权限一致性、UX 验收 UX-P0-01~11 / UX-P1-01~08、**12 项产品裁决（UX-D01~D12）** |

## 2. 立项核心判断（一段话版）

AI-PM 的关系数据已在库中显式建模，但 UI 看不见——可视化引擎把**关系升级为第三表达层**，使系统从"记录认知"升级为"看见并验证自身认知结构"。工程形态为**共享基础设施**：自有 Spec + 渲染 Adapter + Artifact 版本审批 + 权限继承 + 可拔降级。**专家最终定调：不做图形工具集合，做 AI-PM 的统一关系表达与视觉呈现基础设施。不可妥协原则：Visualization 可以读取、解释、关联和呈现业务事实，但不能创造、修改或批准业务事实。**

## 3. 八个最终问题的回答（评审后定稿）

**Q1 SeeCode 是否适合作为第一代渲染技术基础？**
作为 AI 生成图形产物的渲染器：**实验接入**（P0 完成 vendor 评估 + Adapter Contract Tests 后决定转正，**不作为核心运行依赖**，DV-11）；作为在线交互图引擎：不适合。在线图 P1 采用 G6 5（Q2 包体/性能实测后冻结）。

**Q2 哪些复用、哪些自建？**
复用 SeeCode：产物渲染/SVG 导出、编辑级排版、纠错回路、复杂度预算理念。自建：Spec 权威、Relation Registry、溯源与版本审批、权限投影、图查询聚合、在线 Viewer、Fallback Adapter 与降级矩阵。

**Q3 Spec 如何防第三方绑定？**
Spec 零渲染器字段 + 受控词表 + 三道门禁：多目标契约测试、**Spec Golden Tests（与 SeeCode 无关，防绑定的真正关键）**、降级表格/Mermaid 达标（DV-9/DV-11）。

**Q4 如何确保每个业务关系可追溯？**
生成侧对象 ID 白名单（E 级拒绝编造）+ source_ref 与 kb.memory_ref 同构（revision/content_hash 驱动 stale）+ 展示侧 unlinked 灰标 + AC-16 审计断言"接受图形不改库"。二跳拼接边必须用 `derived_trace`（direct:false, via, structurally_supported）显式标注，Direct/Indirect 影响分开呈现（DV-8）。

**Q5 如何与 AI Conversation / Context Panel / Response Envelope 融合？**
envelope 第 9 类 `viz_artifact`（fail-closed 不变）；Accept/Reject 复用 conversation_interaction；Context Item 既是生成对象来源又接收图上反哺；右栏 Graph Tab。解释图（explanatory）仅存于对话通道（DV-13）。

**Q6 MVP 选哪些场景？**
**评审后调整（DV-14）：P0 不含 Live 图。** P0 = Foundation & Contract（Spec/校验器/Registry/provenance/契约测试/权限投影/一致性）+ Security & Viewer（独立 Origin/CSP/postMessage/注入测试）+ SeeCode 实验接入（HTML/SVG/PNG、中文、降级）；P1 = G6 + 追踪图 + Task DAG + 架构图。

**Q7 独立 Visualization Domain 还是基础设施？**
共享基础设施（DV-1 修订）：独立 crate ≠ 独立 Domain；`pm-domain/visualization/{spec,artifact,adapter,validation,projection}`。服务拆分按**部署与运行边界**：V1 同库同服务；V1.x 可独立渲染 Worker；V2 仅当资源/隔离/多产品共享成刚需再拆。

**Q8 现在做还是晚点做？**
现在立项（评审已通过）：晚做=各领域自造画图方案，溯源/权限无法统一收口；P0 全部是只读投影+展示层簿记，AC-12 可拔测试托底。

## 4. 裁决登记（DV-1~14）

| 编号 | 裁决 | 来源 |
|---|---|---|
| DV-1 | 引擎定位=共享基础设施非独立域；独立 crate ≠ 独立 Domain；服务拆分按部署边界（V1 同库同服务） | 初版 + 评审 Q1 |
| DV-2 | 双 Adapter 架构保留（SeeCode=解释性产物，G6=在线关系图；Cytoscape 仅备选不同时引入；不自研 SVG 引擎） | 初版 + 评审 Q2 |
| DV-3 | Spec 为自有四段式格式；渲染器 spec 仅是编译目标 | 初版 |
| DV-4 | 引擎零业务写路径；接受图形不生效业务变更 | 初版（专家定为不可妥协原则） |
| DV-5 | 迁移自 0020 取号 | 初版 |
| DV-6 | 统一图形数据模型 + 独立响应封装（{data: VizSpec 核心, meta: 查询语义}），API 语义不进 Spec | 评审 Q3 |
| DV-7 | Spec Git 化推迟（安全）；三条件：provenance 齐备 / stale 可识别 / 原 Spec 快照必留 | 评审 Q4 |
| DV-8 | 二跳拼接不新增业务表；derived_trace 合成边显式标注（direct:false + via + structurally_supported）；Direct/Indirect 影响强制区分 | 评审 Q6 |
| DV-9 | Relation Registry：带命名空间受控字符串（domain.relation）+ lint + owner 会签；不做数据库全局 ENUM | 评审 Q5 |
| DV-10 | V1 不部署 headless Chrome，接受 GIF/MP4 内网降级；V2 条件立项；SVG/PNG 导出链零外网须实测 | 评审 Q8 |
| DV-11 | SeeCode 有条件采用：vendor 锁版本、非核心运行依赖、Fallback Adapter + 降级能力矩阵替代"2 周切换"承诺、Spec Golden Tests 防绑定 | 评审 Q10 |
| DV-12 | 安全增强：独立产物 Origin（与 opaque origin 并用）+ CSP 全套走 HTTP Header + postMessage 白名单协议（source+会话标识）+ iframe 零凭证零 API + Live 一致性快照标记 | 评审 Q7/Q12 |
| DV-13 | 新增三项设计：权限过滤在查询+投影双层（边存在性不泄露）、一致性快照、Business/Explanatory 图形性质区分 | 评审 Q9/Q11/Q12/Q13 |
| DV-14 | 实施顺序修订：P0 不含 Live 图；P1=G6 Live Graph | 评审 §七 |

**待技术验证后冻结**：Q2（G6 包体/性能、SeeCode Adapter 实测）、Q7（沙箱+CSP 实测）、Q9（全输出路径注入测试）、Q10（vendor 构建+降级测试）→ 对应任务 V0-4/V0-6/V0-7/V0-9。

## 5. 事实核查声明

- SeeCode 数据取自 2026-11-12 读取的 README / SKILL.md / spec.md / package.json（main 分支）；star 数为第三方统计未经官方页核实；转义行为、贡献者数量等未承诺项已列入 P0 验收测试而非假设。
- AI-PM 侧表名/路径引用来自本次代码摸底（migrations/0001~0019 等）；关键缺口（无图查询端点、前端零图形库、Task↔Design 二跳）已进入 G-1~G-5。
- 专家评审意见基于架构摘要作出，专家声明"涉及具体实现细节的结论应在正式冻结前再核验"——对应待验证项已排入 P0 任务。
