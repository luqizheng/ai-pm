# 迁移台账（Migration Ledger）

> 统一登记迁移号段的预留与占用（评审 P1-1 收编产物，2026-11-02）。
> 规则：① 设计稿预留**不锁定**实施——实施以先到者为准，后到者按台账顺移并回填本表；② 取号前必须先对账本表与 `migrations/` 实况；③ 实施完成后把「状态」置为「已应用」并填文件名。

## 一、已应用（权威 = `migrations/` 目录）

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
| 0063 | `0063_runner_availability.sql` | D-2=c 派发可用性真源：is_runner + last_heartbeat_at 设备级心跳 + 领取型项目 worker 排除（runner_availability 批，20:31:56 先落地） | 已应用（2026-11-08，派发竞争批） |
| 0064 | `0064_pc_formal_model.sql` | KP-1 PC-01B 正式模型：pc 正式 18 表（对象/关系/版本成员/治理/追溯/账本 + 正文承载 pc.document\*）+ 0053 最小表原地收敛（正文迁 pc.document_revision、证据钉修订、locator 拆结构化列）+ 触发器/种子/闭合约束——设计权威 docs/architecture/product-cognition/pc-01b/ V1.2；**原取 0063 与 D-2=c 撞号，对方先落地 4 秒，按台账纪律②顺移 0064（2026-11-08 20:32 后实施）**；偏差登记（PCGIT 后置/snapshot 四类型保留/analysis_mode 保留且放宽可空=非分析草稿 NULL/证据身份含 commit_sha/doc_key 兜底形态/补 source_context 列）在迁移头注与验收记录 | 已应用（2026-11-08，KP-1 会话） |
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