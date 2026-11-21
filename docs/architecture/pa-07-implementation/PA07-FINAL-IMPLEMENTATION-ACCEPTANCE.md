# PA-07 最终实施验收报告（FINAL IMPLEMENTATION ACCEPTANCE）

- **任务：** PA-07 剩余实施核验、测试执行、缺陷修复与最终验收（用户 2026-11-22）
- **日期：** 2026-11-22
- **核验 HEAD：** `6ed0f1a`（核验与修复的工作树状态，随本报告一并提交，commit 见 §10）
- **证据链：** [PA07-IMP-001](PA07-IMP-001-schema-mapping.md)（差距快照）→ [PA07-IMP-002](PA07-IMP-002-implementation-audit.md)（历史审计）→ [PA07-IMP-003](PA07-IMP-003-current-state-verification.md)（当前逐项核验）→ 本报告（验收结论）

---

## 1. 实际核验的 Git HEAD

`6ed0f1a`（fix(pa-07): E2「AI 永不置 ready」命令通道缺口单点封堵）。本批修复以工作树状态在该 HEAD 之上完成并随本报告提交（blast radius / IMP-008 SSH RAII / IMP-009 device 单活跃 / CI 工作流 / 文档）。工作区核验时点干净（无用户未提交修改被覆盖）。

## 2. IMP-001~019 核验矩阵与统计

完整矩阵（代码位置/当前实现/测试证据/状态）见 [IMP-003 §4](PA07-IMP-003-current-state-verification.md)。统计：

| 状态 | 数量 | 明细 |
|---|---:|---|
| VERIFIED | **12** | 001 · 002 · 003 · 004 · 005 · 006 · 007 · 016 · 019（P0 全量+016/019，当前 HEAD 重验）+ **008 · 009（本批修复后 VERIFIED）** + E2/BlastRadius 专项 |
| PARTIAL | 5 | 011（Runner 主循环单测）· 012（Run 集中 guard）· 013（lifecycle 源状态+E2E 场景 4-6）· 015（token 预算）· 017（credential TTL） |
| NOT_IMPLEMENTED | 3 | 010（Workspace/mirror 回收，P1）· 014（Submit 事务缩短，P2）· 018（可观测性，P2） |
| IMPLEMENTED_UNVERIFIED / DEFECT / BLOCKED / INHERITED | 0 | 历史审计 ⚠️/❌ 十项全部在当前 HEAD 重验，未沿用旧结论 |

相对 IMP-002（✅9/⚠️5/❌5）的变化：❌ 5 → 008/009 本批修复转 VERIFIED，010/014/018 维持已登记排期债；⚠️ 5 维持 PARTIAL（证据更新至当前 HEAD）。

## 3. 已验证的实现

- **P0 数据模型与门禁全套**：Task Revision（0042 不可变+Run 钉定+CAS）、Dependency（0043 条件边+advisory lock+CTE 检环+并发测试）、Run 启动门禁快照（0044 四门同事务）、Product 四态与验收单点门（0044/0045 + `transition.rs` acceptance_gate）、Human Consultation（0046 四态+DB CHECK 完整性）、审计闭环、DNS rebinding 防护（mcp_hosts）、Product Key（0049）。
- **E2「AI 永不置 Ready」**（6ed0f1a + 本批复核）：单点执法于 `apply_task_transition`（唯一状态入口，全仓唯一裸写点）；四类入口（REST/E5/命令通道/Service 解锁）收口；AiPm/DevAgent 拒绝且状态不变；3 边界测试 + 12 套件回归绿。
- **Blast Radius（本批修复）**：`item_count()`+`round_blast_radius` 整轮累计；任务书八项验证点全部有测试（单测 5+端到端 2）；`create_tasks` 维持非红线（PA-06-04 裁决不变），红线优先级不变。

## 4. 本批修复的缺陷

| # | 缺陷 | 修复 | 回归 |
|---|---|---|---|
| 1 | blast_radius=proposals.len()（IMP-002 §4 P2）：单条 CreateTasks 含 N 任务时 radius=1，批量闸失效；拆分提案亦可绕过 | `ProjectCommand::item_count()`（批量按载荷长度）+ `self_review::round_blast_radius`（整轮累计）+ orchestrator 接线 | pm-domain 2 + pm-ai lib 3 + orchestrator E2E 2（11→escalated / 10→applied）全绿 |
| 2 | IMP-008：CodexExecutor 镜像路径明文写共享 `keys/{repo}.pem` 无删除路径 | 改复用 `git_access::write_temp_key` RAII（0700 临时目录，守卫随 git 子进程作用域，Drop 连目录删除） | codex_executor lib 2 新单测 + 7 集成回归绿 |
| 3 | IMP-009：无 per-device 单活跃 Run 服务端约束（双 runner 进程可并发领两条 Run） | 迁移 0053 部分唯一索引 `uq_task_run_device_active ON (lease_client_id) WHERE status IN ('claimed','running')`（DB 级，多槽位须显式改约束） | `device_single_active_run_is_enforced_server_side`（tokio::join! 并发恰一成功+串行拒+终态释放+异设备不受限）+ run_dispatch 32/32、device_credential/acp/run_context 回归绿 |

## 5. 未验证的实现 / 确认存在的缺陷

无未知缺陷。已确认未实现项=§2 的 3 个 NOT_IMPLEMENTED + 5 个 PARTIAL，全部为 IMP-001 快照即已登记的排期债（P1/P2），非本批回归；逐项现状证据见 IMP-003 §4，处置建议见 IMP-003 §8（O-1~O-10）。

## 6. 测试命令、环境与真实结果

**环境**：本地 Windows（可访问私有 Registry：Kellnr sparse `192.168.2.103:16444` 4 个 digua_* crate 解析成功）+ PostgreSQL 16 容器 `127.0.0.1:16433`（max_connections=400 / max_locks_per_transaction=512）；`CARGO_BUILD_RUSTC_WRAPPER=""` 旁路 sccache；Q10 按仓库规则单独跑。

| 命令 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | **存量红**：164 文件 fmt 漂移=HEAD 既有卫生债（本批改动文件全部 fmt-clean；不重排全仓避免无关 diff） |
| `cargo metadata --format-version 1 --no-deps` | ✅ exit 0 |
| `cargo test -p pm-domain` | ✅ **92/92** |
| `cargo test -p pm-ai` | ✅ lib **131/131** + 集成 **50/50**（orchestrator 10=8 既有+2 新） |
| `cargo test -p pm-application --no-fail-fast -- --skip q10` | ✅ 全绿（含 command_executor 13、run_dispatch 32、task_dependency 18+9、run_start_gates 3、task_revision 5、task_product_acceptance 6、human_consultation 3、state_transition_guard 4、review 15、delivery 24） |
| `cargo test --workspace --no-fail-fast -- --skip q10` | **1907 pass / 2 fail**（明细见下） |
| `cargo test -p pm-application --test q10_race_closure --test q10_r3_snapshot_check --test q10_classify_matrix` | ✅ **18/18**（11.7s/9.8s/8.1s，无挂死） |

**全量 2 个失败的定性与复验**（均与本批改动无关——本批 diff 不触碰 grant/tenant 与渲染代码路径）：

| 失败 | 定性 | 单独复跑 |
|---|---|---|
| `pm-api project_members::tenant_removal_racing_grant_has_single_final_state` | 并行全量下 DB 资源竞争导致的竞态类 flaky（与 AGENTS.md 规则 6 记录的 q10 同类现象） | ✅ 1/1 绿（5.4s） |
| `pm-infrastructure viz_seecode_adapter::real_render_is_deterministic` | 已知环境 flaky：Google Fonts 实时子集字节漂移（2026-11-19 GAP-32 关闭时定案留档） | ✅ 1/1 绿（1.7s） |

附注：Q10 三套件在本次全量中因测试名不含 `q10` 前缀未被 `--skip q10` 过滤、实际随全量执行且全绿（18/18），后又按规则单独复跑同样全绿——规则 6 的过滤写法对这批测试名不生效，建议后续把该三套件用例名补前缀或改用 `--skip` 目标名列表（登记 IMP-003 §8 建议，不阻塞验收）。

## 7. CI 执行状态

任务书所述 `.github/workflows/pa07-blast-radius.yml` **在核验起点不存在**——本批新建（内容核验见 [IMP-003 §5](PA07-IMP-003-current-state-verification.md)：YAML 解析通过、触发条件=workflow_dispatch+PR 路径过滤、测试过滤名逐一匹配、`RUSTC_WRAPPER=env` 直通透传正确、`--locked` 锁定依赖）。

**执行状态：未运行（awaiting internal runner）。** 该工作流 `runs-on: [self-hosted, internal-registry]`——GitHub hosted runner 无法访问私有 Registry（Cargo.lock 4 个 kellnr crate），hosted 构建必死于网络错误，那是网络限制不是代码失败，不得据此判红代码。在内部机器打上标签即可作为定向回归入口；DB 依赖的集成 job 由 `vars.AIPM_TEST_DATABASE_URL` 门控，未配置时显式 SKIP（非假绿）。未删任何测试换取 CI 变绿。

## 8. 安全 / 并发 / 事务风险

**安全**：
- IMP-008（SSH 明文残留）本批修复——镜像 fetch/clone 路径密钥 RAII 化；submit 远端校验路径 RAII 先例未动。残余风险=Windows 无 POSIX 权限语义（0700/0600 退化，与既有 `write_temp_key` 同口径）。
- DNS rebinding（mcp_hosts 白名单）与 Device credential 撤销/轮换维持 VERIFIED/PARTIAL 现状；credential 无 TTL（O-8）。
- Git 命令执行参数化（无 shell 注入面）、路径越界防护为存量已测能力（e2e/security 套件绿）。

**并发/事务**：
- IMP-009 本批修复后，per-Task（uq_task_run_active）+ per-device（0053）双约束在库层封死重复执行面；fencing/generation 迟到事件防护有测试（run_dispatch 32/32）。
- IMP-014（submit 长事务内 git 子进程）维持未修——行锁持有期跑网络子进程的窗口仍在，是最大的已知事务债（P2 排期，O-2）。
- Run 终态不可被迟到事件覆盖：fencing 测试绿；集中 guard 缺失为 PARTIAL（O-5）。

## 9. 剩余 OPEN 项

O-1~O-10 全量清单见 [IMP-003 §8](PA07-IMP-003-current-state-verification.md)：P1×4（010 回收 / 011 主循环单测 / 012 Run guard / 013 lifecycle 源状态+E2E）、P2×3（014 事务 / 015 token 预算 / 018 可观测）、低×3（017 TTL / 仓库级 fmt 漂移 / IMP-002 P3-P5 文档与执法窄口）。**无 P0 级 OPEN 项。**

## 10. 最终验收结论

### **APPROVED_WITH_LIMITATIONS**

**依据**：
- 全部 P0 关键要求（IMP-001~007 + 016 + 019）有充分代码与测试证据（当前 HEAD 重验，非沿用历史审计）；
- 两项指定复核（E2 / Blast Radius）均 VERIFIED，其中 blast radius 为本批修复并补齐八项验证点测试；
- 上批 P1 安全债两项（008 SSH 明文 / 009 device 单活跃）本批修复且回归绿；
- 全量真实测试 1907 pass，仅 2 个已定性环境 flaky（均单独复跑绿，与本批改动无关），Q10 单独 18/18 绿；
- PA-06 授权边界全程未动（create_tasks 红线维持摘除裁决，本批只收严批量闸、未放宽任何红线）。

**限制（非关键、已记录）**：剩余 3 项 NOT_IMPLEMENTED（010/014/018）与 5 项 PARTIAL（011/012/013/015/017）为 P1/P2 排期债，处置建议与优先级已在 IMP-003 §8 登记；其中 010（磁盘回收）与 014（长事务）建议列为下一批 P1/P2 首位。CI 待内部 runner 接入后生效。仓库级 fmt 漂移（164 文件）建议独立 chore 批次清理。

**未达 APPROVED 的原因**：按任务书口径 APPROVED 要求「全部关键要求均有充分代码和测试证据」——上述 P1 执行安全/运维债虽为已排期事项，但按不降低验收标准原则，如实计入限制项。
