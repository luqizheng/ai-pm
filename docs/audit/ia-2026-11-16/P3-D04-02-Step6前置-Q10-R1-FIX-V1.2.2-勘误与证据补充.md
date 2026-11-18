# P3-D04-02 Step 6 前置 · Q10-R1-FIX — V1.2.2 勘误与证据补充

- 编制日期：2026-11-18
- 性质：**V1.2.1 的短勘误（只读；零实施）**——四点澄清；不推翻 V1.2.1、不改裁决 B、不扩范围；GAP-32 维持 OPEN
- 版本事实：PostgreSQL 16（容器基线）/ sqlx **0.8.6**（Cargo.lock 实测）

---

## 一、R1 勘误：双重错误保留 + 连接淘汰表述降级

### 1.1 JOIN+ROLLBACK 双故障（采纳审阅方结构）

`ClassifyOutcome::FailedTech` 增 `cleanup_error: Option<sqlx::Error>`；F3 分支修订：

```rust
Err(join_error) => match cls.rollback().await {
    Ok(()) => FailedTech { kind: "join", error: join_error, cleanup_error: None },
    Err(rollback_error) => FailedTech {
        kind: "join_and_rollback",
        error: join_error,                              // 原始故障不丢失
        cleanup_error: Some(rollback_error),            // 清理故障并列保留
    },
},
```

warn 日志序列化两错误链（`error`+`cleanup_error`）。其余失败点（F1/F2/F4）单错误形态不变。

### 1.2 「sqlx 健康检查淘汰」表述降级（指正成立）

V1.2.1 F5 之"连接归还池由 sqlx 健康检查淘汰"**不是已证明事实**，降级为待验证假设。sqlx 0.8.6 实际行为（据其连接池实现：`Pool` 对使用中连接的中止处理依赖归还时的 `is_closed` 检测与 `test_before_release` 语义；事务连接 drop 归还路径中检测 IO 层断连）——**实施期以 T8-e 集成实测确认**（terminate 后连接被池剔除、后续获取正常），证据入修复报告；在此之前不将其作为正确性依据（正确性不依赖之：即使坏连接滞留，后续 `begin` 报错走 F1→failed+warn，语义仍安全）。

## 二、R2 勘误：锁类型双层修正 + READ COMMITTED 快照证明重写

### 2.1 表级锁与行级锁分层（修正 V1.2.1 R2.2 第 3 点的错误表述）

**V1.2.1 错误**：「分类 JOIN 需 proposal 行 AccessShare；…可能被 decide 的 FOR UPDATE 短暂阻塞」——术语与语义双错：普通 SELECT/JOIN 获取的是**表级 ACCESS SHARE**，**不取行锁**；MVCC 下纯读**不等待**他事务持有的行级 FOR UPDATE（行锁仅阻塞写与另一 FOR UPDATE/SHARE 行锁请求）。

**修正后的证明（两层）**：
- **表级层**：分类 JOIN 持表级 ACCESS SHARE（proposal/write_operation 两表）——与 DDL/表级排他锁冲突，与一切 DML 并发兼容；运行面无 DDL → **无表级等待**；
- **行级层**：分类事务**不请求任何行锁** → 与 decide/withdraw/expire/submit 的行级 FOR UPDATE **零等待交互**（它读这些事务提交前的最新已提交版本——即 Err 后态 approved/failed，分类 failed，正确）；decide 等对 approved/failed 提案守卫拒绝（V1.2.1 R2.1 已证）→ 即使它们并发作用于同提案，其未提交变更**不可见且不可能落地为 applied/completed**。

**净效果**：分类事务在持 P 期间的**唯一阻塞点=advisory 获取处**（等待先行发布者）；JOIN 本身即时执行。V1.2.1 的"行锁短等待"断言**删除**——证明更强而非更弱。

### 2.2 READ COMMITTED 语句快照证明（替换"锁内终局"笼统表述）

advisory 语句与 JOIN 语句在 RC 下**各自取得独立语句快照**（审阅方四步框架采纳）：

1. 分类事务执行 `pg_advisory_xact_lock(P)`：若先行发布者持 P，**在此等待**；
2. 先行者提交（applied+completed 同事务落库）并释放 P；分类事务获得 P；
3. 此后**任何发布者**（必经 P）在分类事务提交前无法进入临界区；
4. JOIN 作为**新语句**取得新快照——该快照 ≥ 步骤 2 的提交点（RC 语句快照取自语句开始时最新已提交），故先行者效果**必然可见**；步 3 保证无更新的发布效果存在 → JOIN 读到的即 **P 序列化下的分类时点终局**。

正确性依据=「advisory 串行化 + 语句快照单调包含先行提交」，**不依赖**"事务固定快照"（该表述错误，撤回）。两谓词写入面被 P 覆盖（V1.2.1 R2.1 逐点枚举）不变。

## 三、R3 勘误：T8 确定性同步补强

### T8-a 修订（F1 精确覆盖改经**封装边界**达成，非时序插桩）

黑盒下「publish 已归还连接、分类未 begin」的窗口无法用外部占位确定性命中（占位过早将饿死 publish 子事务本身——审阅方指出的风险成立）。修订为两层：

- **T8-a-1（确定性，精确覆盖 F1）**：`classify_after_publish_err` 已是独立封装函数（V1.2.1 §R1.2）——其**入口即 publish Err 之后**（函数契约）。单测：受控池 `max_connections(1)+acquire_timeout(短)`+测试线程先占唯一连接 → 直调函数 → `pool.begin` 超时 → 断言 `FailedTech{kind:"begin"}`+warn。无时序竞态；
- **T8-a-2（端到端，行为级）**：小池(max=2)+占位，跑完整 close——断言不 panic、报告 closed、failed 计数、日志含池超时——**不宣称精确覆盖 F1**（覆盖"池耗尽被逐提案容错吞没"面）。

### T8-b~e 修订（terminate 目标确定性）

放弃跨线程时序猜测：故障注入改在**测试自持的协议同款事务连接**上进行——同一线程内顺序执行至目标语句边界（天然同步点：advisory 已获/JOIN 未发、JOIN 已发未 commit 等，由语句顺序即代码顺序保证），`pg_backend_pid()` 取 pid 后 `pg_terminate_backend` 自终止，再执行下一语句得 Err——**终止目标与时机由同线程控制流唯一确定**。产品分支逻辑（F2-F5 分类）另以封装函数+受控池（坏连接注入）单测覆盖。每项断言集不变（清理/计数/日志/报告/后续 publish）。

## 四、R4 勘误：failed 口径按审阅方措辞定稿

CloseOutcome doc comment（II-A）failed 定义修订为：

> **failed：本次未计为成功发布——包括**①业务失败（分类时点确认无先行完成且本次尝试失败）**与**②分类事务技术故障（**无法确认**是否存在先行完成；≠已观察到未发布）。技术故障经结构化 warn 日志（kind/error/cleanup_error）区分，不改变计数口径。

三分类最终形态（与审阅方 §三 表一致）：applied=本次 close 自身完成首发；replayed=已确认先行发布完成（来源双入口：`Ok(replayed=true)` 短路 / Err 后锁内 JOIN 确认）；failed=如上。**已观察未发布**的表述仅适用于 failed①，不适用 failed②。

## 五、更新后审批对象汇总

V1.2（c562a20）+ V1.2.1（531088e）+ 本 V1.2.2 = 最终审批对象；diff 相对 V1.2.1 仅增：`FailedTech.cleanup_error` 字段与双错误日志、T8-a-1 封装单测载体、doc comment failed 措辞。其余（锁序证明换证更强、T8 设计确定性升级）不增产品 diff。

## 六、零实施声明

本勘误只读：未修改代码/测试/Schema/迁移/触发器；sqlx 连接淘汰行为待 T8-e 实测（登记为实施期验证项而非事实引用）；GAP-32 维持 OPEN；实施批准仍待负责人对合并对象的最终审批。
