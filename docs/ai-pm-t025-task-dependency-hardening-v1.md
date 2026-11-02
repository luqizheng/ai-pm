# T-025：Task Dependency 并发与解锁正确性加固（任务书）

> 状态：**P1 四项全部完成并验收（2026-11-02：§三 方案 A / §五 行锁+图锁 / §六 跨租户 404 / §四 预算核实+C62，均见 §十）；C59–C66、C68 全绿，C67 按 P2 说明；遗留两项待裁（见 §10.9）**（编号核实 2026-11-01：`todo.md` 中 T-025~T-029 段位空缺，暂用 **T-025**，落在 M2 段位、与 T-023 设计稿 → T-024 实施 → T-025 加固语义连续；如需换号，本文档与 `todo.md` 登记行各改一处）
> 类型：专项审查 + 最小修复（T-024 追补，**不推翻现有架构**）
> 来源：专家评审 `docs/ai-pm-task-dependency-review.md`（2026-11-01）§9.1 P0 ×2 + §9.2 P1 ×4；任务定义由用户最终裁定
> 前置：T-023 / T-024 已交付归档（`docs/ai-pm-task-dependency-v1.md` §10；`todo.md` §13）
> 一句话：基于 T-024 已实施的 Task Dependency v1，进行一次针对生产环境正确性的专项审查和修正——**先审查报告，后实施修复**，最小范围，并发行为必须以真实并发测试验证。

---

## 0. 目标与硬边界

**目标**（用户裁定原文）：

> 基于 T-024 已实施的 Task Dependency v1，进行一次针对生产环境正确性的专项审查和修正。
>
> 禁止推翻现有架构，禁止重新设计依赖边表，优先进行最小范围修复。

**范围**：评审文档 §9.1 P0 两项（并发建边环检测、建边与终态迁移竞态）+ §9.2 P1 四项（Blocked 解锁条件、`dependency_unmet` 计数、终态写入入口审计、租户隔离）。评审文档 §9.3 的 P2 六项（依赖满足策略可配置、解锁审计增强、依赖图查询、大规模解锁优化、依赖类型扩展、`try_unblock_task` 抽象）**不在本次范围**。

**流程**：按 `todo.md` §0.1 执行——审查报告先行（读代码 + 构造并发验证）；修复方案（锁协议选型、Blocked 判定模型修正）涉及设计决策，须经用户确认后方可进入编码。

---

## 一、并发建边与环检测（P0 ↔ 评审 §2.1 / C59）

检查 `add_dependency`（`crates/pm-application/src/task_dependency.rs`）中递归 CTE 的并发安全性。

重点验证：

- 两个事务同时添加 A → B、B → A 时，是否可能同时通过环检测；
- 不同事务同时修改同一个项目的依赖图时，是否可能产生环。

如果存在风险，设计项目级事务锁或等效串行化机制。要求：

- 所有建边入口使用统一锁协议；
- 环检测与插入边处于同一事务；
- 不影响不同项目之间的并发操作；
- 不引入新的死锁风险。

（评审给出的候选方案：`pg_advisory_xact_lock(hashtextextended($project_id::text, 0))`，且锁必须在任何任务行锁**之前**获取，避免锁顺序反转——最终方案以审查报告 + 用户确认为准。）

---

## 二、建边与上游终态迁移竞态（P0 ↔ 评审 §2.2 / C60）

检查 `add_dependency` 与 `unlock_downstream` 的事务隔离及行锁行为。

构造以下并发场景：

1. B 为 `blocked`，A 为 `running`；
2. T1 添加 B → A；
3. T2 将 A 转为 `completed`；
4. 两个事务交错执行并提交。

**确保最终 B 一定能被正确解锁。**

检查所有锁的获取顺序，避免通过新增行锁或 advisory lock 引入死锁。

---

## 三、Blocked 解锁语义（P1 ↔ 评审 §3.1 / C61）

检查当前 `unlock_downstream` 和 `unlock_task_if_ready` 是否可能解除非依赖原因造成的阻塞。

特别关注 `task_waiting.reason` 的四类等待：

- `needs_info`
- `needs_decision`
- `needs_approval`
- `external_dependency`

**确保依赖满足不会错误地解除其他有效阻塞条件。**

如果当前模型无法区分阻塞来源，提出最小兼容的领域模型修正方案（评审倾向方案 B：统一 Blocker 聚合判定；若 v1 采用更小修正，须在审查报告中说明理由与代价）。

---

## 四、Run 退避与执行预算（P1 ↔ 评审 §3.2 / C62）

检查 `dependency_unmet` 使用的 `release_back`（接入点 `run_dispatch/claim.rs`）：

- 是否消耗最大执行尝试次数；
- 是否可能将 Run 标记为 exhausted / dead；
- 是否影响真正的执行失败统计。

**确保依赖等待不消耗执行失败预算。**

优先复用现有调度设施，不要引入不必要的新状态。

---

## 五、终态写入入口（P1 ↔ 评审 §4.3 / C68）

扫描所有修改 `delivery.task.status` 的业务代码。检查：

- `apply_task_transition`（`command_executor/transition.rs`）；
- `pm_write::update_task`（UI 直写历史路径，T-024 实施记录 §10.2 偏差 1 已接入）；
- 其他直接 UPDATE 路径；
- 系统任务及后台操作。

**确保所有终态迁移都能触发一致的依赖解锁副作用。**

---

## 六、租户隔离（P1 ↔ 评审 §5 / C66）

检查 `task_dependency`：

- RLS 是否完整；
- `tenant_id` 是否正确约束；
- API 的 upstream/downstream 查询是否严格限制项目与租户；
- 是否存在跨租户访问或修改风险。

对照基准：T-021b（`0019_tenant.sql` 模式）与 ADR-0001 的 RLS 约束。注意：进 `tenant_schema.rs` 表清单（39 → 40，XT-3 机械断言）**不等于**隔离完整——表清单只证明「表被登记」，不证明「策略正确生效」。

---

## 七、新增测试（↔ 评审 §7 / C59–C68）

至少增加（用户裁定最低集）：

- 并发反向建边不能形成环（C59）；
- 建边与上游完成并发不能漏解锁（C60）；
- `needs_approval` 不会被依赖解锁错误解除（C61）；
- `dependency_unmet` 不会耗尽执行失败预算（C62）；
- 多个上游并发完成不会产生重复有效解锁（C64）；
- 不同租户之间不能访问依赖关系（C66）。

评审文档 §7 全集（另含 C63 上游 failed 恢复路径 / C65 删边与完成并发 / C67 大量下游解锁 / C68 全部终态入口一致副作用）应尽量覆盖；无法覆盖的项须在审查报告中说明理由。

**方法论硬要求**：并发用例必须使用**两个独立数据库连接 + 显式事务屏障（barrier）**构造交错（两个事务各自开启、各自执行到关键点、屏障对齐后各自提交），必须使用**真实 PostgreSQL 集成测试**验证并发行为；禁止随机碰撞式并发测试——不可重复、不可调试的测试不能作为验收证据。

---

## 八、交付要求

**先输出审查报告，再实施修复。** 报告需要区分：

1. 已确认的 Bug；
2. 潜在风险；
3. 现有实现已经正确解决的问题；
4. 建议修改的代码位置；
5. 数据库迁移是否必要。

> 迁移号对账提示（2026-11-01 登记 T-025 时发现）：`todo.md` 中 T-041 写 `0022_delivery_manifest.sql`、T-083 写 `0022_discovery.sql`，两者预留号冲突；`0021` 已被 T-024 占用。若 T-025 需要迁移，取下一个空闲迁移号并在 `todo.md` 登记时一并澄清该冲突。

修复完成后：

- 运行 `cargo test --workspace`（dev 库常驻 5433）；
- 运行 `cargo check --workspace --all-targets`；
- 检查迁移兼容性；
- 更新 T-023/T-024 实施记录（`docs/ai-pm-task-dependency-v1.md`）；
- 新增 T-025 验收矩阵及测试映射（编号续 C58，见 §九）。

**红线**：不得为了让测试通过而降低现有状态机、租户隔离和事务一致性要求。

---

## 九、验收矩阵（续 C58；标注 ★ 者为用户裁定最低集）

| # | 场景 | 预期 | 对应章节 |
|---|---|---|---|
| C59 ★ | 两个事务同时建立反向依赖 | 至多一个成功，不能形成环 | §一 |
| C60 ★ | 建边与上游完成并发发生 | 最终下游必须解锁 | §二 |
| C61 ★ | `blocked` 任务同时存在 `needs_approval` | 依赖满足后仍保持 `blocked` | §三 |
| C62 ★ | `dependency_unmet` 超过普通重试上限 | 不消耗执行失败预算 | §四 |
| C63 | 上游 `failed` | 下游保持阻塞，存在明确恢复路径 | §三/§八（报告说明） |
| C64 ★ | 多个上游并发完成 | 只产生一次有效解锁 | §一/§二 |
| C65 | 删除依赖与上游完成并发 | 不发生漏解锁或错误状态迁移 | §二 |
| C66 ★ | 不同租户访问边表 | 无法读取或修改其他租户的数据 | §六 |
| C67 | 大量下游同时解锁 | 事务耗时和锁竞争在约定范围内 | 报告说明（P2 规模优化不在本次） |
| C68 | 所有终态写入入口 | 都触发一致的解锁副作用 | §五 |
---

## 十、实施记录

### 10.1 审查结论（P0 部分，2026-11-01）

五分类（对应 §八 交付要求）：

1. **已确认 Bug**：
   - P0-1 并发建边绕过环检测——`add_dependency` 仅 `FOR UPDATE` 锁下游行（`task_dependency.rs` 原 :161-163），上游行无锁（原 :172 普通 SELECT），无任何图级串行化；两事务锁不同行互不阻塞，递归 CTE 快照互不可见 → 双双通过 → 成环。
   - P0-2 建边与上游完成竞态——`unlock_downstream` 扫边用普通 SELECT（MVCC 快照），建边事务未提交的边不可见；建边事务的解锁判定同样读不到并发完成的终态 → 交错即漏解锁且永不补偿。
2. **潜在风险**：`add_dependency` ① 的注释声称「与解锁路径同一加锁顺序：先被依赖方，后依赖方」，但代码从未锁上游——注释与实现不符（已随修复一并修正）。
3. **现有实现已正确解决**：`unlock_downstream` 固定 `ORDER BY t.id` 加锁防死锁；`ON CONFLICT DO NOTHING` 兜底并发重复边；`unlock_task_if_ready` 锁后重读状态、重复调用幂等；解锁迁移走唯一受守卫入口。
4. **建议修改位置**：`task_dependency.rs`（图锁函数 + add/remove 入口改造）、`command_executor/transition.rs`（终态入口取锁）、新增并发测试——**均已实施**（§10.2）。
5. **数据库迁移是否必要**：**否**——advisory lock 为运行时机制，零 DDL，不触碰迁移号。

### 10.2 修复记录（§一 / §二，2026-11-01 完成）

| 落点 | 改动 |
|---|---|
| `task_dependency.rs::acquire_project_graph_lock`（新增） | `pg_advisory_xact_lock(hashtextextended($project_id::text, 0))`；同事务可重入，事务结束自动释放 |
| `task_dependency.rs::add_dependency` | `begin()` 后**无锁预读** `project_id` → 取图锁 → `FOR UPDATE` 锁下游做权威校验（预读 TOCTOU 后果仅为 `TaskNotFound`）；修正失实注释 |
| `task_dependency.rs::remove_dependency` | 同款预读 + 取锁（删边同样是图修改） |
| `command_executor/transition.rs::apply_task_transition` | `unlocks_downstream(to)` 为真时在 `FOR UPDATE` **之前**取图锁；非终态热路径（ready/claimed/running）零开销 |

死锁分析：全部取锁点先于该事务第一个行锁——终态迁移唯一入口为命令通道 `executor.rs::update_task_status`（`begin()` 后立即调 `apply_task_transition`）；`run_dispatch`（→Ready/Claimed）、`run_executor`/`run_context`（→Running）、解锁路径（→Ready）均非终态不取锁；`unlock_task_if_ready` 在已持图锁内调非终态迁移，可重入安全。

### 10.3 验收映射（本轮完成部分）

| # | 场景 | 用例 | 结果 |
|---|---|---|---|
| C59 ★ | 并发反向建边不能形成环 | `task_dependency_concurrency.rs::c59_concurrent_reverse_edges_cannot_form_a_cycle`（12 轮，双连接并发） | ✅ 恰一成功、败者为 `Cycle`、库内无环 |
| C60 ★ | 建边与上游完成并发 | `::c60_edge_insertion_racing_with_upstream_completion_still_unlocks`（4 轮 × 双启动序） | ✅ 两序皆解锁、`TaskReady` 恰 1 条 |
| C64 ★ | 多上游并发完成 | `::c64_concurrent_completion_of_multiple_upstreams_unlocks_once`（5 轮） | ✅ 单次有效解锁 |
| C65 | 删边与上游完成并发 | `::c65_edge_removal_racing_with_upstream_completion_still_unlocks`（4 轮 × 双启动序） | ✅ 无漏解锁、上游状态不被误迁 |
| C61 ★ | `blocked` + `needs_approval` 不被依赖解锁 | `task_dependency.rs::c61_active_waiting_survives_dependency_satisfaction` | ✅ 保持 `blocked`、0 条 `TaskReady`；等待解除后一次放行 |
| C68 | 所有终态写入入口一致副作用 | `task_dependency_concurrency.rs::c68_ui_direct_write_to_terminal_unlocks_downstream` + `::ui_terminal_write_waits_for_the_project_graph_lock` | ✅ 两入口解锁一致；直写等图锁、放行后恰 1 条 `TaskReady` |
| C66 ★ | 跨租户访问边表 | `pm-api/tests/task_dependencies.rs::c66_cross_tenant_dependency_access_is_invisible` | ✅ 读/写/删一律 404；跨租户上游 = `task_not_found`（非 409） |
| C62 ★ | `dependency_unmet` 超过普通重试上限 | `task_dependency.rs::c62_dependency_unmet_never_consumes_the_execution_budget`（6 轮 > 3/5 双上限） | ✅ 仍 `queued`、0 `task_run_failure`、infra 计数 0；上游完成后同一条 Run 可领取 |
| C63 | 上游 `failed` | `::c63_failed_upstream_keeps_the_downstream_blocked` | ✅ 保持 `blocked`、0 `TaskReady`；删边后解锁（恢复路径明确） |
| C67 | 大量下游同时解锁 | —（见 §10.8 说明） | ⏸ P2 规模优化不在本次范围，无基准数据 |
| — | 图锁互斥 / MVCC 隔离 / 提交后可见 | `::graph_lock_serializes_writers_and_exposes_committed_edges`（屏障式定向） | ✅ |
| — | 跨项目不阻塞 | `::graph_locks_of_different_projects_do_not_block` | ✅ |

### 10.4 实测

- `cargo test --workspace`：**73 套件、462 passed、0 failed**（含新增 6 例并发测试；T-024 基线 441 无回归）。
- `cargo check --workspace --all-targets`：0 error 0 warning。
- **2026-11-02 全仓复核（T-025 全部改动后，隔离实例 5434）**：**74 套件、496 passed、0 failed**，无 panic；同时 `cargo check --workspace --all-targets` exit 0。
- **2026-11-02 合并前复核（rebase 到 T-043 `60ef4d2` 之后）**：**75 套件、522 passed、0 failed**（增量来自 T-043 的 D5 用例，T-025 用例无回归）。

### 10.5 审查结论与修复记录（§三 Blocked 解锁语义，2026-11-02，方案 A）

五分类（对应 §八 交付要求）：

1. **已确认 Bug**：`unlock_task_if_ready` 原判定只有 `status == "blocked"` + `dependencies_satisfied` 两条，缺少「其它阻塞源是否仍在」这一维。`blocked` 任务若同时存在活跃 `task_waiting`（`needs_info / needs_decision / needs_approval / external_dependency`），依赖一满足就会迁 `ready` 并发 `TaskReady`；而领取侧又会因活跃等待拒绝（`run_dispatch/policy.rs:171-180`，`deny = "task_waiting"`）。故障形态不是「错误执行」，而是**状态谎报 + 事件污染 + 永远领不到的 Run + 无限退避**——比单纯卡住更难排查。
2. **潜在风险**：`delivery.task_waiting` 目前**没有任何业务写入代码**（全仓只有领取侧读取，测试直接 INSERT），所以该缺陷当前只能由历史数据或未来审批/等待流触发。它不是「已发生的线上事故」，但是必然发生的正确性缺口。
3. **现有实现已正确解决的部分**：`task_waiting.reason` 的 CHECK 不含依赖类原因（`0010_task_state_machine.sql:94-95`），因此「存在活跃等待」恰好等价于「依赖之外的阻塞仍在」；`uq_task_waiting_active` 保证每任务至多一条活跃行，判定无歧义；终态集合单点定义、解锁仍走唯一受守卫入口（不需要第二套状态写入口）。
4. **建议修改位置（已实施）**：
   - `task_dependency.rs::has_active_waiting`（新增，`:123-146`）：`project_id + task_id` 限定的 `EXISTS(... ended_at IS NULL)`。
   - `task_dependency.rs::unlock_task_if_ready`（`:426-431`）：依赖满足后追加一重门禁，仍有活跃等待 → 不解锁。
   - 新增 `tests/task_dependency.rs::c61_active_waiting_survives_dependency_satisfaction`。
5. **数据库迁移是否必要**：**否**——纯运行时谓词，零 DDL，不触碰迁移号（0024/0026 号段争议与本次无关）。

**方案选型（用户裁定 2026-11-02：A）**

| 方案 | 结论 |
|---|---|
| A 最小谓词（本次采纳） | 以「活跃 `task_waiting` 存在」为门禁。因为该表枚举里不存在依赖类原因，这一条即等价「无其它阻塞源」。~10 行、零迁移、不新增抽象。 |
| B Blocker 聚合（评审 §3.1 推荐） | **本次不采纳**，登记为 P2（与评审 §9.3 #12 `try_unblock_task` 合并）。理由：需要定义「哪些表算阻塞源」并持续维护，v1 无第二种阻塞写入者，收益为零；若做成 SQL 视图还要吃迁移号。 |
| C 依赖写入 `task_waiting` | **否决**：与 T-023 F2（关系只存边表）冲突，且一任务一活跃等待无法同时表达依赖与审批。 |

**A 的代价（必须与实现同在文档里，否则就是隐瞒）**

- **对称义务**：任何「解除等待」的写入路径都必须重新触发一次解锁判定，否则任务会停在 `blocked`。当前仓库没有 waiting 写入口，所以无人违反；未来审批/等待流落地时必须接上（方案 B 同样不免除这条义务——聚合只改判定，不产生触发）。
- **只影响 `blocked` 起点**：`draft` 不自动进 `ready`（D7），`AwaitingDecision` 有独立的 `waiting_resolved` 守卫，均不受本次改动影响。

**实测（本工作树 `feat/t025-dependency-p1`，隔离实例 5434）**

- `cargo test -p pm-application --test task_dependency`：**11 passed / 0 failed**（含新增 C61）。
- `cargo test -p pm-application --test task_dependency_concurrency`：6 passed / 0 failed（P0 用例无回归）。
- `cargo test -p pm-api --test task_dependencies`：8 passed / 0 failed（C43–C48 / C57 无回归）。
- `cargo check --workspace --all-targets`：0 error 0 warning。
- 未跑全仓 `cargo test --workspace`：共享实例 `template1` 的迁移 1 校验和已漂移（另一 session 占用该模板库），本工作树改用独立容器 `aipm-pg-t025:5434`，**没有重建共享模板、没有干扰其它 session**。全仓回归留到归档前在有窗口时补跑。

### 10.6 审查结论与修复记录（§五 终态写入入口，2026-11-02，用户裁定「行锁 + 图锁并入本次」）

五分类：

1. **已确认 Bug**：`pm_write::update_task`（UI 直写，第二终态入口）有两点与 P0-2 同源——① revision 校验用**无锁读**，两个并发 PATCH 可同时通过校验后各写一版；② 终态直写**不取项目图锁**，与并发建边交错时仍可漏解锁（P0-2 在同一条路径复发）。
2. **潜在风险（本次明确不做）**：`update_task` 仍不经 `validate_transition`，任意状态字面量可直写，绕过状态机与 E1/E4 守卫（T-024 §10.2 偏差 1 的历史遗留）。接状态机会改变 UI 契约与错误码，属独立条目，需单独裁定。另：非终态直写现在也拿行锁，带来轻微锁竞争（单行、短事务，可接受）。
3. **现有实现已正确解决**：全仓业务代码的 `UPDATE delivery.task SET status` 仅两处（`command_executor/transition.rs` 受守卫入口、`pm_write/task.rs` 直写）；系统/后台路径（`run_dispatch`、`run_executor`、`run_context`、`delivery` 提交、解锁回写）全部走 `apply_task_transition`，**不存在第三条裸 UPDATE**。因此 C68 的「入口全集」就是这两个。
4. **建议修改位置（已实施）**：
   - `pm_write/task.rs`：终态直写先无锁预读 `project_id` → 取项目图锁（先于任何行锁）→ 行读改 `FOR UPDATE`；解锁判定改用 `unlocks_downstream(new_status)`，与命令通道共用同一终态集合定义。
   - 新增两个用例：`c68_ui_direct_write_to_terminal_unlocks_downstream`（副作用一致）、`ui_terminal_write_waits_for_the_project_graph_lock`（屏障 + `pg_locks` 定向观测：持锁期间直写不得改行，放行后恰 1 条 `TaskReady`）。
5. **数据库迁移是否必要**：**否**——零 DDL。

死锁分析：图锁只在终态直写获取且先于行锁；非终态直写只拿行锁、**从不**在持行锁后取图锁 → 不存在「行锁 → 图锁」反向等待，与 `add_dependency` / `remove_dependency` / `apply_task_transition` 的「先图锁后行锁」同向。

**实测（同一隔离实例）**

- `cargo test -p pm-application`：**20 个测试二进制、172 passed、0 failed**（含 8 例并发测试二进制中的 2 例新增）。
- `cargo test -p pm-api --test pm_writes`：13 passed；`cargo test -p pm-mcp --test contract`：6 passed（`update_task` 契约无回归）。
- `cargo check --workspace --all-targets`：exit 0，无新 warning。

### 10.7 审查结论与修复记录（§六 租户隔离，2026-11-02）

**前提纠正（重要）**：本仓**不使用 PostgreSQL RLS**——全 `migrations/` 没有任何 `CREATE POLICY` / `ENABLE ROW LEVEL SECURITY`。ADR-0001 的隔离模型是三条：`tenant_id NOT NULL`（I1）、以 `tenant_id` 打头的组合外键（I1/I5）、**读路径按 tenant 过滤且不靠 project join 推导**（I3），并用 `information_schema` 机械断言（XT-3）。因此「RLS 是否完整」这个问题在本仓不成立，§六 的正确问法是「I1/I3/I5 在本表是否落实」。

五分类：

1. **已确认 Bug（本次修复）**：`add_dependency` 的上游查询不做租户判定——拿别的租户的任务 id 当上游会得到 `CrossProject`（409 `CROSS_PROJECT_DEPENDENCY`），而不存在的 id 得到 404 `task_not_found`。这构成**跨租户存在性 oracle**，且与模块自述契约（「不存在 / 跨租户 / 非调用方成员一律 404，不泄露存在性」）直接矛盾。
2. **潜在风险（未改，登记 P2）**：`list_dependencies(task_id)` 只按 `task_id` 过滤，没有 project/tenant 条件（严格按 I3，读路径应带 tenant 过滤）。当前唯一调用方是 API 且已过 `require_task_project`（`tenant_id` + 项目可见性），所以**现在没有可利用的外部面**；但它是 `pub` 的应用层函数，第二个消费者（MCP / 报表 / 导出）复用时即成为跨租户泄露面。修复方式需要改签名（加 `tenant_id`），属契约改动，本次不动。
3. **现有实现已正确解决的部分**：
   - `0021_task_dependency.sql` 结构齐备：`tenant_id UUID NOT NULL`、`UNIQUE(tenant_id, id)`、三条组合外键（`(tenant_id, project_id)`→`core.project`、`(project_id, task_id)`/`(project_id, depends_on_task_id)`→`delivery.task`）、`UNIQUE(task_id, depends_on_task_id)` 兜底重复边。
   - INSERT 的 `tenant_id` 由 `(SELECT tenant_id FROM core.project WHERE id = $1)` 派生——与父项目必然一致，组合外键是第二道保险，**写入侧不可能伪造归属**。
   - `tenant_schema.rs` 的 41 张表清单 + XT-3 机械断言覆盖本表（`tenant_id` 非空 + 以 `tenant_id` 打头的组合外键），实测 6 passed。
   - API 三入口（GET / POST / DELETE）都先过 `require_task_project`：按 `(id, tenant_id)` 查任务，再 `require_project_visible`——跨租户与「非项目成员」都是 404。
   - DELETE 幂等返回 204（边不存在也 204），不构成存在性 oracle。
4. **建议修改位置（已实施）**：`task_dependency.rs::add_dependency`——下游行与上游行都取 `tenant_id`；跨租户 → `TaskNotFound`（404），同租户跨项目仍 `CrossProject`（409，C46 契约不变）。新增 `apps/pm-api/tests/task_dependencies.rs::c66_cross_tenant_dependency_access_is_invisible`。
5. **数据库迁移是否必要**：**否**——零 DDL。

**实测**

- `cargo test -p pm-api --test task_dependencies`：**9 passed**（C43–C48 / C57 / C66，C46 的 409 语义未变）。
- `cargo test -p pm-application --test task_dependency`：11 passed；`--test task_dependency_concurrency`：8 passed。
- `cargo test -p pm-infrastructure --test tenant_schema`：6 passed（I1/I5 机械断言）。
- `cargo check --workspace --all-targets`：exit 0。

### 10.8 审查结论与修复记录（§四 Run 退避与执行预算，2026-11-02）

五分类：

1. **已确认 Bug**：**无**。核实结论（代码 + 用例）：`dependency_unmet` 走 `requeue_with_backoff`（`run_dispatch/lease.rs`），只做四件事——`status='queued'`、`lease_*` 清空、`dispatch_attempts+1`、`next_dispatch_at = now() + LEAST(cap, 2^attempts)` + `release_reason`；**不写 `task_run_failure`、不迁任务状态、不碰 `infrastructure_retry_count`、不把 Run 判失败**。
2. **潜在风险（已登记，非 Bug）**：
   - `dispatch_attempts` 全仓**没有任何上限判定**——`max_run_attempts` 只存在于 `RunDispatchConfig` 与 `pm-domain::RetryCounters`（后者在应用层未接线）。所以依赖等待不会「耗尽」，但反过来也没有显式上限防护：未来若有人给 dispatch 计数加阈值，依赖等待会被误算成失败。**约束应入文档**：`dispatch_attempts` 是退避计数，不参与失败判定。
   - `dispatch_attempts` 的指数增长会顶到 `backoff_cap_secs`（默认 300s）：等待越久，解锁后重领延迟越长。这是**延迟**问题，不是预算问题；C67（大规模/长等待优化）属 P2。
   - `max_infrastructure_attempts` 只在 `run_executor::release_after_start_failure` 里用 `infrastructure_retry_count` 判定，依赖路径根本不经过它（C62 断言 `infrastructure_retry_count = 0`）。
3. **现有实现已正确解决的部分**：`requeue_with_backoff`（计数）与 `requeue_for_redispatch`（不计数）分离，语义清楚；Model A 显式请求遇到依赖未满足直接回 `TaskNotReady("dependency_unmet")`，**不进退避队列**（不污染无人值守节奏）；每次退避都写一条 `skip_task_run` 审计，可观测。
4. **建议修改位置**：**无需改代码**。新增 C62 用例把上述结论钉成证据；C63 用例补齐 `failed` 上游的阻塞语义与恢复路径。
5. **数据库迁移是否必要**：**否**——零 DDL。

**C62 断言明细**（连续 6 轮，已超过 `max_run_attempts = 3` 与 `max_infrastructure_attempts = 5`）：

- Run 始终 `queued`，`release_reason = dependency_unmet`，`dispatch_attempts` 恰为 1…6（只有退避计数在动）。
- `task_run_failure` 计数 0；`infrastructure_retry_count` 0。
- 任务三元组（`status`、`retry_exhausted`、`attempt_count`）与首轮前完全一致。
- `skip_task_run` 审计恰 6 条。
- 上游完成后同一条 Run 能被正常领取（`ClaimKind::Claimed`）——退避不是永久禁用。

**C63 断言明细**：`failed` 上游使下游保持 `blocked` 且不发 `TaskReady`；删边后立即 `ready` + 1 条 `TaskReady`。恢复路径在 v1 只有人工动作（重跑上游 / 删边 / 换新任务）；「人工豁免依赖」属评审 §9.3 #7（策略可配置）P2。

**C67（大量下游解锁）未覆盖的理由**：该场景需要规模基准与监控指标（评审 §6.3：规模假设 + 三项指标），评审明确列为 P2 第 10 项，任务书 §0 也把它排除在本次范围。当前实现为同步事务内逐个 `ORDER BY id` 加锁解锁，正确性已由 C64/C65 覆盖，**性能基线未测**，不做无数据的断言。

**实测（隔离实例 5434）**

- `cargo test -p pm-application`：**22 个测试二进制、200 passed、0 failed**（本次全量口径，含 C61/C62/C63/C68 新增用例）。
- `cargo test -p pm-api --test task_dependencies`：9 passed；`--test pm_writes`：13 passed；`pm-mcp contract`：6 passed；`pm-infrastructure tenant_schema`：6 passed。
- `cargo test --workspace`（隔离实例 5434，T-025 全部改动后）：**75 套件、522 passed、0 failed**（rebase 到 T-043 之后的最终复核；rebase 前为 74/496）。
- `cargo check --workspace --all-targets`：exit 0。

### 10.9 待办（后续领取）

- **§五 遗留一项待裁**：`pm_write::update_task` 依旧不经 `validate_transition`（可直写任意状态字面量，绕过状态机与 E1/E4 守卫）。本次只并入行锁 + 图锁，状态机校验需单独裁定（会改 UI 契约与错误码）。
- **§六 遗留一项（P2）**：`list_dependencies` 加 `tenant_id` 过滤（现依赖调用方前置校验）。
- **C67 / 方案 B / 依赖完成策略可配置**：评审 §9.3 P2，本次明确不做；需要规模基准或策略裁定后再立项。
- ~~全仓 `cargo test --workspace` 回归~~：**已完成**（74 套件 / 496 passed / 0 failed，隔离实例 5434）。
