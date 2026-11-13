# ACP 通信面 V1.2 专家复审意见 · 第 1 块（活风险三项）

| 项目 | 内容 |
|---|---|
| 审阅对象 | `smark-pm-s03-acp-slice-design-acp01-02-08-v1-2.md`（V1.2）第 1 块提问：现存于代码的 3 项活风险（execution_mode 准入缺失 / R-4 consumed 解耦欠账 / R-11 离线恢复死锁） |
| 代码基线 | ai-pm master @ 74c2911（实现先行批 = e1feb5f） |
| 意见到达 | 2026-11-07（用户转发） |
| 收编状态 | **全部收编进 V1.2**（修订记录"专家复审第 1 块意见收编"；§1.3/§3.6/§5.3/§8 ACP-454/455/§9 ACP-C1~C4） |

---

以下为专家意见原文（逐字保留）：

**复审结论：三个对策的基本方向均成立，不要求改变 ACP 现有架构。建议批准进入实施，但需要补充以下约束。**

## 1. ACP-448：execution_mode 准入

批准 V1.2 对策。

1. `execution_mode='spawn'` 作为 `pm_report_interaction` 的执行模式准入判据。
2. `execution_mode='human'` 必须 fail-closed 拒绝，且不得产生 Interaction 或 Command 副作用。
3. execution_mode 负责准入，command route invariant 负责确保命令存在有效投递目标，两者不得混为一套判定。
4. 对历史 human Run 下尚未终局的 orphan terminate 命令实施定向审计和修复，禁止直接删除，应保留审计记录。
5. 命令修复不得未经独立证据就改变 Task 状态。

## 2. R-4：执行恢复与 consumed 解耦

批准 V1.2 对策。

1. `INPUT_INJECTED` 是 Continuation 恢复的业务确认边界，前提是协议明确保证输入已被执行端接受且 Continuation 已进入可执行状态。
2. 在 continue ACKED 同事务内完成 Task→running、waiting_resolved=true、chain_open=false、task_waiting resolve、guard→none。
3. consumed 降为 Interaction 级异步观测事实，只影响 consumed_at 和指标，不再承担 Task 状态迁移、Chain 关闭或 Session 准入职责。
4. 迟到、缺失或重复 consumed 回执不得改变已确认的 Continuation 恢复结果。
5. 旧 Chain 关闭后允许建立新 Chain，旧 Interaction 与新 Chain 必须保持独立关联。
6. 迁移期历史 responded+running 数据必须根据 continue ACK 证据分类修复，禁止仅凭 Task 状态推断恢复成功。

## 3. R-11 / R-16：执行端离线续领

批准 R-16 方案 a，但补充一个关键约束：

**terminate ACKED 不应作为 lease reacquire 的前置条件，而应作为新 Session 启动的前置条件。**

推荐顺序：

1. 同 Client 身份发现名下 start_wait Run。
2. 满足原有前置条件后，同事务续领租约，generation+1。
3. 同事务将未终局命令重新路由至新 lease_owner。
4. 新执行端消费原 terminate 命令并确认旧 Session 已终止；若旧进程已不存在，应支持幂等终止确认。
5. 只有 terminate 已确认或具有等价的旧进程不存在证明，才允许启动新的 Session。

补充要求：

- generation fencing 必须覆盖旧 Runner 的迟到 ACK 和所有状态写入。
- command_id、idempotency_key、payload 保持不变。
- 重新路由不等于重新创建命令。
- claim_count 不重置，重新领取才计入实际投递次数。
- 续领本身不得自动创建新 Session。

deadline 不因离线顺延，予以批准。deadline 到期后禁止自动恢复执行，但已持久化的人工回复必须保留；后续执行需要进入 ACP-07 人工授权。

## 4. 建议纳入 V1.2 的四条横向架构约束

- ACP-C1：执行端命令必须具有有效、可验证的路由目标。
- ACP-C2：INPUT_INJECTED 是业务恢复确认边界，consumed 仅为观测事实。
- ACP-C3：执行端回执必须校验 Run generation，旧 generation 不得污染新状态。
- ACP-C4：租约续领与 Session 重启分离，Session 重启必须经过旧 Session 终止确认。

**最终意见：批准 ACP-448、ACP-450/451、ACP-452/453 的基本设计方向。上述补充约束应在实施设计中闭合，并通过真实栈 E2E 验证，尤其覆盖无 consumed 证据 Adapter、Runner 离线超租约、旧 Runner 迟到 ACK、terminate 未确认时禁止启动新 Session 等场景。**

---

## 收编对照（我方，2026-11-07）

| 专家意见 | V1.2 落位 |
|---|---|
| 1.1~1.2 准入判据 + fail-closed 无副作用 | §1.3 准入行（原文已有，补"不产生 Interaction"） |
| 1.3 准入/路由两层分离 | §1.3 准入行"两层判定分离" + ACP-C1（§9） |
| 1.4 孤儿命令定向审计修复、禁删 | §1.3 准入行"存量治理" |
| 1.5 修复不得无证据改 Task 状态 | §1.3 准入行"存量治理" |
| 2.1~2.3 INPUT_INJECTED 边界 + 四写 + consumed 降级 | §5.3 R-4 核心修订（原文已有，专家确认） |
| 2.4 迟到/缺失/重复 consumed 不改变恢复 | §5.3 专家复审补强 ③ |
| 2.5 新链独立关联 | §5.3 专家复审补强 ④ |
| 2.6 迁移期按 ACK 证据分类修复 | §5.3 专家复审补强 ⑤ |
| 3 关键修正：terminate 前置改位 | §3.6 "terminate 确认的落位"五步时序 + ACP-C4 |
| 3.4 幂等终止确认 | §3.6 时序第 4 步（引 §6.8 既有幂等 + 孤儿强杀） |
| 3 补充：fencing 覆盖迟到 ACK | §3.6 fencing 条目 + ACP-454 |
| 3 补充：改路由≠重建 / claim_count 不重置 | §3.6 "改路由 ≠ 重建命令" |
| 3 补充：续领不自动创建 Session | §3.6 对应条目 |
| 3 deadline 批准 + ACP-07 保留回复 | §3.6 deadline 条目（补专家批准注记） |
| 4 ACP-C1~C4 | §9 横向架构约束表 |
| 终审 E2E 四场景 | §8 末"E2E 必测场景" |

**文档结束**
