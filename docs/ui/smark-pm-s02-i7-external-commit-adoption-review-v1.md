# S02 · S02-I7 外部代码变更接管设计稿 V1.0 评审与代码核验 V1.0

> 评审对象：专家《S02-I7 · External Commit Adoption & Requirement Attribution V1.0》（2026-11-04，**新增第七切片**）。
> 输入基线：S02-01 V1.1（含 2026-11-04 勘误）· S02-02 报告 · S02-I6-A V1.1/V1.2 + 两份核验 · S02-04 评审 R-7~R-19 · 供稿核验（Q-18~21）。
> 核验基线：worktree `ai-pm-s01` @ `feat/s01-implementation`（含 0031/0032/0033），2026-11-04。
> 版本：V1.0　状态：已交付，待专家回应

---

## §0. 总评

方向**成立且必要**：反向入口（代码先变、治理后至）填补了「非任务驱动变更脱离需求体系」的真实缺口。P1~P8 原则、证据四分类（与 finding `kind` 对齐）、双链不混淆（§7.6）、幂等 `(repository_id, commit_sha)`、场景 A~G 验收——质量高，无结构性反对。

**但有一处 P0 级表述越权、两处事实过期（其一源头在我方 S02-01，已勘误）、一处实施路径缺失**。逐条如下。

## §1. 代码核验结果（附录 A 可即答项）

| # | 核验项 | 结论 | 证据 |
|---|---|---|---|
| 1 | Repository 模型与受管分支 | `integration.project_repository` 已有 **`default_branch`**（受管分支定义有落点）+ `head_commit_sha` + `last_verified_at` + `status` | 实库 information_schema；0008+0031 |
| 2 | Task Run Commit 登记与唯一性 | `delivery.task_repository_commit`：`UNIQUE(task_run_id, repository_id)`——**每 Run×仓一 Commit（Run HEAD 模型）**，`commit_sha` 平台自 Run 工作区 git HEAD 读取（不取执行端声明）；`commit_author` 为自由文本非 Principal ✓。**无全局 `(repository_id, commit_sha)` 唯一**——I7 的排除查询按此查找可行 | `0022_delivery_manifest.sql:51-84` |
| 3 | Push/PR Webhook 基础设施 | **不存在**（无 Git provider webhook 接收器/签名验证；仅通用 `inbound_event` 通道可承载 `source='webhook'` 事件）。**但**：`head_commit_sha`+`last_verified_at` 构成现成的**同步巡检差分基线**——见 R-I7-2 | grep 全仓；`platform_schema.rs:101` |
| 4 | discovery_session 承接可行性 | 0031 已做 `project_id` 结构对齐（session/finding 等）；会话复用**结构上可行**，边界（会话类型区分初始化发现 vs 变更接管）留待 I3 统一 | `0031:15-26` |
| 5 | Finding 的 Commit 级追溯 | **本稿 §7.3 已过期**：① finding 已有 `project_id`（`0031:112-119`，NOT NULL+组合唯一）；② Commit 级证据通道**已存在**——`discovery_finding_evidence`（`0031:128-144`）含 `repository_id`+`commit_sha`+`evidence_type='commit'`+`import_id/attempt_no`。**仍缺**的是 Commit 治理登记实体（唯一 `(repo,sha)` 状态机载体）——正是 `repository_change_intake` 的定位。**勘误责任在我方 S02-01 §5.4/§8.1**（只查 0025 漏 0031，已就地勘误，I7 继承了该过期事实） | 同左 |
| 6 | Question/Answer 关系 | 存在（0025 `discovery_question/answer`，0031 对齐 project_id）；`finding_id` 锚定（0025 实施补列） | 0025/0031 |
| 7 | discovery_proposal ↔ document_proposal 映射 | 存在提案关联（0025）；与 `ai.document_proposal` 的真实映射边界留 I3 核验（本稿 §7.2 已正确声明"待核验"） | 0025 |
| 8~12 | 原始需求实体 B / I6-A 边界 / Worker 幂等 / 多 Agent 协调 / 迁移登记 | 8=**见 R-I7-1（未定）**；9=I6-A V1.2 已定 ✓；10/11=现成（outbox/lease/知识写入协调器）；12=0034 起 ✓ 本稿口径正确 | 前序交付物 |

## §2. 评审意见

**R-I7-1 · P0 表述越权：「方案 B 已确定」不成立（§4.2）**
本稿称"遵循 S02 **已确定的**原始需求实体定位方案 B"。事实：方案 B（五对象模型）是外部供稿，定位为 **S02-04 输入、不单独占号**；其对象命名（Q-18 `source.source_input`）、`source_artifact_id` 语义（Q-20）、`storage_ref`（Q-21）、来源继承语义——**全部仍在 S02-04 V1.2 待裁台账中未裁**。I7 §4.2 应改为「依据外部供稿方案 B（待 S02-04 V1.2 裁决台账确认）」，并把 Q-18/20/21+来源继承列为 I7 的**前置裁决**——否则 V1.2 定稿时会出现 I7 引用未裁决对象的地基悬空。

**R-I7-2 · P1 实施路径缺失：webhook 之外有更低的起步路径（§2.1）**
Webhook 接收器零存在、是 I7 最大的新建成本。但 `project_repository.head_commit_sha` + `last_verified_at` + **既有同步验证环**（S01-I3/I6 已建）构成现成差分基线：**V1 触发可走「同步巡检差分」（定期/验证时比对 head 基线，落新增 Commit 入 intake），webhook 作为增强**。建议 §2.1 事件表补此路径并标注其为 V1 推荐——不影响协议设计（§2.4 幂等按事件重投设计本就同时覆盖巡检重扫）。

**R-I7-3 · P1 事实过期修正（§7.3）**
按 §1#5 改写：finding 已有 `project_id`；Commit 级证据通道已存在（evidence 表）。缺口收窄为「**无 Commit 治理登记实体**」——这反而**强化**了 Q-I7-01 新增 `repository_change_intake` 的必要性（登记实体+状态机），并给出前向增强：Intake 落地后 `discovery_finding_evidence.commit_sha` 可加 FK 收口（文本引用→实体引用）。

**R-I7-4 · P2 排除判据精度（§1.2）**
`task_repository_commit` 无全局 `(repo, sha)` 唯一（它是 Run×仓 HEAD 记录），排除查询 `EXISTS(... WHERE repository_id=? AND commit_sha=?)` 成立 ✓；但注意本稿 §1.2 第 1 条的表述"已通过 task_repository_commit 完成治理"应精确为"该 sha **等于某 Run 登记的 HEAD sha**"——一个 Run 内多个 commit 的中间提交不在登记表（HEAD 模型），**中间 commit 仍应进入 intake**（它们正是"非任务治理范围"的典型对象）。建议补此一句，避免实施者把整条 Push 都算作已治理。

**R-I7-5 · P2 切片结构并入**
I7 超出 S02-05 启动基线的 I1~I6：① todo §10.9 切片表补 **S02-I7（P1，依赖 I3+I6-A+I1/I2 治理链路就绪）**；② **S02-04 V1.2 修订（REV-01）须把 I7 并入切片表与依赖图**——这是 R-9 修复的增量输入，建议随 V1.2 一并收编，避免基线与实施计划再次脱节。

**R-I7-6 · P2 长挂起风险（§6.1B/§10）**
`GOVERNANCE_PENDING` 的关闭判据（提案仍待正式确认/发布时不得关闭）叠加 S02-I5 验收双链后，intake 可能长期悬挂。§11 可观测性已提积压查询——落地归 I5/I6-B，建议在依赖表注明「intake 积压视图」为 I6-B 交付项。

**R-I7-7 · 小项**
① §8.3 不可信输入原则很好，补一句与 I6-A 的衔接：intake 分析属 MCP 可出的**分析类**工具面，确认类工具仍按 A* 不出；② Q-I7-04（Parent SHA vs 分支基准）建议直接采纳"两者均保存"，与 evidence 表 `base_commit_sha` 候选字段呼应；③ 状态机大小写（`CLOSED_*` 大写下划线 vs 修订表小写下划线）留库设计统一 ✓ 本稿已自知。

## §3. 无异议确认

P1~P8 原则 · 证据四分类（§3.5）· 澄清协议与对象选择（§5，Author≠Approver ✓ 与 I6-A 一致）· 双终态+非正常终态+重开（§6）· 双链不混淆（§7.6）· 权限与审计（§8，继承 I6-A V1.2 ✓）· 场景 A~G（§9，含"Message 不规范不强制提问"的正确取向）· 迁移 0034 起/不占号/一变更一主责（§7.7 ✓）。

## §4. 结论

修正 R-I7-1（表述越权）与 R-I7-3（事实过期）后，本稿可作为 **S02-05 的第七切片候选基线**；R-I7-2 的巡检起步路径建议随 V1.1 修订一并采纳。**我方 S02-01 V1.1 已就地勘误**（§5.4 project_id 条目，责任在本稿漏查 0031）。
