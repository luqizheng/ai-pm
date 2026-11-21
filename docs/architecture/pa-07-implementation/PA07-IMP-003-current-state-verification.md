# PA07-IMP-003：当前差异核对报告（最终实施核验）

- **任务：** PA-07 最终实施核验与验收（用户 2026-11-22）
- **日期：** 2026-11-22
- **核对基准：** HEAD `6ed0f1a` + 本批修复（blast radius / IMP-008 / IMP-009 / CI 工作流，随本报告同批提交）；migrations 0001–0053
- **方法：** 逐项代码证据核对（双代理独立盘点 ❌/⚠️ 十项 + P0 项锚点复核）+ 全量真实测试（本地私有 Registry 环境）
- **上游：** [PA07-IMP-002-implementation-audit.md](PA07-IMP-002-implementation-audit.md)（2026-11-21，基准 75094e1 的历史审计）；本报告为当前 HEAD 的差异核对，历史审计结论未重验处标 INHERITED

## 1. 本批代码变更（相对 6ed0f1a）

| # | 变更 | 位置 | 动机 |
|---|---|---|---|
| 1 | **Blast radius 按实际影响条数计**（IMP-002 §4 P2 修复） | `pm-domain/src/command.rs`（`ProjectCommand::item_count`）· `pm-ai/src/self_review.rs`（`round_blast_radius`）· `pm-ai/src/orchestrator.rs`（接线） | 旧 `blast_radius = proposals.len()` 使单条 CreateTasks 含 N 任务时 radius=1，批量闸永不触发 |
| 2 | **SSH 私钥 RAII 清理**（IMP-008 修复） | `pm-infrastructure/src/codex_executor.rs` | 旧实现明文写共享 `keys/{repository_id}.pem` 无删除路径；改复用 `git_access::write_temp_key`（0700 临时目录，Drop 连目录删除） |
| 3 | **per-device 单活跃 Run 服务端约束**（IMP-009 修复） | `migrations/0053_device_single_active_run.sql` | 部分唯一索引 `uq_task_run_device_active ON (lease_client_id) WHERE status IN ('claimed','running')`——并发双 claim 仅一成功（DB 级），多槽位须显式改约束 |
| 4 | 定向回归 CI | `.github/workflows/pa07-blast-radius.yml` | 此前不存在；建为内部 runner 执行入口（hosted runner 不可达私有 Registry，见 §5） |
| 5 | 文档 | 本报告 + FINAL 验收 + IMP-002 状态更新 + README 索引 | 验收交付 |

## 2. E2（AI 永不置 Ready）复核 —— VERIFIED

6ed0f1a 的单点封堵在当前 HEAD 完整在位，且本次复核确认无绕过路径：

- **单点执法：** `crates/pm-application/src/command_executor/transition.rs:112-121`——`apply_task_transition`（R1 唯一状态入口）在 `validate_transition` 之后、状态推进之前拦截：`to == Ready && actor_kind ∉ {Human, Service}` → `CommandError::Validation("E2: ...")`，状态/revision 不变。
- **全仓唯一裸写点：** `UPDATE delivery.task SET status` 仅存在于该入口内部（transition.rs:138）；不存在第二条绕过状态机的写路径。
- **入口收口：** ①REST 人类面（`require_human_principal` + require_human）②E5 整批确认 `mark_task_batch_ready`（`pm_write/task.rs:564` require_human + 以 ActorKind::Human 经同一入口执行）③命令通道 `UpdateTaskStatus`（同一入口 actor 门）④依赖解锁 D7/D8（ActorKind::Service 放行，平台调度路径，设计允许）。AiPm/DevAgent 一律拒绝。
- **测试：** `crates/pm-application/tests/command_executor.rs:957-` 3 条边界测试（AiPm 拒/DevAgent 拒/Human 放）+ 7 处机械测试改 Human 后全绿（本批全量复跑，见 §4）。

## 3. Blast Radius 修复验证 —— VERIFIED

任务书 §4.2 的八项验证点全部落地为测试：

| 验证点 | 测试 | 结果 |
|---|---|---|
| 单条 CreateTasks 含 11 任务 → radius=11 | `pm-domain command::tests::item_count_counts_payload_items_for_batch_commands` + `pm-ai self_review::tests::round_blast_radius_counts_items_not_proposals` | ✅ |
| 10 任务不超阈值（10>10 为假） | `self_review::tests::round_blast_radius_escalates_batch_create_tasks_above_threshold`（10→Auto） | ✅ |
| 11 任务超阈值（→Escalate） | 同上（11→Escalate `blast_radius_above_10`） | ✅ |
| 多 Proposal 实际影响数累计 | `round_blast_radius_accumulates_across_proposals_no_split_bypass`（CreateTasks(6)+CreateTasks(5)=11） | ✅ |
| CreateMilestones/CreateRequirements/WriteDocuments 按载荷长度计 | `item_count_counts_payload_items_for_batch_commands`（3/2/1） | ✅ |
| 单对象命令恒为 1 | `item_count_is_one_for_every_single_object_command`（全变体枚举） | ✅ |
| Redline 优先级高于 blast radius | `red_line_wins_over_low_confidence_and_large_blast_radius`（既有，规则 1 短路不变） | ✅ |
| 拆分 Proposal 不绕过整轮阈值 | `round_blast_radius_accumulates_across_proposals_no_split_bypass`（11×单任务=11） | ✅ |

**端到端**（`pm-ai/tests/orchestrator.rs`，真实 Postgres）：`batch_create_tasks_with_11_items_escalates_on_item_count_not_proposal_count`——一揽子 11 任务提案 → proposal `escalated`、零任务落库、`escalated` 通知 reason=`blast_radius_above_10`、事件 `needs_clarification`；对照 `batch_create_tasks_with_10_items_auto_applies_at_the_threshold`——10 任务 → `executed`、11 行任务（种子 1+批量 10）、事件 `applied`。

**授权边界不变**：`create_tasks` 不入红线（PA-06-04 裁决维持——AI 提案建任务=执行性自主授权）；`policies/ai-pm-ai-lead-v2.yaml` 注释口径（「超大批量仍走 escalate（blast_radius>10）」）在 item_count 语义下按对象数成立，无需改约。纵深第二闸（`PolicyEngine::evaluate` 的 PolicyContext）同用累计值。

## 4. IMP-001~019 当前核验矩阵

状态口径：VERIFIED（代码+测试证据均充分，本批在当前 HEAD 重验）/ IMPLEMENTED_UNVERIFIED / PARTIAL / DEFECT / NOT_IMPLEMENTED / BLOCKED / INHERITED（仅继承历史审计，本次未重验）。历史审计为 ⚠️/❌ 的十项本批全部重新核验（代码位置+测试证据逐条复核），不沿用旧结论。

| ID | 目标契约 | 代码位置（当前 HEAD） | 当前实现 | 测试证据（本批实跑） | 状态 |
|---|---|---|---|---|---|
| 001 | Schema 核对 | migrations 0042–0049；报告即交付物 | P0 缺口全闭环（revision=0042/dependency=0043/门禁快照+四态=0044/验收=0045/consultation=0046/link 软删=0047/key=0049） | 迁移随全量 sqlx::test 建库执行 | VERIFIED |
| 002 | Task Revision 不可变 | 0042 表+指针；`run_dispatch/port.rs:33` Run 钉定 | 不可变版本行+Run 固定引用+CAS | task_revision 5/5 绿 | VERIFIED |
| 003 | Dependency DAG/门禁 | 0043 条件列；`task_dependency.rs`（advisory lock+递归 CTE） | 多前置 AND/检环/自环拒/并发串行化 | task_dependency 18/18 + concurrency 9/9 绿 | VERIFIED |
| 004 | Run 启动门禁快照 | 0044 快照表；`claim.rs:746` product 门；`port.rs:33` revision 钉定 | claim 同事务四门+快照 | run_start_gates 3/3 + run_dispatch 32/32 绿 | VERIFIED |
| 005 | Product 维度验收 | 0045；`transition.rs:126-136` acceptance_gate 单点 | Completed 前全部有效 Product 最新验收=approved；撤回不阻塞 | task_product_acceptance 6/6 + command_executor（含 E2/验收门）13/13 绿 | VERIFIED |
| 006 | 人工决策边界 | 0046 四态+DB CHECK（decided 四元组完整性）；挂 task 阻塞 | 决定人/内容/时间/范围落库；未回应≠同意 | human_consultation 3/3 绿 | VERIFIED |
| 007 | 审计闭环 | governance.audit_log 全链接线（revision/dependency/product/run/验收） | 六域可追溯 | 上述各套件断言审计行 | VERIFIED（小债：依赖边无原位变更审计位，只有删/建） |
| 008 | SSH 私钥 RAII | `codex_executor.rs`（**本批修复**） | 解密→`write_temp_key` 0700 临时目录→守卫与 git 子进程同作用域→Drop 连目录删除；共享 `keys/*.pem` 布局废弃 | codex_executor lib 2 条新单测（守卫存续/释放+https 不落盘）绿；submit 路径 RAII 先例 e2e 已有 | **VERIFIED（本批修复）** |
| 009 | per-device 单活跃 Run | `migrations/0053`（**本批修复**） | 部分唯一索引 DB 级约束；queued 未定设备/worker 领取不受影响；终态释放名额 | run_dispatch `device_single_active_run_is_enforced_server_side`（tokio::join! 并发双 claim 恰一成功+败者 run 不动+串行重试拒+终态释放+异设备不受限）绿；run_dispatch 32/32、device_credential 6/6、acp06_07 5/5、run_context 11/11 回归绿 | **VERIFIED（本批修复）** |
| 010 | Workspace/mirror 回收 | `run_cleanup.rs`（仅进程终止账本） | 保留期/水位回收**未实现**；mirror 只增不减 | cleanup 账本测试有，空间回收无 | NOT_IMPLEMENTED（P1 债） |
| 011 | Runner 主循环单测 | `apps/ai-pm-runner/src/runner_loop.rs:137` | 辅助函数+单拍级测试有；run_loop 全链（claim→spawn→heartbeat→command→submit→exit）与崩溃/迟到场景无测试驱动 | runner 内 3 单测+comm_loop 9+checkpoint2 7 | PARTIAL |
| 012 | Run 转换一致性 | Run 侧分散写：claim.rs:921 / run_executor.rs:170,730+ / lease.rs:67+ / archive.rs:87 | Task 侧集中守卫+矩阵；Run 侧靠 fencing（token/generation 行锁重验）+各写点行内条件，无集中 guard | state_transition_guard 4/4 + run_dispatch 32/32（含 fencing/stale/迟到路径级用例） | PARTIAL |
| 013 | Product 生命周期 E2E | `product.rs:773` set_product_lifecycle；`task_products.rs:229` 门禁 | 四态+REST+claim 门禁齐；**lifecycle 不校验源状态**（任意态可跳）；§12.3 场景 4-6 无端到端 | run_start_gates 3/3（单/双 Product+竞态）；resuming/级联零用例 | PARTIAL |
| 014 | Submit 事务缩短 | `delivery/mod.rs:395-656`（pool.begin :405 → commit :654） | **未实现**：行锁持有期间执行 git 子进程/远端校验（verify_repository_head/verify_remote_submission :560-620）；纯函数校验已挪事务外 | delivery 语义测试有，无事务时长断言 | NOT_IMPLEMENTED（P2 债） |
| 015 | LLM token 预算 | pm-ai 字符预算（discovery_context.rs）+超时（openai_adapter.rs:65）+usage 落库（call_record.rs） | 字符/超时/usage 有；**无请求级 max_tokens、无按模型/任务 token 上限、无成本封顶**（grep max_tokens/cost 零实现） | 预算单测 5 个（字符面）；token 上限无 | PARTIAL |
| 016 | DNS rebinding 防护 | `apps/pm-api/src/mcp_hosts.rs`（默认 loopback、永不空、精确端口） | 白名单防护在位 | mcp_hosts 3 用例（本批 pm-api 套件实跑绿，见 FINAL §测试） | VERIFIED |
| 017 | Device credential TTL | `device_credential.rs`（issue/revoke/resolve :273-309 四条件） | 撤销+两步轮换+每请求直查；**无 expires_at 列与到期失效**（0001 表结构+resolve 均无时间比较） | device_credential 6/6 绿（撤销/轮换/四条件；TTL 无代码故无用例） | PARTIAL |
| 018 | Runner 可观测性 | worker `comm_overview` 聚合日志（main.rs:219-244，注释自认「仓内零 metrics 基建」） | 聚合日志+日志上限 50MiB+needs_human 队列；**无 metrics 导出/磁盘水位/告警通道** | needs_human 队列测试有；metrics 无 | NOT_IMPLEMENTED（P2 债） |
| 019 | Product Key | 0049（NOT NULL+`uq_product_key ON lower(key)`+格式 CHECK+派生/兜底/冲突后缀） | 全套实现+API 带 key | product_domain 套件（3 key 用例，2026-11-21 实施时 11/11；本批全量含 product 域套件复跑绿） | VERIFIED |

**统计：VERIFIED 12 · PARTIAL 5 · NOT_IMPLEMENTED 3 · 其余 0。**（相对 IMP-002 的 ✅9/⚠️5/❌5：❌ 5→本批修复 008/009 两项、剩 010/014/018 为 P1/P2 排期债；⚠️ 5 项维持 PARTIAL 且证据更新至当前 HEAD。）

## 5. CI 工作流核验

任务书所述 `.github/workflows/pa07-blast-radius.yml` **在 6ed0f1a 不存在**（origin 与本地均无）——本批新建（内容见 §1）。核验结论：

- **YAML 语法：** 解析通过。
- **触发条件：** workflow_dispatch + pull_request（路径过滤命中 blast radius 相关五文件+工作流自身）。
- **命令与测试名匹配：** `cargo test --locked -p pm-domain item_count` / `-p pm-ai --lib self_review` / `-p pm-ai --test orchestrator batch_create_tasks`——与实际存在的过滤名逐一核对无误。
- **RUSTC_WRAPPER=env：** 环境变量优先于 `.cargo/config.toml` 的 `rustc-wrapper="sccache"`；值 `env` 为直通透传（`env rustc …`），未装 sccache 的机器可构建。本地已用 `CARGO_BUILD_RUSTC_WRAPPER=""` 等效禁用验证全量构建。
- **私有 Registry 限制：** Cargo.lock 含 4 个 kellnr crate（digua_common_consul/digua_common_jwt/digua_file_sdk/digua_http_client，sparse+http://192.168.2.103:16444）——GitHub hosted runner 不可达，**任何 hosted 构建都会死在依赖解析的网络错误上，这是网络限制不是代码失败**。处置：`runs-on: [self-hosted, internal-registry]` 保留为内部 runner 执行入口；DB 依赖的集成 job 以 `vars.AIPM_TEST_DATABASE_URL` 门控，未配置时显式 SKIP（warning），不假绿。

## 6. 与 PA-06 授权边界的一致性（复核）

- E2/E5 语义：见 §2，未动。
- create_tasks 红线摘除维持（PA-06-04 裁决）；本批 blast radius 修复**收严**了批量闸（对象数计），未放宽任何红线/授权——`policies/ai-pm-ai-lead-v2.yaml` 未改。
- IMP-009 约束只影响领取面（执行资源），不触碰业务授权。
- IMP-008 修复只改密钥落盘方式，不改变执行授权语义。
- `discovery_writeback` require_human 维持 2026-11-21 CLOSED 裁决口径（G-01 放开 create_task、闸位上移），本批未触碰。

## 7. 测试执行（真实环境）

环境：本机 Windows（Rust workspace），私有 Registry 可达（Kellnr sparse 192.168.2.103:16444 + rsproxy），PostgreSQL 16 @ `127.0.0.1:16433`（容器 aipm-dev-postgres，max_connections=400/max_locks_per_transaction=512），`CARGO_BUILD_RUSTC_WRAPPER=""`（本机 sccache 旁路）、Q10 三套件按 AGENTS.md 规则 6 从全量排除单独跑。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | **存量失败**：164 文件 fmt 漂移为 HEAD 既有状态（本批改动文件全部 fmt-clean；未重排全仓避免无关 diff 入库）——登记为仓库级卫生债 |
| `cargo metadata --format-version 1 --no-deps` | ✅ exit 0 |
| `cargo test -p pm-domain` | ✅ 92/92（含 item_count 新 2） |
| `cargo test -p pm-ai` | ✅ lib 131/131（含 round_blast_radius 新 3）+ 7 集成套件 50/50（orchestrator 10 含新 2） |
| `cargo test -p pm-application -- --skip q10` | ✅ 全绿（exit 0；套件级明细见 FINAL §8） |
| `cargo test --workspace --no-fail-fast -- --skip q10` | 见 FINAL §8（本报告提交前的最终全量数字以 FINAL 为准） |
| `cargo test -p pm-application --test q10_race_closure --test q10_r3_snapshot_check --test q10_classify_matrix` | 见 FINAL §8 |

## 8. 遗留 OPEN 项（按优先级）

| ID | 问题 | 级别 | 建议处置 |
|---|---|---|---|
| O-1 | IMP-010 Workspace/mirror 回收缺失（磁盘只增不减） | P1 | run_cleanup 账本扩一列保留期扫描：只清「无活跃 Run+无引用+超保留期」目录，dry-run+审计 |
| O-2 | IMP-014 Submit 长事务含 git 子进程（行锁持有期跑网络/子进程） | P2 | 分层：事务外先跑 git 校验拿事实，短事务内重验指纹后落库（保留 TOCTOU 防护） |
| O-3 | IMP-018 可观测性缺失（metrics/水位/告警） | P2 | 沿 comm_overview 结构化日志路线补 Prometheus 导出与磁盘水位 |
| O-4 | IMP-011 runner 主循环全链单测缺失 | P1 | 抽 run_loop 可注入时钟/命令源后补崩溃/迟到回报场景 |
| O-5 | IMP-012 Run 侧无集中 transition guard | P1 | 抽共享 guard 或统一矩阵测试（fencing 已覆盖竞态面） |
| O-6 | IMP-013 lifecycle 不校验源状态；§12.3 场景 4-6 无 E2E | P1 | product.rs 补源状态校验+补三场景测试 |
| O-7 | IMP-015 无 token 级预算/成本封顶 | P2 | openai_adapter 请求体加 max_tokens+按模型上限配置 |
| O-8 | IMP-017 device credential 无 expires_at | P2 | 加列+resolve 时间比较+轮换提醒 |
| O-9 | 仓库级 fmt 漂移（164 文件） | 卫生 | 独立 chore 提交一次性 `cargo fmt --all`，不与功能批混排 |
| O-10 | IMP-002 §4 P3/P4（Consultation Membership 执法、platform_flow 死值）与 P5 文档漂移 | 低 | 沿原报告建议批次处理 |

## 9. 结论

P0（001–007）+ 016 + 019 全部 VERIFIED；上批 P1 安全债 008/009 本批修复并回归绿；E2 与 blast radius 两项指定复核均 VERIFIED。剩余 3 项 NOT_IMPLEMENTED（010/014/018）与 5 项 PARTIAL 均为已登记排期债，无 P0 级未知缺陷。最终验收结论见 [PA07-FINAL-IMPLEMENTATION-ACCEPTANCE.md](PA07-FINAL-IMPLEMENTATION-ACCEPTANCE.md)。
