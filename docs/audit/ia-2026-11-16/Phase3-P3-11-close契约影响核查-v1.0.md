# P3-11 close 契约影响核查 V1.0（实施前置门）

- 日期：2026-11-16 · 基准：master `965e99cf`（方案 V1.0 + 四项范围确认）
- 目的：**在写任何实现代码之前**，逐条核对 P3-11 T-1/T-2/T-3 改动对 Phase 2 close 契约的影响，确认零破坏
- 结论：**契约零破坏**——12 条契约条款中 10 条零改动、2 条纯加法扩展（响应体新增字段，向后兼容）；pc 分支代码逐行保留

---

## 1. Phase 2 close 契约清单与逐条影响

| # | 契约条款 | 现行依据（实证） | 本批改动 | 影响 |
|---|---|---|---|---|
| C-1 | 端点与响应签名：`POST /v1/pm/analysis-reports/{id}/close` → `Json<CloseOutcome>` | `apps/pm-api/src/analysis_reports.rs:101-112` | handler 不动 | **不变** |
| C-2 | 主体门：仅 interactive 真人会话可关（`ensure_closable`） | `analysis_report.rs:592-598` | 不动 | **不变** |
| C-3 | 状态机门：仅 `analyzed` 可关，否则 409 Conflict | `close_report` :415-423 | 不动 | **不变** |
| C-4 | approved pc 提案批量发布：`pc-decision:{id}` 幂等键、CAS 失配内置 failed、失败计数不阻断关闭 | :437-466 | **pc 分支逐行保留** | **不变** |
| C-5 | 未决 pc 提案（draft/submitted）→ withdrawn + 逐条审计 | :468-484 | 不动 | **不变** |
| C-6 | KB 草稿修订 → rejected + 审计 | :487-504 | 不动 | **不变** |
| C-7 | 报告置 closed（closed_by/closed_at）+ 门释放（再次分析 409 report_open 解除） | :507-512 | 不动 | **不变** |
| C-8 | D-1 锚点固化：`project_repository.last_analyzed_commit_sha` + revision 乐观锁 | :524-531 | 不动 | **不变** |
| C-9 | 审计事件 `analysis_report.closed`（payload=处置计数） | :514-523 | payload **加法**：+projc_applied/projc_withdrawn 两键 | 加法扩展 |
| C-10 | 响应体 `CloseOutcome` | :94-105；前端 `api/analysis-reports.ts:76-83` CloseReportOutcome | struct **加法** 2 字段；TS 接口加 2 可选字段（JSON 新增字段对既有消费端向后兼容） | 加法扩展 |
| C-11 | 详情聚合 `ReportDetail{header,kb_revision,proposals,pc_revisions}` | `report_detail` :330-378；TS :70-74 | **加法** 2 数组：projc_proposals / projc_revisions（by 新列 projc_source_id） | 加法扩展 |
| C-12 | 列表未决计数 `pending_proposals`（pc draft/submitted 子查询） | `list_reports` :296-301 | 不动（projc 未决数为详情聚合新增字段承载，不并入既有计数语义） | **不变** |

## 2. close_report 改动的 diff 级形态（W5）

```text
close_report(pool, project_id, report_id, actor):
  ... 现行前置（边界/行锁/状态门 C-2~C-3）——零改动 ...
  ... C-4 pc 批量发布块 ——零改动 ...
  ... C-5 pc withdraw 块 ——零改动 ...
+ // ── P3-11 新增：ProjC 分流（纯追加块）──
+ if let Some(projc_source_id) = projc_source_id {          // 新列，0026 迁移
+     approved projc.proposals → projc::publish_version(   // 与 pc 同构：真人门+CAS+幂等
+         operation_id = format!("projc-decision:{id}"))    // 失败计数不阻断（C-4 同口径）
+     pending projc.proposals → withdrawn（逐条审计）
+ }
  ... C-6 KB rejected 块 ——零改动 ...
  ... C-7/C-8 closed + D-1 锚 ——零改动 ...
  audit payload + CloseOutcome：+2 计数字段（C-9/C-10）
```

要点：**新增块位于 pc withdraw 之后、KB rejected 之前；既有行零修改**（仅 struct 定义与返回字面量加字段）。pc 分支在 projc_source_id 缺失（旧报告行）时天然短路，行为与今日完全一致。

## 3. 回归门（实施完成的必要条件，非充分替代）

| 回归项 | 用例 | 通过标准 |
|---|---|---|
| close 契约全链 | `apps/pm-api/tests/analysis_reports.rs::close_disposes_pending_items_and_releases_gate`(:420) | 全绿（C-4~C-8/C-12 行为断言，含 :559 "approved proposal published at close"） |
| 门与终态 | 同文件 `analyze_acceptance_creates_report_and_gate_blocks`(:235)、`mark_analyzed_derives_links_and_failed_is_terminal`(:288) | 全绿 |
| 项目隔离 | 同文件 `list_and_detail_are_project_scoped`(:621) | 全绿 |
| 身份综合门 | 同文件 `identity_synthesis_gates`(:700) | 全绿（PC 通道不变证据） |
| 服务层回归 | pc_model 6 / pc_proposal 8 / projc_model 3（Stage 1 既有） | 全绿 |
| 新增矩阵 | `projc_formal_chain.rs`（T-3 验收矩阵，方案 §4） | 全绿 |

## 4. 与 Phase 2 任务包的关系（依赖确认）

1. **P2-01~08 尚未落地 master**（前置核查结论维持）：无文件冲突面；本批触碰 `analysis_report.rs`/`knowledge_services.rs`，P2 落地时若同文件需 rebase——已在 P2 协调注记登记。
2. **E2E-02（P2 任务包测试项）尚未编写**：不存在可破坏的既有 E2E；其未来断言应按**双域分流口径**编写——需对 P2 任务包文档做一行修订（登记为协调事项，非本批代码）。
3. 原子批次依赖链：`W1 迁移 0026 → W2 提案/发布链 → (W3 关系冲突 / W4 worker 切换 / W5 close 分流) → W6 审阅路由 → W7 pc 字典收敛 → W8 测试矩阵`；关键路径=W2（行为等价 mirror 量最大）；**W1~W8 单批合并，合并前 §3 全绿**。

## 5. 回滚策略

单 commit 序列（0026 迁移 + 代码同 revert）：迁移为纯 DDL+种子 UPDATE，`sqlx migrate revert`（或手工 DROP schema projc 内新增对象 + 还原 4 行字典 status + DROP COLUMN projc_source_id）可完全回退；pc.* 数据面零写入，无数据残留。

## 6. 结论

**close 契约 12 条：10 条零改动、2 条加法扩展（新增字段向后兼容）；pc 发布分支逐行保留；既有回归用例预期全绿且作为验收门。** 不破坏 Phase 2 close 契约——请求放行实施。
