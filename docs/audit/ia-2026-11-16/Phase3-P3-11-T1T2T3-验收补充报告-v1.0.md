# P3-11 T-1/T-2/T-3 验收补充报告 V1.0

- 日期：2026-11-17 · 基准：master `e9f66ec`（含 391c57e → ea9aafb → 阶段二）· 性质：验收补充证据（不改变实施范围、不改变裁决与契约）
- 结论口径：**逐项 PASS/FAIL/BLOCKED/NOT RUN**；本文不宣布验收通过、不冻结、不 push——最终验收与冻结由用户裁决
- 环境修正（重要）：交付报告 V1.0 §6.5「本机无 node 运行时」经复查**不成立**——node v26.13.1 位于
  `C:\Users\Administrator\AppData\Local\Author Software\nvm\.nodejs`（ZCode shell 默认 PATH 未含，补 PATH 后全部前端门禁与 viz 测试可执行并已补跑；pm-infrastructure 因此**实际全绿**，此前 3 例 viz 失败纯环境因素）

## 1. 前端验证

| 项 | 状态 | 命令与实际结果 |
|---|---|---|
| AnalysisReportDetailView vitest | **PASS** | `pnpm exec vitest run src/views/project/analysis/AnalysisReportDetailView.test.ts src/views/project/analysis/AnalysisHomeView.test.ts` → `Test Files 2 passed (2)`、**`Tests 38 passed (38)`**（Detail 26 + Home 12） |
| ProjC 两个新增展示区域 | **PASS** | 用例 `P3-11：ProjC 提案/修订列表渲染（技术认知徽标 + 只读，无决策按钮）`：断言「ProjC 技术认知提案（1）/技术认知徽标/ProjC 对象修订（1）/技术栈（rust）」渲染、决策按钮数=0；六节标题断言扩展为含 ProjC 两节的八节 |
| Close 回执计数 | **PASS** | 既有 closeReceipt 用例全绿（applied/withdrawn/rejected/锚点）；`doClose` 追加逻辑=仅当 projc 三计数任一非零时追加 `closeReceiptProjc` 句（zh/en 双语键已入 locales/analysis.yaml） |
| 国际化 | **PASS** | `check:i18n` 门禁体系未受影响；新增 7 键（sectProjcProposals/techBadge/noProjcProposals/sectProjcRevisions/noProjcRevisions/closeReceiptProjc）zh-CN 与 en 成对落盘 |
| 旧数据兼容 | **PASS** | 既有 24 个 Detail 用例零修改通过（夹具补 `projc_source_id/projc_pending_proposals/total_pending_proposals/projc_proposals/projc_revisions` 五个加法字段后其余断言原样；旧数据=新增数组恒 `[]` 渲染为空态文案） |
| TypeScript 类型检查 | **PASS** | `pnpm typecheck`（vue-tsc --noEmit --skipLibCheck）→ **exit 0** |
| 前端生产构建 | **PASS** | `pnpm build`（release-guard 挂载）→ **exit 0**，`dist/index.html` 产出 |
| dist mock 门禁（AGENTS 规则 4） | **PASS（附登记）** | release-guard（模块级：chunk.moduleIds 匹配 `fixtures/demo/src/api/mock`）0 命中=无 mock 模块代码；运行时符号（mockProjects/mockDelay 等）0 命中。**登记**：`dist/static/js/index-*.js` 含 2 处 **i18n 文案字符串**（`locales/pm.yaml:586/588`，历史设置页提示语提及 `src/api/mock/fixtures.ts` 路径）——非模块、非数据，属文案级残留；最小清理=一行 locale 修订，交验收裁量（本次未动） |

## 2. 数据库迁移验证（实际执行，非静态检查）

方法：无 sqlx-cli，采用 `docker exec -i psql -v ON_ERROR_STOP=1` 逐文件顺序执行 `migrations/*.sql`（每文件独立事务，失败即中止）。

| 项 | 状态 | 实际结果 |
|---|---|---|
| 干净库完整迁移 | **PASS** | 新建 `aipm_mig_full` → **0001→0027 全部 27 文件逐个 OK**（输出逐文件 `OK <name>`，无失败） |
| 阶段一库升级至阶段二 | **PASS** | 新建 `aipm_mig_s1` → 0001→0026 应用成功 → 快照字典四类 `active` → 应用 0027 → 四类全部 `deprecated`（repository/tech_stack/delivery_activity/module）；identity 链 `product_profile/product_vision/feature` 保持 `active` 不受波及 |
| 0026/0027 新增约束、索引、触发器正确 | **PASS** | 探针 P1–P6 全过（见下）；迁移输出无错误 |
| PC snapshot 废弃路径 fail-closed | **PASS（服务层）+ 如实分层** | 服务层：`pc::load_object_type` 仅取 `status='active'`（pc/mod.rs:217-230），集成测试 `snapshot_dictionary_deprecated_fails_closed` 断言四类型全部 `Invalid` 拒绝、`product_profile` 正常。**数据库层无阻断**（探针 P7：直插 `pc.cognition_object` object_type='repository' 成功落库——object_type 无 FK/CHECK 绑定字典）。fail-closed 依据=V-6 服务层唯一写面纪律，DB 硬化属 P3-D04 候选项（见 REG 核查报告 §REG-03） |
| ProjC 正式发布链数据库约束有效 | **PASS** | 探针 P2（账本 `write_operation UNIQUE(tenant,project,operation_id)` 拒重复幂等键）、P3（`trg_projc_proposal_member_anchored` 拒 submitted 提案追加成员）、P4（`trg_projc_vm_closure` 端点闭合触发器拒缺端点的关系成员）、P5（`uq_projc_vm_relationship` 拒重复关系钉扎）、P6（对象终态守卫拒 superseded 无后继）、P1（`relationship_type_owner_domain_check` 拒 owner_domain='pc'）——全部命中预期错误 |

探针明细（全部真实执行，错误码/消息逐条命中）：

```
P1 owner_domain CHECK rejects 'pc'                  → owner_domain_check        PASS
P2 write_operation ledger UNIQUE rejects dup op_id  → duplicate key             PASS
P3 anchored trigger rejects member on submitted     → members are anchored      PASS
P4 closure trigger rejects missing endpoints        → endpoints missing         PASS
P5 uq_projc_vm_relationship rejects duplicate pin   → duplicate key             PASS
P6 terminal guard rejects superseded w/o successor  → must reference its successor PASS
P7 direct SQL insert w/ deprecated type SUCCEEDS    → 落库 repository/draft（如实登记：无 DB 层阻断）
```

## 3. G-1 / G-2 契约核查（实现代码 + 数据库约束 + 测试三向证据）

复跑捕获：`cargo test -p pm-api --test analysis_reports` → **11 passed; 0 failed**（逐用例名实录于下）。

| 核查项 | 状态 | 证据三角 |
|---|---|---|
| G-1 双源登记 | **PASS** | 实现=`analysis_report.rs::mark_analyzed` 单事务（行锁→kb_revision→**projc 源反查**→pc 源反查→四写合一）；DB=0026 `ALTER TABLE integration.repo_analysis_report ADD projc_source_id`；测试=`mark_analyzed_registers_dual_sources`（断言 pc_src+projc_src 同事务落列）+`mark_analyzed_derives_links_and_failed_is_terminal`（G-1 语义扩展后仍绿） |
| projc_source_id 必达（硬失败） | **PASS** | 实现=projc 源缺失→不推进 analyzed + 审计 `analysis_report.analyze_gate_failed` 同事务落库（`append_audit_with_after`，pm_write/audit.rs:70）；测试=`mark_analyzed_hard_fails_without_projc_source`（断言状态保持 analyzing、projc_source_id NULL、审计行≥1） |
| Legacy NULL 分支 + 审计留痕 | **PASS** | 实现=close_report `match projc_source_id { None => projc_legacy_no_source=true }` → payload["projc_branch"]="legacy_no_source"；测试=`close_legacy_report_without_projc_source_writes_audit_marker`（断言审计键精确值、projc 计数=0） |
| G-2 双域发布 | **PASS** | 实现=close_report ProjC 追加块（契约核查 §2 diff 形态：位于 pc withdraw 之后、KB rejected 之前，pc 行零修改）调 `projc::publish_version(operation_id=projc-decision:{id})`；DB=projc.cognition_version+version_member（探针 P4/P5 闭合/唯一）；测试=`close_disposes_both_domains_with_independent_counts`（pc_versions=1 ∧ projc_versions=1，applied/withdrawn/projc_applied/projc_withdrawn=1/1/1/1） |
| 独立失败计数 | **PASS** | 实现=CAS 失配由发布事务内置 failed（projc/publish.rs 与 pc 同构）+ close 独立计数 `projc_publish_failed_proposals`；测试=`close_projc_publish_failure_is_independent_and_non_blocking`（pc applied=1 与 projc failed=1 并存、status=closed、提案=failed） |
| ProjC 发布失败不阻断 close | **PASS** | 同上用例：报告照常 closed、KB rejected、门释放；计数落 CloseOutcome |
| close 主事务原子性 | **PASS（附注）** | 实现=close_report 单事务承载双域撤回+KB 处置+closed+锚点+审计（发布事务自带独立事务、账本可审计）；测试=`close_disposes_both_domains_with_independent_counts` 联合提交断言 + 重复 close 409。**附注**：无独立「事务中段故障注入」用例，原子性以单事务结构+联合提交证据支撑 |
| PC 原有 close 分支语义不变 | **PASS** | 实现=pc 块逐行保留（C-4~C-8/C-12 契约）；测试=既有 4 用例零修改全绿：`analyze_acceptance_creates_report_and_gate_blocks`/`close_disposes_pending_items_and_releases_gate`（:559 approved→applied 核心契约证据）/`list_and_detail_are_project_scoped`/`identity_synthesis_gates` |
| 重复 close | **PASS** | 测试=`close_disposes_both_domains_with_independent_counts` 尾段（第二次 close → `AnalysisReportError::Conflict`）+ 既有用例 409 断言 |
| 幂等重放 | **PASS** | DB=P2 探针（账本唯一约束）；实现=publish_inner ON CONFLICT + completed 短路回 receipt；测试=`projc_formal_chain::publish_replays_receipt_and_cas_mismatch_fails_proposal`（同 operation_id → replayed=true 同 version_id，账本/版本各 1 行；CAS 失配 → 409+提案 failed+`projc_version.publish_failed` 审计≥1） |

复跑实录（11 用例名）：

```
test mark_analyzed_registers_dual_sources ... ok
test mark_analyzed_hard_fails_without_projc_source ... ok
test mark_analyzed_derives_links_and_failed_is_terminal ... ok
test close_disposes_both_domains_with_independent_counts ... ok
test close_projc_publish_failure_is_independent_and_non_blocking ... ok
test close_legacy_report_without_projc_source_writes_audit_marker ... ok
test close_compat_old_consumers ... ok
test analyze_acceptance_creates_report_and_gate_blocks ... ok
test close_disposes_pending_items_and_releases_gate ... ok
test list_and_detail_are_project_scoped ... ok
test identity_synthesis_gates ... ok
test result: ok. 11 passed; 0 failed
```

配套：`cargo test -p pm-infrastructure --test viz_seecode_adapter`（补 PATH 后）→ **7 passed; 0 failed**——pm-infrastructure 全量无环境性失败。

## 4. Git 与交付完整性

| 项 | 状态 | 证据 |
|---|---|---|
| 提交链 | **PASS（附更正）** | `git log --format="%h parent=%p"`：`391c57e(parent=b0d8973) ← ea9aafb(parent=391c57e) ← e9f66ec(parent=ea9aafb)`。**更正**：任务书所引 `0785f97` 已在 push 前 amend 为 `e9f66ec`（`git reflog`：`e9f66ec HEAD@{0}: commit (amend) ← 0785f97 HEAD@{1}: commit`；差异仅=交付报告提交号引用修正，树内容一致）；下文凡引阶段二提交均以 **e9f66ec** 为准 |
| 工作树无未交付改动 | **PASS** | `git status`：仅 `ai-frontend/…design.yaml/design.ts/design.test.ts/DesignLayout.vue/DesignNavV2View.vue/nav-v2/`（用户在途改动，本次未触碰）；无其他未交付文件 |
| 迁移/测试/文档齐备 | **PASS** | 阶段一 12 文件（0026+服务层五文件+事件+handlers+测试）；阶段二 23 文件（0027+W4~W8+前端+addendum V1.2+台账 §10.3+交付报告）；均入提交 |
| 不含本机环境配置与敏感凭证 | **PASS** | `git diff 391c57e~1..e9f66ec` 扫描私钥块/AKIA/ghp_/password= 模式 → 0 命中；仅 1 处 `aipm:aipm@127.0.0.1:16433` = AGENTS.md 规则 3 已声明的本地开发库约定（非泄漏）。测试夹具 `apps/pm-api/tests/keys/*.pem` 为既有仓库内 JWT 测试键（本批未新增） |
| 约束遵守 | **PASS** | 未 push（origin/master 仍止于 65ef3af）；未自行宣布验收/冻结；未改裁决；未删改任何测试断言（本报告所有测试均原样通过） |

## 5. 汇总

| # | 项 | 状态 |
|---|---|---|
| 1.1 | 两视图 vitest（38 用例，含 ProjC 新区/回执/i18n/旧数据兼容） | PASS |
| 1.2 | vue-tsc 类型检查 | PASS |
| 1.3 | 生产构建 + release-guard | PASS |
| 1.4 | dist mock 门禁 | PASS（文案级残留登记） |
| 2.1 | 干净库 0001→0027 全量迁移 | PASS |
| 2.2 | 阶段一→阶段二升级 | PASS |
| 2.3 | 约束/索引/触发器探针 P1–P6 | PASS |
| 2.4 | PC snapshot fail-closed | PASS（服务层；DB 层无阻断如实登记） |
| 2.5 | ProjC 发布链 DB 约束 | PASS |
| 3.1–3.10 | G-1/G-2 十项契约 | PASS |
| 4.1–4.5 | git/交付完整性 | PASS（0785f97→e9f66ec amend 更正） |

**交付方结论：开发交付基本完整，补充验证全绿（无一 FAIL/BLOCKED）；P3-11 处于「待验收」状态，验收与冻结由用户裁决。**
