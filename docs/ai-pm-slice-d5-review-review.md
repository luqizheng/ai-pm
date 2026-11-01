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