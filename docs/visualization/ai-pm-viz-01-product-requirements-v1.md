# AI-PM Visualization Engine 产品需求说明书 V1.0

> 状态：V1.1 已修订（并入 2026-11-12 专家评审；决策基线见 [ADR V1.0](ai-pm-viz-09-architecture-decision-record-v1.md)） · 日期：2026-11-12 · 配套文档见 [00 索引](ai-pm-viz-00-index-v1.md)

## 1. 立项价值：从「信息展示系统」到「认知交互系统」

AI-PM 当前的信息表达只有两层：**文本叙事**（Markdown 文档、对话）与**结构化记录**（表格、Card、列表）。而系统的核心资产——对象之间的**关系**（追踪、依赖、溯源、传播）——只能以文字描述或单条列表的形式被"线性"阅读。

| 表达层 | 承载内容 | 现状 | 用户动作成本 |
|---|---|---|---|
| 文本叙事 | 知识内容、决策理由 | ✅ Markdown+Git 权威已建 | 读长文，靠人脑拼装关系 |
| 结构化记录 | 对象状态、属性 | ✅ 表格/Card 已建 | 逐条翻找，关系不可见 |
| **关系图形** | 对象间的拓扑、路径、依赖 | ❌ 缺失 | **高**：跨页面手动拼接 |

**立项判断**：AI-PM 已经把关系写进了数据库（见 §2 证据），但用户在 UI 上"看不见"这些关系。可视化引擎不是增加一个画图页面，而是把**关系升级为一等公民的表达层**——使 AI-PM 从"记录认知的系统"升级为"可以看见、验证、追问自身认知结构的系统"。这是本立项的真正产品价值。

**为什么现在做**：
1. 关系数据已就绪：追踪链、依赖、溯源的存储层建模已完成并显式落表（见 §2），可视化是纯读投影，边际成本低。
2. AI 协作工作台（CX1~CX4、Response Envelope）已落地，图形作为对话产物有了天然的宿主与审批通道。
3. 延迟的代价是架构性的：若各领域各自引入画图方案（截图贴图、mermaid 片段、一次性脚本），溯源与权限将无法统一收口，返工成本高于现在建一层 Spec。

## 2. 领域事实底座（关系数据已显式建模，均经代码核实）

| 关系 | 存储位置 | 说明 |
|---|---|---|
| Requirement↔Design | `design.design_requirement`（migrations/0010:120） | design_revision_id + requirement(_revision)_id |
| Requirement↔ADR | `design.adr_requirement`（0010:220） | 同构 |
| Design↔ADR | `design.adr_design_revision`（0010:204） | 同构 |
| Task↔Requirement | `delivery.task_requirement`（0001:1881） | (task_id, revision_id) 主键 |
| Task 依赖 | `delivery.task_dependency`（0001:1754） | task_id / depends_on_task_id，防自依赖 |
| 产品认知对象关系 | `pc.relationship(_revision/_type)`（0001:3293/3327/3369） | 带修订与类型体系 |
| 项目技术认知 | `ai.cognition_item`（0001:770，domain 六分类） | identity/repository/domain/architecture/convention/evolution |
| 知识溯源 | `kb.memory_ref`（0001:2914） | target_domain/type/id/revision_id + locator |
| Agent 执行事件 | `delivery.task_run_event`（0013，追加式、幂等） | 时序回放数据源 |
| 对话对象引用 | `ai.conversation_context_item`（0019，多态+revision 引用） | 图形选中对象的注入协议现成 |

**已知缺口**（不阻塞立项，进入任务拆解）：Task↔Design/ADR 无直接关联表（需经 task_requirement + design_requirement 二跳拼接）；后端无图/边聚合查询端点；前端零图形库（ai-frontend/package.json 无 mermaid/d3/g6/echarts/vue-flow）。

## 3. 产品定位与原则

**定位**：AI-PM 的基础产品能力层——把结构化业务认知转换为可交互、可追溯、可版本化的图形表达。不是独立画图工具，不是 BI，不是白板。

**五条产品原则**（对应立项约束，全部为硬性）：

| # | 原则 | 需求含义 |
|---|---|---|
| P1 | 可视化不是知识权威 | 图形是派生展示产物；图形中的业务判断生效必须走原领域审批；图形不产生新业务事实 |
| P2 | 统一 Spec，渲染可替换 | 领域只出数据与关系；AI-PM 自有 Visualization Spec；SeeCode/G6/Mermaid 等一律是 Adapter，领域禁止依赖任何渲染器内部格式 |
| P3 | 人工确认机制 | Preview / Accept / Reject / Feedback / Compare / Source Traceability / Stale Detection 七项必备；接受图形 ≠ 接受其中全部业务关系 |
| P4 | 不影响业务独立性 | 引擎整体不可用时，需求/知识/任务/分析全部照常工作；前端 lazy 加载，后端独立路由分区 |
| P5 | 权限与安全 | 图形继承原对象权限；sandbox 隔离；模型输出不直接执行；私有知识不出域 |

## 4. 目标用户与核心场景

| 用户 | 高价值动作 | 对应图 |
|---|---|---|
| 产品负责人 | 看产品能力地图、模块关系，验证 AI 产出的认知是否成体系 | 能力地图、领域关系图 |
| 架构师 | 看技术架构、依赖，做变更影响分析 | 架构图、追踪图、影响分析图 |
| 开发/执行 | 看任务依赖与关键路径，知道"先做什么、卡在哪" | Task DAG、泳道 |
| 所有用户 | 在 AI 对话中要一张图并相信它 | 对话内生成图 + 溯源 |

## 5. 功能需求

### FR-1 图形资产（Artifact）与版本
- 图形以 Artifact 形式持久化：`viz_artifact`（spec + 渲染产物 + 来源指纹 + 版本号 + 状态机）。
- 状态机：`draft → published → stale`（+ `rejected`）；草稿不可见（仅作者预览），发布需审批（P3）。
- 同一"图形槽位"（slot，如 `project:{id}/architecture-map`）多版本并存，可对比、可回看。
- Stale 判定：Artifact 记录其输入数据指纹（source_hash：所引用对象 id+revision_id+content_hash 集合的哈希）；任一源对象变更 → 标记 stale，不删除旧版本。

### FR-2 在线图视图（Live View）
- 六领域图视图（清单见 D5 矩阵），数据来自图查询端点实时聚合，随数据即时更新，不落 Artifact。
- Live View 无审批语义（只读投影），但有完整权限继承（P5）。
- 影响分析区分 **Direct**（直接业务关系支撑）与 **Indirect**（经中间对象推导的传播）两类，图中显式区分呈现，不得混同（DV-8）。

### FR-3 AI 对话内生成图形
- Response Envelope 新增 `viz_artifact` 响应类型（白名单内扩展，保持 fail-closed）。
- 对话内卡片：图形预览（sandbox iframe）+ Accept / Reject / Feedback + 跳转源文档。
- Accept 后进入该槽位的版本历史；Reject 必带 reason（复用 `ai.conversation_interaction` 语义）。
- 生成侧校验：spec 引用的对象 ID 必须存在于当前项目上下文，引用不存在 → 生成失败（E 级），不允许"编造节点"。

### FR-4 溯源交互
- 图中每个节点/边可点击查看来源：`source_ref`（domain/type/id/revision/locator，形态对齐 `kb.memory_ref`）→ 跳转原文档与定位点。
- 无溯源的节点/边必须显式标注 `unlinked`（灰色/图标），不可静默冒充已核实知识。

### FR-5 审批与版本管理
- 复用 `design.design_approval` 模式（target_hash 乐观并发、每槽位至多一条 pending、409 语义）。
- Compare：任意两版本的 spec diff + 视觉并排。
- 关键约束（重申）：**接受图形不生效任何业务关系变更**；若用户在图形上发现关系错误，修正动作指向原领域（需求确认、design approval 等），引擎只提供跳转。

### FR-6 导出
- V1 导出能力：独立 HTML / SVG / PNG（**不部署 headless Chrome**，DV-10）；GIF/MP4 推迟至 V2——确有客户需求再建独立渲染 Worker。
- 导出物带来源水印（图形 + 槽位 + 版本 + 生成时间），可关闭。
- 内网部署默认 `fonts=system`、外部编码器禁用；SVG/PNG 导出链路需**实测**零外网请求，不以格式名称推断（AC-6）。

### FR-7 Stale 与刷新
- Viewer 打开时校验 source_hash，stale 图形顶部黄条提示，提供"重新生成"入口（重新走 P3 审批）。

### FR-8 权限
- Artifact 与 Live 查询均经 `require_project_visible` 同一门；跨项目引用 404 同形（不泄露存在性）；api_key 访问人类端点 403（对齐 `pm_requirement_read.rs` 既有先例）。
- 过滤发生在**业务查询与图形投影阶段**（服务端裁剪，非前端隐藏）；无权对象不得经由节点、Tooltip、**边存在性或关系计数**泄露任何信息（DV-13 新增项，AC-17）。

### FR-9 图形性质区分（DV-13 新增项）

| 性质 | 数据来源 | 约束 |
|---|---|---|
| Business Visualization | 业务数据库与正式知识 | 必须使用真实业务 ID 与真实关系（库内可校验） |
| Explanatory Visualization | AI 对话中的假设/示意内容 | 恒显「AI 生成 · 非正式认知」标识；不得发布至业务槽位、不得冒充 Project Cognition 事实 |

AI 可以在对话中画假设性的未来架构图，但假设节点不得进入业务认知（服务端 + UI 双重断言，AC-20）。

## 6. 非功能需求

| 项 | 要求 |
|---|---|
| 性能-图形规模分档 | S ≤30 节点（完整交互）；M ≤150（视口裁剪+LOD）；L ≤500（聚合/折叠+分页探索）；>500 拒绝渲染并提示拆分（对齐"复杂度预算"理念） |
| 性能-首屏 | Viewer chunk lazy 加载，不进主 bundle；Live 查询 P95 < 800ms（500 边内） |
| 降级 | 渲染失败/引擎禁用 → 原位显示数据表格降级视图 + 错误说明，业务功能零影响 |
| 中文 | 节点/边标签中文为第一公民：换行、字号、图例、导出内嵌字体全部验收（SeeCode `lang:"zh-CN"` 与 G6 均支持，作为验收用例而非假设） |
| i18n | Viewer 文案走 `viz.*` 命名空间，过 `check:i18n` 门禁 |
| 视觉 | 沿用 PRISM 令牌与 V2 深色体系（s05 锁定），禁止硬编码色值 |
| 合规 | 发布构建零 mock（AGENTS.md 规则 4 / release-guard）；产物不出域（禁 Google Fonts 外网请求，用 system fonts 模式） |

## 7. 范围界定（不做的事）

1. **不做图形编辑器**：不提供拖拽改图并回写业务的画布；对图的"修改"= 重新生成 + 审批。
2. **不做 BI 报表**：指标趋势图归分析域既有通道。
3. **图形不替代文档**：正式知识仍以 Markdown+Git 为权威，图是索引与导航。
4. **不做独立部署域**：引擎是 AI-PM 内的基础设施模块（论证见 D4 §7），不引入独立渲染微服务（P0 阶段）。
5. **不在 P0 做实时协同编辑**：多人同图并发编辑不在本期。

## 8. 成功度量

| 度量 | 目标（P0 结束时） |
|---|---|
| 对话内生成图采纳率（Accept/总生成） | ≥ 50%（说明生成质量可用） |
| 溯源覆盖率（有 source_ref 的节点占比） | ≥ 90%（生成侧强制校验后应接近 100%） |
| Stale 检测正确率 | 源变更后 stale 标记 100%（可测） |
| 业务零影响 | 关闭 viz 路由分区，全量既有测试绿（可测） |
| 中文验收 | 指定中文 spec 用例在双 Adapter 下渲染一致（可测） |

## 9. 里程碑与后续文档

- MVP 范围与任务拆解：[D7 路线图](ai-pm-viz-07-roadmap-v1.md)
- 架构影响分析：[D4 系统架构](ai-pm-viz-04-system-architecture-v1.md)
- Spec 设计：[D3 Visualization Spec V1.0](ai-pm-viz-03-visualization-spec-v1.md)
- 渲染技术选型：[D2 SeeCode 调研](ai-pm-viz-02-seecode-research-v1.md)
- 风险与验收：[D8](ai-pm-viz-08-risks-acceptance-v1.md)
