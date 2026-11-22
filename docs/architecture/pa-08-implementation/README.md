# PA-08 Implementation

PA-08 将 PA-07 验收遗留的执行可靠性与运维债务转为可测试、可观测的工程闭环。

- [PA-08 Implementation Plan](PA08-IMPLEMENTATION-PLAN.md)：范围、工作流、约束与验收门槛。
- [PA08-00 Current-State Inventory](PA08-00-CURRENT-STATE-INVENTORY.md)：关键模块静态盘点与实施前风险。
- 前置证据：[PA-07 最终验收](../pa-07-implementation/PA07-FINAL-IMPLEMENTATION-ACCEPTANCE.md)、[PA-07 当前差异核验](../pa-07-implementation/PA07-IMP-003-current-state-verification.md)。

## Workstream status

| ID | Workstream | Priority | Status |
|---|---|---:|---|
| PA08-00 | Baseline & inventory | P1 | COMPLETE (static inventory) |
| PA08-01 | Registry portability | Foundation | VERIFIED |
| PA08-02 | Runner lifecycle test harness | P1 | IN PROGRESS |
| PA08-03 | Run transition guard | P1 | PLANNED |
| PA08-04 | Workspace / mirror reclamation | P1 | PLANNED |
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
- Added real child-process terminate + ack, continue + human-reply injection, and frozen-guard refusal tests. The first command-path CI attempt [Run #14](https://github.com/luqizheng/ai-pm/actions/runs/44968788640) exposed a test compile error (the test passed one extra argument to `adapter::spawn_agent`); this was corrected in commit `0281c40f940e703d6d7ede14bcf63b27eea9b73d`. A fresh hosted run for the corrected tree is pending; do not treat the new command-path tests as verified yet. Restart recovery and broader transport/lease/spawn failure-injection coverage remain open; PA08-02 is not complete.

Status changes require code/test evidence. A plan or merged documentation is not implementation completion.
