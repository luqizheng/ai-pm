# P3-11 T-1/T-2/T-3 交付报告 V1.0

- 日期：2026-11-17 · 基准：master `ea9aafb`（阶段一）+ 阶段二切换批（本报告对应的工作树提交）
- 依据：《Phase3-P3-11下一阶段实施方案-T1T2T3-v1.0.md》（V1.1，§4 验收矩阵/§8 四项闭合）+《Phase3-P3-11-close契约影响核查-v1.0.md》（12 条契约）+《Phase3-裁决稿-…-v1.1.md》（R-01~R-10）+ 放行意见（§8.1/§8.4=编码前门槛，测试断言先于实现编写）
- 状态：**开发完成，待验收**（放行≠验收/冻结；不自行标记冻结；不 push——push 由用户确认）
- 环境：真实 DB `postgres://aipm:aipm@127.0.0.1:16433/aipm`（127.0.0.1，AGENTS.md 规则 3）；本机补建 `aipm-dev-postgres` 容器（16433:5432，postgres:16，max_connections=400）

## 1. 交付物与提交

| 阶段 | 提交 | 内容 |
|---|---|---|
| 前置 | `391c57e` test: align stale assertions | 两处存量 master 失败对齐（0023 守卫白名单 / DEF-CHAIN-02 NotFound），P3-11 全量门禁前置 |
| 阶段一 | `ea9aafb` feat(projc) 阶段一：纯增量 | 迁移 0026 + projc 服务层镜像五文件 + 事件目录 16 项 + 留痕 handlers + projc_formal_chain 6/6；现网零行为变化（新表零生产写入者） |
| 阶段二 | `0785f97` feat(projc) 阶段二：切换批 | W4 worker 重挂 / G-1 双源 / G-2 close 双域分流 / W6 并行端点+前端 / W7 迁移 0027 / W8 矩阵 |

## 2. 两道编码前门槛（G-1 / G-2）

**G-1 projc_source_id 完整性（方案 §8.1）**：

- 双源登记写入点唯一：`mark_analyzed` 单事务完成报告行锁 → kb_revision 解析 → **projc 源反查（必达）** → pc 源反查（identity 通道，口径不变）→ 状态推进四写合一；`ensure_for_intake` 维持受理建行语义（受理时点尚无分析产物、无源可登记——登记全部发生在 analyzed 门槛事务内）。
- **硬失败**：projc 源缺失 → 报告不得进入 analyzed（保持 analyzing），审计事件 `analysis_report.analyze_gate_failed` 同事务落库（不静默）；测试 `mark_analyzed_hard_fails_without_projc_source`（apps/pm-api/tests/analysis_reports.rs）先于实现编写。
- NULL = 显式 Legacy：close 对 `projc_source_id IS NULL` 跳过 ProjC 分支并写审计键 `projc_branch:"legacy_no_source"`；测试 `close_legacy_report_without_projc_source_writes_audit_marker`。
- 双源正路径：`mark_analyzed_registers_dual_sources` + `mark_analyzed_derives_links_and_failed_is_terminal`（G-1 语义扩展）。

**G-2 close 失败语义（方案 §8.4，逐行对照）**：

| §8.4 情形 | 实现与测试 |
|---|---|
| 发布成功 | pc `pc-decision:{id}`（C-4 分支逐行不动）；projc `projc-decision:{id}` → projc.cognition_version；`close_disposes_both_domains_with_independent_counts` 断言双域各 1 版本、applied/withdrawn/projc_applied/projc_withdrawn 独立计数 |
| 发布失败（CAS 失配等） | 发布事务同构内置 failed；**独立计数** `projc_publish_failed_proposals`，不阻断关闭；`close_projc_publish_failure_is_independent_and_non_blocking`（pc applied=1 与 projc failed=1 并存、报告照常 closed） |
| 重试 | 提案终态 failed；报告已 closed 不可再关（重复 close 409 断言同用例） |
| 重复 close | 状态门共用（409），双域一致 |
| 幂等重放 | projc 账本唯一约束 + 同 operation_id 重放返回既有 receipt（`projc_formal_chain::publish_replays_receipt_and_cas_mismatch_fails_proposal`：同 receipt + 账本/版本单行）；close 侧同键天然幂等 |
| 事务边界 | close 主事务（双域撤回 + KB 处置 + closed + 锚点）任一结构失败整体回滚；发布事务自带独立事务、账本+审计可追溯 |

登记偏差：契约核查 C-9/C-10 写「加法 2 键/字段」，G-2 闭合项四要求失败独立计数 → 实落 **3 键/字段**（projc_applied / projc_withdrawn / projc_publish_failed_proposals）；新增键向后兼容性质不变（close_compat_old_consumers 锁旧 7 字段/旧 6 键逐一存在）。

## 3. T-3 验收矩阵逐条对照（方案 §4）

| 完成标准 | 验证方式与结果 |
|---|---|
| 技术栈/仓库结构/Repository Module 进 ProjC | `kb_pc_repo_flows::verify_triggers_full_kb_pc_chain`（真实 git 仓库全 worker 链）：projc.cognition_object 4 行（repository+tech_stack+2×repository_module，全 draft）、evidence 4、pc.cognition_object snapshot 类 **0 行**；`projc_derive_switch::derive_routes_technical_cognition_to_projc`（服务层直调）：3 类有行 + derived 归因不变 ✅ |
| 产品能力/规则/Product Module 进 PC | identity 链零改动证据：`analysis_reports::identity_synthesis_gates` 绿；pc.source 仍随分析登记（G-1 双源之一，derive 断言 pc_sources=1）；W7 后 product_* 字典仍 active（`snapshot_dictionary_deprecated_fails_closed`）✅ |
| Finding 仅候选 | `projc_formal_chain` 全生命周期（draft→submitted→approved→publish→applied 才有版本）；`projc_derive_switch` 断言 derive+propose 后 cognition_version 0 行 ✅ |
| 技术认知不能发布到 PC | W7 字典废弃 fail-closed：pc::upsert_object_draft 四类 snapshot 全部 Invalid；worker 重挂后 pc 侧无 snapshot 提案起草者；close pc 分支无 snapshot 提案可发布（双域分流测试 pc applied 仅来自 identity/产品提案）✅ |
| 产品认知不能发布到 ProjC | projc 独立字典拒 product_*：`projc_model::object_draft_upsert_idempotent_and_content_addressed`（product_profile → Invalid，Stage 1 延续）✅ |
| 未经人工确认不正式发布 | AI/服务主体 approve/publish 403：`projc_formal_chain::formal_writes_fail_closed_for_ai_and_unauthorized`（含非授权真人 403 + approver_grant 放行 + 同态重放幂等）✅ |
| 越权/跨域/版本冲突/重复提交 | SEC-P2-2 探测（projc_model）；CAS 409 + 提案内置 failed（publish_replays…）；重复 close 409；关系活跃唯一 Conflict ✅ |
| 旧 snapshot 路径不能绕过 | 字典废弃守卫（上两行）+ close 分流断言 + `kb_pc_repo_flows::confirmation_loop…` 断言 `:activity` 不再入链（R-05）✅ |
| Phase 2 close 契约回归 | `apps/pm-api/tests/analysis_reports.rs` **11/11**（含 `close_disposes_pending_items_and_releases_gate` 核心契约证据）+ identity_synthesis_gates ✅ |

回归范围实测（全部真实 DB）：

- `pm-application` 全量：**74 测试二进制 / 800 passed / 0 failed**（阶段一与阶段二切换后各跑一轮，均全绿）
- `pc_model` 6/6 + `pc_proposal`（pm-application）8/8 + `projc_model` 3/3 + `projc_formal_chain` 6/6
- `pm-infrastructure` 全量：**27 二进制绿 / 143 passed**（含 `kb_pc_repo_flows` 7/7 双域口径改写、`intake_flows::handler_assembly_covers_catalog` 目录+2、`projc_derive_switch` 2/2）；`viz_seecode_adapter` 3 例 = SeeCode CLI 需 spawn node，**本机无 node 运行时**（与本批无关的环境限制）
- `apps/pm-api` 全量：**73 测试二进制 / 486 passed / 0 failed**（含 `analysis_reports` 11/11、`pc_proposals` 6/6、`repositories` 12/12）
- 前端 vitest：**无法执行——本机无 node 运行时**（vitest/SeeCode/前端工具链同根因）；改动为类型加法 + 新增节渲染 + 夹具补字段，详见 §6 风险

## 4. 集成点落地对照（方案 §1 I-1~I-8）

| # | 落点 | 结果 |
|---|---|---|
| I-1 | kb_pc_analysis 派生 | `plan_cognition_objects` 停派 delivery_activity、module→repository_module；`derive_projc_cognition`（自 derive_pc_cognition 改名）全链 projc（register_source×2 双源/upsert/register_evidence/propose）；LLM D-1/D-2/D-3 归因语义逐字保留 |
| I-2 | 提案起草 | projc::propose_from_analysis（origin_kind='repo_analysis'、advisory `projc_analysis_proposal:`、outbox `projc.change_proposed`、候选剔除口径=版本已含/有效提案覆盖/rejected 阻断/failed-expired 放行）；pc 版保留（identity/服务通道） |
| I-3 | 身份综合 | identity.rs 零 diff；pc_source_id 通道保持 |
| I-4 | close 分流 | 契约核查 §2 diff 形态逐字落地（追加块位于 pc withdraw 之后、KB rejected 之前；既有行零修改） |
| I-5 | 详情聚合 | +projc_proposals/projc_revisions（恒数组）+projc_pending_proposals/total_pending_proposals（§8.2 口径：既有 pending_proposals 零改动） |
| I-6 | P5 决策路由 | `POST /v1/pm/projc/proposals/{id}/decide`（p5 同构：可信交互上下文 + decide 暂记 + CAS） |
| I-7 | API 回归 | analysis_reports 11/11 全绿 |
| I-8 | 关系域约束 | projc.relationship_type 独立建表（owner_domain CHECK IN ('project_cognition')），pc 0007 约束零改动；跨域 ext 键走 0007 `domain.type` 先例（implements 目标端 pc.feature_module） |

## 5. 文档交付

1. 注册表附录升 V1.2：`docs/architecture/four-domain/object-registry-v1.1-projc-addendum.md`（projc 七对象全 COMPLETE、pc snapshot=DEPRECATED 字典、双源登记）。
2. 台账 §10.3：`Phase3-基线校准与裁决台账-v1.0.md`。
3. 本报告（差异登记随附）。

## 6. 差异与风险登记

1. **契约核查 C-9/C-10 加法 2 键 → 实落 3 键**（G-2 独立失败计数），见 §2；向后兼容不变。
2. **prior_cognition_digest / NO_CHANGE result_ref 随全链切换改读 projc**：增量合并输入与任务 result_ref 锚 projc.source（target_domain=project_cognition）；P3 `/v1/pc/analyses/{id}` 的 source 分支对 projc ref 返回任务面（无对象展开），ProjC 展示统一走报告详情 projc_* 字段（W6）。
3. **0025 两处缺陷修正（随 0026）**：`enforce_proposal_member_anchored` 误绑 BEFORE（RETURN NULL 静默吞掉全部成员插入）→ 按 pc 真源 0001:6994/7001 重绑 AFTER 约束触发器；`projc.write_operation` 缺账本幂等唯一约束 → 补（0001:5746 同款）。阶段一交付时发现并修正，属实现缺陷修复（非裁决变更）。
4. **存量测试对齐五处**（均系先前批次变更未同步断言，非本批改动引发；`391c57e` 单独提交两处，其余随本批）：0023 守卫白名单（kb_revision_confirm）、DEF-CHAIN-02 NotFound（repo_facts）、审阅流 v2 暂记语义（apps/pm-api pc_proposals 两用例——91003f6 未同步「approve=审批+发布」旧断言）、D-5 报告门先于幂等重放（repositories::manual_analyze_http_contract——c50bc1c 起门先于重放，测试未同步）；另 `intake_flows::handler_assembly_covers_catalog` 目录断言 +2、W7 字典废弃的夹具类型机械适配（pc_model/pc_proposal/pc_proposals 中 snapshot 类型 → 活跃类型，断言意图不变）。
5. **前端 vitest 未执行**：本机无 node 运行时（vitest 与 viz_seecode_adapter 同根因）。前端改动面：`api/analysis-reports.ts` 类型加法；`AnalysisReportDetailView.vue` 追加 ProjC 两节（技术认知徽标+只读列表）+ 关闭回执 ProjC 计数追加；两测试夹具补新字段 + 标题断言扩展 + 新增 ProjC 渲染用例；locale zh-CN/en analysis.yaml 加键。待有 node 环境补跑 `vitest AnalysisReportDetailView AnalysisHomeView`。
6. **W1 计划 vs 实落**：迁移 0026/0027 拆两份（阶段一/阶段二各自提交，迁移号全分支扫描确认未占用）；「W1~W8 单批合并」按 §8.5 分阶段结构调整执行（放行意见授权）。

## 7. 完成标准逐条结论

| 完成标准 | 结论 |
|---|---|
| 仓库分析结果按业务含义进入正确领域 | ✅（§3 行 1-2） |
| ProjC 独立完整可审计的正式发布链 | ✅（proposal→approval→publish→ledger/audit/outbox 全链测试） |
| 技术认知不能发布到 PC；产品认知不能发布到 ProjC | ✅（字典废弃 fail-closed + 独立字典互不可见） |
| Finding 仅候选 | ✅（无 approve 不出版本行） |
| 未经人工确认不正式发布 | ✅（真人+A3 门，AI 403） |
| 越权/跨域无权限/版本冲突/重复提交正确处置 | ✅（403/404/409/幂等重放） |
| 旧 snapshot 路径不能绕过新规则 | ✅（字典废弃+重挂+分流） |
| Phase 2 close 接口及既有契约回归通过 | ✅（analysis_reports 11/11 含既有 4 用例零修改） |
| 不依赖历史迁移或双写 | ✅（AGENTS.md 规则 1：无回填/无兼容分支；Legacy 行仅审计留痕跳过） |
