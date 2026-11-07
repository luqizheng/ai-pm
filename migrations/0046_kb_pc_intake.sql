-- 0046_kb_pc_intake：KB-PC 服务化 Phase 3 · Intake 统一子任务载体
-- （docs/architecture/knowledge-services/kb-pc-independent-services-architecture-v1.1.md §9；
--   基础形态沿 V1.0 §9.2；评审 N-8 隔离范式补齐）。
-- 编号注：以开工时 migrations/ 目录实况起号——0015~0045 已占（0029 为历史
-- 空号），0045 被 S02-I3（DEF-01 顺移）占用，本批次自 0046 起。
-- 红线：本迁移**不建** kb.*/pc.* 表（ADR-06b 先于 Phase 5/0045，仅 intake 两表）。
-- DDL 注（实施偏差登记，回填 V1.1 §9.2 ChangeLog）：intake_record 增
-- idempotency_key 列 + UNIQUE(tenant_id, project_id, idempotency_key)——
-- V1.1 §9.5-7（D-12：调用方凭 Idempotency-Key 安全重试）的落地载体，
-- V1.0 §9.2 原稿未列该列。

-- ── ① intake_record：接收壳（L1）──────────────────────────────────────────
CREATE TABLE integration.intake_record (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL,
    project_id        UUID NOT NULL,
    intake_type       TEXT NOT NULL CHECK (intake_type IN (
                        'document_upload','meeting_notes','product_fact',
                        'product_decision','engineering_artifact',
                        'engineering_result','external_change','analysis_request')),
    channel           TEXT NOT NULL CHECK (channel IN ('web','mcp','agent_api','system')),
    submitted_by      UUID NOT NULL REFERENCES core.principal(id),
    source_ref        JSONB NOT NULL,
        -- §7.4 Source Reference：{kind, uri|inline_ref, content_hash, size, mime}
    content_hash      TEXT,
        -- 文本类 = sha256(content)；文件类 = sha256(bytes)（对账锚）
    title             TEXT,
    intake_status     TEXT NOT NULL DEFAULT 'RECEIVED' CHECK (intake_status IN (
                        'RECEIVED','PERSISTED','ROUTED','CLOSED','FAILED','CANCELLED')),
        -- L1 接收壳生命周期（V1.0 §9.4）；受理端点同事务三写成功即 'ROUTED'
    idempotency_key   TEXT NOT NULL,
        -- D-12 幂等重放锚（见头部实施偏差注）
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
        -- 0019 租户级身份键（intake_task 组合 FK 目标锚，评审 N-8）
    UNIQUE (tenant_id, project_id, idempotency_key),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);
CREATE INDEX idx_intake_record_project
    ON integration.intake_record (tenant_id, project_id, created_at DESC);

-- ── ② intake_task：统一子任务载体（L2；V1.1 §9.2 DDL 为准）─────────────────
CREATE TABLE integration.intake_task (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL,
    project_id        UUID NOT NULL,
    intake_id         UUID NOT NULL,
    target_service    TEXT NOT NULL CHECK (target_service IN ('kb','pc')),
    task_type         TEXT NOT NULL CHECK (task_type IN
                        ('kb_analyze','pc_analyze','pc_decide','kb_archive','kb_supply')),
    phase             TEXT NOT NULL DEFAULT 'required' CHECK (phase IN ('required','optional')),
        -- 必需阶段：父 Intake 完成的前置；可选阶段：SKIPPED 不阻塞（§9.4.2）
    task_status       TEXT NOT NULL DEFAULT 'PENDING' CHECK (task_status IN
                        ('PENDING','DISPATCHED','PROCESSING','COMPLETED','FAILED',
                         'NEEDS_HUMAN_REVIEW','SKIPPED','CANCELLED')),
        -- 大写枚举为有意选择：L1/L2 层 Intake 状态机惯例（§9.2 评审 N-8 声明），
        -- 与仓库领域数据枚举的小写 CHECK 惯例分属不同层级
    attempt           INT NOT NULL DEFAULT 0,
    idempotency_key   TEXT NOT NULL,
    signal_ref        JSONB,
        -- 触发来源（kb.cognition_signal 摘要：relevance/confidence/四元组引用；
        -- 多信号合并为 {"signals":[...]} 数组形态，§9.4.3-1）
    result_ref        JSONB,
        -- DomainRef 四元组（§7.3；禁止内嵌 service 字段）
    last_error        TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, project_id, idempotency_key),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, intake_id) REFERENCES integration.intake_record (tenant_id, id)
        -- 0019 组合模式（评审 N-8 补齐）
);

-- 同一 Intake 下同类型任务至多一个活跃实例（防多信号/重试重复创建，D-2 配套）。
CREATE UNIQUE INDEX uq_intake_task_active
    ON integration.intake_task (intake_id, task_type)
    WHERE task_status IN ('PENDING','DISPATCHED','PROCESSING');
-- 终态任务不受限：FAILED 后人工重试 = 复用原行（attempt+1 重置 PENDING），不建新行。

CREATE INDEX idx_intake_task_intake
    ON integration.intake_task (intake_id, created_at);
CREATE INDEX idx_intake_task_status
    ON integration.intake_task (task_status, updated_at)
    WHERE task_status IN ('PENDING','DISPATCHED','PROCESSING');
