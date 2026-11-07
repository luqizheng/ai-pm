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
| 5 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1.md` | 通信面切片实施设计 V1.1（已被 V1.2 取代，保留供追溯）：闭合 N-1/N-2 + 命令查询面；R-ACP-1~9 已裁；独立复审判不通过（见 6） |
| 5b | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2.md` | **★ 通信面切片实施设计 V1.2（2026-11-07，本会话起草）**：收编独立复审全部 R-1~R-11 + P2×6，逐项可核（修订记录含对照）。核心增量：C3 事实改正 + 情形 5 真实路径化（R-ACP-14 已裁=接受）/ §3.6 同主体续领（R-ACP-16 已裁=方案 a：发现面 + pm_reacquire_lease + 命令改路由 + reacquired_generation）/ R-4 执行恢复移入 acked 事务 / R-3 execution_mode 准入（ACP-448）/ R-7 选 b（Process 事实=receipt 权威）/ R-ACP-15 基线偏离登记（新裁决项编号 14~16=复审建议 10~12 因 ACP-03 稿占用顺移） / 代码基线更新至 e1feb5f（§2 事实表改写，C9 已实现化）/ 实现收敛项五条清单（§10.5）。**专家三轮复审全部收编（2026-11-07）**：第 1 块活风险全批准+ACP-C1~C4（5c）；第 2 块四维持两修正（V-05-1 硬前置/pm_discover_runs/previous_chain_id，5d）；第 3 块排期=V-05-1 含撤销最小集立即插队、0048/0049 拆分、收敛批两层、ACP-09 两阶段、ACP-04·05 联合设计、task_repository P0 独立、双主线共享基础设施（5e + 排期落 project-ai-team-v2/roadmap）；**本稿进入收敛实施**（§10.5 两层清单） |
| 5c | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-1.md` | **专家复审意见第 1 块原文 + 收编对照（2026-11-07）**：ACP-448/450-451/452-453 方向批准；关键修正=terminate ACKED 从续领前置改判 New Session 启动前置（ACP-C4，§3.6 五步时序）；孤儿 terminate 定向审计禁删；迟到/重复 consumed 不改变恢复结果；改路由≠重建命令/claim_count 不重置；E2E 必测四场景（ACP-454/455 新增） |
| 5d | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-2.md` | **专家复审第 2 块原文 + 收编对照（2026-11-07）**：四维持（R-7 选b+receipt 五点补强/R-16 不持久化 token+Client 身份持久化+退避告警停止/T3 严格不顺延+UI 快捷入口进 ACP-07/Chain 多独立链）两修正（V-05-1=client_name 登记制+生产禁令+R-16 启用硬前置；推翻双态 Poll=独立工具 pm_discover_runs 十字段分页无副作用）；链加 previous_chain_id（0048）；收敛后重点验证 6 条；总评=实施设计收敛、不改总体架构 |
| 5e | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-3.md` | **专家复审第 3 块原文 + 收编对照（2026-11-07，排期依赖）**：五阶段关键路径+WP-A~D 立即启动（ACP Correctness/Client Identity/Repository Integrity/ACP-04·05 联合设计）；V-05-1=平台级 Client Identity Foundation（最小集含撤销+权限字典，撤销影响全部工具面）；0048 ACP 收敛/0049 Registry 拆分+回填三分类（历史兼容≠身份可信）；ACP-09 贯穿式两阶段；task_repository P0 独立六约束；V2 七待裁分类（调度入口升硬前置）；双主线共享基础设施；排期全景=docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md |
| 6 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-review-v1.md` | **★ 独立复审 V1.0（2026-11-07，代码基线 e87efd5）：不通过**——P0×4（B3-2 事实错位 / 同 Run 租约重取缺失 / lease_owner 非身份·Runner 与 Model A 不可区分 / awaiting_decision 出路挂在 CONSUMED）+ P1×6（含迁移号顺移 0046~0047）+ P2×6；R-ACP-1~9 不重开；V1.2 修订清单 R-1~R-10 + **R-11（2026-11-07 对账新增：等待窗口执行端离线恢复死锁——e1feb5f 对账确认 P0-1/P0-3 闭合、P0-2/P0-4 主路径闭合后的统一残留）** + 新增验收 ACP-446~451（R-11 另需 ACP-452~453）。**本结论为设计修订的治理依据** |
| 6b | `smark-pm-s03-acp-slice-design-acp01-02-08-review-v1.md` | 实现方自查评审（同日）：判通过 + 实现对照（84c67e9/1234162）+ F-1[P1]——**与独立复审结论冲突，以独立复审（6）为准**；两份结论的分歧点全部入 V1.2 修订范围 |
| 7 | `smark-pm-s03-acp-implementation-audit-v1.md` | **实现对照审计 V1.0（2026-11-07）**：F-6~F-13 续编——F-7[P1] 权限码无签发面（真实栈 403）、F-8[P1] 取消联动缺失；处置顺序见 §3 |
| 8a | `smark-pm-s03-acp-e2e-report-v1.md` | **真实栈 E2E 报告 V1.1（PASS）**：.10 平台 + .59 Runner 全故事线（假 Agent + opencode 真 CLI 两阶段）+ F-7 双态 + E2E 修复三处 + F-14/F-15 |
| 8 | `smark-pm-s03-acp-slice-design-acp03-v1.md` | **ACP-03 实施设计 V1.0（已裁决+已实施）**：REST 回复入口 + §8.3 六项检查操作化 + interaction_response（0047）+ 同事务 continue 下发；R-ACP-10~13 已裁；实施 = 服务层 + REST 路由 + ACP-446~456 测试（17/17 绿，远程库 192.168.2.10/aipm_test） |
| 8b | `smark-pm-s03-acp-slice-design-acp04-05-joint-v1.md` | **★ ACP-04/05 联合数据契约 V1.0（2026-11-07，WP-D 交付物）**：专家五问逐题裁决提案（Delivery 引 Run+命令不含 Session 外键 / generation 表带列工具不加参 / Delivery 不参与 Task 完成 / ACP-05 只读消费 04 + 单列回填 / 不另建 Session 身份——Session 三件定案）；0050 delivery 表（基线 §12.3 字段集+UNIQUE(response_id) 一回复一账）+ 0051 continuation 表（UNIQUE(run_id,generation)；状态机 open→started→consumed/closed/frozen 与 R-4 acked 同事务推进）；I-1/I-2/I-3 接口契约 + 六点挂账清偿表；验收 ACP-470~477；实施顺序=本契约→0050→0051→06/07 |

## 二、状态一览

- **基线**：V1.3.1 冻结 ✅（2026-11-07）
- **实施设计**：ACP-01/02/08 通信面 **V1.2 已出（2026-11-07，见 5b）**——收编独立复审 R-1~R-11 + P2×6（R-ACP-16 已裁=方案 a 续领；R-ACP-14 已裁=接受），**待独立复审**；复评通过前不作为对外基线。**治理注记**：实施已按用户 2026-11-07"开工"授权先行并真实栈 E2E PASS（见 8a）——代码在库；实现欠账三处（R-4 acked 转 running / R-3 execution_mode 准入 / R-11 发现面+续领）已显式登记为收敛项（5b §10.5）。ACP-03 已实施（见 8）；ACP-04~07/09 未启动
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
