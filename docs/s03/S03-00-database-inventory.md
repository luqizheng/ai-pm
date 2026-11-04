# S03-00 · 数据库资产清单（Database Inventory）V1.1

> V1.1：收编 R1（需求正文口径）/R7（迁移编号推导性）——修订记录见 [S03-00-v1.1-revision-log.md](S03-00-v1.1-revision-log.md)。

基线：`feat/s01-implementation` @ `3ccb42f`。全部证据=migrations/ 目录（33 个文件，0001~0034，**0029 空号**）。

## 1. Migration 文件全清单

```
0001_core.sql                    0018_principal_identity.sql
0002_delivery.sql                0019_tenant.sql
0003_platform.sql                0020_agent_run_identity.sql
0004_audit_source.sql            0021_task_dependency.sql
0005_audit_notify.sql            0022_delivery_manifest.sql
0006_knowledge_documents.sql     0023_run_executor_hardening.sql
0007_llm_provider.sql            0024_knowledge_section_index.sql
0008_repositories.sql            0025_discovery.sql
0009_repo_import.sql             0026_delivery_review.sql
0010_task_state_machine.sql      0027_devenv.sql
0011_run_dispatch.sql            0028_run_execution_mode.sql
0012_task_run_event_idempotency.sql  0030_project_member_role.sql
0013_policy_scope.sql            0031_s01_discovery_import_v2.sql
0014_knowledge_v2.sql            0032_s01_git_capability.sql
0015_task_contract.sql           0033_s01_git_access_simplify.sql
0016_policy_composition.sql      0034_s01_command_center_indexes.sql
0017_customer_master_data.sql
```

一致性：序列连续无重复，唯一缺口 0029（预留后撤销）；`integration.git_capability_grant` 于 0032 建表、**0033:26 DROP**（现存 create 语句来自 0032，查询 information_schema 时该表不存在）。

## 2. Schema 与表全清单（70 张表，含 1 张已撤；1 个视图）

### core（9）
| 表 | 建于 | 说明 |
|---|---|---|
| core.tenant | 0001 | 租户 |
| core.project | 0001 | 项目 |
| core.principal | 0001 | 主体（kind: human/service 等） |
| core.principal_identity | 0018 | 主体身份（provider,tenant,subject→principal） |
| core.tenant_member | 0001 | 租户成员（role owner/admin/member） |
| core.project_member | 0001+0030 | 项目成员（0030 补 role CHECK 六值：owner/member/ai_pm/architect/developer/tester） |
| core.customer | 0017 | 客户主数据 |
| core.tenant_agent | 0020 | 租户 Agent 配置 |
| core.project_initialization_operation | 0031:42-43 | 项目初始化操作（单 active） |

### delivery（20）
| 表 | 建于 | S03 相关度 |
|---|---|---|
| delivery.requirement | 0002:3-14 | **核心复用**（8 列+current_revision_id 指针；status 无 CHECK；无类型/created_by） |
| delivery.requirement_revision | 0002:16-34 | **核心复用**（11 列，state CHECK draft/approved/superseded；content JSONB=当前运行时正文读写源【与 S02 正式裁决（Markdown+Git 唯一权威）存在架构偏差，不得提升为目标权威】；UNIQUE(requirement_id,version)） |
| delivery.task | 0002:51-66+0010 | 状态机 13 值（0010:149-153）+revision 乐观锁+superseded_by_task_id |
| delivery.task_requirement | 0002:68-80 | **形态参照**（PK(task_id,revision_id)+复合 FK） |
| delivery.task_acceptance_criterion | 0015:25 | 验收标准 |
| delivery.task_knowledge_ref | 0015:42 | 知识引用 |
| delivery.task_dependency | 0021:9 | 依赖 |
| delivery.task_waiting | 0010:90 | 等待（单活跃唯一索引） |
| delivery.task_supersession | 0010:73 | 取代链 |
| delivery.task_policy_override | 0016 | 策略覆盖 |
| delivery.policy_scope | 0013 | 策略范围 |
| delivery.task_repository | 0011:86 | **Task↔仓库（实施范围）**+write_access |
| delivery.task_run | 0010:18-41+0011/0013/0016/0019/0020/0022/0023/0026/0028 | Run 主体（attempt_no 分轨、policy_snapshot_hash 0032:15、execution_mode 0028:15-17、run_kind/review_verdict 0026） |
| delivery.task_run_repository | 0011:108-121 | 每仓基线（base_ref/base_commit/workspace_path） |
| delivery.task_repository_commit | 0022:51-81 | 交付 commit（FK RESTRICT；branch/sha/author/changed_files/test_evidence） |
| delivery.task_run_failure | 0011 | 失败记录 |
| delivery.task_run_cleanup | 0011 | 清理 |
| delivery.milestone | 0002:47-60 | 里程碑（乐观锁 revision） |
| delivery.review_finding | 0026:165-186 | 评审发现（append-only 触发器 0026:265-266） |
| delivery.acceptance_criterion_verdict | 0026:~200 | 验收条目裁决（append-only） |

视图：`delivery.v_task_delivery_manifest`（0022:105-181；0026:88-159 重建）——列：task_id/project_id/repository_id/repository_name/write_access(delivery_required)/latest_run_id/latest_attempt_no/integration_status/latest_submitted_run_id/base_commit/commit_sha/changed_files/test_evidence/branch/commit_author/commit_authored_at/expected_delivery_count/actual_delivery_count/delivery_state（四态：expected=0→delivered；actual=expected→delivered；0<actual<expected→partial；actual=0→never_submitted）。**无需求/设计/ADR/基线列**（需求追溯须经 task_id 间接 Join task_requirement）。

### ai（15）
| 表 | 建于 | 说明 |
|---|---|---|
| ai.project_document | 0006:7-20 | 知识文档（doc_type CHECK 4 值 requirement/decision/architecture/knowledge 0006:11-13；**无正文列**=Git 权威；UNIQUE(project_id,doc_key/canonical_path/id)） |
| ai.document_revision | 0006:22-39 | Git 账本（draft/published/rejected/rolled_back；git_commit_sha NOT NULL；revision_no；content_hash；UNIQUE(document_id,revision_no/id)） |
| ai.document_proposal | 0014:21-51 | 八态提案（proposed_markdown 候选正文；redline_kind/confirmed_by/at；resulting_revision_id；**draft/quality_gates_passed/confirmed 三态无代码写入路径**） |
| ai.document_index_state | 0014+0024 | 检索索引态（派生） |
| ai.document_section_index | 0024 | 章节索引 |
| ai.knowledge_reference | 0014:82-104 | 引用（方向：analysis_run/event/manual→文档+修订+章节；reference_kind evidence/decision/constraint） |
| ai.knowledge_write_operation | 0024:43 | 知识写幂等账本 |
| ai.change_proposal | 0003:128-154 | AI 变更提案（7 态 CHECK，expired 为死态；target_type/target_id/expected_revision/command_type/proposed_data JSONB/rationale/evidence/risk_level/confidence；UNIQUE(analysis_run_id,proposal_seq)） |
| ai.analysis_run | 0003+0020 | AI 分析运行 |
| ai.agent_definition / ai.agent_feedback / ai.skill_definition / ai.project_skill_binding / ai.project_ai_config / ai.llm_provider | 0003/0007/0020 | AI 配置族 |

### governance（5）
| 表 | 建于 | 说明 |
|---|---|---|
| governance.approval_request | 0003:157-168+0019:138,374-375 | 审批（**proposal_id UNIQUE FK→change_proposal 1:1 硬绑**；status pending 无 CHECK；required_role/decided_by/decision_reason/decided_at） |
| governance.audit_log | 0003:188-202+0004+0005:30-31+0018:25-28+0019+0027 | 审计（before_data/after_data/compensation/source_kind/source_ref/actor 四列含 auth_mode/claimed_device_id；INSERT 触发器 0005） |
| governance.tenant_audit_log | 0019 | 租户审计 |
| governance.review_approval | 0026:223-249 | 交付复核审批（kind 3 值；三重锚定 review_run+delivery_run+manifest_hash；pending 单向触发器 0026:273-296；uq_review_approval_pending 0026:250-251） |
| governance.human_intervention | 0003 | 人工干预 |

### integration（11）
| 表 | 建于 | 说明 |
|---|---|---|
| integration.inbound_event | 0003:13-17+0012+0019 | 入站事件（source/event_type/idempotency_key/payload/payload_hash/status；UNIQUE(project_id,source,idempotency_key)） |
| integration.job | 0003 | 作业（ai_analysis 等） |
| integration.outbox_event | 0003+0019 | Outbox（UNIQUE(tenant_id,idempotency_key)；SKIP LOCKED 消费） |
| integration.notification | 0003:40-49 | 通知 |
| integration.project_repository | 0008:18-32+0009:14-21 | 仓库（UNIQUE(project_id,url)；role 4 值；status 4 值 pending/verified/unreachable/archived；head_commit_sha） |
| integration.project_ssh_key | 0008:6-16 | SSH 密钥（一项目一把，密文） |
| integration.repo_import | 0009 | 导入（10 值状态机 queued→…→applied｜rejected/failed/cancelled；每项目一个进行中） |
| integration.repo_import_source | 0031 | 导入源（head_commit_sha write-once 0031:68） |
| integration.repo_import_draft_version | 0031:46-58 | 草案版本（generation_status 3 值） |
| integration.repo_import_source_attempt | 0031:88 | 导入尝试 |
| integration.git_capability_grant | 0032→**0033:26 DROP** | 已撤（六能力授权模型作废） |

### discovery（8）
discovery_session / discovery_question / discovery_answer / discovery_message / discovery_finding（kind+verification 四值）/ discovery_finding_evidence（含 commit_sha 0031:128）/ discovery_finding_repository（0025:67）/ discovery_proposal（pending/approved/rejected/conflict）——均 0025+0031。

### devenv（2）
devenv.device（0027）/ devenv.device_project_binding（0027，组合 FK 引 tenant_member PK）。

## 3. S03 相关关键约束汇总（唯一性/不可变性）

| 约束 | 位置 |
|---|---|
| requirement_revision UNIQUE(requirement_id,version)、UNIQUE(requirement_id,id) | 0002:30-31 |
| task_requirement PK(task_id,revision_id)+复合 FK (requirement_id,revision_id) | 0002:70-79 |
| **无「同需求至多一 approved 修订」约束**（并发双 approved 无 DB 兜底） | 缺失 |
| project_document UNIQUE(project_id,doc_key/canonical_path/id) | 0006:17-19 |
| document_revision UNIQUE(document_id,revision_no/id) | 0006:35-36 |
| change_proposal UNIQUE(analysis_run_id,proposal_seq) | 0003:151 |
| approval_request.proposal_id UNIQUE | 0003:160 |
| inbound_event UNIQUE(project_id,source,idempotency_key) | 0003:17 |
| outbox_event UNIQUE(tenant_id,idempotency_key) | 0019 |
| review_approval：uq_review_approval_pending、attempt 分轨唯一、once-written/pending 单向触发器 | 0026:62-79,250-251,273-296 |
| tenant 化 UNIQUE(tenant_id,id) 批量 | 0019:157-167 |
| review_finding/verdict 禁 UPDATE 触发器 | 0026:257-268 |

## 4. 运行时登记表口径

`_sqlx_migrations` 为部署产物：本机演示栈 `infrastructure-aipm-pg-1`（只读 SELECT 抽查）=28 条（0001~0028），构建早于 S01 批次（0030~0034），与代码基线脱钩属预期；0031 升级演练有提交背书（`bcb169f`）。静态编号事实=最大 0034、0029 空号；**0035 仅为当前 HEAD `3ccb42f` 下的顺序推导，不是永久确定的迁移编号**——实施时必须重新检查最新合并基线、并行分支与 Migration 登记情况（R7）。
