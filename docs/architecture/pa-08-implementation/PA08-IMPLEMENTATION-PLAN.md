# PA-08 — Execution Reliability & Runtime Operations

- **状态：** IMPLEMENTATION IN PROGRESS (PA08-01/03 VERIFIED; PA08-02/04/06 follow-up hardening implemented and awaiting latest Hosted validation)
- **日期：** 2026-11-22
- **基线分支：** `master` at `a7a522ccf818c7eac4d4ab096ae8e447c8f2bef2`
- **前置验收：** PA-07 `APPROVED_WITH_LIMITATIONS`
- **权威边界：** PA-07 架构基线继续有效；本文是 PA-07 已登记工程债的实施计划，不新增或改变业务授权。

## 1. 目标

PA-08 将 PA-07 验收时登记的执行可靠性与运维债务转化为可验证的工程闭环。目标不是增加新的调度产品或分布式运行平台，而是让现有 PostgreSQL + Rust Runner/Orchestrator 在进程崩溃、迟到回报、资源积累和 Product 状态竞争时保持安全、可恢复、可诊断。

完成标准：
1. Run 生命周期的合法转换、fencing、迟到/重复事件处理具有统一可测试的守卫。
2. Runner 主循环关键阶段可注入依赖并由确定性测试覆盖。
3. Workspace/mirror 资源可安全识别、预览、回收；绝不删除活跃 Run 或仍被引用的资产。
4. Product 生命周期只允许合法状态转换；暂停/恢复/归档与 Run 启动竞争有确定性结果。
5. 关键运行指标、资源水位和 needs-human 状态可观测；不以日志代替关键指标。
6. 所有 PA-08 依赖均可从公开 crates.io 获取，不再要求内部 Kellnr Registry。

## 2. 范围与来源

PA-07 最终核验的 OPEN 项映射如下：

| PA-07 项 | PA-08 工作流 | 优先级 | 目标 |
|---|---|---:|---|
| IMP-011 Runner 主循环测试不完整 | PA08-02 Runner lifecycle harness | P1 | claim→spawn→heartbeat→command→submit→exit、崩溃与迟到回报 |
| IMP-012 Run 状态转换分散 | PA08-03 Run transition guard | P1 | 统一转换矩阵与 fencing / terminal protection |
| IMP-010 Workspace/mirror 无回收 | PA08-04 Resource reclamation | P1 | 引用安全、保留期、dry-run、审计和可恢复删除策略 |
| IMP-013 Product lifecycle 源状态校验及 E2E 缺口 | PA08-05 Product lifecycle integrity | P1 | 合法状态图、并发门禁和恢复场景 E2E |
| IMP-018 缺少 metrics / 水位 / 告警 | PA08-06 Runtime observability | P2 | 指标导出、磁盘水位、needs-human 可发现性 |
| crates.io 已发布 digua 依赖 | PA08-01 Registry portability | Foundation | 去除 Kellnr 专属依赖并验证公开 Registry 构建 |

IMP-014（Submit 长事务）、IMP-015（LLM token/cost budget）、IMP-017（Device credential TTL）及仓库级 fmt 漂移不属于本批核心范围，保留为独立后续债务，不借 PA-08 静默扩项。

## 3. 工作流与交付顺序

### PA08-00 — Baseline & inventory
- 固定代码基线、迁移基线、测试命令和现存运行状态语义。
- 对照 PA-07 架构文档、IMP-003 和当前代码；所有结论标记为代码事实、目标要求或待决事项。
- 不因清理或重构改变 PA-07 已验收行为及 PA-06 授权边界。
- 当前结果：静态盘点报告已提交；不是测试结果，也不声称验证了并发缺陷。

### PA08-01 — Registry portability (foundation) — VERIFIED
- Workspace direct dependencies for `digua_common_jwt`, `digua_common_consul`, and `digua_file_sdk` use crates.io 1.7.0 with `default-features = false` and `tls-native`.
- `.cargo/config.toml` no longer defines the Kellnr registry or rsproxy source replacement.
- Cargo.lock source identifiers for all four Digua packages, including transitive `digua_http_client`, point to canonical crates.io; checksums match the published index.
- Hosted verification: [GitHub Actions Run #4](https://github.com/luqizheng/ai-pm/actions/runs/44967203110) completed successfully. Locked metadata, all four registry-source checks, and `cargo check --workspace --locked` passed.
- Follow-up CI portability: `.github/workflows/pa07-blast-radius.yml` was moved from the now-unavailable `self-hosted, internal-registry` labels to `ubuntu-latest` for both jobs. The prior queued runs were infrastructure-blocked, not test failures; the new Hosted regression run is [Run #25](https://github.com/luqizheng/ai-pm/actions/runs/44968929358). PostgreSQL integration still explicitly skips unless `AIPM_TEST_DATABASE_URL` is configured.

### PA08-02 — Runner lifecycle test harness — VERIFIED (initial acceptance slice)
- Keep the production Runner state machine and MCP contract authoritative; add the smallest test seams needed rather than rewriting the execution model.
- Initial integration slice added at `apps/ai-pm-runner/tests/pa08_lifecycle.rs`: production `claim_and_start` is exercised against scripted Wiremock MCP responses for ready-task discovery, claim, run-context retrieval, lease-owner routing identity, and real child-process spawn/cleanup. Empty-queue behavior is also covered.
- A production `tick` test advances elapsed-time fields directly and verifies heartbeat renewal followed by command polling through the real MCP client, without wall-clock sleeps.
- CI now runs `cargo test -p ai-pm-runner --tests --locked` after the workspace build. The initial claim/spawn + empty-queue slice passed [Run #7](https://github.com/luqizheng/ai-pm/actions/runs/44968483995); the expanded tick test passed in [Run #10](https://github.com/luqizheng/ai-pm/actions/runs/44968579673).
- Runner does not call `pm_submit_result`: Agent submits delivery through MCP directly. Tests must not invent a Runner-side submit step; cover Agent submission separately at its protocol/contract boundary.
- Additional command-path tests exercise real child-process termination + ack through production polling, continue spawning + human-reply injection through production polling, and frozen-guard refusal. The command-path suite passed in [Run #19](https://github.com/luqizheng/ai-pm/actions/runs/44968871944). Heartbeat retry hardening (clock advances only after Platform confirms renewal) and its HTTP 503 regression test passed in [Run #25](https://github.com/luqizheng/ai-pm/actions/runs/44969004043). Restart recovery/reacquire, empty discovery, and spawn failure passed in [Run #28](https://github.com/luqizheng/ai-pm/actions/runs/44969367934).
- PA08-02 follow-up now covers delayed duplicate `agent.task.continue` delivery (the existing live Agent session is acknowledged without spawning a second process) and command-poll HTTP 503 retry (failed polls do not advance the poll clock). Both new production-path integration tests, plus the full Runner suite and the PA08 Hosted workflow, passed [Run #44979290867](https://github.com/luqizheng/ai-pm/actions/runs/44979290867).
- Current PA08-02 P0 hardening: startup recovery now returns a typed failure for RPC/transport/malformed discovery responses and only accepts an explicit `runs: []` as empty; lease responses require a nonempty token/worker ID and positive generation, and active reacquisition must advance generation. Runner startup identity-checks and terminates orphaned process groups before recovery; failed orphan cleanup writes `cleanup-needs-human.json` and blocks startup. Lease-loss cleanup retains the Child handle and retries termination/reap while blocking command processing and new claims. Continue and Terminate ACK 503/redelivery tests now verify no duplicate Agent process/session. These changes are awaiting latest Hosted validation.
- Next refactor target: isolate time/scheduling decisions and process lifecycle operations behind narrow seams only where needed by the remaining tests. Do not introduce production sleeps or a parallel test-only state machine.

### PA08-03 — Run transition guard — VERIFIED
- Static source inventory and implementation evidence: `PA08-03-RUN-TRANSITION-INVENTORY.md`.
- Added `run_transition_guard.rs`: typed RunKind/RunStatus parsing, separate Delivery/Reviewer matrices, terminal immutability, and transaction-scoped `SELECT ... FOR UPDATE` validation.
- Integrated into delivery claim/start/requeue/isolation/cancel/finish/termination, MCP first-start, Project archive, and Reviewer claim/success/failure/timeout paths. Existing Task transitions remain routed through `apply_task_transition`.
- Project archive is the only allowed active Reviewer Run cancellation reason; Reviewer lease/session and verdict semantics remain isolated.
- Real PostgreSQL evidence: [Run #52](https://github.com/luqizheng/ai-pm/actions/runs/44970075346) passed guard integration + unit tests; [Run #57](https://github.com/luqizheng/ai-pm/actions/runs/44970217926) passed again with the active Reviewer archive cancellation scenario.
- Latest Project-scoped infrastructure exhaustion compare-and-set hardening is pending [Run #58](https://github.com/luqizheng/ai-pm/actions/runs/44970354176). Remaining gates: claim-vs-archive and stale lease token/generation concurrency tests.

### PA08-04 — Workspace / mirror reclamation — WORKER WIRED; DEFAULT-OFF; HOSTED VERIFIED
- Added migrations `0054` and `0055`: atomic process-cleanup claims with lease/fencing, and a separate worktree/mirror reclamation ledger.
- Candidate staging is idempotent and metadata-only. Default retention is 30 days; Run worktrees require terminal retention and completed process cleanup; archived mirrors require no active Run or unfinished cleanup reference.
- CodexExecutor has guarded worktree/mirror primitives: canonical path checks, symlink rejection, shared mirror lock, Git worktree remove/prune, and linked-worktree refusal before mirror deletion. Mirror locking now uses persistent OS-backed advisory file locks (no mtime stale-lock stealing or unlink/recreate race); a stale-mtime contender regression test and Windows-specific Hosted job cover the kernel lock behavior.
- Worker now runs both Run-worktree and Repository-mirror rounds. Mirror reclamation locks the Repository row and keeps that transaction open through the filesystem adapter, serializing against Repository archive/update writes; the existing mirror lock and Git linked-worktree checks remain in force.
- File deletion is disabled by default. Enablement requires `AI_PM_WORKSPACE_RECLAMATION_ENABLED=true`, `AI_PM_WORKSPACE_RECLAMATION_SHARED_ROOT=true`, and an absolute `AI_PM_WORKSPACE_ROOT`. The Worker performs fenced claims and a second DB eligibility check before filesystem operations.
- Added `needs_human` listing and explicit operator requeue application operations. Project-scoped process-cleanup escalation is exposed at `GET /v1/pm/task-run-cleanups/needs-human`; the API first checks project visibility and the application query repeats tenant/project scope in SQL.
- Workspace reclamation completion records logical file bytes removed in migration `0056`; CodexExecutor measures regular-file logical sizes without following symlinks. The metric `ai_pm_workspace_reclaimed_bytes_total` is exported by the API. This is logical file-size accounting, not physical disk-block reclamation.
- Earlier complete validation: [Run #117](https://github.com/luqizheng/ai-pm/actions/runs/44973471988). Expanded mirror-lock, worker, ledger, process-cleanup and Codex path-safety tests all pass in [Run #150](https://github.com/luqizheng/ai-pm/actions/runs/44975873084). The byte-accounting, needs-human API, workspace build, Runner integration and PostgreSQL ledger suites passed [Hosted Run](https://github.com/luqizheng/ai-pm/actions/runs/44978431052). New OS-backed lock, orphan cleanup/fencing, and command ACK redelivery tests are awaiting current PA08/PA07 Hosted regression results.
- File deletion remains opt-in and disabled by default. Reclamation backlog and cumulative logical reclaimed bytes are included in the PA08-06 API metrics endpoint.

### PA08-05 — Product lifecycle integrity — IMPLEMENTED; HOSTED VERIFIED
- Added an explicit source-state/action matrix from PA-07 §§4/11/12: `normal → suspended → resuming → normal`, with `resuming → suspended` for a renewed pause and `abandon → archived` from non-archived states.
- Archived is terminal for ordinary lifecycle operations. Retained legacy `restart` only as a compatibility alias for `resuming → normal` (migration 0044); it is rejected from `suspended`/`archived`, so it cannot bypass Portfolio Review or restore archived Products.
- Product Owner authorization, Product row lock, source-state validation, status update, and lifecycle audit remain one transaction. Invalid transitions have no DML or audit side effects.
- Added API integration coverage for the complete suspend/resume sequence, invalid source-state actions, archived terminal behavior, and audit invariance. See [PA08-05 Product Lifecycle Inventory](PA08-05-PRODUCT-LIFECYCLE-INVENTORY.md).
- Hosted verification: [Run #150](https://github.com/luqizheng/ai-pm/actions/runs/44975873084) passed the Product lifecycle API and Product gate vs Run claim concurrency suites.


### PA08-06 — Runtime observability — PARTIAL IMPLEMENTATION
- `pm-api` 已新增受现有 Bearer 鉴权保护的 `GET /metrics`，输出 Prometheus text exposition；数据库聚合覆盖 Run 状态/活跃年龄、process-cleanup 状态、workspace reclamation 状态，以及现有 communication overview。
- Label 仅使用 Run kind/status、cleanup status、resource kind/status 等有限状态维度；不输出任何 entity ID、用户内容、仓库 URL、凭据或原始错误。
- 数据库 scrape 查询失败返回 503，不会把旧值伪装成当前值。指标实现与转义测试见 `apps/pm-api/src/metrics.rs`。
- Workspace reclamation 已记录每次成功删除的逻辑文件字节数（migration `0056`），并输出 `ai_pm_workspace_reclaimed_bytes_total`。该值不是物理磁盘块回收量。
- Process-cleanup `needs_human` 列表已实现为项目范围 API：`GET /v1/pm/task-run-cleanups/needs-human?project_id=...`；项目可见性与 SQL tenant/project scope 双重约束。
- 上述 reclaimed-byte accounting、needs-human API、metrics exposition、workspace build、Runner tests 及 PostgreSQL suites 已由 Hosted [Run #166 follow-up](https://github.com/luqizheng/ai-pm/actions/runs/44978431052) 验证通过。
- Worker loopback Prometheus listener 已实现，默认绑定 `127.0.0.1:9465`，可由 `AI_PM_WORKER_METRICS_ADDR` 覆盖；已接入 AI analysis/repo import claim、Run dispatch attempt/error、Run poll error、cleanup error 与 scrape counters，并补充 `/metrics` 200、非目标路径 404 的 HTTP 测试。指标不含实体 ID、凭据或高基数标签。
- Worker cleanup failure counter 已接入 run-cleanup round 与 finished-process cleanup 两条错误路径。Worker exporter 当前提交仍须由最新 Hosted CI 验证。
- 尚未完成：Runner MCP 操作 counters/可选 exporter、Worker workspace filesystem capacity/available-space gauges、告警规则/阈值、API scrape/endpoint integration tests。故 PA08-06 不得标记完成。

## 4. 全局红线

- 不改变 PA-06 人类/AI 授权边界；AI 不得获得额外状态变更权。
- 不改变 PA-07 Task / Run / Product / Workflow 状态隔离原则。
- 不自动重开终态 Task/Run，不将 Run 成功等同 Task 完成。
- 不删除活跃或被引用的 Workspace、mirror、Repository、Commit、交付物及审计历史。
- 不引入 Kafka、分布式锁服务、图数据库、独立事件溯源平台或 Runner HA。
- 不做旧数据兼容层；遵循仓库 AGENTS.md 新项目规则。
- 所有数据库行为以真实 PostgreSQL 集成测试验证；关键安全/并发路径不能只靠 mock。

## 5. 验收门槛

PA-08 不以“代码已合并”作为完成定义。每个工作流必须提交：
1. 实施差异与受影响文件清单；
2. 对应状态机/事务/权限边界说明；
3. 正常、异常、并发、重试测试证据；
4. 本地真实测试命令和结果；
5. CI 运行链接或明确的外部阻塞；
6. 未完成项、风险和后续建议。

整体验收要求：
- Registry portability 在 GitHub Hosted Runner 上可解析和构建，不依赖 self-hosted 内网环境。
- Runner 生命周期、Run guard、资源回收、Product lifecycle 各自的专项测试全部通过。
- 现有 PA-07 回归套件保持通过；Q10 测试遵守 AGENTS.md 独立执行规则。
- 资源回收默认 dry-run，启用真实删除前必须证明二次校验和引用保护有效。
- 指标 endpoint 不泄漏密钥、用户数据或高基数标签。

## 6. 当前状态

PA08-01 已由 Hosted CI 验证通过。PA08-02 基础生命周期测试已由 Run #28 验证；延迟重复 continue 与命令轮询 HTTP 503 即时重试补充测试及完整 Runner suite 已由 Hosted Run #44979290867 验证通过。更广泛的 lease/transport 故障注入仍待补齐。PA08-03 转换守卫已由 Run #63 完整验证（真实 PostgreSQL 七项集成测试 + 单元测试）。PA08-04 与 PA08-05 已由 Hosted CI Run #150 验证通过（包含回收 Worker、PostgreSQL 账本、Product lifecycle API 与 Product gate vs Run claim 并发测试）；reclaimed-byte accounting 与 process-cleanup needs-human API 后续由 Hosted Run #44978431052 验证通过。PA08-06 API 聚合指标、回收字节指标、needs-human API 已由 Hosted Run #44978431052 验证；Worker/Runner 运行指标、Worker exporter、文件系统容量指标、阈值告警及完整 endpoint/scrape 集成测试仍未完成。PA08-04 文件删除默认关闭。本文件状态以代码、测试和 CI 证据为准。
