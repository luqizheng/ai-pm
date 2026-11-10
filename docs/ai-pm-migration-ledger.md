# 迁移台账（Migration Ledger）

> 统一登记迁移号段的预留与占用（评审 P1-1 收编产物，2026-11-02）。
> 规则：① 设计稿预留**不锁定**实施——实施以先到者为准，后到者按台账顺移并回填本表；② 取号前必须先对账本表与 `migrations/` 实况；③ 实施完成后把「状态」置为「已应用」并填文件名。

## 〇、基线合并（2026-11-09，冻结点 squash，编号自此重启）

旧链 0001~0065（64 文件，0029 历史空号）已在全会话冻结点合并为 **`migrations/0001_baseline.sql`**（pg_dump 终态提取，非朴素拼接；新链建库 vs 旧链建库 schema/data 逐行 diff 为零）。约束与影响：

- **后续新迁移自 0002 起编号**，不再受旧链历史与空号约束；
- **存量数据库不兼容**：旧链库的 `_sqlx_migrations` checksum 与新链对不上，`sqlx::migrate!` 会拒迁（pm-api 启动即 migrate，fail-fast）——本机容器与 **.10 服务器栈**升级前须**重锚**（drop 重建；或 pg_dump/restore 后清账重放 0001）；
- 迁移文件文本断言用例（migration_0012/0014 的 `migration_file_is_present_and_forward`）与 `scripts/upgrade-drill-0028-to-0031.sh` 演练随之退役（无迁移链即无升级场景）；migration_0064.rs 的手工建链「收敛路径」用例同样退役，fresh DDL 断言保留；
- 下节历史「已应用」表保留为审计记录，文件名不再对应 `migrations/` 现存文件；
- DK 系列在途迁移（DK-A conversation_discovery_link，取号链 0063→0066）已改取 **0002**，见下表。

### 〇·二、基线后在途迁移（权威 = `migrations/` 目录，编号自 0002 起）

| 号 | 文件 | 任务 | 状态 |
|---|---|---|---|
| 0002 | `0002_conversation_discovery_link.sql` | DK-A 对话编排底座：`ai.conversation.discovery_session_id` 1 列 + 三元组外键 `(tenant_id, project_id, discovery_session_id) → discovery.discovery_session(tenant_id, project_id, id)`（比设计稿 (tenant_id,id) 更严：钉同项目）+ 部分索引；无回填——设计 `docs/product/ai-pm-dialog-orchestration-design-v1.md` §9.2-①（D-DK-2）。**取号史**：设计稿预留 0063 → 并行批依次取走 0063/0064/0065 → 顺移 0066 → **2026-11-09 冻结点基线合并后改取 0002**（§〇） | 已实施（2026-11-09，DK-A 收官；验收 `docs/product/ai-pm-dk-a-acceptance-record-v1.md`，1226/1226 全绿） |
| 0003 | `0003_conversation_context_anchor.sql` | 对话上下文整合 Batch 2：`ai.conversation.context_anchor` jsonb 可空 1 列（线程级上下文锚点，建会话时定格、消息级不收，D-IC-1）+ 三重 CHECK（对象形态 / 键白名单 `project_id, project_name, repository_name, default_branch, repository_role` 减法表达 / 锚点 project_id 与行同项目）；NULL=无锚点，存量零回填——设计 `docs/product/ai-pm-assistant-context-integration-plan-v1.md` §三 Batch 2；服务端配套 `POST /conversations` 锚点校验（`deny_unknown_fields` + 仓库真实性 400）。**取号史**：DK-F 分支台账曾代登在途；0004 已由 DK-F 占用（llm_call_record_metadata），本表 0003 行=占用者就地登记 | 已实施（2026-11-09，整合会话 sess_05e4b2f0；pm-api ai_conversations 10/10 + pm-application 909/909 + pm-ai 154/154 绿） |
| 0004 | `0004_llm_call_record_metadata.sql` | DK-F 真栈联调：`ai.llm_call_record.metadata` JSONB 可空 1 列（无回填、无约束/索引）；编排轮次写 `sections_dropped` 等——设计 §5.3/§8.3-D2（D-DK-7 硬验收）；缺口登记 `docs/product/ai-pm-dk-f-step0-verification-v1.md` G-2。**取号史**：初取 0003 → 并行会话 0003_conversation_context_anchor 先占（其库已应用）→ DK-F 顺移 **0004**（2026-11-09） | 已实施（2026-11-09，DK-F §8.3 全表 30/30 PASS；随 §八合并执行进 master） |
| 0005 | `0005_conversation_interaction.sql` | 对话上下文整合 Batch 3C：`ai.conversation_interaction` 新表（交互终态权威：action/status/reason/option_id/answer/decided_by/reference 列 + UNIQUE(message_id) 一卡一生命周期 + reject 必带 reason / decision 必带 option_id CHECK + conversation/message FK 引主键）+ 同事务信封 `metadata.envelope.status` 终态化回读（A-21）——设计同上 §三 Batch 3C（D-IC-7=A）；REST `POST .../messages/{mid}/interaction`（终态幂等 200 重放 / 换 action 409 / 属主外 404 同形）。**取号史**：计划稿预估 0004 → DK-F 先占 0004（llm_call_record_metadata）→ 顺移 **0005**（2026-11-09，台账纪律 2） | 已实施（2026-11-09，整合会话 sess_05e4b2f0；pm-api ai_interactions 7/7 + 三 crate 1070/1070 绿） |
| 0010 | `0010_design_domain.sql` | S03 设计与技术决策域：新建 `design` schema 九表（design/design_revision 内容不可变守卫/design_requirement 绑需求修订/adr 六态/adr_revision 提交后冻结/adr_design_revision/adr_requirement/review_finding/design_approval 专表三 kind pending 唯一/implementation_baseline 冻结不可变守卫）+ `delivery.task.implementation_baseline_id` 追溯列——总体设计 `docs/ui/smark-pm-s03-01-analysis-design-technical-decisions-v1.md` Q1~Q5 裁决 + 核验基线 `docs/s03/S03-00-code-facts-report.md` 七缺口 | 已实施（2026-11-10，S03 后端批；验收 `docs/s03/S03-01-backend-implementation-report-v1.md`，SC-01~08 集成测试全绿 + 全 workspace 回归） |
| 0011 | `0011_task_plan_domain.sql` | S04 任务计划域：新建 `plan` schema（task_plan 聚合根 draft/in_review/approved/superseded + task_plan_revision 内容不可变守卫）+ `design.design_approval` 增 plan_id/kind 'task_plan'（审批基座复用）+ `delivery.task` 增 plan_revision_id/plan_task_key 追溯列与部分唯一索引（幂等正式化）——设计 `docs/s04/ai-pm-s04-backend-design-proposal-v1.md` Q1/Q2/Q3/Q5/Q6 裁决 | 已实施（2026-11-10，S04 后端批；验收 `docs/s04/S04-01-backend-implementation-report-v1.md`，全链集成测试 5/5 绿 + 全 workspace 回归） |
| 0012 | `0012_system_embedding_config.sql` | 系统级 embedding 供给配置：新建 `platform` schema 单行表 `platform.embedding_config`（`id smallint PK CHECK(id=1)` 强制单例；endpoint/model/enabled/updated_by/updated_at；种子 `http://192.168.2.10:11434/v1` + `bge-m3`，enabled=false——Vector 通道 G-3 门控，配置先行消费后置）。配套 REST `GET/PUT /v1/admin/embedding-config` + `POST /v1/admin/embedding-config/test`（实调 `/embeddings` 返回 `{ok, dims, latency_ms, error}`，写门 `require_system_admin`）+ 前端系统设置页（`/admin/settings`） | 已实施（2026-11-10，平台管理面批；验收 `crates/pm-application/tests/system_embedding_config.rs` 5/5 绿 + 前端 `AdminSettingsView.test.ts` 4/4 绿 + i18n 命名空间对齐） |

## 一、已应用（历史审计——文件已并入 0001_baseline.sql）

| 号 | 文件 | 任务 | 状态 |
|---|---|---|---|
| 0001–0021 | `migrations/0001_core.sql` … `0021_task_dependency.sql` | 既有 | 已应用 |
| 0041 | `0041_team_llm_provider.sql` | Team LLM Provider Management Phase 1（DEC-LLM-001~004；R-LLM-01/02 修复） | 已应用（2026-11-06） |
| 0042 | `0042_s02_i2_projection.sql` | S02-I2 Markdown 反向投影（并行会话） | 已应用（2026-11-06） |
| 0043 | （预留→本批） | Project AI Model Policy（§10.2 项目指定 Provider） | 实施中（2026-11-06） |
| 0044 | `0044_team_onboarding.sql` | Team Onboarding（Team≡tenant 扩展：slug/description + 邀请三表；feat/team-onboarding 已合入 master） | 已应用（2026-11-06） |
| 0045 | `0045_s02_i3_source_traceability.sql` | S02-I3 来源与追溯（5936105 曾以 0044 提交，与 Team Onboarding 撞 sqlx 版本号 → DEF-01 P0，2026-11-07 顺移） | 已应用（2026-11-06） |
| 0046 | `0046_acp_communication_face.sql` | S03-ACP 通信面切片（ACP-01/02/08，设计稿 docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp01-02-08-v1.md V1.1；R-ACP-6 原裁 0045 起，0045 已被 s02_i3 顺移占用 → 本迁移顺移 0046） | 已应用（2026-11-07） |
| 0047 | `0047_interaction_response.sql` | S03-ACP ACP-03 Human Response（interaction_response 表 + REST 回复入口同事务 continue 下发） | 已应用（2026-11-07） |
| 0048 | `0048_kb_pc_intake.sql` | KB-PC 服务化 Phase 1-3 Intake（intake_record + intake_task；**原 0046_kb_pc_intake，后实施者顺移——0046 已被 ACP 通信面占用，双 0046 撞号修复**；已建专用容器 aipm-kbpc-pg:16447 需重置后复测） | 已应用（2026-11-07，KB-PC 会话） |
| 0049 | `0049_client_registry.sql` | V-05-1 Client Registry 纵切（device 四列+双唯一约束+task_run.lease_client_id 复合外键+回填三分类）——设计 `docs/project-ai-team-v2/ai-pm-v05-client-registry-design-v1.md`（联合裁决通过 + 约束 A/B/C） | 已应用（2026-11-08，WP-B 实施批；真实栈 E2E 待联调批） |
| 0050 | `0050_response_delivery.sql` | ACP-04 Delivery 持久化（一回复一账 UNIQUE(response_id)；重投复用原行 attempt+1；generation 恒 1；continuation_id 前向可空）——契约同上（联合裁决五问通过） | 已应用（2026-11-08，批 2 实施；E2E 待联调批） |
| 0051 | `0051_continuation.sql` | ACP-05 Continuation 正式身份与状态机（UNIQUE(run_id,generation)；open→started→consumed/closed+frozen；身份三件冻结之三=continuation.id）——同上契约（暂定确认→已实施；Recovery 批已先行） | 已应用（2026-11-08，批 4） |
| 0052 | `0052_recovery_foundation.sql` | Recovery Foundation（`worker_command.reacquired_generation` + `task_interaction.previous_chain_id` 链间弱关联）——批 3 实施（联合基线 §4） | 已应用（2026-11-08） |
| 0053 | `0053_checkpoint1_kb_pc_minimal.sql` | Checkpoint-1 最小纵切：KB/PC 目标表最小子集（kb.document/revision + pc.object_type/cognition_object/cognition_revision/source/evidence；正文暂存 PG、关系/版本/提案/冲突面不建——偏差登记在迁移头注）——取号时 0052 尚为 tentative 预留，按台账纪律 max+1 取号（Recovery 批后实施占 0052，双方无撞号） | 已应用（2026-11-08，checkpoint-1 会话） |
| 0054 | `0054_acp06_07.sql` | ACP-06 Checkpoint 登记元数据（正文在 Workspace）+ ACP-07 Deadline 重新授权（独立审计/有效期；T3 不改原 deadline）——联合设计 acp06-07-joint-v1.md（批 5） | 已应用（2026-11-08） |
| 0022 | `0022_delivery_manifest.sql` | T-041（D4） | 已应用（2026-11-02） |
| 0023 | `0023_run_executor_hardening.sql` | T-032 | 已应用 |
| 0024 | `0024_knowledge_section_index.sql` | T-060（知识索引 V2） | 已应用（2026-11-02） |
| 0025 | `0025_discovery.sql` | T-083（Discovery 数据模型） | 已应用（2026-11-02） |
| 0026 | `0026_delivery_review.sql` | T-043（D5） | 已应用（2026-11-02） |

## 二、已预留（设计稿已过稿/提交，未实施）

| 号 | 预留者 | 设计稿 | 内容 | 备注 |
|---|---|---|---|---|
| ~~0024~~ | ~~T-060（知识索引 V2）~~ → **已实施** | `docs/ai-pm-t060-knowledge-index-v1.md` §4 | `0024_knowledge_section_index.sql` | 2026-11-02 先到先得，已落「一、已应用」 |
| ~~0025~~ | ~~T-083（Discovery 数据模型）~~ → **已实施** | `docs/ai-pm-project-discovery-v1.md` §附录A | `0025_discovery.sql` | 2026-11-02 已落「一、已应用」；实施时补 `ai.document_proposal` 租户内唯一键（0019 清单遗漏） |
| 0027 | 空闲（缓冲） | — | — | 供下一任务（注：0022–0040 已由各任务就地取号应用，本表「二」仅存历史；新取号以 `ls migrations/` 实况为准） |

## 三、纪律

1. 新设计稿取号：先读本表 → 再 `ls migrations/` 核实 → 预留号 = `migrations/` 最大号 + 1 与本表最大预留号 + 1 的较大者。
2. 实施撞号：后实施者无条件顺移，不得改已应用迁移；顺移后同步回填本表 + 自己的设计稿。
3. 本表由每个取号/顺移动作的执行任务就地维护（含在对应 `chore(todo)` 或 `feat` 提交内）。
