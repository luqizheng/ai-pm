# S03-ACP 通信面实现对照审计 V1.0 —— 设计 V1.1 ↔ 代码（84c67e9 / 1234162）

| 项目 | 内容 |
|---|---|
| 审计对象 | V1.1 设计稿（含复审 F-1~F-5 处置状态）↔ `feat/s03-acp-comm-face` @ 85fde61 实现 |
| 审计方法 | §1~§7 逐条款 ↔ 代码对照（全文 grep + 调用方核查），续接独立复审编号（F-6 起） |
| 结论 | 复审 F-1~F-5 维持；**新发现 F-7 [P1] / F-8 [P1]（须在真实栈 E2E / ACP-05 前闭合）**、F-6/F-9~F-13 [P2/注记]；无设计-代码方向性冲突（全部为"设计有、代码无"或语义细化） |
| 复核声明 | F-7/F-8/F-6 疑点经 grep 调用方核查坐实（见各条证据）；F-9~F-13 为代码阅读结论 |

---

## 1. 逐条款对照总表（一致项从略，仅列差异）

| 设计条款 | 实现状态 | 差异 |
| --- | --- | --- |
| §1.2 标记文件全案 | ✅ 全落地（两层侦测/schema/64KiB/event_id 归档/隔离/幂等重报） | 拒绝类归档缺失 → **F-9** |
| §1.3 pm_report_interaction 契约 | ✅ 契约一致；事务七步一致 | event_id 唯一性作用域实现更严 → F-13① |
| §1.4 三阶段 | ✅ acked 双真值判据 + 注入失败处置 | — |
| §1.5 消费回执 | ✅ 双证据/N-6 稳态 | 可观测面缺失 → 复审 F-5 |
| §1.6 权限码 | ⚠️ 中间件已强制，**签发面缺失** | → **F-7** |
| §3.1/3.2 worker_command + 领取租约 | ✅ 逐字段一致 | 陈旧 issued 命令无清理 → F-13② |
| §3.3 身份双轨 | ✅ 过渡形态如设计 | 持有证明缺失 → 复审 F-2 |
| §3.4 轮询节奏 | ✅ env/默认值一致 | — |
| §3.5 / §6.4 失败升级 | ⚠️ 平台侧 ✅；**执行端有界重试未实现** | → **F-6** |
| §4.1 guard 六情形 | ✅ 五处转换落地 | **Task 取消/归档联动缺失** → **F-8** |
| §4.2 S1/S2/S3 | ✅ 三落位 | S1 跳过审计缺失 → F-11 |
| §5.1/§5.2/§5.3 介入链与映射 | ✅ 全落地（含 chain_open 列、superseded 让出） | 同 F-8 |
| §6 ACP-02 | ✅ 触发/执行/回执/耗尽副作用 | 进程账本未更新 → F-10；执行端重试 → F-6 |
| §7 ACP-08 | ✅ 能力声明/侦测约定/进程面继承 | 进程树硬化未做 → 复审已记（ACP-08） |
| §9 裁决 R-ACP-1~9 | ✅ 全部落地（R-ACP-6 顺移 0046 已回填台账） | — |

## 2. 新发现

### F-7 [P1] 新权限码无签发来源——四新工具在真实栈必然 403

`apps/pm-api/src/auth.rs` 的 `require_mcp_permission` 对四新工具强制 `PM_MCP_AGENT_REPORT` / `PM_MCP_COMMAND`，但全仓（rs/sql/json）**无任何签发/播种来源**（grep 仅命中 auth.rs 门禁与注释）——权限码来自 lmms JWT `permissions` claim，IAM 侧字典/角色模板未含这两个码。后果：今天用任何真实令牌调用四新工具 → `IAM_PERM_3001` 403；**测试全绿掩盖了这一点**（pm-mcp 测试直调 tools 层，不经中间件）。与设计 §1.6"字典落地随 V-05-1 迁移批"的排期语义冲突：门禁现在就生效，字典却没有。
**处置建议**（二选一，须在真实栈 E2E 前完成）：① lmms 侧把两码并入 executor 令牌模板/角色播种（正式路径，随 V-05-1 提前）；② auth.rs 过渡放行（两码缺失时降级为仅 `PM_MCP_READ` + 显式 log，标注 V-05-1 撤除）——推荐 ①，避免 fail-closed 门禁开洞。

### F-8 [P1] Task 取消/归档 ↔ Interaction/guard 联动缺失

设计 §5.1"CANCELLED 触发：Task 被取消/归档联动取消未决 Interaction"与 §14 两行（取消→取消 Interaction/Continuation；归档→禁新 Continuation）在实现中**无接线**：`cancel_open_interaction`（interaction.rs:535）零生产调用方；Task 状态唯一写入面 `command_executor/executor.rs update_task_status` 未挂钩。后果：Task 终态后 `chain_open`/`scheduling_shield` 残留脏数据；ACP-445（取消与上报并发）当前不可成立；守护语义"Task 终态 → guard 归 none"（§4.1）落空。
**处置建议**：`update_task_status` 迁移至 `Cancelled`（及归档路径）成功后、同事务调用链级取消（把 `cancel_open_interaction` 改造为 `cancel_for_task_tx(&mut Tx, ...)` 以并入事务，保持 V-6 单事务纪律）；补 ACP-445 测试。

### F-6 [P2] terminate 有界重试（执行端）未实现

设计 §6.4"执行端按有界重试（`AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS=10`，次数/间隔配置化），重试耗尽 → failed"；Runner 实现为**单次尝试即回执终态**（main.rs terminate 分支无重试循环、不读该 env）。安全网仍在（failed → `continuation_start_frozen_at` 冻结启动），故降级 P2；但 §23.3"Process Termination 失败处理"在执行端未完整落地，且 failed 为终态后该 Interaction 的终止通道永久关闭。
**处置建议**：Runner 增加重试循环（默认 N=3 × 2s，env 配置化）后再回执 failed；随 F-1 一起在 lib 化重构中补测试。

### F-9 [P2] Platform 拒绝的标记文件永久重试 + 队头阻塞

设计 §1.2"被唯一活跃链约束拒绝的 → 改名 `.rejected` + 指标，不重试"；Runner 实现对 Platform 拒绝一律"保留原文件待下轮"（main.rs:295）→ Conflict 类永久拒绝（如 Run 已非活跃）每 5s 重试一次：日志刷屏 + **队头阻塞**（find_unreported_marker 总返回同一文件，后续事件永不侦测）。缓解：开放链类拒绝会在链关闭后自愈（重报成功）。
**处置建议**：区分错误类别——Conflict → `archive_marker` 改 `.rejected` 后缀（不重试）；传输/Db 类 → 保留重试。

### F-10 [P2] ack terminate 后进程账本未更新

设计 §6.6"Platform 收到 acked 后：Process 状态更新（扩展既有进程清理账本，§12.6）"未实现；Runner spawn 的进程在 `delivery.task_run_cleanup` 无账本行（该账本仅服务器 Worker spawn 时登记），终止回执仅落在命令 payload + 审计。
**处置建议**：随 ACP-08 进程面统一（Runner 进程纳入账本或 ack 时回写终止结果），避免 §12.6 的"优先扩展不新建"落空。

### F-11 [P2] S1 跳过无审计记录

设计 §4.2 S1"跳过时记录 guard 值供审计"；实现为 poll SQL 直接排除（无逐 tick 审计——避免每 3s 刷审计表的合理性取舍），仅 S2 落了 `skip_task_run` 审计。**处置建议**：设计补注"S1 以指标替代逐次审计"（将 F-5 的 shielded 指标一并落），或在 poll 排除行数 >0 时输出一条节流日志。

### F-12 [P2] 自领循环主体在 bin 不可测

`run_loop`/`tick`/`claim_and_start`/`execute_command` 全在 main.rs（bin 目标），集成测试无法驱动——F-1 的修复（continue 前置检查）恰在此代码内。
**处置建议**：F-1 修复时同步把循环体迁入 lib（`ai_pm_runner::loop` 模块），用 wiremock + 伪 agent 命令补 tick 级测试（含 ACP-444 执行端拒绝场景）。

### F-13 [注记] 两处语义细化（建议回写设计，无需改码）

① event_id 幂等作用域：设计 §1.3"event_id 唯一索引"，实现为 `UNIQUE(run_id, event_id)`——更正确（跨 Run 防碰撞语义），建议设计改文对齐；② 回收（generation 递增）后旧执行端的 issued 命令被 fencing 永久滞留，无清理任务——依赖 `worker_command_pending_age` 告警人工处置（指标本身属复审 F-5 范围）。

## 3. 处置顺序建议

1. **F-7（真实栈 E2E 前）**：权限码签发路径裁决（推荐 lmms 播种）。
2. **F-8 + ACP-445**：取消联动事务化（小改动，可与 F-1 同批）。
3. **F-1 + F-12 + F-6**：Runner lib 化重构一批完成（continue 前置检查 / 循环入库 / 终止重试），补 ACP-444 执行端测试。
4. F-9/F-10/F-11/F-5：随 ACP-08（进程面/可观测）切片。
5. F-13：下次修订设计稿时回写两处语义。

---

**审计人**：ZCode（同会话链，复审后续审计）
**日期**：2026-11-07
