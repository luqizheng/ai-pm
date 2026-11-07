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
| 5 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1.md` | **★ 通信面切片实施设计 V1.1**：闭合 N-1/N-2 + 命令查询面；R-ACP-1~9 已裁（2026-11-07）。**独立复审判不通过 → V1.2 修订中（P0×4，见 6）**；实施已按用户授权先行（E2E PASS，见 8a），V1.2 须吸收 P0 修订 |
| 6 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-review-v1.md` | **★ 独立复审 V1.0（2026-11-07，代码基线 e87efd5）：不通过**——P0×4（B3-2 事实错位 / 同 Run 租约重取缺失 / lease_owner 非身份·Runner 与 Model A 不可区分 / awaiting_decision 出路挂在 CONSUMED）+ P1×6（含迁移号顺移 0046~0047）+ P2×6；R-ACP-1~9 不重开；V1.2 修订清单 R-1~R-10 + **R-11（2026-11-07 对账新增：等待窗口执行端离线恢复死锁——e1feb5f 对账确认 P0-1/P0-3 闭合、P0-2/P0-4 主路径闭合后的统一残留）** + 新增验收 ACP-446~451（R-11 另需 ACP-452~453）。**本结论为设计修订的治理依据** |
| 6b | `smark-pm-s03-acp-slice-design-acp01-02-08-review-v1.md` | 实现方自查评审（同日）：判通过 + 实现对照（84c67e9/1234162）+ F-1[P1]——**与独立复审结论冲突，以独立复审（6）为准**；两份结论的分歧点全部入 V1.2 修订范围 |
| 7 | `smark-pm-s03-acp-implementation-audit-v1.md` | **实现对照审计 V1.0（2026-11-07）**：F-6~F-13 续编——F-7[P1] 权限码无签发面（真实栈 403）、F-8[P1] 取消联动缺失；处置顺序见 §3 |
| 8a | `smark-pm-s03-acp-e2e-report-v1.md` | **真实栈 E2E 报告 V1.1（PASS）**：.10 平台 + .59 Runner 全故事线（假 Agent + opencode 真 CLI 两阶段）+ F-7 双态 + E2E 修复三处 + F-14/F-15 |
| 8 | `smark-pm-s03-acp-slice-design-acp03-v1.md` | **ACP-03 实施设计 V1.0（已裁决+已实施）**：REST 回复入口 + §8.3 六项检查操作化 + interaction_response（0047）+ 同事务 continue 下发；R-ACP-10~13 已裁；实施 = 服务层 + REST 路由 + ACP-446~456 测试（17/17 绿，远程库 192.168.2.10/aipm_test） |

## 二、状态一览

- **基线**：V1.3.1 冻结 ✅（2026-11-07）
- **实施设计**：ACP-01/02/08 通信面 V1.1 独立复审**不通过** ❌（2026-11-07）→ V1.2 修订清单 R-1~R-11（P0×4 见 6；R-11=对账新增的离线恢复死锁必答题）。**治理注记**：实施已按用户 2026-11-07"开工"授权先行并真实栈 E2E PASS（见 8a）——代码在库；V1.2 须吸收 P0×4 修订并复评，复评通过前该设计不作为对外基线。ACP-03 已实施（见 8）；ACP-04~07/09 未启动
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
