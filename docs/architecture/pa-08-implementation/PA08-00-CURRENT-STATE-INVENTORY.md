# PA08-00 — Current-State Inventory

- **日期：** 2026-11-22
- **代码基线：** `master` / `a7a522ccf818c7eac4d4ab096ae8e447c8f2bef2`
- **核对方式：** GitHub 当前文件与目录核对；本报告是静态代码盘点，不声称已运行测试。
- **上游证据：** PA-07 IMP-003 §4、§8；PA-07 FINAL §2、§9。

## 1. Findings

### INV-01 — Runner 主循环缺少完整生命周期测试

**代码事实**
- 主循环位于 `apps/ai-pm-runner/src/runner_loop.rs`。
- 当前文件内单元测试覆盖 heartbeat 配置校验、退避计算和 receipt envelope。
- 尚未看到围绕完整 `claim → spawn → heartbeat → command → submit → exit` 的可注入驱动测试。

**风险 / PA-08 处置**
- 崩溃、阶段间失败、重启后对账、迟到回报容易只在真实运行中暴露。
- PA08-02 抽取最小可注入边界，采用 scripted fake 与 fake clock；不为测试重写 Runner 业务逻辑。

### INV-02 — Run 生命周期写入分散

**代码事实**
- Run 领取/重领：`crates/pm-application/src/run_dispatch/claim.rs`。
- 租约与执行代次：`run_dispatch/lease.rs`。
- 执行编排及 Run 状态回写：`crates/pm-application/src/run_executor.rs`。
- PA-07 IMP-003 另列 `run_executor.rs`、`lease.rs`、`claim.rs`、`archive.rs` 为 Run 状态写入点。
- `run_dispatch/transition.rs` 当前是 Task transition 的统一入口，不是 Run transition guard。

**风险 / PA-08 处置**
- fencing 与条件 UPDATE 已覆盖部分竞态，但合法状态边、终态保护和迟到事件处理缺少一个清晰统一的 Run 状态变更契约。
- PA08-03 先枚举全部裸写点与现有 Run 状态值，再建立统一 guard；不得把 Task 状态守卫误当作 Run 守卫。

### INV-03 — Workspace / mirror 回收能力缺失

**代码事实**
- `crates/pm-application/src/run_cleanup.rs` 管理的是进程终止清理账本：pending / cleaning / completed / needs_human。
- 该模块不负责 Workspace 或 Git mirror 的保留期扫描、引用核对和磁盘目录回收。
- PA-07 IMP-003 已将 IMP-010 判为 NOT_IMPLEMENTED。

**待核风险**
- `claim_batch` 的 SELECT ... FOR UPDATE 与后续 UPDATE 分成独立语句且未显式共享事务；在 PostgreSQL autocommit 下，SELECT 语句结束即释放行锁。多个 cleanup worker 是否会重复领取同一批记录，需要专项并发测试确认。

**PA-08 处置**
- PA08-04 分开处理进程清理账本与磁盘资产回收。
- 若并发测试证实领取竞态，使用单事务 CTE + UPDATE ... RETURNING 或显式事务 + SKIP LOCKED；不得只凭静态观察直接重写。
- 任何磁盘删除必须先 dry-run，并在删除前二次检查活跃 Run、引用关系、保留期和受控根路径。

### INV-04 — Product 生命周期只检查 no-op，未验证合法源状态

**代码事实**
- `crates/pm-application/src/product.rs::set_product_lifecycle` 将 action 映射为目标状态。
- 当前实现锁定 Product 行并验证 Product Owner，但只拒绝 `status == target_status`；没有按源状态验证 action 是否允许。
- 因此诸如从 `archived` 再执行 `resume`，或从 `normal` 直接执行 `complete_resume`，当前代码层没有状态边校验（数据库 CHECK 只约束状态值集合，不表达状态图）。

**PA-08 处置**
- PA08-05 以 PA-07 Product 状态语义为准建立显式转换矩阵。
- 需先核对 REST action 调用点与产品端调用语义，尤其 `restart` / `complete_resume` 的区别；不可静默改变 API 语义。
- 对非法转换验证状态、审计和 Outbox 均无副作用；并发 Product transition 与 Run start 必须由同一数据库锁/门禁顺序解决。

### INV-05 — Runtime observability 仍以结构化日志为主

**代码事实**
- PA-07 IMP-003 将 IMP-018 判为 NOT_IMPLEMENTED：无 metrics 导出、磁盘水位与告警通道。
- 现有 cleanup needs-human 查询和 Runner comm overview 可复用，不需要新建消息平台。

**PA-08 处置**
- PA08-06 先盘点仓库已有 metrics crate / endpoint，再决定复用或新增最小 Prometheus exporter。
- 指标标签必须有界，禁止 run/task/project 等实体 ID 作为标签。

## 2. Workstream readiness

| Workstream | 静态盘点结论 | 下一步 |
|---|---|---|
| PA08-00 | 已完成目标模块与主要风险点定位；尚未运行本地测试 | 后续实施遇到新写入点时增量更新 |
| PA08-01 | crates.io 源切换已提交；Hosted build 正在验证 | 等待 GitHub Actions 结果 |
| PA08-02 | Runner 单测仅覆盖辅助逻辑 | 设计可注入测试 seam |
| PA08-03 | Run 写入分散；Task guard 不可替代 Run guard | 建立全仓写入点清单与状态矩阵 |
| PA08-04 | 无磁盘回收；进程清理账本另有潜在领取竞态 | 先补并发测试，再做原子领取与回收器 |
| PA08-05 | lifecycle action 未校验源状态 | 核对 API 调用与 PA-07 状态图 |
| PA08-06 | 无 metrics 导出 | 盘点现有依赖和服务端挂载点 |

## 3. Non-goals

- 本盘点不改变 PA-06 人类/AI 授权。
- 不改变 PA-07 Task / Run / Product 状态隔离。
- 不把进程清理账本等同 Workspace/mirror 回收。
- 不将静态代码推断表述为测试已证实缺陷。
