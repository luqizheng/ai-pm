# P3-D04-02 Step 6 前置 · Q10-R1-FIX — GAP-32 关闭申请 V1.0

- 编制日期：2026-11-18
- 申请对象：GAP-32「close/decide 竞态处置」（总账 V1.10 当前状态=**已实施待验收**）
- 申请性质：**验收申请（非自行关闭）**——按硬边界 4，GAP-32 维持 OPEN 至负责人验收明示+归档完成后方可关闭；本文件=验收输入之一
- 审批链完整性：裁决 B（小范围修复授权，R-1 负责人）→ B''（锁内分类）→ II-A 契约（分类时点一致性）→ 最终审批对象=四件套合并语义（V1.2 c562a20 + V1.2.1 531088e + V1.2.2 b0c86c4 + 审批附件 7d8909f）→ **实施批准=负责人指令 2026-11-18**（审阅方已确认技术方向定稿，通过后不再讨论已冻结的架构与语义问题）

---

## 一、关闭条件核对（对照指令 §四 与硬边界）

| # | 条件 | 状态 | 证据 |
|---|---|---|---|
| 1 | 修复实施（D1-D4+R1 五失败点+R4 Ok 拆分+II-A doc） | ✅ | analysis_report.rs（classify_after_publish_err 两域+ClassifyOutcome+CloseOutcome +2 字段）+analysis-reports.ts（+2 字段）；修改面=授权 diff 清单逐项一致，零越界（报告 §八） |
| 2 | T1-T8e 定向测试全绿 | ✅ | q10_classify_matrix.rs 14/14（3 轮稳定 10.2-10.4s）+q10_race_closure 2/2（场景 C 断言升级）+q10_r3_snapshot_check 2/2（**原样运行，禁改达成**） |
| 3 | 全量回归 | ✅ | workspace `--no-fail-fast`（指令所列 pm-api 为部署容器名，仓库无该 crate——回归面取 workspace 全量覆盖；豁免口径=flow_01 网络抖动单跑验证/c84_c85 已知 git 环境）（报告 §六） |
| 4 | `_sqlx_test_%` 泄漏=0 | ✅ | 收尾复测=0（报告 §七） |
| 5 | 报告+总账落账+关闭申请 | ✅ | 本文件+《实施与验证报告 V1.0》+总账 V1.9→V1.10（GAP-32→已实施待验收） |
| 6 | commit & push（HEAD=origin/master，树净） | ✅ | 7b9bf8f |

## 二、申请关闭的差距内容

GAP-32 原记（总账）：「close/decide 竞态处置——现状=窄窗口+显式补发布兜底」。经 Q-10 裁决链收敛为 B''/II-A 实施形态：

1. **Err 分支分类事务**：close 批量发布 Err 后，经独立短命只读事务（BEGIN→既有项目级 advisory〔逐字同 publish.rs:117/:118〕→单条 JOIN→COMMIT）确认先行发布终局——applied∧completed→replayed；否则业务 failed；分类事务自身技术故障→FailedTech{kind,error,cleanup_error}+结构化 warn，计数仍 failed（**技术故障绝不产生 Replay**，不变式 I-1）；
2. **Ok 分支拆分**（R4）：receipt.replayed→replayed_proposals（短路确认=来源①）；分类 JOIN 确认=来源②；
3. **II-A 契约文档化**：CloseOutcome doc comment 明示三分类语义+replayed 双来源+failed 口径（业务失败+技术故障，无法确认≠已观察未发布）+显式不保证 COMMIT 时点终态一致（II-B 不可实现性=FIX-02 Q2，授权外变体未采用）；
4. **前端契约同步**：CloseReportOutcome +replayed_proposals/projc_replayed。

## 三、随验收须确认的实施期发现（报告 §三，均不触碰产品语义与硬边界）

1. **T5 注入机制**=pre-advisory Forbidden（附 L3-Forbidden 端到端 Replay 不可能性证明）；L3-Forbidden 的 Err/分类 Failed 路径由 T4 端到端覆盖；
2. **T7 字面 (1,1,1) CAS 耦合墙不可满足**（同代两真实发布互斥证明）；以两 close 组合 (1,0,1)+(0,1,1) 承载三计数路径；
3. **审计 payload（analysis_report.closed）未含 replayed 计数**——不在批准 diff 范围内未顺带扩展；如需，独立微批另行授权。

## 四、随本申请的运行约束登记（请求纳入运维文档）

分类事务持表级 ACCESS SHARE（pc/projc 的 proposal+write_operation 四表）期间，取得 ACCESS EXCLUSIVE 的 DDL 与之互斥——**迁移仅在应用实例停止或维护窗口执行**（现有部署惯例即如此；附件 B.3）。

## 五、验收请求

请负责人对以下事项明示验收：
- ① B''/II-A 实施形态符合四件套合并语义（含报告 §3.1/§3.2 两项测试设计发现的等价性）；
- ② T1-T8e 矩阵+全量回归+泄漏复测证据充分；
- ③ T8-e Pool 剔除实测背书、F2 清理限制声明、DDL 运行约束登记三项按附件口径接受；
- ④ 验收确认后：总账 GAP-32→「已满足」、活跃 3→2、关闭本差距并归档本申请。

**GAP-32 在验收明示前维持 OPEN。**
