# ACP 通信面 V1.2 专家复审意见 · 第 2 块（六项设计取舍）

| 项目 | 内容 |
|---|---|
| 审阅对象 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2.md`（V1.2）第 2 块提问：六项起草方自行拍板的设计取舍（R-7 选 b / token 不持久化 / T3 死守 / 发现面前缀过渡 / 双态 Poll / 链 acked 即关） |
| 代码基线 | ai-pm master @ e9909d3 |
| 意见到达 | 2026-11-07（用户转发） |
| 收编状态 | **全部收编进 V1.2**（修订记录"专家复审第 2 块裁决收编"；§1.6/§3.1/§3.3/§3.6/§5.1/§5.2/§6.6/§8/§9/§10.1/§10.5） |

---

以下为专家意见原文（逐字保留）：

**总体结论：六项设计中，四项维持原方向，两项需要调整。其中 2.4 和 2.5 应修改 V1.2 原设计。**

## 1. R-7：Process 不建表

维持方案 b。

当前 ACP 业务不需要将 Process 提升为独立领域对象。Process 生命周期可以通过结构化、版本化的 worker_command.receipt 和平台审计记录表达。

补充要求：
- Receipt 必须有稳定 Schema 和版本。
- 区分 terminate_requested 与 terminate_confirmed。
- 记录 Run generation，防止旧回执污染新 Session。
- executor-events.jsonl 仅作为执行端诊断证据，不作为平台权威事实。
- 未来出现多 Process 独立控制、迁移或查询需求时重新评估建表。

本轮不新增 task_run_process。

## 2. R-16：Runner 不持久化 lease_token

维持原选择。

不在 Runner 本地加密持久化一次性 lease_token，服务端同主体续领仍是唯一恢复路径。

补充：
- WP-3 增加稳定 Client 身份持久化，不持久化 lease_token。
- Runner 重启后必须恢复原 Client 身份。
- 续领失败采用指数退避与 jitter。
- 持续失败应告警并提供人工处置入口，不允许 panic 导致恢复上下文丢失。
- 身份失效和 deadline 到期应停止自动恢复。

## 3. T3：Deadline 不顺延

维持严格不顺延。

但产品界面可以提供快捷入口，进入 ACP-07 重新授权流程。

约束：
- 不直接修改原 deadline。
- 不自动延长人工决策有效期。
- 已持久化人工回复不得丢弃。
- 新授权必须形成独立审计记录和有效期。
- 未取得新授权之前不得自动启动 Session。

## 4. V-05-1：Client 身份识别

修改原过渡方案。

`lease_owner` 前缀匹配只能用于受控环境的临时兼容，不能作为生产环境的安全身份认证。

建议：
- client_name 必须登记。
- 登记信息必须绑定当前认证主体。
- 发现和续领最终必须验证稳定 Client 身份。
- V-05-1 升格为 R-16 对外启用的硬前置。
- 未完成身份安全能力前，禁止在生产环境开放基于自报 client_name 的续领。

不能把 generation 被抢占简单视为无害的执行干扰。

## 5. pm_poll_commands 增补发现语义

推翻原选择，改为独立工具。

新增：

`pm_discover_runs`

保留原有：

`pm_poll_commands`

原因是 Poll 解决单 Run 命令轮询，Discover 解决 Client 名下待恢复 Run 的发现，两者属于不同业务能力。

Discover 最少返回：
- run_id
- execution_mode
- interaction_id
- interaction_status
- deadline_at
- shield_state
- lease_generation
- lease_status
- terminate_command_id（适用时）
- terminate_status（适用时）

补充要求：
- 只能发现当前认证 Client 有权恢复的 Run。
- 不返回完整人工回复正文和 Command payload。
- 支持分页。
- 重复调用不产生状态副作用。

## 6. Chain 链间关系

维持多条独立 Chain 的模型，但增加弱关联字段：

`previous_chain_id`

约束：
- 只能引用同一 Task 已关闭的前驱 Chain。
- 不得自引用或形成循环。
- 不改变 retry_of 的链内语义。
- 不共享不同 Chain 的生命周期和状态。
- 只服务历史追溯、审计和 UI 展示。

不增加冷却时间，不合并连续介入。

通过 event_id 幂等、Session 标识及 Run generation 校验，区分重复事件和真正的新介入。

## 7. 最终架构裁决

本轮建议：
- R-7：批准维持。
- R-16 Token 策略：批准维持。
- T3：批准维持。
- V-05-1：提高优先级，作为 R-16 启用前置。
- Discover：推翻双态 Poll，采用独立工具。
- Chain：增加 previous_chain_id，不增加冷却合并。

以上修改属于 V1.2 实施设计收敛，不要求改变 ACP 的总体架构。

收敛后重点验证：
1. Runner 重启但 lease_token 已丢失时能够同主体恢复。
2. 未授权 Client 无法发现或抢占其他 Client 的 Run。
3. Discover 不改变原 Poll 契约。
4. deadline 到期不会自动恢复执行，但人工回复仍然保留。
5. 同一 Task 连续发生两次真实人工介入时形成两条独立且可追溯的 Chain。
6. 旧 generation 的事件和回执不能污染新 Session。

**专家最终意见：批准四项原设计方向，修正两项设计取舍后进入实施设计收敛。**

---

## 收编对照（我方，2026-11-07）

| 专家裁决 | V1.2 落位 |
|---|---|
| 1 R-7 维持 + receipt 五点补强 | §6.6（schema 版本/requested vs confirmed/记录 generation/jsonl 降诊断/建表重评条件） |
| 2 R-16 维持 + Client 身份持久化 + 退避/告警/停止 | §3.6 凭据持久化裁决段 |
| 3 T3 维持 + UI 快捷入口五约束 | §3.6 deadline 条目 |
| 4 V-05-1 修正：登记制/绑定主体/生产禁令/升格硬前置/抢占=安全事件 | §3.3 过渡形态风险登记 + §3.6 生产启用门 |
| 5 推翻双态 Poll → `pm_discover_runs`（十字段/分页/无副作用/不含回复正文） | §1.6 工具表 + §3.6 发现面重写 + §10.1 六工具 |
| 6 Chain 维持 + `previous_chain_id`（同 Task 已关前驱/禁环/不改 retry_of/不冷却不合并） | §5.1 表新增行 + §5.2 规则 6 + §3.1（0048 迁移列） |
| 7 收敛后重点验证 6 条 | §8 E2E 必测场景（第 2 块六项 ⑤~⑩） |
| 收敛项清单 | §10.5 更新为九项（①~⑨） |

**文档结束**
