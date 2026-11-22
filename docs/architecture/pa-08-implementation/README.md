# PA-08 Implementation

PA-08 将 PA-07 验收遗留的执行可靠性与运维债务转为可测试、可观测的工程闭环。

- [PA-08 Implementation Plan](PA08-IMPLEMENTATION-PLAN.md)：范围、工作流、约束与验收门槛。
- [PA08-00 Current-State Inventory](PA08-00-CURRENT-STATE-INVENTORY.md)：关键模块静态盘点与实施前风险。
- [PA08-03 Run Transition Inventory](PA08-03-RUN-TRANSITION-INVENTORY.md)：状态写路径、转换矩阵、守卫实施与 PostgreSQL 证据。
- [PA08-04 Resource Reclamation Inventory](PA08-04-RESOURCE-RECLAMATION-INVENTORY.md)：Run worktree / shared Git mirror 所有权、删除红线、进程清理账本风险与安全实施顺序。
- 前置证据：[PA-07 最终验收](../pa-07-implementation/PA07-FINAL-IMPLEMENTATION-ACCEPTANCE.md)、[PA-07 当前差异核验](../pa-07-implementation/PA07-IMP-003-current-state-verification.md)。

## Workstream status

| ID | Workstream | Priority | Status |
|---|---|---:|---|
| PA08-00 | Baseline & inventory | P1 | COMPLETE (static inventory) |
| PA08-01 | Registry portability | Foundation | VERIFIED |
| PA08-02 | Runner lifecycle test harness | P1 | VERIFIED (implemented slice; follow-up failure cases remain) |
| PA08-03 | Run transition guard | P1 | VERIFIED |
| PA08-04 | Workspace / mirror reclamation | P1 | PROCESS CLAIM FIX + CANDIDATE STAGING IMPLEMENTED; CI PENDING |
| PA08-05 | Product lifecycle integrity | P1 | PLANNED |
| PA08-06 | Runtime observability | P2 | PLANNED |

### PA08-01 verification evidence

- GitHub Actions: [Run #4 — public-registry-build](https://github.com/luqizheng/ai-pm/actions/runs/44967203110)
- Result: `completed / success`
- Verified steps: locked Cargo metadata, all four Digua crates sourced from crates.io, and `cargo check --workspace --locked`.
- PA-07 blast-radius regression was migrated to GitHub Hosted runners after the public-registry cutover; the first new hosted run is [Run #25](https://github.com/luqizheng/ai-pm/actions/runs/44968929358). PostgreSQL integration remains a true skip unless `AIPM_TEST_DATABASE_URL` is configured.

### PA08-02 current slice

- Added `apps/ai-pm-runner/tests/pa08_lifecycle.rs`.
- Exercises the production `claim_and_start` path against a scripted Wiremock Platform: ready-task discovery, claim response parsing, run-context retrieval, lease-owner routing identity, and actual child-process spawn/cleanup.
- Covers the empty-queue path without claiming or spawning.
- CI now runs `cargo test -p ai-pm-runner --tests --locked` after the workspace build.
- Initial CI run [#7](https://github.com/luqizheng/ai-pm/actions/runs/44968483995) passed the workspace build and Runner integration tests for the claim/spawn and empty-queue cases.
- Expanded heartbeat/command-tick coverage passed in [Run #10](https://github.com/luqizheng/ai-pm/actions/runs/44968579673).
- Added real child-process terminate + ack, continue + human-reply injection, and frozen-guard refusal tests. The corrected command-path suite passed in Hosted CI [Run #19](https://github.com/luqizheng/ai-pm/actions/runs/44968871944): workspace build and Runner integration tests both succeeded. The earlier [Run #14](https://github.com/luqizheng/ai-pm/actions/runs/44968788640) caught the extra `spawn_agent` argument and was fixed in commit `0281c40f940e703d6d7ede14bcf63b27eea9b73d`. Restart recovery and broader transport/lease/spawn failure-injection coverage remain open; PA08-02 is not complete.

### PA08-03 verification evidence

- [Run #52](https://github.com/luqizheng/ai-pm/actions/runs/44970075346): complete success — workspace build, Runner integration, real PostgreSQL guard tests, and guard unit tests.
- [Run #57](https://github.com/luqizheng/ai-pm/actions/runs/44970217926): complete success — includes Project archive cancellation of an active Reviewer Run.
- [Run #58](https://github.com/luqizheng/ai-pm/actions/runs/44970354176): complete success — Project-scoped infrastructure exhaustion compare-and-set tightening.
- [Run #63](https://github.com/luqizheng/ai-pm/actions/runs/44970788999): complete success — latest tree, Runner tests, seven PostgreSQL guard/concurrency tests, and guard unit tests.
- PA-07 Hosted unit regression [Run #60](https://github.com/luqizheng/ai-pm/actions/runs/44970217908) passed. Its PostgreSQL integration job was explicitly skipped because `AIPM_TEST_DATABASE_URL` is not configured; do not count that as a DB regression pass.

Status changes require code/test evidence. A plan or merged documentation is not implementation completion.
