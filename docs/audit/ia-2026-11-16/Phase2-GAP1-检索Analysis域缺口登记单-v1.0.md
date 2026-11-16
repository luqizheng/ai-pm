# Phase 2 GAP-1 检索 Analysis 域缺口登记单 V1.0

**状态：** 已登记（P2-07 暂缓产物，2026-11-16）
**来源：** 《Phase 2 实施任务包 V1.0》P2-07 /《实施方案 V1.2》§10 GAP-1 /《源码核验报告 V1.0》§7（CTRL-3 前置门不成立）
**性质：** 只登记，不实现。本单是后端契约缺口，前端不得以任何猜测性跳转绕过。
**验收口径：** 方案 V1.2 AC-09 以 **B 态结项**（缺口已登记 + 未做任何猜测性跳转）。

---

## 1. 现状证据（实施日 2026-11-16 重新 grep 复核）

| # | 证据 | 位置 |
|---|---|---|
| 1 | 检索唯一端点 `POST /v1/retrieval/query`；evidence 契约 `source_domain: KnowledgeDomain` | `apps/pm-api/src/retrieval.rs`；`crates/kb-pc-contracts/src/retrieval.rs:544` |
| 2 | `KnowledgeDomain` 枚举 = **PC / KB / REQUIREMENT 三域**（无 Analysis 域值） | `crates/kb-pc-contracts/src/retrieval.rs:449-454` |
| 3 | 通道产出仅 Pc/Kb 两类 `source_domain`（REQUIREMENT 未在检索通道产出） | `crates/pm-application/src/retrieval/channels.rs:96,216,326` |
| 4 | evidence 全链无 Report ID / 报告引用字段 | `crates/kb-pc-contracts/src/retrieval.rs`（全文 grep 0 命中） |
| 5 | 前端全局搜索接线：KB/PC 证据 target 硬编码 `/project/knowledge` | `ai-frontend/src/components/shell/WorkspaceShell.vue:115` |
| 6 | 前端 evidence 类型 `source_domain: "PC" \| "KB" \| "REQUIREMENT"` | `ai-frontend/src/api/knowledge.ts:127` |

**与核验报告 V1.0 §7.1 的差异记录：** 核验快照（master @ 58bdd988）记录为 KB/PC 双域；
实施日复核发现契约已演进为 **PC/KB/REQUIREMENT 三域**（`REQUIREMENT` 域由后续提交引入，
检索通道未见产出）。差异不改变结论：**仍无 Analysis 域值、无 Report ID**，CTRL-3 门条件
依旧不成立。

## 2. 缺口定义（后端需提供，缺一不可）

1. **evidence 级 `source_domain` 扩 Analysis 域值**（`KnowledgeDomain` 增列，或独立
   Analysis 引用契约——由后端专项定版）。
2. **稳定 `report_id`**（或等价引用）：Analysis 报告的唯一身份 =
   `integration.repo_analysis_report.id`，evidence 须可直接携带该 ID。
3. **项目归属**：evidence 携带 `project_id`（或可由消费方解析的项目上下文），
   保证跨项目搜索结果可路由且项目隔离语义不被绕过。

## 3. 后端契约就绪后的前端承接（预告，不在本阶段实施）

`WorkspaceShell.vue` 的 `searchFn` 映射表按 `source_domain` 分流：

| source_domain | target |
|---|---|
| KB | `/project/knowledge`（现状不变） |
| PC | `/project/knowledge`（现状不变） |
| REQUIREMENT | 按该域既有路由定版 |
| **Analysis（新增）** | **`/project/analysis/reports/{report_id}`** |

## 4. 禁止事项（登记即生效）

- **禁止任何前端猜测性跳转**：不得凭标题/内容摘要推断「这条证据像报告」而改写 target。
- 在后端契约就绪前，全局搜索不出现 Analysis 结果（现状即如此，无回归）。
- 本缺口不阻塞 Phase 2 任何 AC（AC-09 走 B 态）；承接实施归后端契约卡 + 独立前端卡。

## 5. 关联登记

| 关联项 | 关系 |
|---|---|
| 方案 V1.2 §10 GAP-1 | 本单为其承接载体 |
| 任务包 P2-07 | 本单为该卡唯一交付物（不写代码） |
| 核验报告 §7.3 CTRL-3 门 | 门条件不成立的实证基线 |
| AC-09 | B 态结项依据 |
