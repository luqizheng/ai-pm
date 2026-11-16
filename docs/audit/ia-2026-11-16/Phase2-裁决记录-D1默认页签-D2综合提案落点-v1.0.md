# Phase 2 裁决记录：D-1 默认页签 · D-2 综合提案落点 V1.0

**状态：** 已裁决（2026-11-16，用户裁决）
**依据：** 《Phase 2 Analysis 独立化源码核验报告 V1.0》§11 / §13 条件 1
**生效范围：** 方案 V1.2 §5/§6、任务包 P2-06；本记录落盘即关闭核验报告 §13 解除条件之第 1 项

---

## D-1：Knowledge 默认页签

**裁决：选 A —— 默认显示 KB 页签。**

| 项 | 裁决前 | 裁决后 |
|---|---|---|
| `/project/knowledge` 无参默认 | reports 页签（KnowledgeView.vue:45-46 `TabKey` 默认 `"reports"`） | **KB 页签** |
| 页签构成 | reports \| kb \| pc | **kb \| pc**（reports 迁出） |
| 报告列表与详情 | knowledge 页内 | `/project/analysis` 承担 |
| `?doc=` 深链 | kb 文档详情 | 不变 |
| `?object=` 深链 | pc 对象详情 | 不变 |
| `?report=` 深链 | reports 页签定位 | 兼容跳转 Analysis（见方案 §6 兼容表） |

**理由：** 报告已独立成为 Analysis 的页面职责，Knowledge 不应继续默认进入一个已经迁走的领域。

---

## D-2：D-4 身份综合提案落点

**裁决：选 A —— 跟随 Analysis 报告迁移，保留在 Analysis 报告详情中。**

理由：报告详情已包含 PC 提案集，D-4 综合提案同属当前报告处理链；若只迁移 KB 修订与 PC 提案而把 D-4 留在旧 Knowledge 页，会造成同一报告的审阅操作分散在两个入口。

**迁移后报告详情结构（六节）：**

1. 报告基本信息
2. 分析报告正文（当前 KB 修订）
3. PC 提案
4. PC 修订内容
5. D-4 身份综合
6. 报告关闭与处置结果

**边界（不得逾越）：** 这是**前端承载位置的调整**，不代表把 D-4 提案所有权转移给 Analysis。原有领域端点（`POST /v1/pm/projects/{pid}/cognition/identity-synthesis`，api/analysis-reports.ts:120-133）、权限和审批语义保持不变。现状证据：AnalysisReportsPanel.vue:124-127（hasAnalyzedReport 门）、:263-278（runSynthesis）、:340-352（综合按钮区）。

---

## 附：裁决后即时效果

- 核验报告 §13「READY WITH CONDITIONS」解除条件第 1 项（两项裁决）**关闭**。
- 剩余解除条件：方案 V1.2 修正冻结（本次随 V1.2 落盘完成）与 P2-07 缺口登记（任务包内承接）。
