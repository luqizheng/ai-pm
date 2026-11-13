# S03-00 · 权限与审批清单（Permission and Approval Inventory）V1.1

> V1.1：收编 R2（完整继承 S02-I6-A A* 七条约束，现存偏差标注为实现缺陷）/R3（DEF-02 关联 S02）/R6（审批路线边界）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

覆盖任务书 VF-10/VF-13/VF-14（含尾项：审批依据、事务边界、原状态与新状态）。基线 @ `3ccb42f`。

## 1. 角色与权限码体系

| 层 | 事实 | 证据 |
|---|---|---|
| DB 项目角色 | CHECK 六值 owner/member/ai_pm/architect/developer/tester | migrations/0030_project_member_role.sql:7-13 |
| 代码项目角色 | ProjectMemberRole{Owner,Member}+is_manager（**DB 六值在代码收敛为两档**） | pm-domain/project_member.rs:14-41 |
| 租户角色 | TenantRole{Owner,Admin,Member} | pm-domain/tenant.rs:19-58 |
| 消费点 | require_project_member（仅成员资格，authz.rs:33-57）；caller_is_manager=项目 owner 或租户 admin（project_member.rs:174-208）；grant 仅 human（:265） | — |
| Permission Code | 定义=JWT permissions claim（auth.rs:107-135）；**仓库内无权威清单**；消费全集=PM_MCP_READ（auth.rs:556）+PM_MCP_DOCUMENT_WRITE（auth.rs:562-568，5 个 MCP 写工具）；PM_MCP_TASK_UPDATE 已定义未分配（:561）；**REST 审批/确认/定稿端点零权限码校验** | — |
| 策略角色 | policy yaml required_role（release_manager/account_owner/authorized_acceptor/project_owner/platform_admin/quota_approver）**无角色实体支撑、无裁决人比对消费**（policy_engine.rs:300-322 只用于标记需预确认） | policies/ai-pm-ai-lead-v2.yaml:16-39 |

## 2. 身份链与真人门体检

**身份链**：三层中间件 require_bearer→resolve_principal→require_active（routes.rs:341-353，默认 jwks，main.rs:44）；MCP 侧 caller_from_ctx 读 rmcp extensions→HumanActor/DeviceClaim（pm-mcp/caller.rs:29-46；HumanActor 唯一生产点 auth.rs:178-186,247-248）。

**两个 P0 根因**：
1. **api_key 令牌链路 provisioning 无条件写 `kind='human'`**（auth.rs:486-494，本人复核：`INSERT INTO core.principal ... VALUES ($1, 'human', ...)` 无条件分支）+ service token 拒绝仅在部分路径（auth.rs:238-240）。
2. **auth_mode 三分类存在但零准入消费**（auth.rs:121-131；唯一消费=写入 audit 列 0018:25-28）→真人 key 经 MCP 使用不可区分（MCP/REST token 同源同一 JWT/JWKS，auth.rs:190-220）。

**继承治理约束（S02-I6-A R-I6A-1 用户裁决 A*，V1.2 收编）——本清单评审真人门的标尺**：

1. 正式确认只能通过可信的人类交互式 REST/UI 入口。
2. MCP 不提供正式确认能力。
3. Agent 即使持有真人 MCP key，也不能因此获得 REST 正式确认权。
4. 无法证明请求来自可信人类交互式操作时，必须拒绝确认。
5. `kind='human'` 不能单独作为真人身份和操作可信性的证明。
6. 身份认证与业务授权必须分别校验。
7. DeviceClaim 不得单独作为授权凭据。

**对照结论：现有身份模型不能满足 S02-I6-A A* 的正式确认安全要求。**上列 P0 根因、下表各"无真人门/身份自报"入口、以及现存 MCP confirm/veto 入口，均为**与 A* 相悖的现存实现缺陷**（不得解释为目标能力）；修复沿 S02 既有治理线（含 step-up 确认票据建议），S03 不重复建立同义规则。另两处 A* 对照：认证与授权未分别校验（REST 审批/确认端点零权限码，违反 A* ⑥）；DeviceClaim 现仅作不可信声明记录（与 A* ⑦一致，无违规消费点）。

**真人门逐入口体检表**（真人门原语=principal.kind='human'：support.rs:4-17、discovery/web.rs:199-209）：

| 入口 | 范围校验 | 身份来源 | 真人门 | AI(api_key) 能否过 |
|---|---|---|---|---|
| POST /v1/pm/review-approvals/{id}/decision | 项目成员（pm_reviews.rs:105） | 认证 principal | 无 | **能**（→任务可 completed decide.rs:596-613） |
| POST /v1/pm/approvals/{id}/decide | 仅租户（pm_writes.rs:85-91） | 认证 principal | 无 | **能** |
| POST /api/v1/supervision/approvals/{id}/decision | 无（approval.rs:24-32） | **body.actor_id** | 无 | **能，可冒充** |
| POST /api/v1/supervision/veto | 无 | body.actor_id | 无 | **能** |
| POST /api/v1/supervision/freeze、unfreeze | 无 | body.actor_id | 无 | **能** |
| POST /v1/pm/documents/proposals/{id}/confirm | 项目成员（documents.rs:277） | **body.confirmed_by**⚠ | require_human | **能**（自报者亦 kind='human'） |
| POST /v1/pm/documents/revisions/{id}/veto | 项目成员（documents.rs:302） | **body.vetoed_by**⚠ | require_human | **能** |
| MCP pm_confirm_document_proposal | 成员+双权限码 | 认证 caller（tools.rs:504） | require_human | **能**——【现存实现缺陷：A* 目标=MCP 无正式确认入口】 |
| MCP pm_veto_document_revision | 同上 | 认证 caller（tools.rs:527） | require_human | **能**——【同类缺陷】 |
| POST discovery findings/{fid}/verification | 项目成员（pm_discovery.rs:299） | 认证 principal | kind 二次校验（web.rs:205） | **能** |
| POST requirements/{rid}/revisions/{rvid}/approve | 仅租户（pm_writes.rs:60-65） | 认证 principal | 无 | **能** |
| 命令通道红线自我执行 | — | — | RequirePreConfirmation（policy_engine.rs:301-303）+executor 仅认 Human+RedlineApproval（approval.rs:86-101） | **不能（唯一真闸）** |

**任务书 VF-13 特别验证题**：「AI Agent 尝试批准 Design Revision→系统拒绝」——**验证对象不存在（N/A）**：无 Design Revision 实体与批准入口（见领域清单 §6）；技术上最接近路径=技术设计提交→change_proposal→红线审批（上表 2/3 行，api_key 可过）。

**越权风险登记**：R1（P0）REST confirm/veto 身份冒充（documents.rs:286,311）；R2（P0）supervision actor_id 自报+无租户/成员过滤+审计写冒认身份（routes.rs:410/464；approval.rs:13-17,24-32；veto.rs:182）；R3（P0）token 同源+auth_mode 零消费；R4（P1）auth-off dev-off owner 后门（auth.rs:299-359）；R5（P1）required_role 无比对；R6（P2）confirm 与 submit 共用同一权限码。完整 RSK-1~10 见主报告 §八与 reuse 清单。

## 3. 四张审批实体（DDL 摘要+状态机+入口）

### 3.1 governance.approval_request（0003:157-168+0019）
- 列：id/project_id/`proposal_id UNIQUE FK→ai.change_proposal`（**1:1 硬绑**）/status（pending，无 CHECK）/required_role/decided_by/decision_reason/decided_at。
- 状态机：pending→approved|rejected（守卫 UPDATE...WHERE status='pending'，supervision/approval.rs:39-56）。
- 创建：仅 orchestrator 红线路由（orchestrator.rs:589-598）；入口：POST /api/v1/supervision/approvals/{id}/decision + POST /v1/pm/approvals/{id}/decide。
- Approve 语义硬编码=反序列化 proposed_data 执行 ProjectCommand（approval.rs:89-113）→proposal 'executed'；Reject→'vetoed'。**无对象类型字段（无 (object_type,object_id) 多态列）**；间接对象集=command_target {task,requirement,milestone,project,audit_log}（orchestrator.rs:695-728）。**边界（R6）：其 1:1 绑定语义不得直接复用为通用设计审批机制；五类业务决策（Requirement Approval/Design Approval/ADR Acceptance/Design Review Decision/Implementation Baseline Freeze）的结构路线留待 S03-05 论证。**

### 3.2 ai.change_proposal（0003:128-154）
- 列：target_type/target_id/expected_revision/command_type/proposed_data(JSONB)/rationale/evidence/risk_level/confidence+status CHECK 7 态。
- 状态机逐态触发：proposed（orchestrator.rs:440-477）→executed（AutoApply :271-277/approve 后 approval.rs:104-113）｜redline_pending（:602）｜escalated（:630，**无审批行仅通知+veto**）｜vetoed（approval.rs:60 等）｜failed（:316-318,125）｜**expired 仅 DDL:146，代码零写入**。executed 后否决走补偿链不改状态（supervision/mod.rs:8-20）。

### 3.3 ai.document_proposal（0014:21-51）
- 列：document_id 可空/doc_type 4 值/proposed_markdown（候选正文非第二权威 0014:5-7）/base_revision_id/quality_gate_result/redline_kind/redline_confirmed_by,at/resulting_revision_id/status CHECK **8 态** default 'draft'。
- 八态实际触发：redline_pending（submit.rs:140-146）｜commit_pending（同处）｜rejected（同处+rejection_reason）｜committed（finalize_committed veto.rs:286，回填 document_id+resulting_revision_id）｜rolled_back（veto.rs:107+revision rolled_back+Git revert+指针回退）；**draft/quality_gates_passed/confirmed 三态无代码写入路径**（确认红线只写 redline_confirmed_by/at，submit.rs:361-368，状态直跳 committed）。
- 发布语义：质量门过且无红线→平台进程内 git CLI 自动 commit（submit.rs:242-249）→published 修订→索引重建；红线不写 Git 待 confirm；Goal 提案强制红线（submit.rs:103-110）。**发布成功≠设计批准**（S03 V1.0 关键约束在代码上的对应事实）。

### 3.4 governance.review_approval（0026:223-249）
- 列：kind CHECK ('auto_accept_gate','manual_review','review_degraded')/status pending/approved/rejected/decided_by/decision_reason/锚定三元组（review run+delivery run+manifest_hash）。
- 形态：once-written+pending 单向迁移触发器（0026:273-296）；同 task 至多一个 pending（0026:250-251 uq_review_approval_pending）。
- 写链 review/decide.rs:460-498；裁决 decide.rs:508-610（锚定回读校验+E2 复核；第二次→AlreadyDecided）。
- **专表形态的现行先例（结构路线本身留待 S03-05 设计评审与批准切片论证，本清单不预设）**。

### 四类审批区分
| 类别 | 结论 | 实体 |
|---|---|---|
| 文档发布审批 | 已存在 | document_proposal 红线管线+veto |
| 普通变更审批 | 已存在（限 AI 提案红线条目） | approval_request 1:1→change_proposal；交付侧 review_approval |
| 技术设计批准 | 不存在 | 无设计实体 |
| ADR 正式接受 | 不存在 | 无 ADR 实体 |

## 4. VF-14 并发和审计（含尾项）

**乐观锁**【✅】：delivery.task/milestone.revision+requirement.row_revision（0002:10,47,60）；apply_task_transition 行锁+expected_revision+revision+1（transition.rs:34-115）；RevisionConflict（executor.rs:297,1010）；REST 任务编辑 409（pm_write/task.rs:100-109）；FOR UPDATE 30+ 处（executor.rs:136,281,825,849,874,991；transition.rs:58；project_member.rs:135-517；discovery/*；outbox.rs:112 SKIP LOCKED）。**例外：REST 定稿路径无锁（revision.rs:84-120）**。

**Revision 唯一约束**【✅ 清单】：requirement_revision UNIQUE(requirement_id,version)/(requirement_id,id)（0002:30-31）；project_document UNIQUE(doc_key/canonical_path/id)（0006:17-19）；document_revision UNIQUE(revision_no/id)（0006:35-36）；change_proposal UNIQUE(analysis_run_id,proposal_seq)（0003:151）；approval_request.proposal_id UNIQUE（0003:160）；review 族 uq_*（0026:62-79,250-251）；inbound_event UNIQUE(project_id,source,idempotency_key)（0003:17）；outbox UNIQUE(tenant_id,idempotency_key)（0019）。**无「同需求至多一 approved」约束**。

**审批幂等性**【✅ 三套实现】：I2-C operation_id→inbound_event ON CONFLICT+重放零写入+同值 no-op（web.rs:169-197,228-237）；review_approval 第二次→AlreadyDecided 200（decide.rs:519-534）；supervision 原子 WHERE 守卫→already:true（approval.rs:39-56；routes.rs:481-485）；confirm/veto operation_id 账本（submit.rs:301-359；veto.rs:28-37）。

**并发双批准**【🟡】：review_approval=行锁+锁后重读+DB 触发器三重（decide.rs:520-534；0026:273-296）→输家 AlreadyDecided；supervision=原子守卫。**残留**：REST 定稿 TOCTOU（不同修订并发定稿可短暂双 approved，无唯一约束兜底）；同修订重复定稿 409；执行器路径三重 FOR UPDATE（executor.rs:823-880）。

**审计事件**【✅】：audit_log 全列=0003:188-202（before_data/after_data/compensation/action/target）+0004（source_kind/ref）+0018:25-28（actor_identity_id/client/auth_mode）+0027（claimed_device_id）+0019（tenant_id）+0005:30-31 INSERT 触发器；写入=业务事务直写（command_executor/audit.rs:26；review/queue.rs:360-387 source_kind='review'；pm_write/audit.rs:48-102）+outbox RunCompletedHandler（advisory lock+条件 INSERT 幂等，outbox_handlers.rs:219-284）；outbox 注册事件=TaskReady/DeliverySubmitted/RunCompleted（有 handler）+ReviewCompleted/KnowledgeBaseWriteRequested（登记不产，outbox.rs:100-131；装配 outbox_handlers.rs:298-306）。

**操作人/操作时间**【✅】：actor 四列（actor_identity_id/actor_client/actor_auth_mode+actor principal 0003:191）；created_at 默认 now（0003:201）。

**原状态与新状态（before/after）**【🟡 口径不统一】：before_data 列存在；有写入点=任务迁移 before={status,revision}（transition.rs:114-115）、finding.decided（web.rs:294-306）、执行器命令审计（executor.rs:950-961 before/after+compensation）；**只写 after 三处**=review 审计（queue.rs:368-385）、pm_write 通用审计（pm_write/audit.rs:48-102）、RunCompleted（outbox_handlers.rs:238-250）。

**审批依据**【🟡 各实体形状不一】：approval_request.decision_reason+required_role；change_proposal.rationale/evidence/risk_level/confidence+expected_revision（乐观依据）；document_proposal.quality_gate_result/redline_kind+base_revision_id（版本依据）；review_approval.decision_reason+manifest_hash（内容锚定）；**需求定稿无审批依据字段**（approve_revision 无 reason 参数，revision.rs:77-141）；策略 required_role 无裁决人比对。

**事务边界**【✅ 单事务纪律，一处例外】：定稿 REST=begin→3×UPDATE→audit→commit 单事务（revision.rs:83-130）；执行器定稿=三重 FOR UPDATE 单事务+补偿记录（executor.rs:823-969）；decide_approval 单事务原子迁移+同事务执行命令（approval.rs:39-113）；红线确认/veto=operation_id 账本单事务（submit.rs:301-359；veto.rs:28-37，veto 另有 Git revert 在事务外的两段语义）；finalize_committed=advisory lock+单事务（veto.rs:205-286）；ingestion=事件+job 单事务（ingestion.rs:68-110）；review decide=行锁单事务（decide.rs:520-534）；**例外=I1 归档的 P2 逐 Run cancel 在主事务外补偿执行**（archive.rs:147-165，有显式设计注释）；知识写并发=KnowledgeWriteCoordinator advisory lock+operation_id（E-2）。
