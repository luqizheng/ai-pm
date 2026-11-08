# DK-E 实施验收记录 —— 场景 2 更正回流（更正 finding + 知识提案落点 + 零写入边界）

日期：2026-11-09。工作树/分支：`../ai-pm-dk0e` / `feat/dk0-dialog-orchestration-dke`。
基点：`master a11927a`（DK-B 已由 `86b5f66` 合入、DK-C 由 `c951bf2`、DK-D 由 `722ebd7`；
即基点已含 C7 双入口行锁与 DK-C S6 注入面——任务书的「DK-B 分支 tip」已不存在独立
分支，取含其全部交付的 master tip）。

## 一、交付形态

| 层 | 文件 | 内容 |
|---|---|---|
| 识别/校验 | `crates/pm-ai/src/discovery_orchestrator.rs` | 模型契约新增可选 `correction` 对象；服务端两级 fail-closed 复算；更正轮把会话推进 `REVIEW_REQUIRED`（受 C1 保护） |
| 单点写入 | `crates/pm-application/src/discovery/orchestrator_round.rs` | `RoundCorrection` 数据契约 + `apply_orchestrator_round` 第 3b 步（同一事务）：`contradicted` + `user_confirmed` 双行 + `[更正待知识应用]` deferred 标记 |
| 自动开访谈 | `crates/pm-application/src/discovery/auto_onboarding.rs`（新增） | `analysis_unknown_questions`（纯函数）+ `ensure_onboarding_from_analysis`（幂等、开关门控） |
| 事件接线 | `crates/pm-infrastructure/src/kb_pc_handlers.rs` | `KbAnalysisCompletedHandler` 在任务完成回写后顺带开访谈（失败仅 WARN） |
| 开关装配 | `kb_pc_handlers.rs` / `outbox_handlers.rs` / `apps/ai-pm-worker` / `apps/pm-worker` | `register_all_handlers(..., discovery_orchestrator_enabled)`；两个 worker 从 `DiscoveryOrchestratorMode::from_env()` 传入（默认 off） |
| 测试 | `pm-ai/src/discovery_orchestrator/tests/dke.rs`（6 例）<br>`pm-application/tests/discovery_auto_onboarding.rs`（5 例）<br>`pm-infrastructure/tests/kb_analysis_onboarding.rs`（1 例） | 见 §三/§四 |
| 文档 | `docs/product/ai-pm-dk-e-step0-f8-f23-verification-v1.md`、设计稿 §12.2-4 | 第 0 步核实与偏差登记 |

**零迁移**：`migrations/` 仍是 `0001_baseline.sql` + `0002_conversation_discovery_link.sql`
（0002 为 DK-A 交付）。

## 二、第 0 步结论（F8 ↔ F23）

**走 (b)**，取证见 `ai-pm-dk-e-step0-f8-f23-verification-v1.md`：

- `discovery.discovery_proposal` 的三个对象 FK 全指 `ai.project_document` /
  `ai.document_revision` / `ai.document_proposal`；服务层 `add_proposal` 对
  `modify` 还要求 `base_revision_id` 是 `document_id` 的 `ai.document_revision` 行；
- checkpoint-1 KB digest 实写 `kb.document_revision`（`kb_pc_analysis.rs`），
  PC 对象在 `pc.*`；kb/pc 对 `ai.project_document` 既无 FK 也无镜像写入；
- 因此**不为绕 FK 造影子行、不迁移**：更正止步于 findings 双行 + `REVIEW_REQUIRED`
  + deferred 标记；KB 应用挂账 KP-3、PC 挂账 KP-2；F2 口径改为「零提案行 + 标记可见」。

## 三、F 组验收（§8.3，逐条可判）

| # | 判据（原口径） | 本批口径/结果 | 证据 |
|---|---|---|---|
| F1 | 更正产生 `contradicted` + `user_confirmed` 两行，旧 finding 不被删改 | **PASS**：双行落库；旧行 `kind/content/verification_status` 三项快照前后全同；contradicted 行以 `system_observation` 证据 `contradicts:<旧 id>` 指向旧行 | `dke.rs::correction_records_both_rows_and_keeps_kb_pc_untouched` |
| F2 | 提案 `base_revision_id` 指向当时 KB revision；人工决定前 KB 行零变化 | **改口径 PASS（(b)）**：**零提案行**（`discovery_proposal` count=0）+ **标记可见**（`discovery_question` status=`deferred`、正文前缀 `[更正待知识应用]`、`finding_id`=contradicted 行）+ KB 行零变化（`kb.document_revision` count=0） | 同上 |
| F3 | PC 对象侧提案落 `discovery_proposal`，`pc.*` 零写入 | **PASS（(b)）**：`pc.proposal` / `pc.cognition_object` / `pc.cognition_revision` 三个表 count=0，零写入 | 同上 |

## 四、新增验收面

| 项 | 结果 | 证据 |
|---|---|---|
| 更正识别 fail-closed ①：普通问答不误判 | **PASS**：`intent` 正确但引文落不进本轮用户原话 → 整条 correction 丢弃；零 contradicted、零标记、会话留 `IN_PROGRESS` | `dke.rs::ordinary_question_is_not_misread_as_a_correction` |
| 更正识别 fail-closed ②：target 不可定位 | **PASS**：target 为任意 uuid → 更正事实落 `observed`、**不硬绑**旧行；零 contradicted、零标记 | `dke.rs::unresolvable_correction_target_degrades_to_observed` |
| 更正后同轮回答已反映新事实 | **PASS（两层）**：①本轮 ai 消息原样落库（含更正后事实）；②下一轮 system prompt 的 S6 同时含「用户更正：此前记录的事实「认证使用 session cookie」不再成立」与「认证使用 JWT（Bearer token）」——`wiremock` 抓取真实请求体断言 | 同上 + `dke.rs::follow_up_round_is_answered_from_the_corrected_facts` |
| C1 护栏：更正不得绕过阻塞问题 | **PASS**：仍有未答阻塞问题时更正照落，但会话不进 `REVIEW_REQUIRED`（留 `IN_PROGRESS`，C1 只禁复核态） | `dke.rs::correction_cannot_bypass_open_blocking_questions` |
| F7 人工裁决不可绕 | **PASS**：两条新行创建即 `PENDING`；`ai_pm` principal 直调 `record_finding_verification` → `Validation` 且行状态不变；同行走真人 principal → `CONFIRMED`（闸门只拦非人类）。编排器代码路径不引用该函数（grep 零命中） | `dke.rs::correction_verification_requires_a_human_decision` |
| 导入自动开访谈：事件触发建会话 + unknowns→questions | **PASS**：`kb.analysis.completed` 消费 → `source=SYSTEM` / `kind=INITIAL_ONBOARDING` / `created_by=dispatch principal` 会话；`analysis_mode=derived_fallback + truncated` → 2 条 `why_blocking` 非空的 open 问题；同一事件重放零新增（会话复用） | `discovery_auto_onboarding.rs::analysis_event_opens_interview_with_questions`、`kb_analysis_onboarding.rs::the_analysis_completed_event_opens_the_interview_only_when_switched_on` |
| 导入自动开访谈：开关 off 零动作 | **PASS**：应用层 `enabled=false` → 零会话零问题；handler 层 `with_auto_onboarding(false)` → 事件仍完成回写、零会话 | `discovery_auto_onboarding.rs::switch_off_writes_nothing`、`kb_analysis_onboarding.rs` |
| 自动开访谈的复用/不复活 | **PASS**：已有进行中 `INITIAL_ONBOARDING` → 复用（不建第二条）；最近一条 `COMPLETED` → 零动作不复活；问题按正文去重 | `discovery_auto_onboarding.rs::existing_interview_is_reused` / `completed_interview_is_not_resurrected` |
| 派生规则纯函数 | **PASS**：`llm`+未触顶 → 0 条；`derived` → 1 条；`derived_fallback`+触顶 → 2 条；`why_blocking` 全非空 | `discovery_auto_onboarding.rs::unknowns_are_derived_from_analysis_confidence_and_coverage` |

## 五、偏差登记（有意偏离设计稿/任务书之处）

1. **F2 口径改 (b)**（§二）：F8 与 §8.2 在现 schema 下互相矛盾；本批按任务书 (b)
   处置并已回写设计稿 §12.2-4。**不是实施缩水**：零影子行、零迁移、零 pc/KB 写入。
2. **更正轮强制 `REVIEW_REQUIRED` 不要求槽位全绿**：§4.3 的三条件在更正轮放宽
   「槽位全绿」一项——复核对象是「待人类决定的知识更正」（deferred 标记），
   不是模型自称问完了。C1（阻塞问题）仍硬拦；C2（模型自称问完了但槽位缺）
   的夹具没有 correction 对象，原语义不变。
3. **「分析产物的未知/低置信点」的落地解释**：kb digest 没有结构化 unknowns 列表
   （`kb.document_revision.content` 是 Markdown，`RepoFacts` 无 unknowns 字段），
   故从**分析元数据**派生：`analysis_mode != 'llm'`（低置信）+ 审计
   `after_data->>'truncated'`（采集触顶，覆盖未知）。导入草案
   `draft.unknowns → questions` 的既有路径（S01 G-20）保持不动，不重复造第二套。
4. **自动开访谈复用规则**：复用项目最近一条 `INITIAL_ONBOARDING`（不限 source），
   不按分析 revision 建第二套会话——零迁移下没有「会话↔分析修订」链接列；
   幂等锚=会话 + 问题正文去重。最近一条 `COMPLETED` 不复活（避免把已结束的访谈
   重新拉回开放态）。
5. **contradicted → 旧 finding 的链接载体**：`discovery_finding_evidence`
   （`system_observation`，`description='contradicts:<uuid>'`），不用 `actor_role`
   ——该列是槽位标签（`slot:<id>`）的语义位。
6. **commit 拆分**：更正「识别 + 落库」作为一个纵切提交（识别层单独提交无法
   可观测、测试会为空转）；自动开访谈独立提交；验收记录本文件单独提交。

## 六、回归与门禁

- `bash scripts/cargo-test.sh`（nextest 全量 + doctest）：
  **1313 tests run: 1313 passed, 4 skipped**（116~131s；白名单内无红，白名单外零红）。
- `cargo clippy --workspace --all-targets`：clean。
- `cargo fmt`：**本批改动文件 rustfmt 干净**；`cargo fmt --all -- --check` 仍有
  40 处**存量**漂移（`apps/pm-api/*`、`pm-ai/conversation_reply.rs`、
  `discovery/web.rs` 等，master 基点即存在，非本批引入；按并行会话纪律不 sweep）。
- 零迁移；MCP 四工具未动；V-6 单写入（编排器零直写，全部经
  `apply_orchestrator_round` / discovery 既有服务）。

## 七、提交清单（本分支）

| commit | 内容 |
|---|---|
| `385308a` | 第 0 步 F8↔F23 核实 + 设计稿 §12.2 第四缺口登记 |
| `69b112b` | 三态提案分支：更正识别（fail-closed 两级）+ 双行/标记/复核态落库 |
| `be03783` | 导入自动开访谈：`kb.analysis.completed` 消费接线（同开关门控） |
| `b51e536` | 注释措辞避开 ADR-06a 容器守卫（`ai.document*` 冻结集） |
| `后续` | 验收记录（本文件）+ 更正轮 ai 消息断言 |

## 八、未做 / 交接

1. **KB 应用**：不在本批。`[更正待知识应用]` 标记与 `contradicted` finding 是
   KP-3 评审链（k4）的输入；KP-2 建 PC 提案面后，同一标记可平移为正式提案。
2. **demo 话术（DK-F）**：场景 2 在 V0.5 内止步于「复核页可见的更正待办」，
   **不得宣称 KB/PC 已被更新**。
3. **`REVIEW_REQUIRED` 会话的后续问答**：编排器扫描集是
   `IN_PROGRESS/NEEDS_USER_INPUT`；复核后要继续问答，需人工走既有
   `REVIEW_REQUIRED → IN_PROGRESS` 边（本批测试即按此路径验证注入面）。
   若 DK-F 想让「复核中继续追问」自动回到编排，需要单独裁决（新语义，不在本批）。
