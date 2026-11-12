# AI-PM Visualization Engine Architecture Decision Record V1.0.1

> **状态**：Conditional Approved —— 附条件通过，进入 P0 实施准备阶段  
> **版本**：V1.0.1  
> **日期**：2026-11-12  
> **基线**：V1.0 + 2026-11-12 架构复审意见  
> **适用范围**：AI-PM Visualization Engine  
> **修订性质**：架构边界闭合、验收条件补充、实施风险修正  
> **原始基线**：docs/visualization/ 文档包 V1.1  
> **裁决追踪**：保留原 DV-1～DV-14、ADR-VIZ-001～011、AC-1～21 编号，不重新编号。

---

# 1. 修订说明

## 1.1 修订目标

本版本根据 V1.0 实施前架构复审意见，对原架构决策进行补充和闭合。

本次不重新讨论已经确定的架构方向，重点解决以下问题：

1. Migration 编号和迁移责任的确定方式。
2. P0/P1 阶段与 AC-1～21 的验收边界。
3. PNG 导出的实现路径。
4. Artifact 与 Approval 状态机的职责分离。
5. Artifact 数据库存储容量与清理策略。
6. nature 的表达粒度。
7. Live consistency 的版本语义。
8. 产物 Origin、Cookie 与 postMessage 安全约束。
9. Relation Registry 的版本治理。
10. VIZ_ENABLED 关闭时的完整隔离验证。

## 1.2 本次不改变的核心决策

以下原始决策保持不变：

- Visualization 是共享基础设施，不是独立业务 Domain。
- Visualization 不拥有业务事实，不创造、修改或批准业务事实。
- VizSpec 是 AI-PM 自有的统一图形数据模型。
- Live 与 Artifact 复用同一核心 Spec。
- Relation Registry 负责受控映射与校验，关系语义由领域 Owner 确认。
- 不新增业务领域写路径。
- Task 与 Design/ADR 继续使用二跳关系推导，不为画图新增 task_design 表。
- Artifact 与正式知识权威分离。
- 业务查询与图形投影双层执行权限过滤。
- HTML 产物采用独立 Origin、sandbox iframe 和 CSP。
- P0 不包含 Live Graph。
- GIF/MP4 继续 Deferred。

## 1.3 复审结论

本 ADR 获得附条件通过。

允许启动不依赖阻断项的 P0 准备工作，包括：

- VizSpec 设计与校验器。
- Relation Registry 设计。
- Artifact 数据模型设计。
- Contract 与 Golden Tests。
- 权限投影与一致性设计。
- 安全策略及自动化测试设计。

在以下三个问题闭合前，不建议启动对应的完整交付：

| 编号 | 阻断项 | 完成条件 |
|---|---|---|
| R-01 | Migration 编号 | 核对真实 migration 文件与 migration ledger，确定下一个可用编号及负责人 |
| R-02 | AC 阶段归属 | 明确 AC-1～21 的 P0/P1 验收范围和依赖 |
| R-03 | PNG 导出路径 | 明确实现方案并通过内网零外网验证，或明确降级条件 |

---

# 2. 不可妥协原则

本 ADR 继续以以下原则作为所有技术决策的最高约束：

> **Visualization 可以读取、解释、关联和呈现业务事实，但不能创造、修改或批准业务事实。**

配套原则：

- P1：图形不是知识权威。
- P2：统一 Spec，渲染器可替换。
- P3：人工确认七件套。
- P4：业务独立性可拔。
- P5：权限继承与安全。

## 2.1 零业务领域写路径

原 ADR-005 中的“零业务写路径”进一步明确为：

**Zero Business-Domain Write Path（零业务领域写路径）。**

Visualization 可以写入自己的基础设施数据，例如：

- viz.artifact
- viz.approval
- viz.render_output
- viz.slot_config

但不允许通过 Visualization 修改：

- Requirement
- Design
- ADR
- Task
- Project Cognition
- Product Cognition
- Knowledge
- 其他业务领域权威对象

Artifact 发布审批仅批准图形产物，不代表批准或修改它所引用的业务事实。

AC-16 应验证上述业务边界，而不是简单断言 Visualization 不能执行任何数据库写操作。

---

# 3. ADR 决策记录

## ADR-VIZ-001 定位与部署形态

**状态：Accepted（保持不变）**

### 决策

采用共享基础设施 shared kernel。

独立 crate 不等于独立 Domain。

目录结构：

- pm-domain/visualization/{spec, artifact, adapter, validation, projection}
- pm-application visualization services
- pm-api /v1/visualizations/*
- 前端 src/components/viz/

部署策略：

| 阶段 | 部署方式 |
|---|---|
| V1 | 同库、同服务 |
| V1.x | 根据运行需求允许独立渲染 Worker |
| V2 | 出现明确的资源压力、多产品共享、独立扩缩容、故障隔离或安全边界需求后，再评估独立服务 |

不提前拆分微服务。

### VIZ_ENABLED 关闭要求

VIZ_ENABLED=false 必须实现完整的功能隔离：

1. Visualization API 不提供正常业务访问能力。
2. 前端不展示 Visualization 功能入口。
3. Visualization 专属后台任务不再被调度或执行。
4. 既有 AI 对话、Task、Requirement、Design、Knowledge 等链路不受影响。
5. 既有业务模块不依赖 Visualization 的初始化。
6. 关闭 Visualization 后，原业务数据仍然可正常读取。
7. 已有 Artifact 不得导致业务请求阻塞。

### 验收

AC-12。

除原有可拔回归外，增加关闭状态下的后台任务、初始化和既有业务链路测试。

---

## ADR-VIZ-002 渲染 Adapter 策略

**状态：Conditional（冻结条件 V-1/V-4）**

### 决策

维持三层渲染架构。

#### Adapter A：在线交互

目标：P1。

技术方向：AntV G6 5。

要求：

- 动态加载。
- 独立 lazy chunk。
- 布局按需注册。
- 不进入主应用 bundle。
- Cytoscape 只作为未来关系分析备选，不同时引入。
- 不自研完整 SVG 引擎。
- Vue Flow 仅在未来出现节点编辑需求时单独评估。

#### Adapter B：解释性产物

目标：P0 实验接入。

技术方向：SeeCode。

约束：

- vendor 锁定版本。
- 存放于 tools/seecode/。
- MIT 声明保留。
- 非核心运行依赖。
- 仅使用 render（HTML）和 export --for svg。
- 渲染失败时使用 Fallback，不阻断业务链路。

#### Fallback Adapter

常备能力：

- 结构化表格。
- Mermaid。

降级矩阵必须按实际 Kind 建立，不得只依赖大类名称。

每个 Kind 至少明确：

1. P0 主呈现方式。
2. P0 降级方式。
3. P1 增强方式。
4. 降级后失去的交互能力。
5. 降级后失去的导出能力。

建议初始矩阵：

| Kind | P0 主呈现 | P0 降级 | P1 增强 |
|---|---|---|---|
| Flowchart | SeeCode 实验 | Mermaid/表格 | 根据需要评估 G6 |
| Sequence | SeeCode 实验 | Mermaid/表格 | 根据需要评估 |
| Trace | 结构化表格或适用静态产物 | 结构化表格 | G6 |
| DAG | 结构化表格或适用静态产物 | 结构化表格 | G6 |

以上是实施建议矩阵，最终 Kind 范围以 D3 Spec 为准。

### Adapter Contract

必须满足：

- Contract 不依赖特定渲染器。
- Spec 不包含渲染器专属字段。
- 不允许 Adapter 反向污染 VizSpec。
- 不同 Adapter 通过同一套核心 Contract 验证。
- Spec Golden Tests 与 SeeCode 解耦。

### AC-2 验收范围调整

AC-2 保留原编号，但明确分阶段：

**P0：**

- 每个已交付 Kind 至少具备一个可用主呈现路径。
- 每个已交付 Kind 必须具备一个可用 Fallback。
- Adapter Contract 和降级行为通过测试。
- 不要求 G6 在 P0 上线。

**P1：**

- G6 Adapter 接入。
- 完成对应 Kind 的交互增强。
- 完成 G6 性能和独立加载验收。

是否满足原 AC-2 对多个渲染目标的完整要求，必须结合 D8 原始定义确认，不得用表格自动替代尚未定义的第二个渲染目标。

### 验收

AC-2、AC-21。

V-1、V-4 通过后冻结具体 Adapter 实现。

---

## ADR-VIZ-003 Spec 数据模型与响应封装

**状态：Accepted（保持不变）**

### 决策

采用 AI-PM 自有四段式 VizSpec：

- view
- nodes
- edges
- meta

不包含渲染器专属字段。

Live 与 Artifact 复用同一核心模型。

Live 响应采用独立响应封装：

- data：VizSpec 核心数据。
- meta：source_revision、generated_at、consistency、truncated 等响应元数据。

API 查询游标、分页、权限错误和缓存状态属于响应层语义，不进入 VizSpec 核心。

source_ref 与 kb.memory_ref 同构：

- domain
- type
- id
- revision_id
- locator
- content_hash

### 增补：Spec Contract

D3 必须明确以下规则：

| 项目 | 规则要求 |
|---|---|
| schema_version | 明确版本及兼容策略 |
| Node ID | 唯一性规则 |
| Edge ID | 唯一性规则 |
| Edge 引用 | source/target 必须指向有效节点 |
| 类型 | Node/Edge 类型受 Registry 约束 |
| 大小限制 | 节点数、边数、文本长度和 payload 上限 |
| 确定性 | 明确 Spec hash 的计算输入 |
| 校验 | 明确 E/W/I 分级及阻断行为 |

### 兼容要求

旧版本 Artifact 必须有明确的读取和兼容策略。

不得依赖前后端同步升级才能读取历史 Spec。

### 验收

AC-1。

---

## ADR-VIZ-004 Relation Registry 词表治理

**状态：Accepted（词表内容完成领域 Owner 会签后冻结）**

### 决策

继续采用受控字符串：

domain.relation

例如：

- task.implements_requirement
- derived_trace

Registry 负责：

- schema 校验。
- lint 白名单。
- 关系类型映射。
- 关系使用合法性检查。
- 领域 Owner 会签记录。

不采用数据库全局 ENUM。

### 增补：治理机制

每个 relation 必须明确：

1. 唯一 relation key。
2. 业务语义定义。
3. source object type。
4. target object type。
5. 关系方向。
6. 所属领域。
7. 领域 Owner。
8. 状态及版本。

### 关系变更规则

- 新增关系必须经过领域 Owner 确认。
- 已有关系不能直接改变业务语义。
- 语义变更应视为新版本或新关系。
- 废弃关系必须保留历史解释能力。
- 历史数据不能因为关系废弃而失去可解释性。
- lint 检查未登记关系、类型不匹配和错误使用。

Registry 是映射与校验机制，不是新的业务关系权威。

### 实施要求

领域 Owner 会签必须在 V0-1 内完成。

不能将会签作为 V0-1 完成后的补充工作。

### 验收

AC-1。

---

## ADR-VIZ-005 业务权威边界与追踪语义

**状态：Accepted（保持不变）**

### 决策

维持零业务领域写路径。

Visualization 模块禁止 import 领域写 service。

Task↔Design/ADR 继续采用二跳关系推导。

不新增 task_design 业务表。

合成边必须显式标注：

```json
{
  "relation": "derived_trace",
  "direct": false,
  "via": ["requirement_id"],
  "semantic_confidence": "structurally_supported"
}
```

影响分析必须区分：

- Direct：直接业务关系。
- Indirect：经中间对象推导的关系。

### UI 要求

Indirect 关系必须展示推导路径。

例如：

Task → Requirement → Design

不得将二跳推导结果展示成数据库中真实存在的直接关系。

### 未来新增直连关系的条件

只有同时满足以下条件，才允许重新评估 task_design：

1. 真实业务流程要求记录 Task 对 Design 的直接实施关系。
2. 现有 Requirement 关系不能可靠推导。
3. 领域 Owner 确认这是业务事实，而非图形展示需求。

### 验收

AC-16。

AC-16 验证 Visualization 不改变业务领域数据，不限制其对自身 Artifact 数据的正常写入。

---

## ADR-VIZ-006 Artifact 生命周期与存储

**状态：Accepted（补充状态模型和存储约束）**

### 决策

新增：

- viz.artifact
- viz.approval
- viz.render_output
- viz.slot_config

迁移编号不预设为 0020。

### Migration 编号与责任

Visualization Migration 使用当前 migration ledger 的下一个可用编号。

V0-1 启动前必须：

1. 检查仓库中现有 migration 文件。
2. 检查 docs/ai-pm-migration-ledger.md。
3. 确认当前最高已使用编号。
4. 分配下一个可用编号。
5. 指定 Migration Owner。
6. 将编号、用途、依赖和回滚责任登记入台账。

Migration Owner 必须由实施团队明确指定。

### Artifact 状态机

Artifact 状态：

- draft
- published
- stale
- rejected

状态流转：

draft → published → stale

draft → rejected

发布版内容不可变。

published 转为 stale 时，只改变生命周期状态，不修改历史发布内容。

### Approval 状态机

Approval 独立于 Artifact：

- pending
- approved
- rejected
- cancelled（如果支持撤回）

审批状态不应直接混入 Artifact 状态字段。

### Slot 并发约束

每个 slot 最多允许一条 pending Approval。

必须由数据库唯一约束、事务机制或等效的并发控制保障。

不能只依赖应用层先查询再写入。

### Source Hash 与 Stale

source_hash 必须有明确计算规则，至少说明：

- 来源业务对象。
- 来源 revision。
- Spec 核心内容。
- 参与推导的关系版本。
- 影响图形解释的配置版本。

stale 检测方式在 V0-3 确定。

不自动重渲染。

重新生成或重新发布必须形成新的 Artifact 版本，不覆盖历史发布版。

### PostgreSQL 存储约束

渲染产物继续优先存 PG，但 V0-3 必须明确：

- 单个 Artifact 最大大小。
- 单个 Render Output 最大大小。
- 历史版本保留策略。
- 失败产物保留策略。
- 清理策略。
- 备份和恢复影响。
- 大文件读取成本。

派生物可重建不代表允许无限累积。

### Spec Git 化

继续推迟全量 Git 化。

只有满足以下条件后再评估：

1. provenance 完整。
2. stale 可识别。
3. 原 Spec 快照必留。

正式交付物类图形按正式文档进入 Git 权威链。

临时 Artifact 不要求全部 Git 化。

### 验收

AC-3、AC-4。

---

## ADR-VIZ-007 导出与外网策略

**状态：Accepted（PNG 路径待验证）**

### 决策

V1 导出目标：

- HTML
- SVG
- PNG（路径验证通过后）

GIF/MP4 Deferred。

不部署 headless Chrome。

内网默认：

- fonts=system。
- 禁止外部编码器。
- 页面内位图导出关闭。
- 不依赖外部 CDN。

### 导出能力矩阵

| 格式 | V1 状态 | 实现要求 |
|---|---|---|
| HTML | 支持 | SeeCode render，独立 Origin |
| SVG | 支持 | SeeCode SVG export，零外网实测 |
| PNG | 条件支持 | 必须明确本地生成路径并完成验证 |
| GIF | Deferred | V2 条件项 |
| MP4 | Deferred | V2 条件项 |

### PNG 实现要求

当前架构不允许仅凭 HTML 或 SVG 输出就推定 PNG 可用。

P0 必须明确：

1. PNG 由哪个组件生成。
2. 是否需要额外运行时。
3. 是否依赖浏览器截图。
4. 是否需要外部编码器。
5. 是否满足内网零外网要求。
6. 中文字体是否正确。
7. 生成失败是否能够降级。

如果 P0 无法完成验证，PNG 应从 V1 无条件能力调整为条件支持，不得对外承诺已经具备。

### 零外网验收

必须实测：

- 不访问 CDN。
- 不访问外部字体服务。
- 不加载远程图片。
- 不加载外部脚本。
- 不因导出失败自动访问外部服务。

### GIF/MP4

继续 Deferred。

只有出现明确客户需求，并且本地 Chrome/Edge 导出能力通过验证后，才单独立项渲染 Worker。

### 验收

AC-6。

---

## ADR-VIZ-008 产物安全隔离与通信

**状态：Conditional（冻结条件 V-2）**

### 决策

HTML 产物按不可信主动内容处理。

采用五层安全策略：

1. 独立产物 Origin。
2. sandbox iframe。
3. CSP HTTP Response Header。
4. iframe 零凭证、零 AI-PM API。
5. postMessage 白名单协议及宿主二次权限检查。

iframe 不授予：

- allow-same-origin
- allow-top-navigation

### 独立 Origin 与 Cookie

必须确保：

- 主应用认证 Cookie 使用 host-only 策略。
- 产物 Origin 不接收主应用认证凭证。
- 不共享认证状态。
- 产物页面不能访问主应用存储。
- 不得通过子域共享 Cookie 破坏隔离。

独立 Origin 必须通过实际部署环境验证，不能只根据域名不同认定已经隔离。

### CSP

CSP 必须通过 HTTP Response Header 下发。

meta CSP 不能替代 Header。

基线：

```text
default-src 'none';
connect-src 'none';
form-action 'none';
object-src 'none';
base-uri 'none';
frame-ancestors <app-origin>;
```

其他资源策略根据实际产物收紧：

- script-src
- style-src
- img-src
- font-src
- worker-src
- frame-src

默认禁止：

- Worker。
- 顶层导航。
- 弹窗。
- 任意外部资源。
- 未授权的表单提交。

### postMessage

协议必须校验：

1. event.source。
2. 随机 session ID。
3. message type 白名单。
4. payload schema。
5. 当前图形节点 ID 合法性。
6. 消息长度。
7. 消息频率。

节点 ID 回传宿主后，宿主必须重新查询业务数据并执行权限检查。

不能直接信任 iframe 提交的节点内容。

### 产物安全边界

iframe 不具备：

- AI-PM API 调用能力。
- 主应用认证凭证。
- 业务对象写入能力。
- 自主权限判断能力。

### 验收

AC-7、AC-8、AC-18。

V-2 通过后冻结。

---

## ADR-VIZ-009 权限投影、一致性与图形性质

**状态：Accepted（补充元数据防泄露和 nature 粒度）**

### 决策

权限过滤必须在两个层次执行：

1. 业务查询层。
2. 图形投影层。

不得只依赖前端隐藏节点。

无权限对象不能通过以下方式泄露：

- Node。
- Edge。
- Tooltip。
- Focus Panel。
- 关系计数。
- 聚合数量。
- 搜索结果总数。
- 截断信息。
- 布局预留位置。
- 错误消息。
- 日志。
- 缓存和响应行为。

### Live 一致性

Live 响应 meta 必须包含 consistency：

- current
- mixed-revision

并提供各对象 revision refs。

禁止无标记的混合时点数据冒充同一时点完整图。

### current 的语义

V0-3 必须定义 current 的实际语义：

- 使用了哪些权威来源。
- 每个来源对应的 revision。
- 读取时间边界。
- 是否来自同一事务或同一一致性快照。
- 哪些场景必须标记 mixed-revision。
- 无法证明一致性时如何降级。

不得仅凭一次数据库事务就声称跨多个权威来源已经达到统一时点一致性。

### nature 图形性质

继续区分：

- business：真实业务事实。
- explanatory：AI 生成的假设内容。

explanatory 必须恒显：

“AI 生成 · 非正式认知”。

不得发布至业务正式认知槽位。

服务端和 UI 双重断言。

### 粒度补充

V0-3 必须明确 nature 的粒度：

- Artifact 是否具有整体 nature。
- Node 是否分别具有 nature。
- Edge 是否分别具有 nature。
- 同一个图形是否允许混合 business 与 explanatory。
- derived_trace 与 explanatory 是否属于不同语义维度。

建议将 nature 明确到 Node/Edge 层级，Artifact 层仅表达整体组成情况。

derived_trace 表示关系推导方式，不应与 business/explanatory 的事实性质混为一谈。

### 验收

AC-17、AC-19、AC-20。

---

## ADR-VIZ-010 输入转义与注入防线

**状态：Conditional（冻结条件 V-3）**

### 决策

维持三层防线。

#### 输入层

- label 一律作为纯文本。
- 限制长度。
- 拒绝控制字符。
- URL、图片地址、CSS 属性分别验证。
- 模型输出不作 HTML。

#### 渲染层

- 验证 SeeCode 是否使用安全文本 API。
- 禁止不可信数据通过 innerHTML 拼接。
- HTML 模板必须做上下文转义或严格 Sanitization。
- 禁止盲目预转义。

#### 测试层

必须覆盖所有输出位置：

- Node label。
- Edge label。
- Tooltip。
- Focus Panel。
- HTML 导出。
- SVG 导出。

注入向量覆盖：

- HTML 注入。
- 事件属性注入。
- URL 协议注入。
- 外部资源注入。
- SVG 注入。
- Unicode。
- RTL。
- 零宽字符。
- 双重转义。
- 超长输入。

### 硬门槛

SeeCode 未通过安全渲染测试时：

- 不允许接收不可信 AI Spec。
- 不允许通过关闭测试或降低测试覆盖来放行。
- 使用安全 Fallback，或更换 Adapter。

安全不能外包给 iframe 沙箱。

### 验收

AC-9。

V-3 通过后冻结。

---

## ADR-VIZ-011 实施顺序与验证门

**状态：Accepted（补充 AC 分阶段验收规则）**

### 实施顺序

#### P0：Foundation & Contract

V0-1～V0-5：

- VizSpec。
- 校验器。
- Relation Registry。
- Provenance。
- 权限投影。
- 一致性语义。
- Contract Tests。
- Golden Tests。
- Fallback。
- AI 链路实验。

#### P0：Security & Viewer

V0-6～V0-8：

- 独立 Origin。
- CSP。
- postMessage。
- 注入与转义。
- Viewer 组件。
- 安全降级。

#### P0：First Renderer

V0-9～V0-10：

- SeeCode vendor 评估。
- HTML/SVG。
- PNG 路径验证。
- 中文用例。
- 失败降级。

SeeCode 属于实验接入，不是核心运行依赖。

#### P1：Live Graph

G6 验证后进入：

- 追踪图。
- Task DAG。
- 架构图。
- Direct/Indirect 影响。
- 上下文反哺。

#### P2：AI Collaboration Visualization

#### P3：Project Intelligence

GIF/MP4 仍为条件项。

### AC 阶段验收

AC-1～21 保留为整个 Visualization Engine 的完整验收基线。

但不得将 P1 能力提前作为 P0 的强制交付。

| 阶段 | 验收要求 |
|---|---|
| P0 | 完成基础 Spec、校验、Registry、权限、一致性、Artifact、安全、Fallback、SeeCode 实验和可拔验证 |
| P1 | 完成 G6 Adapter、Live Graph、交互、性能和对应 Contract |
| P2 | 完成 AI Collaboration Visualization 对应验收 |
| P3 | 完成 Project Intelligence 及条件性动画能力 |

AC-1～21 的具体条款必须逐条标注阶段、依赖和验收证据。

当前已明确的阶段归属如下：

| AC | 主要验收主题 | 阶段 |
|---|---|---|
| AC-1 | VizSpec 校验 | P0 |
| AC-2 | Adapter 与降级能力 | P0 基础能力；P1 G6 完整能力 |
| AC-3 | Artifact 生命周期 | P0 |
| AC-4 | Artifact provenance / 版本 | P0 |
| AC-6 | 导出能力 | P0，PNG 路径通过验证后 |
| AC-7 | 安全隔离 | P0 |
| AC-8 | postMessage 通信安全 | P0 |
| AC-9 | 输入转义与注入防线 | P0 |
| AC-12 | Visualization 可拔能力 | P0 |
| AC-16 | 零业务领域写路径 | P0 |
| AC-17 | 权限投影 | P0 |
| AC-18 | 安全通信 | P0 |
| AC-19 | 一致性语义 | P0 |
| AC-20 | business/explanatory 区分 | P0 |
| AC-21 | Contract 与 Golden Tests | P0 基础能力；P1 Adapter 扩展 |

AC-5、AC-10、AC-11、AC-13、AC-14、AC-15 的具体归属必须依据原始 AC 文本在 D8 中核对后登记，不在本修订稿中推测其内容。

### M1

P0 全部验收通过：

- P0 归属的 AC 全部通过。
- V-2、V-3、V-4 通过。
- V-1 的 G6 验证不作为 P0 M1 阻断项，但必须在 P1 前通过。
- 既有业务测试全部通过。
- VIZ_ENABLED 关闭回归通过。
- Viz 可拔验证通过。

### M2

P1 完成：

- 三类 Live Graph 上线。
- G6 动态加载通过。
- G6 性能验证通过。
- 交互能力验收通过。
- 对应 AC 和 Contract 全部通过。

---

# 4. 技术验证项与冻结条件

## V-1：G6 5 引入验证

**对应：ADR-002、ADR-011。**

真实构建验证：

- 动态加载。
- 独立 chunk。
- 布局按需注册。
- 150 节点交互。
- 不进入主 bundle。

初始目标：

- gzip ≤350KB。
- 150 节点交互 ≥50fps。

正式验收前必须固定：

1. 浏览器版本。
2. 测试设备规格。
3. 节点和边数量。
4. 图形复杂度。
5. 交互操作集。
6. 测试时长。
7. 帧率统计方法。
8. 首次加载与稳定交互阶段。

建议记录 p50/p95 性能数据。

上述性能指标属于待复审确认的初始目标，不是已经完成的实测结果。

## V-2：沙箱与 CSP 实测

**对应：ADR-008。**

自动化验证：

- 独立 Origin。
- opaque origin。
- 零凭证。
- 零 AI-PM API。
- CSP Header 生效。
- 禁止非白名单 postMessage。
- Cookie 不跨应用共享。
- iframe 无越权导航能力。

AC-7/8/18 全部通过后冻结。

## V-3：SeeCode 全输出路径注入测试

**对应：ADR-010。**

测试覆盖：

- 所有输出位置。
- 所有注入向量。
- 双重转义。
- textContent/innerHTML 实测。
- HTML/SVG 导出路径。

AC-9 全部通过后冻结。

失败时使用 Fallback 或更换 Adapter。

## V-4：Vendor 构建与降级演练

**对应：ADR-002。**

验证：

- SeeCode 固定版本。
- 构建可复现。
- 依赖和许可证可追踪。
- 模拟 SeeCode 不可用。
- Contract 八项断言通过。
- Fallback 按降级矩阵生效。
- 业务链路不阻断。

AC-21 全部通过后冻结。

---

# 5. 影响面与实施责任

| 面 | 内容 |
|---|---|
| 数据库 | 新增四张 viz 表，不改既有业务表；Migration 编号由真实台账确定 |
| 后端 | /v1/visualizations/* 独立分区；envelope 白名单 +1（viz_artifact）；ai_interaction_envelope.rs fail-closed 不变 |
| 查询 | viz_query 聚合读端点，追踪/DAG 两类先行 |
| 前端 | src/components/viz/ lazy chunk |
| 安全 | ArtifactFrame 为全仓唯一 artifact iframe，lint 守卫 |
| 设计系统 | PRISM viz tokens |
| 国际化 | i18n viz.* |
| 测试 | AC-1～21；Contract + Golden；安全专项 E2E；可拔回归 |
| 部署 | VIZ_ENABLED；独立产物 Origin；Node Runtime 的阶段归属待明确 |
| 存储 | PG 保存 Artifact/Render Output；容量和清理规则在 V0-3 确认 |

## 5.1 责任人要求

| 责任领域 | 必须承担的责任 |
|---|---|
| Architecture Owner | ADR、领域边界和架构一致性 |
| Database/Migration Owner | Migration 编号、台账、迁移与回滚 |
| Domain Owners | Relation Registry 词表会签 |
| Backend Owner | Spec、Artifact、权限投影、查询与状态机 |
| Frontend Owner | Viewer、Adapter、ArtifactFrame |
| Security Owner | Origin、CSP、iframe、postMessage、注入测试 |
| QA Owner | AC-1～21 阶段归属、Contract、Golden、E2E |

具体人员姓名在实施任务分配时登记，不在本 ADR 中虚构。

---

# 6. 风险与未决项

| 编号 | 风险 | 级别 | 关闭条件 |
|---|---|---|---|
| R-01 | Migration 编号可能冲突 | P0 阻断 | 核对真实文件和 ledger |
| R-02 | AC 阶段归属不明确 | P0 阻断 | AC-1～21 逐条分阶段 |
| R-03 | PNG 生成路径未闭合 | P0 阻断 | 技术验证或调整能力承诺 |
| R-04 | Artifact/Approval 状态混合 | P0 设计闭合 | 分离状态机 |
| R-05 | PG 产物容量治理缺失 | P0 设计闭合 | 大小、保留、清理规则 |
| R-06 | nature 粒度不明确 | P0 设计闭合 | 明确 Artifact/Node/Edge |
| R-07 | current 语义不充分 | P0 设计闭合 | 明确版本与读取边界 |
| R-08 | Origin/Cookie 隔离细节不足 | P0 安全设计 | 明确凭证隔离并实测 |
| R-09 | Registry 版本治理不足 | P0 设计闭合 | 完成版本和废弃规则 |
| R-10 | VIZ_ENABLED 关闭范围不足 | P0 验收 | 后台任务和业务回归 |

---

# 7. 本轮复审裁决记录

| # | 复审事项 | 裁决 |
|---|---|---|
| 1 | ADR-001/003/004/005/006/007/009/011 | 确认原架构方向，按 V1.0.1 补充后冻结实施契约 |
| 2 | ADR-002/008/010 | Conditional 保留，按 V-1～V-4 验证 |
| 3 | Relation Registry Owner 会签 | V0-1 内完成 |
| 4 | Migration 0020 | 不预先确认，按真实台账分配 |
| 5 | G-3 二跳 + derived_trace | 确认，不新增 task_design |
| 6 | DV-10 GIF/MP4 | 确认 Deferred |
| 7 | AC-1～21 | 确认为完整验收基线，按 P0/P1 分阶段 |
| 8 | AC-12/16 | 确认为不可妥协原则的可测表达 |

---

# 8. P0 启动门槛

正式进入 P0 完整实施前，必须满足：

- [ ] R-01 Migration 编号核查完成。
- [ ] R-01 Migration Owner 明确。
- [ ] R-02 AC-1～21 阶段归属完成。
- [ ] R-03 PNG 路径有明确实施方案或降级决策。
- [ ] Artifact/Approval 状态机设计完成。
- [ ] Artifact 存储容量规则完成。
- [ ] nature 粒度设计完成。
- [ ] consistency 语义完成。
- [ ] Origin/Cookie 隔离设计完成。
- [ ] Relation Registry Owner 会签安排完成。

允许在上述门槛完成之前开展不涉及这些阻断项的设计准备工作，但不得将尚未闭合的内容标记为已经验收。

---

# 9. 最终架构结论

本轮复审结论：

**AI-PM Visualization Engine V1.0.1：附条件通过。**

架构方向保持冻结：

1. Visualization 是共享基础设施，不是新的业务事实 Domain。
2. VizSpec 是唯一图形数据 Contract。
3. Visualization 不拥有业务事实权威。
4. 业务领域零写路径。
5. 二跳推导不创造业务关系。
6. Artifact 不等于正式知识。
7. 权限必须在业务查询与图形投影双层执行。
8. 不可信 HTML 必须隔离。
9. P0 优先验证基础 Contract、安全和 Fallback。
10. Live Graph 放在 P1。

本 ADR 的最终实施原则：

> **先证明图形不会改变业务事实、不会泄露无权信息、不会破坏现有业务链路，再逐步增加图形表达能力。**

本次修订不改变原始 DV-1～DV-14 的架构裁决，不重新开启已经通过的核心选型。

待 P0 启动门槛闭合后，按 D7 路线图进入实施。

---

**文档状态：V1.0.1 修订稿，附条件通过。**

---

## 附录 A：V1.0.2 待签收修订（2026-11-12 产品经理安全审查）

依据 [15 号 V0-6 安全方向审查](ai-pm-viz-15-v0-6-security-review-v1.md)（**放弃 iframe → 产物静态化 + 零脚本呈现**），以下修订待 Architecture/Security Owner 签收后并入 V1.0.2：

| 编号 | 修订对象 | 修订要点 |
|---|---|---|
| M-1 | **ADR-VIZ-008** | 标题与结论改为「产物静态化与零脚本呈现」：删除五层 iframe 隔离（独立 Origin / sandbox / CSP / 零凭证 / postMessage），改为三层（静态化 / 零外网 / 自绘交互）；删除 postMessage 协议章节；新增「重新引入 HTML 产物须重走 ADR-008 全套」条款 |
| M-2 | **ADR-VIZ-002** | SeeCode 定位：浏览器内产物渲染器 → **服务端出图器（SVG，纯 Node）**；删除"SeeCode 独立服务器 + iframe"选项；Fallback Adapter 保留 |
| M-3 | **ADR-VIZ-007** | SVG = 服务端纯 Node 出图（V1 承诺）；HTML 不向浏览器投递（仅可选下载且须静态化检查）；PNG/GIF/MP4 维持 Deferred |
| M-4 | **ADR-VIZ-009 / D6** | 交互归属：产物通道静态；交互通道（G6）承担 hover/focus/route/lens |
| M-5 | **AC-6/7/18** | 重写为 AC-6′ / AC-7′ / AC-18′（见 15 号 §6）；AC-8 不变 |
| M-6 | **V-2 / V-3** | V-2（沙箱 CSP 实测）**取消**；V-3 转为 SVG 静态化断言 + 注入向量用例；V-4 扩展为"SVG 出图链可复现 + 离线" |
| M-7 | **AC-11** | 产物通道不再有 iframe 开销；新增 SVG 体积与前端渲染耗时指标 |
| M-8 | **D2 / D4 / D6 / D7** | 同步删除 iframe / 独立 Origin / postMessage 描述 |

**实测依据（2026-11-12 本地实跑 SeeCode v0.6.3）**：SVG 导出纯 Node 无浏览器；产物 `<script>`=0、`foreignObject`=0、真实外链=0、字体 base64 内嵌；注入 payload 全部正确转义（`esc()` 转义 `& < > "`，未转义单引号 → 补测试用例）。→ **ADR-VIZ-010 硬门槛实测通过，可解除**。