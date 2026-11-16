# Phase 2 验收报告 V1.0

**阶段：** Phase 2 — Analysis 展示层与导航独立化（Presentation & Navigation Independence）
**实施基线：** master @ `d3b3073`（2026-11-16），实施完成后至 `test(analysis): isolation and regression coverage`（P2-08，本报告随该 Commit 提交）
**验收依据：** 方案 V1.2 §12 AC-01~16 / 任务包 V1.0 P2-08 必测矩阵 / §5.3 关闭语义七条 / §4.5 里程碑边界表
**执行：** 开发 Agent（一卡一 Commit，串行 P2-01→06→07→08）

---

## 1. Commit 清单（附 B 链，全量落地）

| 卡 | Commit | 消息 |
|---|---|---|
| P2-01 | `65b7e8b` | feat(analysis): add independent analysis route |
| P2-02 | `e15ffd4` | feat(analysis): migrate report list |
| P2-03 | `36defcf` | feat(analysis): add report detail |
| P2-04 | `fa3e598` | feat(analysis): migrate review actions |
| P2-05 | `24eef43` | feat(review-center): route analysis report items |
| P2-06 | `a5b977b` | fix(knowledge): split tabs and preserve legacy report deep links |
| P2-07 | `927d6f5` | docs(analysis): register retrieval analysis-domain gap |
| P2-08 | 本提交 | test(analysis): isolation and regression coverage |

说明：P2-05 未拆设置页子 Commit（任务包允许二选一）；P2-06 附带 P2-01~04 交付文件
的 prettier 格式化（无语义变更，AC-14 门禁）；P2-08 含 close 403 走 forbidden 文案
的一处行为补齐（E2E-11 三动作负向覆盖）。

## 2. 质量门禁实测（P2-08 收口时）

| 检查 | 结果 | 对比基线 |
|---|---|---|
| 定向 vitest（router/shell/analysis/knowledge/review-center） | **187/187 绿**（21 文件） | 基线 148 → 187（+39 为本次新增覆盖） |
| typecheck | **0** | 保持 0 |
| prettier | **全绿（exit 0）** | 存量 20 红已在并行工作中清零；本次交付文件全部合规，未新增 |
| i18n hardcode | 已提交树 **39 处 = 基线 39，未扩大** | 工作树另有 95 处来自他人**未提交**的 design/nav-v2 文件（本阶段不碰不提交，见 §7） |
| i18n namespace | 通过（zh-CN/en 48 命名空间全对齐） | 保持通过 |
| Release Guard（production 构建） | **通过**（exit 0；dist 无 fixtures/demo/mock 模块） | — |

## 3. AC-01~16 逐项判定表

| 编号 | 验收项 | 判定 | 证据 |
|---|---|---|---|
| AC-01 | 独立入口 | **PASS** | `/project/analysis` 真组件路由（router.test「P2-01」断言）；导航 planned 移除（navigation.test / SideNav.test 5→4）；选中态走壳既有 is-active 机制 |
| AC-02 | 报告列表 | **PASS** | AnalysisHomeView.test 12 例：分区/历史开关/轮询启停/增量徽标/pending 计数/失败明细/四态/项目切换；数据唯一来源 list API（契约零改动） |
| AC-03 | 报告详情 | **PASS** | AnalysisReportDetailView.test：六节渲染断言；非法/跨项目 ID → 显式 404 语义态（**GAP-6 关闭**）；每次进入重拉 |
| AC-04 | 三类产物 | **PASS** | KB 修订全文（MarkdownViewer + revision/status 徽标）/ PC 提案集（过滤+暂记文案）/ PC 修订集（名/类型/版本/摘要/全文）；F/C/R 无占位假数据（kbMissing/noProposals/noPcRevisions 空态如实） |
| AC-05 | 审阅交互 | **PASS** | 三组既有端点原样复用（confirmKbRevision / decidePcProposal / closeAnalysisReport，api/knowledge.ts·api/analysis-reports.ts **零改动**）；关闭语义七条全部保持（§4 自查表） |
| AC-06 | Review Center | **PASS** | report_pending 仅跳转改导向（`/project/analysis/reports/{id}?project_id=`）；RC 其余四类（finding/proposal/delivery）零改动（diff 仅一处路径） |
| AC-07 | 深链兼容 | **PASS** | §6 定版表逐行测试固化：`?report=` 保参跳详情（E2E-04 单测级）；`?doc=`/`?object=` 不变（回归通过）；report+doc 同现 report 先行跳离；非法 ID 落详情 404 态 |
| AC-08 | Knowledge | **PASS** | 双页签 kb|pc、默认 kb（裁决 D-1）；reports 页签不存在断言；旧面板文件退役删除 |
| AC-09 | 搜索 | **PASS（B 态）** | GAP-1 缺口登记单 V1.0（`927d6f5`）：后端三项契约需求 + 承接预告 + 禁止猜测性跳转；未做任何搜索改造 |
| AC-10 | 项目隔离 | **PASS** | 项目上下文单源 shell.currentProjectId；无项目空态不展示他项目数据；列表/详情跨项目 ID → 404 语义态；切换重置重载测试；K-05 维持登记（未引入 AbortController/全局请求管理，未新增竞态模式） |
| AC-11 | 操作权限 | **PASS** | 三动作 403 → forbidden 文案显式呈现（KB 确认 / PC 决策 / close，E2E-11 单测级）；按钮显隐条件与后端门一致；不重试不吞错 |
| AC-12 | 版本一致性 | **PASS** | 详情以详情 API 单通道回填、每次进入重拉（缓存陈旧原则）；closed 后决策按钮与关闭入口消失（冻结） |
| AC-13 | 回归 | **PASS** | 导航/Shell/项目切换 187/187 全绿；路由表断言含新详情路径 |
| AC-14 | 质量门禁 | **PASS** | §2 五项全过且基线不扩大（hardcode 已提交树 39=39；prettier 0 新增） |
| AC-15 | 单操作面 | **PASS** | 审阅操作（KB 确认/PC 提案决定/关闭）仅存在于 Analysis 详情页一处；RC 报告事项无决策输入（只跳转）；旧面板已删除，KnowledgeView 无报告决策入口。注：RC proposal 页签就地决策属 D-6 既有设计（PC 领域待办），不在「报告审阅操作」范围 |
| AC-16 | 边界确认 | **PASS** | §5 边界表逐项核实（防止误判领域已完成独立化） |

## 4. 关闭语义七条自查表（P2-04 核心验收）

| # | 语义 | 实现位置 | 测试证据 | 判定 |
|---|---|---|---|---|
| 1 | 批准=暂记（approvedPending），不得设计成正式发布 | `proposalStatusLabel`：approved → 「已批准·待发布」；成功文案「已批准（暂记——报告关闭时统一发布…）」 | 测试「七条#1」；E2E-02 全链断言暂记文案 | ✓ |
| 2 | 开放期批准/驳回可互转（显隐按现条件） | `canDecide`：报告 analyzed 且 status ∈ {submitted, approved, rejected}；approved 只显驳回、rejected 只显批准 | 测试「七条#2」（submitted/approved/rejected 三态显隐） | ✓ |
| 3 | 关闭后冻结（后端 409 frozen 保留；前端 closed 不渲染决策按钮） | `canDecide` 以 `detail.status==='analyzed'` 为门 | 测试「七条#3」；E2E-02 全链 closed 后无决策按钮 | ✓ |
| 4 | close 时统一发布（回执 applied 计数） | `doClose` 回执 `analysis.detail.closeReceipt` applied | P2-03「关闭链路」测试；E2E-02 全链「发布 1 个提案」 | ✓ |
| 5 | 未决撤回 + KB 草稿拒绝（回执 withdrawn/rejected） | 同上（withdrawn、rejected_kb→1） | P2-03「关闭链路」测试（撤回 2/拒绝 1） | ✓ |
| 6 | 锚点固化（回执 last_analyzed_commit 短 sha） | 同上（commit 12 位） | P2-03 测试 + E2E-02 全链「abc123def456」 | ✓ |
| 7 | 仅 analyzed 可关；409 报错路径 UI 可见 | 关闭入口仅 `detail.status==='analyzed'` 渲染；catch 显式呈现 | P2-03「仅 analyzed」测试；P2-04「close 409」测试 | ✓ |

## 5. §4.5 里程碑边界表逐项核实记录（AC-16）

| 层次 | 迁移完成后状态 | 核实证据（实施日复核） |
|---|---|---|
| 前端导航 | Analysis 独立 ✓ | navigation.ts 真组件项；PLANNED 清单仅剩 Product Cognition |
| 前端页面 | Analysis 报告独立承载 ✓ | `views/project/analysis/`（列表 + 详情），旧面板文件已删除 |
| API 入口 | Analysis API 独立存在 ✓（3 端点） | `api/analysis-reports.ts` 本阶段零 diff：list / detail / close |
| 报告正文权威 | **仍是现有 DB 中的 KB 修订内容** | 详情 API 返回 `kb_revision.content`（DB 直读，GAP-5 维持登记）；前端仅渲染，未建任何正文持久 store（● 红线 13 遵守） |
| Report Revision | **尚不存在** | 前后端 grep 复核 0 命中；详情「版本」= commit_sha + kb_revision.revision_no |
| Findings / Conclusions / Recommendations | **尚不存在** | 前后端 grep 复核 0 命中；详情无 F/C/R 假数据 |
| 审阅决策 | **仍由 KB、PC、Integration 各自负责** | confirm/decide/close 三端点原样；状态机与 close 事务零改动（crates/、apps/、migrations/ 零 diff） |
| 全局检索 | **尚不支持 Analysis** | GAP-1 登记单（`927d6f5`）；WorkspaceShell target 未改 |

**结论：Presentation & Navigation Independence 达成；Domain Independence 未发生、
亦未伪装发生（本报告即边界登记）。**

## 6. E2E 执行状态（必测矩阵 → 单测级覆盖映射）

**浏览器级 E2E：NOT_RUN**（本轮无可用 E2E 执行环境，如实登记，不以单测谎称 E2E 通过）。
以下为逐场景单测级覆盖（同等语义断言，全绿）：

| E2E | 场景 | 单测级覆盖 |
|---|---|---|
| E2E-01 | 查看报告 | 详情六节渲染测试（AnalysisReportDetailView.test） |
| E2E-02 | 审阅全语义（暂记→互转→close 批发 + 409 冻结） | 「七条#1~#3/#7」+ E2E-02 全链 describe |
| E2E-03 | RC 跳转 / 仓库设置入口 | RC test 跳转断言 + 设置页跳转断言（P2-05） |
| E2E-04 | `?report=` 兼容 | KnowledgeView.test 保参跳转断言 + AnalysisHomeView 承接测试 |
| E2E-05 | `?doc=` + 默认页签=kb | 「默认页签 = KB」测试 + `?doc=` 回归测试 |
| E2E-06 | 项目隔离 | 列表/详情无项目空态 + 404 语义态测试 |
| E2E-07 | 项目切换无残留 | 列表切换重载测试 + 详情上下文变化重拉测试 |
| E2E-08 | CAS 409 | 「CAS 409」测试（过期决定被拒文案可见） |
| E2E-09 | 列表-详情同源 | 行点击 → 详情路由断言 + 详情同契约渲染（多报告形态由 GROUPS 分区测试覆盖） |
| E2E-10 | D-4 在详情可达 | 「第 5 节 D-4 门」+「D-4 综合动作」测试 |
| E2E-11 | 无权限三动作 403 | P2-08「close 403」+ P2-04「KB/PC 403」测试 |

## 7. 安全验收表述与登记

- 本次迁移**不削弱、不修改**任何既有权限与身份门：后端 crates/、apps/、migrations/
  **零改动**（全阶段）；前端按钮显隐条件与后端门一致。
- 不改变既有端点身份校验方式；**Z-01**（close/KB confirm 人身门未查库
  principal.kind）维持登记，归 ui-ux-security-baseline 安全基线专项复核。
- K-05（前端请求竞态无守卫）维持登记：迁移等价复刻既有模式，未新增竞态，
  未引入未经设计的全局请求管理机制。
- **UI 迁移通过 ≠ Analysis 审批安全审计通过。**
- 工作树纪律：他人未提交改动（design/nav-v2 系）全程未碰、未提交，其引入的
  95 处 i18n hardcode 不计入本阶段基线。

## 8. 回滚说明（成对要求）

| 卡 | 回滚方式 | 成对约束 |
|---|---|---|
| P2-01 | 删路由与页面目录；navigation 项恢复 planned 占位；测试同步还原 | — |
| P2-02 | 列表组件还原至 P2-01 框架态；路由仍在 | — |
| P2-03 | 详情路由保留占位；列表 `?report=` 承接仍可用 | — |
| P2-04 | 删新交互（旧面板当时仍在）；**双侧同步** | 旧面板退役在 P2-06，届时回滚须成对恢复 |
| P2-05 | 两处导向还原 | **必须与新页存废成对**（不留死链中间态） |
| P2-06 | 五件套一次还原：页签结构 / 默认页签 / `?report=` 消费 / 旧面板文件 / 测试 | 不留半迁移态；且须先回滚 P2-05（导向目标存在性） |

## 9. 未关闭项清单（移交后续）

| 项 | 状态 | 承接 |
|---|---|---|
| GAP-1 检索 Analysis 域契约 | 已登记（P2-07） | 后端契约卡 → 前端分流卡 |
| GAP-2 列表无分页 | 维持登记 | 后端独立卡 |
| GAP-3 report_detail 全列表扫描 | 维持登记 | 后端独立卡 |
| GAP-5 Git 正文权威未落地（DB 直读） | 维持登记 | 四域目标模型演进 |
| GAP-6 非法 Report ID 静默 | **已关闭**（P2-03 显式 404 态） | — |
| GAP-7 / Z-01 人身门未查库 kind | 维持登记 | ui-ux-security-baseline 专项 |
| K-01 KnowledgeView.vue.js 构建产物入库 | 维持登记 | 仓库卫生独立卡 |
| K-05 请求竞态无守卫 | 维持登记（未恶化） | 既有登记 |
| K-06 AnalysisReportsPanel 无专属测试 | **已关闭**（P2-02 起 37 例专属测试） | — |
| 浏览器级 E2E（E2E-01~11） | **NOT_RUN** | E2E 环境就绪后按 §6 映射执行 |
