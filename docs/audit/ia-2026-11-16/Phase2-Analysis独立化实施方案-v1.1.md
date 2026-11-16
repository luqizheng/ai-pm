# PM-AI Phase 2：Analysis 独立化实施方案 V1.1

**文档状态：** 实施方案草案，已并入 2026-11-16 评审修订（REV-1~6）与实施控制点（CTRL-1~3），待冻结
**所属项目：** PM-AI / smark-pm
**阶段：** Phase 2
**编制日期：** 2026-11-16（V1.1 同日修订）
**前置阶段：** Phase 0 源码核验、Phase 1 全局导航改造
**核心范围：** Analysis 独立入口、报告列表、报告详情、报告审阅、人工审核中心联动、历史深链兼容

---

# 0. V1.1 修订记录

| 编号 | 类型 | 内容 | 落点 |
|---|---|---|---|
| REV-1 | P1 | 补默认页签与 D-4 综合提案落点边界 | §3.3 / §7 P2-06 / §10.3 E2E-05 / §11 AC-08 |
| REV-2 | P1 | 区分现状存储（正文借住 kb）与 Q2 目标态（Git 权威） | §1.2 / §5.2 / §7 P2-03 |
| REV-3 | P1 | 审阅语义实名审阅流 v2（暂记/改期/close + 409 report_open） | §5.3 / §7 P2-04 / §10.3 E2E-02 |
| REV-4 | P1 | 质量基线刷新为 Phase 1 结算值 | §10.1 |
| REV-5 | P2 | 深链影响面补仓库设置页「分析报告」链接 | §3.3 / §6.2 / §7 P2-06 |
| REV-6 | P2 | 实名 AnalysisReportsPanel.vue + analysis_run 双写入者分叉提示 | §6.2 |
| CTRL-1 | 控制点 | 审阅归属核验前置门：先追链再迁移 | §5.4 / §7 P2-04 |
| CTRL-2 | 控制点 | 列表与详情不共接口，Revision 读路径一致性 | §5.2 / §7 P2-03 / §10.3 E2E-09 / §11 AC-12 |
| CTRL-3 | 控制点 | P2-07 收紧为前置核验门，AC-09 双通过态 | §6.3 / §7 P2-07 / §11 AC-09 |
| VERIFY | 抽验 | 2026-11-16 四项断言源码抽验通过 | §14 |

## 0.1 抽验结论（VERIFY，2026-11-16）

| 方案断言 | 抽验结果 |
|---|---|
| `/project/analysis` 占位已存在 | MATCH：`ai-frontend/src/router/navigation.ts:296`，router/navigation 测试已引用 |
| ReviewCenterPage.vue:549 为 `?report=` 深链生产方 | MATCH：生成 `/project/knowledge?project_id=…&report=…` |
| `?report=` 深链面 | 全仓仅 4 文件：ReviewCenterPage.vue（生产）、KnowledgeView.vue:190（消费）、AnalysisReportsPanel.vue、两组测试 |
| Knowledge 三页签结构 | MATCH：`reports\|kb\|pc`；**reports 为默认页签** |

---

# 1. 项目背景

## 1.1 当前问题

根据 Phase 0《前端导航改造源码核验报告 V1.0》，当前 Analysis Reports 仍然位于：

`/project/knowledge`

具体表现为 Knowledge 页面中的 Analysis Reports Tab（2026-11-16 抽验：`KnowledgeView.vue` 三页签 `reports | kb | pc`，`reports` 为默认页签）。

人工审核中心也收录了分析报告待审阅事项，存在两个不同页面入口：

1. Knowledge 页面中的 Analysis Reports。
2. Review Center 中的分析报告待审阅事项。

当前结构存在以下问题：

- Analysis 报告的领域归属与页面入口不一致。
- Analysis 与 KB 的页面职责混合。
- 报告审阅操作与其他领域审核事项的聚合入口容易混淆。
- 全局搜索仍可能将报告结果导向 `/project/knowledge`。
- 历史 `?report=` 深链依赖旧页面。
- 后续 Analysis 领域独立演进时，前端结构不利于扩展。

## 1.2 已确认的架构依据

Phase 2 必须遵循以下已确认架构决策：

| 编号 | 已确认决策 | 对 Phase 2 的约束 |
|---|---|---|
| P-03 | Analysis 拥有报告审阅页面，审核中心只聚合并跳转 | 报告审阅操作归 Analysis |
| Q1 | Run、Intake Task、Report 不可互相替代 | 前端不能混淆执行身份与报告身份 |
| Q2 | 报告正文 Git 权威，DB 保存 Revision 身份、状态、Git 引用（**目标态**；现状见下方警示） | 前端必须区分正文与元数据；按现有 API 读取，不得假设 Git 直读通道 |
| Q5 | Finding 保持领域独立，通过公共 Finding Contract 对外表达 | 不能强行统一所有 Finding 生命周期 |
| Q12 | 零生产数据时先做 E2E，不先重构 Schema | Phase 2 不借页面迁移修改数据库模型 |

**现状态警示（REV-2）：** DB 审计实证：报告正文当前**借住 kb 存储链路**读取；四域注册表中 Analysis Report 内容权威误标 Git、待修正。"正文 Git 权威"是目标态而非现约束。Phase 2 前端一律按现有 API 实现；权威归位属后端演进项，登记为独立缺口，不属于 Phase 2。

## 1.3 Phase 2 核心目标

建立真正独立的 Analysis 前端入口，使 Analysis 成为独立领域页面，能够独立呈现报告、版本、分析结果和审阅操作。

同时确保：

- KB 继续维护自己的知识文档和知识审核。
- Analysis 继续维护自己的报告及报告审阅。
- 人工审核中心只聚合待处理事项。
- Project Cognition 和 Product Cognition 不被混入 Analysis。
- 旧入口和历史深链继续有效。

---

# 2. Phase 2 范围

## 2.1 本阶段必须完成

| 编号 | 工作内容 | 优先级 |
|---|---|---|
| P2-01 | Analysis 独立页面及路由 | P0 |
| P2-02 | Analysis 报告列表 | P0 |
| P2-03 | Analysis 报告详情 | P0 |
| P2-04 | Analysis 报告审阅流程迁移 | P0 |
| P2-05 | 人工审核中心联动 | P0 |
| P2-06 | 旧报告深链兼容 | P0 |
| P2-07 | 全局搜索报告结果跳转（前置核验门控，见 §7 P2-07） | P1 |
| P2-08 | 权限、项目隔离与回归测试 | P0 |

## 2.2 本阶段不做

以下工作明确排除在 Phase 2 之外：

1. 不重构 Analysis Run 后端统一身份模型。
2. 不新增 Analysis Run 与 Report 的数据库外键。
3. 不修改 Report / Report Revision 的权威模型。
4. 不将 Report Revision 改成 KB Document Revision。
5. 不合并不同领域的 Finding。
6. 不将 Analysis Report 自动转换成 KB Memory。
7. 不修改 KB 的正式知识审核流程。
8. 不重构 Product Cognition。
9. 不重构 Project Cognition。
10. 不修改四域的数据库 Schema。
11. 不新建通用审批领域。
12. 不在前端实现第二套报告审批逻辑。
13. 不在前端实现报告正文权威归位（Git 通道假设），权威归位另立后端缺口（REV-2）。

如果源码核验发现现有 API 无法满足页面需求，应登记为独立后端契约缺口，不得未经裁决自行改变领域模型。

---

# 3. 目标导航和路由结构

## 3.1 导航位置

Analysis 位于 Current Project / 项目工作区下，与 KB、Product Cognition、Project Cognition 同级。

```text
Current Project
├── 项目总控台
└── 项目工作区
    ├── 需求
    ├── 技术设计
    ├── 任务管理
    ├── AI 执行中心
    ├── 代码发现
    ├── 人工审核中心
    ├── Analysis
    ├── KB
    ├── Product Cognition
    ├── Project Cognition
    └── 项目设置
```

Analysis 是一个一级导航入口，不在全局左侧栏增加报告列表、报告详情、报告审阅三个独立菜单。

## 3.2 建议路由

以下为目标路由建议，必须在 Phase 2 源码核验后与现有路由对照，避免覆盖已有业务路径。

| 路由 | 用途 | 页面类型 |
|---|---|---|
| `/project/analysis` | Analysis 首页 | 报告列表 |
| `/project/analysis/reports/:reportId` | 报告详情 | 详情页 |
| `/project/analysis/reports/:reportId/review` | 报告审阅 | 审阅页或详情页审阅模式 |

说明：

- 如果现有路由已经有相同语义，应优先复用。
- 如果详情与审阅可以通过 Tab 或状态面板组织，不必强制增加独立页面。
- 路由结构不应暗示尚未实现的业务能力。
- 所有路由必须具备当前项目上下文。
- 不得绕过现有 `shell.currentProjectId` 和项目切换守卫。
- `/project/analysis` 导航占位已存在（navigation.ts:296，Phase 1 C3 落地），本阶段将其从占位升级为真实页面。

## 3.3 路由迁移兼容

历史入口：

`/project/knowledge?report=:reportId`

目标行为：

1. 识别 `report` 参数。
2. 保留报告 ID。
3. 导航至 Analysis 对应报告。
4. 保留必要的审阅上下文。
5. 不允许丢失当前项目上下文。
6. 不允许将 `report` 当作 `doc` 或 `object`。

迁移策略：

- `/project/knowledge` 不带 `report` 参数时继续进入 KB。
- `/project/knowledge?report=xxx` 时跳转 Analysis。
- 已有 `/project/knowledge?doc=xxx` 继续进入 KB 对应文档。
- 如果同一 URL 同时存在 `report` 和 `doc`，不得自行猜测优先级；应根据现有实际调用和业务契约确定，并登记核验结论。
- 旧深链不能因为新路由上线而失效。

**深链影响面清单（2026-11-16 抽验，REV-5）：**

- `?report=` 生产方仅一处：`ReviewCenterPage.vue:549`。
- `?report=` 消费方：`KnowledgeView.vue:190`（切换到 reports 页签并定位）。
- 另有 `SettingsProjectRepositoriesView.vue` 仓库设置行内「分析报告」链接导向 reports 页签——非 `?report=` 参数，但同属本次迁移影响面，须同步改导向。

**默认页签影响（REV-1）：** `reports` 现为 Knowledge 页默认页签。报告页签迁出后，Knowledge 页无参数默认页签必须重新裁决（建议 `kb`），不得保留指向已迁出内容的默认态；该变更纳入回归测试。

---

# 4. 四域职责与数据边界

## 4.1 Analysis

Analysis 是报告及其分析结果的唯一领域归属方。

负责：

- Analysis Report。
- Report Revision。
- Analysis Findings。
- Conclusions。
- Recommendations。
- 报告来源和分析执行关联。
- 报告审阅及领域内的审阅状态。
- 报告版本和历史记录。

页面不得自行推导或伪造报告的审阅状态。

## 4.2 KB

KB 负责：

- Knowledge Source。
- Knowledge Document。
- Knowledge Revision。
- Memory。
- Memory Proposal。
- Knowledge Review。
- KB 自己的确认、发布和生命周期。

Analysis 报告被审阅或确认，不代表其自动成为 KB 正式知识。

如果未来需要把报告内容纳入 KB，应通过 KB 领域自己的正式提案和确认流程。

## 4.3 Project Cognition

Project Cognition 维护项目技术和项目层面的正式认知。

Analysis Findings 不应直接等同于 Project Cognition Item。

若后续需要将分析发现转化为项目认知，应通过 Project Cognition 自己的提案、确认和生命周期。

## 4.4 Product Cognition

Product Cognition 维护正式产品认知。

Analysis 页面不得直接创建或修改 Product Cognition 正式认知对象。

## 4.5 人工审核中心

人工审核中心负责：

- 聚合各领域待处理事项。
- 显示待处理类型、来源领域、优先级和状态。
- 提供跳转入口。
- 显示必要的待处理数量和简要上下文。

人工审核中心不负责：

- 报告正文编辑。
- Report Revision 管理。
- Analysis 报告审阅状态写入。
- 代替 Analysis 执行报告审阅操作。

**核心规则：审核中心负责找到事项，领域页面负责处理事项。**

**单操作面规则（CTRL-1）：** P2-04 迁移完成后，Review Center 对报告事项只保留「进入报告审阅」跳转，删除就地审阅决定入口（决定暂记、改期、close 等操作只存在于 Analysis 审阅界面一处）。

---

# 5. 页面设计方案

## 5.1 Analysis 首页 / 报告列表

路由：

`/project/analysis`

页面目标：

提供当前项目所有 Analysis Report 的统一入口。

建议布局：

```text
┌──────────────────────────────────────────────────────────────┐
│ Analysis                                                     │
│ 项目分析报告、分析结果与审阅                                  │
├──────────────────────────────────────────────────────────────┤
│ [全部报告] [待审阅] [已审阅] [历史报告]                       │
├──────────────────────────────────────────────────────────────┤
│ 搜索报告                       筛选：来源 / 状态 / 时间       │
├──────────────────────────────────────────────────────────────┤
│ 报告名称          来源       版本      状态       更新时间    │
│ Repository ...   Discovery  Rev 3     待审阅     ...          │
│ Architecture...  Analysis   Rev 2     已审阅     ...          │
│ ...                                                          │
└──────────────────────────────────────────────────────────────┘
```

上述状态和筛选字段是 UI 候选项，必须以现有 API 和领域状态契约为准。

**来源字段警示（REV-6）：** DB 审计已知 analysis_run 存在双写入者分叉（P0）。列表「来源」列如何呈现执行身份，必须以 §6 源码核验结果为准，不得凭名称猜测或自行归并。

要求：

- 默认限定当前项目。
- 列表数据来自 Analysis 领域 API。
- 不从 KB Document 列表推导 Analysis Report。
- 报告 ID 是详情页稳定定位依据。
- 支持分页或现有列表机制。
- 加载、空状态、错误状态、无权限状态均需设计。
- 不允许通过硬编码模拟正式报告数据。

### 列表操作

建议支持：

- 查看报告。
- 进入待审阅报告。
- 查看历史版本（仅在 API 支持时）。
- 根据已有能力进行搜索和筛选。

不得在本阶段引入未有业务契约支撑的删除、重新分析、发布等操作。

## 5.2 Analysis 报告详情

路由：

`/project/analysis/reports/:reportId`

建议页面布局：

```text
┌──────────────────────────────────────────────────────────────┐
│ Analysis / Reports / Report Title                            │
├──────────────────────────────────────────────────────────────┤
│ 报告名称                                    [进入审阅]       │
│ 当前版本 / 状态 / 创建时间 / 更新时间                        │
├──────────────────────────────────────────────────────────────┤
│ [报告正文] [Findings] [Conclusions] [Recommendations]        │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                    报告内容                                  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ 来源信息 / 关联执行信息 / Revision 历史                       │
└──────────────────────────────────────────────────────────────┘
```

建议内容区：

**A. 报告正文**

- 展示当前有效 Report Revision 对应的正文。
- 正文读取遵循 Analysis 当前权威存储机制。
- 不允许用 DB 摘要字段替代完整正文。
- 不允许前端直接拼接 Git URL 来假装读取成功。
- **（REV-2）** 现状正文经借住 kb 的存储链路由详情 API 返回（见 §1.2 现状态警示）；前端不得假设 Git 直读通道，也不得自行拼装详情 API 以外的读取端点。
- **（CTRL-2）** 详情加载以详情 API 返回的 Revision 身份为准回填头部版本信息，不得沿用列表快照的版本/状态字段。

**B. Findings**

- 显示分析发现。
- 展示严重度、证据和来源。
- 通过明确关联引用定位其他领域对象。
- 保留领域 Finding 的原始语义。

**C. Conclusions**

- 展示分析结论。
- 显示结论与 Findings 的关联（API 有提供时）。

**D. Recommendations**

- 展示建议。
- 区分事实、结论与建议。
- 不将建议自动转换成任务或正式认知。

**E. Revision 历史**

- 展示 Report Revision 的身份和版本。
- 区分重新分析和修订报告。
- 重新分析可能产生新 Run 和新 Report / 分析轮次。
- 修订报告则是同一 Report 身份下的新 Revision。
- 具体显示形式必须遵循现有领域数据契约。

## 5.3 报告审阅页面

路由建议：

`/project/analysis/reports/:reportId/review`

页面目标：

由 Analysis 自己承载报告审阅流程。

**现状与迁移性质（REV-3）：** 报告审阅已随 S01 D-5/D-6 在人工审核中心 ReportItem 落地（审阅流 v2，commit 91003f6b）：决定暂记、可改期、close 批量发布 approved；D-5 报告门对 analyze/import 链返回 `409 report_open`。**P2-04 是操作面搬家，不是补缺**——v2 全语义必须原样保留并随迁，不得退化为一次性 approve/reject。

建议布局：

```text
┌──────────────────────────────────────────────────────────────┐
│ Analysis / Report Review                                     │
├──────────────────────────────────────────────────────────────┤
│ 报告基本信息                         当前审阅状态             │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│ 报告正文                         审阅上下文                   │
│                                  Findings                    │
│                                  Evidence                    │
│                                  Review History               │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│ 审阅意见                                                     │
│ [意见输入区]                                                 │
│                                                              │
│                       [提交审阅决定]                         │
└──────────────────────────────────────────────────────────────┘
```

重要要求：

1. 必须复用现有 Analysis 审阅 API。
2. 必须复用现有领域状态机。
3. 只有领域契约允许的身份才能执行审阅决定。
4. 不能由前端直接写 DB 或修改领域状态。
5. AI 可以提供审阅建议，但不得替代受保护的人工确认。
6. 审阅意见和操作历史应按现有审计契约保存。
7. 页面需要处理并发修改、重复提交、权限失效和过期 Revision。
8. **（REV-3）** 完整保留审阅流 v2 语义：决定暂记、改期、close 批量发布；`409 report_open` 报告门必须在 Analysis 审阅界面有明确呈现。
9. **（CTRL-1）** 实施前先完成 §7 P2-04 的审阅归属核验前置门，确认后再迁移组件。

**不得将报告审阅简化为通用的 approve/reject 两个按钮。** 具体动作名称、条件和状态迁移必须从现有 Analysis 业务契约核验。

## 5.4 人工审核中心联动

Review Center 继续展示报告待审阅事项，但不展示第二份完整报告正文编辑器。

建议事项卡：

```text
┌──────────────────────────────────────────────────────────────┐
│ Analysis Report Review                                      │
│                                                              │
│ 报告名称                                                     │
│ 来源：Analysis                                               │
│ 优先级：P1                                                   │
│ 状态：待人工审阅                                             │
│                                                              │
│ [进入报告审阅]                                               │
└──────────────────────────────────────────────────────────────┘
```

行为：

- 点击后跳转 Analysis 对应报告审阅页面。
- 带上 Report ID。
- 如果需要特定 Revision，携带现有契约支持的 Revision ID。
- 跳转失败时显示明确错误。
- 已完成事项根据现有规则处理，不自行改变领域状态。
- Review Center 的聚合数据必须与 Analysis 领域真实数据一致。
- **（CTRL-1 / P-03 单操作面）** 迁移完成后，ReviewCenterPage.vue:549 的就地审阅链接改为跳转新审阅页；Review Center 内不得残留任何报告审阅决定操作入口。

---

# 6. 源码核验阶段

在正式实施前，先建立 Phase 2 源码核验任务。

## 6.1 核验目标

确认现有实现中：

- 哪些 Analysis 页面组件可以复用。
- 哪些组件需要迁移。
- 哪些 API 已具备。
- 哪些数据契约尚未提供。
- 哪些深链生产方和消费方必须同步修改。
- 哪些搜索入口需要修改。
- 哪些权限和项目隔离规则必须保持。

## 6.2 必查源码范围

以下是根据 Phase 0 结果确定的核验起点，不代表这些文件当前内容已经重新核验。

| 范围 | 核验对象 | 重点 |
|---|---|---|
| Knowledge 页面 | KnowledgeView.vue 及其子组件 | 三页签 `reports\|kb\|pc` 结构、reports 默认页签行为、`?report=`/`?doc=`/`?object=` 消费逻辑 |
| 报告面板 | `src/views/project/knowledge/AnalysisReportsPanel.vue`（REV-6） | 列表+详情一体组件的复用/拆分边界，迁移的最小改动路径 |
| 报告列表 | Analysis Reports 相关组件 | 列表 API、筛选、分页 |
| 报告详情 | Report Detail 相关组件 | 正文（借住 kb 存储链路，REV-2）、版本、关联数据 |
| 报告审阅 | Report Review 相关组件 | 审阅 API、状态机、操作权限；审阅流 v2 暂记/改期/close 链路（REV-3） |
| 审核中心 | ReviewCenterPage.vue | 报告事项聚合和跳转（:549 深链生产方，抽验仍准确） |
| 仓库设置 | `src/views/project/settings/SettingsProjectRepositoriesView.vue`（REV-5） | 行内「分析报告」链接导向 reports 页签的跳转逻辑 |
| 深链 | `?report=` 生产方与消费方 | 参数生成、解析、兼容（抽验：全仓 4 文件，见 §3.3） |
| 搜索 | 全局搜索组件和结果路由 | 当前 `/project/knowledge` 硬编码 |
| 路由 | router/index.ts | 目标路径和 redirect |
| 导航 | navigation.ts | Analysis 导航项（:296 占位已存在） |
| Shell | SideNav.vue / WorkspaceShell.vue | 导航呈现和项目上下文 |
| API | Analysis 相关 API client（`src/api/analysis-reports.ts` 等） | 端点、响应结构、错误处理 |
| 类型 | Analysis 相关 TypeScript 类型 | Run、Report、Revision、Finding |
| 权限 | 路由守卫及 API 权限处理 | 项目可见性、操作权限 |
| 测试 | 相关 Vitest / E2E | 现有行为基线 |

特别核验：

`ReviewCenterPage.vue:549`

该位置在 Phase 0 报告中被识别为 `?report=` 深链生产方，2026-11-16 抽验确认仍准确（生成 `/project/knowledge?project_id=…&report=…`）。实施时仍必须重新定位，不能假设行号在实施日仍然有效。

## 6.3 核验输出

必须形成：

《Phase 2 Analysis 独立化源码核验报告 V1.0》

至少包含：

1. 实际源码路径和符号。
2. 当前页面组件树。
3. 组件复用清单。
4. API 清单。
5. 数据类型和字段清单。
6. 深链生产方、消费方清单（含 SettingsProjectRepositoriesView.vue 链接，REV-5）。
7. 搜索跳转清单。
8. 权限和项目隔离核验。
9. 测试基线。
10. 已知缺口。
11. 建议实施顺序。
12. 不允许在 Phase 2 擅自修改的后端边界。
13. 审阅归属追链记录：审阅按钮 → API endpoint → 后端 handler/Service → 权限检查 → 状态机（CTRL-1）。

每个结论必须注明：

- MATCH：源码与预期一致。
- MISMATCH：源码与预期不一致。
- UNKNOWN：缺少证据，无法判断。
- NOT_IMPLEMENTED：当前没有实现。

不得将 UNKNOWN 写成 NOT_IMPLEMENTED，也不得把计划功能写成已实现。

**（CTRL-3）** 搜索领域识别的结论判定：只有检索结果能稳定携带领域标识（`source_domain=analysis` 或等价字段）与 Report ID 才可写 MATCH；仅凭标题/内容"看起来像报告"只能写 UNKNOWN 或 NOT_IMPLEMENTED，不得写 MATCH。

---

# 7. 实施任务拆分

源码核验通过后，按以下任务卡实施。

## P2-01：Analysis 页面框架与独立路由

**目标：** 建立 Analysis 独立入口和页面框架。

工作内容：

- 注册 `/project/analysis`（占位已存在，升级为真实页面）。
- 接入 Phase 1 已有 Project Navigation。
- 建立 Analysis 页面框架。
- 实现项目上下文和导航选中状态。
- 保留现有 Shell 和 Grid。
- 设计加载、空、错误和无权限状态。
- 确保无项目上下文时不展示其他项目数据。

禁止：

- 修改全局 Grid。
- 新增第二条侧栏。
- 修改 Project Switcher。
- 修改项目切换守卫。
- 修改四域数据库 Schema。

交付：

- Analysis 首页。
- 路由配置。
- 导航测试。
- 页面截图。
- 变更清单。

## P2-02：Analysis 报告列表

**目标：** 将现有 Analysis Report 列表迁入 Analysis 页面。

工作内容：

- 接入 Analysis 报告列表 API。
- 显示报告名称、来源、版本、状态和时间（以实际 API 字段为准）。
- 支持现有 API 能力范围内的搜索、筛选和分页。
- 点击报告进入详情页。
- 保持当前项目过滤。
- 保持错误和权限处理。

禁止：

- 从 KB Document 表构建报告列表。
- 使用 Mock 数据冒充正式业务数据。
- 自行定义报告生命周期状态。
- 擅自新增后端字段。
- 凭猜测归并「来源」列的执行身份（analysis_run 双写入者分叉未收口前，以核验结论为准，REV-6）。

交付：

- 报告列表。
- 列表交互。
- API 和类型变更清单。
- 单元测试。

## P2-03：Analysis 报告详情

**目标：** 独立呈现 Report、Revision 和分析结果。

工作内容：

- 接入报告详情 API。
- 显示正文。
- 显示 Report Revision 信息。
- 显示 Findings。
- 显示 Conclusions。
- 显示 Recommendations。
- 显示来源和执行关联信息（现有契约支持时）。
- 显示版本历史（现有契约支持时）。
- **（CTRL-2）** 列表的版本/状态字段与详情正文必须同源同一 Revision；详情页版本头部以详情 API 返回值为准回填，不得信任列表快照。
- **（REV-2）** 正文读取按现有详情 API（借住 kb 存储链路）实现；不得假设 Git 直读通道或拼装其他端点。

禁止：

- 将 Report Revision 映射成 KB Revision。
- 将 Run、Intake Task 和 Report 混为一个实体。
- 直接将 Finding 转成 Project Cognition Item。
- 直接将报告结论转为 KB Memory。
- **（CTRL-2）** 列表与详情共用同一份摘要数据导致"列表显示最新版本、详情读取旧正文"的分叉。

交付：

- 报告详情页。
- 版本展示。
- 关联信息展示。
- 相关测试（含 E2E-09 读路径一致性）。

## P2-04：Analysis 报告审阅迁移

**目标：** 使报告审阅由 Analysis 页面独立负责。

### 前置核验门（CTRL-1，未通过不得迁移）

追踪审阅操作真实归属，不能只看当前组件放在哪里：

- 追踪链：审阅按钮 → API endpoint → 后端 handler/Service → 权限检查 → 状态机迁移。
- 判定审阅语义的后端归属方（Analysis 域服务，还是 review/interaction 域服务）。
- **通过态 A：** 审阅写入由 Analysis 领域服务负责 → 正常迁移。
- **通过态 B：** 审阅写入实际归 review/interaction 域服务所有 → 允许迁移 UI 操作面（页面归 Analysis 符合 P-03），但必须登记「审阅后端域归属与 P-03 页面归属不一致」为后端缺口，不得顺手改服务归属。
- 追链记录写入 §6.3 核验输出第 13 项。

工作内容：

- 迁移现有报告审阅组件。
- 接入现有 Analysis 审阅 API。
- 保持现有状态机。
- 保持人工身份和权限要求。
- 保留审阅意见和操作历史。
- 处理重复提交和 Revision 变化。
- 审阅完成后更新页面展示。
- **（REV-3）** 完整随迁审阅流 v2 语义：决定暂记、可改期、close 批量发布 approved；呈现 `409 report_open` 报告门。
- **（P-03 单操作面）** 迁移完成后移除 Review Center 的报告审阅决定入口，仅保留跳转。

禁止：

- 在 Review Center 中重新实现审阅操作。
- 新建通用审批实体。
- 将 AI 建议当成人工决定。
- 绕过领域服务直接更新状态。
- 擅自改变历史状态。
- **（REV-3）** 将审阅退化为一次性 approve/reject 提交。
- **（CTRL-1）** 前置核验门未通过即开始迁移组件。

交付：

- Analysis 报告审阅页或详情页审阅模式。
- 审阅归属追链记录（前置门产物）。
- 审阅操作测试（含暂记/改期/close 全语义）。
- 权限拒绝测试。
- 操作历史验证。
- Review Center 决定入口移除的回归证明（单操作面）。

## P2-05：人工审核中心联动

**目标：** 保持 Review Center 为统一入口，而非重复业务页面。

工作内容：

- 保留 Analysis Report Review 待处理事项。
- 点击后跳转 Analysis。
- 正确传递 Report ID 和必要 Revision 信息。
- 处理报告不存在、项目不匹配、权限不足和状态变化。
- 保持其他领域审核事项不受影响。

禁止：

- 在 Review Center 中编辑报告正文。
- 在 Review Center 中维护 Report Revision。
- 在 Review Center 中直接改变 Analysis 状态。
- 将所有领域审核操作强行统一为一种生命周期。

交付：

- Review Center 跳转改造（:549 指向新审阅页）。
- 导航与深链测试。
- 不同领域审核事项回归测试。

## P2-06：历史深链兼容

**目标：** 保证旧报告链接在 Analysis 独立化后继续有效。

工作内容：

- 识别旧 `?report=` 路由。
- 跳转至新 Analysis 报告详情或审阅页面。
- 保留 Report ID。
- 保留当前项目上下文。
- 与 `?doc=`、`?object=` 参数兼容。
- 保持其他旧路径行为不变。
- **（REV-5）** 同步改导向 SettingsProjectRepositoriesView.vue 行内「分析报告」链接。
- **（REV-1）** 裁决并实现 Knowledge 页迁出后的默认页签（建议 `kb`）；确认 D-4 身份综合提案的展示落点——现待办视图将综合提案挂 analyzed 报告下，报告页签迁出后落点二选一：随报告迁至 Analysis，或保留在知识页待办视图并改挂跳转链接；须核验裁决后实施，不得静默丢失。

重点核验：

- `ReviewCenterPage.vue` 中的报告跳转。
- 所有生成 `?report=` 的链接（抽验：仅 ReviewCenterPage.vue:549 一处）。
- 所有解析 `?report=` 的页面（抽验：仅 KnowledgeView.vue）。
- `SettingsProjectRepositoriesView.vue` 的 reports 页签导向。
- 可能存在的通知、历史记录和外部入口。

交付：

- 深链兼容实现。
- 路由映射表。
- D-4 综合提案落点裁决记录。
- 深链测试。

## P2-07：全局搜索跳转

**目标：** 使搜索结果按领域归属跳转。

Phase 0 已发现全局搜索 target 硬编码 `/project/knowledge`，而检索 API `/v1/retrieval/query` 已具备按 `source_domain` 区分 KB/PC evidence 的基础材料。

### 前置核验门（CTRL-3，未通过整卡转缺口，不进入实施）

核验并回答：

- Analysis 报告是否已出现在搜索结果中。
- 是否存在可用于区分 Report 的来源类型。
- 是否有 Report ID。
- 是否有对应的 Revision 或来源引用。

**门条件：** 检索结果能稳定携带领域标识（`source_domain=analysis` 或等价字段）+ Report ID。

**门未通过的处置：** 整卡转为后端契约缺口登记，不实施任何跳转改造；不得通过标题或内容猜测领域归属。结论词汇按 §6.3：只能 NOT_IMPLEMENTED / UNKNOWN，不得写 MATCH。

目标映射（仅前置门通过后生效）：

| 搜索结果领域 | 目标入口 |
|---|---|
| KB | KB 页面 |
| Analysis | Analysis 报告详情 |
| Project Cognition | Project Cognition |
| Product Cognition | Product Cognition（后续阶段或现有入口） |

## P2-08：权限与回归验收

**目标：** 保证页面迁移没有造成权限、租户或项目数据泄漏。

至少覆盖：

- 项目 A 用户不能读取项目 B 的报告。
- 无权限用户不能执行审阅操作。
- 已失效的授权不能继续操作。
- 不存在的 Report ID 正确返回错误状态。
- 报告 Revision 发生变化时，页面不会提交过期决定。
- 项目切换期间不展示上一个项目的报告数据。
- Review Center 不绕过 Analysis 的权限校验。
- 旧深链不会绕过项目访问控制。

交付：

- 回归测试报告。
- 权限负向测试。
- 项目隔离测试。
- Release Guard 结果。
- Phase 2 验收报告。

---

# 8. 任务执行顺序和依赖

建议严格串行推进，避免多人同时修改 Knowledge 页面、Review Center 和路由。

```text
Phase 2 Source Audit（含 CTRL-1 追链、CTRL-3 搜索前置门）
        │
        ▼
P2-01 Analysis 页面框架
        │
        ▼
P2-02 报告列表
        │
        ▼
P2-03 报告详情
        │
        ▼
P2-04 报告审阅迁移（前置门已过）
        │
        ▼
P2-05 审核中心联动
        │
        ▼
P2-06 历史深链兼容
        │
        ▼
P2-07 全局搜索跳转（前置门通过才实施，否则转缺口）
        │
        ▼
P2-08 权限与回归验收
```

说明：

- P2-01 与 P2-02 可以在源码核验后评估是否并行，但默认串行。
- P2-03 依赖报告详情 API 和数据契约。
- P2-04 依赖审阅 API、权限及状态机核验（CTRL-1 前置门）。
- P2-05 依赖 P2-04 确定的目标页面。
- P2-06 需在目标路由稳定后实施。
- P2-07 依赖检索结果能准确标识 Analysis（CTRL-3 前置门）；门未通过时该卡转缺口登记，不阻塞 P2-08。
- P2-08 是整个阶段的最终门槛。

---

# 9. Commit 与回滚纪律

## 9.1 Commit 规则

每张任务卡：

- 一个任务卡对应一个独立 Commit。
- 不将不相关的修复混入导航迁移。
- 不修改四域数据库 Schema。
- 不修改与任务无关的 API 契约。
- 保留清晰的 Commit Message。
- 每张任务卡独立测试和回滚。

建议 Commit 前缀：

- `feat(analysis): add independent analysis route`
- `feat(analysis): migrate report list`
- `feat(analysis): add report detail`
- `feat(analysis): migrate report review`
- `feat(review-center): route analysis review items`
- `fix(analysis): preserve legacy report deep links`
- `fix(search): route analysis results by domain`
- `test(analysis): add isolation and regression coverage`

具体命名服从仓库现有 Commit 规范。

## 9.2 回滚要求

每个任务卡都必须说明：

- 修改文件清单。
- 新增路由和移除路由。
- 兼容入口。
- 受影响组件。
- 需要回滚的测试。
- 回滚后旧入口是否仍可访问。

特别注意：

- P2-04 和 P2-06 不能只回滚新页面而遗留失效的旧深链。
- **（CTRL-1）** P2-04 回滚时必须同步恢复 Review Center 审阅决定入口——迁移后决定入口只存在于 Analysis 一侧，单侧回滚会造成审阅操作无处可做。

---

# 10. 测试与质量门禁

## 10.1 基线保护

Phase 1 结算基线（REV-4，2026-11-16 刷新）：

| 检查项 | 已记录基线 |
|---|---|
| Prettier | 20 files issues（Phase 1 结算） |
| i18n hardcode | 39 |
| Namespace | 5379 keys，通过（Phase 1 结算） |
| Layout test | `styles/layout.test.ts` 存量红（master 基线；Phase 0 记录 1 failed / 13 passed，开工前复测刷新） |
| SeeCode | 3 项历史 flaky，尚需核实复测情况 |

Phase 2 不得扩大这些存量问题。

Phase 2 新增问题必须单独登记，不能以旧问题为理由忽略新增问题。

## 10.2 必须执行的测试

根据仓库实际测试脚本和 Phase 0 基线，至少执行：

1. Analysis 页面相关 Vitest。
2. Report List 测试。
3. Report Detail 测试。
4. Report Review 测试。
5. Review Center 测试。
6. Router 测试。
7. Shell / Navigation 测试。
8. 项目切换测试。
9. 深链兼容测试。
10. 权限负向测试。
11. Typecheck。
12. i18n hardcode 检查。
13. i18n namespace 检查。
14. Release Guard。
15. KnowledgeView / AnalysisReportsPanel 迁移后测试全绿（REV-6）。

Phase 1 已采用的公共回归命令应继续保留：

```bash
pnpm -s vitest run src/router/ src/components/shell/
pnpm typecheck
```

其他测试命令必须以仓库实际 package scripts 为准，不得编造不存在的命令。

## 10.3 E2E 测试场景

至少覆盖以下场景：

**E2E-01：正常查看报告**

用户进入项目 → Analysis → 报告列表 → 报告详情。

预期：当前项目报告正常加载，正文与版本正确。

**E2E-02：报告审阅（审阅流 v2 语义，REV-3）**

用户进入待审阅报告 → Analysis 报告审阅 → 暂记决定 →（可选）改期 → close 批量发布。

预期：暂记可改期语义保持，`409 report_open` 报告门呈现正确，审计信息正常，列表状态同步，Review Center 决定入口已移除（单操作面）。

**E2E-03：审核中心跳转**

用户进入人工审核中心 → 点击 Analysis 待审阅事项 → 进入对应报告。

预期：Report ID 正确，不重复实现审阅操作。

**E2E-04：历史深链**

用户打开 `/project/knowledge?report=xxx`。

预期：跳转到 Analysis 对应报告。

**E2E-05：KB 文档不受影响（含默认页签，REV-1）**

用户打开 `/project/knowledge?doc=xxx`。

预期：仍进入 KB 对应文档；且无参数进入 Knowledge 页时默认页签为迁移后新默认（不再是已迁出的 reports），页面无空报告页签残留。

**E2E-06：项目隔离**

用户在项目 A 打开项目 B 的 Report ID。

预期：拒绝访问，不返回项目 B 报告内容。

**E2E-07：项目切换**

用户从项目 A 切换到项目 B。

预期：切换期间不泄漏 A 的报告数据；完成后只显示 B 的数据。

**E2E-08：过期 Revision**

用户打开报告后，报告 Revision 发生变化，再提交旧版本审阅决定。

预期：按领域契约拒绝或要求刷新，不得静默覆盖新版本。

**E2E-09：列表/详情 Revision 一致性（新增，CTRL-2）**

多 Revision 报告：列表标 Rev N → 进入详情 → 正文与版本同为 Rev N；触发修订产生 Rev N+1 后复查列表与详情。

预期：列表版本字段与详情正文同源同一 Revision；不出现「列表显示最新版本、详情读取旧正文」。

**E2E-10：综合提案落点（新增，REV-1）**

D-4 身份综合提案挂 analyzed 报告的场景：报告页签迁出后打开提案所在入口。

预期：综合提案仍可达（新落点或改挂链接），不因迁移静默丢失；落点与 P2-06 裁决记录一致。

---

# 11. Phase 2 验收标准

Phase 2 必须同时满足以下条件才能宣布完成。

| 编号 | 验收项 | 通过标准 |
|---|---|---|
| AC-01 | 独立入口 | Analysis 可从项目导航独立进入 |
| AC-02 | 报告列表 | 数据来自 Analysis 领域 |
| AC-03 | 报告详情 | 正文和版本信息正确 |
| AC-04 | 分析结果 | Findings / Conclusions / Recommendations 正确展示 |
| AC-05 | 报告审阅 | 由 Analysis 领域负责，v2 全语义保留（REV-3） |
| AC-06 | 人工审核中心 | 只聚合并跳转 |
| AC-07 | 深链兼容 | 旧 `?report=` 入口有效 |
| AC-08 | KB 隔离 | `?doc=` 和 KB 页面正常；含迁出后默认页签与待办视图（综合提案落点）正常（REV-1） |
| AC-09 | 搜索 | **双通过态（CTRL-3）**：A = 可识别的 Analysis 结果进入 Analysis（前置门通过）；B = 前置门未通过且缺口已登记 |
| AC-10 | 项目隔离 | 无跨项目读取 |
| AC-11 | 操作权限 | 无权限用户不能执行审阅 |
| AC-12 | 版本一致性 | 不发生旧 Revision 静默覆盖；列表版本字段与详情正文同源同一 Revision（读路径一致性，CTRL-2） |
| AC-13 | 回归 | 导航、Shell、项目切换正常 |
| AC-14 | 质量门禁 | Typecheck、相关测试、Release Guard 通过 |
| AC-15 | 单操作面 | 迁移后 Review Center 无报告审阅决定入口，审阅操作仅存在于 Analysis（CTRL-1） |

如果某项依赖尚未实现的后端 API，应标记为 BLOCKED，并记录缺口，不得把前端静态页面完成视为该项通过。AC-09 的 B 通过态为唯一例外（缺口登记即结项）。

---

# 12. 风险和处理策略

| 风险 | 影响 | 处理方式 |
|---|---|---|
| 报告与 KB 页面高度耦合 | 迁移导致 KB 回归 | 先做组件依赖核验，再拆分 |
| 审阅 API 归属不清 | 重复审批逻辑 | CTRL-1 追链前置门；Analysis 保留操作面，后端归属差异登记缺口 |
| 旧深链遗漏 | 通知和历史入口失效 | 建立生产方/消费方清单（含仓库设置页链接，REV-5） |
| 搜索领域信息不足 | 错误跳转 | CTRL-3 前置门未通过即转缺口登记，不猜测 |
| Report Revision 语义混淆 | 版本错误 | 明确区分重新分析与修订；CTRL-2 列表/详情同源 |
| 列表最新、正文旧 | 版本呈现分叉 | CTRL-2 详情 API 回填 + E2E-09 断言 |
| 默认页签/综合提案落点未定 | 知识页回归、提案丢失 | REV-1 裁决后实施 + E2E-10 |
| 审阅 v2 语义迁移走样 | 审批流退化 | REV-3 全语义随迁 + E2E-02 |
| 项目切换数据残留 | 跨项目信息泄漏 | 复用现有项目切换守卫 |
| Review Center 与 Analysis 双重状态 | 数据不一致 | 审核中心只读聚合并跳转；AC-15 单操作面 |
| 前端越权提交 | 审阅权限绕过 | 后端领域权限校验，负向测试 |
| 单侧回滚 | 审阅操作无处可做 | §9.2 P2-04 回滚须同步恢复 Review Center 入口 |

---

# 13. Phase 2 交付物

Phase 2 最终需要交付：

1. 《Phase 2 Analysis 独立化源码核验报告 V1.0》（含 CTRL-1 追链记录）。
2. Analysis 独立页面。
3. Analysis 报告列表。
4. Analysis 报告详情。
5. Analysis 报告审阅页面或审阅模式（v2 语义随迁证明）。
6. Review Center 联动改造（决定入口移除回归证明）。
7. 旧报告深链兼容（含仓库设置页链接改导向）。
8. 全局搜索路由调整（前置门通过时；否则为缺口登记单）。
9. 路由兼容矩阵。
10. 组件变更清单。
11. API 和类型依赖清单。
12. 测试报告。
13. E2E 测试结果。
14. Commit 清单。
15. 回滚说明。
16. D-4 综合提案落点与 Knowledge 默认页签裁决记录（REV-1）。
17. 《Phase 2 验收报告 V1.0》。

---

# 14. Phase 2 开工门槛

以下条件全部满足后，才正式进入实施（✓ = 2026-11-16 抽验已满足）：

- [x] Phase 1 C1～C5 已完成并通过验收。
- [x] `/project/analysis` 路由占位已经存在（navigation.ts:296，router/navigation 测试已引用），或已核验可安全新增。
- [x] `ReviewCenterPage.vue:549` 深链生产方定位确认（抽验仍准确；实施日仍须重新定位行号）。
- [x] `?report=` 生产方/消费方清单已建立（4 文件，见 §3.3）。
- [ ] 已重新核验 Knowledge 页面 Analysis Reports 实现（含默认页签与三页签消费逻辑）。
- [ ] 已核验 Report Detail 和 Report Review 的现有组件（含 AnalysisReportsPanel.vue 拆分边界）。
- [ ] 已核验 Analysis 报告列表和详情 API。
- [ ] 已核验 Report Revision 数据契约（含正文借住 kb 存储链路的读取路径，REV-2）。
- [ ] 已通过 CTRL-1 审阅归属追链前置门。
- [ ] 已核验审阅流 v2 暂记/改期/close 与 409 report_open 在迁移后的承接方式。
- [ ] 已裁决 Knowledge 页迁出后默认页签（建议 `kb`，REV-1）。
- [ ] 已裁决 D-4 综合提案展示落点（REV-1）。
- [ ] 已通过 CTRL-3 全局搜索前置核验门（或完成缺口登记）。
- [ ] 已确认 Phase 2 不需要 Schema Migration。
- [ ] 已建立可回滚的实施任务卡。

---

# 15. 最终架构原则

Phase 2 必须始终遵循以下原则：

**原则一：领域归属决定页面归属。**

Analysis Report 是 Analysis 的对象，因此报告管理和报告审阅属于 Analysis。

**原则二：审核中心是入口，不是审批领域。**

人工审核中心可以聚合多个领域的待处理事项，但不能接管领域自己的审批语义和生命周期。

**原则三：页面迁移不改变领域权威。**

Analysis 的 Report、Report Revision、Finding 仍由 Analysis 负责；KB、PC、Project Cognition 继续维护各自的正式对象。前端迁移不假设、不推动存储权威归位（REV-2）。

**原则四：不以 UI 迁移推动 Schema 重构。**

本阶段先兑现页面独立化。领域模型、数据库结构和存储权威必须遵循既有架构裁决。

**原则五：兼容性属于正式交付。**

旧深链、通知、搜索、历史入口不能因为页面拆分而失效。

**原则六：单操作面。**

领域审阅操作在同一时刻只存在于一个界面（CTRL-1 / P-03）；迁移完成即移除旧侧操作入口，回滚必须双侧同步。

---

**最终目标：Analysis 从 Knowledge 页面中的一个 Tab，演进为具有独立导航、独立报告管理、独立报告审阅和清晰领域边界的正式产品模块。**
