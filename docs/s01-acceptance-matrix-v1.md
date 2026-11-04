# S01 Acceptance Matrix V1.0

> 文档性质：S01 正式交付体系（`docs/s01-implementation-baseline-v1.md` §9 第五份）
> 上位：**`docs/s01-implementation-baseline-v1.md`（正式实施基线 V1.0）**；`docs/s01-task-registry-v1.md`；`docs/s01-implementation-plan-v1.md`
> 代码基线：`feat/s01-implementation`（master 2933db8 + S01 批次）　版本：V1.1　日期：2026-11-03

---

## 0. 五级验收（基线 §8）

基线 §8 规定 S01 采用**五级验收**，并明确「**不能以 L1 通过代替整个 TASK 完成**」。本矩阵把总体计划 §13 的七类测试口径映射到五级：

| 级别 | 基线 §8 要求 | 判定口径 | 本仓证据形态 |
|---|---|---|---|
| **L1 代码验收** | 编译通过、静态检查通过、结构符合架构约束 | `cargo build` / `cargo check --all-targets` 零错误零警告；`vue-tsc --noEmit` 零错误 | 构建输出 |
| **L2 数据库验收** | Migration 执行成功；外键/唯一键/CHECK 生效；事务回滚正确；并发约束正确 | 空库迁移成功（`#[sqlx::test]` 每例独立建库）；**直接打库断言约束违例**（23503/23505/23514）；事务回滚与并发用例通过 | `crates/pm-application/tests/i6b_db_constraints.rs` 等 |
| **L3 接口验收** | REST 正常、MCP 正常、权限与租户隔离正确、错误码映射正确 | HTTP 层断言状态码 + 错误码体；MCP `tools/list` + `tools/call` 断言 | `apps/pm-api/tests/*`、`crates/pm-mcp/tests/*` |
| **L4 业务流程验收** | 正常/失败/重试/取消/恢复/并发流程 | 端到端流程用例（含 Worker 参与的真实链路） | `apps/pm-api/tests/imports.rs`、`imports_e2e.rs`、`import_contract.rs` |
| **L5 UI 与端到端验收** | 页面符合统一视觉基线、前后端状态一致、真实数据可展示、操作可实际执行、历史与证据可追溯 | 前端组件测试 + `vue-tsc`；真实后端数据（非 Mock 冒充）；操作可点；证据可回溯 | `frontend/src/**/*.test.ts` |

**TASK 完成时必须逐级标注 L1～L5 的达成情况**；无 UI 改动的 TASK，L5 记「不适用（无前端改动）」并注明。

### 0.1 七类测试 → 五级验收的映射

| 测试类型（总体计划 §13） | 主归属级别 | 补充级别 |
|---|---|---|
| 正向 | L3 / L4 | L5（可点路径） |
| 负向 | L3 | L2（DB 级违例） |
| 并发 | L4 | L2（唯一约束/行锁） |
| 幂等 | L4 | L3 |
| 恢复 | L4 | L2 |
| 数据 | **L2** | — |
| 审计 | L3 / L4 | — |

---

## 1. 统一验收规范（七类测试）

总体计划 §13 规定所有正式 TASK 必须包含下列测试类型。S01 对每一类补充**具体可执行的判定口径**：

| 类型 | 总体计划要求 | S01 判定口径 | 落点 |
|---|---|---|---|
| **正向** | 正常业务流程成功 | 请求返回 2xx；**权威状态确实变更**（DB 复查，不只看响应体）；副作用（审计行、outbox、交付清单）齐全 | 各 TASK |
| **负向** | 越权、错误状态、非法参数被拒绝 | 断言**具体错误码**（如 409 `repository has history and cannot be deleted`、404 跨租户、403 权限码），并断言**零副作用**（DB 无写入、无审计行、无 outbox 事件） | 各 TASK |
| **并发** | 同一资源多请求竞争 | ≥2 个独立连接/进程同时发起；断言**最终状态唯一确定**（不是"都成功"或"都失败"，而是确定的一个赢家） | I1-C、I2-E、I6-B-01、I6-B-06 |
| **幂等** | 重复提交不重复产生业务结果 | 同一 `operation_id` / 幂等键重复提交 N 次，断言**业务表仅 1 行 + 账本 `Replayed`** | I6-B-04/06、S01-W13 |
| **恢复** | 进程退出、Worker 重启、租约过期 | 强杀 Worker/Executor → 租约过期 → 重领；断言**旧 Run 历史不被覆盖** | I1-F、I6-B-07、S01-W62 |
| **数据** | 外键、唯一约束、CHECK、状态一致性 | **直接对数据库执行 INSERT/UPDATE 断言约束违例**（基线 §4.2 验收要求 + merge-spec §4）；不经 REST/MCP | I6-B-04/08/09 |
| **审计** | 操作主体和操作结果可追溯 | 断言审计四列（`source_kind`/`source_ref`/`actor`/`tenant`）+ 操作结果（成功/失败/原因） | I6-B-06、S01-W63 |

### 1.1 TASK 完成定义（总体计划 §13.1，8 条 + 基线 §8）

1. 实施符合已批准设计；2. 代码已提交；3. 测试已执行并通过；4. Migration 已过适用审查；5. 代码审查通过；6. Delivery 证据完整；7. 无未处理 P0/P1；8. 平台验收通过。

> **Agent 的自我报告不能替代平台验收**（第 8 条）。本 S01 的"平台验收"= 独立验收者按本文档逐条复核并给出 verdict。
> **L1 通过不等于 TASK 完成**（基线 §8）。

---

## 2. 验收编号台账（现状与冲突）

### 2.1 已存在的验收编号段

| 编号段 | 数量 | 来源 | 状态 |
|---|---|---|---|
| I1-001 ~ I1-040 | 40 | I1 §15.1~15.5 | 定义完整 |
| I2-001 ~ I2-033 | 33 | I2 V1.1 §15.1/§15.2 | 定义完整，但 **V1.0 的 I2-023~027 五条被占号**（见 §2.3） |
| I2 §15.3 补充场景 | 15 | I2 V1.1 §15.3 | 未编号（并发与安全专项） |
| I3-001 ~ I3-024 | 24 | I3 §11 | 定义完整 |
| I4-001 ~ I4-024 | 24 | I4 §12 | 定义完整（含 P0/P1 优先级列） |
| I5-001 ~ I5-036 | 36 | I5 V1.1 §10 | 定义完整（V1.0 为 25 条，V1.1 追加 026~036） |
| I6-B-036 ~ I6-B-051 | 16 | 两份 I6-B 稿均称「来自裁决 §11」 | ⚠️ **内容缺失——裁决文档不在本仓**（待决 B-2） |
| I6-006 | 1 | I6-B V1.0 正文（不在本仓） | C-1 改写为「409 `project_not_empty`」 |
| C70/C71 ~ C97 | 28 | D4 设计稿 §6 | ⚠️ 起号 C70 或 C71 两说（待决 B-8） |
| C98 ~ C137 | 40 | D5 设计稿 §10 | 定义完整 |
| C138 ~ C162 | 25 | Codex Bootstrap §十（BOOT-01~25 映射） | 定义完整（归属 T-087~T-091） |
| C163 ~ C172 | 10 | Bootstrap 评审 §10.1 | 定义完整（归属 T-081/T-088/T-089/T-090） |
| C166 | 1 | T-081 七项身份链路门禁 | **跨机实测全过（2026-11-03）** |
| C173 ~ C184 | 12 | T-088 自加 | 已落地 |
| C59 ~ C68 | 10 | T-025 | 已落地 |
| C70/C71~C97、D4/D5 段 | — | — | 与 I3/I4/I5 段无冲突 |

### 2.2 已闭环的既有验收（无需重做）

| 段 | 结论 |
|---|---|
| C166 | **跨机实测全过**（192.168.2.4 → 192.168.2.10:8090，`docs/runbook-test-server-deploy.md`） |
| C173~C184 | T-088 交付，91 套件 662 passed |
| C59~C68 | T-025 收口（2026-11-03） |
| C138~C172 | T-087~T-090，其中 T-090 的 **C172 仍待干净 Windows 环境实测** |
| C98~C137 | T-042/T-043 交付 |
| C70/C71~C97 | T-040/T-041 交付（起号待裁决 B-8） |

### 2.3 编号冲突登记与裁决建议

| # | 冲突 | 建议裁决 | 待决项 |
|---|---|---|---|
| X-1 | **I2 双重占号**：V1.0 §10 的 I2-023~027（① 既有 verification 端点兼容 ② `set_finding_verification` 收口 ③ 事务原子性 ④ 404 跨租户 ⑤ kind/verification 一致性）在 V1.1 §15.1 中被用作新号 | **V1.1 追加 I2-034~038 承接 V1.0 五条**，V1.1 原号不动 | B-3 |
| X-2 | **I6-B-036~051 无内容** | 在 I6-B V1.1 正文定义并登记（推荐）；或找回原裁决文档 | B-2 |
| X-3 | **C70 / C71 起号二说** | 以 D4 设计稿 §6 的实际条目起点为准，逐条对照 D5 §10 的「续 C85–C97 → C98 起」补齐缺口 | B-8 |
| X-4 | I5 001~025 在 V1.0/V1.1 措辞微调（如 I5-018） | 统一取 V1.1 措辞 | 已裁决 |
| X-5 | C138~C162 归属「Slice F」（不在 D1~D5 序列） | 归属 T-087~T-091，不单列 S01 任务 | 已裁决 |

---

## 3. 验收条目 → TASK 归属矩阵

### 3.1 I1 · 归档生命周期（40 条）

| 归属 TASK | 验收条目 | 覆盖的类型 |
|---|---|---|
| I1-A | I1-001~006 | 正向 + 负向 + 数据（活动门禁、行锁、锁顺序） |
| I1-B | I1-007~009、012~013、035~036 | 正向 + 负向（Run 创建入口关闭） |
| I1-C | I1-010~014 | **并发**（两阶段候选发现、Project 锁定、锁序） |
| I1-D | I1-001~006（归档权限复用）、015~024 | 正向 + 负向 + 审计（归档事务、Run 取消、queued 条件取消） |
| I1-E | I1-025~034、040 | 正向 + **恢复** + 负向（Executor 启动握手、残留进程告警） |
| I1-F | I1-035~040 及全量回归 | **恢复**（Worker 重启、Outbox 重放、多 Worker 并发）+ 历史只读 |

**阻塞**：~~I1 正文需 V1.1 并入 P1~P4（待决 B-1）~~ **已解除（2026-11-04，B-1 默认并入已实施 + I1-B/C/E/F 第二批落地，正文回填 V1.2 §19）**。

**I1 全链落地结果（2026-11-04，两批）**：
- **第一批（I1-A/D，正文 V1.1 §18）**：统一门禁三入口 + 归档事务（P1/P2/P4/N-2）+ 外部取消；5 例全绿。
- **第二批（I1-B/C/E/F，正文 V1.2 §19）**：
  - **I1-B**：TaskReady / enqueue_review 两入口改「先锁 Project 行」（I1-007/008/009/012/013/035/036）；
  - **I1-C**：`claim_core` 两阶段协议——阶段 A 无锁发现候选 Project（LIMIT 32）→ 阶段 B 锁 `core.project` 行 → 查 archived → 重新查候选（SKIP LOCKED）→ 锁 Task（I1-010~014）；
  - **I1-E**：启动确认先锁 Project 行（归档先落定 → 拒确认 + 终止子进程，I1-025/026/028）+ 持久化清理账本 `delivery.task_run_cleanup`（迁移 0035；无时间窗、退避重试 8 次、needs_human 可查询可恢复，I1-030/031/032/033/040）；
  - **I1-F**：归档抑制 + 合法 ACK（I1-035/036）在 infrastructure 测试；重放死信幂等（I1-037）、清理登记幂等（I1-038）、重启继续（I1-039）由既有幂等/持久化覆盖。
  - 验收：`run_claim_lifecycle.rs` 3 例 + `outbox_handlers.rs` 11 例（含 2 例新增）+ `run_executor_handshake.rs` 5 例 + `project_archive.rs` 5 例。
  - 进程级实操（真实 PID/身份锚点）已由 T-032 CodexExecutor 落地；两机端到端实测归 **T-090**。

### 3.2 I2 · Finding 人工确认（33 + 15）

| 归属 TASK | 验收条目 | 覆盖的类型 |
|---|---|---|
| I2-B | I2-001~022（领域规则部分） | 正向 + 负向（Agent 直建 user_confirmed 被拒；Agent 转述仍 PENDING） |
| I2-C | I2-023~033（V1.1 新号段）+ V1.0 原 023~027（待 B-3 重编为 034~038） | 正向 + **并发**（条件 UPDATE）+ 幂等（operation_id）+ 审计（待补充状态由审计推导） |
| I2-D | I2-D 双入口专项（REST `add_finding_handler` + MCP `pm_discovery_submit_findings` **必须同修**） | 负向（跨项目拒绝）+ **零副作用**断言 |
| I2-E | I2-001~033 全量 + §15.3 十五场景 | 并发（确认/拒绝竞争、归档竞争）+ 幂等 + 审计（历史审计缺失处理）+ REST/MCP 一致性 |

**铁律（不可降级）**：I2-D 的双入口 P0 修复**不得降级为普通功能**，REST 与 MCP 必须同修防分叉（I2 V1.1 §18）。
**身份铁律**：R2-B 必须落 `core.principal.kind=='human'` 的**可信身份**判定，而非字符串比较（防 Agent 伪造）。

### 3.3 I3 · Git 权限与仓库写入授权（24，**V1.1 简化裁决后**）

| 归属 TASK | 验收条目（按 I3 §0.0.6 处置） | 覆盖的类型 |
|---|---|---|
| I3-B | I3-001~006（**改写**：无授权概念 → 无自申请入口；范围内 verified 仓 → 生成有效快照） | 负向 + 数据 |
| I3-C | I3-007~012（**保留 I3-004/005/013，合并 I3-007~010**；写操作不分级） | 正向 + 负向 |
| I3-D | I3-013~017（**保留**：快照 SHA-256 内容寻址、不可变、历史可查） | 数据（列约束）+ 正向 |
| I3-E | I3-018~021（**保留 I3-017/018，I3-016 改写为「状态变化即时阻断」**） | 负向（越权执行被拒）+ 幂等 |
| I3-F | I3-022~024（**改写**：授权授予/撤销审计随授权模型一并撤下；保留快照生成审计与归档/取消联动） | 审计 + 负向 |

**V1.1 简化裁决（2026-11-04，用户）**：「不区分，都可以读写。因为这部分太复杂。」
→ 六能力/依赖闭包/逐主体授权表与 REST **全部废止**；模型收敛为
`可访问仓库 = Task 关联仓 ∩ verified`，档位恒为 `read_write`。
映射明细见 `docs/ui/smark-pm-s01-i3-git-permission-and-repo-write-authorization-v1.md` §0.0.6。

**现状事实（2026-11-04 更新）**：I3-B~I3-E **已实施**（简化后）：
`pm-domain::repo_access` + `pm-application::repo_access`（快照构建 + 执行期强校验）、
迁移 `0032`（`policy_snapshot_hash`）+ `0033`（撤下授权表）、执行器物化仓库前按快照过滤。
未覆盖：I3-019/I3-020（直接 Git 绕过与凭据隔离）——属 **T-090 两机实测**项。

### 3.4 I4 · Project Goal（24）

| 归属 TASK | 验收条目 | 覆盖的类型 |
|---|---|---|
| I4-B | I4-001~006（正式 Goal 识别、唯一性、与普通 Knowledge 区别） | 正向 + 数据（唯一约束） |
| I4-C | I4-007~012（复用 DocumentProposal、基础版本不得静默覆盖、归档只读） | 正向 + 负向 + 幂等 |
| I4-D | I4-013~018（多 Agent 并发、版本冲突识别、Project 级并发控制） | **并发** + 幂等 |
| I4-E | I4-019~021（Agent 上下文读取、区分正式与待确认、记录 Run 使用的 Goal 版本） | 正向 + 数据（一致性） |
| I4-F | **I4-019/020**（归档只读、归档竞争——**口径勘误见 ledger D-C-04**：本行原登记 I4-022~024，与设计稿 §12 验收表不符） | 负向 + **并发** + 审计 |

**已有机制复用判定**：I4-D 的"确认现有写入协调 / 版本冲突处理 / 基础版本校验"三项**已被 T-060 覆盖**（`base_revision` + Project 级 advisory lock + `knowledge_write_operation` 幂等账本），实施时只做验证，不重建。

**I4-F 落地结果（2026-11-04）**：知识写入门禁覆盖**四个入口**
（`create_document` / `submit_proposal` / `confirm_redline_proposal` / `finalize_committed`）；
守卫与归档事务共享 `core.project` 行锁 → 提交与归档线性化，**归档之后不出现新的
published 修订**；归档后正式 Goal / 提案 / 修订历史仍可读。
验收：`apps/pm-api/tests/project_goal_archive.rs` **5 例全绿**。

### 3.5 I5 · 项目总控台（36）

| 归属 TASK | 验收条目 | 覆盖的类型 |
|---|---|---|
| I5-C | I5-001~025（聚合查询、权威来源、空状态、数据新鲜度、权限过滤、活动去重、部分失败） | 正向 + 负向 + 数据 |
| I5-D | I5-026~031（前端区块、下钻、归档只读） | 正向 |
| I5-E | I5-032~036（性能、并发变化、归档状态、部分数据源失败） | **并发** + 负向 + 性能 |

**实施顺序约束**：I5 **最后实施**（I5 V1.1 §11.2 `I1→I2→I3→I4→I6→I5`）。且 I5-B/I5-C/I5-D 的前置（I5-A 代码事实核验、`docs/s01-i5-read-model-contract.md`）**当前不存在**，是 S01-3 的实际起点。

**I5-A/B/C 落地结果（2026-11-04）**：
- **I5-A**：`docs/s01-i5-code-facts.md`——14 项核验全答，缺口清单 `I5-G-01~24`（端点/查询/字段/索引/前端契约五类）。
- **I5-B**：`docs/s01-i5-read-model-contract.md`——六数据块（`project_summary` / `initialization_progress` /
  `work_summary` / `human_action_summary` / `risk_summary` / `recent_activity`）、每字段权威来源、
  展示态规则、空状态、新鲜度、权限、去重、部分失败全部定稿；**裁定**：单一聚合端点 +
  只对活动流分页；**无百分比**；初始化完成度 `not_determinable`（无项目类型/要求配置）；
  Goal **不下「已确认」结论**（领域无该状态字段）。
- **I5-C**：`pm-application::command_center` + `GET /v1/pm/projects/{id}/command-center`
  + 迁移 `0034`（审计项目维度索引 + Finding 项目维度索引）；按块降级（`ok`/`unavailable`），
  全部 SQL 带 tenant 谓词（契约 C-2）；验收 `apps/pm-api/tests/command_center.rs` **6 例全绿**。
- **未完成**：I5-E 全量验收与性能（I5-032~036 的全量映射 + 权限/部分失败/并发/性能专项）。

**I5-E 落地结果（2026-11-04）**：`apps/pm-api/tests/command_center_acceptance.rs` **6 例全绿**
（初始化收缩/再展开、活动去重 + 稳定分页 + 可追溯、Outbox 不入活动、缺失数据四态区分、
多领域原始状态不混淆、归档后历史可读且不再产生新活动）。

**I5-001~036 逐条覆盖映射（2026-11-04）**

| 编号 | 覆盖方式 | 证据 |
|---|---|---|
| I5-001 | 新建项目正常展示（六块全 ok） | `command_center.rs::new_project_renders_all_blocks_and_zero_repository_is_legal` |
| I5-002 | 零仓库合法：仓库「未开始」+ 分析「不适用」，不判失败 | 同上 |
| I5-003 | 多仓库区分（repository_count / verified_repository_count 逐仓口径） | `command_center.rs` + `command_center.rs::project_summary` |
| I5-004 | 分析进行中 → 初始化事项「进行中」 | `repo_access`/`project_import` 事实 → 契约 §4.2 `code_analysis` |
| I5-005 | 分析失败 → 「存在阻塞」+ 风险项 `analysis_failed` | 契约 §4.2/§4.5 |
| I5-006 | 成功后再次失败 → 以**最新**作业判定（不看历史成功） | 契约 §4.2（最新作业口径） |
| I5-007 | 无 Goal → `goal.present=false` + 空状态，不创建虚假 Goal | `command_center.rs`（goal.present=false） |
| I5-008 | Goal 已确认 → 只展示 I4 权威数据（published 修订） | `repo_access`/`project_goal` 事实 → 契约 §4.1 |
| I5-009 | Finding 待确认 → `display_state=PENDING` 入待人工处理 | `command_center.rs::pending_findings_appear_as_human_actions_without_cross_project_leak` |
| I5-010 | Finding 待补充 → `display_state=REVISION_REQUESTED`（I2 审计推导） | 同上（revision 项断言） |
| I5-011 | 已确认/拒绝 → 不再计入待办（查询只取 PENDING） | 同上 + 查询条件 |
| I5-012 | 多 Agent 并行 → 活跃 Run 列表（多条） | `command_center.rs::active_runs_report_the_task_assignee_as_agent` |
| I5-013 | Run 重跑 → 历史 Run 分列 `recent_runs`，不覆盖 | 契约 §4.3 |
| I5-014 | Delivery 待验收 → `delivery_pending` 入待人工处理 | `command_center_acceptance.rs::multi_domain_backlog_keeps_each_domain_state` |
| I5-015 | 无待办 → 空状态文案 | `command_center.rs`（前端 5 例）+ `command_center_acceptance.rs` |
| I5-016 | 数据源部分失败 → 该块 `unavailable`，其余照常 | 契约 §2/§7 + 前端「不可用≠0」用例 |
| I5-017 | 跨 Project 不泄露 | `command_center.rs::pending_findings_...`（他项目 Finding 不出现） |
| I5-018 | 无权限 → 404 | `command_center.rs::non_member_cannot_read_command_center` |
| I5-019 | 归档 → 只读态 | `command_center.rs::archived_project_is_reported_read_only` |
| I5-020 | 归档后清理状态 → 风险项 `archive_cleanup` | 契约 §4.5（残留活跃 Run 计数） |
| I5-021 | 并发变化 → 每块 `as_of` + 详情页重读权威数据 | 契约 §7.4 |
| I5-022 | 无无依据百分比 | `command_center.rs`（断言无 percent 字段）+ 前端用例 |
| I5-023 | 不适用事项不计入未完成 | `code_analysis=not_applicable` 且 `required=false` |
| I5-024 | 状态来源可追溯 | 契约 C-5（每字段 source）+ `source_state` 保留 |
| I5-025 | 最近活动只展示真实业务事件 | `command_center.rs::recent_activity_filters_whitelist_and_dedups`（白名单外不展示） |
| I5-026 | 首屏优先级：待人工处理先于统计卡片 | 前端用例（顺序断言） |
| I5-027 | 初始化完成 → 自动收缩 | `command_center_acceptance.rs::initialization_progress_collapses_and_expands_again` |
| I5-028 | 新增事项 → 重新展开 | 同上 |
| I5-029 | 必需项未定义 → 不伪造完成 | 同上（`completion=not_determinable`） |
| I5-030 | 活动重复来源 → 去重 | `command_center_acceptance.rs::activity_is_deduped_stably_paged_and_traceable` |
| I5-031 | 活动分页：稳定排序 + 连续分页 | 同上（id DESC tiebreaker + 两页不重叠） |
| I5-032 | 活动来源可追溯至审计 | 同上（source_type / source_id / summary） |
| I5-033 | 不依赖 Outbox 保存活动历史 | `command_center_acceptance.rs::outbox_events_never_appear_in_activity` |
| I5-034 | 区分无数据 / 未接入 / 无权限 / 加载失败 | `command_center_acceptance.rs::missing_data_is_distinguished_from_not_applicable_and_forbidden` + 前端「不可用」用例 |
| I5-035 | 多领域待办不混淆原始状态 | `command_center_acceptance.rs::multi_domain_backlog_keeps_each_domain_state` |
| I5-036 | 归档项目历史可查、不可新建活动 | `command_center_acceptance.rs::archived_project_keeps_history_but_accepts_no_new_activity` |

> **性能项说明**：I5-E 的「页面性能」以「单端点 + 分项查询 + 索引（迁移 0034）」为实现手段；
> 未做压测基准（无性能基线数据）。**建议后续补**：以真实项目数据量跑一次 p95 采样，
> 关注 `audit_log` 项目维度索引是否命中（`EXPLAIN` 可验证）。此项登记为遗留，不阻塞 S01。

**I5-D 落地结果（2026-11-04）**：`frontend/src/api/command-center.ts`（类型 + 客户端）+
`frontend/src/shell/views/pm/command-center/index.vue`（六区块）+ 路由
`/projects/:projectId/command-center` + `pm.commandCenter` i18n 命名空间（zh-CN / en）；
前端 `vitest` **5 例全绿**（首屏优先级顺序 / 区块不可用显示「暂不可用」而非 0 /
归档只读提示 / 初始化无百分比且完成度不可判定 / 空集合空状态）；
`vue-tsc --noEmit` 干净；前端全量 **94 文件 / 569 测试全绿**。

### 3.6 I6-B · Discovery / Import（16 + 7 条 DB 级负向）

| 归属 TASK | 验收条目 | 覆盖的类型 |
|---|---|---|
| I6-B-01 | I6-B-036~039（待补齐） | 正向 + 负向 + 并发（active 唯一性、占有/释放 `rows_affected`） |
| I6-B-02 | I6-B-040~043（待补齐） | 数据 + 恢复（Attempt 轮次留档） |
| I6-B-03 | I6-B-044~045（待补齐） | 数据（write-once SHA） |
| I6-B-04 | I6-B-046~047（待补齐） | 数据 + **CHECK 判别器** |
| I6-B-05 | I6-B-048（待补齐） | 负向（跨项目越权）+ 审计 |
| I6-B-06 | I6-B-049（待补齐） | 幂等 + 审计 |
| I6-B-07 | I6-B-050（待补齐） | 数据（空库初始化 + 0028 升级双路径 Schema 一致） |
| I6-B-08 | I6-B-051（待补齐） | 数据（全部 FK 换轨生效） |
| I6-B-09 | DB 级负向测试 1~5 + 追加 2 条（见 Migration Registry §3.3） | 数据（约束违例，直接打 DB） |

**独立于 I6-B 编号的 7 条 DB 级负向测试**（merge-spec §4）必须在 I6-B-09 中以"直接 INSERT/UPDATE 断言约束违例"的方式落地，**不经 REST/MCP**。

### 3.7 S01-0/1/2/5/6 的无编号验收（按七类口径直接执行）

以下任务在现有设计稿中无编号验收条目，S01 为其**新建**验收条目段（编号段 `S01-Axxx`）：

| 任务 | 新建验收段 | 要点 |
|---|---|---|
| S01-W13（Project 成员授权） | S01-A001~A016 | 正向（成员可访问项目）/ 负向（跨租户、跨项目 404、授权前提、非管理者、Agent 行）/ 并发（并发授予唯一赢家、租户撤权竞速无悬空）/ 幂等（重复授权不产生重复关系）/ 恢复（审计失败整体回滚）/ 数据（role CHECK、PK）/ 审计（授权、撤销、级联撤销）。**2026-11-03 落地 15 例全绿** |
| S01-W15（撤权即时生效） | S01-A011~A015 | 撤权后立即领取被拒；历史 Run 保留授权快照；但**不得**凭历史快照继续执行已撤销操作 |
| S01-W23（仓库独立验证） | S01-A016~A020 | 一仓失效不影响他仓；归档不破坏历史执行记录 |
| S01-W25（Git 基准） | S01-A021~A025 | 基准 write-once；多仓独立基准 |
| S01-W60（项目级 Delivery 聚合） | S01-A026~A032 | 按项目展示待验收/可验收/阻塞；逐仓交付、预期与实际 Commit、测试结果、证据、Review 状态 |
| S01-W62（失败恢复） | S01-A033~A038 | 租约过期回收；失败重试创建新 Run；历史不被覆盖 |
| S01-W63（全链路审计） | S01-A039~A050 | 13 条追溯链（谁建 Project / 谁关联仓库 / 谁建 Task / 谁领取 / 谁执行 / 用哪个 Commit / 改哪些文件 / 哪些测试通过 / 哪些证据 / 谁审核 / 谁批准发布） |
| S01-W64（E2E） | S01-A051~A060 | 端到端：建 Project → 关联多仓 → 起 Discovery → Agent 领取 → MCP 取上下文 → Codex 执行 → 回传证据 → 生成 Delivery → 人工验收 → Knowledge 发布 |
| I1-D（归档） | S01-A061~A065 | 归档后：入口关闭、并发安全、凭据撤销、进程可控、消息安全、审计完整、故障可恢复（**七项须分别证明**，I1 §17.3） |
| I2-B/D（Finding 定态） | I2-023 / I2-024 / I2-026 | 创建初态由入口派生：human 直建 user_confirmed → CONFIRMED；Agent 转述 → PENDING；外部不可指定初态（字段已从契约删除）。**2026-11-04 落地 3 例全绿**（`crates/pm-application/tests/discovery.rs` i2_023/024/026），REST 与 MCP 同批改造防分叉 |
| I2-C（统一决定服务） | I2-014 / I2-019 / I2-020 / I2-025 / I2-029 / I2-030 / I2-035 / I2-036 / I2-037 / I2-038 / I2-034 | 单事务决定（Finding+Q/A+审计）；审计 `finding.decided` 带 before/after；operation_id 重放零副作用；非真人被拒零审计；同值重复 no-op；跨 Session 404；裸导出删除；F3 原子性可观测（读取与 after_data 一致）；REST 契约兼容。**2026-11-04 落地 4 例新绿 + 既有 15 例回归全绿**（`crates/pm-application/tests/discovery.rs`） |
| I2-E（并发全量验收） | §15.3 场景 1~6 / 12 / 14 / 15 | 并发决定行锁串行终态唯一；归档项目拒绝零写入；跨 Session 404；同 operation_id 不同内容幂等吸收；无审计保守展示；AI open question 不参与待补充推导。**2026-11-04 落地 6 例新绿**；场景 4/5/7/8/9 由 I2-B/D + DB 负向覆盖；场景 10（审计失败回滚）由同事务结构性保证 |
| I4（Project Goal） | I4-001/002/007/012/019（部分） | 每项目唯一 Goal 文档（UNIQUE project_id+doc_key）；ensure 幂等；Agent 提案强制红线挂起不自动发布（Q2）；人工确认发布 rev2；过期 base 乐观并发拒绝；RunContract.goal 快照（缺失=null）。**2026-11-04 落地 3 例新绿**（`apps/pm-api/tests/project_goal.rs`） |
| I3-B/C/D/E（**V1.1 简化模型**） | I3-001~021（按 §0.0.6 改写） | 可访问仓库 = Task 关联仓 ∩ verified；档位恒 `read_write`；claim 落快照 + SHA-256（`policy_snapshot_hash` + `repository_policy_snapshots`）；执行期 = 快照 ∩ 现状（仓库失验 / 项目归档 → 拒绝）；快照不可变；执行器物化仓库前按快照过滤。**2026-11-04 落地 4 例单测 + 12 例集成全绿**（`pm-domain::repo_access` + `apps/pm-api/tests/run_scope_snapshot.rs` + `repo_access_enforcement.rs`）。**同日按用户简化裁决撤下六能力/授权表/REST（迁移 0033）** |
| I1-A/D（归档生命周期） | I1-001~006（守卫）+ I1-015~024（归档与 Run 取消，部分） | 统一门禁三入口接入；归档事务（P1 queued 条件取消 / N-2 Import 终态化 / P4 名额释放）；P2 外部取消入口（200/409）；重复归档 409；P3 残留进程告警。**2026-11-04 落地 5 例全绿**（`apps/pm-api/tests/project_archive.rs`） |
| I1-B/C/E/F（**第二批**） | I1-007~014、025~034、035~040 | **I1-B** 创建入口先锁 Project 行；**I1-C** claim 两阶段协议（候选发现 → 锁 Project → 重查 SKIP LOCKED）；**I1-E** 启动确认行锁守卫 + `delivery.task_run_cleanup` 持久化清理账本（无时间窗/退避/needs_human）；**I1-F** 归档抑制 ACK + 重放/重启/幂等覆盖。**2026-11-04 落地 3+2+5 例全绿**（`run_claim_lifecycle.rs` / `outbox_handlers.rs` 新增 2 例 / `run_executor_handshake.rs`）；正文回填 V1.2 §19，偏差 V-3/V-4 闭合 |
| I6-B-05/06（导入契约） | S01-A066~A075 | 导入契约与审计：`repository_ids` 必填（缺省/空 → 422）、跨项目仓库拒绝（422）、导入集合在创建时固化（Source + 首轮 Attempt）、初始化操作被占有、同幂等键重放同一导入（`replayed=true` 且不新建）、`proposed` 态 terminate 后释放名额、`cancel` 带 reason 写入审计、导入启动/初始化占有/终止三类审计行可追溯。**2026-11-03 落地 7 例全绿**（`apps/pm-api/tests/import_contract.rs`） |

---

## 4. S01 总体验收（总体计划 §15）

| 段 | 要求 | 判定依据 | 当前状态 |
|---|---|---|---|
| **A 设计基线** | I1–I6 无未解决关键接口冲突；权限边界统一；状态机边界统一；数据模型明确；未决事项有责任与门禁 | 实施计划 §1、§4、§9 | ⚠️ A 段未全绿：D-10（两套状态机）、D-7（Git 能力零基础）为**已知未解决项**，均已归属 TASK 且有门禁 |
| **B 数据库** | Migration 顺序完整；旧环境升级通过；新环境初始化通过；关键 FK/UNIQUE/CHECK 有测试；I6-B 复合外键落地 | Migration Registry §3.2 冻结门槛 9 条 | ❌ 未开始（0031 未写） |
| **C 执行闭环** | REST 边界明确；MCP 上下文与回传完整；Worker 可恢复；Codex CLI 可执行；多仓库基准与交付可追溯 | Interface Matrix §3、§4 | ⚠️ 部分：Context/进度/结果 ✅、**回传证据 ❌**（D-5/D-12）、bootstrap 层 ❌ |
| **D 知识闭环** | 多 Agent 并行生产；Coordinator 统一协调；冲突可识别；审核发布可追溯；失败可恢复 | `knowledge_write_coordinator.rs` + `knowledge_proposal/` | ✅ 基本达成（`base_revision` + advisory lock + 幂等账本） |
| **E 交付闭环** | 项目级交付可查；逐仓库验收；失败 Run 可恢复；审计链完整；E2E 通过 | §3.7 的 S01-A026~A065 | ❌ 未开始（项目级聚合缺失 D-13） |

**当前 S01 终态判定：未达成。** 已达成的是 S01-0（基线）与 S01-5（知识闭环）的大部分；缺口集中在 S01-1（授权）、S01-3（Discovery/Import）、S01-2 的 Git 能力部分。

---

## 5. 验收执行约定

1. **类型覆盖要求**：每个正式 TASK 的验收至少含正向 + 负向 + 审计三类；涉及状态变更的加并发；涉及唯一约束的加数据；涉及外部进程的加恢复。
2. **执行层级**：单元测试（纯逻辑）/ 集成测试（真实 PG，事务级）/ DB 级约束测试（直接打库）/ E2E（真实栈）。
3. **DB 约束测试的写法**：不通过 REST/MCP 制造违例，而是直接 `INSERT`/`UPDATE` 并断言 PostgreSQL 错误码（`23503` FK 违例、`23505` 唯一违例、`23514` CHECK 违例）。
4. **并发测试写法**：真实多连接（`tokio::join!` 两个独立 pool 连接或两个进程），断言**最终唯一赢家** + 审计行数正确。
5. **幂等测试写法**：同一 `operation_id` 重放 N 次，断言业务表行数不变 + `ai.knowledge_write_operation` 记录 `Replayed` + 第二次不产生新副作用。
6. **审计测试写法**：断言 `governance.audit_log` / `governance.tenant_audit_log` 的四列（租户、主体、来源种类、来源引用）+ 结果字段，**与操作一一对应**。
7. **证据要求**：每个 TASK 的 Delivery 需附「验收命令 + 实际输出」，**不得以"我认为通过"替代**（总体计划 §13.1 第 8 条）。
8. **门禁**：Migration 变更类任务必须先过 G-DDL / G-DATA，再进入代码评审。
