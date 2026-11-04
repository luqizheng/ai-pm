# S03-00 · 复用/扩展/新建评估（Reuse, Extension & New-Build Assessment）V1.1

> V1.1：收编 R1（目标以 S02 裁决为准）/R2（A* 对照）/R4（不可变性三层表述）/R6（审批路线边界）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

基线 @ `3ccb42f`。结论口径：**A=直接复用**（已有实现满足目标要求，无需改变业务结构）；**B=扩展复用**（具备基础能力，需增字段/关联/状态/权限）；**C=新建**（无相应实体或机制）；**D=暂不确定**。判定纪律：不为减少工作量把 B/C 归成 A；每一格附依据。**目标要求一律以 S02/S03 已裁决架构为准（含 S02 正文权威裁决与 I6-A A* 治理裁决）；当前实现与裁决的偏差在「现有实现」列如实记录为缺陷/偏差，不据此改变 A/B/C/D 判定。**

## 一、差异矩阵（任务书第七部分 15 项）

| # | 能力 | 现有实现（证据） | 目标要求 | 结论 | 依据 |
|---|---|---|---|---|---|
| 1 | Requirement | delivery.requirement 实体+current_revision_id 指针+审计（0002:3-14） | 独立实体 | **A** | 实体、归属、指针、幂等 code 均在；缺类型/created_by 属字段增补不改变结构（若 S03 需要则并入 #2 扩展） |
| 2 | Requirement Revision | 版本链+state CHECK+UNIQUE(version)（0002:16-34；revision.rs）；应用层未检索到正文覆盖路径，数据库层无不可变保护 | 不可变版本（应用层+数据库层共同保障） | **B** | 数据库层无正文不可变保护（无触发器/约束，不得由"未检索到 UPDATE"推导"数据库禁止修改"）；REST 无并发防护（TOCTOU）；执行器无 state 前置（superseded 可复活）；content_hash 双公式需统一 |
| 3 | Technical Design | 无实体；唯一入口=inbound_event 事件（tools.rs:36；ingestion.rs） | 独立实体 | **C** | 十二环追踪证明无实体/版本/审批/关联 |
| 4 | Design Revision | 无 | 不可变版本 | **C** | 同上；结构参照=requirement_revision+document_revision 双先例 |
| 5 | Design-Requirement N:N | 无关联表；唯一同构先例=task_requirement 复合 FK 绑 revision（0002:68-80） | 多对多+Revision 级追溯 | **C** | 新建表；形态照抄 task_requirement |
| 6 | ADR | 无（decision 仅 doc_type 枚举值 0006:11-13） | 独立实体 | **C** | 十问全否 |
| 7 | ADR Revision | 无 | 历史可追溯 | **C** | 决策语义层（accept/reject/supersede/决策人）全缺；正文载体可挂知识文档管线 |
| 8 | Design Review | D5 交付复核专用（run_kind='review'+review_approval+findings，0026） | 独立评审流程 | **C** | 不支持任意对象/多评审人/问题关闭；其不可变/锚定/多轮形态可复刻 |
| 9 | Human Approval | 三套专表审批（approval_request/change_proposal 管线/review_approval）+require_human 原语（0014;0026） | 人类正式确认（按 A*：仅可信人类交互式 REST/UI） | **B** | 机制在但：现有身份模型不满足 S02-I6-A A* 七条安全要求（P0 缺陷：api_key→human+auth_mode 零消费+REST 身份自报+MCP 现存确认入口）；approval_request 语义硬编码不可挂新对象；结构路线留待 S03-05 |
| 10 | Audit | audit_log 全列（before/after/compensation/actor 四列/auth_mode）+触发器+outbox（0003~0027） | 完整审计 | **B** | before 口径三处不统一；身份冒认会写入脏审计（R1/R2）；新实体需首版带 before |
| 11 | Implementation Baseline | 无对象；同构先例=policy_snapshot_hash（0032:15）+review_approval 三重锚定（0026:223-249） | 不可变快照 | **C** | 新建实体；快照/锚定/单向 pending 形态有同构先例 |
| 12 | Readiness Gate | 三类检查可组合（依赖✅/仓库基线✅/验收证据🟡） | 五项准入条件 | **C** | 需求完整性/设计审批/ADR 检查无对象可查；五条件聚合判定+冻结+授权语义不存在 |
| 13 | Task Traceability | task_requirement 绑 revision（0002）+task_repository 范围（0011:86） | 引用实施基线 | **B** | 无 design_revision/adr/baseline 引用列；加列+组合 FK 即可，形态现成 |
| 14 | Run Traceability | task+每仓基线+策略快照+supersession 链完整保留（0010~0032；lease.rs:140-167） | 保留执行依据 | **A** | 重新执行保留原始依据已满足（无 DELETE、RESTRICT、supersedes_run_id 链）；设计依据字段待设计实体出现后属 #3/#4 范畴 |
| 15 | Delivery Traceability | commit 级（branch/sha/author/test_evidence）+manifest 四态+验收裁决（0022/0026） | 保留交付依据 | **B** | 交付依据保留已满足，但需求/设计/ADR/基线追溯列缺失（VF-17 六问中 4 否） |

### 统计

| 结论 | 数量 | 占比 | 明细 |
|---|---|---|---|
| A 直接复用 | 2 | 13.3% | Requirement、Run Traceability |
| B 扩展复用 | 5 | 33.3% | Requirement Revision、Human Approval、Audit、Task Traceability、Delivery Traceability |
| C 新建 | 8 | 53.3% | Technical Design、Design Revision、Design-Requirement N:N、ADR、ADR Revision、Design Review、Implementation Baseline、Readiness Gate |
| D 暂不确定 | 0 | 0% | — |

## 二、可复用资产清单（供 S03 设计引用，不构成实施决定）

1. **结构参照三件套**：DesignRevision→requirement_revision（version/指针/state/superseded）；DesignRequirement→task_requirement（复合 FK 绑 revision_id）；ImplementationBaseline→policy_snapshot_hash+review_approval 三重锚定。
2. **知识域正文设施**：Markdown+Git 权威+document_revision 账本（git_commit_sha/revision_no/content_hash）+八态提案管线+质量门（knowledge_quality.rs）+红线 confirm/veto+KnowledgeWriteCoordinator（advisory lock+operation_id+base_revision 校验）+I4-F 四写入口行锁门禁。
3. **审批形态资产**：pending 守卫单语句迁移（approval.rs:39-56）、DB 触发器不可变约束（0026:273-296）、required_role/decided_by/decision_reason/decided_at 字段形状、多轮 attempt_no 分轨唯一索引、同对象单 pending 部分唯一索引。**边界（R6）：结构路线（独立专表/共享基础设施/其他）留待 S03-05 论证；approval_request 的 1:1 绑定语义不得复用为通用设计审批。**
4. **并发资产**：乐观锁模式+FOR UPDATE 清单（30+ 处）、outbox SKIP LOCKED、幂等三件套（idempotency_key/operation_id/request_fingerprint）。
5. **审计体系**：全列 audit_log+INSERT 触发器+outbox RunCompletedHandler。
6. **AI 约束机制**：policy 红线黑名单+RequirePreConfirmation+executor「仅 Human+RedlineApproval」通道（唯一真闸，已有测试）。
7. **门禁资产**：project_gate 行门闩+锁序、I1 归档事务、Run 快照/执行期双段判定（deny-all）。

## 三、需扩展缺口清单（B 项的缺口明细）

| 缺口 | 现状 | 证据 |
|---|---|---|
| 审批多态化或 S03 专表 | approval_request FK 硬绑 change_proposal；Approve=执行命令语义 | 0003:160-161；approval.rs:89-113 |
| 真人门 11 入口 | api_key→human provisioning+auth_mode 零消费+REST 身份自报 | auth.rs:486-494,121-131；documents.rs:286,311 |
| 需求 Revision 库级不可变 | 无触发器/无「单 approved」约束/REST 无锁 | 0002:30-31（无）；revision.rs:84-120 |
| 双入口收口 | REST/执行器定稿语义分叉 | revision.rs:93-95,112-120 vs executor.rs:847-915 |
| AutoApply 红线名单 | approve_requirement_revision 不在名单 | ai-pm-ai-lead-v2.yaml:16-39 |
| 审计 before 口径 | 三处只写 after | queue.rs:368-385；pm_write/audit.rs:48-102；outbox_handlers.rs:238-250 |
| REST 权限码 | 零校验+无权威清单 | routes.rs:341-353；auth.rs:556-568 |
| required_role 比对 | 无消费点 | policy_engine.rs:300-322 |
| 追溯列 | Task 无 design/adr/baseline 引用；manifest 无需求/设计/基线列 | 0002/0011/0022 |

## 四、需新建清单（C 项的实体级展开）

TechnicalDesign（+当前指针+状态）、DesignRevision（不可变+Git 正文候选）、DesignRequirement(requirement_id, requirement_revision_id)、ADR（六态 proposed/under_review/accepted/rejected/superseded/deprecated 为 S03 V1.0 目标，代码无任何对应）+ADRRevision（备选/理由/后果/决策人）、DesignReview（多评审人/多轮/问题登记+关闭/专家与裁决分离/重新提交）、ImplementationBaseline（不可变快照：设计 Revision 集合+ADR 集合+需求 Revision 集合+仓库基线+验收标准）、Readiness Gate（五条件聚合+冻结+授权）、设计 diff 能力（后端+前端）、S03 确认类 MCP/REST 入口。

## 五、判定不变式声明

- 本清单所有 A 判定均经「现有实现满足目标要求且不改变业务结构」检验（仅 2 项通过）；
- B/C 判定均有具体缺口证据（§三/§四），无降格归类；
- D 未使用：15 项均有足够静态证据定级，无需暂不确定项。
