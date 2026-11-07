# S03-ACP 通信面切片实施设计 V1.1《ACP-01/02/08》独立复审报告 V1.0

| 项目 | 内容 |
|---|---|
| 评审对象 | `docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp01-02-08-v1.md`（V1.1，R-ACP-1~9 已裁 2026-11-07） |
| 评审性质 | 独立复审（前轮评审结论"修订为 V1.1 再进行独立评审"的闭环动作）+ 代码事实复核 |
| 代码基线 | ai-pm `feat/s03-acp-comm-face` @ 1234162（含 84c67e9 平台通信面 + 1234162 Runner 客户端，2026-11-07 已 push） |
| 前置输入 | 初步独立评审 V1.0（P0×4 / P1×5 / 验收 ACP-437~445 / R-ACP 裁决建议）；用户裁决 R-ACP-1~9 全落 |
| 结论 | **设计 V1.1：通过**（P0×4 / P1×5 全部闭合，可冻结为 ACP-01/02/08 实施设计基线）；**实现：附 1 项 P1 修复项（F-1）**，不阻塞本分支，阻塞 ACP-05 Continuation 实装前的真实 Continuation 启用 |

---

## 1. 结论摘要

1. 前轮 **P0×4 全部闭合**（§2），且经代码复核确认设计文本与实现一致（84c67e9）。
2. 前轮 **P1×5 全部闭合**（§3），P1-1 三阶段模型在平台侧以 acked 从严判据落地并有测试钉住（ACP-441）。
3. **R-ACP-1~9 裁决记录完整自洽**（§4）；R-ACP-7 对基线 V1.3.1 §11.4 的操作化（仅冻结新 Session 启动、回复提交开放）已按用户裁决入档，并已消除与 §4.4（回复先行持久化）的矛盾——基线 D-01~D-07 未被触碰。
4. **代码事实复核通过**（§5）：迁移 0046、单事务创建链、N-1 三落位、身份双轨、四新工具契约与设计文本对齐；实现超出前轮要求的验收覆盖（ACP-427~431/433/437~439/441 有自动化测试钉住）。
5. 新发现 5 项（§6）：**F-1 [P1]**（Runner continue 启动前置检查缺失）+ P2×4。无一推翻设计，均不阻塞本设计基线冻结。

---

## 2. P0×4 闭合核验

### P0-1 N-1 屏蔽判据矛盾 — 闭合 ✅

- **设计**：V1.1 §4.1 弃 status 推导，改为持久化 `task_run.scheduling_shield`（none/human_wait/start_wait/timed_out）+ 六情形决策表；CANCELLED（Task 活）→ none 明确入表；timed_out 未处置保持屏蔽（ACP-439）。
- **代码**：迁移 0046 `scheduling_shield` CHECK 四值 + 评注注明"非 Run 状态机扩展"（`migrations/0046_acp_communication_face.sql`）；转换由 InteractionService 事务内显式维护（`interaction.rs` report ⑤→human_wait、record_response→start_wait、sweep→timed_out、cancel/consumed→none）；S1 poll 排除（`run_executor.rs` SQL `AND r.scheduling_shield = 'none'`）、S2 claim B3 分支拒重领（`claim.rs` candidate_shield 检查，Model B skip/Model A TaskNotReady）、S3 由 claim_core 候选拒绝覆盖。
- **测试**：acp427_428（poll 跳过 + 重领拒绝 + Model A 拒绝）、acp439（超时不自动重领）。

### P0-2 Worker Command delivered 无重投 — 闭合 ✅

- **设计**：§3.2 领取租约（claimed_by/claimed_at/claim_expires_at/claim_count），claim 过期同 command_id 重投、自方无条件重领、claim 过期 ≠ failed。
- **代码**：`worker_command.rs:172` poll_commands 原子 UPDATE（`status='issued' OR (delivered AND (claim_expires_at < now() OR claimed_by=$3))`）；failed 仅由最终失败条件产生；基线四态未动（CHECK 约束）。
- **测试**：acp437（崩溃重投 attempt=2、同 command_id、worker 绑定、acked 后不再投递）。

### P0-3 租约与命令身份矛盾 — 闭合 ✅

- **设计**：§3.3 身份双轨——命令面不要求 lease token，改 Client 凭据（V1 过渡：/mcp 中间件 + 成员校验）+ worker_id 绑定 + `lease_generation` 只读比对；执行面五工具维持 T-032 双校验。
- **代码**：`poll_commands`/`ack_command` 均 `worker_id = $` 绑定 + `c.lease_generation = $run_generation` 只读比对；`pm_report_interaction`/`pm_report_consumption` 走 `verify_lease`（token+generation）。工具签名无 lease_token（命令面）。V-05-1 升级路径已在工具 doc 注明（仅替换身份来源，契约不变）。
- **测试**：acp437 含"租约过期 + claim 过期下命令仍可达"（ACP-438 语义）。

### P0-4 标记文件路径与多事件归档 — 闭合 ✅

- **设计**：§1.2 路径 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}/.aipm/interaction.json`、schema_version/64KiB/reason 枚举校验、event_id 键控归档、重启扫描、改名失败恢复、权威性声明。
- **代码**：`adapter.rs` find_unreported_marker（两层路径含 workspace_path 段）、parse_marker（schema/64KiB/必填校验）、archive_marker（event_id 键控）、quarantine_marker（非法隔离）；侦测返回 tri-state——解析失败必须浮现（防静默滞留）；Runner 改名失败保留原文件下轮幂等重报（main.rs tick ②，ACP-442）。
- **测试**：marker_file_lifecycle（合法解析/归档/隔离/子目录层）。

---

## 3. P1×5 闭合核验

| # | 评审要求 | V1.1 落位 | 代码/测试证据 | 判定 |
|---|---|---|---|---|
| P1-1 | SESSION_CREATED / INPUT_INJECTED / CONSUMPTION_CONFIRMED 三阶段分离；注入失败不得判交付 | §1.4：continue acked 判据 = session_created + input_injected；注入失败立即终止新进程（不变量） | `worker_command.rs:257` ack_command 对 continue 强制双真值；guard 释放点 = continue acked（`scheduling_shield='none'` @ :346）；测试 lifecycle + ACP-441 断言 | ✅ |
| P1-2 | 介入链唯一性升级（retry_of 链、RESPONDED 不允许新建、task_waiting 对齐） | §5.2 五规则 + `chain_open` 持久化列 | `uq_task_interaction_open_chain`（UNIQUE(task_id) WHERE chain_open）；重试旧行让出（retry_interaction superseded）；测试 acp431（开放链冲突）+ acp439（timed_out 保持开放、重试挂链） | ✅ |
| P1-3 | terminate 失败冻结语义拆分（启动冻结 vs 回复提交） | §6.5 + R-ACP-7 用户裁决入档 | `continuation_start_frozen_at` 列；ack terminate failed 落值；回复提交入口不冻结（§8.3 独立管辖）；situation6 测试断言 frozen 落值 | ✅ |
| P1-4 | 文件责任边界（标记文件非权威、Agent 只创建不改写） | §1.2 责任边界段 | task_interaction 为唯一权威；Runner 对 Platform Validation 类拒绝隔离不上报伪造状态 | ✅ |
| P1-5 | 新工具不挂 PM_MCP_READ；权限边界清晰 | §1.6（R-ACP-8 已裁命名） | `pm-api/auth.rs` MCP_TOOL_PERMISSIONS：AGENT_REPORT（report×2）/ COMMAND（poll/ack×2）；注释明确"人类正式确认永不入任何 MCP 权限码"（A*） | ✅ |

---

## 4. 基线一致性与裁决记录

- **D-01~D-07**：逐条对照未发现触碰——同 Run 代次（§4 屏蔽释放走 continue acked，未引入新 Run）、D-02（等待窗口 Deadline 不冻结，§4.3 续租失败为预期）、D-03（terminate 同事务下发，无"确认转人工"分支）、D-04（回复 REST/UI + Runner 消费回执）、D-05（report_interaction 强制 execution_mode='spawn'，Model A 退化路径保留并有对照测试）、D-06（checkpoint 引用登记，文件在 Workspace）、D-07（Revision Log 语义未动）。
- **A\***：MCP 四新工具无任何人类确认语义；`pm_report_interaction` 为 Agent 侧事实上报（§18.4 兼容）。
- **V-05 裁决**：V-05-1（device 升级）未抢跑——命令面 V1 过渡身份已显式标注升级路径；V-05-2③（服务器 Model B 维持现状）与 §0 双形态表的偏差见 F-3。
- **V-6**：全部写入经 pm-application（MCP tools.rs 仅参数转换 + authorize/verify_lease）；未发现绕行路径。
- **禁重建区**：13 态/7 态未改（含实施中发现的关键点：AwaitingDecision→Blocked 非合法边，terminate failed 时 awaiting_decision 保持，由冻结列承载阻塞语义——**正确处理**，作为设计补充记录）；四工具契约零改动（MCP_TOOL_PERMISSIONS 仅增行、smoke 工具清单 19→23 为新增非修改）。

---

## 5. 代码事实复核与验收覆盖

代码对照（1234162）：`interaction.rs`（report 单事务七步全链：Run 归属/形态校验→幂等重放→开放链检查→行插入→guard→Task 迁移→task_waiting→terminate 命令→审计）；`worker_command.rs`（claim 租约/身份双轨/情形 6 内置过滤/acked 从严判据/failed 副作用）；`claim.rs:756`（runner: 前缀 stamp，普通 MCP 领取仍 human 有对照测试）；Runner `adapter.rs`/`main.rs`（标记文件全案、消费回执双证据、N-6 不伪造）。

验收覆盖映射（本报告补充，供测试资产管理）：

| 编号 | 覆盖 |
|---|---|
| 427/428/429/430/431/437/439/441 | ✅ 自动化测试（acp_comm_face） |
| 432 | ◐ 幂等语义经 acp437 重投路径覆盖，显式"重复投递直接 acked"用例未单列 |
| 433 | ◐ situation6 覆盖 failed 副作用；有界重试本身（次数/间隔配置）未测 |
| 434 | ✅（mark_consumed 证据不足拒绝） |
| 435/436 | ◐ 需 V-05-1 Client Registry（436 显式不可测，已声明） |
| 440 | ✅（lifecycle：ack 丢失经防御性 guard 释放 + consumed 事务兜底） |
| 442/443 | ✅（Runner 侧标记生命周期） |
| 444 | ◐ 平台侧覆盖（frozen+Task 阻塞表达）；执行端禁止启动见 **F-1** |
| 445 | ✗ 未测（Task 取消与上报并发）——随 ACP-05/取消联动切片 |

---

## 6. 新发现

### F-1 [P1] Runner continue 启动前置检查缺失（§9.4 八项 / ACP-444 执行端职责）

`apps/ai-pm-runner/src/main.rs:349` continue 分支在收到命令后**直接 spawn 新进程**，未执行：① `interaction.continuation_start_frozen_at` 检查（terminate 失败后 Platform 仍会投递 continue——设计预期，"启动拒绝由执行端落实"，V1.1 §3.5/§6.5）；② §9.4 八项前置（Deadline 有效性、旧进程退出确认、Workspace 可用）。平台侧"拒绝得干净"已具备（情形 6 过滤 + 冻结列 + generation 比对），但执行端闸未落——**真实 Continuation 启用（ACP-05）前必须闭合**，否则 terminate 失败 + Deadline 过期场景下 Runner 会启动未携带合法授权的新进程（恰是 ACP-444 要防的事故）。修复方向：Runner 在 continue 执行前经 `pm_get_run_context`（租约三要素）取 RunContract 的 deadline/stage 做前置，frozen 检查经命令 payload 或新增只读工具（推荐前者：continue payload 已含 interaction_id，可扩 payload 携带 frozen/deadline 快照，避免新工具）。

### F-2 [P2] 命令面过渡身份无持有证明

worker_id（`mcp:{client}:{token8}`）中 token8 是唯一屏障；lease_owner 对项目成员可见（审计/Run 上下文），知道 worker_id 的成员可冒领命令（V1.1 §3.3 已声明过渡形态）。缓解到位（token8 一次性且不落日志），**V-05-1 前可接受**；建议在 V-05-1 迁移批把 `PM_MCP_COMMAND` 与 Client 凭据绑定后关闭此风险。

### F-3 [P2] 服务器 Model B 载体的命令消费/标记侦测未接线

V1.1 §0 双形态声明"Worker 进程内直连"，实现仅接了超时清扫；terminate/continue 消费与标记侦测仅 Runner 载体具备。按 V-05-2③（服务器通道维持现状）与 V0.5 目标形态为 Runner，**判定为显式延后而非违反**——但 §0 表格应补注"V1 实现覆盖 = Runner 载体；Worker 载体的命令消费随服务器需求另行接线"，避免读者按双形态等价理解现状。

### F-4 [P2] 并发上报竞态返回 Db 而非 Conflict

两执行端并发对同 Task 上报时，后到者经 `uq_task_interaction_open_chain` 唯一索引失败，映射为 `InteractionError::Db`（`map_waiting_conflict` 只识别 task_waiting 约束）→ MCP 层 ToolError::Db。语义仍为拒绝（无正确性问题），但 ACP-431 期望 Conflict 语义。建议 map_waiting_conflict 同步识别 open_chain 约束名。

### F-5 [P2] N-6 稳态无可观测面

设计 §1.5 要求 `interaction_unconsumed_total` 告警指标；Runner 侧消费回执被拒后仅 warn 日志、无重试与指标；Platform 侧 RESPONDED 未消费稳态无指标暴露。不阻塞（稳态合法性已定义），随可观测性切片补齐。

---

## 7. 结论与处置建议

1. **设计 V1.1 冻结为 ACP-01/02/08 实施设计基线**（P0×4/P1×5 闭合 + R-ACP 裁决齐备 + 代码事实复核一致）。
2. **F-1 [P1]** 列入实施闭合清单（与 §23.3 同级管理）：**ACP-05 Continuation 实装前必须闭合**；本分支合并不受阻。
3. F-2~F-5 随对应切片处置：F-2→V-05-1；F-3→文档补注（可立即做，一行）；F-4→下次触碰 interaction.rs 顺带；F-5→可观测性切片。
4. 验收缺口（432 显式用例 / 433 有界重试 / 445 并发取消）随 ACP-04/05 与取消联动切片补测。
5. 前轮评审的其他要求复核完毕：建议的 V1.1 修订顺序六步全部执行；验收 ACP-437~445 全数入稿；不重开 D-01~D-07 的纪律双方均遵守。

评审通过标准：P0×4 闭合 ✅、P1×5 闭合 ✅、裁决记录 ✅、代码事实复核 ✅、独立复审结论 **通过** ✅。

---

**复审人**：ZCode 独立评审（同一会话链，非作者视角复核；P0/P1 级发现均给出 file:line 证据）
**日期**：2026-11-07
