# KP-2 — PC-01C 确认闭环实施验收记录 V1.0

| 项 | 值 |
|---|---|
| 批次 | KP-2（V0.5 范围基线 V1.3 §3.3 第四批：PC/KB 应用完善——操作链终态最后一段「人工确认」） |
| 分支 | `feat/kp2-pc01c-confirmation-loop`（worktree `ai-pm-kp2`，基 master 44c4c8d：迁移基线 0001_baseline + 0002、DK-A~D、KP-3 已在；收官前集成 master a0c7261（DK-E），见 §八） |
| 提交 | 1d4b4ea 发布同身份替换修复 → 62972fe 提案/审批命令服务 → bbce675 REST P4/P5 + 分析→提案接线 + 单例挂接 → 2e19598 P5 原子确认 → 13f437f 测试套件 + 验收记录 → b37472d 集成 DK-E → ff23340 DK-E F3 断言适配 → 本文档收官批 |
| 前序 | KP-1 验收记录 `docs/kp1-pc01b-acceptance-record-v1.md`（§六 KP-2 移交清单 1/2 由本批兑现） |
| 设计权威 | `docs/architecture/product-cognition/pc-01b/` 拆分件 04（§10 提案/审批/冲突）、00（§11.1 PC-D-01、§11.3 PC-D-06）、06（§17 锁序、§18.1 十四步、§19.1 审计词表） |
| 迁移 | **零新迁移**（0001_baseline 已含 pc 18 表 + 触发器 + 部分唯一索引全套） |
| 日期 | 2026-11-09 |

---

## 一、用户裁决与执行判断

| # | 事项 | 结论 | 性质 |
|---|---|---|---|
| 1 | 确认 UI 最小面是否随 KP-2 | **不随**：V1 后端闭环先行，UI 随 PC-I4 / V0.6 | 用户裁决（2026-11-09 开工 AskUserQuestion） |
| 2 | PcDecisionSubmittedStub（流程 E）接真还是留桩 | **保留桩 + 登记**（偏差 o） | 用户裁决（同上） |
| 3 | KP-1 发布缺陷（update 目标不替换前版本携带的同身份旧修订） | 本批最小修复，独立提交 1d4b4ea（偏差 k）：update_object 目标按身份置换携带（与 deprecate 同口径）+ 单修订断言；KP-1 门禁用例语义不变 | 用户裁决（2026-11-09 ①） |
| 4 | 分析→提案「新修订」判定口径 | 三条件口径：本次分析钉扎 ∧ 未被任何存活提案覆盖（排除 failed/expired）∧ 不在现行版本（弃 `revision_created=true`；偏差 l） | 用户裁决（2026-11-09 ②） |

## 二、交付物

| 层 | 内容 | 位置 |
|---|---|---|
| 服务层 | `pc/proposal.rs`：create（成员规范化 + F-05 端点自动并入 + R7/F-07 等级下限 + 闭合预检 + 内容锚定哈希）/ submit（②b 完整性 + 哈希一致 + L3 冻结 + 基线核对 + pending 审批锚定）/ approve·reject（真人 + A3 + 请求 CAS + FOR UPDATE + 三方锚点比对 + approve 侧冻结）/ withdraw / decide（P5 编排）/ load_proposal（P4 读面）/ propose_from_analysis（AI 起草）/ 单例骨架 | `crates/pm-application/src/pc/proposal.rs`、`object.rs`、`mod.rs` |
| 发布 | 同身份替换 + 每身份一修订断言（缺陷修复）；`publish_inner` + `InlineApproval`（P5 原子确认，`publish_version` 对外行为不变） | `crates/pm-application/src/pc/publish.rs` |
| REST | P4 GET 提案详情、P5 POST 审批转达（501 → 落真）；PcError → HTTP 映射；P3 提案分支 + KP-1 来源展开两缺陷勘误 | `apps/pm-api/src/knowledge_services.rs` |
| 接线 | 分析派生后起草提案（pc.change_proposed 真事件）/ NO_CHANGE；项目创建同事务挂 Profile/Vision 骨架 | `crates/pm-infrastructure/src/kb_pc_handlers.rs`、`crates/pm-application/src/bootstrap.rs` |
| 测试 | 新套件 `pc_proposal`（8）、`pc_proposals` REST（5）、`kb_pc_repo_flows` 用例 7、`pc_model` +1；既有 `intake`/`kb_pc_repo_flows` 适配 | 各 tests |

## 三、PC-I3 验收条目对照（需求 09 稿 §17）

**交付内容**

| 交付项 | 兑现 | 证据 |
|---|---|---|
| Change Proposal 复用 | ✓（PC-D-01：PC 专属 pc.proposal/approval，复用 A2 不可变范式 + A3 权限闸 + governance.audit_log） | `proposal.rs`；01B §11.1 |
| L0～L3 审批矩阵 | 部分（V1 = L1/L2 同权 A3 + L3 冻结；矩阵正式化 V0.6 显式切出） | 范围基线 V1.3 §3.3 |
| 人工确认 | ✓ | P5 + `approve_proposal`；AI/服务主体恒 403 |
| 正式认知版本 | ✓ | 全链用例：draft → 提案 → 审批 → 发布 → 版本（两轮） |
| 决策状态与执行状态分离 | ✓ | 审批事实（pc.approval：pending → approved/rejected，不可变）≠ 提案执行态（pc.proposal：submitted → approved → applied / failed）≠ 版本（pc.cognition_version）；发布 CAS 失配 = 提案 failed、审批事实不改写 |
| 正式写入安全权限闸 | ✓ | 真人闸 + A3 + A* 可信交互上下文 + L3 冻结三落点 |

**验收条目**

| 验收条目 | 兑现 | 证据 |
|---|---|---|
| L0 不产生正式版本 | **不实现（登记）**：V1 无 L0 入口——一切正式版本必经人工确认，不存在「自动处理」路径；L0 语义随 V0.6 矩阵正式化 | 服务层无自动确认分支；`propose_from_analysis` 只起草提交，approve 对服务主体 403 |
| L1 确认逻辑正确 | ✓ | `pc_proposal::confirmation_loop_full_chain`（提交锚定 → Lead 审批 → 发布；L1 推导；审计 create/submit/approve/publish 各 1） |
| L2 只能由授权审批人确认 | ✓（V1 同权 = A3：Lead OR `core.approver_grant` 生效行） | `decision_gates_fail_closed`：普通成员/非成员 403、显式 Approver 通过；R7/F-07：business_rule/product_vision 固定 L2 不可下调（`drafting_validates_members_and_levels`、单例用例） |
| L3 能够冻结 | ✓（三落点：submit §10.3 / approve §18.1 ⑥ 确认入口 / 发布⑥兜底） | `decision_gates_fail_closed`（submit 侧 403 + approve 侧 403 → 解冻后通过）；KP-1 `publish_gates_fail_closed`（发布侧） |
| 正式写入安全测试通过 | ✓（AI 403 矩阵） | 服务层：dev_agent / service（dispatch）/ 非成员 / 普通成员 × approve·reject·decide 全 403；REST：ai_pm 主体 403、普通成员 403、API Key 上下文 403（同主体交互式令牌通过）；`kb_pc_repo_flows` 用例 7 分析链 AI 确认 403 |
| 历史版本不可覆盖 | ✓ | `confirmation_loop_full_chain` ⑦：版本头锚定列改写 P0001（T-12）、版本删除 P0001（T-23）、v2 发布后 v1 成员原样；`migration_0064` fresh 负向（T-03/04/06/08/09/13/14/15/26/27/28 持续绿）；锚定篡改 P0001（提案内容列 / 审批锚点 / 已裁决审批状态） |

**需求 §19 风险控制对照**

| 风险 | 控制落点 |
|---|---|
| §19.2 正式写入越权 | 正式确认只经 pc 服务层（V-6）；fail-closed 真人 + A3；MCP 零确认工具；REST 治理动作挂可信交互上下文 |
| §19.3 审批状态不一致 | **P5 原子确认**（2e19598）：审批事实与版本同事务，失败不留「已确认无版本」；`decide_approve_is_atomic_with_publish` 注入发布尾段失败验证 |
| §19.4 并发更新 | 请求侧 `expected_version_no` CAS + 发布版本基线 CAS（失配转 failed）+ 提案/审批 FOR UPDATE + `uq_pc_approval_pending`（23505→409）+ 锚点重算 |
| §19.5 AI 错误推断 | AI 只起草（候选 draft/提案）≠ 正式（版本）；确认按 L1/L2；冲突 L3 冻结 |

**确认链复跑证据**（任务书验收门槛）：全链 = `pc_proposal::confirmation_loop_full_chain` + `kb_pc_repo_flows::confirmation_loop_publishes_versions_across_commits`（真 git 仓库：commit → 分析起草 → 人工确认发布 v1 → 再 commit 仅变化修订入链 → v2）；AI 主体 403 = 上表；同修订集重复提案幂等 = `analysis_drafts_uncovered_revisions_once`（重投递/重分析 NO_CHANGE）+ 用例 7（同 HEAD 零新提案）+ `reverify_same_head_is_idempotent`（提案计数 1）。

## 四、审计点登记（C174/C180）

| 文件 | INSERT 点 | 动作（§19.1） | 身份列 |
|---|---|---|---|
| `crates/pm-application/src/pc/proposal.rs` | 1（helper） | `pc_proposal.create` / `submit` / `withdraw` / `approve` / `reject` | 四身份列全写（审批 = 真人 / 交互上下文；分析起草 = dispatch 服务主体，三身份列 NULL） |
| `crates/pm-application/src/pc/publish.rs` | 2（不变） | P5 原子确认的 `pc_proposal.approve` **复用 proposal.rs helper**，不新增 INSERT 点 | — |

`audit_actor_coverage.rs` PRODUCTION_SITES 已登记 `pc/proposal.rs`（1）；c174/c180 绿。

## 五、偏差登记（续 KP-1 a~j）

| # | 偏差 | 依据 / 处置 |
|---|---|---|
| k | **publish.rs 有改动**（任务书「publish_version 不改」）：① 成员集同身份替换（KP-1 只剔 deprecate 目标的携带钉扎，update/create 目标不替换前版本同身份旧修订——第二次发布起版本含同对象双修订，违 T-11 语义；分析链第二个 commit 必触发）+ 每身份一修订断言 + 废弃关系不并端点；② `publish_inner` + `InlineApproval`（P5 原子确认）。`publish_version` 对外语义不变 | 1d4b4ea / 2e19598；回归 `publish_next_version_replaces_carried_revision`（修复前红：v2 成员 3 行同对象双修订） |
| l | 「新修订」判定 = **三条件口径**（用户裁决 ②）：本次分析钉扎 ∧ 未被任何存活提案覆盖（failed/expired 以外任何状态均视为覆盖）∧ 不在现行版本。实现细化一条：**仍为对象当前修订**（迟到重投不提已被新草稿取代的修订）。正常路径与 `revision_created=true` 等价；额外：重投递/起草前崩溃自愈、KP-1 存量草稿首轮入链、被驳回内容不变不重复打扰；免改 `kb_pc_analysis.rs` | 用户裁决 ②；`analysis_drafts_uncovered_revisions_once` |
| m | P5 approve = **审批 + 发布同请求原子**（契约 `published_version_ref` 位）；两步法 `approve_proposal` + `publish_version` 服务层保留（无 REST 入口） | 需求 §19.3；`decide_approve_is_atomic_with_publish` |
| n | create/submit/withdraw **无 REST 入口**：V1 起草 = 分析链自动（AI）+ 服务层调用；人工起草/撤回入口随 UI（PC-I4）。P1/P2/P6/P7 维持 501 | 用户裁决 1（UI 不随） |
| o | 流程 E 桩保留：`PcDecisionSubmittedStub` 仍发占位 `proposal_ref`（`pc-proposal-{task_id}`，非 UUID，P4 对其 404）；产品决策直达通道的提案生成归后续批 | 用户裁决 2 |
| p | Profile/Vision 初始化仅 **identity 骨架**：PC-D-06 规则中的「Git 模板骨架文档 + `pc.source(manual_input)` 登记」随 PCGIT 后置（identity 态无修订即无正文承载，偏差 a 同源）；`created_by` = 项目 owner；骨架由后续草稿 upsert（object_key `product-profile`/`product-vision`）填充首修订 | 01B §11.3；单例用例 |
| q | withdraw 不改 pending 审批行（审批表 pending → 终态须有裁决人，撤回非裁决）——由 `proposal.status = withdrawn` 判死，裁决入口 409；**expire 不做定时器**（登记） | 01B §10.2 触发器约束；任务书「V1 从简」 |
| r | **P3 契约变化 + KP-1 缺陷勘误**：① 来源展开查询 `rev` 别名重复（PG 42712，真实分析任务 P3 恒 500——套件无真数据覆盖故未暴露）；② `versioned` 误读视图恒非空列（恒 true）→ 改读 `in_published_version`；③ 分析起草提案时 result_ref 改锚 proposal（KP-1 锚 source），P3 增 `conclusion`/`proposal_ref`/`proposal` 字段；NO_CHANGE 仍锚 source | `pc_proposals::p3_expands_proposal_and_no_change_conclusions`（含发布前后 versioned 翻转） |
| s | 分析结论事件**共键** `pc_analysis:{task_id}`：`pc.change_proposed` 与 `pc.analysis.completed` 同幂等键——一任务一结论。开工提示词建议 `pc_proposal_source:{source_id}`，未采纳：`register_source` 每次运行新插来源行，worker 重投时 source_id 变化无法去重（可致任务 result_ref 显示 NO_CHANGE 而提案已存在）；按任务键则重投的结论坍缩进首个事件，且沿用 KP-1 既有 NO_CHANGE 键（目录 hint=`proposal_id`；任务与分析提案 1:1，等价） | `pc::analysis_outcome_key` |
| t | **闭合预检**（create 时，设计未明文的服务层前置）：现行版本携带关系的 PC 内端点被本提案改钉/废弃 → 须同提案更新或废弃该关系，否则拒绝（否则审批后发布闭合断言必败） | `endpoint_merge_conflict_and_projected_closure` |
| u | 独立 approve 只核请求侧 CAS（不核现行版本 = 基线）；基线移动后的发布 → 提案 failed（§17.2 场景 3）。P5 原子路径同事务先版本 CAS：陈旧提案 409 + failed + 审批保持 pending | 01B §17.2 |
| v | 审计 `source_ref` = proposal_id（UUID 列；§19.2 允许 operation_id 或 proposal_id） | `governance.audit_log.source_ref UUID`（0004） |
| w | AI 起草主体 = `ai-pm-dispatch` 服务主体（`AuditActor::dispatch`），审计三身份列 NULL（系统面，KP-1 分析审计同款） | Q2-C「AI 动作 actor = 发起主体」 |
| x | rustfmt 顺带：`audit_actor_coverage.rs` 两行既有注释对齐（master 原非 fmt 干净，语义零变更） | — |
| y | **跨批测试适配**：DK-E F3 断言（PC/KB 零写入）由全表零改为更正轮前后零变化——项目创建自 KP-2 起含 Profile/Vision 骨架 | ff23340；§八 |

## 六、测试与门禁证据

- **新套件**：`pm-application::pc_proposal` 8 例（全链、门禁矩阵、起草校验、F-05 冲突与闭合预检、驳回与撤回、分析起草、单例骨架、P5 原子）；`pm-api::pc_proposals` 5 例（P4 作用域、P5 发布/幂等/驳回/400/409、成员与 AI 403、API Key 403、P3 两结论）；`kb_pc_repo_flows` 用例 7（真 git 两轮确认）；`pc_model` +1（第二轮发布替换）。
- **既有适配**：`pm-api::intake` 契约面（P4 移出 501 组 → 404 组，P6 补入 501 组）；`kb_pc_repo_flows` 用例 1/2（结论改提案 / 同 HEAD 提案不重复）。
- **反证（测试确能拦截）**：发布修复回归在修复前代码上红（v2 成员同对象双修订）；P3 别名缺陷在 psql 直接复现 42712；P5 原子用例以账本终态注入发布尾段失败（两步法实现下会留下 approved 无版本）。
- **PC 五套件**：pc_proposal / pc_model / audit_actor_coverage / pc_proposals / kb_pc_repo_flows = 28/28 绿。
- **全量回归**：`bash scripts/cargo-test.sh`（nextest 全量 + doctest，专用容器 aipm-kp1-pg 127.0.0.1:16473，快档 template1 命中）——提交 ③ 树 **1315/1315 + doctest 绿，EXIT=0**（nextest 160s / 全程 8m49s）；终态树（提交 ④ + 新套件）**1316/1316 + doctest 绿，EXIT=0**（nextest 141s / 全程 9m06s；= master 44c4c8d 基线 1301 + 本批 15 例）。其后仅向 `confirmation_loop_full_chain` 补 T-12/T-23 断言（纯测试断言增补），`pc_proposal` 套件复跑 8/8 绿。
- **clippy**：`cargo clippy -p pm-application -p pm-infrastructure -p pm-api --all-targets` = 0 告警。
- **fmt**：改动面单文件 rustfmt（edition 2021）干净。
- **ADR-06a 守卫**：新增/改动文件零 `ai.*` 文档容器字面量。
- **迁移**：零新迁移（台账无变更）。

## 七、遗留与移交

1. 确认 UI（提案列表 / 详情 / 审批动作）——PC-I4 / V0.6（用户裁决 1）。
2. L0～L3 审批矩阵正式化（L0 自动处理语义、L2/L3 分级审批人）——V0.6。
3. 流程 E 接真（产品决策直达 → PC 起草提案）——后续批（偏差 o）。
4. 人工起草 / 撤回 REST 入口（随 UI）；P1/P2/P6/P7 落真；expire 定时器。
5. NO_CHANGE/CONFLICT 冲突发现与影响分析族（PC-I5/I6，范围外）。
6. 真栈 E2E：pm-api 镜像重建后 `scripts/checkpoint1-e2e.sh` 补 P4/P5 步（与 KP-1 移交 4 同一待办）。
7. 偏差 k/l 已获用户裁决确认（2026-11-09 ①②）。
8. **DK-E 挂账承接**：DK-E（a0c7261）场景 2 更正回流按 F8↔F23 裁定 (b) 止步「findings 双行 + deferred 标记」，PC 侧显式挂账 KP-2（`docs/product/ai-pm-dk-e-step0-f8-f23-verification-v1.md` §5/§7）。本批已提供承接面（`create_proposal` / `submit_proposal`，origin_kind 可取 `manual_input`/`conflict_resolution`），**标记 → 正式提案的平移接线不在本批任务书范围**，留后续批（与流程 E 接真同类）。
9. **DK-C S2 交互（登记，归 DK 侧裁量）**：访谈上下文 S2（`crates/pm-ai/src/discovery_context.rs` `product_cognition`）按设计收录 identity 占位行；集成后每个新项目恒有 Profile/Vision 两骨架，原「暂无认知对象（尚未完成仓库快照分析或人工确认）」提示不再出现，代之以两行空摘要（`product-profile`/`product-vision` + 显示名）。不误导（无内容即无确认语义）、无用例回归；是否对 identity 行加「骨架·待确认」标注由 DK 侧决定（对话编排域，本批不触碰）。

## 八、集成记录（收官前合入 master DK-E）

| 项 | 内容 |
|---|---|
| 背景 | 本批开发期间 master 由 44c4c8d 前进至 a0c7261（DK-E 收官：场景 2 更正回流 + 导入自动开访谈；零迁移） |
| 方式 | 在 KP-2 分支 `merge master`（b37472d）先集成验证，再 `--no-ff` 合入 master——master 在集成树全绿前不动 |
| 冲突 | 1 处：`kb_pc_repo_flows.rs` 用例 1 断言——master 仅格式化 KP-1 的 `"source"` 断言，KP-2 已改为提案口径 → 取 KP-2 侧；`kb_pc_handlers.rs` 两侧改动不同 hunk（DK-E：`KbAnalysisCompletedHandler` 自动开访谈 + `register_kb_pc_handlers` 增开关参数；KP-2：PC 分析 handler 起草提案）自动合并 |
| 语义适配 | DK-E `correction_records_both_rows_and_keeps_kb_pc_untouched` 的 F3（PC/KB 零写入）原按全表 count==0 断言，与 KP-2 项目创建建骨架冲突（2≠0）→ 改「更正轮前后计数不变」（ff23340，语义不变） |
| 门禁 | clippy（pm-application / pm-infrastructure / pm-api / pm-ai，--all-targets）0 告警；集成树首轮全量 1327/1328（唯一红 = 上述 F3，适配后 DK-E 六例 6/6）；终态全量 **1328/1328 + doctest 绿，EXIT=0**（nextest 123s / 全程 3m34s；= DK-E master 1313 + 本批 15）——即合入 master 的树 |
