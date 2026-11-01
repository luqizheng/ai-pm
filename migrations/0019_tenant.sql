-- 0019_tenant: 多租户基础设施（T-021b α）
-- 依据：docs/ai-pm-tenant-infrastructure-v1.md §2 §3
-- 上位约束：docs/ai-pm-adr-0001-tenant-agent-executor-identity.md（D1–D6）
--
-- 原则：新项目、无存量数据（AGENTS.md 规则 1）——tenant_id 一律 NOT NULL 无默认值，
-- 不写 DEFAULT + UPDATE + SET NOT NULL 的数据搬运三段式。dev 库执行前重建。
--
-- 结构（顺序不可打乱：父表唯一键必须先于子表组合外键就位）：
--   1) 租户根 / 成员 / 审计 / 系统租户种子
--   2) principal_identity 改造（删上游原文列，加内部租户外键）
--   3) 39 张表加 tenant_id
--   4) 15 张被引用表加 UNIQUE (tenant_id, id)
--   5) 拆除旧的单列 FK（32 处）
--   6) 建立新的组合 FK
--   7) I4：4 处全局唯一改租户内唯一

-- ═══════════════════════════════════════════════════════════════════
-- 1) 租户根
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE core.tenant (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider     TEXT NOT NULL,              -- V1 只写 'lmms' 或 'system'
    external_key TEXT NOT NULL,              -- lmms claims.tid 字符串化
    display_name TEXT NOT NULL,
    is_active    BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, external_key)
);

CREATE TABLE core.tenant_member (
    tenant_id    UUID NOT NULL REFERENCES core.tenant(id) ON DELETE CASCADE,
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    role         TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    granted_by   UUID REFERENCES core.principal(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, principal_id)
);

CREATE INDEX tenant_member_principal_idx ON core.tenant_member (principal_id);

-- 系统租户：dev-off 模式与测试夹具共用（设计稿 Q6）。与 0015 的 ai-pm-dispatch
-- service principal 同款幂等种子。
INSERT INTO core.tenant (id, provider, external_key, display_name)
VALUES ('00000000-0000-0000-0000-000000000010', 'system', 'default', 'system')
ON CONFLICT (id) DO NOTHING;

-- 租户级审计：governance.audit_log.project_id 是 NOT NULL，租户成员变更不属任何
-- 项目（设计稿 Q9），故独立成表。列名与 audit_log 保持同口径。
CREATE TABLE governance.tenant_audit_log (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    actor_id            UUID NOT NULL REFERENCES core.principal(id),
    actor_identity_id   UUID REFERENCES core.principal_identity(id),
    actor_client        TEXT,
    actor_auth_mode     TEXT,
    action              TEXT NOT NULL,
    target_principal_id UUID REFERENCES core.principal(id),
    before_data         JSONB,
    after_data          JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX tenant_audit_log_tenant_idx
    ON governance.tenant_audit_log (tenant_id, created_at DESC);

-- ═══════════════════════════════════════════════════════════════════
-- 2) principal_identity 改造（设计稿 Q3）
-- ═══════════════════════════════════════════════════════════════════
-- 删上游原文列 claims_tenant_key：Q1 定 1:1 镜像后它与 core.tenant.external_key
-- 是同一事实，保留即双来源。audit_log 的 actor_identity_id 引用本表，需先摘除
-- 再加回，避免改列期间外键悬空。

ALTER TABLE governance.audit_log DROP COLUMN actor_identity_id;

ALTER TABLE core.principal_identity
    DROP CONSTRAINT principal_identity_provider_claims_tenant_key_subject_key,
    DROP COLUMN claims_tenant_key,
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    ADD CONSTRAINT uq_principal_identity UNIQUE (provider, tenant_id, subject);

-- actor_client / actor_auth_mode 已在 0018 建立，此处只把被摘除的 actor_identity_id
-- 按新外键加回。
ALTER TABLE governance.audit_log
    ADD COLUMN actor_identity_id UUID REFERENCES core.principal_identity(id);

-- ═══════════════════════════════════════════════════════════════════
-- 3) 39 张租户业务表加 tenant_id
-- ═══════════════════════════════════════════════════════════════════
-- A 类根表（2）：tenant_id 由调用方显式给；带 tenant 外键。

ALTER TABLE core.customer
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id);
ALTER TABLE core.project
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id);

-- B 类直挂 project（33）：tenant_id 由 project 子查询推导。
-- C 类锚定非 project 父行（3）：tenant_id 由父行子查询推导。
-- 两者此处只加列，外键统一在 §6 建（父表唯一键先在 §4 就位）。

ALTER TABLE core.project_member           ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.requirement          ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.requirement_revision ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.milestone            ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task                 ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_requirement     ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_run             ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_supersession    ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_waiting         ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_policy_override ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_repository      ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_run_repository  ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_acceptance_criterion ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_knowledge_ref   ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.inbound_event     ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.job               ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.notification      ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.project_ssh_key   ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.project_repository ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.repo_import       ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.outbox_event      ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.project_ai_config          ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.project_skill_binding      ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.analysis_run               ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.change_proposal            ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.agent_feedback             ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.project_document           ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.document_revision          ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.document_proposal          ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.document_index_state       ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE ai.knowledge_reference        ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE governance.human_intervention ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE governance.audit_log          ADD COLUMN tenant_id UUID NOT NULL;

-- C 类（3）

ALTER TABLE governance.approval_request   ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE delivery.task_run_failure     ADD COLUMN tenant_id UUID NOT NULL;
ALTER TABLE integration.repo_import_source ADD COLUMN tenant_id UUID NOT NULL;

-- D 类伪根表（1）：scope_key 是文本不是 UUID 引用，只能显式给 tenant_id +
-- 单列外键（设计稿 §3.5 对 I1 的例外）。

ALTER TABLE delivery.policy_scope
    ADD COLUMN tenant_id UUID NOT NULL REFERENCES core.tenant(id);

-- ═══════════════════════════════════════════════════════════════════
-- 4) 15 张被组合 FK 引用的表补 UNIQUE (tenant_id, id)
-- ═══════════════════════════════════════════════════════════════════
-- 其中 5 张今天没有对应唯一键（设计稿 §3.6 的 ⭐）：requirement_revision /
-- task_run / document_revision / change_proposal / approval_request。

ALTER TABLE core.customer          ADD CONSTRAINT uq_customer_tenant_id   UNIQUE (tenant_id, id);
ALTER TABLE core.project           ADD CONSTRAINT uq_project_tenant_id    UNIQUE (tenant_id, id);
ALTER TABLE delivery.milestone     ADD CONSTRAINT uq_milestone_tenant_id  UNIQUE (tenant_id, id);
ALTER TABLE delivery.requirement   ADD CONSTRAINT uq_requirement_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE delivery.requirement_revision ADD CONSTRAINT uq_requirement_revision_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE delivery.task          ADD CONSTRAINT uq_task_tenant_id       UNIQUE (tenant_id, id);
ALTER TABLE delivery.task_run      ADD CONSTRAINT uq_task_run_tenant_id   UNIQUE (tenant_id, id);
ALTER TABLE integration.inbound_event      ADD CONSTRAINT uq_inbound_event_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE integration.project_repository ADD CONSTRAINT uq_project_repository_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE integration.repo_import        ADD CONSTRAINT uq_repo_import_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE ai.analysis_run        ADD CONSTRAINT uq_analysis_run_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE ai.project_document    ADD CONSTRAINT uq_project_document_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE ai.document_revision   ADD CONSTRAINT uq_document_revision_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE ai.change_proposal     ADD CONSTRAINT uq_change_proposal_tenant_id UNIQUE (tenant_id, id);
ALTER TABLE governance.approval_request ADD CONSTRAINT uq_approval_request_tenant_id UNIQUE (tenant_id, id);

-- 索引：按租户过滤的列表查询（设计稿 §4.3 对外入口）

CREATE INDEX customer_tenant_idx ON core.customer (tenant_id, created_at DESC);
CREATE INDEX project_tenant_idx  ON core.project  (tenant_id, created_at DESC);

-- ═══════════════════════════════════════════════════════════════════
-- 5) 拆除旧的单列 FK
-- ═══════════════════════════════════════════════════════════════════

-- 5.1 A 类根表的既有关联
ALTER TABLE core.customer DROP CONSTRAINT customer_parent_id_fkey;
ALTER TABLE core.project  DROP CONSTRAINT project_customer_id_fkey;

-- 5.2 22 张表指向 core.project 的单列 FK（其余 11 张已是组合或本就无单列 FK）
ALTER TABLE core.project_member            DROP CONSTRAINT project_member_project_id_fkey;
ALTER TABLE delivery.requirement           DROP CONSTRAINT requirement_project_id_fkey;
ALTER TABLE delivery.milestone             DROP CONSTRAINT milestone_project_id_fkey;
ALTER TABLE delivery.task                  DROP CONSTRAINT task_project_id_fkey;
ALTER TABLE integration.inbound_event      DROP CONSTRAINT inbound_event_project_id_fkey;
ALTER TABLE integration.job                DROP CONSTRAINT job_project_id_fkey;
ALTER TABLE integration.notification       DROP CONSTRAINT notification_project_id_fkey;
ALTER TABLE integration.project_ssh_key    DROP CONSTRAINT project_ssh_key_project_id_fkey;
ALTER TABLE integration.project_repository DROP CONSTRAINT project_repository_project_id_fkey;
ALTER TABLE integration.repo_import        DROP CONSTRAINT repo_import_project_id_fkey;
ALTER TABLE integration.outbox_event       DROP CONSTRAINT outbox_event_project_id_fkey;
ALTER TABLE ai.project_ai_config           DROP CONSTRAINT project_ai_config_project_id_fkey;
ALTER TABLE ai.analysis_run                DROP CONSTRAINT analysis_run_project_id_fkey;
ALTER TABLE ai.agent_feedback              DROP CONSTRAINT agent_feedback_project_id_fkey;
ALTER TABLE ai.project_document            DROP CONSTRAINT project_document_project_id_fkey;
ALTER TABLE ai.document_proposal           DROP CONSTRAINT document_proposal_project_id_fkey;
ALTER TABLE ai.document_index_state        DROP CONSTRAINT document_index_state_project_id_fkey;
ALTER TABLE ai.knowledge_reference         DROP CONSTRAINT knowledge_reference_project_id_fkey;
ALTER TABLE governance.human_intervention  DROP CONSTRAINT human_intervention_project_id_fkey;
ALTER TABLE governance.audit_log           DROP CONSTRAINT audit_log_project_id_fkey;

-- 5.3 指向其他租户业务表的单列 FK（不换成组合 FK 就会允许跨租户关联）
ALTER TABLE delivery.task_supersession  DROP CONSTRAINT task_supersession_approval_request_id_fkey;
ALTER TABLE governance.human_intervention DROP CONSTRAINT human_intervention_target_proposal_id_fkey;
ALTER TABLE integration.job             DROP CONSTRAINT job_event_id_fkey;
ALTER TABLE integration.repo_import     DROP CONSTRAINT repo_import_analysis_run_id_fkey;
ALTER TABLE ai.document_proposal        DROP CONSTRAINT document_proposal_document_id_fkey;
ALTER TABLE ai.document_proposal        DROP CONSTRAINT document_proposal_base_revision_id_fkey;
ALTER TABLE ai.document_proposal        DROP CONSTRAINT document_proposal_resulting_revision_id_fkey;
ALTER TABLE ai.document_index_state     DROP CONSTRAINT document_index_state_document_revision_id_fkey;
ALTER TABLE ai.knowledge_reference      DROP CONSTRAINT knowledge_reference_document_id_fkey;
ALTER TABLE ai.knowledge_reference      DROP CONSTRAINT knowledge_reference_document_revision_id_fkey;

-- C 类原有父行 FK
ALTER TABLE governance.approval_request  DROP CONSTRAINT approval_request_proposal_id_fkey;
ALTER TABLE delivery.task_run_failure    DROP CONSTRAINT task_run_failure_run_id_fkey;
ALTER TABLE integration.repo_import_source DROP CONSTRAINT repo_import_source_import_id_fkey;
ALTER TABLE integration.repo_import_source DROP CONSTRAINT repo_import_source_repository_id_fkey;

-- ═══════════════════════════════════════════════════════════════════
-- 6) 建立组合 FK
-- ═══════════════════════════════════════════════════════════════════

-- 6.1 A 类根表
ALTER TABLE core.customer
    ADD CONSTRAINT fk_customer_parent
        FOREIGN KEY (tenant_id, parent_id) REFERENCES core.customer (tenant_id, id);
ALTER TABLE core.project
    ADD CONSTRAINT fk_project_customer
        FOREIGN KEY (tenant_id, customer_id) REFERENCES core.customer (tenant_id, id);

-- 6.2 B 类 33 张：(tenant_id, project_id) → core.project(tenant_id, id)
--     CASCADE 语义按原 FK 保持（project_ssh_key / project_repository / repo_import）。

ALTER TABLE core.project_member
    ADD CONSTRAINT fk_project_member_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.requirement
    ADD CONSTRAINT fk_requirement_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.requirement_revision
    ADD CONSTRAINT fk_requirement_revision_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.milestone
    ADD CONSTRAINT fk_milestone_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task
    ADD CONSTRAINT fk_task_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_requirement
    ADD CONSTRAINT fk_task_requirement_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_run
    ADD CONSTRAINT fk_task_run_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_supersession
    ADD CONSTRAINT fk_task_supersession_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_waiting
    ADD CONSTRAINT fk_task_waiting_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_policy_override
    ADD CONSTRAINT fk_task_policy_override_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_repository
    ADD CONSTRAINT fk_task_repository_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_run_repository
    ADD CONSTRAINT fk_task_run_repository_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_acceptance_criterion
    ADD CONSTRAINT fk_task_acceptance_criterion_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE delivery.task_knowledge_ref
    ADD CONSTRAINT fk_task_knowledge_ref_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE integration.inbound_event
    ADD CONSTRAINT fk_inbound_event_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE integration.job
    ADD CONSTRAINT fk_job_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE integration.notification
    ADD CONSTRAINT fk_notification_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE integration.project_ssh_key
    ADD CONSTRAINT fk_project_ssh_key_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE;
ALTER TABLE integration.project_repository
    ADD CONSTRAINT fk_project_repository_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE;
ALTER TABLE integration.repo_import
    ADD CONSTRAINT fk_repo_import_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE;
ALTER TABLE integration.outbox_event
    ADD CONSTRAINT fk_outbox_event_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.project_ai_config
    ADD CONSTRAINT fk_project_ai_config_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.project_skill_binding
    ADD CONSTRAINT fk_project_skill_binding_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.analysis_run
    ADD CONSTRAINT fk_analysis_run_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.change_proposal
    ADD CONSTRAINT fk_change_proposal_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.agent_feedback
    ADD CONSTRAINT fk_agent_feedback_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.project_document
    ADD CONSTRAINT fk_project_document_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.document_revision
    ADD CONSTRAINT fk_document_revision_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.document_proposal
    ADD CONSTRAINT fk_document_proposal_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.document_index_state
    ADD CONSTRAINT fk_document_index_state_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE ai.knowledge_reference
    ADD CONSTRAINT fk_knowledge_reference_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE governance.human_intervention
    ADD CONSTRAINT fk_human_intervention_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);
ALTER TABLE governance.audit_log
    ADD CONSTRAINT fk_audit_log_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id);

-- 6.3 跨表关联的单列 FK → 组合 FK（10 处）

ALTER TABLE delivery.task_supersession
    ADD CONSTRAINT fk_task_supersession_tenant_approval
        FOREIGN KEY (tenant_id, approval_request_id)
        REFERENCES governance.approval_request (tenant_id, id);
ALTER TABLE governance.human_intervention
    ADD CONSTRAINT fk_human_intervention_tenant_proposal
        FOREIGN KEY (tenant_id, target_proposal_id) REFERENCES ai.change_proposal (tenant_id, id);
ALTER TABLE integration.job
    ADD CONSTRAINT fk_job_tenant_event
        FOREIGN KEY (tenant_id, event_id) REFERENCES integration.inbound_event (tenant_id, id);
ALTER TABLE integration.repo_import
    ADD CONSTRAINT fk_repo_import_tenant_analysis
        FOREIGN KEY (tenant_id, analysis_run_id) REFERENCES ai.analysis_run (tenant_id, id);
ALTER TABLE ai.document_proposal
    ADD CONSTRAINT fk_document_proposal_tenant_document
        FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document (tenant_id, id),
    ADD CONSTRAINT fk_document_proposal_tenant_base_revision
        FOREIGN KEY (tenant_id, base_revision_id) REFERENCES ai.document_revision (tenant_id, id),
    ADD CONSTRAINT fk_document_proposal_tenant_result_revision
        FOREIGN KEY (tenant_id, resulting_revision_id) REFERENCES ai.document_revision (tenant_id, id);
ALTER TABLE ai.document_index_state
    ADD CONSTRAINT fk_document_index_state_tenant_revision
        FOREIGN KEY (tenant_id, document_revision_id)
        REFERENCES ai.document_revision (tenant_id, id) ON DELETE CASCADE;
ALTER TABLE ai.knowledge_reference
    ADD CONSTRAINT fk_knowledge_reference_tenant_document
        FOREIGN KEY (tenant_id, document_id)
        REFERENCES ai.project_document (tenant_id, id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_knowledge_reference_tenant_revision
        FOREIGN KEY (tenant_id, document_revision_id)
        REFERENCES ai.document_revision (tenant_id, id) ON DELETE CASCADE;

-- 6.4 C 类 3 张

ALTER TABLE governance.approval_request
    ADD CONSTRAINT fk_approval_request_tenant_proposal
        FOREIGN KEY (tenant_id, proposal_id) REFERENCES ai.change_proposal (tenant_id, id);
ALTER TABLE delivery.task_run_failure
    ADD CONSTRAINT fk_task_run_failure_tenant_run
        FOREIGN KEY (tenant_id, run_id) REFERENCES delivery.task_run (tenant_id, id);
ALTER TABLE integration.repo_import_source
    ADD CONSTRAINT fk_ris_tenant_import
        FOREIGN KEY (tenant_id, import_id)
        REFERENCES integration.repo_import (tenant_id, id) ON DELETE CASCADE,
    ADD CONSTRAINT fk_ris_tenant_repository
        FOREIGN KEY (tenant_id, repository_id)
        REFERENCES integration.project_repository (tenant_id, id) ON DELETE CASCADE;

-- 6.5 D 类伪根表：主键改租户内（I4-4）

ALTER TABLE delivery.policy_scope DROP CONSTRAINT policy_scope_pkey;
ALTER TABLE delivery.policy_scope ADD PRIMARY KEY (tenant_id, scope, scope_key);

-- ═══════════════════════════════════════════════════════════════════
-- 7) I4：全局唯一 → 租户内唯一（4 处）
-- ═══════════════════════════════════════════════════════════════════

-- I4-1 core.project.code
ALTER TABLE core.project DROP CONSTRAINT project_code_key;
CREATE UNIQUE INDEX project_code_uniq ON core.project (tenant_id, code);

-- I4-2 core.customer.code（原为部分唯一索引；索引归属 core schema，DROP 需带前缀）
DROP INDEX core.customer_code_uniq;
CREATE UNIQUE INDEX customer_code_uniq ON core.customer (tenant_id, code)
    WHERE code IS NOT NULL;

-- I4-3 integration.outbox_event.idempotency_key
ALTER TABLE integration.outbox_event DROP CONSTRAINT outbox_event_idempotency_key_key;
ALTER TABLE integration.outbox_event
    ADD CONSTRAINT outbox_event_tenant_idempotency_key UNIQUE (tenant_id, idempotency_key);

-- I4-4 已在 6.5 完成
