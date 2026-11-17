# P3-D04-02 Step 5 Batch 5 实施与验证报告 V1.0

- 编制日期：2026-11-18
- 授权：负责人《剩余批次一次性实施授权与执行指令 V1.0》（2026-11-18）
- 依据：总账 V1.4 GAP-06/10/11/21/29 行+《19 项 GAP 实施责任矩阵 V1.0》对应行+设计方案 V1.0.7 §D1.2 合法转换表
- 范围遵守：仅 GAP-06+10+11+21+29；22/32（Q-07/Q-10 硬阻塞）/31-B（Q-06）/09①③/37-C 未触碰

## 一、实施内容

### 1.1 Migration 0034（GAP-06，两域镜像）

`{domain}.guard_proposal_transition()` BEFORE UPDATE 触发器（纯拒绝型，无推进逻辑）：allowlist=§D1.2 冻结转换表 8 转换（draft→{submitted,withdrawn}；submitted→{approved,rejected,withdrawn,expired}；approved→{applied,failed}）；同态（OLD=NEW）放行（T-09 幂等语义+非状态列更新不受影响）；其余 RAISE EXCEPTION。

### 1.2 GAP-10 draft 修改入口+重锚（两域 proposal.rs）

- 新服务命令 `update_proposal_draft(pool, project_id, actor, proposal_id, members)`；
- **共享管线抽取**：create_proposal_in 的 ①规范化②目标解析③F-05 端点并入④等级下限抽取为 `normalize_proposal_members`（create/update 共用=重锚语义与创建一致）；
- 仅 draft 可修改（FOR UPDATE+status 守卫+rows_affected 双保险）；基线 expected_version_no 不变（基线移动仍由 submit CAS fail-safe 承载=重新起草）；title/rationale/origin 维持原值（锚定面=成员集+元组）；等级下限复检（新成员抬升 L2 而存量 L1 → Invalid）；
- 重锚=content_hash 重算落列+成员集整组替换+同事务审计 `{domain}_proposal.draft_update`。

### 1.3 GAP-11 草稿物化审计（两域 object.rs）

upsert_object_draft 全部 4 分支落点（dedup 刷新/追加修订/identity 占位补首修订/新建对象）于 commit 前同事务审计 `{domain}_object.draft_upsert`（object_key/type/display_name/content_hash/revision_no/revision_created）。

### 1.4 GAP-21 对账巡检（`scripts/reconciliation/write_operation_consistency.sql`）

只读差分三段（零写路径）：①completed 发布账本行→version 缺失（孤儿账本，两域）；②正式版本→无对应 completed 发布行（无账本产物，两域）；③起草段自检（completed 起草行→活提案缺失，两域）。期望 0 行；非 0=违反同事务不变量（D6.7）须人工核查。

### 1.5 GAP-29 PC 侧冲突门禁用例（tests/batch5_gates_and_draft.rs）

R-06 口径（pc.conflict=Legacy）：直写 pc.conflict/conflict_object 种子测门禁代码（非复活写路径）——submit 侧冻结 403→resolved 放行；approve 侧由既有 decision_gates_fail_closed（frozen approve 403）承载；publish 侧由 pc_model 冻结用例承载。

## 二、测试与实际结果

```
batch5_gates_and_draft  ok. 4 passed; 0 failed（20.93s）
  illegal_transition_rejected_by_db_guard      —— draft→{applied,rejected,approved,expired,failed} 5×直写拒绝；同态放行；T-02 服务转换不受影响
  draft_upsert_audited_and_reanchor_allows_submit —— GAP-11 审计落行+GAP-10 重锚（锚列变更+审计）+submit 无 fail-safe 409+非 draft 409
  pc_conflict_gates_submit_and_approve         —— GAP-29 PC submit 冻结 403/解冻放行
  reconciliation_diff_is_empty_on_consistent_state —— GAP-21 双向差分=0/0（一次发布后自洽）
```

## 三、全量回归

并入 Step5 终态全量回归（pm-application+pm-api+pm-infrastructure+pm-mcp 四 crate 全量；结果落盘 target/final-regression-b5.log，见《Step5 统一实施最终交付报告》§回归）。

## 四、GAP 逐项验收对照

| GAP | 验收项（矩阵行） | 结果 |
|---|---|---|
| GAP-06 | 非法转换直写 DB 被拒 | ✅ 5 组合实测 |
| GAP-06 | 合法 9 转换不受影响（8+同态） | ✅ 同态放行+服务路径回归全绿 |
| GAP-10 | draft 期修改后 submit 不触发 fail-safe 409 | ✅ 实测 |
| GAP-10 | 修改产生新锚定（重算+更新 content_hash） | ✅ 锚列变更断言 |
| GAP-11 | 每次草稿落库有同事务审计行 | ✅ 实测 |
| GAP-21 | 差分报告可产出；零写路径 | ✅ 脚本+零差分实测 |
| GAP-29 | PC 侧三落点门禁用例绿 | ✅（submit 本用例+approve/publish 既有用例） |

## 五、边界自检

转换 allowlist 逐条对应 §D1.2 依据类别（未重开任何裁决）；GAP-10 未引入新权限面（draft-only+既有主体语义）；GAP-21 零写路径；dev 库 0034 已应用；泄漏检查并入终态回归收尾。

**状态：Batch 5 实施与验证完成；独立验收证据齐备。至此统一授权下全部可实施批次（Batch 2/3/4a/4b/5）实施完毕，转入最终交付报告与总账落账。**


## 六、终态回归补记（2026-11-18，如实登记）

1. **GAP-11 pc 侧两审计点补齐**：841f1d4 提交时 pc/object.rs 仅落 P3/P4 两分支审计（首轮实施脚本在 P3 处断言中止、P1/P2 未落盘——实施序缺陷）；终态回归前补齐 P1（dedup 刷新）/P2（追加修订）两分支，现两域各 4 分支全覆盖（本修复随终态文档 commit 入库）。
2. **0034 守卫暴露 pc_model 夹具直写跳态**：seed_approved_proposal 原以直写 draft→approved 布夹具（本属非法转换）——0034 守卫正确拒绝（守卫语义的又一实测证据）；夹具改为合法链 draft→submitted→approved 两步，pc_model 6/6 绿。
3. **终态全量回归首两轮两处环境/实施事件**（均已如实登记）：①并行 76 临时库触发 max_locks_per_transaction 耗尽（53200）×28——容器扩容 max_locks=512（与 max_connections=400 同惯例）后消除；②0032 曾遗漏落盘的 42703 级联（见 Batch4a 报告）。