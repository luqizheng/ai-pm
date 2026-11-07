# S03-ACP 档案索引（Agent Communication Protocol / 人工介入与执行连续性）

> 本目录归拢 S03-ACP 全部在档文件。历史稿 V1.0~V1.2 按 2026-11-06 用户裁决**不再取回归档**（仅 Revision Log + §22.3 留档）。

## 一、阅读顺序（理解 Task-Based Continuation 与通信面）

| 序 | 文件 | 定位 |
| --- | --- | --- |
| 0 | `smark-pm-s03-agent-interaction-v0.1.md` | 上层能力：Agent Interaction & Human-in-the-Loop V0.1（讨论版）。**ACP 之外**的自动回答/分流层（N-5：其职责发生在 `interaction.required` 发出之前）；Q1~Q5 待裁 |
| 1 | `smark-pm-s03-acp-task-continuation-v1.3.md` | V1.3 正式方案（Task-Based Continuation 转向；已被 V1.3.1 取代，保留供追溯） |
| 2 | `smark-pm-s03-acp-task-continuation-v1.3-review-v1.md` | V1.3 独立评审（有条件通过；R-1~R-14、Q-1~Q-7、事实清单 F1~F11） |
| 3 | `smark-pm-s03-acp-task-continuation-v1.3.1.md` | **★ 冻结设计基线（2026-11-07 用户决议）**：D-01~D-07 生效裁决、§23.3 实施设计闭合清单 |
| 4 | `smark-pm-s03-acp-task-continuation-v1.3.1-review-v1.md` | V1.3.1 独立评审（**通过**；N-1/N-2 → §23.3，N-3~N-6 文本修订） |
| 5 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1.md` | **★ 通信面切片实施设计 V1.1**：闭合 N-1/N-2 + 命令查询面；R-ACP-1~9 已裁（2026-11-07）；**独立复审未通过（见 6），待修订 V1.2** |
| 6 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-review-v1.md` | 切片 V1.1 独立复审（2026-11-07，代码基线 e87efd5）：**不通过**——P0×4（B3-2 事实错位 / 同 Run 租约重取缺失 / lease_owner 非身份·Runner 与 Model A 不可区分 / awaiting_decision 出路挂在 CONSUMED）+ P1×6（含迁移号顺移 0046~0047）+ P2×6；R-ACP-1~9 不重开；V1.2 修订清单 R-1~R-10 + 新增验收 ACP-446~451 |

## 二、状态一览

- **基线**：V1.3.1 冻结 ✅（2026-11-07）
- **实施设计**：ACP-01/02/08 通信面 V1.1 复审**不通过** ❌（2026-11-07）→ 待 V1.2 修订后再复审；**复审通过前不进代码**。ACP-03~07/09 未启动（见切片稿稿头"不覆盖"声明）
- **迁移号**：R-ACP-6 的 0045 已被 DEF-01 顺移占用 → ACP 批次 0046~0047（落地前再核 `ls migrations/`）
- **下一批切片**：ACP-03（REST/UI 回复入口权限模型）→ ACP-04/05/06/07 → ACP-09（Model A 回归）

## 三、关联文件（不在本目录，指针）

| 主题 | 位置 |
| --- | --- |
| V0.5 MVP 范围基线（Runner 客户端化、WP 分解、V-05-1~5 裁决） | `docs/project-ai-team-v2/ai-pm-v05-mvp-scope-baseline-v1.md` |
| V2 架构基线（§7.2 Worker 定位、V2.0-B/C Client Registry/授权） | `docs/project-ai-team-v2/ai-pm-v2-architecture-baseline-v1.md` |
| V2 代码对照核验（禁重建区 §四） | `docs/project-ai-team-v2/ai-pm-v2-code-alignment-verification-v1.md` |
| S03-00 审计执行基线与盘点 | `docs/s03/`（S03-00-* 十份） |
| S03-00 技术设计/ADR/域资产盘点 | `docs/s03-00-technical-design-adr-domain-asset-inventory-v1.md` |
| S03-01 分析设计稿 | `docs/ui/smark-pm-s03-01-analysis-design-technical-decisions-v1.md` |
