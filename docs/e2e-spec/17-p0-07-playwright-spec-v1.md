# AI-PM · P0-07 知识学习、外部 Commit 采纳与统一知识协调 · Playwright E2E 规格 V1.0

> **性质**：P0-07 套件的**可执行**自动化规格（8 条案例：P0-049 ~ P0-056）。
> **上位文件**：`docs/e2e-spec/10-p0-test-specification-v1.md`（编号裁决 **D32**：原「权限隔离与安全」套件撤销独立，内容已分散承接；本套件 049~056，总数 34→**38**）。
> **边界红线（贯穿全部案例）**：AI 学习到的信息 ≠ 正式知识；外部 Commit ≠ 知识采纳；AI 生成的草稿 ≠ 正式 Revision —— 只有质量门禁 + 人工确认后才成为权威。
> **设计权威**：知识空间 V2（质量门 §十一 / 三类红线）、`docs/ai-pm-markdown-knowledge-space.md`、14 号文档（三层权威）、16 号文档（R47/D30 采纳管道待查证）。
> **基线**：master @ `2b3f509e`。

---

## 0. 评审结论速览（先读这节）

规格的**边界原则全部正确**，且知识治理机制在本仓比预期完整。8 处核验结论：

| # | 级别 | 规格假设 | 本仓事实 | 处置 |
|---|---|---|---|---|
| **R50** | ✅ | 学习候选需来源与证据 | **真实机制**：`kb.memory_ref`（`ref_role ∈ evidence/subject`、`target_domain/type/id`、`target_revision_id` + `revision_missing_reason` CHECK —— 引用缺失必须声明原因）；`kb.source`（`source_kind` + `uri‖inline_content` 互斥 + sha256 `content_hash`）（`migrations/0001_baseline.sql:2901-3000`） | P0-050 全锚点真实 |
| **R51** | ✅ | 质量门禁由后端强制 | **真实且强于规格**：`knowledge_quality.rs` **8 项规则**，模块头明文「**规则本身不依赖模型**，保证模型不可用时仍可判定」；**3 类红线** `external_commitment / delete_history / rewrite_delivered` → `redline_pending` 等人类确认；重复内容 = 幂等无操作（`duplicate`） | 直接回应「不能只依赖 AI 自己判断」 |
| **R52** | ✅ | 统一协调机制 | **真实**：`knowledge_write_coordinator.rs` —— 项目级写锁 + `WriteCommand ∈ SubmitProposal/ConfirmRedline/VetoRevision` + **幂等账本 `ai.knowledge_write_operation`**（含崩溃恢复路径：账本行存在但绑定缺失 → 补齐绑定） | P0-053/056 全锚点 |
| **R53** | ✅ | 过期版本校验 | **真实**：`GateInput.base_revision_id`（「请求声明的基线修订，取代原 base_content_hash」）+ `current_revision_id`（最新 published）—— 旧版 Proposal 必须重校验 | P0-053 并发/055 一致性 |
| **R54** | 🔴 | 外部 Commit 自动发现与分析（P0-051） | **无发现/同步管道**（grep `discover/watch/commit_sync` 为空）；`/v1/kb/analyses`（K1）接受提交式分析，无自动 watch | **NOT_IMPLEMENTED**（承接 16 号 R47/D30）；归属验证语义已由 P0-05 P0-020 覆盖 |
| **R55** | 🔴 | 澄清与采纳闭环（P0-052） | 提案确认面真实（`documents/proposals` + `confirm_redline` + `veto`），但「Commit 发现→AI 分析→澄清问询→免澄清记录→采纳」**专用管道无实现**（图2 待立项） | **NOT_IMPLEMENTED**；澄清语义可经 `pm_interactions`（幂等键内建）与 KB review 面部分预演 |
| **R56** | 🟡 | 执行结果触发知识学习（P0-049） | 触发链存在但不全自动：intake → `integration.job('ai_analysis')` → `ai.analysis_run` + `ai.change_proposal`（P0-03 已证）；「Run/Delivery 完成自动触发知识影响分析」未证实 | 拆 **a（证据与提案链，✅可测）** + **b（自动触发，待查证）** |

**逻辑对象 → 本仓实体映射（规格 §2.2）**

| 规格逻辑概念 | 本仓实体 |
|---|---|
| Execution Evidence | `ai.analysis_run` + 事件/命令账 + `kb.source` |
| Learning Candidate | `kb.memory`（+ `kb.memory_ref` 证据/主体引用） |
| Knowledge Proposal | `ai.document_proposal`（V2 提案面）+ `ai.change_proposal` |
| Clarification | `pm_interactions`（`idempotency_key` 强制）/ K4 review |
| Knowledge Revision | `ai.document_revision`（`git_commit_sha` + `content_hash`） |
| Knowledge Adoption | `confirm_redline`（红线确认，human）+ 账本 `ConfirmRedline` |

---

## 1. 测试范围（8 例）

| 编号 | 测试案例 | 可测性 | 主锚点 |
|---|---|---|---|
| P0-049 | 执行结果触发知识学习 | 🟡 拆 a/b（R56） | intake→analysis_run→change_proposal |
| P0-050 | 学习结果来源与可信度 | ✅ | `kb.memory_ref` / `kb.source` |
| P0-051 | 外部 Commit 自动发现与分析 | 🔴 NOT_IMPLEMENTED（R54） | 归属验证语义 → P0-020 |
| P0-052 | 外部 Commit 澄清与采纳闭环 | 🔴 NOT_IMPLEMENTED（R55） | 提案确认面可预演 |
| P0-053 | 多 Agent 并行知识提议与协调 | ✅ | write_coordinator + `base_revision_id` + `pc.conflict` |
| P0-054 | 质量门禁与人工确认 | ✅ | `knowledge_quality` 8 规则 + 3 红线 |
| P0-055 | Revision 与 Git 版本一致性 | ✅ | 14 号 §3 口径 + 账本恢复 |
| P0-056 | 学习失败、冲突与恢复 | ✅ | `ai.knowledge_write_operation` + 0012 + reindex |

**权限套件解散说明（D32）**：原「权限隔离与安全」（跨 Team/跨 Project 写/AI 审批/MCP 绑定）内容已分散承接 —— 跨 Team 读 = P0-007 + FLOW-08 + 16 号 P0-047；跨 Project 写 = P0-015 步骤 6-7（守卫深验 `guards.rs` 仍挂 10 号 §8）；AI 审批 = P0-010 三道闸 + P0-019C1 + **DEF-P0-01**；MCP 绑定 = 16 号 P0-041。

---

## 2. 基础 Fixture

| 对象 | 落实 |
|---|---|
| Project-A | 独立 tid（95xx 段）；KnowledgeRoot（14 号 D17 同款 tmp git） |
| K-001（Rev-1）/ R-001（Rev-1） | 14 号 P0-011 全链产生（draft→submit→confirm） |
| Repo-A / Repo-B | 本地双仓库 + `task_repositories` 绑定 |
| AI-PM / Agent-A / Agent-B | ai_pm 主体（bootstrap 产）+ 可控 MCP client/adapter |
| 授权确认人 / 普通成员 | human 主体 ×2（`ensure_governance_authorized` 权限差） |

**测试知识示例（规格原文保留为夹具常量）**：K-001 = 多 Repository 架构 + 统一协调 + 外部 Agent 只能提交建议。

---

## 3. P0-049 · 执行结果触发知识学习（拆 a/b）

### 049a · 证据与提案链（✅ 可测）

| Step | 断言 | 锚点 |
|---|---|---|
| 1-2 Task/Run 正常创建 | P0-05 链路 | 计划物化 + mark-ready |
| 3 Agent 提交结果与学习线索 | 结果归属 Task/Run；线索带证据（文件路径/SHA） | intake `source_ref` + 命令账 |
| 5 触发分析 | `ai.analysis_run` + `ai.change_proposal(proposed)` | `POST /v1/kb/analyses`（K1）或 intake→ai_analysis |
| 6 正式 K-001 不变 | 仍为 Rev-1（`git_commit_sha` 不变） | 14 号口径 |
| 7 Git 无未确认变更 | HEAD 不变 | 投影绑定唯一点 |

**关键断言（规格原文 → 口径）**：候选可追溯 Task/Run/Delivery/Agent（`kb.memory_ref` evidence 行 + `analysis_run_id`）；重复提交不重复建候选（0012/幂等键）；AI 不得直接建正式 Revision（红线 + `submit` 只落待确认）；失败 Run 的证据可保留但**不得**标为已验证事实（`revision_missing_reason` 语义同理）。

**负向 4 项**（声称完成无证据 / 失败却交总结 / 与正式知识矛盾 / 夹带自动批准指令）→ 全部停在提案/候选态，权威不动（Git HEAD + `current_revision_id` 双不变）。

### 049b · Run 完成自动触发知识影响分析（⏳ 待查证）

自动触发器未证实存在 —— 若无，本场景标 **NOT_IMPLEMENTED**（D33）；落地前用显式提交（K1 / intake）代替触发。

---

## 4. P0-050 · 学习结果来源与可信度（✅ 全锚点）

四类来源（规格表 → 落实）：

| 来源 | 落实 |
|---|---|
| 已合并正式 Commit | `kb.source(source_kind, uri, content_hash)` + git 直验 |
| Agent 执行报告 | `kb.memory_ref(ref_role='evidence', target_domain='analysis_run'…)` |
| 人工澄清回答 | interactions 回包（保留回答者/时间）+ 审计 |
| AI 推断 | 提案 `rationale` + 可信度字段；不冒充事实 |

**关键断言**：每条候选可溯源（来源类型/标识/原始位置）；Commit 来源保留 Repo+SHA+路径；人工澄清保留回答者+时间；推断明确区分；证据不足不得升级为已确认（`revision_missing_reason` 模式：**缺失必须声明原因**）；来源变化不悄悄改写历史结论（账本 + 审计）。

> **规格「特别关注」正确**：不能只验文字标签 —— 可信度/待确认必须是**持久化字段 + 状态流转约束**（`kb.memory_ref` CHECK + 门禁 `failures[]` 结构化落盘），实现时按实际列断言（D34）。

---

## 5. P0-051 / P0-052 · 外部 Commit 发现、分析与采纳闭环（🔴 NOT_IMPLEMENTED）

**R54/R55**：发现/同步管道与专用「分析→澄清→采纳」管道均无实现（16 号 R47/D30 同源）。两案例**整体标 NOT_IMPLEMENTED，不计入 PASS**；落地后按以下已验证语义转正：

| 转正时断言 | 已验证锚点 |
|---|---|
| SHA 真实存在、归属本 Project | `git cat-file` 直验 + `task_repositories` 绑定（P0-020 口径） |
| 文档变更 vs 代码变更区分、与正式知识冲突识别 | `knowledge_quality` 门禁（冲突/重复检测）|
| 澄清问询与「无需澄清」记录 | `pm_interactions`（幂等）+ 提案 `rationale`（**免澄清也必须留判断记录**） |
| 人工确认采纳 → 正式 Revision | `confirm_redline`（红线确认，human）+ `git_commit_sha` 绑定 |
| 人工拒绝 | `veto` 面 + 拒绝人/时间/理由审计；正式内容与 Git 不动 |
| 重复触发不重复采纳 | `ai.knowledge_write_operation` 账本幂等 |

> 期间可**预演**的子集（不计 PASS）：经 `/v1/pm/documents/proposals` 提交提案 → 门禁判定 → `confirm_redline`/`veto` 全链（= P0-054 主体）。

---

## 6. P0-053 · 多 Agent 并行知识提议与统一协调（✅）

| Step | 断言 | 锚点 |
|---|---|---|
| 1-2 三 Agent 并行分析、独立提交候选 | 候选各自带来源 | K1 提交式分析 + `kb.memory_ref` |
| 3 统一协调接收 | 进入协调流程（项目级写锁） | `knowledge_write_coordinator` |
| 4 重复检测 | A/B 相似知识被识别 | 门禁 `duplicate`（相同内容幂等无操作） |
| 5 冲突分析 | C 的局部规范不误升全局 | 提案 `applicable scope` 字段 + `pc.conflict` |
| 6 统一 Proposal | 保留各自来源证据 | 多 `memory_ref` 挂同一 proposal |
| 7 人工确认 | 确认后才入正式 | `confirm_redline` |

**并发特别验证（规格 → 硬锚点）**：A/B 同时基于 Rev-1 对 K-001 提修改 → 两个 Proposal 可并存（提案不互斥）；正式 Revision 不被并行提议覆盖（写入走协调者串行化）；第二个基于过期基线 → `base_revision_id` ≠ `current_revision_id` → **门禁拒绝/要求重校验**，无静默覆盖；全局 vs Repository 局部规则正确区分；冲突不静默（`pc.conflict` 面）。

---

## 7. P0-054 · 知识质量门禁与人工确认（✅ 本套件权限红线主体）

**质量门 8 规则（后端强制，不依赖模型 —— R51）**：来源完整 / 证据可验证 / 与现有知识冲突 / 适用范围明确 / 未解决澄清 / Requirement·Design·ADR 关联 / **基于最新正式 Revision（`base_revision_id`）** / 重复检测。红线类（对外承诺/删历史/改写已交付）→ `redline_pending` 等人工。

**负向矩阵（规格 → 断言）**

| 操作 | 预期 | 锚点 |
|---|---|---|
| AI 自己批准自己的 Proposal | 拒绝 | `ActorKind::AiPm` ≠ human；`ensure_governance_authorized` |
| MCP Agent 直接写正式 Knowledge | 拒绝 | 无该工具（5 固定集）+ 写入唯一通道 = 协调者命令面 |
| 普通成员调用确认 API | 拒绝 | 授权确认人 vs 普通成员（governance 授权） |
| 伪造人工用户 ID | 拒绝 | 身份判库（kind），不信参数 |
| 缺证据强制采纳 | 拒绝/保持待处理 | 门禁 `failures[]` 非空 → 不放行 |
| 旧版 Proposal 直接采纳 | 必须重校验 | `base_revision_id` ≠ current → fail |
| 人工拒绝后后台任务继续写入 | 拒绝 | veto 终态 + 账本 |

**正向收尾**：授权人工确认 → 正式 Markdown/Git/DB/审计四面一致（14 号 §3 断言复用）。

---

## 8. P0-055 · Knowledge Revision 与 Git 版本一致性（✅）

沿用 14 号 §3 全套口径（K-001 Rev1→Rev2）：Rev1 保留不被覆盖；Rev2 = 真实 Git 版本（`git show` 验证）；PG 引用指向 Rev2；正文可从 Markdown+Git 读取；**PageIndex 更新失败不伤正式 Revision**（`reindex_required` 标记 + 可重建）；PageIndex 不得成为权威；**Git 成功 DB 失败 → 幂等账本恢复路径**（账本行带 revision_id → 返回该 sha；绑定缺失 → 补齐绑定，`requirement_projection` §6.7 同构）；DB 成功 Git 缺失 → 一致性故障，不得显示正常采纳。

**并发确认**：两授权用户同时确认不同 Proposal（都基于 Rev-1）→ 第一个成 Rev-2；第二个过期基线被门禁拒（R53）；审计说明冲突与处理。

---

## 9. P0-056 · 学习失败、冲突与恢复（✅）

| 故障注入 | 预期 | 锚点 |
|---|---|---|
| 学习分析超时 | 状态保留，可安全重试 | job 重试 + `attempt_count`（analysis_run） |
| 同一学习事件重复投递 | 幂等，不重复建候选 | 0012 同族 + 幂等键 |
| PageIndex 更新失败 | 正式知识不受影响，索引可重建 | `reindex_required` + reindex 路由 |
| Git 写入失败 | 无虚假正式 Revision | Git 先行事务（失败即整体失败） |
| Git 成功 DB 失败 | 可恢复，保留真实 Git 证据 | 账本恢复路径（R52） |
| 确认过程中服务重启 | 不重复采纳/错误批准 | `ConfirmRedline` 账本幂等 |
| 多 Agent 冲突提交 | 进统一冲突处理 | 协调者 + `pc.conflict` |
| Commit 被撤销/变更 | 原分析标需重校验 | `base_revision_id` 语义 |
| 澄清未完成 | Proposal 不得越过澄清门禁 | 门禁「未解决澄清」规则 |
| 协调者执行失败 | 候选与证据不丢，可恢复协调 | 账本（proposal_id/revision_id 回读） |

**恢复重点（规格原文 → 断言）**：重试复用原业务标识；不重复 Proposal/Revision/Git Commit；无丢失候选；已拒内容不被重新采纳；恢复后 DB/Git/审计三面一致。

---

## 10. 跨案例端到端综合场景（规格 §四 → 可测子集）

10 步链路中，**步骤 1（外部 Commit）、2（自动发现）、4（澄清）属 NOT_IMPLEMENTED**（R54/R55）；其余可测：

```
[3] AI 分析变更（显式提交式）→ [5] 多 Agent 并行分析 → [6] 协调者整合（去重/局部vs全局/留来源）
→ [7] 人工确认 → [8] Revision 创建（Markdown+Git+PG）→ [9] PageIndex 更新可检索 → [10] 全链审计追溯
```

**综合通过标准（规格 7 条 → 本仓口径）**：正式知识只发生**一次**经人工确认的版本变更；全部分析与来源可追溯；局部规范未误升全局；Git/PG 引用一致；PageIndex 可重建；未确认/被拒/证据不足内容未入正式（Git HEAD 与 `current_revision_id` 全程只动一次）。

---

## 11. 测试层次与辅助能力（规格 §五 → 本仓映射）

| 层 | 落点 |
|---|---|
| UI E2E | 提案/确认/否决页面 + Command Center |
| API Integration | `/v1/kb/analyses`、`/v1/pm/documents/proposals(+confirm/veto)`、`/v1/pc/*` |
| Git Integration | tmp 仓库 + `git show/cat-file`（14 号 git-version 同款） |
| DB Assertion | psql 直查（`kb.memory_ref` / `ai.knowledge_write_operation` / revision 行） |
| Agent Integration | 可控 adapter + MCP client（16 号同款） |
| Recovery | 账本/门禁/索引故障注入 |

**辅助函数 → 真实封装**：`createExternalCommit()`（tmp git）、`submitLearningEvidence()`（K1 + `kb.source`）、`triggerKnowledgeAnalysis()`（intake/K1）、`waitForProposal()`（轮询提案态）、`confirmKnowledgeProposal()`（`confirm_redline`，真实授权人）、`assertKnowledgeRevision()`（14 号断言复用）、`assertNoUnauthorizedKnowledgeWrite()`（Git HEAD + `current_revision_id` 双不变）、`assertKnowledgeProvenance()`（`memory_ref` 链完整性）。

---

## 12. 最终验收清单（规格 14 项 → 状态）

1. 执行结果可产生学习候选、不直接写正式知识 —— ✅ 049a
2. 候选保留来源/证据/可信度/不确定性 —— ✅ 050
3. 外部 Commit 自动发现并分析 —— 🔴 NOT_IMPLEMENTED（R54）
4. Commit 来源/SHA/仓库/归属验证 —— ✅ 语义已在 P0-020（发现管道待补）
5. 澄清问询/回答/免澄清理由可追溯 —— 🔴 随 052（interactions 机制已备）
6. 外部 Commit 采纳必须人工确认 —— ✅ 语义（confirm_redline）；管道 🔴
7. 多 Agent 并行分析、不并行写入 —— ✅ 053
8. 统一协调处理重复/冲突/过期版本 —— ✅ 053
9. 质量门禁后端强制 —— ✅ 054（R51）
10. 正式 Revision 与 Git 一致 —— ✅ 055
11. PG 只维护状态与引用，不取代 Markdown+Git —— ✅ 14 号口径
12. PageIndex 失败不影响正式知识、可重建 —— ✅ 055
13. 重复/并发/异常/恢复不重复采纳不丢版本 —— ✅ 056
14. 所有正式变更可追溯到人工确认与原始证据 —— ✅ 全链审计

> 门禁：**3 / 5 / 6(管道半) 为 NOT_IMPLEMENTED，不计 PASS**；D30/D33 落地后转正，38/38 过 G-release。

---

## 13. 待裁决项

| # | 议题 | 建议 |
|---|---|---|
| **D32** | 编号裁决 | 本套件 = P0-07 **049~056**（8 例，用户编号）；「权限隔离与安全」撤销独立套件（内容分散承接，守卫深验挂 10 号 §8）；总数 34→**38**，门禁 **38/38** |
| **D33** | 049b 自动触发 | 查证「Run/Delivery 完成自动触发知识分析」是否存在；无则 NOT_IMPLEMENTED，显式提交式代替 |
| **D34** | 可信度字段 | `kb.memory_ref`/门禁 `failures[]` 为结构化锚点；「可信度」具体列实现时确认后写死断言 |
| **D35** | 051/052 转正条件 | 外部 Commit 发现管道立项（D30）+ 澄清问询管道落地后，按 §5 表转正 |
| **D36** | 门禁 8 规则清单 | 实现时列出全部 gate code，写进 054 断言（当前已证 8 项规则 + 3 红线 + duplicate + base_revision） |

---

## 14. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/16-p0-06-playwright-spec-v1.md` | R47/D30（采纳管道待查证）、MCP/设备 fixture |
| `docs/e2e-spec/14-p0-04-playwright-spec-v1.md` | 三层权威与 Revision/Git 一致性断言全套复用 |
| `crates/pm-application/src/knowledge_quality.rs` | 8 项门禁 + 3 红线 + `base_revision_id`（后端强制） |
| `crates/pm-application/src/knowledge_write_coordinator.rs` | 统一协调 + `ai.knowledge_write_operation` 幂等账本 |
| `apps/pm-api/src/knowledge_services.rs` | K1-K4 / P1-P7（kb/pc 服务面） |
| `migrations/0001_baseline.sql:2901-3260` | `kb.memory(_ref)` / `kb.source` / `pc.approval` / `pc.conflict` / `pc.proposal` |
