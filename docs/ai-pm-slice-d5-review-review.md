# D5 · Review 设计稿评审记录（T-042 · V1 → V1.1）

> 评审对象：`docs/ai-pm-slice-d5-review-v1.md`（commit `faa3f34`，2026-11-02）
> 评审结论：**有条件过稿**——P0 六项过稿前必须确认（已全部收编），P1 四项实施前必须落实（已全部收编）；收编后设计稿升级 **V1.1**，正文以「〔评审收编 P0-x/P1-x〕」标注。
> 收编日期：2026-11-02

---

## 一、评审原文（照录）

**P0（过稿前必须确认）**

1. Review Run 独立活跃唯一约束，以及 executor_kind=platform 与 Codex Executor 的隔离。
2. Review 必须绑定不可变的 Delivery Manifest Hash 和 Review Policy Version。
3. 审批必须绑定具体 Review、Delivery 和交付快照，并通过原子事务完成。
4. 失败判定服务必须按 run_kind 隔离，Review 失败不能触发 Delivery 重跑。
5. DeliverySubmitted 重复消费、Worker 崩溃及 Review 重试必须具备完整幂等性。
6. Task completed 与 Knowledge Proposal draft 创建的事务边界必须明确。

**P1（实施前必须落实）**

1. 迁移编号 0024 与知识索引 V2 预留编号冲突，统一迁移台账。
2. 新增表的租户隔离、复合外键和历史不可变约束。
3. 补充多 Worker 并发扫描、重复审批、重复 Review 创建测试。
4. 明确四种 Review 结论与 Task 状态转换的完整映射表。

---

## 二、逐项核对表（原稿现状 → 缺口 → 收编方案）

| # | 评审要求 | 原稿现状 | 缺口判定 | 收编方案（V1.1 落点） |
|---|---|---|---|---|
| **P0-1** | Review Run 独立活跃唯一约束；executor_kind=platform 与 Codex Executor 隔离 | `uq_task_review_active` 已是独立活跃唯一约束（§2.1）；`executor_kind='platform'` 仅是**列约定**（§2.1 注释），无库层约束 | 隔离是应用层约定而非库层不变量：review 行被误写凭据列、delivery 行被误占 platform 保留名均无拦截 | §2.1 补两个 CHECK：`chk_review_platform_isolation`（review 行 ⇒ `executor_kind='platform'` 且 `executor_session`/`lease_token_hash`/`deadline_at`/`last_agent_heartbeat_at`/`stage` 全 NULL 且 `lease_generation=0`——列名实测：0011/0015/0023/0010:23）+ `chk_delivery_executor_kind`（delivery 行 `executor_kind <> 'platform'`，保留名独占） |
| **P0-2** | Review 绑定不可变的 Delivery Manifest Hash 与 Review Policy Version | `reviewed_manifest_hash` 已锚定（§3.2）但不可变无机制；**Policy Version 完全缺失**（Q-D5-2 把角色版本管理延后，审查依据版本不可追溯） | 两处：不可变无载体；策略版本不可追溯 | §2.1 补列 `review_policy_version TEXT`（review 行语义必填，随 `chk_review_role_present` 同款 CHECK）+ §3.2 口径：**内容寻址**——§4 检查项清单（L1–L4 声明 canonical JSON）sha256 前 12 位，对齐 0016 `policy_version` 惯例；once-written 机制并入 P1-2 触发器 |
| **P0-3** | 审批绑定具体 Review、Delivery、交付快照 + 原子事务 | `review_approval` 锚定 `review_run_id`+`task_id`（§2.4），**缺 delivery run 锚定与交付快照**；审批事务边界未成文（§5.1「人工审批两出口」未写原子性） | 审批对象欠明确（批的是哪次交付？）；裁决与状态迁移的原子性无定义 | §2.4 补列 `delivery_run_id UUID NOT NULL` + `delivery_manifest_hash TEXT NOT NULL`（审批时点交付快照 = 被审 delivery run 的 `reviewed_manifest_hash`）+ 组合外键；§5.2 补审批裁决单事务：UPDATE 审批行 + 锚定回读校验（漂移 → 409 `anchor_drift`，审批行保持 pending）+ E2 全集 + 状态迁移 + `task_waiting` 结行 + 审计 + 事件，同事务 |
| **P0-4** | 失败判定服务按 run_kind 隔离；Review 失败不触发 Delivery 重跑 | §6.1 行 8 排除 review run、§5.3 不变量 1 已立——但均为**语义排除**，扫描查询未强制谓词 | 结构性缺失：实现者可能在扫描 SQL 中遗漏 run_kind 过滤 | §6.2 补：失败判定候选扫描 SQL **强制 `run_kind='delivery'` 谓词**（结构性排除）；review run 的失败处置（重试/超时回收/降级）全部归 review 领取拍自理；反例推演入文：若扫描不隔离，review run `failed` 会被判 B4-A → 触发 delivery run 重跑 → 违反 C1 验收锚点 |
| **P0-5** | DeliverySubmitted 重复消费、Worker 崩溃、Review 重试的完整幂等 | 仅 outbox `idempotency_key`（事件层）；拍① handler 重入、拍② worker 崩溃、重试并发三场景**均无机制** | 幂等矩阵不完整 | §3.4 新增幂等矩阵五场景（重复消费重入判据 / handler 崩溃重投 / 拍② running 卡死超时回收 / 重试并发条件 UPDATE / 管线重复执行跳过），机制全部落到库层索引或条件 UPDATE |
| **P0-6** | Task completed 与 Knowledge Proposal draft 创建的事务边界 | §7 只写「completed 同事务写事件」；draft 提案创建的归属与失败语义未定义 | 事务边界模糊：提案创建失败是否回滚 completed 未定义 | §7 补事务边界表：**同事务原子集**（task 迁移 + waiting 结行 + 审计 + 两事件）/**异步集**（消费 handler 建 draft 提案，独立事务）/**失败语义**（completed 不回滚；handler 失败重试 → 死信 → T-061 死信重放可补；缺窗期最终一致——提案非 completed 的裁决依据） |
| **P1-1** | 迁移编号冲突，统一台账 | **实测三方撞 0024**：T-083（`todo.md:261` 预留 `0024_discovery`）、T-060（`docs/ai-pm-t060-knowledge-index-v1.md` §4 明确「本迁移取 0024」，随 `0b26e6b` 提交）、D5 原稿 §2 | 撞号属实；且项目无统一台账（历次撞号靠逐文档人工对账） | D5 迁移 **0024 → 0026**；新建统一台账 `docs/ai-pm-migration-ledger.md`（号段 × 任务 × 状态 × 文件名）；T-060/T-083 之间的撞号移交各自任务实施时顺移（本稿不越权改他稿） |
| **P1-2** | 新增表租户隔离、复合外键、历史不可变约束 | 租户隔离（`tenant_id` + `UNIQUE(tenant_id,id)` + 组合外键 + tenant_schema 收编）三表均已具备（§2.2–2.4）；**历史不可变无库层约束**（应用层无 UPDATE 路径而已） | 不可变靠纪律不靠约束 | §2.5 补迁移内 BEFORE UPDATE 触发器：三新表**全表禁 UPDATE**；`task_run` 的 `run_kind='review'` 行受保护列（`role_id`/`input_manifest_hash`/`reviewed_manifest_hash`/`review_policy_version` 创建后、`review_verdict` 非 NULL 后）禁改；delivery 行与既有 D3 更新路径零影响（run_kind 过滤 + 列级 OLD/NEW 判定） |
| **P1-3** | 多 Worker 并发扫描、重复审批、重复 Review 创建测试 | C115（并发判定）/C116（重复审批）/C99（第二个活跃 review run）已有；缺并发**消费**、并发**创建**、并发**重试** | 并发用例覆盖不全 | §10 补 C120–C123：双 worker 并发消费 `DeliverySubmitted`（幂等跳过，单 run 单审计）/ 并发创建同锚定 review run（唯一索引单胜）/ 并发重试 failed review run（条件 UPDATE 先到先得单重试）/ 受保护列 UPDATE 被触发器拒绝 |
| **P1-4** | 四种 Review 结论与 Task 状态转换的完整映射表 | §5.1 已有 7 行映射，但以「场景」为行，四结论 × 策略分支的**完备性不可机械验证** | 映射不完备（无策略分支维度） | §5.1 重构为完备矩阵：行 = 四结论（approved / changes_requested / inconclusive / 无 verdict[failed|timed_out]），列 = 策略分支（auto 放行 / 需人工 / 硬禁令命中），单元格 = task 动作 + 副作用，全部 12 格无空缺 |

---

## 三、收编结论

- **P0×6 全部收编**：P0-1/P0-2/P0-3 落 §2 DDL（三列 + 四 CHECK + 两列审批锚定），P0-4 落 §6.2 扫描谓词，P0-5 落 §3.4 幂等矩阵，P0-6 落 §7 事务边界表。
- **P1×4 全部落实**：P1-1 迁移号改 0026 + 台账落盘，P1-2 触发器入迁移，P1-3 验收矩阵扩至 C123，P1-4 §5.1 矩阵化。
- **七项 Q-D5 裁定无一推翻**——本次收编全部为「强化既有裁定」与「补齐缺口」，架构不变。
- **移交项**：T-060（已声明 0024）/ T-083（todo:261 声明 0024）之间的撞号，各自实施时按台账顺移；D5 不改他稿。

## 四、验收矩阵增量

原 C98–C119 保留，新增：C120（并发消费幂等）/ C121（并发创建唯一索引）/ C122（并发重试先到先得）/ C123（触发器拒改受保护列）。触发器相关断言并入 C98（迁移断言扩：四 CHECK + 两触发器存在性）。
---

## 五、第四轮补充：验收场景清单收编（评审「四、建议增加的验收场景」）

评审补充 10 个验收场景，逐一对账现有 C98–C123：3 个由既有用例覆盖（③→C111、⑥→C103、⑦→C115），7 个补新编号 **C124–C130**：

| 场景 | 编号 | 断言要点 |
|---|---|---|
| ① 同一 `DeliverySubmitted` 消费两次 → 单活跃 Review | **C124**（串行版；并发版 C120 既有） | 第二次幂等返回，无第二 run/重复审计/重复迁移 |
| ② Review Worker 执行中崩溃 → 恢复且不创建 Delivery Run | **C127** | running 卡死 → 拍②回收 `timed_out` → 重试 → 推进；全程无新 delivery run |
| ③ Review Run failed → 原 Delivery Run 保持 `succeeded` | C111（既有） | delivery run status/outcome 全程不变 |
| ④ 两审批人并发审批 → 唯一有效结果 | **C125** | FOR UPDATE 行锁单胜，败者 409 `already_decided`，状态/事件只落一份 |
| ⑤ 审批后 Delivery 变化 → 旧审批不可用于新交付 | **C126** | 锚定回读 409 `anchor_drift` 保持 pending；测试构造库态直测（V1 为理论防御，随 L2 激活） |
| ⑥ `integration_status=failed` → 人/机均不能 completed | C103（既有） | §5.2 步骤 3 拒绝 approve |
| ⑦ Worker 多实例扫描 → 失败判定不重复 | C115（既有） | 条件 UPDATE 只判一次 |
| ⑧ Task completed 后崩溃 → 提案不丢失 | **C128** | outbox 重投补建 → 死信重放可补；completed 不回滚 |
| ⑨ 历史 Review 重新执行 → 旧报告完整保留 | **C129** | 两轮 review，第一轮 findings/裁决/verdict 全量可查，第二轮新行 |
| ⑩ 全只读仓交付 → 正常进入 Review | **C130** | D4 V1.1 Q1 delivered(0/0) → B1 守卫过 → 管线全过 → approved |

**附带一致性修正**：设计稿全文 `apply_task_transition`（9 处）→ `update_task_status` + `validate_transition`——D4 V1.1 已裁定旧名称废弃且代码中不存在，D5 稿沿用属笔误级偏差，本轮一并清零。实施顺序用例范围更新为 **C98–C130**（设计稿 §11）。
---

## 五、最终架构判断（2026-11-02）

**D5：有条件通过**

不建议推翻七项裁定，也不建议增加新的状态体系。

当前设计最值得保留的是：
1. 统一 Run 历史。
2. 确定性 Reviewer。
3. 独立审批记录。
4. Review 失败与执行失败隔离。
5. 知识提案与正式知识分离。

需要修订的主要是并发、幂等、审批绑定、失败恢复和事务边界。

**架构原则**：D5 应该是一个治理闭环，而不是一个更复杂的执行引擎。
- D1 决定能不能执行。
- D3 负责实际执行。
- D4 记录执行产生的事实。
- D5 判断这些事实是否满足任务完成条件。
- 知识库负责将经过治理的事实转化为长期知识。

**不要让 D5 重新承担 D1、D3 或 D4 的职责。**

**暂缓 T-043**：先让实施方根据 P0/P1 清单修订设计稿，并提交：
1. 七项裁定表的修订版 → D1 完成（§1 增架构原则列）
2. Review / Delivery 状态转换矩阵 → D2 完成（§3.5）
3. 四值结论映射表 → D2 完成（§3.5 矩阵）
4. 审批事务流程 → D4 完成（§5.2.1 + §5.2.2）
5. 失败恢复流程图 → D5 完成（§6.3）
6. C98–C119 验收矩阵的补充映射 → D6 完成（§10，终态 C98–C136）

**验收矩阵终态**：C98–C136，共 39 条。

---


## 六、D1–D6 交付物清单（2026-11-02 修订完成）

| # | 交付物 | 落点 | 状态 |
|---|---|---|---|
| D1 | 架构原则 + 七项裁定表修订 | §0 + §1 | ✅ 已提交 |
| D2 | Review/Delivery 状态转换矩阵 + 幂等与并发规则表 | §3.5 + §3.6 | ✅ 已提交 |
| D3 | 四值结论映射表（含策略分支） | §3.5（主干矩阵） | ✅ 已提交 |
| D4 | 审批事务流程图 + 审批继承禁令 | §5.2.1 + §5.2.2 | ✅ 已提交 |
| D5 | 失败恢复流程图 | §6.3 | ✅ 已提交 |
| D6 | 验收矩阵终态确认 | §10（C98–C136） | ✅ 已提交 |

**设计稿版本**：V1.2（待过稿）。T-043 暂缓，在 D1–D6 全部过稿前不进入实施。
---

## 七、最终判断补充（2026-11-02，第二则消息）

评审补充两项（第 5 项「Knowledge Proposal 的事务边界（P1）明确：」正文在传输中截断，待补全）。

### 7.1 补充 4 强化：失败恢复**不能重新创建 Delivery Run**（已收编）

| 触发失败的对象 | 恢复动作 | 新建 Run 的 `run_kind` | 是否创建 delivery run |
|---|---|---|---|
| review run（`failed`/`timed_out`） | 重试 review | `'review'`（`attempt_no+1`） | **❌ 绝不创建** |
| delivery run（B4-A/B） | 重试交付 | `'delivery'`（`attempt_no+1`） | ✅ 合法（属 D1/D3 执行重试，非 review 恢复） |

收编落点：§6.1 恢复链 `run_kind` 恒定性硬约束表 + §6.3 流程图显式标注「✗ 绝不创建 Delivery Run」+ 关键不变量第 2 条 + §5.4 硬不变量第 4 条 + §10 **C136**（`delivery.task_run` 表零 `run_kind='delivery'` INSERT 的 DB 层断言）。

### 7.2 补充 5 已收编：Knowledge Proposal 的事务边界（P1）

评审补全的原文（流程图）：

```
Task completed
       |
       v
创建 Knowledge Proposal
       |
       v
status = draft
```

收编落点：设计稿 **§7.1.1**——流程图 + 三节点语义表（`Task completed` = 既成事实因、不可回滚，属 §7.1 同事务原子集；`创建 Knowledge Proposal` = 因果动作果、异步执行、幂等键 `knowledge_writeback:{review_run_id}`；`status = draft` = 初始且唯一落库状态）。同时固化因果方向不可逆：completed 为因、提案为果，果失败不回滚因（重试/死信重放补建），反向不成立。

§7.1 事务边界表（已存在，本次确认）：

| 集合 | 动作 | 事务归属 | 失败语义 |
|---|---|---|---|
| 同事务原子集 | `reviewing→completed` + `task_waiting` 结行 + 审计 + `ReviewCompleted` 事件 + `KnowledgeBaseWriteRequested` 事件 | 单事务 | 任一失败全回滚，completed 不落 |
| 异步集 | 消费 `KnowledgeBaseWriteRequested` → 查重 + INSERT `draft` 提案 → outbox 标 delivered | handler 独立事务 | 重试 → 死信 → 重放补建；completed 不回滚 |

**候选点 a/c/d/e 的处置**（评审未逐条答复，按现有设计固化，如有异议请指出）：
- a) 幂等：事件级 `knowledge_writeback:{review_run_id}`（outbox UNIQUE 兜底）+ 应用层 `(project_id, doc_key, 活跃状态集)` 查重——**维持**。
- c) 死信重放：沿用 T-061 死信重放机制——**维持**。
- d) 可见性：缺窗期提案暂缺，属可接受最终一致——**维持**（T-070 页面按提案存在性展示）。
- e) `doc_key` 命名空间：按 `(tenant_id, project_id, doc_key)` 隔离——**维持**。
---

## 八、第三则：同事务建议 + 迁移核实 + 四要素 + 明确不做（2026-11-02）

### 8.1 Knowledge Proposal 与 Task completed 同事务（收编，**推翻 §7.2 的 a)/d) 异步处置**）

评审建议：**Knowledge Proposal 应与 Task completed 在同一个数据库事务中完成，避免任务已经完成却没有生成知识提案。**

以本建议为准，设计稿修订（v1.3）：

| 落点 | 修订 |
|---|---|
| §7.1 | 事务边界表：`INSERT ai.document_proposal(draft)` 移入**同事务原子集**；原「异步集」**废止** |
| §7.1.1 | 流程图重绘为「单数据库事务」框；三节点**原子绑定**（同生同灭），失败全回滚 |
| §7.1.2 | 新增：`KnowledgeBaseWriteRequested` 语义由「异步创建请求」变更为「提案已创建**通知**」，发现 V1 不注册消费 handler |
| §7.2 | 触发行改为「completed 同事务内既 INSERT draft 又写通知事件」；去重行改为「事务级条件 UPDATE + 事务内查重」 |
| §9 事件清单 | `KnowledgeBaseWriteRequested` 消费者列改为「V1 不注册 handler」 |
| §10 验收 | C117 改同事务版、C118 改事务内查重、C128 改「事务崩溃全回滚」、新增 **C137**（跨 schema 同事务断言：提案 INSERT 失败 → completed 回滚） |

**对评审记录 §7.2 的影响**：候选点 a)（事件级幂等）与 d)（缺窗期最终一致）的「维持」处置被本建议取代——幂等改由 completed 事务的条件 UPDATE 保证；「completed 但无提案」中间态在库层不可能发生。

### 8.2 迁移号核实

**结论：0024 已被知识索引 V2（T-060）占用**——`docs/ai-pm-t060-knowledge-index-v1.md` §4 明确「本迁移取 0024」，并多处引用 `0024_knowledge_section_index.sql`；T-083（discovery）原亦预留 0024（`todo.md:261`）。实测 `migrations/` 最大号 = **0023**（0024 尚未落文件）。**D5 已于 P1-1 收编改为 0026**（台账 `docs/ai-pm-migration-ledger.md` 登记为「T-043 · 0026」），无需再调整。

### 8.3 验收矩阵四要素化

评审要求：C98–C119 不要只列测试编号，要有**测试名称、前置条件、执行动作、预期结果**。设计稿 §10 全矩阵（C98–**C137**，共 40 条）已重构为五列表：`# | 测试名称 | 前置条件 | 执行动作 | 预期结果`。

### 8.4 明确不做列示（评审「不建议增加」七项）

设计稿 **§12.1** 新增「明确不做」清单表，逐条列示并挂接裁定依据：

| # | 不做项 | 依据 |
|---|---|---|
| 1 | V1 引入 LLM Reviewer | Q-D5-2 |
| 2 | 新增 Review 专属状态机 | Q-D5-4 |
| 3 | 新增独立 Review Report 主表 | Q-D5-1 |
| 4 | 引入新的 Review 调度服务 | Q-D5-5 |
| 5 | V1 引入 `task_deadline` | Q-D5-5 |
| 6 | 自动把 AI 执行结果写入正式知识库 | Q-D5-7 + §0.2 不变量 4 |
| 7 | 为 Review 失败重新执行 Codex | §5.4 不变量 4 + §6.3 |

**验收矩阵终态**：C98–C137，共 40 条。设计稿版本 **V1.3（待过稿）**。