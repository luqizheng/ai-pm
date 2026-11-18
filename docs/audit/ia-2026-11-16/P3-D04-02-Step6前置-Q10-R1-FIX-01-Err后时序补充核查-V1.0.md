# P3-D04-02 Step 6 前置 · Q10-R1-FIX-01 — Err 后时序补充核查 V1.0

- 编制日期：2026-11-18
- 性质：**只读补充核查**（审阅方 V1.1 审查意见 §4 三项要求；零代码/测试/Schema 修改；不选择 A/B'）
- 前置：**撤回 V1.1 §1.2「不可达」结论**——审阅方反例（回滚型 Err 释放 advisory 后的竞争）成立，本核查 §二证实可达

---

## 一、核查 1：Err 来源全量核查（B3 短路之后，CODE FACT 行号=pc/publish.rs）

| # | 位置 | Err 类型 | 事务归宿 | advisory 释放时机 |
|---|---|---|---|---|
| E1 | :223 `NotFound`（提案行缺） | 业务 | **回滚**（sqlx Tx drop=rollback） | 回滚即释放 |
| E2 | :227 守卫 Conflict（提案非 approved） | 业务 | **回滚** | 同上 |
| E3 | :243 审批 pending 行缺 Conflict | 业务 | **回滚** | 同上 |
| E4/E5 | :266/:270 Invalid（成员集） | 业务 | **回滚** | 同上 |
| E6/E7 | :285/:290 锚点三方比对 Conflict | 业务 | **回滚** | 同上 |
| **E8** | **:344-345 CAS 失配**（UPDATE 双 failed → **`tx.commit()`** → Err(Conflict)） | 业务 | **提交**（failed+failed 落库） | commit 即释放 |
| E9 | :373 L3 冻结 Forbidden | 业务 | **回滚** | 回滚即释放 |
| E10/E11 | :472/:563 Conflict（版本单向/前驱） | 业务 | **回滚** | 同上 |
| E12 | :468 Invalid（空成员） | 业务 | **回滚** | 同上 |
| E13 | B3 后任意 `?` 传播的 **Db 错**（INSERT version/members/审计/outbox、commit 本身） | 技术 | **回滚**（commit 错除外：结果不定，极少） | 回滚即释放 |
| —— | :721 `tx.commit()` 成功 | —— | 提交（applied+completed） | commit 释放 |

**分类结论**：提交型仅 E8（CAS——JOIN 将见 `(failed, failed)`→failed 分类正确，无窗口）；**其余全部回滚型=审阅方时序 2 的窗口 opener**（Err 返回时 advisory 已释放、库内提案仍 approved、账本无行/回滚消失）。

## 二、核查 2：Err→JOIN→竞争提交时序（审阅方表 3 第二种）

**可达性判定：成立（撤回 V1.1 §1.2）**。合法时序：close publish 遇 E1-E13 任一回滚型 Err→JOIN 读 `(approved, None)`→计 failed→竞争者（此前无锁冲突）获 advisory 发布成功→库终态 `(applied, completed)` 而报告计 failed。V1.1 的守卫型论证（"applied∧无 completed 行不可达"）**与本时序正交**——审阅方指正完全正确。

**但该窗口是否构成"误计"，取决于 CloseOutcome 的语义契约（本核查的核心新发现）**：

| 契约 | 定义 | 时序 2 判定 | 现状评分 |
|---|---|---|---|
| **契约 I=动作语义**（现状实现语义；CloseOutcome 现注释"批量发布的数量/失败…的数量"） | 计数描述**本次 close 自身动作**的成败：close 的发布尝试真实失败（E1-E13）→failed=1 **如实**；竞争者随后的成功是独立事件，不改变 close 自身成败 | **非误计** | 服务层全部可达路径（守卫型不可达+CAS 提交型+回滚型）下**零误计** |
| **契约 II=终态语义** | 计数须与报告 closed 时的库内终态一致（failed 意味着"该提案未因此次 close 发布完成"） | 时序 2=**误计**（库已发布而计 failed） | 存在偏差窗口（E1-E13 回滚型+后继竞争） |

**约束内技术解（仅契约 II 需要）**：Err 分支的分类读放入 `BEGIN; SELECT pg_advisory_xact_lock('pc_publish:'||project); <单条 JOIN>; COMMIT` 的**只读小事务**——
- 使用**既有 advisory 键**（非新增锁机制；该键协议=D5.1 既有登记）；
- 独立新事务（**不改变任何既有事务的边界**——close 外层/publish 子事务原样）；
- 只读（状态机零触碰；隔离级别不变）；
- **正确性证明**：持锁期间所有发布者（同键）无法提交 → JOIN 观察的是**锁内终局**——`(applied,completed)`→replay；`(approved,无行/failed行)`→锁内无人能改变（要变必经同 advisory）→failed 分类对"close 时点"终局正确；锁释放后的竞争=close 之后的世界演变，任何时点计数均无法也不应追认。
- 该解同时覆盖审阅方表 3 三种时序（时序 1 在锁内读 `(applied,completed)`→replay；时序 3 同理）。

**不采用的替代**（审阅方已排除）：二次无锁重读（同窗）、仅 tracing::warn（不解决契约 II 偏差，仅改善可观测）。

## 三、核查 3：现有测试覆盖

`q10_race_closure.rs` 覆盖=时序 1 变体（竞争者先提交→close 短路 Ok）与无失败基线；**未覆盖时序 2**（close 发布失败→JOIN 旧态→竞争者后发）。gap 成立。

**时序 2 测试设计（本版仅设计，不实施）**：
```
① seed：approved 提案+未解决 pc.conflict（圈定其目标对象）
② 并发：close_report ∥ [resolve_conflict → 显式 publish_version]
③ 预期交错（确定性可由 barrier 控制，或依赖冲突检查先行）：
   close 的 publish 在 :373（E9）Forbidden 回滚→计 failed
   → 竞争腿 resolve+发布成功→库终态 (applied, completed)
④ 断言（按契约分别给出）：
   契约 I：failed=1 ✓（close 自身失败如实）+版本恰 1+提案 applied
   契约 II（若采纳锁内分类）：replayed=1/failed=0
```
该测试对两个契约均可复用（断言面切换），并实测锁定时序 2 的真实发生。

## 四、提交负责人（更新后的处置选项；不自行选择）

| 选项 | 内容 | 前提 |
|---|---|---|
| **A=取消修复（契约 I）** | 零代码；**正式文档化 CloseOutcome=动作语义契约**；归档本核查+V1.1 勘误；补 §三 时序 2 测试（断言 failed=1=真失败）；GAP-32 按「原风险+守卫型不可达+回滚型=真失败」申请关闭 | 接受计数描述 close 自身动作 |
| **B''=锁内分类（契约 II）** | V1.0 方案 D1-D4 + 分类读升级为**锁内只读事务**（§二技术解：既有键/零新机制/零边界变更）；replayed 字段保留；§三 测试断言 replayed=1 | 接受"close 时点终态"语义+确认既有键的新只读用法不违约束 |
| （B' 已被本核查取代——V1.0 的无锁 JOIN 分类在契约 II 下不完整，在契约 I 下多余） | | |

**技术依据补齐情况**：审阅方 §5 所列三核查（Err 全量/时序 2 可达性/测试覆盖）已全部完成；A 与 B'' 均具备完整技术依据，差异纯为**语义契约选择**——属负责人裁决面。

## 五、零实施声明

本核查只读：未修改代码/测试/Schema/迁移/触发器；未选择 A/B''；GAP-32 维持 OPEN；撤回 V1.1 §1.2 结论已正式登记（§〇）。
