# P3-D04-02 Step 5 Batch 2 实施任务书 V1.0

- 编制日期：2026-11-18
- 性质：**实施任务书（草案）**——交付待负责人审阅；**本任务书获批≠实施授权**
- 版本：V1.0（首版草案）
- Git 基点：`dec64b2`（Batch 1 落账后 HEAD=origin/master）
- ⚠ **执行门（与 Batch 1 同款双门）**：①本任务书经负责人审阅批准为基线 ②负责人明确下达 Batch 2 实施授权——两门齐备方可动工；不得提前修改任何代码/测试。
- **批次性质**：行为收敛批（设计冻结后首个行为变更批）——本批收敛的「互改能力」现状存在且有测试固化（TEST FACT），收敛=Q-02b/Q-11 已裁语义的落地，**不是新设计、不重开任何裁决**。

---

## 〇、任务目标

| GAP | 内容 | 总账 V1.4 状态锚 |
|---|---|---|
| GAP-35 | 决策终局语义实施：decide 跨态翻转收敛（仅 submitted 可裁决；同态重放幂等不动=GAP-14） | 活跃实施差距（全部裁决齐备） |
| GAP-31-A | 发布键：`PublishVersionInput.operation_id` 入参废止/内化=服务端确定性派生 `{pc\|projc}-decision:{proposal_id}`；close 链回归不变 | 活跃实施差距（键来源已裁，31-A 验收定稿） |

**不在本批**：GAP-31-B（wire 命名+校验实现位置=Q-06 B 级后置，**不实现 INVALID_CONTRACT 错误分支与任何键校验入口**）；GAP-01/27（账本扩展）、GAP-36/33/05/09②（expire 族）、GAP-09①③（HTTP 501 桩/MCP）、GAP-22/32（Q-07/Q-10 硬阻塞）、GAP-37-C/N-1（待负责人设立裁决）。**本批零 Migration、零 Schema 变更**（迁移编号 0031 不占用）。

## 一、权威输入与验收锚

总账 V1.4 GAP-35/GAP-31 行；Q-02b/Q-11 裁决归档记录；《显式补发布键来源裁决归档记录 V1.0》§二/§四（31-A 验收定稿）；设计方案 V1.0.7+V1.0.8 §D6.5（组合基线）；《Step5 实施批次规划 V1.0》（批 2 范围）；AGENTS.md 全部规则。

## 二、实施范围（获批后执行）

### 2.1 GAP-35：decide 跨态守卫收敛（两域镜像）

现状（CODE FACT，实施日须复核）：
- `pc/proposal.rs:970`：`if status != "submitted" && status != "approved" && status != "rejected"` ——放行 approved↔rejected 互改；
- `projc/proposal.rs:970`：同款守卫；`projc/proposal.rs:1057-1063` UPDATE `AND status IN ('submitted','approved','rejected')`。
- 同态重放分支（status==new_status → 幂等返回既有审批事实）在守卫**之前**，GAP-14 已满足——**本批不动**。
- 互改写路径仅此一处（E-51 全量写点清点：pc proposal.rs:1064 写点经此守卫；projc 同构）。

目标：
1. pc/projc 守卫收窄为 `status != "submitted"` → 其余状态一律 `Conflict` 409（approved/rejected 终态不可再决，Q-02b+Q-11）；
2. projc UPDATE 的 `status IN (...)` 集合同步收窄为 `= 'submitted'`（DB 端兜底同语义，防并发窗口内翻转）；
3. 409 消息与既有 Conflict 文案风格一致（说明仅 submitted 可裁决）；
4. 跨态拒绝路径不产生 approval 行/审计事件（守卫在写点前，事务回滚=天然零残留；T-5 断言承载）。

### 2.2 GAP-31-A：发布键内化（两域镜像）

现状（CODE FACT，实施日须复核）：
- `PublishVersionInput { proposal_id, operation_id }`（pc/publish.rs:21、projc/publish.rs:23）——operation_id 由调用方任意给定（pc_proposal.rs:409 "kp2-chain-1"、pc_model.rs:999 "op-publish-1" 等实证）；
- close 链已合规：analysis_report.rs:597/:661 派生 `{pc|projc}-decision:{proposal_id}`（E-52）。

目标：
1. **`PublishVersionInput.operation_id` 字段废止**（两域）；`publish_inner` 入口处服务端派生 `format!("{domain}-decision:{}", input.proposal_id)`——与 close 链同款（键来源裁决 §二.2「全部入口键构造规则统一」）；
2. close 链两处调用改为不传键（构造 `PublishVersionInput { proposal_id }`），行为零变化（同键同值）；
3. **fail-closed 结构性满足**：内化后发布入口不存在键输入面，非规范键无法进入（键来源裁决 §二.4 的落地形态）；INVALID_CONTRACT 错误分支属 31-B（校验位置=Q-06）**不实现**；
4. 幂等语义零变化：同 proposal 派生同键 → completed 短路/replayed=true 路径不动（GAP-15 范围限定维持）。

### 2.3 测试面（T-1~T-5）

| # | 测试 | 性质 | 验收锚 |
|---|---|---|---|
| T-1 | 跨态 409 负向用例（**两域各一**）：submitted→approve/reject 正常；approved→reject 409；rejected→approve 409；**同态重放幂等断言保留不动**（GAP-14） | 新增用例（建议 pc_proposal.rs/projc_formal_chain.rs 各一或独立文件） | 总账 GAP-35；Q-02b/Q-11 |
| T-2 | **互改固化测试改造**：pc_proposal.rs:1013-1020「rejected 仍可互改（approve 反转回 approved）」断言反转为 409；同名测试注释同步（「审阅流 v2：close 前可改」→「决定终局：submitted 后即终态可决，终态不可再决」） | 修改既有用例 | GAP-35；decide_is_provisional 测试修改（总账行） |
| T-3 | **发布键测试改造**：28 处 `PublishVersionInput { .., operation_id }` 构造点全量改为不传键（pc_model 9 / pc_proposal 7 / projc_formal_chain 10 / apps pc_proposals 2；实施日 grep 复核为准）；pc_model.rs:1046 账本 SQL 断言键改派生键 `pc-decision:{proposal_id}`；pc_proposal.rs:1476 注入行已用派生键形状=不动 | 修改既有用例 | GAP-31-A 验收定稿 |
| T-4 | 回归：pm-application `pc_proposal`+`pc_model`+`projc_formal_chain`+`projc_mirror_guards` 全量；pm-api `analysis_reports`（close 链 E2E）+`pc_proposals` 全量；sqlx::test 全新临时库 0001~0030 全序应用（本批无新迁移） | 既有回归 | close 链回归不变（31-A）；幂等重放语义不变（GAP-15） |
| T-5 | 追加断言：①跨态拒绝零残留（approval/审计/账本行数不变）②发布账本 operation_id=派生键（两域）③同 proposal 二次 publish → replayed=true 且版本数不增 | 融合 | GAP-35+31-A |

**改造边界**：仅守卫条件/入参结构与测试；不改审计动作名、不改状态机 CHECK（GAP-06 另批）、不动 write_operation Schema、不动 close 事务结构（GAP-32/Q-10 域）。

## 三、实施前置检查（动工日，全部只读）

1. Git：工作树干净+HEAD=origin/master；
2. GAP-35 现状复核：两域守卫行号与形状 grep 复核（漂移即停）；
3. GAP-31-A 现状复核：PublishVersionInput 字段/调用点/测试构造点计数复核；
4. 迁移编号 0031 空闲确认（本批零 Migration）；
5. close 链派生键现状确认（analysis_report.rs:597/661 未漂移）。

## 四、验收标准（8 项）

①守卫收敛生效（T-1/T-2 实测跨态 409）②同态重放幂等保持（既有断言不动）③发布键内化（T-3 全量改造+T-5 账本断言）④close 链回归不变（T-4 analysis_reports 全过）⑤全量回归通过 ⑥零 Migration/零 Schema ⑦审计/账本零残留断言（T-5①）⑧验证报告（含 GAP-35/31-A 逐项验收对照）。

## 五、回滚方案

纯代码 revert（本批无 Schema/数据面）：`git revert` 实施 commit 即恢复互改能力与显式键入参；无数据库对象需要清理；回滚后复跑任选一套回归。状态登记同 Batch 1 V1.2 R-6 口径（失败保留授权记录，不自动恢复实施资格）。

## 六、执行纪律（同 Batch 1）

127.0.0.1:16433／RUSTC_WRAPPER=""／_sqlx_test% 泄漏检查清理／完成即停等验收／不跨批／不触碰 31-B/Q-06/Q-07/Q-10 域／不自行宣告 GAP 状态变化（落账=总账维护者）。

## 七、交付物与停止点

本任务书（V1.0 草案）→ 负责人审阅批准为 Batch 2 基线【**当前停止点**】→ 负责人明确实施授权 → GAP-35+31-A 实施+T-1~T-5+验证报告 → 停止等验收 → commit&push 与批 3 由负责人决定。

## 八、零实施声明

本任务书为文档交付：未修改任何代码/测试/Schema/Migration/API/数据库；§二 全部内容为获批后执行的草案；未自行裁决 Q-06（31-B 范围）；未触碰 GAP-37-C/N-1 设立裁决。
