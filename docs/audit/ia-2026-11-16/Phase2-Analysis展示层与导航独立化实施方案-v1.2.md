# PM-AI Phase 2：Analysis 展示层与导航独立化实施方案 V1.2

**文档状态：** 已冻结（核验通过 + 两项裁决闭合，作为任务包 V1.0 的实施依据）
**阶段正式名称：** Phase 2 — **Analysis Presentation & Navigation Independence（Analysis 展示层与导航独立化）**
**编制 / 修订：** 2026-11-16（V1.1 → V1.2）
**前置：** 《Phase 2 Analysis 独立化源码核验报告 V1.0》（A~J 全项，READY WITH CONDITIONS）、《Phase 2 裁决记录 D-1/D-2 V1.0》（已裁决）
**核心范围：** Analysis 独立入口、报告列表、报告详情（三类产物 + D-4）、审阅交互迁移、审核中心与仓库设置入口迁移、旧深链兼容、回归验收

---

# 0. V1.2 修订记录

| 编号 | 内容 | 来源 |
|---|---|---|
| N-1 | 阶段更名：**展示层与导航独立化**，不是 Analysis 领域模型重构；新增里程碑边界表（§4.6）与原则七 | 用户终审 §一/§五/§六 |
| N-2 | 两项裁决闭合并入：D-1 默认页签=KB；D-2 综合提案随报告迁移（详情六节结构） | 裁决记录 V1.0 |
| N-3 | §5.2 按**现数据契约**改写：详情=报告头+KB 修订全文+提案集+PC 修订集（无 F/C/R、无 Report Revision） | 核验 §3.3/§3.4 |
| N-4 | §5.3 审阅改为**复用三组既有端点**（KB confirm / P5 决定 / close），P2-04 核心验收=**关闭语义七条保持** | 核验 §4.2 / 用户终审 §三.2 |
| N-5 | P2-07 正式**暂缓→后端契约缺口（GAP-1）**，AC-09 走 B 态 | 核验 §7.3 / CTRL-3 门 |
| N-6 | 新增六条禁止事项（不创建 Revision/FCR 实体、不改状态机与关闭事务、不造第二份正文权威等） | 用户终审 §三.1 |
| N-7 | 深链兼容表定版（含仓库设置入口→Analysis 列表） | 用户终审 §三.3 |
| N-8 | Z-01/K-05 处置定版：迁移不削弱安全门、不改身份校验方式、Z-01 入安全基线专项、K-05 保留登记且不引入全局请求管理 | 用户终审 §三.4 |
| N-9 | 任务卡边界与顺序按终审调整（P2-05 条件=P2-01；P2-06 含 Knowledge 拆分；P2-07 独立排期） | 用户终审 §四 |
| N-10 | 质量基线以核验报告 §9 实测值为准（148/148、typecheck 0、prettier 20、i18n 39） | 核验 §9 |

---

# 1. 阶段定位（原「项目背景」）

## 1.1 一句话定位

**Phase 2 是 Analysis 的前端领域独立化（展示层与导航），不是 Analysis 后端领域模型重构。**

现有 `AnalysisReportsPanel.vue` 已具备独立迁移条件（与 KnowledgeView 零领域耦合，props 仅 `projectId`，核验 §2.2）。当前报告门模型、KB 修订承载、PC 提案与关闭流程在领域归属上不完全符合长期目标架构，**但不借本次 UI 迁移顺手重构**。

## 1.2 现状事实基线（源码实证，详见核验报告）

- Analysis 现状 = D-5 报告门模型：`integration.repo_analysis_report`，一次运行=一份报告；状态机 analyzing→analyzed→closed（failed/cancelled 终态）。
- API 仅 3 端点：list / detail / close（`analysis_reports.rs:25-33`）。
- 详情聚合 = KB 修订全文（DB `kb.document_revision.content` 直读）+ PC 提案集 + PC 修订集。
- Findings / Conclusions / Recommendations、Report Revision：**全链不存在**。
- 审阅 = 三域三动作：KB confirm（三重门）/ PC 提案决定（暂记可互改、closed 冻结）/ 报告关闭（interactive 门 + 批量发布 + 撤回 + 拒稿 + 锚点固化）。
- Review Center 对报告事项已是只聚合+跳转（ReviewCenterPage.vue:542-555）。
- 检索仅 KB/PC 双域，无 Analysis 引用。

## 1.3 核心目标

1. `/project/analysis` 成为 Analysis 独立入口，承载报告列表、详情、审阅交互与关闭。
2. `/project/knowledge` 保留 KB、PC 两页签，默认 KB（裁决 D-1）。
3. Review Center 与仓库设置入口改导向 Analysis。
4. 旧深链全部兼容（§6 定版表）。
5. KB / PC / Integration 的领域端点、状态机、权限门**零改动**。

---

# 2. Phase 2 范围

## 2.1 必须完成

| 编号 | 工作内容 | 优先级 | 开工条件 |
|---|---|---|---|
| P2-01 | Analysis 独立页面与路由 | P0 | 可立即开工 |
| P2-02 | Analysis 报告列表迁移 | P0 | P2-01 |
| P2-03 | 报告详情与三类产物展示（含 D-4 六节结构） | P0 | 按现有数据契约 |
| P2-04 | KB 确认 / PC 决策 / 报告关闭交互迁移 | P0 | 保留三组既有端点 |
| P2-05 | Review Center、仓库设置入口迁移 | P0 | P2-01 |
| P2-06 | Knowledge 双页签拆分、默认页调整、D-4 落点迁移、旧深链兼容 | P0 | 两项裁决（**已闭合**） |
| P2-07 | 全局搜索 Analysis 支持 | P1 | **暂缓，转后端契约缺口** |
| P2-08 | 兼容性、权限、测试与回归验收 | P0 | 前序任务完成 |

## 2.2 本阶段禁止（终审定版，六条新增以 ● 标记）

1. 不修改 Report / Report Revision 的权威模型。
2. 不将 Report Revision 改成 KB Document Revision。
3. 不合并不同领域的 Finding。
4. 不将 Analysis Report 自动转换成 KB Memory。
5. 不修改 KB / PC 的正式审核流程与审批逻辑。
6. 不修改四域数据库 Schema；不创建数据库 Migration。
7. 不新建通用审批领域；不实现第二套审批逻辑。
8. ● 不创建 Report Revision 数据库实体。
9. ● 不创建 Findings / Conclusions / Recommendations 表或 API。
10. ● 不迁移 KB、PC 的审批逻辑（迁移的是 UI 承载位置）。
11. ● 不修改 `repo_analysis_report` 状态机。
12. ● 不修改报告关闭时的批量发布、撤回和锚点固化事务。
13. ● 不将报告正文复制到新的 Analysis 前端状态存储，造成第二份权威数据。
14. 不修改 Phase 0 已登记的存量基线问题（prettier 20 / i18n 39 / layout test 存量红）。

如发现现有 API 无法满足页面需求，登记为独立后端契约缺口（§10 缺口清单），不得顺手实现。

---

# 3. 目标导航与路由

## 3.1 导航位置

Analysis 位于 Current Project / 项目工作区，与 KB、PC 同级（Phase 1 C3 已建占位 navigation.ts:296，本阶段升级为真实页面）。

## 3.2 路由

| 路由 | 用途 | 页面类型 |
|---|---|---|
| `/project/analysis` | Analysis 首页 = 报告列表 | 列表页 |
| `/project/analysis/reports/:reportId` | 报告详情（含审阅交互） | 详情页 |

说明：详情与审阅**合并在详情页**（行内展开形态升级为页面形态即可，不强制独立 review 路由）；路由不暗示未实现能力；全部复用 `shell.currentProjectId` 与现有项目切换守卫。

## 3.3 Knowledge 页改造（裁决 D-1）

- 页签构成：`kb | pc`；无参默认 **kb**。
- `?doc=`、`?object=` 行为不变。
- `?report=` 兼容跳转见 §6。

---

# 4. 职责边界与里程碑

## 4.1 Analysis（本阶段=展示层职责）

承载：报告列表、报告详情、三类产物展示、审阅交互（KB 确认 / PC 决策 / 关闭）、D-4 综合入口（裁决 D-2）。页面不得自行推导或伪造审阅状态；不得复制正文造第二权威。

## 4.2 KB / PC / Integration（零改动）

- KB：documents/revisions/confirm 端点与状态机不变。
- PC：proposals/decision 端点（P5）、暂记-互转-冻结语义、发布事务不变。
- Integration：`repo_analysis_report` 状态机、close 事务（批量发布 pc-decision:{id} / 撤回 / 拒稿 / 锚点固化）不变。

## 4.3 Review Center

对 report_pending 只聚合+跳转（现状已成立，改导向即可）；其 finding/proposal/delivery 待办的就地决策是各领域本职（D-6 既有设计），**不在本阶段处理范围**。

## 4.4 D-4 身份综合（裁决 D-2）

前端承载迁入 Analysis 报告详情第 5 节；端点、权限、审批语义、所有权不变。

## 4.5 里程碑边界表（防误解，终审 §五）

迁移完成后实际状态：

| 层次 | 迁移完成后的状态 |
|---|---|
| 前端导航 | Analysis 独立 ✓ |
| 前端页面 | Analysis 报告独立承载 ✓ |
| API 入口 | Analysis API 独立存在 ✓（3 端点） |
| 报告正文权威 | **仍是现有 DB 中的 KB 修订内容** |
| Report Revision | **尚不存在** |
| Findings / Conclusions / Recommendations | **尚不存在** |
| 审阅决策 | **仍由 KB、PC、Integration 各自负责** |
| 全局检索 | **尚不支持 Analysis** |

**本阶段命名即边界：Presentation & Navigation Independence ≠ Domain Independence。** 后续 Report Revision、Finding Contract、Git 正文权威迁移各有其阶段。

---

# 5. 页面设计

## 5.1 Analysis 首页 / 报告列表（P2-02）

数据源：`GET /v1/pm/analysis-reports?project_id=`（现有 API，无分页——继承现状，GAP-2 另行登记）。

- 状态分区：分析中 / 待审理（默认）/ 已关闭 / 失败；「显示历史」切换（现面板行为原样迁移）。
- 行字段：仓库名 @ 短 sha、状态徽标、run_mode 增量徽标、pending 计数（`pending_proposals + has_pending_kb`）、时间（以实际 API 字段为准）。
- analyzing 行 3s 轮询自动浮现（现行为保留）。
- 「来源」列不得凭猜测归并执行身份（analysis_run 双写入者分叉属既有登记，UNKNOWN 不自填）。
- 加载 / 空 / 错误 / 无权限（404 语义）状态齐备；零 mock。
- 点击行 → 详情页。

## 5.2 报告详情（P2-03，按裁决 D-2 六节结构）

数据源：`GET /v1/pm/analysis-reports/{id}?project_id=`（现有契约；**无 Findings/Conclusions/Recommendations、无 Revision 身份——不得虚构**）。

| 节 | 内容 | 数据 |
|---|---|---|
| 1 | 报告基本信息 | 仓库名 @ sha、状态、run_mode/since_commit、created/updated/closed_at、失败 error_code/detail |
| 2 | 分析报告正文（当前 KB 修订） | `kb_revision.content`（Markdown 渲染；DB 直读是现状权威，GAP-5 另行登记）；revision_no/status 徽标 |
| 3 | PC 提案 | `proposals[]`；待决/已决/全部过滤；approved 显示「已批准·待发布」 |
| 4 | PC 修订内容 | `pc_revisions[]`（对象名/类型/版本号/变更摘要/全文） |
| 5 | D-4 身份综合 | 综合按钮（hasAnalyzedReport 门）+ 综合产出三提案状态展示；端点语义不变 |
| 6 | 报告关闭与处置结果 | 关闭按钮（仅 analyzed）+ 确认对话框（发布/撤回/拒稿计数预览）+ 关闭回执（applied/withdrawn/rejected/锚点） |

要求：

- 版本头部以详情 API 返回为准回填；每次进入详情重新拉取（现面板「缓存会陈旧」原则保留）。
- 正文读取走详情 API 单一通道；不得假设 Git 直读、不得拼装其他端点、不得复制正文进前端 store 造第二权威（§2.2 ● 13）。
- 非法/跨项目 Report ID → 显式错误态（修复现面板静默不展开，GAP-6 随本卡关闭）。

## 5.3 审阅交互（P2-04，核心验收=关闭语义保持）

**复用三组既有端点，零后端改动：**

| 交互 | 端点 | 现有门（不得削弱） |
|---|---|---|
| KB 确认 | `POST /v1/kb/documents/{d}/revisions/{r}/confirm` | 三重门（interactive+真人+成员）；幂等重确认 |
| PC 提案批准/驳回 | `POST /v1/pc/proposals/{id}/decision` | trusted 上下文 + DB 查 human + 治理授权 + CAS expected_version_no |
| 报告关闭 | `POST /v1/pm/analysis-reports/{id}/close` | interactive 门 + 事务 + FOR UPDATE |

**关闭语义七条 = P2-04 核心验收（终审 §三.2）：**

1. PC 提案「批准」只是**暂记**（approved = 已批准·待发布）——新页面不得把批准设计成正式发布。
2. 批准与驳回在报告开放期间（analyzed）**可互转**——按钮显隐条件保持 `status ∈ {submitted, approved, rejected}` 且报告 analyzed。
3. 报告关闭后决定**冻结**（后端 409 frozen；前端关闭态不再渲染决策按钮）。
4. 关闭时才统一发布已批准提案（幂等键 pc-decision:{id}）。
5. 未处理提案（draft/submitted）撤回；KB 草稿修订拒绝——关闭回执如实展示计数。
6. 关闭事务固化仓库分析锚点（last_analyzed_commit）——回执展示锚点短 sha。
7. 仅 analyzed 可关闭；409 report_open 报告门语义在 UI 可见（重复分析被拒的报错路径）。

其余要求：理由输入与审计按现有端点契约（PC 决策 note、关闭无理由字段——不擅自新增）；二次确认（关闭 ConfirmationDialog、提案批准可沿用现面板单击或 RC 的 armOrSubmit 双击确认——**二选一随迁，不引入新确认范式**）；并发/重复提交沿用前端单飞守卫 + 后端幂等。

## 5.4 Review Center 联动（P2-05）

- ReviewCenterPage.vue:549 跳转目标改为 `/project/analysis/reports/{report_id}`（link.params 结构不变，仅路径变）。
- 仓库设置入口 `goToAnalysisReports`（SettingsProjectRepositoriesView.vue:354）改导向 `/project/analysis?project_id=…`（**列表**，裁决兼容表）。
- RC 其余四类待办行为零改动。

---

# 6. 深链兼容（定版表，终审 §三.3）

| 旧入口 | 迁移后行为 |
|---|---|
| `/project/knowledge?report=X` | **兼容跳转** Analysis 报告 X（保留 Report ID + 项目上下文 + 定位行为；不得退化为丢失参数的普通 redirect） |
| `/project/knowledge` | KB 默认页（裁决 D-1） |
| `/project/knowledge?doc=X` | 原 KB 文档详情，不变 |
| `/project/knowledge?object=X` | 原 PC 对象详情，不变 |
| Review Center 报告事项 | 跳转 Analysis 报告详情 |
| 仓库设置分析报告入口 | 跳转 Analysis 报告列表 |

实现约束：

- `?report=` 兼容跳转在 Knowledge 页路由层处理：识别 `report` 参数 → 保留 `project_id` → 跳 `/project/analysis/reports/{id}`；`report` 与 `doc/object` 同时出现时**不得自行猜测优先级**——迁移后 report 分支先行跳离，组合自然消解；实施时在测试中固化该行为。
- 深链全量清单（生产/消费方）以核验报告 §6.1 为准（生产 1 处、消费 2 处、导向 1 处、通知 0 处）；实施日须重新 grep 复核行号。
- 跳转失败（Report 不存在/跨项目）显示明确错误，不静默。

---

# 7. 任务卡（摘要；可执行版见《Phase 2 实施任务包 P2-01~P2-08 V1.0》）

| 卡 | 边界要点 |
|---|---|
| P2-01 | 注册 `/project/analysis`、页面框架、四态设计、导航选中；不改全局 Grid/侧栏结构/切换守卫 |
| P2-02 | 列表迁移：现 API 现字段；状态分区+历史开关+轮询保留 |
| P2-03 | 详情页六节；零虚构字段；GAP-6（非法 ID 错误态）随卡关闭 |
| P2-04 | 三组端点原样接入；关闭语义七条=核心验收 |
| P2-05 | RC :549 + 仓库设置 :354 两处改导向 |
| P2-06 | Knowledge 拆两页签、默认 kb、`?report=` 兼容跳转、D-4 随详情已在 P2-03 承载（本卡回归验证） |
| P2-07 | 不实施。产出：GAP-1 缺口登记单（检索 Analysis/Report 引用契约） |
| P2-08 | 回归+权限负向+基线不扩大+验收报告 |

---

# 8. 执行顺序（终审定版）

```text
① 裁决闭合（D-1/D-2 —— 已完成，裁决记录 V1.0 落盘）
        ▼
② P2-01 独立 Analysis 页面（/project/analysis，不再依附 KnowledgeView）
        ▼
③ P2-02 ~ P2-04 迁移报告功能（列表 → 详情 → 审阅交互；复用原 API 与业务逻辑）
        ▼
④ P2-05 ~ P2-06 迁移入口并拆分 Knowledge（RC 跳转、仓库设置入口、旧深链兼容、默认页）
        ▼
⑤ P2-08 全量回归（权限、项目切换、历史报告、异常 ID、重复操作、关闭事务语义）
        ▼
⑥ P2-07 独立排期（等待检索 API 提供稳定 Analysis/Report 引用契约）
```

说明：P2-05 开工条件=P2-01（终审表），但跳转目标页由 P2-03 提供，故实际排在 ③ 批次之后执行；每卡独立 Commit、独立可回滚（§9）。

---

# 9. Commit 与回滚

- 一卡一 Commit；建议前缀：
  - `feat(analysis): add independent analysis route`
  - `feat(analysis): migrate report list`
  - `feat(analysis): add report detail`
  - `feat(analysis): migrate review actions`
  - `feat(review-center): route analysis report items`
  - `fix(knowledge): split tabs and preserve legacy report deep links`
- 回滚特别项：P2-04/P2-05 回滚必须双侧同步（新页与 RC/设置入口导向一致），不留「跳转到不存在页面」的中间态；P2-06 回滚须恢复默认页签与 `?report=` 消费的成对改动。

---

# 10. 缺口登记清单（本阶段只登记不实现）

| 编号 | 内容 | 承接 |
|---|---|---|
| GAP-1 | 检索无 Analysis 域/Report ID（CTRL-3 门未过） | P2-07 暂缓依据；转后端契约 |
| GAP-2 | 列表无分页 | 后端独立卡 |
| GAP-3 | report_detail 经全列表扫描定位 | 后端独立卡 |
| GAP-4 | F/C/R 对象不存在 | 四域目标模型演进序列 |
| GAP-5 | Git 正文权威未落地（DB 直读） | 四域目标模型演进序列 |
| GAP-6 | 非法 Report ID 深链静默 | **随 P2-03 关闭** |
| GAP-7 | close/KB confirm 人身门未查库 principal.kind（Z-01） | **安全基线专项复核**（本阶段不改身份校验方式） |
| K-05 | 前端请求竞态无守卫 | 保留登记：迁移时避免新增竞态，不引入未经设计的全局请求管理机制 |

---

# 11. 测试与质量门禁

## 11.1 基线（核验实测，2026-11-16）

| 检查 | 基线 | 约束 |
|---|---|---|
| 定向 vitest（router/shell/knowledge/review-center） | 148/148 绿 | 迁移后全绿且覆盖新行为 |
| typecheck | 0 | 保持 0 |
| prettier | 20（存量红） | 不扩大 |
| i18n hardcode | 39（存量红） | 不扩大；新增文案全部走 locales |
| namespace | 通过 | 保持通过 |

公共回归命令保留：`pnpm -s vitest run src/router/ src/components/shell/`（并纳入 `src/views/project/analysis/`、knowledge、review-center）。

## 11.2 E2E 场景（10 项，沿用 V1.1 并按裁决校正）

E2E-01 查看报告 / E2E-02 审阅 v2 全语义（暂记→互转→close 批发，含 409 冻结）/ E2E-03 RC 跳转 / E2E-04 `?report=` 兼容 / E2E-05 `?doc=` + 默认页签=kb / E2E-06 项目隔离 / E2E-07 项目切换 / E2E-08 过期 Revision（CAS 409 路径）/ E2E-09 列表-详情同源（多报告多 commit 形态）/ E2E-10 D-4 综合在 Analysis 详情可达（裁决 D-2 落点）。

---

# 12. 验收标准（V1.2 定版）

| 编号 | 验收项 | 通过标准 |
|---|---|---|
| AC-01 | 独立入口 | `/project/analysis` 可从项目导航进入，导航选中态正确 |
| AC-02 | 报告列表 | 数据来自 Analysis API；状态分区/历史开关/轮询行为等价迁移 |
| AC-03 | 报告详情 | 六节结构（裁决 D-2）；正文与版本正确；非法 ID 显式错误（GAP-6 关闭） |
| AC-04 | 三类产物 | KB 修订全文 / PC 提案集 / PC 修订集正确展示（F/C/R 不存在，不虚构） |
| AC-05 | 审阅交互 | 三组既有端点复用；**关闭语义七条全部保持**（§5.3） |
| AC-06 | Review Center | report_pending 只跳转（改导向）；其余四类零改动 |
| AC-07 | 深链兼容 | §6 定版表逐行通过；`?report=` 不丢 ID/项目/定位 |
| AC-08 | Knowledge | 双页签、默认 kb（裁决 D-1）；`?doc=`/`?object=` 不变 |
| AC-09 | 搜索 | **B 态结项**：GAP-1 缺口已登记，未做任何猜测性跳转 |
| AC-10 | 项目隔离 | 无跨项目读取；切换期间无残留（不新增竞态，K-05 维持登记） |
| AC-11 | 操作权限 | 无权限用户三动作均被后端门拒绝（负向测试）；前端按钮显隐条件与后端门一致 |
| AC-12 | 版本一致性 | 详情以详情 API 为准回填；关闭后决定冻结不可再改 |
| AC-13 | 回归 | 导航、Shell、项目切换正常 |
| AC-14 | 质量门禁 | §11.1 五项全过且基线不扩大 |
| AC-15 | 单操作面 | 审阅操作只存在于 Analysis 详情页一处；RC 无决定入口 |
| AC-16 | 边界确认 | §4.5 里程碑边界表逐项核实（防止误判领域已完成独立化） |

**安全验收表述（终审 §三.4）：** 本次迁移不削弱既有权限和安全门；不改变既有端点的身份校验方式；Z-01 保留并进入安全基线专项复核；**不把本次 UI 迁移通过等同于 Analysis 审批安全审计通过。**

---

# 13. 交付物

1. 《Phase 2 裁决记录 D-1/D-2 V1.0》✅ 已落盘
2. 《源码核验报告 V1.0》✅ 已落盘
3. Analysis 独立页面 + 路由（P2-01）
4. 报告列表迁移（P2-02）
5. 报告详情六节（P2-03）
6. 审阅交互迁移（P2-04）
7. RC/仓库设置入口改导向（P2-05）
8. Knowledge 拆分与深链兼容（P2-06）
9. GAP-1 缺口登记单（P2-07 暂缓产物）
10. 回归与验收报告（P2-08，含 §4.5 边界表核实记录）
11. Commit 清单与回滚说明

---

# 14. 开工门槛

- [x] Phase 1 C1~C5 完成且导航占位存在（navigation.ts:296）
- [x] 源码核验报告 V1.0 完成（READY WITH CONDITIONS）
- [x] 裁决 D-1（默认页签=KB）✅ 落盘
- [x] 裁决 D-2（综合提案随报告迁移）✅ 落盘
- [x] 方案 V1.2 修正冻结 ✅ 本文档
- [x] P2-07 缺口登记路径明确（GAP-1 → 任务包承接）
- [x] CTRL-1 审阅归属追链完成（通过态 B，三组端点）
- [ ] 任务包 V1.0 经用户确认后开工 P2-01

---

# 15. 最终架构原则（V1.2）

一：领域归属决定页面归属（展示层）。
二：审核中心是入口不是审批领域。
三：**页面迁移不改变领域权威与端点契约**。
四：不以 UI 迁移推动 Schema 重构（六条 ● 禁止事项）。
五：兼容性属于正式交付。
六：单操作面。
**七：展示层独立 ≠ 领域重构——Phase 2 交付的是 Presentation & Navigation Independence；Report Revision、Finding Contract、Git 正文权威、检索 Analysis 化均属后续演进，本阶段只登记不实现。**

---

**最终目标：Analysis 从 Knowledge 页中的一个页签，演进为具有独立导航、独立页面承载和清晰展示层边界的正式产品模块——同时如实地保持其领域模型尚未独立化的现状登记。**
