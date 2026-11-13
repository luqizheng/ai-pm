# S03-ACP V1.3.1《Task-Based Continuation 正式修订设计稿》独立评审报告 V1.0

| 项目 | 内容 |
|---|---|
| 评审对象 | `docs/scenarios/s03/ui/acp/smark-pm-s03-acp-task-continuation-v1.3.1.md`（V1.3.1 修订设计稿，稿内自称"待独立评审"） |
| 评审性质 | 独立评审（V1.3.1 §23.4 基线冻结门槛之一） |
| 代码核验基线 | ai-pm master @ db466ab（2026-11-06），沿用 V1.3 评审 V1.0 事实清单 F1~F11，关键点重验 |
| 前置输入 | S03-ACP V1.3 评审报告 V1.0（R-1~R-14、Q-1~Q-7）；用户确认的七项裁决（D-01~D-07） |
| 结论 | **通过**（无阻塞项；附 2 项实施设计闭合建议 N-1/N-2、4 项文本修订 N-3~N-6，均不阻塞基线冻结） |

---

## 1. 结论摘要

1. **P0 × 3 全部闭合**，且经代码事实复核确认（§2）。
2. **Q-1~Q-7 七项裁决全部落地且全文自洽**（§3）。其中 Q-2（不冻结 Deadline）与 Q-3（立即终止）与 V1.3 评审的倾向项相反，属用户裁决权范围；本评审不重开裁决，只核验其一致性——两者均已全文一致贯彻并配齐相应机制。
3. **R-4~R-14 共 11 项意见全部收编**（§4），其中 R-14 建议的 ACP-421~426 六项补充验收被逐字采纳。
4. 新发现 6 项（N-1~N-6），无 P0：N-1/N-2 建议增补进 §23.3 实施设计闭合清单；N-3~N-6 为文本修订，随下一版收编。

---

## 2. P0 闭合核验

### R-1 Continuation 与 Run 关系 — 闭合 ✅

- D-01/§3.2 正式裁决为**同 Run 执行代次**；§3.1、§3.2、§5.4、§12.4 全文一致贯彻，V1.3 的四处矛盾表述（正文含糊 / 关系图歧义 / 报文携带原 run_id / 数据关系图并列）全部消除。
- §3.3 将 `continuation_generation` 与 `lease_generation` 分列且职责互斥（我方评审曾建议合并为单一代次概念，专家裁决为分立并明确"Continuation 不管 fencing、Lease 不管业务代次"——分立方案自洽，予以接受）。
- 与代码硬后果的核验（V1.3 评审 §3 R-1 表）：同 Run 模型下 `deadline_at` 沿用原值（不触发 F6 的 claim 重固化）、`{root}/{run_id}/` 目录天然跨代次延续（消解 F7 布局冲突）、`uq_task_run_active` 不冲突（F5）。三种硬后果全部正确规避。

### R-2 代码基线现状 — 闭合 ✅

- §2 建立"已存在沿用 / 本轮新建 / 明确不建"三档；§2.3 明确十项新建对象并加"不得宣称为既有功能"约束；§5 开头声明"消息名称是协议设计，不代表现有仓库已经存在对应事件或工具"；§17.1 同样声明事件为逻辑新增。V1.3"把纸面方案当现状"的系统性问题已根治。
- **§2.2 十二项"已存在"声明逐项对照 F1~F11 复核，未发现失实表述**：Run 7 状态、Task 13 状态、Lease Generation+Token、deadline 首次 claim 固化、`delivery.task_waiting`、Outbox 事件目录 13 项、Inbound Event 存在性、Windows Job Object / Unix Process Group 终止能力、Workspace 按 Run 组织路径——均与代码事实相符。
- §2.4 明确不建 inbox 表、跨进程恢复、TOML 框架，不采用"以 Outbox 直接替代 Worker 命令队列"——与代码事实（F11：无 inbox）和评审建议（R-4 选项①）一致。

### R-3 Workspace 布局与清理 — 闭合 ✅

- §3.5 沿用 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}` 布局——在 D-01 同 Run 模型下自洽，无需重设计布局，"可恢复映射"要求由 Run 目录天然满足。
- §3.5 四条负面清单（不删除/不重建/不复制/不自动清理）+ §15.2 清理前置条件八项 + §15.3 保留期显式悬置（"不擅自裁决具体保留天数……裁决前不启用无条件自动删除"）+ §15.4 将"现状零清理代码"如实记录为暂态事实。达到评审要求的最小定义，保留期悬置方式（显式声明待裁决，而非假装已决）是正确处理。

---

## 3. 七项裁决落地核验（Q-1~Q-7 → D-01~D-07）

| 裁决 | 落地位置 | 一致性核验 |
|---|---|---|
| Q-1 同 Run Continuation | §3.2、§12.4（generation 唯一、每 Interaction 至多一个成功 Continuation、重试复用原 Continuation 幂等关系） | ✅ 全文贯彻 |
| Q-2 Deadline 不暂停不顺延、过期需显式授权 | §9.1（六不）、§9.2（AUTHORIZATION_REQUIRED 流程）、§9.3（RunExecutionAuthorization 实体：Lead/Approver 主体、审计事件、不得复用已消费授权、`authorized_deadline_at` 独立记录不动原值）、§9.4（启动前八项检查）、§17.1（`agent.execution.authorization.granted`）、ACP-423 | ✅ 与 T-032/0023 字面语义零冲突，0023 注释无需修订；逃生阀（授权机制）配齐 |
| Q-3 立即终止 | §11.3 终止流程八步、§4.3 规则 3、§6.3 规则 3（"不得为等 Checkpoint 延迟终止"）、§1.1 目标 3 | ✅ 我方倾向"转人工后杀"，裁决相反；核验确认 §1.1 目标已按立即终止路线改写，无残留"自动回答后不终止"分支（唯一残留措辞见 N-5） |
| Q-4 REST/UI 回复 + Worker/Adapter 消费回执 | §8.3（六项权限检查 + fail-closed）、§8.5 三层机制、§8.6（七类不充分证据 + 禁止伪造） | ✅ 与 A*（正式确认仅可信 REST/UI、MCP 无确认入口、fail-closed）完全一致；V1.3 §12 的 RESPOND_INTERACTION 术语已从全文移除（该工具不存在，F2——移除是正确处理而非遗漏） |
| Q-5 仅 Model B | §1.2（Model A 五不：不终止外部进程/不建命令/不自动建 Session/不建 Continuation，走 `awaiting_decision`/`task_waiting`）、ACP-425、ACP-09 切片 | ✅ |
| Q-6 Checkpoint 在 Workspace + Platform 存引用 | §6.1（正文不落库，Platform 存 ID/路径/Hash/大小/时间）、§6.2 格式、§12.7 登记表、§6.4 冲突以 Workspace 为准 + ACP-426 | ✅ |
| Q-7 历史归档 + Revision Log | §22.1 归档内容四项、§22.2 Revision Log 五版、§22.3 ACP-DEC-003 三问留档（含"旧 Q3 不再有效"声明） | ✅ V1.3 评审 R-14 指出的"ACP-DEC-003-Q1 无档可查"悬置就此解除 |

---

## 4. V1.0 评审意见收编核验（R-4~R-14）

| 意见 | 收编位置 | 判定 |
|---|---|---|
| R-4 命令通道未定义 | §10：命令持久化 + Worker 轮询（采纳我方建议①）、`worker_command` 表、issued/delivered/acked/failed 四态、领取时六项校验、§10.5 Outbox 与 Worker Command 分职表 | ✅ |
| R-5 Model A 未覆盖 | §1.2 + ACP-425 + ACP-09 | ✅ |
| R-6 回复入口与 A* | §8.3、§5.6 回执语义重写为"Agent 已消费"凭证操作 | ✅ |
| R-7 投递模型矛盾 | §8.5 三层（持久化→投递→消费回执）：初始输入仍为 Codex 优先形态，但 Delivery 成为带独立 `delivery_id` 与生命周期的持久记录，启动失败按原 delivery_id 幂等重试——V1.3 §4.5/§11.2 的矛盾消解 | ✅ |
| R-8 Task 状态映射 | §7.1：复用 `awaiting_decision`（进入需 open question、离开需 resolve，与现有门槛 F8 咬合）、不增 WAITING_HUMAN/CONTINUING、状态转换必须经应用服务（S02 V-6 架构约束落文） | ✅ |
| R-9 终止时机 | 裁决为立即终止（D-03），见 §3 Q-3 行 | ✅（裁决反向，已核验一致） |
| R-10 生命周期缺分支 | §7.3 增 CANCELLED（Task 取消/归档联动）、§7.4 唯一性约束（对齐 F3 每任务一条活跃等待）+ `retry_of` 关联 | ✅ |
| R-11 等待期间 Deadline | 裁决为不冻结 + 授权机制（D-02），见 §3 Q-2 行 | ✅（裁决反向，已核验一致） |
| R-12 Checkpoint 存储与可操作性 | §6.1/§6.3：文件在 Workspace、Platform 只登记；"终止前记录"改为软检查（规则 4"可以检查"，规则 3 不延迟终止）——与 D-03 一致 | ✅ |
| R-13 协议细节三处 | §5.3 `exit_outcome` 五值 + 组合语义表（对齐 F9 ExitOutcome，误导性 `exit_code:0` 已移除）；§11.4 终止超时有界重试→blocked+告警+回复入口冻结（`response_frozen_at` 字段落表 §12.1）；§21 环境变量载体、不引入 TOML | ✅ |
| R-14 追溯断裂 + 验收补齐 | §22 归档 + Revision Log；ACP-421~426 六项逐字采纳且场景一致 | ✅ |

---

## 5. 新发现（本版独立评审新增）

严重级沿用惯例：P1 = 实施设计（切片设计稿）前必须闭合；P2 = 应修但不阻塞。

### N-1 [P1] 人工等待窗口与现有 Run 自动调度/回收机制的互斥未定义

**问题**。§7.2 规定等待期间"Worker 可以释放当前执行 Lease"、"Run 不得因单纯的人工介入而被标记为 failed"，但未处理**现有自动化在等待窗口内的行为**：

1. `poll_active_runs` 四信号持续运行（`run_executor.rs:227-357`）：若 deadline 在等待窗口内到期，现有信号①会按 TimedOut 处理该 Run——与"等待人工"状态无人仲裁。
2. Lease 释放或过期后，现有回收路径（B3-2 已启动 → 隔离 + 同 Run 重领，`claim.rs:490-548`、`lease.rs:60-167`）会**直接启动无回复上下文的新进程**——旧进程确实退出了，不违反 §11.1 字面，但恰恰绕过了"人工未回复不得启动"的设计意图（D-03 路线里新进程必须携带人工回复作为初始输入）。
3. Worker 崩溃场景：§14 只写"命令保留，等待 Worker 恢复领取"；若原 Worker 永不恢复，上述自动重领会抢跑。

**建议**。定义"人工等待中的 Run"为自动调度屏蔽态：`poll_active_runs` 与无人值守领取（`AI_PM_RUN_DISPATCH`）必须跳过存在未决 Interaction（PENDING/RESPONDED）且未决 Continuation 的 Run；等待结束后的 Lease 重取，是映射到现有回收路径还是新建专用路径，二选一写明。建议增补验收：

- ACP-427：等待窗口内 Run deadline 到期 → 不触发自动回收/重领，Interaction 生命周期不受 poll 信号干扰。
- ACP-428：Worker 崩溃于等待窗口 → 恢复（或新 Worker 接管）后不得启动未携带人工回复的新进程。

### N-2 [P1] Agent → Worker 的 `interaction.required` 与消费回执物理通道未定义

**问题**。§5.1/§5.6 方向标注 Agent → Worker → Platform，但 Model B 下 Agent 是 Worker spawn 的子进程，"如何发出"未定义：stdout 约定？Workspace 标记文件？MCP 工具调用？现有 19 个 MCP 工具无对应工具（F2），stdout 解析现状仅用于进程收割（ExitOutcome 类）。

**建议**。在切片设计（ACP-01/02/08）中定义物理通道，并为每个 Adapter 写明具体形式。倾向二选一：MCP 工具（Agent 主动上报，天然携带租约身份校验，与 §18.4"MCP 可用于 Agent 提交"兼容）或 stdout JSONL 约定（零新增依赖）。裁决一项即可。

### N-3 [P2] Continuation 状态清单缺 `AUTHORIZATION_REQUIRED` 命名

§9.2 流程图使用 `AUTHORIZATION_REQUIRED` 状态名；§7.5 清单只描述"处于等待授权状态"，未将其列入状态枚举。统一命名，补入 §7.5 状态图与 §12.4 `status` 约束。

### N-4 [P2] 验收集同号不同义

V1.3.1 重定义了 ACP-401~420（与 V1.3 同编号、不同场景——如 403：V1.3="120 秒内收到回复→创建新 Session"，V1.3.1="Interaction 持久化后→创建 Terminate Command"）。§19 要求"原 V1.3 的 ACP-401～420 应与本版逐项对照"，但未提供映射表。建议：附 V1.3→V1.3.1 验收映射表，或新集改用独立编号段（如 ACP-501+），避免测试资产管理歧义。（ACP-421~426 为 V1.3.1 新增编号，无冲突。）

### N-5 [P2] §18.3 与 D-03 的时序张力

§18.3"AI-PM 负责……决定问题是否可以由 AI-PM 自己解决"与 D-03"收到 `interaction.required` 立即终止"存在张力：立即终止路线下，ACP 生命周期内不存在"自动回答后不终止"的分支。建议澄清措辞：该职责属于 ACP 之外的上层 Agent Interaction 能力（V0.1 稿域），发生在 `interaction.required` 发出**之前**；一旦进入 ACP 生命周期，即按 D-03 立即终止。

### N-6 [P2] 消费回执硬门槛与禁止伪造组合出的"RESPONDED 永不 CONSUMED"稳态

§13.1 将 Consumption Receipt 列为 Model B **必须**能力，§8.6 同时禁止无可靠证据时伪造回执——组合结果：对无法提供可靠消费证据的 CLI Adapter，Interaction 将永久停留 RESPONDED、永不 CONSUMED，而 §7.3/§14 未定义该稳态是否合法。建议：①明确"RESPONDED 未消费 + Continuation RUNNING"为合法稳态（不阻塞执行）；②为其定义可观测与告警；③各 Adapter 的具体证据形式在 ACP-08 落地（如：初始输入注入成功 + 进程存活读取 + Agent 日志/会话文件含注入内容）。否则 ACP-415 对部分 Adapter 永久不可执行却无解释。

---

## 6. 代码事实复核声明

本评审对 V1.3.1 所引关键事实独立重验：Run 7 状态 / Task 13 状态与 `awaiting_decision` 门槛 / Lease Generation + Token / `deadline_at` 固化 / `{root}/{run_id}/` 布局 / `ExitOutcome` 五值 / Outbox 事件目录 13 项 / Inbound Event 存在性 / 进程终止机制（Job Object + 进程组）/ 无 inbox 表 / Interaction 与 RESPOND_INTERACTION 零实现——结论与 V1.3 评审事实清单 F1~F11 一致；**V1.3.1 §2.2 的十二项"已存在"声明无失实**。

---

## 7. 结论与基线冻结建议

**通过。**

- §23.1 P0 三项确认闭合；§23.2 Q-1~Q-7 确认落地且自洽。
- 基线冻结前建议完成两件小事（不改变"通过"结论）：
  1. 将 **N-1（人工等待窗口的自动调度互斥）** 与 **N-2（Agent→Worker 物理通道）** 增补进 §23.3 实施设计闭合清单——当前清单十项未覆盖这两项，而它们恰是实施设计最容易漏的集成点；
  2. N-3~N-6 为文本级修订，随下一版（V1.3.2 或正式基线版）收编即可。
- §23.3 其余八项与 V1.0 评审 R-4~R-13 对应良好；ACP-09 Model A 回归切片与 §19.1 安全验收清单保留有效。

评审通过标准已满足：P0 闭合 ✅、裁决落地 ✅、代码事实复核 ✅、独立评审结论 **通过** ✅。V1.3.1 可进入正式基线确认。

---

## 8. 补记（2026-11-06 用户裁决）

用户于 2026-11-06 裁决：**S03-ACP V1.0～V1.2 历史稿不再取回归档。**

相应影响：

- V1.3.1 的 D-07、§22.1、§22.2、§20（ACP-10）、§23.2（Q-7）已同步修订为"仅维护 Revision Log + §22.3 裁决文字记录，不归档原始稿"。
- V1.0 评审 R-14 第 1 点中"ACP 系列历史稿落仓归档"的建议就此关闭；其追溯诉求由 Revision Log 与 §22.3（ACP-DEC-003 三问留档）承载。
- 本评审"通过"结论不受影响。V1.3.1 修订记录行已同步注明独立评审通过状态。
