# PC Gate 0 — 数据库 Schema 核查资料清单（Database Schema Inventory）

> 状态：已完成核查（只读核查，未执行/未修改任何 SQL）
> 用途：Product Cognition（PC）研发前置核查 Gate 0 输入资料
> 结论口径：仅以仓库内 Migration SQL 为依据；推测一律标注「建议」或「待核实」

---

## A. 核查元信息

| 项 | 值 |
|---|---|
| 仓库名称 | ai-pm（`D:\kudi-project\templates\ai-pm`） |
| 当前分支 | `master` |
| 当前 HEAD Commit | `36801388b4f131a8292012c16e9b83e0e07bb3d5`（2026-11-06 20:48:50 +0800） |
| Migration 目录 | `migrations/`（相对仓库根） |
| 核查日期 | 2026-11-06 |
| Migration 文件数量 | 44 个 `.sql` 文件（编号 0001–0044；**缺 0029**；**0044 同号双文件**，见 B.2） |
| 迁移执行机制 | `sqlx::migrate!("../../migrations")`（crates/pm-infrastructure/examples/seed_test_template.rs:73），按文件名字典序执行 |

**核查范围**：`migrations/*.sql` 全量静态阅读；CREATE TABLE / ALTER TABLE / CREATE INDEX / ADD CONSTRAINT / DROP / RENAME / 视图 / 触发器 / 种子数据。

**排除范围**：业务实现代码（Rust crates）、设计文档中的示例 DDL（如 `docs/ai-pm-project-discovery-v1.md` 附录 A——其引用表清单已证实与 0019 实际迁移不一致，见 0025:109-112 注释）、运行数据库实况。

---

## B. Migration 清单

### B.1 全量清单（按字典序 = 实际执行顺序）

| Migration 文件 | 主要结构变更 | 涉及表 | 与 PC 的关系 |
|---|---|---|---|
| 0001_core.sql | 建 5 个 schema（core/delivery/integration/ai/governance）；core.customer/project/principal/project_member | core.* | PC 的项目锚点 `core.project` |
| 0002_delivery.sql | requirement/requirement_revision/milestone/task/task_requirement | delivery.* | PC-0005 Requirements 侧对象；revision 带 content_hash |
| 0003_platform.sql | inbound_event/job/notification/project_ai_config/skill_definition/project_skill_binding/analysis_run/**change_proposal**/**approval_request**/human_intervention/audit_log/agent_feedback + 9 索引 | ai.*, governance.*, integration.* | PC-0003 审批载体（初版） |
| 0004_audit_source.sql | audit_log 加 source_kind/source_ref | governance.audit_log | PC-0003 审计证据锚 |
| 0005_audit_notify.sql | audit_log 插入触发器 pg_notify 'pm_audit' | governance.audit_log | 审计广播，无 PC 结构 |
| 0006_knowledge_documents.sql | **project_document / document_revision**（Git 权威、正文不落库） | ai.* | PC-0002/PC-0004 核心 |
| 0007_llm_provider.sql | 全局 llm_provider（后退役） | ai.llm_provider | 无 |
| 0008_repositories.sql | project_ssh_key/project_repository | integration.* | PC-0005 仓库来源侧 |
| 0009_repo_import.sql | ssh_key/仓库补列；repo_import/repo_import_source | integration.* | 仓库导入事实 |
| 0010_task_state_machine.sql | task 补列；task_run/task_run_failure/task_supersession/task_waiting/task_policy_override/outbox_event；task 13 值 status CHECK | delivery.*, integration.* | 无直接关系 |
| 0011_run_dispatch.sql | task_run 状态/租约列；task_repository/task_run_repository | delivery.* | 无直接关系 |
| 0012_task_run_event_idempotency.sql | task_run.source_event_id + 部分唯一索引 | delivery.task_run | 无直接关系 |
| 0013_policy_scope.sql | policy_scope；task.task_type | delivery.* | 无直接关系 |
| 0014_knowledge_v2.sql | **document_proposal / document_index_state / knowledge_reference**；document_revision 补列 | ai.* | PC-0002/PC-0004 核心 |
| 0015_task_contract.sql | task 契约列；task_acceptance_criterion/**task_knowledge_ref**；run 凭据列 | delivery.* | PC-0005 Task↔Knowledge 关联 |
| 0016_policy_composition.sql | policy_scope 补能力列 | delivery.policy_scope | 无 |
| 0017_customer_master_data.sql | customer 主数据列 | core.customer | 无 |
| 0018_principal_identity.sql | principal_identity；audit_log 补身份列 | core.*, governance.* | 无 |
| 0019_tenant.sql | **多租户改造：39 表加 tenant_id、15 表 UNIQUE(tenant_id,id)、拆单列 FK→组合 FK、I4 全局唯一改租户内** | 全 schema | 所有 PC 相关表的隔离基线 |
| 0020_agent_run_identity.sql | agent_definition/tenant_agent；task_run 身份三列 | ai.*, core.*, delivery.* | 无 |
| 0021_task_dependency.sql | task_dependency | delivery.* | 无 |
| 0022_delivery_manifest.sql | task_run 补键/列；task_repository_commit；交付对账视图 | delivery.* | 无 |
| 0023_run_executor_hardening.sql | task_run 四列 + 索引 | delivery.task_run | 无 |
| 0024_knowledge_section_index.sql | **document_section_index / knowledge_write_operation** | ai.* | PC-0004 派生索引与写幂等 |
| 0025_discovery.sql | discovery schema 7 表 + repo_import 关联 | discovery.*, integration.* | PC-0001/PC-0006 认知事实来源 |
| 0026_delivery_review.sql | task_run review 列；review_finding/acceptance_criterion_verdict/**review_approval** + 不可变触发器 | delivery.*, governance.* | PC-0003 审批不可变先例 |
| 0027_devenv.sql | devenv schema 设备域；audit 补 claimed_device_id | devenv.*, governance.* | 无 |
| 0028_run_execution_mode.sql | task_run.execution_mode | delivery.task_run | 无 |
| 0030_project_member_role.sql | project_member.role CHECK | core.project_member | 无 |
| 0031_s01_discovery_import_v2.sql | 发现域跨 Project 约束强化；import 草案版本/attempt/evidence 实体 | discovery.*, integration.*, core.* | PC-0006 认知事实结构升级 |
| 0032_s01_git_capability.sql | git_capability_grant（后被撤） | integration.* | 无 |
| 0033_s01_git_access_simplify.sql | **DROP git_capability_grant** | integration.* | 无 |
| 0034_s01_command_center_indexes.sql | audit_log/finding 项目级索引 | governance.*, discovery.* | 无 |
| 0035_s01_run_cleanup.sql | task_run_cleanup | delivery.* | 无 |
| 0036_s01_cognition.sql | **cognition_run / cognition_item / project_knowledge** | ai.* | **PC-0001 核心** |
| 0037_s01_baseline.sql | **baseline_proposal / baseline_item / project_baseline** | ai.* | **PC-0001 核心** |
| 0038_s01_repo_baseline_confirm.sql | 仓库 selected_* 基准列 + repository_version_confirmation | integration.* | 基线引用的 commit 事实 |
| 0039_s02_requirement_core.sql | requirement 状态改造；revision 五态 + markdown_version_ref；requirement_confirmation | delivery.* | PC-0005/PC-0004 需求侧 |
| 0040_s02_i6a_approver_grant.sql | **approver_grant**（显式审批人授权） | core.* | PC-0003 审批人基座 |
| 0041_team_llm_provider.sql | team_llm_provider/credential/call_record；**DROP ai.llm_provider**；tenant_audit_log 补列 | ai.*, governance.* | 无 |
| 0042_s02_i2_projection.sql | knowledge_write_operation.command 扩第五值 | ai.knowledge_write_operation | PC-0004 写入幂等扩展 |
| 0043_project_ai_model_policy.sql | project_ai_model_policy | ai.* | 无 |
| 0044_s02_i3_source_traceability.sql | source schema source_input；requirement_source_input/requirement_finding | source.*, delivery.* | PC-0005 追溯关联 |
| 0044_team_onboarding.sql | tenant 档案列；tenant_invitation/link/redemption | core.* | 无 |

### B.2 迁移编号异常（已确认事实）

1. **缺 0029**：目录无 `0029_*.sql`，无占位说明文件；`docs/ai-pm-migration-ledger.md` 存在（未逐条核对账本与目录一致性——待核实 H-14）。
2. **0044 同号双文件**：`0044_s02_i3_source_traceability.sql`（commit 5936105，S02-I3 线）与 `0044_team_onboarding.sql`（commit 813cc97 合并，Team Onboarding 线）。两者文件名不同、版本号相同（44）。sqlx 按字典序执行：`0044_s02_i3` 先于 `0044_team_onboarding`（'s' < 't'）。两个文件互不引用对方对象，字典序任意颠倒均不破坏依赖。**风险**：sqlx 对同版本号双文件的 `_sqlx_migrations` 记账/校验行为未在本次范围内验证（见 H-13）。后续新迁移应取 **0045** 起号，禁止再用重复号。

### B.3 结构变更中无法静态确定的部分

- 0011:7-24、0012:12-26、0011:65-80：`DO $$` 块按 `pg_constraint` 元数据动态 DROP CHECK、执行存量核查 RAISE——结构变更本身静态可读，但**依赖执行时点的库内状态**。
- 0017:28-36：`UPDATE core.customer` 数据回填（仅影响 status 标记，非结构）。
- 0039:33-34：`UPDATE delivery.requirement` 防御性归一（非结构）。
- 0031:113-117、174-179、194-199、213-218：4 处一次性 `UPDATE ... SET project_id` 结构对齐（非结构）。

---

## C. 六项核查结果

### C.1 PC-0001 项目知识与项目基线

**结论先行**：四张目标表全部存在于 `ai` schema，且全部由 `0036_s01_cognition.sql` / `0037_s01_baseline.sql` 一次性创建，后续无任何 Migration 修改过它们的结构（0019 早于 0036/0037，不适用）。**没有 `project_knowledge` 之外的语义相近表承载「项目知识」；`baseline_*` 三表即项目基线的完整模型。**

#### C.1.1 `ai.cognition_run`（0036_s01_cognition.sql:7-27）

认知 Run（一次项目认知建立过程）。

- 主键 `id UUID`（0036:8）；`UNIQUE (tenant_id, id)`（0036:25）。
- 外键：`(tenant_id, project_id) → core.project(tenant_id, id)`（0036:26，NO ACTION，无级联）；`tenant_id → core.tenant(id)`（0036:9）；`started_by → core.principal(id)` NOT NULL（0036:22，Q2-C：正式认知由人类启动）。
- 状态：`status` CHECK 7 值 `'pending','collecting','analyzing','completed','partially_completed','failed','cancelled'`，默认 `'pending'`（0036:13-15）。
- 正文/数据：`scope JSONB`、`repo_versions JSONB`（分析范围与仓储基准版本，0036:17-18）；无 Markdown 正文。
- 计数：`item_count`、`confirmed_count`（0036:19-20）。
- 索引：`idx_cognition_run_project (project_id, created_at DESC)`（0036:29-30）。

完整 DDL（0036_s01_cognition.sql:7-27 原文）：

```sql
CREATE TABLE ai.cognition_run (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    -- Run 状态机（§5.1）。V1 只落 pending / completed / partially_completed /
    -- failed / cancelled；collecting / analyzing 属执行器阶段，编排切片启用。
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','collecting','analyzing','completed',
                          'partially_completed','failed','cancelled')),
    -- 分析范围与输入资料（§2.4）：来源类型与仓储基准版本。
    scope JSONB NOT NULL DEFAULT '{}'::jsonb,
    repo_versions JSONB NOT NULL DEFAULT '[]'::jsonb,
    item_count INT NOT NULL DEFAULT 0,
    confirmed_count INT NOT NULL DEFAULT 0,
    -- Q2-C：正式认知由人类启动，AI 不得自行启动（§11）。
    started_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);
```

#### C.1.2 `ai.cognition_item`（0036:32-61）

认知条目（AI 提出的单条认知）。

- 主键 `id UUID`；`UNIQUE (tenant_id, id)`（0036:59）；FK `(tenant_id, project_id) → core.project`（0036:60）。
- `run_id UUID NOT NULL REFERENCES ai.cognition_run(id)`（0036:36，**单列 FK**，见 F-3 风险）。
- `domain` CHECK 6 值：`identity/repository/domain/architecture/convention/evolution`（0036:38-40）。
- 正文：`content TEXT NOT NULL`（0036:41，**存 PG**）；`evidence JSONB DEFAULT '{}'`（0036:44，证据内嵌 JSONB）。
- `confidence` CHECK `high/medium/low`（0036:45-46）；`has_conflict BOOLEAN`（0036:47）。
- 状态：`state` CHECK 6 值 `proposed/confirmed/rejected/investigation/deferred/superseded`（0036:49-51）。
- 确认记录：`decided_by/decided_at/decision_note`（0036:54-56）。
- `knowledge_id UUID`（0036:57，**裸列、无外键、无索引**——确认后指向 ai.project_knowledge，关联仅靠应用层维护）。
- 索引：`idx_cognition_item_run (run_id, state)`（0036:63）。

完整 DDL（0036_s01_cognition.sql:32-61 原文）：

```sql
CREATE TABLE ai.cognition_item (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    run_id UUID NOT NULL REFERENCES ai.cognition_run(id),
    -- 六认知领域（§3）。
    domain TEXT NOT NULL
        CHECK (domain IN ('identity','repository','domain','architecture',
                          'convention','evolution')),
    content TEXT NOT NULL,
    -- 证据来源 + 类型（§4.5）：{type, source, ...}；V1 type ∈
    -- project_record / repository_metadata。
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    confidence TEXT NOT NULL DEFAULT 'medium'
        CHECK (confidence IN ('high','medium','low')),
    has_conflict BOOLEAN NOT NULL DEFAULT false,
    -- Item 状态机（§5.2）。investigation = Request Investigation。
    state TEXT NOT NULL DEFAULT 'proposed'
        CHECK (state IN ('proposed','confirmed','rejected','investigation',
                         'deferred','superseded')),
    -- 确认 / 拒绝记录（§6.3/6.4）：操作人、时间、备注（Edit&Confirm 的修订
    -- 正文落 content 更新，原 AI 提案保存在 decision_note 前缀）。
    decided_by UUID REFERENCES core.principal(id),
    decided_at TIMESTAMPTZ,
    decision_note TEXT,
    knowledge_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);
```

#### C.1.3 `ai.project_knowledge`（0036:66-87）——「已确认认知 = 项目知识」

- 主键 `id UUID`；`UNIQUE (tenant_id, id)`（0036:82）；FK `(tenant_id, project_id) → core.project`（0036:83，NO ACTION）。
- `domain` CHECK 同 6 域（0036:70-72）；正文 `content TEXT NOT NULL`（0036:73，**存 PG**）。
- 来源：`source_item_id UUID REFERENCES ai.cognition_item(id)`（0036:74，**单列 FK**，可空）。
- 版本链：`version INT NOT NULL DEFAULT 1`（0036:75）+ `supersedes_id UUID REFERENCES ai.project_knowledge(id)` 自引用（0036:78）；无 `(project_id, domain, version)` 唯一约束——版本号不强制唯一（同域同 version 可并存，靠 superseded 状态区分）。
- 状态：`status` CHECK `active/superseded/retired`，默认 `active`（0036:76-77）。
- 确认记录：`confirmed_by UUID NOT NULL → core.principal(id)`、`confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now()`（0036:80-81）。
- 索引：`idx_project_knowledge_active (project_id, status, confirmed_at DESC)`（0036:86-87）。

完整 DDL（0036_s01_cognition.sql:66-87 原文）：

```sql
CREATE TABLE ai.project_knowledge (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    domain TEXT NOT NULL
        CHECK (domain IN ('identity','repository','domain','architecture',
                          'convention','evolution')),
    content TEXT NOT NULL,
    source_item_id UUID REFERENCES ai.cognition_item(id),
    version INT NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','superseded','retired')),
    supersedes_id UUID REFERENCES ai.project_knowledge(id),
    -- 确认记录：确认人 + 时间（§2.2 Confirmation Record）。
    confirmed_by UUID NOT NULL REFERENCES core.principal(id),
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);
```

#### C.1.4 `ai.baseline_proposal`（0037_s01_baseline.sql:7-27）

基线提案（一次提案轮次）。

- 主键 `id UUID`；`UNIQUE (tenant_id, id)`（0037:22）；FK `(tenant_id, project_id) → core.project`（0037:23，NO ACTION）。
- `version INT NOT NULL DEFAULT 1`（0037:11，提案自身版本，无唯一约束）。
- 状态：`status` CHECK `pending/confirmed/superseded`（0037:14-15）。
- 来源：`source_run_id UUID REFERENCES ai.cognition_run(id)`（0037:16，**单列 FK**，可空——不强制 run 与提案同项目）。
- 确认记录：`confirmed_by UUID REFERENCES core.principal(id)`（**可空**）、`confirmed_at`（0037:18-19）；`created_by NOT NULL`（0037:20）。
- **部分唯一索引**：`uq_baseline_proposal_pending ON (tenant_id, project_id) WHERE status='pending'`（0037:26-27）——同项目同时至多一个待确认提案（DB 层防并发提案）。

完整 DDL（0037_s01_baseline.sql:7-27 原文）：

```sql
CREATE TABLE ai.baseline_proposal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    version INT NOT NULL DEFAULT 1,
    -- pending：待逐项确认；confirmed：已成为正式基线（条目随即不可变）；
    -- superseded：被更新提案取代。
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','confirmed','superseded')),
    source_run_id UUID REFERENCES ai.cognition_run(id),
    -- 同一项目同时只允许一个待确认提案（变更走新提案，不原地改）。
    confirmed_by UUID REFERENCES core.principal(id),
    confirmed_at TIMESTAMPTZ,
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE UNIQUE INDEX uq_baseline_proposal_pending
ON ai.baseline_proposal (tenant_id, project_id) WHERE status = 'pending';
```

#### C.1.5 `ai.baseline_item`（0037:29-50）

基线条目（提案内逐项）。

- 主键 `id UUID`；`UNIQUE (tenant_id, id)`（0037:48）；FK `(tenant_id, project_id) → core.project`（0037:49）。
- `proposal_id UUID NOT NULL REFERENCES ai.baseline_proposal(id)`（0037:33，**单列 FK**）。
- `category` CHECK 7 值：`identity/scope/technical/architecture/repository/standards/constraints`（0037:35-37）。
- 正文：`content TEXT NOT NULL`（0037:38，存 PG）；`evidence JSONB DEFAULT '{}'`（0037:39）；`confidence` CHECK 同三值（0037:40-41）。
- 状态：`state` CHECK 5 值 `proposed/confirmed/rejected/investigation/deferred`（0037:42-43）；确认记录 `decided_by/decided_at/decision_note`（0037:44-46）。
- 索引：`idx_baseline_item_proposal (proposal_id, state)`（0037:52）。

完整 DDL（0037_s01_baseline.sql:29-52 原文）：

```sql
CREATE TABLE ai.baseline_item (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    proposal_id UUID NOT NULL REFERENCES ai.baseline_proposal(id),
    -- 六基线类别 + 约束（§5 左栏分节）。
    category TEXT NOT NULL
        CHECK (category IN ('identity','scope','technical','architecture',
                            'repository','standards','constraints')),
    content TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    confidence TEXT NOT NULL DEFAULT 'medium'
        CHECK (confidence IN ('high','medium','low')),
    state TEXT NOT NULL DEFAULT 'proposed'
        CHECK (state IN ('proposed','confirmed','rejected','investigation','deferred')),
    decided_by UUID REFERENCES core.principal(id),
    decided_at TIMESTAMPTZ,
    decision_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_baseline_item_proposal ON ai.baseline_item (proposal_id, state);
```

#### C.1.6 `ai.project_baseline`（0037:55-66）——已确认基线版本头

- 主键 `id UUID`；`UNIQUE (tenant_id, id)`（0037:63）；`UNIQUE (tenant_id, project_id, version)`（0037:64，**版本号租户内唯一**）；FK `(tenant_id, project_id) → core.project`（0037:65）。
- `version INT NOT NULL`（0037:59）；`proposal_id UUID NOT NULL REFERENCES ai.baseline_proposal(id)`（0037:60，**单列 FK**）。
- `confirmed_by NOT NULL → core.principal(id)`、`confirmed_at NOT NULL`（0037:61-62）。
- **结构缺失**：project_baseline 只存「版本头」，**没有任何字段或关联表把版本头钉到具体的 baseline_item 集合**（只能经 proposal_id 间接追溯，见 F-2）。

完整 DDL（0037_s01_baseline.sql:55-66 原文）：

```sql
CREATE TABLE ai.project_baseline (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    version INT NOT NULL,
    proposal_id UUID NOT NULL REFERENCES ai.baseline_proposal(id),
    confirmed_by UUID NOT NULL REFERENCES core.principal(id),
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, version),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);
```

#### C.1.7 关键结构事实小结（PC-0001）

| 核查点 | 事实 | 依据 |
|---|---|---|
| 正文存储 | `content TEXT` 直接存 PG；证据以 `evidence JSONB` 内嵌 | 0036:41,44,73；0037:38,39 |
| 版本字段 | knowledge.version + supersedes_id；proposal.version；baseline.version（租户内唯一） | 0036:75,78；0037:11,59,64 |
| 状态字段 | 全部有 CHECK：knowledge 3 值 / proposal 3 值 / item 5-6 值 | 0036:14,50,76；0037:14,42 |
| 来源字段 | source_item_id（knowledge←item）、source_run_id（proposal←run） | 0036:74；0037:16 |
| 创建人/确认人 | created_by / confirmed_by / decided_by 全齐（proposal.confirmed_by 可空） | 0036:22,54,80；0037:18,20,44,61 |
| 与 core.project 关系 | 组合 FK `(tenant_id, project_id)`，NO ACTION（删项目会被阻止） | 0036:26,60,83；0037:23,49,65 |
| 表间关系 | cognition_run 1:N cognition_item；item →knowledge（裸列）；knowledge 自引用 supersede；proposal 1:N item；proposal → baseline（1:1 语义，无 DB 约束强制）；run → proposal（source_run_id） | 0036:36,57,78；0037:16,33,60 |
| 级联删除 | 无 ON DELETE CASCADE/SET NULL（全默认 NO ACTION） | 同上 |
| 防并发 | 仅 `uq_baseline_proposal_pending` 部分唯一索引 | 0037:26-27 |

### C.2 PC-0002 项目文档、Decision、ADR

**结论先行**：文档域由 `ai.project_document`（身份）+ `ai.document_revision`（Git 修订）双表承载；`doc_type='decision'` **只是文档分类值，不存在正式 Product Decision 实体表**；ADR **无独立领域表**。

#### C.2.1 `ai.project_document`（0006_knowledge_documents.sql:7-20；0019 租户化）

- `id UUID PK`；`project_id` 组合 FK `(tenant_id, project_id) → core.project(tenant_id, id)`（0019:316-318）；`UNIQUE (project_id, doc_key)`、`UNIQUE (project_id, canonical_path)`、`UNIQUE (project_id, id)`（0006:17-19）；`UNIQUE (tenant_id, id)`（0019:165）。
- `doc_key TEXT CHECK (doc_key ~ '^[A-Za-z0-9._-]+$')`（0006:10）。
- **`doc_type` CHECK 四值**：`'requirement', 'decision', 'architecture', 'knowledge'`（0006:11-13）——这是全仓唯一 doc_type 枚举定义（document_proposal 同款 0014:27-29；无 ENUM 类型，均为 CHECK）。
- `canonical_path TEXT NOT NULL`（0006:14，知识 Git 仓内规范路径）。
- 无 status 列（文档身份无生命周期；生命周期在 revision/proposal 层）。

完整 DDL（0006_knowledge_documents.sql:7-20 原文；0019 仅追加 tenant_id 列与组合 FK，未动下列结构）：

```sql
CREATE TABLE ai.project_document (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    doc_key TEXT NOT NULL CHECK (doc_key ~ '^[A-Za-z0-9._-]+$'),
    doc_type TEXT NOT NULL CHECK (
        doc_type IN ('requirement', 'decision', 'architecture', 'knowledge')
    ),
    canonical_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, doc_key),
    UNIQUE (project_id, canonical_path),
    UNIQUE (project_id, id)
);
```

#### C.2.2 `ai.document_revision`（0006:22-39；0014:10-18 补列；0019 租户化）

文档修订 = Git commit 的登记行。

- `revision_no INT CHECK (revision_no > 0)`，`UNIQUE (document_id, revision_no)`（0006:26,35）。
- **`git_commit_sha TEXT NOT NULL`**（0006:27）；**`content_hash TEXT NOT NULL`**（0006:28）。
- `status` CHECK 4 值：`draft/published/rejected/rolled_back`（0006:29-31）；`change_summary TEXT`（0006:32）；`created_by NOT NULL → core.principal(id)`（0006:33）。
- 0014 补列：`source_event_id UUID`、`analysis_run_id UUID`、`redline_confirmed_by → core.principal(id)`、`published_at TIMESTAMPTZ`（0014:11-14）。
- 文档身份↔修订关联：`FOREIGN KEY (project_id, document_id) REFERENCES ai.project_document(project_id, id)`（0006:37-38）→ 0019 换为组合租户 FK `fk_document_revision_tenant_project`（0019:319-321）。
- 索引：`idx_document_revision_document (document_id, revision_no DESC)`（0006:41-42）；`idx_document_revision_source_event`（部分，0014:16-18）；`UNIQUE (tenant_id, id)`（0019:166）。

完整 DDL（0006_knowledge_documents.sql:22-42 原文；0014 补 4 列、0019 租户化见上）：

```sql
CREATE TABLE ai.document_revision (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    document_id UUID NOT NULL,
    revision_no INT NOT NULL CHECK (revision_no > 0),
    git_commit_sha TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (
        status IN ('draft', 'published', 'rejected', 'rolled_back')
    ),
    change_summary TEXT,
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_id, revision_no),
    UNIQUE (document_id, id),
    FOREIGN KEY (project_id, document_id)
        REFERENCES ai.project_document(project_id, id)
);

CREATE INDEX idx_document_revision_document
    ON ai.document_revision (document_id, revision_no DESC);
```

0014 追加列（0014_knowledge_v2.sql:10-14）：

```sql
ALTER TABLE ai.document_revision
    ADD COLUMN IF NOT EXISTS source_event_id        UUID,
    ADD COLUMN IF NOT EXISTS analysis_run_id        UUID,
    ADD COLUMN IF NOT EXISTS redline_confirmed_by   UUID REFERENCES core.principal(id),
    ADD COLUMN IF NOT EXISTS published_at           TIMESTAMPTZ;
```

#### C.2.3 `ai.document_proposal`（0014_knowledge_v2.sql:21-57；0019/0025 补键）

- `document_id UUID` **可空**（新文档首次提案时为空，0014:24-25）；`base_revision_id UUID REFERENCES ai.document_revision(id)`（0014:31）；`base_content_hash TEXT`（0014:32）。
- **`proposed_markdown TEXT NOT NULL`**（0014:33，候选正文副本）；`proposed_content_hash TEXT NOT NULL`（0014:34）。
- `doc_type` CHECK 同 0006 四值（0014:27-29）；`canonical_path` 同款（0014:30）。
- `status` CHECK 8 值：`draft/quality_gates_passed/redline_pending/confirmed/commit_pending/committed/rejected/rolled_back`（0014:38-41）。
- 质量门/红线：`quality_gate_result JSONB`、`redline_kind`、`redline_confirmed_by/at`（0014:42-45）。
- 结果回填：`resulting_revision_id UUID REFERENCES ai.document_revision(id)`（0014:46）；`rejection_reason TEXT`（0014:47）；`created_by NOT NULL`（0014:48）。
- 索引：`idx_document_proposal_project_status`（0014:53-54）、`idx_document_proposal_document`（部分，0014:55-57）；`UNIQUE (tenant_id, id)`（0025:111-112 补，0019 清单遗漏）。

完整 DDL（0014_knowledge_v2.sql:21-57 原文）：

```sql
CREATE TABLE IF NOT EXISTS ai.document_proposal (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id             UUID NOT NULL REFERENCES core.project(id),
    -- 新文档首次提案时 document_id 为空，提交后才绑定。
    document_id            UUID REFERENCES ai.project_document(id),
    doc_key                TEXT NOT NULL CHECK (doc_key ~ '^[A-Za-z0-9._-]+$'),
    doc_type               TEXT NOT NULL CHECK (
        doc_type IN ('requirement', 'decision', 'architecture', 'knowledge')
    ),
    canonical_path         TEXT NOT NULL,
    base_revision_id       UUID REFERENCES ai.document_revision(id),
    base_content_hash      TEXT,
    proposed_markdown      TEXT NOT NULL,
    proposed_content_hash  TEXT NOT NULL,
    change_summary         TEXT,
    source_event_id        UUID,
    analysis_run_id        UUID,
    status                 TEXT NOT NULL DEFAULT 'draft' CHECK (status IN (
        'draft', 'quality_gates_passed', 'redline_pending', 'confirmed',
        'commit_pending', 'committed', 'rejected', 'rolled_back'
    )),
    quality_gate_result    JSONB NOT NULL DEFAULT '{}'::jsonb,
    redline_kind           TEXT,
    redline_confirmed_by   UUID REFERENCES core.principal(id),
    redline_confirmed_at   TIMESTAMPTZ,
    resulting_revision_id  UUID REFERENCES ai.document_revision(id),
    rejection_reason       TEXT,
    created_by             UUID NOT NULL REFERENCES core.principal(id),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

#### C.2.4 `ai.document_index_state`（0014:60-79）——PageIndex 索引状态

- `document_revision_id UUID NOT NULL REFERENCES ai.document_revision(id) ON DELETE CASCADE`（0014:63）→ 0019 组合租户化（0019:360-363）。
- `provider TEXT NOT NULL`（0014:64，无 CHECK——provider 值清单不在 DB 层）。
- `external_document_id TEXT`、**`indexed_content_hash TEXT`**、**`index_version TEXT`**（0014:65-67）。
- `status` CHECK 4 值：`pending/indexed/stale/failed`（0014:68-70）。
- **`UNIQUE (document_revision_id, provider)`**（0014:75）——一个修订在一个 provider 下最多一条索引状态。
- **索引版本与文档版本一致性**：`indexed_content_hash` 与 `ai.document_revision.content_hash` 的比对**无 DB 约束**，由应用层判定（stale 状态即其表达），SQL 无法证明比对逻辑。

完整 DDL（0014_knowledge_v2.sql:60-79 原文）：

```sql
CREATE TABLE IF NOT EXISTS ai.document_index_state (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id            UUID NOT NULL REFERENCES core.project(id),
    document_revision_id  UUID NOT NULL REFERENCES ai.document_revision(id) ON DELETE CASCADE,
    provider              TEXT NOT NULL,
    external_document_id  TEXT,
    indexed_content_hash  TEXT,
    index_version         TEXT,
    status                TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'indexed', 'stale', 'failed')
    ),
    last_error            TEXT,
    indexed_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_revision_id, provider)
);
```

#### C.2.5 `ai.knowledge_reference`（0014:82-104）——证据引用

- 钉死修订：`document_id` + `document_revision_id` 双 FK（ON DELETE CASCADE，0014:85-86；0019:364-370 组合化）。
- 章节定位：`section_anchor/section_title/start_line`（0014:87-89）。
- `reference_kind` CHECK 3 值：`evidence/decision/constraint`（0014:90-92）——**decision/constraint 在这里是「引用标签」，不是决策实体**。
- `source_kind` CHECK 3 值：`analysis_run/event/manual` + `source_id UUID`（0014:93-96）。
- 唯一索引：`uq_knowledge_reference_identity (document_revision_id, coalesce(section_anchor,''), source_kind, coalesce(source_id, zero-uuid))`（0014:103-104）。

完整 DDL（0014_knowledge_v2.sql:82-104 原文）：

```sql
CREATE TABLE IF NOT EXISTS ai.knowledge_reference (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id            UUID NOT NULL REFERENCES core.project(id),
    document_id           UUID NOT NULL REFERENCES ai.project_document(id) ON DELETE CASCADE,
    document_revision_id  UUID NOT NULL REFERENCES ai.document_revision(id) ON DELETE CASCADE,
    section_anchor        TEXT,
    section_title         TEXT,
    start_line            INT,
    reference_kind        TEXT NOT NULL DEFAULT 'evidence' CHECK (
        reference_kind IN ('evidence', 'decision', 'constraint')
    ),
    source_kind           TEXT NOT NULL CHECK (
        source_kind IN ('analysis_run', 'event', 'manual')
    ),
    source_id             UUID,
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_knowledge_reference_document
    ON ai.knowledge_reference (document_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS uq_knowledge_reference_identity
    ON ai.knowledge_reference (document_revision_id, coalesce(section_anchor, ''), source_kind, coalesce(source_id, '00000000-0000-0000-0000-000000000000'::uuid));
```

#### C.2.6 Decision / ADR 的结构判定

| 问题 | 判定 | 依据 |
|---|---|---|
| 是否存在正式 Decision 实体表？ | **否**。全 44 个 Migration 无 `ai.decision`、`governance.decision` 等表 | 全目录扫描 |
| `doc_type='decision'` 说明什么？ | 仅说明该文档分类为决策文档（Markdown 内容在 Git） | 0006:11-13 |
| ADR 是否有独立领域表？ | **否**。ADR 只能是 `doc_type='architecture'`（或 'decision'）的文档；`knowledge_reference.reference_kind IN ('decision','constraint')` 提供语义引用标签 | 0006:12；0014:90-92 |
| Decision 与正式决策实体的结构关联？ | **无**。`ai.change_proposal.target_type` 为无 CHECK 的自由 TEXT（0003:133），0044_s02:5 注释明示「Decision 治理关联 = ai.change_proposal.target_type 扩值，列无 CHECK 零迁移」——即决策治理关联未落任何结构 | 0003:133；0044_s02:5 |

#### C.2.7 项目隔离与相关索引

- 文档域全部 7 张表均有 `(tenant_id, project_id) → core.project(tenant_id, id)` 组合 FK + `UNIQUE (tenant_id, id)`（0019:123-132,165-168；0024:33-39,62-64）——项目隔离由 DB 强制。
- `ai.knowledge_write_operation`（0024:43-64）：写幂等账本，`command` CHECK 0042 扩为 5 值（`create_document/submit_proposal/confirm_redline/veto_revision/bind_requirement_projection`，0042:12-20）；`UNIQUE (project_id, operation_id)`（0024:55）。

### C.3 PC-0003 Change Proposal 与 Approval

**结论先行**：存在通用提案-审批链（`ai.change_proposal` + `governance.approval_request`，1:1），但 approval_request 无状态 CHECK、无不可变保护、无历史保留；D5 线的 `governance.review_approval` 提供了「不可变锚定 + 状态单向」的库层先例。通用审批链能否承载 PC 正式认知审批，取决于是否沿用其现状（见 F-5/F-6）。

#### C.3.1 `ai.change_proposal`（0003_platform.sql:128-154；0019 租户化）

- **`target_type TEXT NOT NULL`——无 CHECK，无枚举约束**（0003:133）；`target_id UUID`（可空，0003:134）；**`expected_revision BIGINT`**（0003:135，乐观锁令牌位：提案基于的目标行版本，DB 无 CAS 约束，消费在应用层）。
- `command_type TEXT NOT NULL`（0003:136）；`proposed_data JSONB NOT NULL`（0003:137）；`rationale TEXT NOT NULL`（0003:138）；`evidence JSONB DEFAULT '[]'`（0003:139）。
- `risk_level TEXT NOT NULL`——**无 CHECK**（0003:140）；`confidence` CHECK 3 值 `low/medium/high`（0003:141-142）。
- **`status` CHECK 7 值**：`proposed/executed/redline_pending/escalated/vetoed/expired/failed`，默认 `proposed`（0003:143-147）。
- `executed_at TIMESTAMPTZ`、`executed_audit_id UUID`（0003:148-149，无 FK）。
- `UNIQUE (analysis_run_id, proposal_seq)`（0003:151）；FK `(project_id, analysis_run_id) → ai.analysis_run(project_id, id)`（0003:152-153）→ 0019 组合化（0019:310-312）+ `UNIQUE (tenant_id, id)`（0019:167）。
- 索引：`idx_proposal_project_status`（0003:229-230）、`idx_proposal_executed_time`（部分，0003:232-234）。

完整 DDL（0003_platform.sql:128-154 原文；0019 仅租户化，未动下列结构）：

```sql
CREATE TABLE ai.change_proposal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    analysis_run_id UUID NOT NULL,
    proposal_seq INT NOT NULL,
    target_type TEXT NOT NULL,
    target_id UUID,
    expected_revision BIGINT,
    command_type TEXT NOT NULL,
    proposed_data JSONB NOT NULL,
    rationale TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '[]',
    risk_level TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'medium'
        CHECK (confidence IN ('low', 'medium', 'high')),
    status TEXT NOT NULL DEFAULT 'proposed'
        CHECK (status IN (
            'proposed', 'executed', 'redline_pending',
            'escalated', 'vetoed', 'expired', 'failed'
        )),
    executed_at TIMESTAMPTZ,
    executed_audit_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (analysis_run_id, proposal_seq),
    FOREIGN KEY (project_id, analysis_run_id)
        REFERENCES ai.analysis_run(project_id, id)
);
```

#### C.3.2 `governance.approval_request`（0003:157-168；0019 租户化）

- **`proposal_id UUID NOT NULL UNIQUE → ai.change_proposal(id)`**（0003:160-161）→ 0019 组合化 `fk_approval_request_tenant_proposal`（0019:374-376）+ `UNIQUE (tenant_id, id)`（0019:168）——**1 提案 : 1 审批请求，DB 强制**。
- **`status TEXT NOT NULL DEFAULT 'pending'`——无 CHECK，允许任意 TEXT**（0003:162）。
- `required_role TEXT NOT NULL`（0003:163，无 CHECK）。
- 审批人/原因/时间：`decided_by UUID → core.principal(id)`（可空）、`decision_reason TEXT`、`decided_at TIMESTAMPTZ`（0003:164-166）。
- **无**：提交人字段、审批历史表、状态迁移触发器、行级不可变保护、拒绝原因独立字段（decision_reason 兼任）。

完整 DDL（0003_platform.sql:157-168 原文；0019 仅追加 tenant_id 列与组合 FK，未动下列结构）：

```sql
CREATE TABLE governance.approval_request (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    proposal_id UUID NOT NULL UNIQUE
        REFERENCES ai.change_proposal(id),
    status TEXT NOT NULL DEFAULT 'pending',
    required_role TEXT NOT NULL,
    decided_by UUID REFERENCES core.principal(id),
    decision_reason TEXT,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```
- 审批历史是否保留：**否**——单行被原地 UPDATE 至终态，无历史版本表（SQL 可证：无任何 approval_history/audit 专表引用 approval_request；task_supersession.approval_request_id 只是引用者，0010:80）。

#### C.3.3 `governance.review_approval`（0026_delivery_review.sql:223-251）——库层不可变审批先例

- `kind` CHECK 3 值：`auto_accept_gate/manual_review/review_degraded`（0026:231-232）；`status` CHECK 3 值：`pending/approved/rejected`（0026:233-234）。
- 三重绑定：`review_run_id` + `delivery_run_id` + `delivery_manifest_hash TEXT NOT NULL`（0026:228-230）——防旧交付审批被继承。
- `uq_review_approval_pending ON (task_id) WHERE status='pending'`（0026:250-251）——同 task 至多一个待决审批。
- 不可变触发器 `trg_review_approval_immutable`（0026:273-296）：锚定列 once-written；状态只能 pending→approved/rejected 单向。
- **该模式是本仓库已验证的「PC 正式认知审批」候选范式（建议，非既成结构）**。

完整 DDL（0026_delivery_review.sql:223-251 原文）：

```sql
CREATE TABLE governance.review_approval (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    project_id      UUID NOT NULL,
    task_id         UUID NOT NULL,
    review_run_id   UUID NOT NULL,
    delivery_run_id         UUID NOT NULL,
    delivery_manifest_hash  TEXT NOT NULL,   -- = 被审 delivery run 的 reviewed_manifest_hash
    kind            TEXT NOT NULL
        CHECK (kind IN ('auto_accept_gate', 'manual_review', 'review_degraded')),
    status          TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected')),
    required_role   TEXT,
    decided_by      UUID REFERENCES core.principal(id),
    decision_reason TEXT,
    decided_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task (project_id, id),
    FOREIGN KEY (project_id, review_run_id)
        REFERENCES delivery.task_run (project_id, id) ON DELETE CASCADE,
    -- 不带 CASCADE：审批不得随 run 删除连带消失。
    FOREIGN KEY (project_id, delivery_run_id)
        REFERENCES delivery.task_run (project_id, id)
);
-- 同一 task 同时刻至多一个待决审批（§3.6 I5）。
CREATE UNIQUE INDEX uq_review_approval_pending ON governance.review_approval (task_id)
    WHERE status = 'pending';
```

#### C.3.4 其他审批/干预/审计载体

| 表 | 关键约束 | 依据 |
|---|---|---|
| `governance.human_intervention` | `intervention_type` CHECK 7 值（veto/rollback/freeze/unfreeze/mode_switch/policy_change/rollback_failed）；`target_proposal_id → ai.change_proposal(id)`（0019:344-346 组合化） | 0003:170-186 |
| `governance.audit_log` | before/after JSONB、compensation_command、source_kind/source_ref（0004:7-9）、actor_identity 三列（0018:25-28）、claimed_device_id（0027:87-88）、租户组合 FK（0019:334-336） | 0003:188-202 |
| `governance.tenant_audit_log` | 租户级审计（无 project）；target_type/target_id（0041:109-111） | 0019:51-66 |
| `core.approver_grant` | 每 (project, principal) 一行单行状态表；`UNIQUE (project_id, principal_id)`；revoked_at 置空即生效（复活语义）；部分索引 `approver_grant_active_idx WHERE revoked_at IS NULL` | 0040:23-38 |
| `delivery.requirement_confirmation` | 不可变确认事实；`UNIQUE (revision_id, action)`——一条 revision 最多一条 confirm + 一条 reject | 0039:72-93 |

#### C.3.5 防并发覆盖的数据库约束（PC-0003 专项）

| 机制 | 是否存在 | 依据 |
|---|---|---|
| 乐观锁令牌（expected_revision） | 有列、无 DB 约束（CAS 比对在应用层） | 0003:135 |
| 待决审批唯一（部分唯一索引） | review_approval 有；approval_request **无**（其 1:1 proposal_id UNIQUE 天然限定单审批行） | 0026:250-251；0003:160-161 |
| 状态迁移 CHECK/触发器 | approval_request 无；review_approval 有（单向触发器） | 0003:162 vs 0026:285-291 |
| SELECT ... FOR UPDATE | 属应用层事务行为，SQL 迁移不可证 | —（待核实 H-11） |

### C.4 PC-0004 Markdown + Git / PostgreSQL / PageIndex

**A. SQL 能确认的事实：**

1. **文档正文权威在 Git，不在 PG**：0006:1-5 注释明示「the Markdown file (in the knowledge Git repository) is the authoritative body; ... Deliberately no column stores the body itself」。`ai.project_document` / `ai.document_revision` 无正文列。
2. **PG 中存在两处 Markdown/正文副本（均为受控例外）**：
   - `ai.document_proposal.proposed_markdown TEXT NOT NULL`（0014:33）——候选正文，注释明示「仅在提案生命周期内存在，提交后正文仍以 Git 修订为准，不构成第二权威」（0014:5-7）；
   - `ai.document_section_index.content TEXT`（0024:21）——章节级派生索引正文，注释明示「可重建的派生数据，正文哈希或索引版本一变即失效，检索回退直读 Git」（0024:5-6）。
3. **Git Commit 记录**：`ai.document_revision.git_commit_sha TEXT NOT NULL`（0006:27）。
4. **内容 Hash 记录**：revision.content_hash（0006:28）、proposal.base_content_hash/proposed_content_hash（0014:32,34）、index_state.indexed_content_hash（0014:66）、requirement_revision.content_hash（0002:26；0039:19-21 明示语义切换为 sha256(markdown)）。
5. **文档 Revision 表**：`ai.document_revision` 存在（0006:22-39），含修订号/状态机/创建人/发布时间。
6. **PageIndex 索引状态表**：`ai.document_index_state` 存在（0014:60-79），`UNIQUE (document_revision_id, provider)`；provider 是自由 TEXT（0024:9 注释：未来 PageIndex 自带派生表，不共用 db-section 表）。
7. **索引版本与文档版本一致性字段**：`indexed_content_hash` + `index_version` 列存在（0014:66-67）；与 `document_revision.content_hash` 的一致性判定**无 DB 约束**（无 CHECK/FK 可表达）。
8. **外部存储路径字段**：`ai.project_document.canonical_path`（0006:14）、`ai.document_proposal.canonical_path`（0014:30）；另有 `requirement_revision.markdown_version_ref TEXT`（0039:63，I2 投影占位）与 `requirement_confirmation.markdown_version_ref/content_hash`（0039:77-78）。
9. **写入幂等账本**：`ai.knowledge_write_operation`（0024:43-64），5 类 command（0042:12-20）。

**B. SQL 无法确认、需业务实现或运行环境核查的事项：**

- 知识 Git 仓库（knowledge root）的物理位置、初始化方式与多租户布局（H-1）。
- `provider` 实际取值（'db-section' 之外是否已有 PageIndex 实现）（H-2）。
- `indexed_content_hash` 与 revision.content_hash 的 stale 判定逻辑（应用层）（H-3）。
- `proposed_markdown` → Git commit 的事务边界（先 commit 后落库或反之）是否保证（H-4）。
- `requirement_revision.markdown_version_ref` 当前是否仍为 NULL 占位（0039:17「I1 先留 NULL」）。

### C.5 PC-0005 模块接口与关联（仅从外键/约束/关联表核查）

#### C.5.1 Requirements（delivery.requirement / requirement_revision）

- `delivery.requirement`（0002:3-14）：`current_revision_id` 经 `fk_current_requirement_revision (id, current_revision_id) → requirement_revision(requirement_id, id)`（0002:36-39）**钉住当前修订**；0019 组合化父键（0019:158）；0039 改造：`status`→`lifecycle_status` CHECK 3 值（0039:36-39）、+requirement_type/priority（0039:40-45）、+created_by/updated_by（0039:46-48）。
- `delivery.requirement_revision`（0002:16-34；0039 改）：`content JSONB NOT NULL`（0002:21，**需求正文以 JSONB 存 PG**）、`content_hash`（0002:26）、五态 state CHECK（0039:57-60）、+base_current_revision_id/base_row_revision（CAS 基版本快照，0039:61-62）、+markdown_version_ref（0039:63）、`DROP source_artifact_id`（0039:66）。

#### C.5.2 跨模块真实关联（DB 外键可证）

| 来源 | 目标 | 方式 | 是否钉 Revision | 依据 |
|---|---|---|---|---|
| delivery.task_requirement | requirement / requirement_revision | 复合 FK `(requirement_id, revision_id)` | **是** | 0002:68-80 |
| delivery.task_knowledge_ref | ai.project_document | 复合 FK `(project_id, document_id)` | **否（只钉 document_id）** | 0015:42-51 |
| ai.knowledge_reference | project_document + document_revision | 双 FK，均 CASCADE | **是** | 0014:82-104 |
| discovery.discovery_proposal | project_document / document_revision / document_proposal | 单列租户 FK ×3 | 是（base_revision_id） | 0025:113-133 |
| delivery.requirement_finding | delivery.requirement + discovery.discovery_finding | 复合 FK | 否（需求级，非修订级） | 0044_s02:43-56 |
| delivery.requirement_source_input | delivery.requirement + source.source_input | 复合 FK | 否（来源属需求级） | 0044_s02:27-39 |
| ai.cognition_item → ai.project_knowledge | **裸 UUID 列 knowledge_id，无 FK** | 无 | — | 0036:57 |
| ai.project_knowledge → cognition_item | 单列 FK source_item_id | 非组合，不强制同项目 | — | 0036:74 |
| ai.baseline_proposal → cognition_run | 单列 FK source_run_id | 非组合，不强制同项目 | — | 0037:16 |
| ai.project_baseline → baseline_item | **无直接 FK**（仅经 proposal_id 间接） | 无 | — | 0037:55-66 |

#### C.5.3 各模块判定

- **Knowledge Base**：ai.project_document/document_revision/document_proposal/document_index_state/document_section_index/knowledge_reference/knowledge_write_operation 七表成链，隔离完整。
- **Design / Architecture**：**在本次 Migration 核查范围内未发现**独立 Design/ADR/Decision 领域表；仅有 doc_type 值与 reference_kind 标签（C.2.6）。
- **S01-04 Cognition**：ai.cognition_run/cognition_item/project_knowledge（0036）即已存在的「技术认知」结构。
- **Project Goal**：**在本次 Migration 核查范围内未发现**项目目标实体表。最接近的承载：`delivery.task.goal TEXT`（0015:16，任务级非项目级）；`ai.baseline_item.category='scope'`（0037:35-37）是基线条目分类而非目标实体。不推断系统其他位置绝对不存在（H-9）。
- **跨项目隔离**：组合 FK 模式（0019）覆盖文档域/提案域全部表；但 cognition/baseline 线的**内部引用列**（run_id/source_item_id/supersedes_id/proposal_id/source_run_id）均为单列 FK，库层不强制同项目（F-3）。
- **缺失的关联能力**：PC 认知条目 ↔ 知识文档（Git 文档）之间无外键级关联；cognition_item.knowledge_id 无约束；文档域↔cognition 域完全无 FK 通道。

### C.6 PC-0006 迁移设计输入（仅结构事实；映射方案见 G 节，标记为建议）

旧知识对象清单及其存储事实：

| 旧对象 | 正文位置 | 历史/版本 | 来源/可信度 | 状态/确认 | 依据 |
|---|---|---|---|---|---|
| ai.project_knowledge | PG `content TEXT` | version + supersedes_id 链 | source_item_id（→cognition_item） | status 3 值 + confirmed_by/at | 0036:66-87 |
| ai.cognition_item | PG `content TEXT` + evidence JSONB | 无版本链（被 supersede 由 knowledge 链表达） | evidence JSONB、confidence | state 6 值 + decided_by/at/note | 0036:32-61 |
| ai.baseline_proposal / baseline_item / project_baseline | PG `content TEXT` + evidence JSONB | proposal.version / baseline.version（租户内唯一） | source_run_id、confidence | status/state + confirmed_by/at/decided_* | 0037:7-66 |
| discovery.discovery_finding(+evidence) | PG `content TEXT`；0031 后证据独立成表 discovery_finding_evidence（file_path/symbol/code_range/commit_sha/attempt_no） | 无版本链 | evidence 实体 + confidence + kind 四值 provenance | verification_status 4 值（PENDING/CONFIRMED/REJECTED/CONFLICT） | 0025:48-64；0031:112-171 |
| ai.project_document + document_revision | **Git（PG 无正文）** | revision_no 链 + status | source_event_id/analysis_run_id | status 4 值 + redline_confirmed_by | 0006 + 0014 |
| delivery.requirement_revision | PG `content JSONB`（+0039 起 markdown_version_ref 投影位） | version 链 + 五态 | content_hash | state 5 值 + created_by/approved_by | 0002 + 0039 |
| integration.repo_import(+draft_version/attempt/source) | PG `draft JSONB`（0031 起草案版本化） | draft revision 链 | repo_import_source_attempt 事实快照 | status 10 值 | 0009 + 0031 |

可迁入 PC 的数据、可能重复/冲突的数据、无法自动映射的字段、需保留的外键与审计关系：见 G 节（均为**建议**）。

---

## D. 表结构事实清单

> 「正文存储方式」列：—= 无正文语义；TEXT/JSONB = 存 PG；Git = 权威在 Git；派生TEXT = 可重建副本。

| Schema.Table | 是否存在 | 定义 Migration | 主键 | 项目隔离方式 | 关键外键 | 正文存储方式 | 备注 |
|---|---|---|---|---|---|---|---|
| core.customer | 是 | 0001:10-15；0017；0019 | id | tenant 组合 FK | parent 自引用（组合） | — | |
| core.project | 是 | 0001:17-29；0019 | id | (tenant_id,id) 唯一 + 根表 | customer 组合 FK | — | code 租户内唯一（0019:398-399） |
| core.principal | 是 | 0001:31-39 | id | 全局共享（无 tenant_id） | — | — | |
| core.project_member | 是 | 0001:41-46；0030 | (project_id,principal_id) | 组合 FK | project/principal | — | role CHECK 6 值（0030:8-9） |
| core.principal_identity | 是 | 0018:10-19；0019 改 | id | tenant FK | principal | — | |
| core.tenant | 是 | 0019:21-29；0044_team:24-27 | id | 根表 | — | — | slug 部分唯一（0044_team:29） |
| core.tenant_member | 是 | 0019:31-39 | (tenant_id,principal_id) | 根 | tenant/principal | — | role CHECK owner/admin/member |
| core.tenant_agent | 是 | 0020:33-41 | (tenant_id,agent_definition_id) | 根 | definition/principal | — | |
| core.approver_grant | 是 | 0040:23-34 | id | 组合 FK | project/principal ×3 | — | 单行状态表；revoked_at 复活语义 |
| core.tenant_invitation(+link/redemption) | 是 | 0044_team:38-115 | id | tenant FK | tenant/principal | — | 邀请域 |
| core.project_initialization_operation | 是 | 0031:29-41 | id | 组合 FK CASCADE | project | — | active 部分唯一（0031:42-43） |
| delivery.requirement | 是 | 0002:3-14；0039 改 | id | 组合 FK + UNIQUE(project_id,id) | project；current_revision（组合） | —（正文在 revision） | lifecycle_status 3 值 |
| delivery.requirement_revision | 是 | 0002:16-34；0039 改 | id | 组合 FK | requirement（组合）；principal ×2 | **JSONB**（content）+ content_hash | 五态；markdown_version_ref 占位 |
| delivery.milestone | 是 | 0002:41-49 | id | 组合 FK | project | — | |
| delivery.task | 是 | 0002:51-66；0010/0013/0015 | id | 组合 FK | milestone（组合）；assignee | goal TEXT（任务级目标） | status 13 值 CHECK（0010:150-153） |
| delivery.task_requirement | 是 | 0002:68-80 | (task_id,revision_id) | 双组合 FK | task/requirement/revision | — | **钉 revision** |
| delivery.task_run | 是 | 0010:18-41；0011/0012/0015/0020/0022/0023/0026/0028/0032 | id | 组合 FK + UNIQUE(tenant_id,id)+UNIQUE(project_id,id) | task（组合） | — | run_kind 分轨（0026） |
| delivery.task_run_failure | 是 | 0010:47-59 | run_id | C 类组合 FK | run（组合） | evidence_ref TEXT | append-only 触发器（0010:68-70） |
| delivery.task_supersession | 是 | 0010:73-87 | id | 组合 FK ×3 | task×2、approval_request | — | |
| delivery.task_waiting | 是 | 0010:90-107 | id | 组合 FK | task | questions JSONB | |
| delivery.task_policy_override | 是 | 0010:109-120 | task_id | 组合 FK | task | additional_checks JSONB | |
| delivery.task_repository | 是 | 0011:86-105；0031 强化 | (task_id,repository_id) | 组合 FK | task/repository（RESTRICT，0031:237-240） | path_scope TEXT[] | |
| delivery.task_run_repository | 是 | 0011:108-121 | (run_id,repository_id) | 组合 FK | run/repository | — | base_commit 快照 |
| delivery.task_acceptance_criterion | 是 | 0015:25-39 | id | 组合 FK CASCADE | task | text TEXT | |
| delivery.task_knowledge_ref | 是 | 0015:42-51 | (task_id,document_id) | 双组合 FK | task / **ai.project_document** | — | Task↔文档桥 |
| delivery.task_dependency | 是 | 0021:9-28 | id | 组合 FK ×3 | task×2 | — | |
| delivery.task_repository_commit | 是 | 0022:51-81 | id | 组合 FK ×3 | run（RESTRICT）/repository | test_evidence JSONB | commit_sha |
| delivery.task_run_cleanup | 是 | 0035:16-35 | run_id | run FK CASCADE | task_run | identity_hint TEXT | |
| delivery.review_finding | 是 | 0026:165-186 | id | 组合 FK | review_run CASCADE / criterion | message TEXT | append-only（0026:265-266） |
| delivery.acceptance_criterion_verdict | 是 | 0026:193-216 | id | 组合 FK | criterion / review_run | evidence JSONB | append-only |
| delivery.policy_scope | 是 | 0013:9-18；0016 | (tenant_id,scope,scope_key) | 伪根表直挂 tenant | tenant | additional_checks JSONB | |
| delivery.requirement_confirmation | 是 | 0039:72-93 | id | 组合 FK ×2 | requirement/revision | markdown_version_ref/hash | UNIQUE(revision_id,action) |
| delivery.requirement_source_input | 是 | 0044_s02:27-39 | (requirement_id,input_id) | 组合 FK ×3 | requirement / source_input | — | |
| delivery.requirement_finding | 是 | 0044_s02:43-56 | (requirement_id,finding_id) | 组合 FK ×3 | requirement / discovery_finding | — | relation CHECK ='supports' |
| delivery.v_task_delivery_manifest | 视图 | 0022:105-181；0026:88-159 重建 | — | — | — | — | 只读对账视图 |
| integration.inbound_event | 是 | 0003:6-19 | id | 组合 FK | project | payload JSONB | |
| integration.job | 是 | 0003:22-37 | id | 组合 FK ×2 | project/event | — | |
| integration.notification | 是 | 0003:40-49 | id | 组合 FK | project | payload JSONB | |
| integration.project_ssh_key | 是 | 0008:6-16；0009 | project_id | 组合 FK CASCADE | project/principal | — | |
| integration.project_repository | 是 | 0008:18-32；0009/0031/0038 | id | 组合 FK + 双唯一键 | project | — | selected_commit 确认制（0038:13-18） |
| integration.repo_import | 是 | 0009:27-49；0025/0031 改 | id | 组合 FK | project/import/principal/analysis/discovery_session | draft JSONB（0031 删单列 draft→版本表） | |
| integration.repo_import_draft_version | 是 | 0031:46-58 | (tenant_id,import_id,revision) | 组合 FK CASCADE | repo_import | **draft JSONB** | 草案版本留档 |
| integration.repo_import_source | 是 | 0009:60-74 建→**0031:62-79 整表重建** | (tenant_id,import_id,repository_id) | 组合 FK CASCADE/RESTRICT | import/repository | facts JSONB→attempt | |
| integration.repo_import_source_attempt | 是 | 0031:82-106 | 四列复合 | 组合 FK CASCADE | source | facts JSONB | 分析轮次留档 |
| integration.outbox_event | 是 | 0010:123-144 | id | 组合 FK | project | payload JSONB | 幂等键租户内唯一（0019:407-409） |
| integration.git_capability_grant | **已删除** | 0032:21-34 建→0033:26 DROP | — | — | — | — | |
| **ai.project_document** | 是 | 0006:7-20 | id | 组合 FK + 双唯一键 | project | —（Git 权威） | doc_type CHECK 4 值 |
| **ai.document_revision** | 是 | 0006:22-39；0014 补列 | id | 组合 FK | document（组合）/principal ×2 | —（git_commit_sha+content_hash） | revision_no 链 |
| **ai.document_proposal** | 是 | 0014:21-57 | id | 组合 FK | document/base_revision/result_revision/principal ×2 | **proposed_markdown TEXT**（受控副本） | status 8 值 |
| **ai.document_index_state** | 是 | 0014:60-79 | id | 组合 FK | revision（CASCADE） | indexed_content_hash | UNIQUE(revision,provider) |
| **ai.knowledge_reference** | 是 | 0014:82-104 | id | 组合 FK | document/revision（双 CASCADE） | section 锚点 | kind CHECK |
| ai.document_section_index | 是 | 0024:11-39 | id | 组合 FK ×2 | project/revision（CASCADE） | **content TEXT（派生）** | UNIQUE(revision,provider,section_no) |
| ai.knowledge_write_operation | 是 | 0024:43-64；0042 扩 CHECK | id | 组合 FK | project | — | UNIQUE(project_id,operation_id) |
| ai.analysis_run | 是 | 0003:107-126 | id | 组合 FK | project/event（组合） | config/context/result JSONB | |
| **ai.change_proposal** | 是 | 0003:128-154 | id | 组合 FK | analysis_run（组合） | proposed_data JSONB | target_type 无 CHECK |
| ai.agent_feedback | 是 | 0003:204-214 | id | 组合 FK | project/principal/event | content JSONB | |
| ai.project_ai_config | 是 | 0003:52-73 | project_id | 组合 FK | project/principal | model_config JSONB | |
| ai.skill_definition / project_skill_binding | 是 | 0003:75-104 | id / (project,skill) | 组合 FK | config/skill | settings JSONB | |
| ai.agent_definition | 是 | 0020:14-22 | id | 全局 | — | — | role_kind CHECK |
| ai.llm_provider | **已删除** | 0007 建→0041:117 DROP | — | — | — | — | |
| ai.team_llm_provider / team_llm_credential / llm_call_record | 是 | 0041:30-101 | id | tenant FK / 组合 FK | tenant/provider/principal | configuration JSONB | |
| ai.project_ai_model_policy | 是 | 0043:13-25 | (tenant_id,project_id) | 组合 FK | project/provider | — | |
| **ai.cognition_run** | 是 | 0036:7-27 | id | 组合 FK | project/tenant/principal | scope/repo_versions JSONB | |
| **ai.cognition_item** | 是 | 0036:32-61 | id | 组合 FK | run（单列）/principal | **content TEXT + evidence JSONB** | knowledge_id 裸列 |
| **ai.project_knowledge** | 是 | 0036:66-87 | id | 组合 FK | item（单列）/self/principal | **content TEXT** | version+supersedes |
| **ai.baseline_proposal** | 是 | 0037:7-27 | id | 组合 FK | run（单列）/principal ×2 | — | pending 部分唯一 |
| **ai.baseline_item** | 是 | 0037:29-50 | id | 组合 FK | proposal（单列）/principal | **content TEXT + evidence JSONB** | category 7 值 |
| **ai.project_baseline** | 是 | 0037:55-66 | id | 组合 FK | proposal（单列）/principal | — | UNIQUE(tenant,project,version) |
| governance.approval_request | 是 | 0003:157-168 | id | 组合 FK | proposal（1:1 UNIQUE）/principal | — | status 无 CHECK |
| governance.human_intervention | 是 | 0003:170-186 | id | 组合 FK ×2 | proposal/principal | — | |
| governance.audit_log | 是 | 0003:188-202；0004/0018/0019/0027/0034 | id | 组合 FK | principal/identity/device | before/after JSONB | |
| governance.tenant_audit_log | 是 | 0019:51-66；0041 补列 | id | tenant FK | principal ×2 | before/after JSONB | |
| governance.review_approval | 是 | 0026:223-251 | id | 组合 FK ×4 | task/review_run/delivery_run | — | 不可变触发器 |
| discovery.discovery_session / message / finding / finding_repository / question / answer / proposal | 是 | 0025:12-135；0031 强化 | 各 id | 组合 FK 链 | session/finding/repository/document 系列 | content/proposed_content TEXT | |
| discovery.discovery_finding_evidence | 是 | 0031:128-171 | id | 三列组合 FK | finding/source/attempt | file_path/symbol/code_range | evidence 实体 |
| devenv.device / device_project_binding | 是 | 0027:23-79 | id | 组合 FK ×2 | tenant/tenant_member/project | last_check JSONB | |
| source.source_input | 是 | 0044_s02:10-23 | id | 组合 FK | project | **content TEXT** | input_kind 5 值 |

---

## E. 关系与约束清单（PC 视角核心关系）

| 来源表 | 目标表 | 关联方式 | 是否钉住 Revision | 约束类型 | 备注 |
|---|---|---|---|---|---|
| ai.cognition_item | ai.cognition_run | 单列 FK run_id | — | FK（NO ACTION） | 不强制同项目（0036:36） |
| ai.cognition_item | ai.project_knowledge | **裸列 knowledge_id，无 FK** | — | 无 | 关联仅应用层（0036:57） |
| ai.project_knowledge | ai.cognition_item | 单列 FK source_item_id | — | FK | 可空；不强制同项目（0036:74） |
| ai.project_knowledge | ai.project_knowledge（自引用） | 单列 FK supersedes_id | — | FK | 版本链（0036:78） |
| ai.baseline_item | ai.baseline_proposal | 单列 FK proposal_id | — | FK | （0037:33） |
| ai.baseline_proposal | ai.cognition_run | 单列 FK source_run_id | — | FK | 可空（0037:16） |
| ai.project_baseline | ai.baseline_proposal | 单列 FK proposal_id | — | FK | 版本头→提案轮次（0037:60） |
| ai.project_baseline | ai.baseline_item | **无直接关联** | — | 无 | 只能经 proposal 间接（F-2） |
| ai.document_revision | ai.project_document | 组合 FK (project_id, document_id)→(0019 租户化) | — | FK + UNIQUE(document_id,revision_no) | （0006:35,37-38） |
| ai.document_proposal | ai.document_revision | base_revision_id / resulting_revision_id（组合租户 FK） | **是（base/result 双向）** | FK | （0014:31,46；0019:353-359） |
| ai.document_index_state | ai.document_revision | 组合租户 FK CASCADE | **是** | FK + UNIQUE(revision,provider) | （0014:63,75；0019:360-363） |
| ai.knowledge_reference | ai.document_revision | 组合租户 FK CASCADE | **是** | FK + 身份唯一索引 | （0014:86,103-104） |
| ai.change_proposal | ai.analysis_run | 组合租户 FK | — | FK + UNIQUE(run,seq) | （0003:151-153） |
| governance.approval_request | ai.change_proposal | 组合租户 FK；proposal_id UNIQUE | — | FK（1:1） | （0003:160-161；0019:374-376） |
| governance.review_approval | delivery.task_run ×2 | 组合 FK + manifest_hash 绑定 | —（钉 Run + 交付快照哈希） | FK + 触发器不可变 + pending 部分唯一 | （0026:243-251,273-296） |
| delivery.task_requirement | delivery.requirement_revision | 复合 FK (requirement_id, revision_id) | **是** | FK | （0002:78-79） |
| delivery.requirement | delivery.requirement_revision | fk_current_requirement_revision | **是（当前修订）** | FK（可空） | （0002:36-39） |
| delivery.task_knowledge_ref | ai.project_document | 组合 FK | 否（无 revision 列） | FK | （0015:42-51） |
| delivery.requirement_confirmation | delivery.requirement_revision | 复合 FK + UNIQUE(revision_id,action) | **是** | FK + 唯一 | （0039:89-92） |
| discovery.discovery_proposal | ai.document_proposal | 单列租户 FK document_proposal_id | —（关联的是提案不是修订） | FK | （0025:132） |

---

## F. 关键发现

### F.1 已确认事实（SQL 可证）

1. **PC-0001 四表齐备且从未被后续 Migration 修改**：`ai.project_knowledge`、`ai.project_baseline`、`ai.baseline_item`、`ai.baseline_proposal` 均在 `ai` schema（0036/0037），语义与 PC 目标对象直接对应；另有 `ai.cognition_run/cognition_item` 承载认知过程。
2. **文档域是「Git 权威 + PG 状态」结构**（0006 头注释），有修订链、提案链、索引状态、证据引用、写幂等账本完整闭环。
3. **`doc_type` 全仓仅一处枚举语义**：CHECK 四值 `requirement/decision/architecture/knowledge`（0006:11-13）；无 ENUM 类型对象。
4. **Decision/ADR 无领域实体**：仅文档分类值 + `knowledge_reference.reference_kind` 标签 + `change_proposal.target_type` 自由文本（C.2.6）。
5. **通用审批链 1:1 无历史**：approval_request 经 `proposal_id UNIQUE` 钉死 1:1，无状态 CHECK、无不可变触发器、无历史表（0003:157-168）；对比 `governance.review_approval` 已实现「不可变锚定 + 状态单向 + pending 唯一」三重库层防护（0026）。
6. **多租户隔离以 0019 组合 FK 为统一模式**，cognition/baseline 文档域全覆盖；但 0036/0037 内部引用列沿用单列 FK（F-3）。
7. **正文存 PG 的对象**（与 Markdown+Git 原则的偏离点）：`ai.project_knowledge.content`、`ai.cognition_item.content`、`ai.baseline_item.content`、`discovery.discovery_finding.content`、`source.source_input.content`、`delivery.requirement_revision.content(JSONB)`、`ai.document_proposal.proposed_markdown`（受控副本）、`ai.document_section_index.content`（派生）、`integration.repo_import_draft_version.draft(JSONB)`。
8. **迁移账本异常**：缺 0029；0044 同号双文件（B.2）。
9. **已删除表**：`integration.git_capability_grant`（0033:26）、`ai.llm_provider`（0041:117）、`delivery.requirement_revision.source_artifact_id` 列（0039:66）。

### F.2 结构缺失（相对 PC 需要）

1. **project_baseline ↔ baseline_item 无直接关联**：已确认基线版本头不钉条目集合；若 proposal 确认后又有新提案引用同一 proposal 的条目状态变化，版本快照的「内容构成」无法从库层还原（0037:55-66）。
2. **cognition_item.knowledge_id 无外键**：认知条目→项目知识的确认产物关联无 DB 保证（0036:57）。
3. **无 PC 认知↔知识文档（Git）的结构通道**：cognition/baseline 域与 project_document 域之间零外键。
4. **无正式 Decision/ADR/Project Goal 实体表**（C.2.6、C.5.3）。
5. **approval_request 无状态 CHECK / 无历史 / 无不可变保护**（C.3.2）。
6. **document_index_state 与 revision 的 hash 一致性无 DB 约束**（C.2.4）。

### F.3 结构风险

1. **cognition/baseline 内部引用为单列 FK**（run_id/source_item_id/supersedes_id/proposal_id/source_run_id，0036:36,74,78；0037:16,33,60）：因 id 是全局 UUID 主键不会指错行，但**不强制同租户/同项目**——应用层 bug 可产生跨项目认知链，且项目删除（NO ACTION）与级联语义不完整。
2. **0044 同号双文件**：sqlx 记账与校验行为存在顺序耦合风险（B.2、H-13）。
3. **approval_request.status 自由 TEXT**：任何字符串可写入，审批终态无库层语义（0003:162）。
4. **change_proposal.target_type 自由 TEXT + target_id 无 FK**：提案目标可指向不存在/已删对象（0003:133-134）。
5. **project_knowledge 版本无 (project,domain,version) 唯一约束**：version 数字可能重复，仅靠 supersedes 链表达顺序（0036:75）。

### F.4 无法仅凭 SQL 判断的事项

- Git 知识根的物理布局、commit 与 revision 行写入的事务顺序（H-1/H-4）。
- `expected_revision` 的 CAS 消费逻辑与 SELECT FOR UPDATE 使用点（H-11）。
- PageIndex provider 的实际实现与 stale 判定（H-2/H-3）。
- 迁移在目标环境是否已全部应用、`_sqlx_migrations` 实况（本次未连接数据库）。
- 0029 缺号原因（预留后弃用，或笔误）（H-14）。

---

## G. PC 迁移设计输入（⚠️ 本节全部为**建议**，非已确认设计）

> 以下映射建议基于 D/C 节结构事实推导，供 PC 草稿模型设计参考；所有「目标形态」以 PC 正式设计裁决为准。

### G.1 旧对象 → PC 草稿模型映射建议

| 旧对象 | 建议迁入 PC 的目标 | 映射要点 | 冲突/不可自动映射 |
|---|---|---|---|
| ai.project_knowledge（active 行） | PC 认知条目（已确认态） | content→PC 正文；domain 6 值可保留或重映射；confirmed_by/at→确认记录 | version/supersedes 链→PC 若改用 revision 模型需重算版本号；status='superseded/retired' 行建议作为历史归档不参与活跃迁移 |
| ai.cognition_item（confirmed 且已落 knowledge） | PC 证据/来源记录 | evidence JSONB→PC 证据结构；knowledge_id（裸列）需按应用层数据修复后才能重建关联 | state='proposed/investigation/deferred' 的悬挂条目：无产物，建议按「未确认草稿」迁入或标记废弃（需产品裁决） |
| ai.baseline_proposal/baseline_item | PC 基线草稿条目 | content/category/confidence/decided_* 直映射 | proposal.status='pending' 的半途提案：迁入后 pending 部分唯一索引（0037:26-27）要求每项目至多一个——多项目并存 pending 时需合并或择一 |
| ai.project_baseline | PC 基线版本头 | version（租户内唯一）+ confirmed_by/at 可直迁 | **条目集合无法从库层还原**（F.2-1）——需按 proposal_id 反推快照，属推导性迁移，建议在迁移报告中标明推导口径 |
| discovery.discovery_finding(+evidence) | PC 事实/证据池 | evidence 实体表（0031）字段结构最接近「来源可追溯」要求；verification_status 四值与 PC 草稿状态可对齐 | finding 与 cognition_item 语义重叠（同一事实两个来源域）——迁移时需去重策略（按 evidence 内容/commit_sha 匹配，属建议） |
| ai.project_document + document_revision | PC 文档身份/修订 | **无需正文迁移**（Git 权威不变）；只需保留 doc_key/canonical_path/git_commit_sha/content_hash 映射 | doc_type 四值若 PC 扩枚举（如新增 pc_* 类别），需 CHECK 变更（直改，AGENTS 规则 1） |
| ai.document_proposal（未终结行） | PC 提案 | proposed_markdown 仅提案生命周期内有效 | status ∈ {draft, quality_gates_passed, redline_pending, commit_pending} 的未终结提案：迁移时点 Git 未必有 commit，建议冻结为「迁移前遗留」或拒绝迁移（需裁决） |
| delivery.requirement_revision | 需求域（PC-0005 关联对象，非 PC 迁移主体） | content JSONB 是需求域自有结构；PC 只需引用 revision_id（task_requirement 先例 0002:78-79） | markdown_version_ref 若仍为 NULL，需求正文与 Markdown 投影的对应关系缺失（H-5） |
| integration.repo_import_draft_version | 历史导入事实（建议不迁） | draft JSONB 为一次性导入会话产物 | 建议：仅作只读历史保留，不迁入 PC 活跃模型 |

### G.2 迁移必须保留的关系与审计（建议清单）

1. 全部新表沿用 0019 组合 FK 模式：`(tenant_id, project_id) → core.project` + `UNIQUE (tenant_id, id)`（0019 §4/§6 先例）。
2. cognition→knowledge 的确认产物关联（现 knowledge_id 裸列）建议在 PC 模型中升级为组合 FK（消除 F.3-1）。
3. 审批若复用 review_approval 范式，保留其三要素：锚定不可变触发器 + 状态单向 CHECK 语义 + pending 部分唯一索引（0026:250-296）。
4. 迁移过程写入 `governance.audit_log`（source_kind/source_ref 机制，0004:7-9）以保留「旧对象 → PC 对象」的追溯链。
5. `ai.knowledge_write_operation` 幂等账本模式（UNIQUE(project_id, operation_id)）建议沿用为 PC 写路径防重机制（0024:55）。

---

## H. 待进一步核实问题

| # | 问题 | 需要什么 | 关联发现 |
|---|---|---|---|
| H-1 | 知识 Git 根（KnowledgeRoot）的物理位置、初始化与多租户目录布局 | 业务实现（pm-application::knowledge） | C.4-B |
| H-2 | PageIndex provider 实际取值清单及是否已有非 'db-section' 实现 | 业务实现/运行库 | C.2.4 |
| H-3 | indexed_content_hash vs revision.content_hash 的 stale 判定逻辑位置 | pm-application::knowledge_indexer | F.4 |
| H-4 | proposed_markdown → git commit → resulting_revision_id 的事务顺序保证 | knowledge_proposal 实现 | C.4-B |
| H-5 | requirement_revision.markdown_version_ref 当前是否已有写入方 | 业务代码 | C.4-A9 |
| H-6 | approval_request.status 的应用层取值集合（无 CHECK，实际写入哪些值） | pm-api/pm-application 写路径 | F.3-3 |
| H-7 | change_proposal.target_type 实际使用值清单（含 0044_s02 注释提到的 Decision 扩值是否已用） | 业务代码 + 运行库 | F.3-4 |
| H-8 | cognition_item.knowledge_id 的写入与回读路径（是否有应用层保证同项目） | S01-04 实现 | F.2-2 |
| H-9 | Project Goal 是否在其他子系统（如前端 local state、MCP 工具）有承载 | 产品负责人确认 | C.5.3 |
| H-10 | PC 正式认知审批拟复用 approval_request 还是 review_approval 范式（或新建） | 产品/架构裁决 | F.1-5、G.2-3 |
| H-11 | 审批/提案链是否使用 SELECT ... FOR UPDATE 事务并发控制 | pm-application 事务代码 | C.3.5 |
| H-12 | cognition/baseline 单列 FK 是否计划按 0019 模式组合化（涉及 PC 模型设计基线） | PC 设计裁决 | F.3-1 |
| H-13 | sqlx 对 0044 同号双文件的 `_sqlx_migrations` 记账与 validate 实际行为 | 运行库 `_sqlx_migrations` 表实况 | B.2 |
| H-14 | 0029 缺号原因；`docs/ai-pm-migration-ledger.md` 账本与目录的一致性 | 迁移账本文档核对 | B.2 |

---

## 附：完成标准对照（任务书 §六）

| 问题 | 答案（摘要） | 详情 |
|---|---|---|
| 1. 现有项目知识与项目基线到底是什么结构？ | 六域认知条目（cognition_item）→ 人类确认 → project_knowledge（content TEXT 在 PG，version+supersede 链）；基线 = proposal(条目级确认) → project_baseline（版本头，租户内 version 唯一）；条目集合与版本头无库层直接绑定 | C.1 |
| 2. 现有文档、Decision、ADR 是否有独立领域模型？ | 文档域有（document/revision/proposal/index/reference 完整闭环）；Decision 与 ADR **无**独立领域表，仅 doc_type 分类值与 reference 标签 | C.2 |
| 3. 现有审批机制能否承载 PC 正式认知审批？ | change_proposal+approval_request 链结构存在但弱（status 无 CHECK、无不可变、无历史、1:1）；review_approval 范式已提供库层不可变审批先例；结论：可承载但需按 review_approval 范式加固或新建（待 H-10 裁决） | C.3、F.1-5 |
| 4. 数据库正文存储是否符合 Markdown + Git 权威原则？ | 文档域符合（0006 明示正文不落库）；但 cognition/baseline/discovery/需求修订等结构化对象的 content TEXT 存 PG——它们是短条目而非 Markdown 文档，是否豁免属架构裁决；document_proposal.proposed_markdown 与 document_section_index.content 为受控副本 | C.4、F.1-7 |
| 5. PC 与其他模块之间有哪些真实的数据库关联？ | task_requirement（钉 requirement_revision）、task_knowledge_ref（钉 document）、knowledge_reference（钉 revision）、requirement_finding/requirement_source_input（0044_s02）、discovery_proposal→document 三 FK；cognition/baseline 与文档域之间**零**外键通道 | C.5、E |
| 6. 旧知识迁移到 PC 草稿需要怎样映射？ | 见 G 节映射建议表（标记为建议）；关键难点：project_baseline 条目集合不可库层还原、cognition_item 悬挂状态、pending 提案部分唯一约束、重复认知域去重 | G |