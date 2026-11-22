# PA-08 — Execution Reliability & Runtime Operations

- **状态：** IMPLEMENTATION IN PROGRESS (PA08-01 VERIFIED; PA08-02 lifecycle slice VERIFIED; PA08-03 VERIFIED)
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
- Additional command-path tests exercise real child-process termination + ack through production polling, continue spawning + human-reply injection through production polling, and frozen-guard refusal. The command-path suite passed in [Run #19](https://github.com/luqizheng/ai-pm/actions/runs/44968871944). Heartbeat retry hardening (clock advances only after Platform confirms renewal) and its HTTP 503 regression test passed in [Run #25](https://github.com/luqizheng/ai-pm/actions/runs/44969004043). Restart recovery/reacquire, empty discovery, and spawn failure are now covered by production-path tests and passed in [Run #28](https://github.com/luqizheng/ai-pm/actions/runs/44969367934), including `cargo check --workspace --locked` and `cargo test -p ai-pm-runner --tests --locked`. This verifies the Runner lifecycle slice implemented so far; delayed/duplicate command responses and broader lease/transport failure injection remain follow-up coverage.
- Next refactor target: isolate time/scheduling decisions and process lifecycle operations behind narrow seams only where needed by the remaining tests. Do not introduce production sleeps or a parallel test-only state machine.

### PA08-03 — Run transition guard — VERIFIED
- Static source inventory and implementation evidence: `PA08-03-RUN-TRANSITION-INVENTORY.md`.
- Added `run_transition_guard.rs`: typed RunKind/RunStatus parsing, separate Delivery/Reviewer matrices, terminal immutability, and transaction-scoped `SELECT ... FOR UPDATE` validation.
- Integrated into delivery claim/start/requeue/isolation/cancel/finish/termination, MCP first-start, Project archive, and Reviewer claim/success/failure/timeout paths. Existing Task transitions remain routed through `apply_task_transition`.
- Project archive is the only allowed active Reviewer Run cancellation reason; Reviewer lease/session and verdict semantics remain isolated.
- Real PostgreSQL evidence: [Run #52](https://github.com/luqizheng/ai-pm/actions/runs/44970075346) passed guard integration + unit tests; [Run #57](https://github.com/luqizheng/ai-pm/actions/runs/44970217926) passed again with the active Reviewer archive cancellation scenario.
- Latest Project-scoped infrastructure exhaustion compare-and-set hardening is pending [Run #58](https://github.com/luqizheng/ai-pm/actions/runs/44970354176). Remaining gates: claim-vs-archive and stale lease token/generation concurrency tests.

### PA08-04 — Workspace / mirror reclamation
- 先定义可回收候选：没有活跃 Run、没有数据库引用、已超过明确保留期、路径位于受控 workspace 根目录。
- 实施只读 inventory 与 dry-run；执行前再次核对引用/活跃状态，防止 TOCTOU。
- 删除采用限批、幂等、可重试流程；记录候选、跳过原因、删除结果和失败原因。
- 默认保留安全优先：无法确定归属、引用状态或路径安全时一律跳过，不推测删除。
- 覆盖活跃 Run、终态 Run、共享 mirror、多仓库引用、路径越界、并发 claim、删除失败及重试。

### PA08-05 — Product lifecycle integrity
- 先从 PA-07 目标状态图映射当前字段与迁移；不把业务 lifecycle 与 execution state 混为一谈。
- 对所有状态转换校验源状态、目标状态和允许的操作者/前置条件；非法跳转返回稳定错误且无副作用。
- 通过事务锁/版本检查处理 Product 状态变化与 Run claim/start 竞争。
- 补齐 IMP-003 §12.3 场景 4–6 以及重复请求、并发暂停/恢复、恢复中再次暂停的 E2E。
- 不批量改写 Task 状态；Product 状态与 Task/Run 状态继续隔离。

### PA08-06 — Runtime observability
- 提供可抓取的 Prometheus 格式指标或仓库既有指标协议；优先复用现有依赖和运行时。
- 至少覆盖 Run 状态数量/持续时间、claim/heartbeat/submit 结果、Runner loop 错误、cleanup backlog/needs-human、workspace/mirror 磁盘水位与回收量。
- 指标标签必须有界；禁止使用 run_id、task_id、project_id 等高基数标识作为 label。
- 对阈值只产生可操作的结构化告警/事件；不引入新消息平台或外部 SaaS 依赖。
- 提供健康检查/诊断说明及指标测试。

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

PA08-01 已由 Hosted CI 验证通过。PA08-02 当前实施切片已由 Hosted CI Run #28 验证通过；延迟/重复命令响应和更广泛故障注入仍作为后续增强。PA08-03 转换守卫已由 Run #63 完整验证（真实 PostgreSQL 七项集成测试 + 单元测试），Run #58 的 Project 范围 compare-and-set 收紧也已通过。PA08-04 已开始资源所有权盘点和 process-cleanup claim 缺陷复现；尚未实施文件删除。PA08-05 至 PA08-06 尚未开始。本文件状态以代码、测试和 CI 证据为准。
