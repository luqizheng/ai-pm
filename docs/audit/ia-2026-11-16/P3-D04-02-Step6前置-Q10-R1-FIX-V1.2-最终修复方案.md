# P3-D04-02 Step 6 前置 · Q10-R1-FIX — V1.2 最终修复方案（B''/II-A）

- 编制日期：2026-11-18
- 性质：**修复方案（待审批；零实施）**——Q10-R1-FIX-03 指令七项交付合编本文件
- 决策基线：负责人九条（§八 逐条映射）；技术依据链=前置分析（f76829b）→独立复核（05ae6c3）→R3 核查（cc07041/6009f10）→V1.1 停止报告（276095c，结论已撤回）→Err 后核查（d582c6e）→FIX-02（4a5f491）

---

## 一、修复目标与实现约束（继承 V1.0 D1-D4，不扩大）

1. **目标**：close 批量发布 Err 分支的计数分类修正——区分「本次 close 成功发布」「分类时点已确认先行发布完成」「本次尝试失败」；
2. **约束（V1.0 七契约全部继承）**：单条 JOIN（禁两条 SELECT）/fetch_optional/仅 `applied∧completed`→replay/不凭账本单判成功/两域+前端 TS/R3 快照测试保留运行/五零不变式（零新锁机制〔按基线 6 释义：不新增键〕、零隔离变更、零既有事务边界变更、零状态机变更、零 Schema）；
3. **修改面与 V1.0 D1-D4 相同的四件**（§六），唯一升级=Err 分支分类读由无锁 JOIN 升级为**advisory 保护下的独立只读事务**（§二）。

## 二、分类读升级（B'' 核心）

### 2.1 形态（pc 段示例；projc 同构）

```rust
Err(_) => {
    // R-1 分类事务（Q10-R1-FIX V1.2）：BEGIN → 既有 project 级 advisory →
    // 单条 JOIN → COMMIT。II-A 契约（§三）。
    let mut cls = pool.begin().await?;
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtext('pc_publish:' || $1::text))::bigint",
    )
    .bind(project_id.to_string())
    .execute(&mut *cls)
    .await?;
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT p.status, wo.status FROM pc.proposal p \
          LEFT JOIN pc.write_operation wo \
            ON wo.tenant_id = p.tenant_id AND wo.project_id = p.project_id \
           AND wo.operation_id = 'pc-decision:' || p.id::text \
         WHERE p.id = $1",
    )
    .bind(proposal_id)
    .fetch_optional(&mut *cls)
    .await;
    cls.commit().await?;   // 只读事务提交=释放 advisory
    match row {
        Ok(Some((p, w))) if p == "applied" && w.as_deref() == Some("completed") => {
            replayed_proposals += 1;
        }
        _ => publish_failed_proposals += 1,  // 含 Ok(None)/Ok(不满足)/Err(Db)——§2.4
    }
}
```

### 2.2 六项必须说明

| # | 要求 | 说明 |
|---|---|---|
| ① | advisory 键与现有 publish 协议一致 | 键文本 `'pc_publish:' \|\| project_id::text` **逐字同** publish.rs:117（projc=`'projc_publish:'` 同 :118）；获取语句仅空格差（`))::bigint` vs `)::bigint`——**实现须逐字复制 :117 原句**，本示例的括号差在实施时消除 |
| ② | 锁获取与 JOIN 的具体代码位置 | analysis_report.rs pc 段 Err 分支（现 :598-604 处替换）与 projc 段（现 :662-671 处替换）；顺序=BEGIN→advisory→JOIN→COMMIT |
| ③ | 只读事务不触碰状态机 | 事务内仅 SELECT（advisory 获取+JOIN）；无 INSERT/UPDATE/DELETE；advisory 本身无数据效应 |
| ④ | 原 publish 子事务不变 | publish_version 零改动（与 V1.0 相同声明；diff 范围 §六） |
| ⑤ | 原 close 外层事务不变 | 外层 tx 结构/边界/锁面零改动；分类事务=pool 新连接（与既有"publish 子事务走 pool"同款连接归属） |
| ⑥ | 每条 Err 路径进入分类事务的方式 | close 循环的 `Err(_)` 分支**统一入口**——E1-E13 全部 13 类 Err（Err 后核查 §一 表）均落入同一分支→同一分类事务；无按 Err 类型分流（分类不依赖 Err 文本，只依赖锁内终态） |

### 2.3 正确性（II-A）

持 advisory 期间：所有发布者（唯一同键持有竞争者=publish_inner）无法进入发布临界区 → JOIN 观察到的是**advisory 序列化下的分类时点终局**——`(applied, completed)` 先行发布已确认；`(approved, 无行/failed行)` 锁内无人能改变（任何改变必经同 advisory）→failed 对分类时点终局正确。**不保证报告 COMMIT 时点与库终态一致**（T1-T6 反例窗口保留，见 §三声明）。

### 2.4 分类事务自身错误的处理

`row=Err(Db)`（JOIN 执行/连接失败）与 `cls.commit()` Err：**计数 failed + `tracing::warn!(proposal_id, error)` 结构化日志**——不中止 close（外层逐提案容错语义不变）、不静默（技术故障可观测）、不凭错误推断发布状态。四分类对照（V1.1 §三）落位：未找到提案/不满足条件/JOIN 失败/连接不可用 → 前二=业务 failed；后二=技术故障→warn+failed。

## 三、CloseOutcome 契约（II-A 正式文档化）

以 doc comment 落于 CloseOutcome（D3），并引用本方案为契约源：

```
/// II-A 分类时点一致性契约（Q10-R1-FIX V1.2；负责任确认 2026-11-18）：
/// - applied/published：本次 close 成功完成发布（含短路确认）；
/// - replayed：分类时点（advisory 保护下）已确认先行发布完成；
/// - failed：分类时点未确认先行发布完成，且本次发布尝试失败
///   （含分类读自身技术故障——warn 日志可辨）。
/// 显式声明：不保证报告最终 COMMIT 时数据库终态与计数永久一致
/// （II-B 不可实现性证明=FIX-02 Q2；授权外变体不采用）。
```

## 四、锁序与事务边界证明

### 4.1 修复前后锁序图

```
修复前（仅 publish 获取 advisory）：
  close 外层 tx: R(report 行 Exclusive) ———全程———▶ COMMIT
  publish 子事务: P(advisory) → L(账本行 Exclusive) → Pr(提案行 FOR UPDATE)
                  → A(审批行 FOR UPDATE) → … → COMMIT(释放 P)

修复后（新增分类事务，同一 P 键）：
  close 外层 tx: R ———全程———▶ COMMIT          【不变】
  publish 子事务: P → L → Pr → A → … → COMMIT   【不变】
  分类事务(新):    P → (Pr⁻, L⁻ AccessShare 仅 JOIN 期间) → COMMIT(释放 P)
```

### 4.2 advisory 获取路径全集（修复后）

| # | 路径 | 位置 | 持有期 |
|---|---|---|---|
| 1 | pc publish_inner | pc/publish.rs:117 | 子事务期 |
| 2 | projc publish_inner | projc/publish.rs:118 | 子事务期 |
| 3 | **pc 分类事务（新）** | analysis_report.rs pc 段 Err 分支 | 分类事务期（毫秒级：BEGIN→advisory→JOIN→COMMIT） |
| 4 | **projc 分类事务（新）** | projc 段 Err 分支 | 同上 |

（分析起草键 `-analysis:/-identity:` 为**不同键**，不相交——不列入本图。）

### 4.3 Err 分支与分类事务完整时序

```
close 外层(R 持有)                       竞争发布者
  │ publish_version(pool 新连接)
  │   P…发布主体 Err(E1-E13) → 回滚释放 P
  │ Err 返回
  │ 分类事务: BEGIN → P(等待竞争者?) ──┬── 竞争者持 P：等其 COMMIT
  │                                    └── 获 P → JOIN(锁内终局) → COMMIT 释 P
  │ 计数(replay/failed)
  │ …withdraw 段/KB/审计…
  └─ 外层 COMMIT（报告 closed）
```

### 4.4 外层 close 与竞争发布者交错（穷举）

| # | 交错 | 分类事务行为 | 计数正确性（II-A） |
|---|---|---|---|
| I1 | 竞争者在分类事务获 P 前已 COMMIT | JOIN=(applied,completed) | replay ✓ |
| I2 | 竞争者持 P 中 | 分类等待→获 P 后见其终果 (applied,completed) | replay ✓ |
| I3 | 竞争者在分类 COMMIT 后才进入 | JOIN=(approved,无行)→failed；竞争者发布=**分类时点之后** | failed ✓（II-A 契约内：分类时点未确认先行完成） |
| I4 | 无竞争者 | 按锁内真实态 | ✓ |

### 4.5 死锁风险分析（全局，非局部）

- **锁等待图节点/边**：`P → {L, Pr, A}`（publish，Exclusive/FOR UPDATE）与 `P → {Pr⁻, L⁻}`（分类，AccessShare）——**所有边同向自 P 出发**；`R` 独立（仅 close 持有，且 close 获取 P 系资源**不依赖 R**：子事务/分类事务均在独立连接，不需 R）；
- **环检测**：构成环需某路径从 {L,Pr,A} 回到 P 或 R——账本/提案/审批行锁持有者（publish/分类/decide）**不请求 P 以外的 advisory，也不请求 R**；decide/withdraw/expire 走行锁不经 P，与 P 链无反向边；双 close 被 R 互斥先序。**全局无环**；
- **特殊校验**：外层全程持 P（禁止⑦）会造 R+P 跨连接自等（FIX-02 Q3 排除）——本方案分类事务为**短命独立事务**，外层从不持 P ✓；
- **持锁窗口**：分类事务仅 3 条语句（毫秒级），对发布者阻塞≈单次 publish 排队同量级。

### 4.6 连接归属与事务归属

| 连接 | 事务 | 归属 |
|---|---|---|
| conn-A | close 外层 tx（R） | pool |
| conn-B（每提案） | publish 子事务（P→…） | pool |
| **conn-C（Err 时）** | **分类事务（P→JOIN）** | pool（新增第三类短命用途；pool max 内，无新池） |

外层事务在任何时刻不持有 P → conn-C 获取 P 不等待 conn-A → 无跨连接同事务互等。

## 五、测试矩阵与确定性并发设计（barrier 同步，禁 sleep）

| # | 场景 | 确定性机制 | 预期时序 | 库终态 | CloseOutcome |
|---|---|---|---|---|---|
| T1 | 正常发布 | 单线程 | approved→close→publish Ok | applied+completed+version1 | applied=1 |
| T2 | 已发布 replay（时序 1） | barrier：close 读完 approved_ids 后、调 publish 前放行竞争者完成发布 | close 的 publish 短路 Ok(replayed) | 同上 | applied=1（短路计入；replayed 字段=0——短路不经 Err 分支） |
| T3 | CAS 提交型 Err（E8） | 预置版本 1+提案基线 0 | Err(Conflict)→分类 JOIN=(failed,failed) | failed+failed | failed=1 |
| T4 | 回滚型 Err（E9 L3 Forbidden） | 未解决 conflict | Err(Forbidden)→分类 (approved,None) | approved 保持+conflict | failed=1 |
| T5 | **Err 后竞争者成功发布（时序 2 端到端）** | barrier：close 的 publish 返回 Err 后、分类事务前同步点 1；竞争腿 resolve+publish 完成后 barrier 放行分类 | 分类 JOIN=(applied,completed) | applied+completed+version1 | **replayed=1** |
| T6 | **分类持锁期间竞争者无法完成** | 测试自建协议同款事务：BEGIN+P+JOIN 且**保持打开**；并发 publish_version 断言未完成；释放后 publish=Ok(replayed) | P 互斥实证 | applied+completed+version1 | （协议级） |
| T7 | 混合计数（3 提案：1 正常/1 预发布/1 CAS） | 预置态+barrier 组合 | 各走各径 | 三提案各自终态 | applied=1,replayed=1,failed=1 |
| T8 | 分类事务自身 Db 错 | 受控失效连接注入（占满 pool 至分类获取失败/或 mock 层） | 分类 row=Err→warn+failed | 提案态保持 Err 时态 | failed=1；close 不 panic、报告仍 closed |

（T2 断言依据=守卫不可达性+短路语义；T5/T6 为 FIX-03 点名场景；全部 barrier=tokio::sync::Barrier/oneshot。）

## 六、修改文件清单与预期 diff 范围

| 文件 | diff 范围 |
|---|---|
| crates/pm-application/src/analysis_report.rs | pc 段 Err 分支（:598-604 → §2.1 形态）+projc 段（:662-671 同构）+CloseOutcome 两字段与 II-A doc（:107-123）+tracing 引入 |
| ai-frontend/src/api/analysis-reports.ts | 接口+2 字段（:90-99） |
| crates/pm-application/tests/q10_race_closure.rs | T5/T7 新增+场景 C 断言升级（T2 语义重述） |
| crates/pm-application/tests/q10_r3_snapshot_check.rs | **原样保留**（契约 6）；T3/T4 断言可复用其夹具 |
| 新 tests/q10_classify_matrix.rs（或并入 q10_race_closure.rs） | T1-T8 矩阵载体 |

**零改动**：publish.rs 两文件/0034-0037/触发器/close 外层结构/隔离级别/连接池配置。

## 七、风险与回滚

| 风险 | 缓解 |
|---|---|
| 分类事务获 P 排队（竞争者长发布） | 毫秒级窗口+与既有 publish 排队同量级；外层逐提案容错不变 |
| 分类 JOIN 慢查询 | 单提案主键点查+唯一键点查（基数≤1 已证）；无新索引需求 |
| tracing 依赖 | pm-application 已用 tracing（既有依赖，零新增） |
| 回滚 | 纯代码 revert（零 Schema）；计数行为回 V1.0 前形态；R3/既有测试不依赖新字段 |

## 八、与负责人裁决 B 的一致性说明（九条基线逐条）

| 基线 | 落位 |
|---|---|
| 1 保留裁决 B | 方案=B 的最终实现形态（FIX-02 §Q4：忠实演进，非撤销） |
| 2 采用 B'' | §二 |
| 3 II-A 契约 | §三（doc comment+显式不保证声明） |
| 4 既有键+独立只读事务 | §2.1/§2.2①（键逐字同 :117） |
| 5 不改两事务边界 | §2.2④⑤+§4.1 |
| 6 零新锁机制/零 Schema/触发器 | §六零改动清单 |
| 7 禁外层全程持 P | §4.5（外层任何时刻不持 P） |
| 8 不实施终段持有 | 未采用（FIX-02 授权外结论维持） |
| 9 GAP-32 OPEN | 实施验收完成前不变；本方案不构成关闭 |

## 九、零实施声明

本 V1.2 为方案+证明交付：未修改任何代码/测试/Schema/迁移/触发器；GAP-32 维持 OPEN；实施以负责人对本方案的明确批准为准。
