# P3-11 T-1/T-2/T-3 新会话执行提示词 V1.0

> 用法：新会话开工时将 §2 提示词全文粘贴为首条消息。本文件是提示词的存档真源。

## 1. 前置事实（截至 2026-11-16，均已提交 master 并 push）

- P3-11 Stage 1 已合入（ac8eb298）：迁移 0025（projc schema 13 机制表/8 触发器/15 索引/3 字典种子）+ `pm-application::projc` 数据面（object/evidence）+ `tests/projc_model.rs` 3/3 绿；pc 回归 6/6+8/8 绿。
- 方案与裁决（按序必读）：
  1. `docs/audit/ia-2026-11-16/Phase3-P3-11下一阶段实施方案-T1T2T3-v1.0.md`（**V1.1**）——W1~W8、集成点 I-1~I-8（file:line 实证）、§8 四项闭合、§8.5 分阶段结构、放行记录
  2. `docs/audit/ia-2026-11-16/Phase3-P3-11-close契约影响核查-v1.0.md`——close 契约 12 条（10 零改动+2 加法）、diff 形态、回归门
  3. `docs/audit/ia-2026-11-16/Phase3-裁决稿-P3-11仓库分析认知归属与双域治理-v1.1.md`——R-01~R-10
  4. `docs/architecture/four-domain/object-registry-v1.1-projc-addendum.md`
  5. `docs/audit/ia-2026-11-16/Phase3-基线校准与裁决台账-v1.0.md` §9/§10
- 范围确认已锁定：① pc snapshot 字典迁移内 `status='deprecated'`；② P5 决策并行 projc 端点；③ T-3=集成矩阵+API 回归（E2E 不在硬门）；④ 关系种子 implements/depends_on。
- 放行意见（最新）：**可开工；§8.1 projc_source_id 完整性与 §8.4 close 失败语义=编码前最后两道门槛**（先落设计与测试断言，再动其他）；不得以代码行未修改替代业务契约验收；放行≠验收/冻结。

## 2. 执行提示词（粘贴给新会话）

```text
按已放行的《P3-11 T-1/T-2/T-3 实施方案 V1.1》执行开发。开工前按序通读五份文档（清单见
docs/audit/ia-2026-11-16/Phase3-P3-11-T1T2T3-新会话执行提示词-v1.0.md §1），其中方案 V1.1
§8（四项闭合）与 close 契约核查全文为硬约束。

两道编码前门槛（先落这两条，再写其他任何代码）：
G-1 projc_source_id 完整性（方案 §8.1）：mark_analyzed/ensure_for_intake 同事务双源登记
    （pc_source_id + projc_source_id）；projc 源登记失败=报告不得进入 analyzed（硬失败）；
    close 对 NULL projc_source_id 仅按显式 Legacy 处理（跳过 ProjC 分支必须写审计事件
    projc_branch=legacy_no_source，绝不静默）。先写测试断言再写实现。
G-2 close 失败语义（方案 §8.4）：双域发布各自幂等（pc-decision:{id} / projc-decision:{id}）、
    失败各自独立计数不阻断关闭、CAS 失配内置提案 failed、重复 close 409、账本+审计可追溯
    无重复正式版本。先写测试断言再写实现。

阶段一（纯增量，现网零行为变化，独立提交）：
- 迁移 0026_projc_formal_chain.sql（开工先全分支扫号确认 0026 未被占用）：
  projc.relationship_type（列=pc.relationship_type 同构+owner_domain CHECK IN
  ('project_cognition')，0001_baseline.sql:3377 同构）+ 种子 implements（{repository_module}
  →{pc.feature_module}）/depends_on（{repository_module}→{repository_module}）directed L1；
  projc.relationship/relationship_revision（:3293-3369 同构）；projc.conflict(+conflict_object)
  （:3108-3134 同构）；version_member 端点闭合触发器（pc.enforce_member_endpoint_closure
  同款指向 projc 表，0001_baseline.sql:314）；ALTER integration.repo_analysis_report ADD
  COLUMN projc_source_id uuid；**不含** pc 字典废弃（留阶段二）。
- crates/pm-application/src/projc/ 镜像三文件+新增二文件（行为等价，真源=pc 同名文件，
  勿自创语义）：proposal.rs（pc/proposal.rs 1541 行镜像：create/submit/approve/reject/
  withdraw/decide P5 暂记 + propose_from_analysis origin_kind='repo_analysis'、advisory
  'projc_analysis_proposal:'、outbox projc.change_proposed）；publish.rs（pc/publish.rs 725
  行镜像：advisory 'projc_publish:'、账本命令 'projc_publish_version'、outbox
  projc.cognition.version_published、CAS 失配内置 failed+审计）；relationship.rs（pc/
  relationship.rs 421 行镜像）；conflict.rs（create/resolve 人工 + 冻结查询）。mod.rs 补
  ensure_formal_authorized（复用 crate::approver::ensure_governance_authorized）/
  frozen_by_conflict/canonical 哈希/type_set_allows。
- kb-pc-contracts/src/events.rs 加法新增：PROJ_CHANGE_PROPOSED='projc.change_proposed'、
  PROJ_COGNITION_VERSION_PUBLISHED='projc.cognition.version_published'。
- 审计口径：action/source_kind/target_type 全部 projc_ 前缀（projc_proposal.create/…、
  projc_version.publish/publish_failed）。
- 测试 tests/projc_formal_chain.rs：提案全生命周期、AI 主体与非授权人 403、冻结 403、
  发布幂等重放同 receipt、CAS 409+提案 failed、关系端点类型/跨域 ext 键/自引用/活跃唯一、
  conflict 冻结解冻、propose_from_analysis 候选剔除口径（版本已含/有效提案覆盖）。
- 门禁：cargo test -p pm-application（全量集成）+ pc_model/pc_proposal 回归 + cargo check
  -p pm-api。全绿后提交（message 注明「阶段一：纯增量」）。

阶段二（切换批，阶段一全绿后）：
- W4 crates/pm-infrastructure/src/kb_pc_analysis.rs：plan_cognition_objects 停派
  delivery_activity；derive_pc_cognition 全链改挂 projc（register_source/upsert_object_draft/
  register_evidence/propose_from_analysis）；LLM enrichment 语义保留（derived/llm/
  derived_fallback 归因不变）。
- W5 crates/pm-application/src/analysis_report.rs：mark_analyzed/ensure_for_intake 双源登记
  （G-1）；close 双域分流（pc 分支逐行不动，projc 追加块按契约核查 §2 形态；G-2）；report_
  detail/list_report 加 projc_proposals/projc_revisions（恒空数组）/projc_pending_proposals/
  total_pending_proposals；CloseOutcome +projc_applied/projc_withdrawn。
- W6 apps/pm-api：knowledge_services.rs 新增并行端点 POST /v1/pm/projc/proposals/{id}/decide
  （p5_decide_proposal 同构）；analysis_reports.rs DTO 加法；前端
  AnalysisReportsPanel.vue + api/analysis-reports.ts 最小渲染（技术认知徽标+projc 提案/
  修订列表），不扩 Phase 2 UI 范围。
- W7 迁移内 pc.object_type snapshot 四类（repository/tech_stack/delivery_activity/module）
  UPDATE status='deprecated'（load_object_type 即拒=错误域草稿源头不可达；identity 链
  product_* 不受影响）。
- W8 测试：T-3 验收矩阵（方案 §4 逐条）+ close_compat_old_consumers（§8.3：CloseOutcome
  旧 7 字段/审计旧 6 键/详情旧 4 字段逐一存在，空集恒数组）+ close 双域分流与失败语义
  （G-2 表逐行）+ apps/pm-api/tests/analysis_reports.rs 全部用例全绿（close_disposes_
  pending_items_and_releases_gate 为核心契约证据）。
- 门禁：pm-application 全量 + apps/pm-api 分析相关用例 + 前端 vitest 触及文件；全绿后提交
  （message 注明「阶段二：切换」）。

统一验收交付（全部完成后，一次性提交验收材料，不得标记冻结）：
- docs/architecture/four-domain/object-registry-v1.1-projc-addendum.md 升 V1.2（projc
  六对象 COMPLETE、pc snapshot=DEPRECATED 字典、双源登记）。
- 台账 §10.3 落实状态 + 证据链接；《P3-11 T-1/T-2/T-3 交付报告》逐条对照完成标准（方案 §4
  矩阵）附测试输出。
- 全部 commit 到 master；不 push（push 由用户确认）；发现方案与源码事实冲突时停手上报，
  不得自行改裁决。

约束（全程有效）：AGENTS.md 全部规则；DATABASE_URL 用
'postgres://aipm:aipm@127.0.0.1:16433/aipm'（127.0.0.1 禁 localhost）；sqlx 禁宏（运行时
查询）；迁移号全分支扫号；只 git add 自己的路径；pc 分支/close 契约/E2E-02 未来断言口径
（双域分流）不得擅改；Edit 改 .vue 用 python 或整文件重写（本仓已知坑）。
```

## 3. 完成标准（与方案 §4 矩阵一致，验收逐条对照）

仓库分析结果按业务含义进入正确领域；ProjC 独立完整可审计的正式发布链；技术认知不能发布到 PC、产品认知不能发布到 ProjC；Finding 仅候选；未经人工确认不正式发布；越权/跨域无权限/版本冲突/重复提交正确处置；旧 snapshot 路径不能绕过新规则；Phase 2 close 接口及既有契约回归通过；不依赖历史迁移或双写。
