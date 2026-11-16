# Phase 2 Analysis 独立化源码核验报告 V1.0

**任务书：** 《Phase 2 Analysis 独立化源码核验任务书 V1.0》（A～J 全项）
**执行性质：** 只读核验，零源码修改、零迁移、零 Commit
**执行日期：** 2026-11-16
**核验基线：** master @ `58bdd988`（含 Phase 1 C1~C5 + `e7aad791` 侧栏 DS 重构 + `3dc4c61c` 构建修复）
**实施方案对照：** 《Phase 2 Analysis 独立化实施方案 V1.1》（REV-1~6 / CTRL-1~3）
**结论词汇：** 任务书 §13 六态（MATCH / PARTIAL / MISMATCH / NOT_IMPLEMENTED / UNKNOWN / BLOCKED）

---

# 0. 执行摘要

| # | 核心结论 | 状态 | 四层判定* |
|---|---|---|---|
| 1 | Analysis 现状 = **D-5 报告门模型**（`integration.repo_analysis_report`，一次运行=一份报告），非四域目标模型的 Report+Revision 实体 | MISMATCH（相对 Q2 目标态） | X✓ Y✓ Z✓ W— |
| 2 | Analysis API 面仅 **3 端点**：list / detail / close；详情聚合 = KB 修订全文 + PC 提案集 + PC 修订集 | MATCH（现状如实） | X✓ Y✓ Z✓ W— |
| 3 | **Findings / Conclusions / Recommendations 全链不存在**（无对象/端点/前端类型） | NOT_IMPLEMENTED | X✗ Y✗ Z— W✗ |
| 4 | **Report Revision 身份不存在**；"版本"= `commit_sha` + `kb_revision.revision_no` | NOT_IMPLEMENTED | X✗ Y✗ Z— W✗ |
| 5 | 报告正文权威 = **DB `kb.document_revision.content` 直读**，Git 权威未落地 | MISMATCH（相对 Q2 目标态） | X✓ Y✓ Z✓ W— |
| 6 | 审阅 = 三域三动作组合（KB confirm / PC decision / close），**Analysis 无自有审阅端点**；人身门全部实证 | MATCH（现状）；P-03 迁移可行 | X✓ Y✓ Z✓ W✓ |
| 7 | Review Center 对报告事项**已是只聚合+跳转**（:542 注释 + :549 单动作） | MATCH（P-03 后半句现状已成立） | X✓ Y✓ Z✓ W✓ |
| 8 | 全局搜索 **CTRL-3 前置门不通过**：`source_domain ∈ {KB, PC}`，无 Analysis/Report ID；target 硬编码 | NOT_IMPLEMENTED | X✓ Y✗ Z— W✗ |
| 9 | 深链面极小：`?report=` 生产 1 处、消费 2 处 + 仓库设置页页签导向 1 处；通知中心无报告深链 | MATCH | X✓ Y✓ Z✓ W✓ |
| 10 | 门禁实测：vitest 定向 **148/148 绿**、typecheck **0**、prettier **20**（基线红）、i18n hardcode **39**（基线红）、namespace **通过** | 基线与 Phase 1 结算一致 | W✓ |

\* 四层判定：**X**=代码存在，**Y**=业务可用（真实调用链走通），**Z**=安全正确（人身门/隔离实证），**W**=测试通过（本轮实际执行）。四层独立，不得互相推断。

**开工判定：READY WITH CONDITIONS**（§15）。

---

# 1. 仓库与分支基线（A-01 / A-02）

- 分支 `master`；HEAD `58bdd988`（merge）。
- Phase 1 C1~C5 在历史中（`3bab0469`…`3ca5ad45`）；**Phase 1 结算后新增两笔**：`e7aad791`（侧栏 DS 组件重构+项目分组）与 `3dc4c61c`（构建修复）——本轮一切导航证据以 HEAD 为准。
- 工作区：仅 `docs/audit/ia-2026-11-16/Phase2-Analysis独立化实施方案-v1.1.md` 未跟踪（本评审产物）；其余干净。
- Phase 1 前置条件：`/project/analysis` 导航占位已存在（`ai-frontend/src/router/navigation.ts:296`，`labelKey: shell.app.side.item.analysis`）；`router.test.ts:80` / `navigation.test.ts:42,51` 已引用。**A-02 = 满足，无 BLOCKED。**

**基线异常登记（K-01）：** `ai-frontend/src/views/project/knowledge/KnowledgeView.vue.js` 与 `.vue.js.map` 被 git 跟踪（`git ls-files` 实证）——构建产物误入库。本轮不清理，登记待处置。

---

# 2. Analysis 页面组件树与依赖（B-01 / B-02 / B-03）

## 2.1 真实组件树

```text
KnowledgeView.vue (ai-frontend/src/views/project/knowledge/KnowledgeView.vue, 999 行)
├── Tabs（三页签 reports|kb|pc；TabKey 默认 "reports"，:45-46）
├── AnalysisReportsPanel.vue（同目录，847 行）── Analysis Reports 页签本体
│   ├── 状态分区列表（analyzing/analyzed/closed/failed 四组，GROUPS :72-77；历史默认隐藏 :49）
│   ├── 行展开详情（detailCache :39；每次展开重拉 :198 注释）
│   ├── KB 确认动作（confirmKb :219-238 → confirmKbRevision）
│   ├── PC 提案批准/驳回（decideProposal :240-261 → decidePcProposal；暂记可互改）
│   ├── D-4 身份综合（runSynthesis :264-278 → synthesizeIdentity；hasAnalyzedReport 门 :125）
│   ├── 关闭报告（doClose :280-299 → closeAnalysisReport；ConfirmationDialog :632-652）
│   └── 深链消费（watch route.query.report :174-187 → 定位行 + openDetail）
├── KB 页签（listKbDocuments / getKbDocument / confirmKbRevision / ingest）
├── PC 页签（listPcObjects / pc 行内展开缓存 :124-128）
└── 影响分析面板（retrievalQuery）
```

## 2.2 组件清单

| 组件 | 数据来源 | 职责 | 可复用 | 耦合 | 证据 |
|---|---|---|---|---|---|
| AnalysisReportsPanel.vue | `@/api/analysis-reports` + `@/api/knowledge`（confirm/decide） | 报告列表+详情+审阅+关闭 一体 | **部分**（整体可迁；内部零 KnowledgeView 依赖，props 仅 `projectId`） | **低** | 仅 import `useRoute`（深链）与 UI 件 |
| KnowledgeView.vue | shell.currentProjectId（:43） | 页签容器 + KB/PC 域 | 迁移报告页签后保留 | 中 | 三页签共享 impactResult |

**B-03 关键结论：AnalysisReportsPanel 与 KB 领域服务无代码级耦合**——它虽然挂在 Knowledge 页下、业务对象借 kb/pc 表存储，但其数据全部来自 analysis-reports API 与通用决策端点；**组件名与业务对象一致，是真正的 Analysis 面板**。迁移成本低（props 只有 `projectId`，深链自带 route watch）。

---

# 3. Report 数据契约（C-01 / C-02 / C-03 / C-04）

## 3.1 C-01 列表 API 完整链

```text
AnalysisReportsPanel.loadLists (:133)
  → listAnalysisReports (api/analysis-reports.ts:89)
    → GET /v1/pm/analysis-reports?project_id=[&repository_id&status]
      → analysis_reports.rs:69 list_reports（project_scope→require_project_visible）
        → pm-application/analysis_report.rs:287（SQL 见下）
```

- 项目过滤：`WHERE r.tenant_id = (SELECT tenant_id FROM core.project WHERE id=$1) AND r.project_id=$1`（service :306-307）——租户从 project 行派生，双约束。
- 权限：`require_project_visible`（support.rs:239；不可见一律 **404 不泄露存在性**）。
- 返回 `{ items: ReportListRow[] }`，无分页（LIMIT 无；行数=项目报告总数）。**分页缺口登记（K-02）**：现契约无分页参数，Phase 2 列表若沿用则继承该形态。
- 状态分区供给：analyzing / analyzed（待审理）/ closed（历史）/ failed；`pending_proposals` / `has_pending_kb` 计数列（:298-303）。

## 3.2 C-02 详情 API 与正文权威

- `GET /v1/pm/analysis-reports/{report_id}?project_id=`（analysis_reports.rs:87）→ `report_detail`（service :320-393）。
- **实现注记（K-03）：** `report_detail` 先拉全项目列表再 `find` 单行（:325-329）——行为正确、复杂度 O(n)；Phase 2 详情页高频访问时建议后端补专用 header 查询（缺口登记，非阻断）。
- **正文权威实证：** KB 修订全文 = `SELECT … kv.content FROM kb.document_revision kv …`（service :334-341）——**DB 直读，无 Git 通道**。PC 修订正文同样 DB 直读（`pc.document_revision.content`，:370-383）。实施方案 REV-2 的现状态警示**成立**：Git 权威 = 目标态 NOT_IMPLEMENTED；四域注册表 Analysis Report 权威标注与代码不符（既有登记维持）。
- DB Revision 与 Git Commit 一致性：现模型不存在 Git 正文引用，故**该问题在现状下不适用**（UNKNOWN→随 Git 权威落地再核）。

## 3.3 C-03 Report Revision 契约

- **无 Report Revision 实体。** 报告行唯一身份 = `integration.repo_analysis_report.id`；版本语义 = `commit_sha`（本次分析 commit）+ 关联 `kb_revision.revision_no`。
- 「重新分析 vs 修订」：现模型中**每次 analyze 受理即建新报告行**（`ensure_for_intake` :121-154，`ON CONFLICT (intake_id) DO NOTHING`）——重新分析=新报告；**不存在同一报告下的修订链**。任务书要求的区分语义 = NOT_IMPLEMENTED（由现模型形态替代）。
- 生命周期写入者清单（全量）：
  | 写入点 | 位置 | 动作 |
  |---|---|---|
  | 受理建行 | `repository/repo.rs`（analyze 链调 `ensure_for_intake`） | INSERT analyzing |
  | 分析门 | `repository/repo.rs:825` | open 报告 → 409 `ReportOpen` |
  | KB 阶段刷新 | `analysis_report.rs:203 mark_kb_stage`（worker 经 `pm-infrastructure/kb_pc_handlers.rs`） | stage_detail |
  | 待审 | `analysis_report.rs:222 mark_analyzed`（同上 worker 侧） | status→analyzed + kb_revision_id/pc_source_id 回填 |
  | 失败 | `analysis_report.rs:257 mark_failed` | status→failed（终态） |
  | 关闭 | `analysis_report.rs:400 close_report` | status→closed + 批量发布/撤回/拒稿 + 锚点固化 |
  - DB 审计登记的「analysis_run 双写入者分叉」：本轮核验范围为 `repo_analysis_report` 生命周期（如上，单一 service 文件承载全部状态 SQL）；`analysis_run` 记账表分叉细节本轮未展开 = **UNKNOWN（沿用既有登记，不重复审计）**。

## 3.4 C-04 Finding / Conclusion / Recommendation

- pm-api 路由层（`analysis_reports.rs`）、application 层（`analysis_report.rs`）、前端类型（`api/analysis-reports.ts`）**均无** Findings / Conclusions / Recommendations 对象。
- 现状替代形态：分析结论性内容 = KB 报告修订正文（Markdown）+ PC 对象修订正文。discovery 域的 finding（`/v1/pm/projects/{pid}/discovery/...`）是**另一领域**（代码发现会话），与报告审阅无关，不得混淆。
- 结论：**NOT_IMPLEMENTED（全链）**。Phase 2 方案 §5.2 内容区 B/C/D 与 AC-04 需按此修订：详情页现可实现的是「报告正文（KB 修订）/ PC 提案集 / PC 修订集 / 关闭处置回执」。

---

# 4. 报告审阅链路与权限（D-01～D-05，CTRL-1 追链）

## 4.1 D-01 前端审阅入口（全量）

| 入口 | 位置 | 动作 |
|---|---|---|
| Knowledge 页 reports 页签 | AnalysisReportsPanel | KB 确认（:219）/ 提案批准驳回（:240）/ 关闭（:280）/ 综合（:264） |
| Review Center report 页签 | ReviewCenterPage.vue:543-555 | **仅跳转**（goReview → :549 深链）；:542 注释「报告条目不在审核中心决策（单一决策源=知识审阅报告页）」 |
| Knowledge 页 kb/pc 页签 | KnowledgeView | KB 确认（:97-119）/ PC 行内展开（只读） |
| 通知中心 | api/notifications.ts:43-47 | **无报告深链**（announcements 映射，`target: ""`） |

## 4.2 D-02 完整调用链（三动作，逐层实证）

**动作一：KB 报告确认（发布草稿修订）**

```text
confirmKb (AnalysisReportsPanel.vue:219，仅 kb_revision.status==='draft' && row.status==='analyzed' 时渲染按钮 :472-475)
  → confirmKbRevision (api/knowledge.ts:64) POST /v1/kb/documents/{d}/revisions/{r}/confirm
    → kb_memory_api.rs:151 confirm_document_revision（scope() 含 require_project_visible）
      → kb_service::document::confirm_revision（draft→published；幂等重确认 200 不改原确认人；三重门：interactive + 真人主体 + 项目成员，AI/Service Key 403 —— handler 文档 :155-157）
```

**动作二：PC 提案批准/驳回（审阅流 v2 核心）**

```text
decideProposal (AnalysisReportsPanel.vue:240；按钮渲染条件 :546-553 = 报告 analyzed 且 status∈{submitted,approved,rejected}——approved 可再驳回、未驳回可再批准 = 决定未冻结可互改)
  → decidePcProposal (api/knowledge.ts:270) POST /v1/pc/proposals/{id}/decision（CAS expected_version_no）
    → knowledge_services.rs:574 p5_decide_proposal
      → guard()（require_project_visible）+ ensure_trusted_context（support.rs:91，api_key 403）
      → pc/proposal.rs:1110 decide_proposal → record_decision (:900)
        → ensure_formal_authorized（pc/mod.rs:152：DB 查 core.principal.kind='human'，AI fail-closed + 治理授权 A3）
        → 报告门（proposal.rs:909-922）：提案关联报告若 closed → 409「decisions are frozen」；open 期 approved/rejected 可互转；无报告关联提案（身份综合）不受门约束
        → FOR UPDATE 行锁 + CAS expected_version_no 失配 409 + 同态幂等重放（:945-965 返回既有审批事实）
        → **approve 不发布**：decide_proposal 返回 published_version: None（:1130）——决定暂记；发布统一发生在 close
```

**动作三：报告关闭（门释放 + 批量发布）**

```text
doClose (AnalysisReportsPanel.vue:280；入口仅 row.status==='analyzed' :611 + ConfirmationDialog 二次确认)
  → closeAnalysisReport (api/analysis-reports.ts:112) POST /v1/pm/analysis-reports/{id}/close
    → analysis_reports.rs:101 close_report（project_scope + caller.audit_actor()）
      → ensure_closable（service :573-580：auth_mode==='interactive'，镜像 KB confirm 人身门）
      → close_report（service :400-564）：事务 + FOR UPDATE 行锁（:409-414，project_id+tenant 派生双校验，不匹配一律 NotFound）
        → 仅 analyzed 可关（:423-427 否则 409）
        → approved 提案逐个 publish_version（幂等键 pc-decision:{id}；CAS 失配计 failed 不阻断 :429-457）
        → draft/submitted 提案 → withdrawn（逐条审计 :459-488）
        → KB 草稿修订 → rejected（:490-514）
        → 报告 closed + 审计事件 analysis_report.closed（:526-542）+ 仓库锚点 last_analyzed_commit_sha 固化（:543-552）
```

**CTRL-1 判定：**
- 审阅语义的后端归属 = **kb 域（confirm）+ pc 域（decision）+ integration 域（D-5 close）三组既有端点；Analysis 无自有审阅决定端点，也无需新建**。
- 通过态 = **B**（UI 可迁至 Analysis 页面承载三动作；后端域归属不动；无需登记「不一致缺口」——因为 P-03 只约束页面归属，后端单一决策源结构本身是既有裁决 D-5/D-6 的产物）。
- **方案 §5.3「必须复用现有 Analysis 审阅 API」须修正为「复用 KB confirm + PC decision + close 三组既有端点」。**

## 4.3 D-03 人工身份与 AI 身份边界

| 端点 | interactive 门 | 真人主体门 | 证据 |
|---|---|---|---|
| close 报告 | ✓（auth_mode） | ×（未查库 kind；interactive 即放行） | analysis_report.rs:573-580 |
| PC 提案决定 | ✓（api_key 403） | ✓（DB 查 kind='human'）+ 治理授权 | support.rs:91 + pc/mod.rs:152-161 |
| KB confirm | ✓（三重门） | ✓ | kb_memory_api.rs:155-157 |
| Baseline decide | ✓（interactive/off） | ✓（DB 查 kind） | support.rs:111-138（SEC-P1-1） |
| Finding verification | handler 仅 `project_in_tenant`（pm_discovery.rs:335） | 服务层按鉴权解析（finding.rs:108 注释；P0 E2E 曾实证 require_human 403）——**本轮未重读服务体，PARTIAL 证据** | pm_discovery.rs:328-352 |

**安全观察（非阻断，登记 Z-01）：** `close` 与 KB confirm 的人身门为 `auth_mode==='interactive'`（凭据模式判定），未像 PC/Baseline 那样**查库核验 principal.kind**。按 SEC-P1-1 口径（「携带真人 JWT 的机器凭据不等于真人」），若 interactive JWT 可被非 human 主体持有则存在理论绕过。现无此类主体配置证据（UNKNOW 级），登记为安全基线任务（ui-ux-security-baseline）的复核项，**不阻断 Phase 2**（迁移不改变该门）。

## 4.4 D-04 审阅状态机（后端实证）

| 当前状态 | 操作 | 操作者 | 下一状态 | 条件/守卫 | 证据 |
|---|---|---|---|---|---|
| report analyzing | （自动）PC 任务完成 | worker | analyzed | 回填 kb_revision_id/pc_source_id | :222-251 |
| report analyzing/analyzed | （自动）任务失败 | worker | failed | 终态不占门 | :257-284 |
| report analyzed | close | interactive 真人 | closed | 三重门+事务+批量处置 | :400-564 |
| report analyzed | analyze 再分析 | 任意已授权 | （拒绝） | 409 ReportOpen | repo.rs:825 |
| proposal submitted | approve/reject | 真人+治理 | approved/rejected（暂记） | CAS+FOR UPDATE；可互改 | proposal.rs:900-980 |
| proposal approved/rejected | 反向决定 | 真人+治理 | 互转 | 报告 open 期 | 同上 :909-922 |
| 提案任意已决 | close 后再决定 | — | （拒绝） | 409 frozen | :915-918 |
| kb revision draft | confirm | 三重门真人 | published | 幂等重确认 | kb_memory_api.rs:151+ |

## 4.5 D-05 并发与失败

- 重复点击：前端 `acting/closing/synthesizing` 单飞守卫（:220,245,282）+ 后端幂等（同态重放 :945-965；pc-decision:{id} 发布幂等键 :448）。
- 并发决定：`FOR UPDATE` 行锁 + CAS `expected_version_no`（失配 409）——真并发恰一成功一 409（与 P1 E2E R91 同模式）。
- 陈旧详情：面板每次展开重拉 + 动作后 `refreshAfterAction`（:196-217），提案状态翻转（submitted→failed）缓存失效率已处理（:198 注释）。
- 审阅成功前端超时：后端幂等保证重放返回既有事实；前端 notice 提示，无静默覆盖路径。
- 事务边界：close 全程单事务（:406-554），发布失败计数不阻断关闭（:449-455）。

---

# 5. Review Center 职责核验（E-01 / E-02）

## 5.1 E-01 审核事项来源

数据源唯一：`GET /v1/pm/projects/{pid}/command-center` → `human_action_summary` 块（api/reviewCenter.ts:44-57）。后端构造：`crates/pm-application/src/command_center/action.rs`。

| kind | 来源 | link | RC 行为 |
|---|---|---|---|
| finding_pending / finding_revision_requested | discovery 会话 | verification 端点 | 就地决策（store :275→confirmDiscoveryFinding） |
| **report_pending** | `repo_analysis_report WHERE status='analyzed'`（action.rs:164-186，LIMIT 50） | `knowledge_report` + params{report_id, project_id}（:186-190） | **仅跳转**（:549） |
| proposal_pending | pc.proposal | pc decision 端点 | 就地决策（store :310→decidePcProposal，与报告面板**同端点**） |
| delivery_pending | D5 审批 | review-approvals 端点 | 就地决策（store :350→decideReviewApproval，409 族语义映射 :274-282） |

优先级序：finding 0 < report 2 < proposal 3 < delivery 4（action.rs:210-218）。

## 5.2 E-02 是否存在重复审阅逻辑

| 检查项 | 结论 |
|---|---|
| 直接修改 Analysis Report 状态 | **无**（report 分支无决策按钮，:542-555） |
| 直接调用 Analysis 审阅 API | 无（close 端点 RC 零调用） |
| 复制领域状态机 | 无 |
| 自己维护第二套审阅状态 | 无（decisionError/busy 为 UI 态） |
| 只展示待处理事项并跳转 | **是**（对 report_pending 成立） |

**注意（方案相关）：** proposal_pending 事项在 RC 就地决策与报告面板决策**共享同一 P5 端点与同一暂记语义**——P-03「审核中心只聚合」对报告事项成立；PC 提案作为独立领域待办在 RC 决策是 D-6 既有设计，Phase 2 **不应**顺带移除 RC 的 proposal 页签（超出范围且无裁决）。

---

# 6. 深链与入口矩阵（F-01 / F-02 / F-03）

## 6.1 F-01 全量搜索结论

`?report=` 相关全仓 **4 文件 + 1 页签导向**（grep 实证，与方案 V1.1 §3.3 清单一致并补全消费细节）：

| # | 角色 | 位置 | 行为 |
|---|---|---|---|
| P1 | 生产方 | ReviewCenterPage.vue:549 | 生成 `/project/knowledge?project_id=…&report={link.params.report_id ?? item.id}` |
| C1 | 消费方（页签） | KnowledgeView.vue:190-193 | report 参数 → activeTab='reports' |
| C2 | 消费方（定位） | AnalysisReportsPanel.vue:174-187 | 按 id 匹配行 → 展开行 + openDetail（列表未就绪时先 loadLists） |
| C3 | 页签导向（无 report 参数） | SettingsProjectRepositoriesView.vue:354-359 goToAnalysisReports | push `/project/knowledge?project_id=…`（落在默认页签 reports） |
| — | 通知/历史/外部入口 | — | **无**（announcements target 为空，notifications.ts:43-47） |

## 6.2 F-02 深链矩阵（迁移目标行为）

| 当前 URL | 现行为 | Phase 2 目标 |
|---|---|---|
| `/project/knowledge?report=X&project_id=P` | reports 页签 + 展开 X | 跳 `/project/analysis/reports/X`（或 analysis 页定位展开） |
| `/project/knowledge?project_id=P`（无参） | 默认 reports 页签 | 裁决：默认页签改 kb；analysis 独立入口承接报告列表 |
| `/project/knowledge?doc=X` | kb 页签 + 文档详情 | 不变 |
| `/project/knowledge?object=X` | pc 页签 + 对象详情 | 不变 |
| RC report_pending 卡 | 跳 knowledge 深链 | 改跳 `/project/analysis/reports/X` |
| 仓库设置「分析报告」 | 跳 knowledge 默认页签 | 改跳 `/project/analysis`（带 project_id） |

## 6.3 F-03 参数冲突

- 同时携带 `report` + `doc`：KnowledgeView :190-196 顺序执行——report 置 reports 页签后 **doc 会再置 kb 页签**（applyDeepLink 内顺序处理），实际表现 = 后执行者覆盖。现网无生产方同时携带两者；Phase 2 迁移时按任务书「不得自行猜测优先级」，登记核验结论：**现状=doc 后执行覆盖 report；建议迁移后拆分路由使该组合不可能出现**。
- 非法/不存在 Report ID：AnalysisReportsPanel :181-182 静默不展开（无错误提示）——登记 UX 缺口（K-04，迁移时在新页处理为显式错误态）。
- 跨项目 Report ID：详情 API `require_project_visible` + `find` 失败 → 404（不泄露）。

---

# 7. 全局搜索与检索契约（G-01 / G-02 / G-03，CTRL-3 前置门）

## 7.1 G-01 检索 API

- 唯一端点 `POST /v1/retrieval/query`（apps/pm-api/src/retrieval.rs:32）。
- evidence 契约（api/knowledge.ts:141-152）：`source_domain`（枚举 KnowledgeDomain）、`evidence_id`、`title`、`content_excerpt`、`authority_status`。
- **`source_domain` 实际取值 = `KB` / `PC` 两域**（retrieval/channels.rs:96,216,326；fusion.rs:35,121,170—— KnowledgeDomain 双域直查）。
- **无 Analysis 域、无 Report ID、无 Revision 引用**（全链 0 命中）。

## 7.2 G-02 前端映射

- 全局搜索接线 = `WorkspaceShell.vue:97-119 searchFn`：retrievalQuery → evidence.map → 每条 `target: "/project/knowledge"` **硬编码**（:115）；`source_domain==="PC"` 仅用于文案区分（:108-110）。

## 7.3 G-03 Analysis 支持结论

**NOT_IMPLEMENTED。** CTRL-3 前置门条件（稳定携带 `source_domain=analysis` + Report ID）**不成立**——检索通道只有 KB/PC 双域。

**处置（按方案 V1.1 §7 P2-07 前置门）：** 整卡转后端契约缺口登记；不实施任何搜索跳转改造；AC-09 以 **B 通过态**（缺口已登记）结项。禁止凭标题/内容猜测领域。

---

# 8. 权限、项目隔离与安全（H-01 / H-02 / H-03）

## 8.1 H-01 前端项目上下文

- 项目单源：`shell.currentProjectId`（KnowledgeView.vue:43；RC currentProjectId() 取 route.query.project_id :47-51）。
- 切项目清理：KnowledgeView 清 kb/pc 详情缓存与影响分析（:385-391）；AnalysisReportsPanel watch projectId 清 detailCache/openMap/notice 并重载（:162-172）。
- 深链兜底：「项目就绪后再放深链」（KnowledgeView :393-394 注释 + applyDeepLink）。
- **无独立第二套项目上下文**（全部复用 shell 机制，符合方案禁止项）。
- 竞态注记（K-05）：loadLists/openDetail 无 AbortController/请求序号守卫，慢响应晚到可覆盖新数据——与全仓既有模式一致，登记为已知模式风险（非本次新增）。

## 8.2 H-02 后端项目权限（Analysis 三端点全覆盖）

| 端点 | 校验链 | 404 语义 |
|---|---|---|
| list | project_scope→require_project_visible（tenant+principal） | 不可见 404 |
| detail | 同上 + 列表内 find | 不存在/跨项目/跨租户统一 404 |
| close | 同上 + 行级 `tenant_id=(SELECT … FROM core.project)` 双校验 + FOR UPDATE | 同上 |

无 body/query actor_id 信任问题：actor 一律取自 `caller.audit_actor()`（analysis_reports.rs:108），前端不可注入。

## 8.3 H-03 负向测试覆盖矩阵

| 场景 | 现有覆盖 | 结论 |
|---|---|---|
| 项目 A 读项目 B 报告 | 后端统一 404 语义（代码实证）；**无专属测试** | 代码✓ 测试✗ |
| 跨项目 close | 同上 | 同上 |
| 非真人 close/decide/confirm | 后端门（§4.3）；P0/P1 E2E 曾覆盖 require_human 403 模式 | 部分 |
| CAS 失配 / already_decided / anchor_drift | pm_reviews 409 族（RC :274-282 消费）+ proposal CAS | 部分 |
| 项目切换旧响应污染 | 无守卫无测试（K-05） | 缺口 |

**测试缺口登记（K-06）：** AnalysisReportsPanel **无专属测试文件**（唯一间接覆盖 = KnowledgeView.test.ts 2 例：「reports tab as default with empty state」:503、「opens report tab via ?report= deep link」:548）。决策互改/关闭确认/综合门/历史显隐均无单测。

---

# 9. 测试与质量基线（I-01 / I-02 / I-03，本轮实际执行）

## 9.1 执行记录

| 检查 | 命令 | 退出码 | 结果 |
|---|---|---|---|
| 定向单测 | `pnpm -s vitest run src/router/ src/components/shell/ src/views/project/knowledge/ src/views/project/review-center/ --reporter=dot` | 0 | **19 文件 148/148 通过**（12.97s） |
| Typecheck | `pnpm typecheck`（含 build:packages + vue-tsc） | 0 | 通过 |
| Prettier | `pnpm lint:prettier` | 1 | **20 文件**（=Phase 1 结算基线，存量红，无新增） |
| i18n hardcode | `pnpm check:i18n`（第二段） | 1 | **39 处**（=基线 39，存量红，无新增） |
| i18n namespace | `node scripts/check-i18n-namespace.js` | 0 | 通过 |

环境：本机 Git Bash / Node v24.18.0 / 2026-11-16。**NOT_RUN 项：** Rust 全量（DATABASE_URL 测试库未准备，本轮只读不启动）、P0/P1 E2E、全量 vitest——如实记 NOT_RUN，不声称通过。

## 9.2 I-01 测试清单（本域）

| 测试文件 | 对象 | 覆盖 | 状态 |
|---|---|---|---|
| KnowledgeView.test.ts | 三页签容器 | 装载/徽标/详情确认/空态/页签切换/?doc=/?report=/?object= 深链/摄入/影响分析（14 用例） | PASS |
| ReviewCenterPage.test.ts | RC 容器 | 页签/URL 同步/跳转 | PASS |
| navigation.test.ts / router.test.ts | 导航与路由 | 含 /project/analysis 占位断言 | PASS |
| （缺）AnalysisReportsPanel.test.ts | 报告面板本体 | 无 | **NOT_IMPLEMENTED（K-06）** |
| （缺）api/analysis-reports 单测 | 契约层 | 无 | 缺口 |

---

# 10. 组件复用与缺口登记（J-01 / J-02）

## 10.1 J-01 复用判定

| 组件 | 处理建议 | 理由 |
|---|---|---|
| AnalysisReportsPanel.vue | **迁移**（挂到 /project/analysis；props 不变） | 与 KnowledgeView 零领域耦合；深链自带 |
| 三页签 Tabs 结构 | 拆分 | reports 迁出后 Knowledge 剩 kb|pc 两页签；默认页签须裁决（建议 kb） |
| ReviewCenterPage :549 | 改导向 | `knowledge_report` link 结构不变，仅目标路径变 |
| SettingsProjectRepositoriesView :354 | 改导向 | 同上 |
| api/analysis-reports.ts | 原样复用 | 契约即目标契约 |

## 10.2 J-02 API 缺口登记（不因 Phase 2 顺手实现）

| 编号 | 缺口 | 影响 | 建议 |
|---|---|---|---|
| GAP-1 | 检索无 Analysis 域/Report ID（CTRL-3 门未过） | P2-07 无法实施 | 转后端缺口；AC-09 走 B 态 |
| GAP-2 | 无分页（list_reports 全量返回） | 报告多的项目列表过长 | 后端补分页（独立卡） |
| GAP-3 | report_detail 经全列表扫描定位 | 详情页高频访问低效 | 后端补专用 header 查询 |
| GAP-4 | Findings/Conclusions/Recommendations 不存在 | 方案 §5.2/AC-04 按现形态改写 | 属四域目标模型演进（§16 序列），不在 Phase 2 |
| GAP-5 | Git 正文权威未落地（DB 直读 kb.content） | REV-2 已声明 | 维持后端演进登记 |
| GAP-6 | 非法 Report ID 深链静默无反馈 | UX | 迁移时在新页做显式错误态 |
| GAP-7 | close/KB confirm 人身门未查库核 principal.kind（Z-01） | 理论绕过面 | 归入 ui-ux-security-baseline 复核 |

---

# 11. 风险登记（J-03 / J-04）

| 风险 | 位置 | 严重度 | 阻断 Phase 2？ |
|---|---|---|---|
| D-4 综合提案挂 analyzed 报告，页签迁走后落点未裁决（REV-1） | AnalysisReportsPanel:124-127 + synthesizeIdentity | 高（功能可达性） | 需裁决后开工 P2-06 |
| reports 为默认页签，迁出改变 Knowledge 无参行为 | KnowledgeView:45-46 | 中 | 裁决后实施 |
| proposal 决策双入口（RC + 报告面板）并存 | store:310 / panel:240 | 低（同端点同语义） | 不处理（D-6 既有设计） |
| 构建产物入库（K-01） | KnowledgeView.vue.js(.map) | 低（仓库卫生） | 否，独立清理卡 |
| 竞态无守卫（K-05） | loadLists/openDetail | 低 | 否 |
| cancelled 状态在 API 类型存在但 UI GROUPS 无该组 | analysis-reports.ts:16 vs panel:72-77 | 低 | 迁移时对齐（终态不占门，展示策略裁决） |

---

# 12. 任务卡调整建议（J-05）

| 卡 | 调整 | 依据 |
|---|---|---|
| P2-01 | **可立即开工** | 占位已在（navigation.ts:296） |
| P2-02 | 按现契约实施；无分页继承现状（GAP-2 独立登记） | §3.1 |
| P2-03 | **验收字段改写**：详情=报告头+KB 修订全文+提案集+PC 修订集；AC-04 原文（F/C/R）作废，改「三类产物正确展示」；E2E-09 改写为「多报告（多 commit）列表与详情同源」——现模型天然同源，断言从简 | §3.3/§3.4 |
| P2-04 | 方案 §5.3 修正为复用三组既有端点（§4.2）；v2 语义（暂记/互改/冻结/close 批发）API 层已齐备，UI 随迁即可；RC 决策入口迁移工作量小于原估（现只有跳转） | CTRL-1 通过态 B |
| P2-05 | :549 改导向 + link 结构不变 | §6.1 |
| P2-06 | 维持；前置=两项裁决（默认页签、D-4 落点） | §6.2/§11 |
| P2-07 | **整卡转缺口登记（GAP-1），不实施** | §7.3 |
| P2-08 | 维持 | — |

---

# 13. Phase 2 开工判定（§15）

## **READY WITH CONDITIONS**

核心链路（列表/详情/三动作审阅/深链/聚合跳转）全部有源码级证据且业务可用、安全门实证；条件如下：

1. **裁决两项**（P2-06 前置）：Knowledge 默认页签新值（建议 kb）；D-4 综合提案迁移后落点（随报告迁 / 留知识页改挂链接）。
2. **方案 V1.1 三处修正后冻结**：§5.2 内容区按「三类产物」改写；§5.3「Analysis 审阅 API」改「三组既有端点」；AC-04 作废改写。
3. **P2-07 转缺口**（GAP-1），AC-09 按 B 态结项。
4. **无未关闭的阻断性安全风险**：Z-01（interactive 门未查库 kind）与 K-05（竞态）均已登记，属存量基线，迁移不使其恶化；按任务书 §0.2 本轮不修。

解除条件：上述 1~3 完成即可冻结 Phase 2 开发任务包并开工 P2-01。

---

# 14. 证据索引（关键结论 → 源码）

| 结论 | 证据 |
|---|---|
| 3 端点路由 | apps/pm-api/src/analysis_reports.rs:25-33；routes.rs:284 |
| 正文 DB 直读 | crates/pm-application/src/analysis_report.rs:331-345（kv.content） |
| 状态机 | analysis_report.rs:121/203/222/257/400 |
| close 三重处置+锚点 | analysis_report.rs:400-564 |
| interactive 门（close） | analysis_report.rs:573-580 |
| PC 决定暂记/CAS/冻结 | crates/pm-application/src/pc/proposal.rs:900-980,1110-1131；pc/mod.rs:152-161 |
| KB confirm 三重门 | apps/pm-api/src/kb_memory_api.rs:151-176 |
| RC 报告只跳转 | ai-frontend/src/views/project/review-center/ReviewCenterPage.vue:542-555 |
| 深链生产/消费 | ReviewCenterPage.vue:549；KnowledgeView.vue:190-193；AnalysisReportsPanel.vue:174-187 |
| 设置页导向 | SettingsProjectRepositoriesView.vue:354-359,948-952 |
| 通知无深链 | ai-frontend/src/api/notifications.ts:43-47 |
| 检索双域+硬编码 | retrieval.rs:32；channels.rs:96,216,326；WorkspaceShell.vue:97-119（:115 target） |
| report_pending 聚合 | crates/pm-application/src/command_center/action.rs:164-218 |
| 项目隔离守卫 | apps/pm-api/src/support.rs:91,111,239 |
| 门禁实测 | §9.1 执行记录（本轮真实输出） |
