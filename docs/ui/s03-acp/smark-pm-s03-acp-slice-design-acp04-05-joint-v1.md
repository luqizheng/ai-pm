# S03-ACP 实施设计 · ACP-04 / ACP-05 联合数据契约 V1.0
# —— Response Delivery 持久化 × Continuation 正式身份与状态机

| 项目 | 内容 |
|---|---|
| 文档性质 | 联合实施设计稿（WP-D 交付物；**联合架构裁决 2026-11-07：五问①~⑤全部通过 + 身份三件冻结 + 0050/0051 模型裁决**——约束与基线见 `docs/project-ai-team-v2/ai-pm-v05-implementation-baseline-v1.md`） |
| 上位裁决 | ACP V1.3.1（冻结）：§3.3 两种代次分开 · §12.2/§12.3 字段集 · §10.2/§10.3 命令/回执报文；专家复审第 1/3 块（ACP-C1~C4 · ACP-04 与 05 联合设计按数据依赖实施 · 不得临时另建 Session 身份 · ACP-05 边界=不顺带吸收 06/07 职责） |
| 设计基线 | ACP 通信面 V1.2（R-4/R-7/R-8/§5.3 已定案）· ACP-03 已实施（0047） |
| 排期位 | roadmap §3 第三阶段；实施顺序 = 本契约 → **ACP-04 核心持久化（0050）** → **ACP-05 Continuation 状态机（0051）** → ACP-06/07 |
| 代码基线 | ai-pm master @ 0203472（2026-11-07） |

---

## 0. 五契约问题裁决提案（专家检查单逐题）

| # | 问题 | 裁决提案 | 依据 |
|---|---|---|---|
| ① | Delivery 引用 Run 还是 Session？ | **引用 Run + 命令**：`response_id / interaction_id / run_id / continue_command_id` 四锚，**不含 Session 外键**。Session 归因走 `worker_command.receipt`（R-7 b：receipt 是 Process/Session 事实权威） | Session 正式身份是 ACP-05 产出；delivery 若依赖它则实施顺序倒置 |
| ② | Consumption 需要 continuation_generation 吗？ | **表带列、工具不加参**：delivery 表 `continuation_generation INT NOT NULL DEFAULT 1`（基线 §3.3 两代次分开原则）；`pm_report_consumption` 签名不动（租约三要素 + delivery_id + evidence），generation 由服务端自 delivery 行读取比对 | 工具契约红线；R-8（ACP-05 前恒 1） |
| ③ | Delivery 状态参与 Task 完成判定吗？ | **不参与**。Task 完成 = `pm_submit_result` 既有链（与交互域零耦合）；Delivery 状态只参与：重投幂等、`interaction_unconsumed_total` 告警、§5.3 resolution_note 取证。**ACP-C2 消费面收口**：consumed 仅为观测事实，本设计把它落为表行而非推导态 | V1.2 §1.5/§5.3；C2 |
| ④ | ACP-05 消费 ACP-04 持久化结果吗？ | **是（只读消费 + 单列回填）**：Continuation 状态机的迁移证据 = delivery.status + worker_command ack 状态 + 消费回执；经接口查询不重写（唯一写动作 = `continuation_id` 回填，身份注册 + 审计） | 数据依赖方向：04 → 05 |
| ⑤ | ACP-04 依赖 ACP-05 未建的 Session 身份吗？ | **不依赖**。`continuation_id` 列**先落可空**（与 0047 `continue_command_id` 前向兼容同法），ACP-05 落地后回填；**不临时另建 Session 身份**。Session 身份定案三件（**裁决冻结：不允许后续实施增加第四种 Session 身份**）：Platform Session ID = `task_run.executor_session`（R-8 沿用）· 新 Session 会话标识 = `worker_command.receipt.session_id`（执行端回执）· Continuation 正式身份 = `delivery.continuation.id`（0051） | 专家红线原文 |

---

## 1. 代码事实基线

| # | 事实 | 证据 |
|---|---|---|
| F1 | 0047 `interaction_response` 已含 `continue_command_id`（注释明言"delivery 细账随 ACP-04 前向兼容"）+ 双唯一索引（idempotency 重放 / 同 Interaction 单回复） | `migrations/0047_interaction_response.sql` |
| F2 | 基线 §12.3 建议表 `interaction_response_delivery` 全字段 + 五状态 `PENDING/DELIVERING/DELIVERED/CONSUMED/FAILED` | V1.3.1 §12.3 |
| F3 | 基线 §3.3：`continuation_generation`（Continuation 代次）与 `lease_generation`（Run Lease fencing）**必须分开**，`continuation_generation=3 ∧ lease_generation=8` 合法 | V1.3.1 §3.3 |
| F4 | V1.2 挂账点：delivery_id 幂等重投复用（§1.4 不变量）· `pm_report_consumption` 的 delivery_id "落表后必填"（§1.5）· §5.3 resolution_note 引用 interaction_id + delivery_id（实现现仅引 interaction_id） | V1.2 |
| F5 | R-4 已实现：continue acked 同事务 = 命令回执 + shield 释放 + `resume_execution_after_continue_tx`（interaction.rs）——delivery/continuation 推进必须**并入此事务**，不得另开 | WP-A（b619758） |
| F6 | Process/Session 事实权威 = `worker_command.receipt` + 审计 + executor-events.jsonl（R-7 b）；`continuation_start_frozen_at` 落 `task_interaction`（0046） | V1.2 §6.6 |
| F7 | Outbox 事件子集现七项 interaction.*（P2-4）；delivery.* 尚无 | `interaction.rs` |
| F8 | comm_overview 的 `interaction_unconsumed_total` 现由 Interaction 状态推导 | `comm_overview.rs` |

## 2. ACP-04：Delivery 持久化（迁移 0050）

### 2.1 表（基线 §12.3 字段集 + 联合契约修订）

```sql
CREATE TABLE delivery.interaction_response_delivery (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),  -- delivery_id
    tenant_id               UUID NOT NULL REFERENCES core.tenant(id),
    project_id              UUID NOT NULL REFERENCES core.project(id),
    interaction_id          UUID NOT NULL REFERENCES delivery.task_interaction(id),
    response_id             UUID NOT NULL REFERENCES delivery.interaction_response(id),
    continue_command_id     UUID,        -- 0047 同名列镜像；存量行接线期回填
    run_id                  UUID NOT NULL REFERENCES delivery.task_run(id),
    -- ⑤ 前向兼容：ACP-05（0051）身份落位后回填；V1 恒 NULL 不阻塞任何路径
    continuation_id         UUID,
    -- ② 基线 §3.3：Continuation 代次（与 lease_generation 分离）；V1 恒 1（R-8）
    continuation_generation INT  NOT NULL DEFAULT 1,
    status                  TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','delivering','delivered','consumed','failed')),
    attempt_count           INT  NOT NULL DEFAULT 0,
    last_attempt_at         TIMESTAMPTZ,
    delivered_at            TIMESTAMPTZ,
    consumed_at             TIMESTAMPTZ,
    last_error              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);
-- ③/§1.4 不变量：一回复一投递账——失败重投复用同一 delivery_id（attempt_count 递增）
CREATE UNIQUE INDEX uq_response_delivery_one ON delivery.interaction_response_delivery (response_id);
CREATE INDEX idx_delivery_interaction ON delivery.interaction_response_delivery (interaction_id);
CREATE INDEX idx_delivery_unconsumed  ON delivery.interaction_response_delivery (project_id, status)
    WHERE status IN ('delivering','delivered');
```

- **状态语义**（基线五态，迁移规则）：`pending`（回复已持久化、命令未 acked）→ `delivering`（命令 delivered/claim 租约内）→ `delivered`（continue **acked**＝INPUT_INJECTED，F5 同事务）→ `consumed`（消费回执达）；任何阶段失败 → `failed`（附 last_error），**重投 = failed/pending 行复用**（status 回置 + attempt_count+1），**永不新建行**。
- **Outbox 子集声明**（F7 扩展）：`delivery.created` / `delivery.delivered` / `delivery.consumed` / `delivery.failed` 四项，进 §17.1 目录。

### 2.2 服务接口（I-1，pm-application 统一应用服务，V-6）

```text
create_on_response(tx, response_id, interaction_id, run_id, command_id)   -- 0047 回复事务内同建
mark_delivering(tx, delivery_id)                                          -- 命令 claim 时
mark_delivered(tx, command_id, receipt)                                   -- F5：ack continue 同事务
mark_failed(tx, command_id, error)                                        -- 注入失败/重试耗尽；重投复用行
mark_consumed(tx, delivery_id, evidence)                                  -- pm_report_consumption
get_by_id / get_by_response / count_unconsumed(project_id)                -- ④ 只读面（ACP-05 消费）
```

### 2.3 既有路径接线（I-3 挂账清偿表）

| 挂账点 | 现状 | 接线 |
| --- | --- | --- |
| `submit_interaction_response`（0047） | 写 `interaction_response.continue_command_id` | **同事务** `create_on_response`（V1.2 §1.5「同事务 continue 下发」扩展为三写：response + command + delivery） |
| `worker_command::ack_command` continue-acked | F5 恢复事务 | 同事务 `mark_delivered`（delivery → delivered 与 shield 释放/R-4 恢复四写并列） |
| `ack_command` continue-failed | 命令 failed | `mark_failed`；重投（claim 重领）→ `mark_delivering`，attempt_count+1（ACP-446 链路留痕） |
| `pm_report_consumption` | delivery_id 可选 | **改必填**（V1.2 §1.5 挂账兑现）；`mark_consumed` + Interaction consumed_at 双写不变（ACP-C2：仅观测） |
| §5.3 resolution_note | 仅引 interaction_id | 补引 delivery_id（F5 事务内取） |
| `comm_overview::interaction_unconsumed_total` | Interaction 推导 | 改读 delivery 表（`status IN ('delivering','delivered')` 计数）——N-6 稳态指标口径落表 |

## 3. ACP-05：Continuation 正式身份与状态机（迁移 0051）

### 3.1 表

```sql
CREATE TABLE delivery.continuation (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL REFERENCES core.project(id),
    task_id               UUID NOT NULL REFERENCES delivery.task(id),
    run_id                UUID NOT NULL REFERENCES delivery.task_run(id),
    origin_interaction_id UUID NOT NULL REFERENCES delivery.task_interaction(id),
    generation            INT  NOT NULL,        -- 同 Run 内第 N 次继续（基线 §3.3；与 lease_generation 无关）
    status                TEXT NOT NULL DEFAULT 'open'
                          CHECK (status IN ('open','started','consumed','closed','frozen')),
    frozen_at             TIMESTAMPTZ,          -- terminate 重试耗尽镜像（原 task_interaction 列保留）
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at            TIMESTAMPTZ,
    closed_at             TIMESTAMPTZ,
    UNIQUE (project_id, id),
    UNIQUE (run_id, generation)
);
```

- **身份语义**：`delivery.continuation.id` 即全系统 Continuation 正式身份——0050 的 `continuation_id` 回填目标；`interaction.session_id`（R-8 派生自 executor_session）与 receipt.session_id（新 Session 回执）**不变**，三者各司其职（⑤）。
- **代次规则**：同 Run 每次介入的 continue → `generation = max+1`；B3-2 新 attempt Run 是 **Run 维度**事件（`attempt_no`），continuation 不跨 Run 继承代次——`UNIQUE(run_id, generation)` 自然隔离。

### 3.2 状态机（生命周期最小集；06/07 职责不吸收——经接口接入）

```text
            create（submit_interaction_response 事务，generation=max+1）
   ──▶ open ──ack continue（F5 同事务）──▶ started ──消费回执──▶ consumed
             │                                │
             │ terminate 重试耗尽             │ Run 终态 / Task 完成 / 取消（终态钩子）
             ▼                                ▼
          frozen ◄────────（frozen 解除 = 显式人工处置/ACP-07）────────┘
                                                  任意非终态 ──▶ closed
```

| 迁移 | 触发 | 与既有事实的关系 |
| --- | --- | --- |
| open → started | continue **acked**（INPUT_INJECTED） | **与 R-4 恢复同事务**（F5），只读 delivery.status=delivered 作为证据 |
| started/consumed → consumed | 消费回执达 | 镜像 Interaction consumed（ACP-C2：不承担执行职责，纯生命周期记账） |
| → frozen | terminate 重试耗尽 | `task_interaction.continuation_start_frozen_at` 语义镜像（原列保留双写过渡，收口批归一） |
| 非终态 → closed | Run 终态 / Task 完成 / Task 取消 | 终态钩子（`finish_run`/任务终态路径）；closed 为终态 |
| frozen → started | 显式人工处置（重试/ACP-07 授权） | 情形 6 解除路径；不经自动恢复 |

- **守卫纪律**：全部迁移经统一应用服务入口（R1/V-6）；`consumed` 迁移**不得**反向影响 Task/guard（C2 回归钉）。

### 3.3 服务接口（I-2）

```text
open_continuation(tx, run, interaction) -> (id, generation)   -- submit_response 事务（0051 后三写变四写）
mark_started(tx, run_id, interaction_id)                      -- F5 acked 事务
mark_consumed(tx, delivery_id)                                -- 消费回执（读 delivery 行定位）
freeze(tx, interaction_id) / unfreeze(tx, interaction_id)     -- terminate 耗尽 / 人工处置
close_by_run(tx, run_id)                                      -- 终态钩子（finish_run 接线）
get_open_by_run(run_id) -> Option<Continuation>               -- ④ 只读面；06（Checkpoint/Lock）/07（Deadline）经此接入
```

## 4. 实施顺序与迁移号

| 批 | 内容 | 迁移 | 前置 |
|---|---|---|---|
| 本契约 | 五问裁决（✅ 2026-11-07 通过）+ 字段/接口冻结 | — | — |
| **ACP-04** | 0050 表 + I-1 服务 + I-3 六点接线 | **0050**（裁决确认；落地前重核台账） | 无（纯增量，双工具契约不动） |
| **Recovery Foundation**（裁决插入：先于 0051） | 续领/发现/命令改路由 + ACP 收敛列（tentative 0052） | 0052 tentative | 0049 身份安全 + 0050 |
| **ACP-05** | 0051 表 + I-2 服务 + continuation_id 回填 + 0050 delivery 列填充 | **0051**（暂定确认） | ACP-04 ∪ **Recovery Foundation**（裁决理由：ack 推进与 R-4 恢复同事务，恢复设施未建则语义倒挂） |

- 单批纵切纪律：每批独立审查/测试/回滚（专家第 3 块收敛批规格沿用）。
- 0448/0049 之外的挂账清扫：`interaction_response.continue_command_id` 与 delivery 表 `continue_command_id` 并行双写一个过渡期，收口批删列或留只读（落地时定）。

## 5. 验收标准（新增 ACP-470~477）

| 编号 | 场景 | 必须保证 |
| --- | --- | --- |
| ACP-470 | 回复持久化 | 同事务三写：response + command + delivery（一回复一账；UNIQUE(response_id)） |
| ACP-471 | continue 失败重投 | 复用同一 delivery_id：status 回置 + attempt_count+1；不新建行（§1.4 不变量落表） |
| ACP-472 | 消费回执 | delivery_id 必填（缺失 → 明确拒绝）；consumed_at 落值 + Interaction consumed 双写；重复提交幂等返回原结果 |
| ACP-473 | 代次隔离 | delivery.continuation_generation 恒 1（R-8）；`pm_report_consumption` 签名与 0047 时代逐字节一致（契约红线回归） |
| ACP-474 | 两次介入 | 同 Run 两链 → 两个 continuation（generation 1/2，UNIQUE(run_id,generation)）；衔接 ACP-451 链独立断言 |
| ACP-475 | acked 同事务 | delivery→delivered ∪ continuation→started ∪ R-4 恢复四写同一事务；缺一即回滚 |
| ACP-476 | frozen | terminate 耗尽 → frozen + 原列双写；显式处置 → started；自动恢复路径不存在 |
| ACP-477 | 终态钩子 | Run 终态/Task 取消 → open/started continuation → closed；closed 后不可迁移 |
| E2E | DSH 无证据 | delivery 停 `delivered` + unconsumed 计数 + continuation 停 `started`（衔接 ACP-450；执行不受阻） |

## 6. 工程纪律

1. **不修改 MCP 工具契约**（除 delivery_id 必填化属 V1.2 §1.5 既有裁决的兑现，参数集不变）。
2. 禁重建区：不建 Session 身份表（⑤ 三件定案）、不改 13 态、delivery/continuation 为投递与生命周期记账域，**不承担执行授权**（执行面仍租约三要素 + execution_allows）。
3. 全部写入经 pm-application 统一服务（V-6）；审计四身份列全写（C174/C180 同步登记）。
4. Outbox 四事件进目录前重核 CATALOG（C10）；事件仅通知不承载状态（状态权威在表）。
5. 迁移 0050/0051 落地前重核台账；与 0049（Client Registry 预留）无列交集，可任意先后。
6. R-4/C2 回归钉：ACP-05 的 consumed 迁移不得触碰 Task/guard/执行面——acp_comm_face 既有 20 例 + 本稿 8 例为回归门槛。

---

**文档结束**
