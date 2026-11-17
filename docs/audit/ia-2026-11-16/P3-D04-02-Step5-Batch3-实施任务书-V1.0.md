# P3-D04-02 Step 5 Batch 3 实施任务书 V1.0

- 编制日期：2026-11-18
- 性质：实施任务书（统一实施授权下第 3 批；授权=负责人《剩余批次一次性实施授权与执行指令 V1.0》§一/§四——本任务书随批交付作实施记录，不设独立批准门）
- Git 基点：`09e2dae`（Batch 2 后）
- 范围遵守：仅 GAP-36 + GAP-33 + GAP-05 + GAP-09②（=36 的服务入口）；HTTP 接线（09①）与 MCP（09③）不在本批；Q-05~Q-10 不触碰

## 一、目标与验收锚

| GAP | 内容 | 验收锚 |
|---|---|---|
| GAP-36 | expire 触发与承载：submitted→expired 人工终态（Migration 0031 两域 proposal 增列 expired_at/expired_reason + 服务入口 expire_proposal 两域 + 同事务审计） | T-10（V1.0.7 §60）+§D4.10 八保证+§D2.1 A3 作用域契约+§D2.2 输入契约+Q-12 裁决归档（模式 A） |
| GAP-09② | expire 入口（服务层，随 36 同批；零现状实现 E-45） | GAP-09 行（总账 V1.4） |
| GAP-33 | R38 时间字段分离：cognition_version.confirmed_at 由插入默认（=created_at 同值压缩承载）改为发布入口显式捕获「发布确认决策时刻」；expired_at=expire 决策事实时间（≠updated_at） | D7.1 字段表（V1.0.1 §244）+差距映射 v1.0.1 GAP-33 行+E-42 |
| GAP-05 | decision_reason 强制化：decide（approve/reject）入口 reason 必填，null/空/纯空白拒绝（Invalid），存规范化值 | 19 项 GAP 实施责任矩阵 V1.0 GAP-05 行+差距映射 v1.0 GAP-05 行（D1.3 决策理由约束） |

**不在本批**：自动到期/巡检 Worker（Q-12：V1 无）；HTTP expire 端点（09①）；reason 长度上限（HUMAN DECISION OPTIONAL，默认不加限制除非负责人确认）；GAP-01/27 账本扩展。

## 二、实施内容

### 2.1 Migration 0031（`migrations/0031_proposal_expire_columns.sql`，纯增量）

```sql
ALTER TABLE pc.proposal    ADD COLUMN expired_at timestamp with time zone;
ALTER TABLE pc.proposal    ADD COLUMN expired_reason text;
ALTER TABLE projc.proposal ADD COLUMN expired_at timestamp with time zone;
ALTER TABLE projc.proposal ADD COLUMN expired_reason text;
```

### 2.2 GAP-36+09② expire_proposal（pc/proposal.rs + projc/proposal.rs 镜像）

流程（与 withdraw/decide 同族实现纪律）：tenant→**ensure_formal_authorized（真人+A3，fail-closed，与 decide/发布④同口径）**→expired_reason 契约（D2.2：None/空/纯空白→Invalid；trim 存规范化）→BEGIN→proposal FOR UPDATE→获锁重检**仅 submitted**（其余状态 409）→UPDATE `SET status='expired', expired_at=now(), expired_reason=$, updated_at=now() WHERE ... AND status='submitted'`+rows_affected==1 双保险→同事务审计 `{domain}_proposal.expire`（before=status，after=status+expired_reason）→COMMIT。pending 审批行**保留原状**（withdraw 先例：审批表只承载裁决事实，无裁决人不落终态）。无 Outbox 事件（V1 目录无对应项）；无自动到期。

### 2.3 GAP-05 record_decision 入口 reason 必填（两域）

位置=ensure_formal_authorized 之后（权限矩阵优先：AI/无 A3 先 Forbidden）、report 冻结门之前；`None/""/纯空白 → Invalid`；规范化后为 String（audit after_data reason、approval.decision_reason 落规范化值）。

### 2.4 GAP-33 publish confirmed_at 显式捕获（两域 publish.rs）

publish_inner 在 ensure_formal_authorized 通过后、BEGIN 前捕获 `confirmed_at=now()`（发布确认决策时刻），cognition_version INSERT 显式写 confirmed_at；created_at 维持插入默认承载发布生效时间。两语义不再共用同一默认值（D7.1 字段级分离；pc 0001:562/projc 0025 触发器已保护 confirmed_at 不可变，零 DDL 变更）。

## 三、测试面

| # | 用例 | 验收 |
|---|---|---|
| T-1 | expire 生命周期（pc+projc）：submitted→expired（列值+审计+receipt）；同态再 expire 409；expired 后 decide 409；pending 行保留原状 | GAP-36 |
| T-2 | reason 契约：None/空/纯空白→Invalid；trim 规范化落列 | D2.2/GAP-36 |
| T-3 | 权限门：AI→Forbidden；无 A3 真人→Forbidden（两域） | D2.1/Q-12 |
| T-4 | 并发契约：decide 终态后 expire 409（D4.10 保证 5） | §D4.10 |
| T-5 | GAP-05：None/空→Invalid（decide 两域；既有用例按必填契约补 reason） | GAP-05 |
| T-6 | GAP-33：publish 后 confirmed_at 非空且 ≤ created_at（决策时刻不晚于生效承载） | D7.1 |
| 回归 | pm-application 全量 + pm-api analysis_reports/pc_proposals；临时库 0001~0031 全序 | 迁移全序 |

既有用例改造（冻结语义收敛，非弱化）：approve() 助手与 `decision(x, None)`/`reason: None` 成功路径调用点补规范化 reason；pm-api p5 跨态断言体补 reason（Batch 2 的 409 断言在 GAP-05 落地后需携带 reason 才到达状态守卫）。

## 四、回滚

Migration 0031=DROP COLUMN（回滚前先核列存在）；服务/测试=git revert。失败即停报告。

## 五、零实施声明（本任务书交付时点）

任务书与 Migration 草案随统一授权直接生效；交付时点未执行任何 SQL/代码修改（实施与验证同批完成并另行 commit）。
