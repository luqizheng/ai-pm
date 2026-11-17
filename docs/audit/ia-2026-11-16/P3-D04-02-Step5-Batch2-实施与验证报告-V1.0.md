# P3-D04-02 Step 5 Batch 2 实施与验证报告 V1.0

- 编制日期：2026-11-18
- 性质：实施与验证报告（统一实施授权下第 2 批；授权=负责人《剩余批次一次性实施授权与执行指令 V1.0》2026-11-18）
- 任务书基线：《Batch2 实施任务书 V1.0》（commit 203e950；按统一授权指令 §一/§四 直接生效，不另设逐批批准门）
- Git 基点：`203e950`
- 范围遵守：仅 GAP-35 + GAP-31-A；零 Migration；GAP-31-B/INVALID_CONTRACT 未实现（Q-06 后置）；未触碰 09①③/22/32/37-C

---

## 一、实施内容

### 1.1 GAP-35 决策终局收敛（两域镜像）

| 对象 | 变更 |
|---|---|
| pc/proposal.rs record_decision | ①状态守卫收窄：`submitted/approved/rejected` → 仅 `submitted`（approved/rejected 终态 409，消息=decisions are terminal）；②同态重放分支不动（GAP-14 保持）；③pending 审批行 None 分支由「补插新行」改为 fail-closed 409（重决补插路径随跨态翻转废止，AGENTS 规则 1）；④提案 UPDATE `status IN ('submitted','approved','rejected')` → `= 'submitted'`（DB 端兜底）；⑤相关 doc comment 终局语义化（决定暂记→终局，Q-02b/Q-11 锚注） |
| projc/proposal.rs record_decision | 同款镜像（ProjcError/uq_projc_approval_pending/projc.approval） |
| 跨态拒绝路径 | 守卫在一切写点之前 → 不产生 approval 行/审计事件（T-1 零残留断言承载） |

### 1.2 GAP-31-A 发布键内化（两域镜像）

| 对象 | 变更 |
|---|---|
| pc/publish.rs、projc/publish.rs | ①`PublishVersionInput` 废止 `operation_id` 字段（入参内化=键来源裁决 §四 影响范围）；②`publish_inner` 入口派生 `format!("{pc\|projc}-decision:{}", proposal_id)`（与 close 链同款）；③全部 6 处 `input.operation_id` 引用改派生局部变量；④struct doc 终局语义化 |
| analysis_report.rs（close 链两处） | 构造改 `PublishVersionInput { proposal_id }`——行为零变化（同键同值） |
| fail-closed 形态 | 内化后发布入口不存在键输入面=非规范键结构性无法进入；**INVALID_CONTRACT 错误分支不实现**（31-B=校验位置/wire 命名，Q-06 B 级后置） |

## 二、测试面与实际结果

### 2.1 修改与新增清单

**新增**：
- `pc_proposal.rs::decide_is_terminal_cross_state_rejected`（T-1 pc）：approve 终态+同态重放幂等（审计=1）→ approved→reject 409 → rejected→approve 409 → 零残留断言（approval=2、审计 approve=1/reject=1）。
- `projc_formal_chain.rs::decide_is_terminal_cross_state_rejected`（T-1 projc）：同构（ProjcError::Conflict；审计断言内联 SQL）。

**修改（全部为冻结裁决强制的语义收敛，非测试弱化）**：
| 位置 | 变更 | 依据 |
|---|---|---|
| pc_proposal.rs `decide_is_provisional_and_publish_is_idempotent` | 更名 `decide_is_terminal_and_publish_is_idempotent`；互改断言反转（rejected→approve 由 Ok 改 409；提案保持 rejected） | GAP-35；Q-02b/Q-11（总账 GAP-35 行明列 decide_is_provisional 测试修改） |
| pc_proposal.rs `decision_gates_fail_closed` | 互改段（approve→reject→approve）改终局断言（跨态 409×1；保持 approved） | 同上 |
| pc_proposal.rs / pc_model.rs / projc_formal_chain.rs / apps pc_proposals.rs | 28 处 `PublishVersionInput{operation_id}` 构造改单字段（7+9+10+2） | GAP-31-A 验收定稿（键来源裁决 §四：任意串测试须随实施改造） |
| pc_model.rs 两处账本键断言 | `'op-publish-1'`/`'op-cas'` → 派生键 `format!("pc-decision:{{}}", proposal)` 绑定 | 同上 |
| projc_formal_chain.rs | op_id 变量改派生键形状；fixture-2 账本 SQL 改派生键绑定；**applied 后同提案再发布由 409 断言改为幂等重放断言**（replayed=true+同 version_id/version_no） | Q-04 同态重放：派生键按提案→同 proposal 再发布=同逻辑操作重放（设计语义，非弱化） |
| pc_proposal.rs/projc_formal_chain.rs | 「决定暂记/close 前可改」等陈旧注释终局化 | 注释同步（任务书 T-2） |

### 2.2 实际执行结果

目标套件（改造面）：
```
pc_model          ok. 6 passed; 0 failed（8.22s）
pc_proposal       ok. 9 passed; 0 failed（8.53s）← 含新 T-1
projc_formal_chain ok. 7 passed; 0 failed（8.44s）← 含新 T-1
```
全量回归（详见 §三）：pm-application 全部测试二进制 `test result: ok`（两轮独立执行均零 FAILED）；pm-api `analysis_reports`+`pc_proposals` 全过（见 §三 日志归档 target/b2-regression.log→报告归档版）。

## 三、全量回归

- `cargo test -p pm-application`（74 测试二进制，RUSTC_WRAPPER=""，DATABASE_URL=127.0.0.1:16433）：全部 ok，0 FAILED（实施前基线核验轮+实施后轮各一）。
- `cargo test -p pm-api --test analysis_reports --test pc_proposals`：全部 ok（close 链 E2E 回归不变=31-A 验收④）。
- `_sqlx_test%` 泄漏检查：0。

## 四、GAP 逐项验收对照

| GAP | 验收项 | 结果 |
|---|---|---|
| GAP-35 | 跨态重决 409（approved→reject/rejected→approve，两域） | ✅ T-1 实测 |
| GAP-35 | 同态重放幂等保持（GAP-14 不动） | ✅ 既有断言保留+新用例复核（审计不重复） |
| GAP-35 | decide_is_provisional 测试修改 | ✅ 更名+断言终局化 |
| GAP-35 | 互改写路径唯一性维持（E-51 口径） | ✅ 守卫收敛+UPDATE 集合收窄，无新增写路径 |
| GAP-31-A | operation_id 入参废止/内化 | ✅ 两域 struct+派生 |
| GAP-31-A | 派生规则与 close 链同款 | ✅ `{pc\|projc}-decision:{proposal_id}` |
| GAP-31-A | close 链回归不变 | ✅ analysis_reports/pc_proposals 全过 |
| GAP-31-A | 任意键测试改造 | ✅ 28 处+两处账本断言 |
| GAP-31-B | 不实现 | ✅ 无 INVALID_CONTRACT 分支/无校验位置代码 |

## 五、回滚方案（未触发）

纯代码 revert（零 Schema）；互改能力与显式键入参随 revert 恢复；复跑任选一套回归。

## 六、边界自检

未重开裁决（Q-02b/Q-11/键来源裁决均为既有已裁语义落地）；未触碰 Q-05~Q-10；未实施 31-B/09①③/22/32/37-C；零 Migration/零 Schema；原测试仅按冻结裁决做语义收敛改造（逐条依据见 §2.1），无删除/跳过/弱化；本批「行为变化」=互改能力废止+applied 重发布从 409 改幂等重放，均为已裁设计语义的落地。

**状态：Batch 2 实施与验证完成，独立验收证据齐备；按统一授权自动进入 Batch 3。**
