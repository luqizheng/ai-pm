# P3-D04-02 Step 5 Batch 3 实施与验证报告 V1.0

- 编制日期：2026-11-18（与 Batch 4a 同文件族实施；独立验收证据）
- 授权：负责人《剩余批次一次性实施授权与执行指令 V1.0》（2026-11-18）
- 任务书：《Batch3 实施任务书 V1.0》（随统一授权直接生效）
- 范围遵守：仅 GAP-36+GAP-33+GAP-05+GAP-09②（=36 服务入口）；HTTP/MCP 接线（09①③）未触碰；无自动到期/无巡检（Q-12）；reason 长度上限未擅自设定

## 一、实施内容

### 1.1 Migration 0031（GAP-36 列承载，两域镜像）

pc/projc.proposal ADD `expired_at timestamp with time zone` + `expired_reason text`（纯增量，无数据迁移）。

### 1.2 GAP-36+09② expire_proposal（两域镜像，pc/proposal.rs + projc/proposal.rs）

- 服务入口 `expire_proposal(pool, project_id, actor, proposal_id, reason: Option<String>)`；
- 权限门=ensure_formal_authorized（真人+A3，fail-closed，与 decide/发布④同口径；AI 主体在主体类型层即拒=Q-12.6）；
- expired_reason 契约（§D2.2）：None/空/纯空白→Invalid；trim 存规范化值；
- 并发契约（§D4.10 八保证）：FOR UPDATE→获锁重检仅 submitted（其余 409）→UPDATE `SET status='expired', expired_at=now(), expired_reason=$, updated_at=now() WHERE ... AND status='submitted'`+rows_affected==1 双保险→同事务审计 `{domain}_proposal.expire`（reason 入 after_data）→COMMIT；
- pending 审批行保留原状（withdraw 先例）；expired_at=决策事实时间（R38，≠updated_at 语义）；无 Outbox 事件（V1 目录无对应项）。

### 1.3 GAP-05 decision_reason 必填（两域 record_decision）

- 位置=ensure_formal_authorized 之后（权限矩阵优先：AI/无 A3 先 Forbidden）、报告冻结门之前；
- `None/""/纯空白 → Invalid("decision_reason is required")`；规范化 String 落 approval.decision_reason 与审计 after_data；
- 原 trim-to-Option 块废止（入口已规范化）。

### 1.4 GAP-33 R38 时间字段分离（两域 publish.rs）

- publish_inner 在授权通过后、事务前捕获 `confirmed_at=now()`（发布确认决策时刻，与锁等待无关），cognition_version INSERT 显式写 confirmed_at；
- created_at 维持插入默认承载发布生效时间——两语义不再共用同一默认值（D7.1；confirmed_at 列两域已在位且有不可变触发器保护→零 DDL 变更）。

## 二、测试面与实际结果

**新增**：
- `pc_proposal.rs::expire_proposal_manual_terminal`（T-1/2/3/4 合一）：reason 契约 3×Invalid；AI/无 A3→Forbidden；正常路径列值+trim 规范化+审计=1；pending 行保留=1；同态再 expire 409；expired 后 decide 409。
- `projc_formal_chain.rs::expire_proposal_manual_terminal`：ProjC 镜像。
- `pc_proposal.rs::decision_reason_is_mandatory`（T-5）：approve/reject ×None/空/纯空白=6×Invalid；零残留（approval=1 仅 pending、审计=0、保持 submitted）；正常路径 decision_reason 落规范化值。

**修改（GAP-05 必填契约收敛，逐处对应冻结裁决）**：
- pc_proposal.rs：approve() 助手 reason=None→Some（测试批准）；`decision(x, None)` ×10 → Some("决定理由")；
- projc_formal_chain.rs：14 处 `reason: None` → Some("决定理由")（权限拒绝路径行为不变=Forbidden 优先）；
- apps pm-api tests：analysis_reports.rs 8 处 decide 夹具 reason=None→Some；pc_proposals.rs 跨态断言体补 reason（GAP-05 落地后须携带 reason 方达状态守卫——409 断言语义不变）。

**GAP-33 断言**：pc_model.rs 首发布用例增 `confirmed_at ≤ created_at` 断言（epoch float8 比对；决策时刻不晚于生效承载）。

### 实际执行结果（实施态）

```
pc_model           ok. 6 passed; 0 failed（8.54s）← 含 GAP-33 断言
pc_proposal        ok. 11 passed; 0 failed（14.88s）← 含 expire×1+reason×1 新用例
projc_formal_chain ok. 8 passed; 0 failed（8.46s）← 含 expire×1 新用例
```

全量回归：见《Batch4a 报告》§三（两批同轮回归，pm-application 76 二进制全 ok+pm-api 全 ok；本批用例全绿）。

## 三、GAP 逐项验收对照

| GAP | 验收项 | 结果 |
|---|---|---|
| GAP-36 | 转换入口 submitted→expired 人工触发（A3 fail-closed） | ✅ T-3 实测 |
| GAP-36 | expired_at/expired_reason 列+同事务四写+审计 reason 入 after_data | ✅ T-1 实测 |
| GAP-36 | reason 必填契约（null/空/空白拒绝+trim 规范化） | ✅ T-2 实测 |
| GAP-36 | 并发契约（终态互斥：expired 后 decide/expire 409） | ✅ T-1/T-4 实测 |
| GAP-36 | 无自动到期/无巡检/无 Outbox | ✅ 代码无对应面 |
| GAP-09② | expire 服务入口在位 | ✅（HTTP 不接线=09① 未授权范围） |
| GAP-33 | confirmed_at=发布确认决策时刻（显式捕获，≠created_at 语义） | ✅ T-6 断言 |
| GAP-33 | expired_at=决策事实时间（≠updated_at） | ✅ 列语义+审计 |
| GAP-05 | decide reason 必填（Invalid）+规范化落列 | ✅ T-5 实测 |

## 四、边界自检

未重开 Q-03/Q-12/D2.2/D4.10（全部为已裁语义落地）；reason 长度=未设限（HUMAN DECISION OPTIONAL 默认）；pending 审批行处置=withdraw 既有先例非新裁决；dev 库 0031 已应用；_sqlx_test% 泄漏检查=0。

**状态：Batch 3 实施与验证完成；独立验收证据齐备；按统一授权自动进入 Batch 4a。**
