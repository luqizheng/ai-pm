# P3-D04-02 Step 6 前置 · Q10-R1-FIX — V1.2.1 实施前修订方案

- 编制日期：2026-11-18
- 性质：**V1.2 的最小范围修订（只读；零代码/测试/Schema 修改）**——R1-R4 四项；不重议 B''/II-A；GAP-32 维持 OPEN
- 依据：V1.2（c562a20）+ 实施前修订要求；本轮新增代码取证（行号实测）

---

## R1：分类事务错误处理（修正 §2.1/§2.4 行为不一致）

### R1.1 五失败点分别定义（V1.2 §2.1 示例的隐式 `?`/吞错全部消除）

| 失败点 | 事务状态 | 锁状态 | 清理 | 计数 | 日志 |
|---|---|---|---|---|---|
| F1 `pool.begin` 失败 | 无事务 | 未获任何锁 | 无需 | failed | warn(kind=begin) |
| F2 advisory 获取失败 | 开启未提交 | 未持有（语句失败） | Tx drop=回滚 | failed | warn(kind=advisory) |
| F3 JOIN 失败 | 开启**且持 advisory** | 持有中 | **显式 `rollback()`**（失败再 drop，见 F5）→释放 | failed | warn(kind=join) |
| F4 COMMIT 失败 | 中止态 | 随中止释放 | Tx drop（sqlx 对 aborted tx 的 drop=无操作，服务端已回滚） | failed | warn(kind=commit) |
| F5 ROLLBACK 失败 | 清理失败（连接可能坏） | 由服务端会话状态回收 | drop+warn；连接归还池由 sqlx 健康检查淘汰 | failed（已定） | warn(kind=rollback, source=…) |

### R1.2 可实施 Rust 伪代码（产品 Err 分支统一入口；两域同构）

```rust
Err(publish_err) => {
    // R-1 分类（II-A）。技术故障绝不产生 Replay——Replay 仅由
    // 持锁事务内 JOIN 确认 applied∧completed 产生（不变式 I-1）。
    let classified = classify_after_publish_err(pool, project_id, ProposalId(proposal_id)).await;
    match classified {
        ClassifyOutcome::Replay => replayed_proposals += 1,
        ClassifyOutcome::Failed => publish_failed_proposals += 1, // 业务失败
        ClassifyOutcome::FailedTech(kind, e) => {
            // 结构化记录技术故障（与业务失败可辨）；计数口径 II-A 仍 failed
            tracing::warn!(kind, proposal_id = %proposal_id, source_publish_error = %publish_err, error = %e, "R-1 classify tx technical failure");
            publish_failed_proposals += 1;
        }
    }
}

async fn classify_after_publish_err(pool, project_id, id) -> ClassifyOutcome {
    let mut cls = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return FailedTech("begin", e),            // F1
    };
    if let Err(e) = sqlx::query(ADVISORY_SAME_AS_PUBLISH)   // 逐字同 publish:117
        .bind(project_id.to_string()).execute(&mut *cls).await {
        return FailedTech("advisory", e);                    // F2（drop=回滚）
    }
    let row = sqlx::query_as(JOIN_SQL).bind(id.0).fetch_optional(&mut *cls).await;
    let row = match row {
        Ok(r) => r,
        Err(e) => {                                          // F3：显式回滚释放 advisory
            let rb = cls.rollback().await;
            return match rb {
                Ok(()) => FailedTech("join", e),
                Err(rbe) => FailedTech("rollback", rbe),     // F5（源错误并入日志链）
            };
        }
    };
    match cls.commit().await {                               // F4
        Ok(()) => match row {
            Some((p, w)) if p == "applied" && w.as_deref() == Some("completed")
                => Replay,
            _ => Failed,                                     // 业务：未确认先行完成
        },
        Err(e) => FailedTech("commit", e),                   // 行未采信
    }
}
```

**不变式**：I-1 `Replay ⟸ 持锁 JOIN 确认`（唯一来源）；I-2 技术故障必带 `FailedTech`+warn（不伪装 replay、不静默）；I-3 F3 必显式回滚（勿依赖 drop 时序）。

## R2：全局锁序证明补充（状态写入路径全量核查）

### R2.1 写入路径矩阵（CODE FACT，grep 全量实测 2026-11-18）

**`proposal.status='applied'` 写点（判定式左谓词）**：

| 路径 | 位置 | advisory | 结论 |
|---|---|---|---|
| publish_inner | pc/projc publish.rs:630 | **P（:117/:118）** | **唯一写者，必持 P** ✓ |

**`write_operation(decision 键行).status` 写点（右谓词）**：

| 路径 | 位置 | 键 | 结论 |
|---|---|---|---|
| publish completed | publish.rs:644 两域 | decision 键 | 持 P ✓ |
| publish failed（CAS） | publish.rs:319 两域 | decision 键 | 持 P ✓ |
| 起草段 completed | proposal.rs:1872/1869、identity.rs:294/208 | **analyze/identity 键**（WHERE operation_id=各自键） | 不触 decision 行 ✓ |
| 起草段认领 INSERT | proposal.rs/identity | 同上键 | ✓ |

**其余 proposal.status 写点（与判定式的交互）**：

| 路径 | 位置 | 目标态 | advisory | 持行锁 | 与分类交互 |
|---|---|---|---|---|---|
| decide | proposal.rs:1071 两域 | approved/rejected（仅 submitted） | 无 | proposal FOR UPDATE→approval FOR UPDATE | ①不改 applied；②不碰账本；③对 approved/failed 提案=守卫 409（读态即返） |
| withdraw | proposal.rs:1188 两域 | withdrawn（draft/submitted） | 无 | proposal FOR UPDATE | 同上（approved=409） |
| expire | proposal.rs:1275/:1273 | expired（submitted） | 无 | proposal FOR UPDATE | 同上 |
| submit | proposal.rs:829 两域 | submitted（draft） | 无 | proposal FOR UPDATE | 同上 |
| close withdraw 段 | analysis_report.rs:619/:676 | withdrawn（draft/submitted，**外层 tx**） | 无 | 清单 FOR UPDATE | 段序在 publish 段之后（同外层顺序执行）——不与分类并发同提案 |

### R2.2 正确性与无环论证（升级为全局）

1. **判定两谓词的写入面被 P 完全覆盖**：applied 与 decision 键行 completed/failed 的**全部**写者=publish_inner（唯一、必持 P）——不依赖"publish 用 advisory 所以一切受保护"的泛化推理，而是**逐写点枚举**（R2.1 表）；
2. **分类持 P 期间本提案无合法写者**：Err 后可达态∈{approved（回滚型）, failed（CAS）}——decide/withdraw/expire/submit 对二者全部守卫拒绝（409/403）；先行完成态 applied 时这些路径同样全拒（终态）；→ 持锁 JOIN 结果封闭；
3. **行锁交互与等待**：分类 JOIN 需 proposal 行 **AccessShare**；decide/withdraw/expire/submit 持该行 FOR UPDATE（Exclusive 类）→ 分类可能**短暂等待**这些短事务提交——它们**不请求 P**（矩阵 advisory 列=无）→ 等待边单向 `分类(P持有者)→行锁`，无反向 `行锁持有者→P` → **无环**；
4. **跨域**：pc 分类持 `pc_publish` 键时 projc 段用 `projc_publish` 键（不相交）；起草键 `-analysis:/-identity:` 亦不相交（矩阵键列）；
5. **修正 V1.2 §4.5 的表述**：原"账本/提案/审批行锁持有者不请求 P"结论维持，但证明依据由概述升级为 R2.1 逐路径枚举；新增行锁等待说明（V1.2 未述：AccessShare×FOR UPDATE 短等待，非零但无环）。
6. **未发现需扩大锁范围的风险** → 无最小修正建议提交。

## R3：测试矩阵完善（T8 拆分 + 真实集成优先）

| # | 场景 | 注入方式（真实优先） | 事务清理 | 计数 | 日志 | 报告 | 后续 publish |
|---|---|---|---|---|---|---|---|
| T8-a | pool.begin 失败 | **真实集成**：`PoolOptions::max_connections(1).acquire_timeout(短)` 专属池+外部占位连接→close（传小池）进 Err 分支时 begin 超时 | 无事务 | failed | warn(begin) | 正常 closed | 释放占位后新提案正常（新连接） |
| T8-b | advisory 获取失败 | **协议级集成**：测试自建同款分类事务→`pg_terminate_backend` 其连接→advisory 语句报错；产品路径以 R1.2 封装函数+受控坏池单测佐证 | drop=回滚 | failed | warn(advisory) | 同上 | 同上 |
| T8-c | JOIN 失败 | 同上 terminate 时机后移至 JOIN | **显式 rollback 路径实测**（伪码 F3） | failed | warn(join) | 同上 | 同上 |
| T8-d | COMMIT 失败 | terminate 时机后移至 commit 前 | aborted tx 服务端已回滚；drop 无操作 | failed（行不采信） | warn(commit) | 同上 | 同上 |
| T8-e | rollback 自身失败 | 极端注入（terminate 双发） | 连接淘汰 | failed | warn(rollback) | 同上 | 池健康检查后恢复 |

（T8-b/c/d/e 的产品内注入不可达点以"协议同款事务+terminate 真实集成"承载 DB侧行为，产品分支逻辑以 R1.2 封装受控池单测承载——**不全部 mock**；每项均断言 close 不 panic、报告 closed、下一提案发布正常。）

## R4：T2 与 replay 语义一致性（V1.2 遗漏缺陷修正）

### R4.1 缺陷确认

`PublishReceipt.replayed`（publish.rs:33 两域）在**短路分支**=true——先行发布完成且被 close 经短路确认时，V1.2 的 Ok 分支统一计 `applied`：**错误归类**（按 §三契约应为 replayed）。V1.2 §五 T2 注"applied=1（短路计入）并自我合理化"——指令 R4 指正成立。

### R4.2 修订（纳入 D1/D2，属 R-1 计数分类同族不扩范围）

```rust
// BEFORE（V1.2 及现状）
Ok(_) => applied_proposals += 1,
// AFTER
Ok(receipt) => {
    if receipt.replayed { replayed_proposals += 1 }
    else { applied_proposals += 1 }
}
```

### R4.3 契约/T2 同步

- doc comment（§三）三分语义不变，但**归类规则明确化**：replayed 有且仅有两个来源——①Ok(replayed=true)（短路确认）②分类事务确认（applied∧completed）；published/applied=本次 close 自身完成的首发；
- T2 预期修订：`applied=0, replayed=1, failed=0`（版本恰 1）；
- T7 混合计数相应更新：三提案=1 首发+1 短路 replay+1 CAS failed→`(applied,replayed,failed)=(1,1,1)`；
- 语义收益：契约三分类在 Ok/Err 两分支全对齐。

## 更新后交付物汇总

| 项 | 变化 |
|---|---|
| 锁序证明 | §R2（逐写点枚举+AccessShare 等待说明）取代 V1.2 §4.5 概述 |
| 测试矩阵 | T1/T3-T6 维持；T2/T7 预期修正（R4）；T8→T8-a~e（R3） |
| 预期 diff 清单 | V1.2 §六 + **新增**：close Ok 分支拆分（两段，各 +4 行）+ `classify_after_publish_err` 封装函数（含 ClassifyOutcome 枚举）+ tracing warn |
| 契约文档 | §三 doc comment 增补 replayed 双来源与归类规则 |

## 与 V1.2 及九条基线关系

四项修订均在 V1.2 范围内**收敛**（错误处理显式化/证明升级/测试细化/同族缺陷修正），无一条触碰基线 5-8 的禁止面；R4 属计数分类语义完整化（R-1 目标本身），非范围扩大。

## 零实施声明

本 V1.2.1 为方案修订：未修改代码/测试/Schema/迁移/触发器；GAP-32 维持 OPEN；实施仍待负责人审批（审批对象=V1.2+本 V1.2.1 合并语义）。
