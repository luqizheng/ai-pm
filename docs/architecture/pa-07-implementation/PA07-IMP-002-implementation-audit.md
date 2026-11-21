# PA07-IMP-002：PA-07 实施现状核查报告

- **任务：** PA-07 检查（用户 2026-11-21）——对照基线核查实施现状 + PA-06 改动（f39618e）× PA-07 语义一致性
- **日期：** 2026-11-21
- **原始核对基准：** HEAD `75094e1c2b88719d34364373b338609ed3805b32`，migrations 0001–0052；基线=PA-07 Developer Implementation Specification V1.0 §10/§11 与 PA07-IMP-001（HEAD `ecb1ce2` / migrations 0001–0041）
- **后续差异核对：** HEAD `6ed0f1a5e7339cd701e538c2a13513cd9c0e9adb`（2026-11-22）；详见 [PA07-IMP-003](PA07-IMP-003-current-state-verification.md)
- **方法：** 逐项代码证据核查（两次独立盘点）+ PA-07 关键测试面实跑（10 套件 100/100 绿）

## 1. 总体判定

**以下 IMP-001~019 统计和原始问题发现均是 HEAD `75094e1` 的历史审计结论，不代表该 HEAD 之后的当前状态。** 原始审计发现 E2 命令通道缺口；该缺口已由后续提交 `6ed0f1a5e7339cd701e538c2a13513cd9c0e9adb` 修复并有新增边界测试。原始审计发现的批量 `blast_radius` 弱化问题在当前 HEAD 仍待处理，见 §4 与 PA07-IMP-003。

**原始基准统计：P0 全部闭环，P1 部分落地，P2 未动；发现 E2 缺口（后续已修复）与批量 blast_radius 问题（当前仍 OPEN）。**

PA07-IMP-001 快照列出的全部 P0 缺口已由迁移 0042–0047 一批关闭（task_revision=0042、dependency 条件=0043、run 门禁快照=0044、task_product=0044/0045、human_consultation=0046、link 软删=0047、product key=0049）；快照的 3 项待裁决全部有结果落地（Product 四态按建议、link 软删、D6 维持严格侧）。

## 2. IMP-001~019 判定表

| ID | 判定 | 摘要与证据 |
|---|---|---|
| 001 schema 核对 | ✅ | 报告即交付物；缺口由 0042–0047 补齐；Product 权威字段=0044 单列四值 |
| 002 Task Revision | ✅ | 0042 + Run 钉定（run_dispatch/port.rs:33-51）+ CAS 冲突测试（tests/task_revision.rs:176）；缺 Revision vs Run 启动专用竞态测试 |
| 003 Dependency | ✅ | 0043 条件列 + advisory lock（task_dependency.rs:110）+ 递归 CTE 检环 + 自环拒绝 + 并发测试（concurrency 614 行）；无边条件原位更新接口（改=删+建）；D6 仍限 draft/blocked 加边 |
| 004 Run 门禁快照 | ✅ | 0044 快照表 + claim 同事务四门（claim.rs:713/745-788/790 + port.rs:33）+ suspend vs claim 竞态测试（run_start_gates.rs:318） |
| 005 Product 验收 | ✅ | 0045 + Completed 单点验收门（transition.rs:109）+ 撤回退出/最新生效测试 |
| 006 人工决策 | ✅ | 0046 四态+DB CHECK 完整性+挂 task 阻塞；未回应≠同意 |
| 007 审计闭环 | ✅ | Revision/Dependency/Product 生命周期/Run 快照/验收均有审计；小缺口：依赖边无原位变更审计位（只有删/建） |
| 008 SSH 私钥 RAII | ❌ 缺失 | codex_executor.rs:113-132 仍明文写 keys/{id}.pem 且无删除路径；git_access.rs:67 已有 KeyMaterial RAII 先例未回移 |
| 009 per-device 单活跃 Run | ❌ 缺失 | 仅 per-Task uq_task_run_active；cpk 领取与 MCP Model A 无 device 级活跃检查 |
| 010 Workspace/mirror 回收 | ❌ 缺失 | run_cleanup 仅进程终止账本，无保留期/水位回收 |
| 011 Runner 主循环单测 | ⚠️ 部分 | 辅助函数有；claim→spawn→heartbeat→command→submit→exit 全链与崩溃/迟到场景缺 |
| 012 Run 转换一致性 | ⚠️ 部分 | Task 侧共享守卫+矩阵 ✅；Run 侧靠 fencing+分散检查，无集中 guard |
| 013 Product 生命周期 E2E | ⚠️ 部分 | 四态+REST+claim 门禁+单/双 Product 测试有；规范 §12.3 场景 4-6（多 Product 暂停影响面/Resuming 逐步恢复/归档不级联）缺端到端；lifecycle 不校验源状态（任意态可跳） |
| 014 Submit 事务缩短 | ❌ 缺失 | delivery/mod.rs 提交主流程仍单长事务含 Git 子进程/远端校验 |
| 015 LLM token 预算 | ⚠️ 部分 | 字符预算/超时/usage 落库有；无按模型/任务 token 上限与成本封顶 |
| 016 DNS rebinding | ✅ | mcp_hosts.rs 白名单（默认 loopback、永不空、精确端口）+ 6 组回归测试 |
| 017 Device credential TTL | ⚠️ 部分 | 撤销+两步轮换+测试有；无 expires_at 到期失效 |
| 018 Runner 可观测性 | ❌ 缺失 | 无 metrics/告警/磁盘水位；仅日志上限与 needs_human 队列 |
| 019 Product Key | ✅ | 0049 全套（格式 CHECK/lower 唯一/派生兜底后缀/不可变/API 带 key）+ 3 组域测试 |

统计：✅ 9 · ⚠️ 5 · ❌ 5。❌ 全部集中在 P1 执行安全/运维（008/009/010/014/018）——与 IMP-001 快照方向一致，属已知债务而非回归。

## 3. PA-06 改动（f39618e）× PA-07 一致性

| 改动 | 判定 | 说明 |
|---|---|---|
| create_task 移除 require_human | ✅ 一致 | decompose 建 Task 落 draft；E2 真人闸（update_task draft→ready + mark_task_batch_ready）原样；REST 人类面守卫 require_human_principal 在位；命令通道红线闸仍在 |
| 红线单摘除 create_tasks | ✅ 基本一致 | 三处同步确认；导入 owner 确认流与红线单解耦且完整（proposal 硬置 redline_pending→Human 执行）；但见 §4-P2 |
| 产品验收加人类+Membership 守卫 | ✅ 一致 | 无 platform_flow 隐藏调用方；acceptance_gate 完好；对 §7「平台流程」词的收窄已有文档背书（IMP-001 §3） |
| issue_risk/consultation 人类门 | ✅ 一致 | E2/E5 链路完好；Product 维度未执法（与 DR-PA-06 资格项 OPEN 自洽，窄于 PA-07 文本） |
| pm_reviews 吞检查修复+人类门 | ✅ 一致 | decide_review_approval 调用方唯一；无内部绕过 |
| AI_WRITABLE 白名单 | ✅ 语义一致 | 两处文档漂移见 §4-P5 |

## 4. 发现的问题

| # | 级别 | 问题 | 建议 |
|---|---|---|---|
| P1 | **RESOLVED — 后续提交修复** | E2「AI 永不置 ready」命令通道缺口已由 `6ed0f1a5e7339cd701e538c2a13513cd9c0e9adb` 在 `apply_task_transition` 单点封堵；AiPm/DevAgent 拒绝，Human/Service 保持合法路径；新增 3 条边界测试。修复细节和验证范围见 PA07-IMP-003。 | 当前已关闭，不再列为待修复项 |
| P2 | 中 | 红线摘除后批量门禁弱化：blast_radius=proposals.len()（orchestrator.rs:237，self_review 不读 payload）→ 单条 CreateTasks 提案含 N 任务时 radius=1 永不升级，AiPm 可 auto_apply 最多 200 任务；解析层无 kind 白名单。我们写入 yaml/policy_engine 的注释「超大批量仍走 escalate」只在逐条 create_task 形状下成立 | ProjectCommand 加纯函数 item_count() 并入 blast_radius；或修正注释口径 |
| P3 | 低 | Human Consultation decision 未执法 Product Membership（仅 human+项目可见；product_scope 无比对）——与 DR-PA-06 资格项 OPEN 自洽，窄于 PA-07 §7 文本 | 文档对齐或批次 B 补 Membership 比对 |
| P4 | 低 | `source='platform_flow'` 成死值（API 接受但平台主体必 403）——与 §7「平台流程」主体词冲突，已有文档背书 | API 注释标注「仅人类代录语义」或后续轮次明确平台流程主体 |
| P5 | 卫生 | 文档漂移三处：①IMP-001 §2 表 row 01/02 仍写裁决前状态（与同 commit 代码相反）；②AI_WRITABLE_COMMANDS 注释自称 §3.3 ✅ 行但缺 create_task；③legacy supervision 决策口与 pm_writes decide 均不校验 required_role（导入「owner 确认」实际项目成员即可——先于本批存在） | ①②文档更新；③按需收紧 |
| 附 | 债务确认 | ❌ 五项（008/009/010/014/018）与 ⚠️ 五项缺口均为 IMP-001 快照方向上的已知债务，非回归 | 按 P1/P2 原排期推进 |

## 5. 测试面健康（2026-11-21 实跑）

10 套件 100/100 绿：run_dispatch 31 · task_dependency 18 · review 15 · task_dependency_concurrency 9 · device_credential 6 · task_product_acceptance 6 · task_revision 5 · state_transition_guard 4 · run_start_gates 3 · human_consultation 3。

## 6. 结论

本报告的 IMP-001~019 统计仍然只对应原始 HEAD `75094e1`，不得当作 `6ed0f1a` 的全量重新审计结果。E2 命令通道缺口已由 `6ed0f1a` 关闭，不再需要作为当前缺陷跟进。当前仍需处理的是 **P2 批量 blast_radius 语义**；原审计中的五项执行安全/运维债务及五项部分完成项仍沿用原始判定，除非后续证据报告明确更新。最新差异与当前证据见 [PA07-IMP-003](PA07-IMP-003-current-state-verification.md)。


## 7. 后续状态说明（2026-11-22）

本报告创建后，`master` 从原始审计基准 `75094e1` 前进两个提交至 `6ed0f1a`。Git compare 确认两者之间仅涉及：`crates/pm-application/src/command_executor/transition.rs`、`crates/pm-application/tests/command_executor.rs` 和本审计文件。E2 的代码修复及测试证据详见 PA07-IMP-003。提交说明报告 12 个测试套件 135/135 通过；该数字是提交所附测试结果，本次文档审查未在本地重新执行。

批量 `blast_radius` 问题仍为 OPEN：编排器当前按 proposal 数量计算半径，而 `CreateTasks` 命令可包含多个 Task。其影响面、证据和修复建议见 PA07-IMP-003。不得将此问题与 E2 修复混为一谈。
