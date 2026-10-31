# 任务状态机 V1 · 域核心（Slice A）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `ai-pm-task-state-machine-v1.md`（V12，§十 Q1–Q11 全部裁决）落成 `pm-domain` 纯域层核心：**13 值 TaskStatus + WaitingReason/WaitingContext + 迁移规则引擎（task_transition）+ Run 失败模型与重试计数**，全部带单元测试，不触 DB / API / MCP（后续 Slice B/C 另立计划）。

**Architecture:** 扩展既有 `crates/pm-domain/src/task.rs` 的 `TaskStatus`（5→13 值，保持 `as_str/from_str` + serde snake_case 既有模式）；新增 `waiting.rs`（等待上下文）、`task_transition.rs`（迁移规则引擎：全边矩阵 + Supersede 守卫 + 等待守卫）、`task_run.rs`（Run 四值结局 + 结构化失败记录 + 重试计数器）。域层零 I/O：守卫所需的外部事实（Run 是否停止、替代任务是否存在、等待是否解除）以输入字段传入。

**Tech Stack:** Rust（serde / serde_json / chrono / thiserror / uuid，均为既有依赖，零新增）。

**Spec:** [../../ai-pm-task-state-machine-v1.md](../../ai-pm-task-state-machine-v1.md)（V12）——§1.3 规范迁移边、§十一 Q1（Superseded）、§十二 Q2（WaitingContext）、§十九 Q9（失败模型与重试计数）、§二十一 Q11（枚举/存储口径）；5→13 迁移映射见 §九 M1–M4。

## Global Constraints

- **13 值全集**（§12.4 供稿，序即权威序）：`Draft, Ready, Claimed, Running, AwaitingDecision, Blocked, Submitted, Reviewing, Completed, ChangesRequested, Failed, Cancelled, Superseded`。Rust 变体 PascalCase；DB/serde 存 lowercase snake_case TEXT（`awaiting_decision` / `changes_requested`）。
- **终态四值** `Completed / Cancelled / Failed / Superseded`：**零出边**（图1 底注 + §11.1 终态定义）。
- **Superseded `allowed_from` 精确集** = `{Draft, Ready, Blocked, Running, Submitted, Reviewing}`（§11.2）。`Claimed / AwaitingDecision / ChangesRequested` **不在集内**（衍生 Q1.1 未裁，不得擅自放宽）。
- **Supersede 必填**：`superseding_task_id`（≠ 自身、任务须已存在、禁环）、`reason`、`actor`、`timestamp`；约束 `stop_active_run_before_finalization`：源为 `Running` 时必须先确认停止（§11.2/§11.4/§11.6）。
- **等待模型**（§12.2–12.5）：`AwaitingDecision` 统一承载四类 `WaitingReason`（NeedsInfo / NeedsDecision / NeedsApproval / ExternalDependency）；进入须携带 `WaitingContext`（reason + 非空 questions），离开须 `waiting_resolved = true`；**不得因收到回复即自动恢复**由调用方保证，域层只强制 `waiting_resolved` 守卫。
- **失败模型**（§19）：`RunOutcome` 四值；`Running→Failed` 必须携带 `FailureRecord` 且 `category` ∈ 终止性类别（B4-A 执行失败 / B4-D 不可恢复）或重试耗尽/总期限耗尽标志；**非零退出码单独不构成失败**。
- **重试计数**（§19.4）：默认 `max_run_attempts = 3`（含首次）、`max_consecutive_failures = 3`；成功 Run 后连续失败计数**归零**；总尝试计数**按执行阶段**分别统计（阶段未变才累计）。
- **legacy 迁移口径**（§九 M1–M4 + V3 L1）：`backlog→Draft`、`blocked→Blocked`、`done→Completed`（证据核查属 DB 迁移职责，域层只给映射）、`needs_info→AwaitingDecision(reason=NeedsInfo)`、`in_progress→歧义(Claimed|Running)` **必须显式处理，禁止默认映射**。
- 零新增 crate 依赖；每任务 TDD（先失败测试后实现）；每任务结束即 commit（只落本地）。

## Review Focus

1. **终态漏出边**：任何人给终态加了出边（如 `Failed→Running`）都会破坏审计不可变性 → Task 3 测试 `terminal_states_have_no_outgoing_edges` 钉死全矩阵。
2. **Superseded 集合漂移**：把 `Claimed` 等未裁状态悄悄加进 `allowed_from` 等于替需求方裁决 Q1.1 → Task 4 测试 `supersede_rejects_states_outside_ruled_set` 钉死精确集。
3. **非零退出码误判任务失败**：`exit_code != 0` 单独调用 `Running→Failed` 必须被拒（缺终止性证据）→ Task 5 测试 `running_to_failed_requires_terminating_failure_evidence`。
4. **in_progress 默认映射**：legacy 迁移若对 `in_progress` 给出单一答案即违反 M1 → Task 1 测试 `legacy_in_progress_is_ambiguous`。
5. **等待态未解除即恢复**：`AwaitingDecision→Running` 缺 `waiting_resolved` 必须拒绝（§12.3 关键约束）→ Task 4 测试 `awaiting_decision_requires_resolved_context_to_leave`。

## File Structure

```
crates/pm-domain/src/task.rs             # TaskStatus 5→13 值 + legacy 迁移映射（修改）
crates/pm-domain/src/waiting.rs          # WaitingReason + WaitingContext（新增）
crates/pm-domain/src/task_transition.rs  # 迁移规则引擎 + SupersededRelation + TransitionError（新增）
crates/pm-domain/src/task_run.rs         # RunOutcome + FailureCategory + FailureRecord + RetryCounters（新增）
crates/pm-domain/src/lib.rs              # mod + pub use（修改）
```

## Tasks

### Task 1 — TaskStatus 扩展为 13 值 + legacy 映射

**Files:** Modify `crates/pm-domain/src/task.rs`

**Interfaces (Produces):** `TaskStatus`（13 变体）、`TaskStatus::as_str/from_str`（全 13 值 roundtrip）、`TaskStatus::TERMINAL: [TaskStatus; 4]`、`TaskStatus::is_terminal(self) -> bool`、`legacy_from_str(s: &str) -> LegacyMapping`，其中 `enum LegacyMapping { Mapped(TaskStatus, Option<WaitingReason>), Ambiguous { candidates: &'static [TaskStatus] } }`（`in_progress` → candidates `[Claimed, Running]`）。

- [x] **Step 1: 写失败测试** — 改写 `task_status_string_roundtrip_covers_all_variants` 为 13 值全表（含 `awaiting_decision`、`changes_requested`、`superseded`）；新增 `terminal_states_are_exactly_four`（`TERMINAL == [Completed, Cancelled, Failed, Superseded]`）；新增 `legacy_mapping_covers_all_five_legacy_values`（backlog→Draft、blocked→Blocked、done→Completed、needs_info→(AwaitingDecision, Some(NeedsInfo))）与 `legacy_in_progress_is_ambiguous`。
- [x] **Step 2:** `cargo test -p pm-domain` → FAIL（变体不存在）。
- [x] **Step 3:** 实现 13 值枚举 + `as_str/from_str` + `TERMINAL`/`is_terminal` + `legacy_from_str`（映射表按 Global Constraints；`WaitingReason` 暂以占位引用，Task 2 落地前先用 `crate::waiting::WaitingReason`——若编译顺序冲突，可先在本任务内定义最小 `WaitingReason` 于 `waiting.rs` 并 `pub mod`）。
- [x] **Step 4:** `cargo test -p pm-domain` → PASS。
- [x] **Step 5:** Commit `feat(domain): extend task status to 13 states with legacy mapping`。

### Task 2 — WaitingReason + WaitingContext

**Files:** Create `crates/pm-domain/src/waiting.rs`; Modify `crates/pm-domain/src/lib.rs`（`pub mod waiting;`）

**Interfaces (Produces):** `enum WaitingReason { NeedsInfo, NeedsDecision, NeedsApproval, ExternalDependency }`（serde snake_case + `as_str/from_str`）；`struct WaitingContext { reason: WaitingReason, questions: Vec<String>, requested_from: Option<PrincipalId>, since: DateTime<Utc>, deadline: Option<DateTime<Utc>> }`；`impl WaitingContext { pub fn new(reason, questions) -> Result<Self, TransitionError> }`（questions 为空 → `Err(TransitionError::EmptyQuestions)`）。

- [x] **Step 1: 写失败测试**（`waiting.rs` 内 `mod tests`）：`waiting_reason_roundtrip`（4 值）；`context_rejects_empty_questions`；`context_accepts_needs_info_with_questions`（§12.3：进入时列出具体问题）。
- [x] **Step 2:** `cargo test -p pm-domain` → FAIL。
- [x] **Step 3:** 实现上述接口（`TransitionError` 暂由 Task 3 先行定义最小版本——为避免循环依赖，本任务先在 `task_transition.rs` 落 `TransitionError` 枚举骨架：`EmptyQuestions` + `IllegalTransition` + `MissingEvidence` + `SupersessionViolation(String)`，后续任务扩充）。
- [x] **Step 4:** `cargo test -p pm-domain` → PASS；Commit `feat(domain): waiting reason and context`。

### Task 3 — 迁移规则引擎：全边矩阵

**Files:** Create `crates/pm-domain/src/task_transition.rs`; Modify `crates/pm-domain/src/lib.rs`

**Interfaces (Produces):**
`pub struct TransitionInput<'a> { from: TaskStatus, to: TaskStatus, waiting: Option<&'a WaitingContext>, waiting_resolved: bool, failure: Option<&'a FailureRecord>, retry_exhausted: bool, deadline_exhausted: bool, supersede: Option<SupersedeIntent<'a>>, active_run_stopped: bool }`（提供 `TransitionInput::minimal(from, to)` 缺省构造，其余字段 `None/false`）；`pub struct SupersedeIntent<'a> { superseding_task_id: TaskId, reason: &'a str, actor: PrincipalId }`；`pub fn validate_transition(input: &TransitionInput) -> Result<(), TransitionError>`；`pub fn allowed_edges() -> &'static [(TaskStatus, TaskStatus)]`（全矩阵常量，供测试与后续 Slice 对照）。

矩阵（与 §1.3 + §11.2 + §19.6 一致，终态零出边）：
`Draft→Ready`；`Ready→Claimed`；`Claimed→Running`；`Claimed→Ready`（租约过期）；`Running→{Blocked, AwaitingDecision, Submitted, Failed}`；`Blocked→Ready`；`AwaitingDecision→{Ready, Running}`；`Submitted→Reviewing`；`Reviewing→{Completed, ChangesRequested}`；`ChangesRequested→Ready`；`{Draft,Ready,Claimed,Running,Blocked,AwaitingDecision,Submitted,Reviewing,ChangesRequested}→Cancelled`；`{Draft,Ready,Blocked,Running,Submitted,Reviewing}→Superseded`。

- [x] **Step 1: 写失败测试**：`allowed_edges_matches_ruled_matrix`（把上表 30 条边逐对断言，排序后整体相等）；`terminal_states_have_no_outgoing_edges`（对 4 终态 × 13 目标全部 `Err`）；`identity_transition_is_illegal`；`cancel_allowed_from_every_non_terminal`（9 非终态 × Cancelled 全 `Ok`）。
- [x] **Step 2:** `cargo test -p pm-domain` → FAIL。
- [x] **Step 3:** 实现 `allowed_edges()` 常量 + `validate_transition` 先查矩阵（不在表内 → `Err(IllegalTransition{from,to})`），守卫字段校验留给 Task 4/5。
- [x] **Step 4:** PASS；Commit `feat(domain): task transition matrix`。

### Task 4 — Supersede 与 AwaitingDecision 守卫

**Files:** Modify `crates/pm-domain/src/task_transition.rs`

**Interfaces (Produces):** 在 `validate_transition` 内启用两组守卫：目标为 `Superseded` 时要求 `supersede` 字段完整且 `superseding_task_id != task_id`（`task_id` 经 `TransitionInput::for_task(task_id, from, to)` 携带，`minimal` 场景 `task_id` 可空但 Supersede 时必填 → 违反 `Err(SupersessionViolation(_))`）；源为 `Running` 的 Supersede 还要求 `active_run_stopped == true`（`stop_active_run_before_finalization`）；离开 `AwaitingDecision` 要求 `waiting_resolved == true`（违反 → `Err(MissingEvidence("waiting context unresolved"))`）。

- [x] **Step 1: 写失败测试**：`supersede_rejects_states_outside_ruled_set`（`Claimed→Superseded`、`AwaitingDecision→Superseded`、`ChangesRequested→Superseded` 均 `Err(IllegalTransition)`——集合问题在 Task 3 已拒，此测试防回归漂移）；`supersede_requires_intent_and_distinct_task`（缺 intent / 自我取代 → `SupersessionViolation`）；`running_supersede_requires_stopped_active_run`；`awaiting_decision_requires_resolved_context_to_leave`（未解决 → `Err`，解决 → `Ok`）。
- [x] **Step 2:** FAIL；**Step 3:** 实现守卫；**Step 4:** PASS；Commit `feat(domain): supersede and waiting guards`。

### Task 5 — Failed 守卫 + Submitted/Reviewing 证据位

**Files:** Modify `crates/pm-domain/src/task_transition.rs`

**Interfaces (Produces):** `Running→Failed` 要求 `retry_exhausted || deadline_exhausted || (failure 为 Some 且 category ∈ {Permission, PolicyViolation, MissingDependency})`（B4-D 不可恢复集；`Execution/Infrastructure/Validation/Unknown` 不构成任务级终止——§19.3：临时故障保持可重试）；违反 → `Err(MissingEvidence("no terminating failure condition"))`。`Running→Submitted` 要求 `supersede` 之外另加 `delivery_submitted: bool`（E1 证据位占位，Slice B 接交付校验时填充语义）——本任务先把字段加进 `TransitionInput` 并强制 `true` 才放行。`Reviewing→Completed` 要求 `acceptance_evidence_complete: bool == true`（E2 证据位）。

- [x] **Step 1: 写失败测试**：`running_to_failed_requires_terminating_failure_evidence`（仅非零退出码语义的 `FailureRecord{category: Execution, retryable: true}` → `Err`；`category: Permission` → `Ok`；`retry_exhausted` → `Ok`）；`submitted_requires_delivery_evidence`（`delivery_submitted=false` → `Err`）；`completed_requires_acceptance_evidence`（`false` → `Err`）。
- [x] **Step 2:** FAIL；**Step 3:** 实现；**Step 4:** PASS；Commit `feat(domain): failure and acceptance evidence guards`。

### Task 6 — Run 失败模型 + 重试计数器

**Files:** Create `crates/pm-domain/src/task_run.rs`; Modify `crates/pm-domain/src/lib.rs`

**Interfaces (Produces):** `enum RunOutcome { Succeeded, Failed, TimedOut, Cancelled }`；`enum FailureCategory { Infrastructure, Execution, Validation, Permission, PolicyViolation, MissingDependency, Unknown }`；`struct FailureRecord { run_id: RunId, category: FailureCategory, retryable: bool, reason_code: String, exit_code: Option<i32>, timeout_seconds: Option<u64>, evidence_ref: Option<String> }`；`pub struct RetryCounters { attempt_count: u32, consecutive_failure_count: u32, last_failure_category: Option<FailureCategory>, retry_exhausted: bool }` + `pub struct RetryPolicy { max_run_attempts: u32, max_consecutive_failures: u32 }` + `impl Default for RetryPolicy`（3 / 3）；`impl RetryCounters { pub fn register_outcome(&mut self, outcome: RunOutcome, same_stage: bool, policy: &RetryPolicy) -> OutcomeVerdict }`，`enum OutcomeVerdict { Continue, StopRetry }`。规则：成功 → 连续失败归零；失败且 `same_stage` → 双计数累计；`!same_stage` → 仅累计 `attempt_count`（阶段切换重开连续序列）；任一达阈值 → `StopRetry` 且 `retry_exhausted = true`。

- [x] **Step 1: 写失败测试**：`failure_record_is_immutable_payload`（构造后字段只读——编译期保证即可，无需运行时测试；以 `consecutive_failures_reset_on_success` 为主）；`consecutive_failures_reset_on_success`（fail→success→fail：`consecutive_failure_count == 1`）；`stage_change_restarts_consecutive_series`（§19.4 例：网络失败→编译成功→测试失败，`consecutive_failure_count == 1` 而非 3）；`attempts_reach_max_stops_retry`（3 次 `same_stage` 失败 → `StopRetry`）；`defaults_are_three_and_three`。
- [x] **Step 2:** FAIL；**Step 3:** 实现；**Step 4:** PASS；Commit `feat(domain): run outcome, failure record and retry counters`。

### Task 7 — 导出收口与全量验证

**Files:** Modify `crates/pm-domain/src/lib.rs`

- [x] **Step 1:** `lib.rs` 增 `pub mod task_run; pub mod task_transition; pub mod waiting;` 并 `pub use` 公开类型（`TaskStatus` 既有导出不变）。
- [x] **Step 2:** `cargo test -p pm-domain`（全绿）→ `cargo clippy -p pm-domain --all-targets -- -D warnings`（零告警）→ `cargo build`（workspace 编译不破——`pm-application` 等对 `TaskStatus` 的既有用法仅 `InProgress/Done` 等旧变体，会编译失败；本步骤顺带把 workspace 内旧变体引用迁移到新枚举：`grep -rn "TaskStatus::" crates apps` 逐处替换为语义最近的新值，**只替换编译必需处，不改行为逻辑**，行为级接线留 Slice B）。
- [x] **Step 3:** `cargo test --workspace`（全绿；既有集成测试中依赖旧状态值的用例按同口径机械替换）。**证据（2026-10-31 19:40）**：`bash scripts/cargo-test.sh` → nextest `219 tests run: 219 passed, 0 skipped`（19.157s）+ `cargo test --workspace --doc` 全绿，EXIT=0；`bash scripts/clippy.sh --all` → 见下方验证记录。
- [x] **Step 4:** Commit `feat(domain): wire 13-state machine exports and migrate legacy variant uses`。

## Self-Review 记录

- 覆盖检查：§1.3 全边矩阵（Task 3）、§11 Supersede（Task 4）、§12 等待（Task 2/4）、§19 失败与计数（Task 5/6）、§九 M1–M4 legacy（Task 1）均有对应任务与钉死测试；§13/§16/§17/§18/§20 属应用层与后续 Slice，不在本计划（Scope Check：独立子系统另立计划）。
- 类型一致性：`FailureCategory` 在 Task 5（transition 守卫）与 Task 6（task_run 定义）间共享——Task 5 的测试依赖 Task 6 的类型，**执行顺序必须 Task 6 先于 Task 5**（已按依赖重排：执行序 Task 1→2→3→4→6→5→7）。
- Review Focus 五项全部有钉死测试（终态/集合/退出码/in_progress/等待守卫）。

## 验证记录（Task 7 收尾，2026-10-31）

| 项 | 命令 | 结果 |
| --- | --- | --- |
| 域层单测 | `cargo test -p pm-domain` | 全绿 |
| 域层 lint | `cargo clippy -p pm-domain --all-targets -- -D warnings` | 零告警 |
| 全量测试 | `bash scripts/cargo-test.sh`（nextest + `cargo test --workspace --doc`） | `219 tests run: 219 passed, 0 skipped`（23.2s），doctest 全绿，EXIT=0 |
| 全量 lint | `bash scripts/clippy.sh --all` | `[clippy] PASS`，EXIT=0 |

补充说明：

1. **既有集成测试机械替换**：除 `TaskStatus::InProgress/Done` 变体改 `Running/Completed` 外，测试内的状态字符串字面量也按 §九 口径替换——`needs_info → awaiting_decision`、`done → completed`、执行后断言 `in_progress → running`。**种子数据与补偿回滚断言保留 `in_progress`/`backlog`**：这部分是 DB 既有字面量，属 Slice B 数据迁移职责，域层不越界。
2. **全量 lint 首次失败源于既有代码（非本 Slice 引入）**：`apps/pm-api` 的 9 处 `-D warnings`（3 处 `private_interfaces`、1 处 `redundant_closure`、4 处 `useless_conversion`、1 处 `needless_borrows_for_generic_args`）在改动前即存在，随独立提交 `fix(pm-api)` 以最小机械方式清理（子模块 `router` 收窄为 `pub(crate)`、去掉同类型 `Value::from`、闭包改函数引用），**不改行为**。
3. **风险事件**：验证期间本机 Docker 守护进程重启导致 `aipm-pg-dev` 退出，集成测试一度报 `PoolTimedOut`（非代码缺陷）；重启容器后复跑全绿。记录备查：本地全量验证依赖 dev 库在线。

