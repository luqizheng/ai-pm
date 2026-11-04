# S03-00 · 测试与证据清单（Test Evidence）V1.1

> V1.1：收编 R2（SC-05 按 A* 标注 MCP 入口为缺陷）/R4（SC-02 不可变性三层表述）/R8（静态证据与运行验证严格分级）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

基线 @ `3ccb42f`。**本任务未运行任何测试套件**（避免持久化副作用与并行会话干扰）；本文档=既有测试盘点（只读源码统计）+ 八场景验证分级 + 运行时唯一抽查记录。证据格式遵循任务书 §12.2：

```
[状态标签]
Evidence:
  File: <路径>  Symbol: <符号>  Lines: <行号>  Migration: <文件>  Test: <测试>
Conclusion: <当前已实现的具体能力>
Limitation: <尚未实现的部分>
```

---

## 一、既有测试盘点（只读；数量=# [sqlx::test]/#[test]/#[tokio::test]）

| 域 | 测试文件 | 数 | 测什么 |
|---|---|---|---|
| requirement/revision | apps/pm-api/tests/pm_writes.rs | 13 | 草稿追加+hash、定稿晋升+superseded、非 draft 冲突、任务乐观锁 |
| 同上 | crates/pm-application/tests/command_executor.rs | 13 | 定稿晋升+补偿、**AI redline 拒绝**、stale revision 乐观锁冲突 |
| **并发定稿** | — | **0** | 无并发 approve 同一 revision 专项（搜索词 concurrent+revision 全库无） |
| approval 管线(D5) | pm-application/tests/review.rs | 14 | auto_accept/human 门/changes_requested/immutability/审计幂等 |
| supervision | pm-application/tests/supervision.rs | 8 | veto 回滚/补偿、approve 恰一次、freeze/unfreeze |
| decision/知识文档 | pm-application/tests/knowledge_v2.rs | 10 | 自动 commit/红线待确认/**非人拒确认**/veto 回滚/索引回退 |
| 文档 CRUD | apps/pm-api/tests/documents.rs+doc_write.rs | 7+3 | CRUD 回路/重复创建不加 commit/漂移 stale/项目隔离 |
| Goal(I4) | tests/project_goal.rs+project_goal_archive.rs | 3+5 | 强制红线+人工确认、stale base、归档竞争永不发布 |
| task_requirement | tests/bootstrap_read.rs+command_batch.rs | 5+4 | 读取映射（latest revision wins）+行数断言；**无独立守卫套件** |
| MCP 权限门 | apps/pm-api/tests/mcp_permission.rs | 9 | READ/DOCUMENT_WRITE 分层 |
| Run 派发 | pm-application/tests/run_dispatch.rs | 24 | 并发领取唯一、租约回收/隔离、**审批证据门 block claim**、r5 不可放宽 |
| audit/outbox | outbox_ops.rs+audit_stream.rs+outbox 系列+audit_actor_coverage.rs | 7+4+40+2 | 事件结构/接线/claim/handler/worker/actor 覆盖 |
| 状态机 | pm-domain/src/task_transition.rs 内联+tests/state_transition_guard.rs | 16+4 | 30 边矩阵/E1/E2/B4/取代/等待守卫 |
| Run/交付 | run_executor/run_context/task_dependency_concurrency/delivery.rs | 11+11+9+24 | 孤儿 reconcile/契约映射/**图锁并发**/D4 逐仓校验 |
| 归档/快照 | project_archive/run_scope_snapshot/repo_access_enforcement/project_delivery/acceptance.rs | 5+4+8+4+3 | I1 归档/**快照哈希回读**/执行期门/交付视图/验收端点 |
| MCP 契约 | pm-mcp/tests/{tools,contract,caller_ctx,hosts,discovery}.rs | 6+6+7+7+6 | 工具契约（**19 工具钉死**）/调用者/Host 白名单 |
| 设计 ingestion | e2e_scenarios.rs | 10 | 提交→分析→提案间接覆盖 |

**无测试背书的缺口**：需求修订并发定稿（0）；task_requirement 独立守卫套件（无）；execution_mode 专项（无）；设计领域端到端（对象不存在，无从测）。

## 二、SC-01~SC-08 场景验证（分级：T=已有测试覆盖｜R=已有运行验证｜S=仅静态代码证据｜N=当前无法验证）

### SC-01 AI 创建 Design Revision，人类批准 → **NOT IMPLEMENTED**
- 无设计实体/版本/批准入口（领域清单 §6；K-1）。
- 最近似已验证能力：知识文档提案+红线人工确认（knowledge_v2.rs 10 例，含"非人拒确认"）→ **T**。
- Limitation：该管线是文档发布语义，非设计批准语义（S03 V1.0 关键约束在代码上的对应事实）。

### SC-02 已批准设计产生新 Revision，旧不被覆盖 → **NOT IMPLEMENTED**（设计）；需求域近似=部分成立
- Evidence（S）：应用层——全库仅 4 处 revision UPDATE 且只改 state/approved_by，未检索到 content UPDATE 路径（executor.rs:892,901；revision.rs:98,105）；UNIQUE(requirement_id,version)（0002:30）。**注意（R4）：该证据不能推导为"数据库禁止正文修改"——数据库层尚无正文不可变保护（无触发器/无约束）；目标要求=应用层与数据库层共同保障。**
- Evidence（T）：pm_writes.rs 定稿晋升+superseded 用例；command_executor.rs 定稿+补偿。
- Limitation：approved 无 DB 级不可变保护；执行器无 state 前置（superseded 可复活，executor.rs:847-907）；并发窗口 TOCTOU（无测试）。

### SC-03 一个设计关联三个需求 → **NOT IMPLEMENTED**（无 DesignRequirement 表）
- 近似：task_requirement 支持一 Task 绑多需求修订（PK 允许多行，0002:68-80）→ **S+T**（bootstrap_read/command_batch 行数断言）。

### SC-04 一个 ADR 关联多设计和需求 → **NOT IMPLEMENTED**
- 无 ADR 实体、无任何关联表（领域清单 §9/§11）。无近似能力。

### SC-05 AI 尝试正式批准设计 → **NOT IMPLEMENTED**（无批准对象）
- 近似事实 A（已有测试）：命令通道红线拒 AI——command_executor.rs「AI redline 拒绝」；knowledge_v2.rs「非人拒确认」→ **T**。
- 近似事实 B（静态，P0）：11 个确认/审批 REST/MCP 入口 api_key（provision 成 kind='human'）均可通过（权限清单 §2 体检表）→ **S**（其中 MCP confirm/veto 入口按 S02-I6-A A* 本身为待修缺陷——A* 目标=MCP 不提供正式确认入口）。
- 结论：系统能拒绝的只有「服务主体」与「命令通道红线自我执行」；"AI 尝试批准必须拒绝"在入口层**当前不成立**。

### SC-06 两人并发批准同一设计 → **NOT IMPLEMENTED**（设计）
- 近似 A（已有测试）：review_approval 并发=行锁+锁后重读+DB 触发器→输家 AlreadyDecided（review.rs immutability/decide 幂等用例）→ **T**。
- 近似 B（静态）：需求定稿 REST TOCTOU（revision.rs:84-120 无 FOR UPDATE/UPDATE 无 state 守卫；并发专项测试数=0）→ **S**，风险开放。

### SC-07 关键 ADR 未解决→Readiness 不通过 → **NOT IMPLEMENTED**
- 无 ADR、无 Gate。最近似门禁：审批证据门 block claim（run_dispatch.rs）→ **T**（机制形态参照，非本场景能力）。

### SC-08 设计批准后新版本，原实施基线不被静默改变 → **NOT IMPLEMENTED**（无基线对象）
- 近似 A（已有测试）：policy_snapshot_hash 每 Run 不可变+快照哈希回读（run_scope_snapshot.rs 4 例）→ **T**。
- 近似 B（静态）：task_requirement.revision_id 锁定于 Task 创建，需求后续定稿不触碰既有行（两条定稿路径均不写 task_requirement；无触发器）→ **S**。

### 分级汇总

| 场景 | 结论 | 最高证据级 |
|---|---|---|
| SC-01 | NOT IMPLEMENTED | T（近似） |
| SC-02 | NOT IMPLEMENTED（设计）/近似部分成立 | S+T |
| SC-03 | NOT IMPLEMENTED | S+T（近似） |
| SC-04 | NOT IMPLEMENTED | — |
| SC-05 | NOT IMPLEMENTED（入口层结论为"不成立"） | T+S |
| SC-06 | NOT IMPLEMENTED（设计）/近似部分成立 | T+S |
| SC-07 | NOT IMPLEMENTED | — |
| SC-08 | NOT IMPLEMENTED（基线）/近似部分成立 | S+T |

**八场景无一可在当前代码上完整走通**；与任务书要求一致，未为验证新增任何业务代码或修改数据库。

## 三、运行时唯一抽查记录（只读）

- 对象：本机演示栈容器 `infrastructure-aipm-pg-1`（POSTGRES_DB=aipm/POSTGRES_USER=aipm）。
- 命令：`psql -U aipm -d aipm -Atc "SELECT count(*), min(version), max(version) FROM _sqlx_migrations;"` → **28 | 1 | 28**。
- 结论：该部署登记表=0001~0028，构建早于 S01 批次（0030~0034），与代码基线脱钩属预期；证明登记机制可用、升级演练另行背书（commit `bcb169f` 迁移 0031 升级演练通过）。
- 未执行任何写操作。

## 四、历史运行验证档案（R 级引用，非本次产出）

- S02-02 运行时核验（2026-11-04，容器 pm-api-s02v@旧 commit）：DEF-01/02/03/04 曾以真实请求复现；本次已在 HEAD `3ccb42f` 静态重验（DEF-02/03 REST/04 成立，见主报告 K-10），**未做运行时复测**。
- S01 全量套件：105 套件/781 passed（feat/s01-implementation 分支记录，commit 时间早于本基线一个批次）；前端 94 文件/569 测试。
