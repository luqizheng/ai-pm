-- 0052 · Issue / Risk / Blocker 领域实体（PA-05-01 收尾；2026-11-21）
--
-- PA-05-01 列明的三个「有明确领域语义的对象」落到 delivery schema（与
-- requirement/task 同域）。V1 最小属性集：标题/描述/严重度或影响概率/
-- 生命周期状态；各自独立状态机，经 PA-05-02 统一语义映射（服务层显式
-- 声明）进 core.work_item 注册表与 Activity Timeline。终态完整性约束
-- 沿 human_consultation（0046）同款纪律：resolved/closed/cleared 必带时间。

CREATE TABLE delivery.issue (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    description text,
    severity text NOT NULL DEFAULT 'medium'::text,
    status text NOT NULL DEFAULT 'open'::text,
    resolved_at timestamp with time zone,
    cancelled_reason text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_issue_severity CHECK (
        severity = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text])
    ),
    CONSTRAINT ck_issue_status CHECK (
        status = ANY (ARRAY['open'::text, 'resolved'::text, 'cancelled'::text])
    ),
    CONSTRAINT ck_issue_resolved_complete CHECK (
        status <> 'resolved' OR resolved_at IS NOT NULL
    )
);

ALTER TABLE delivery.issue ADD CONSTRAINT issue_pkey PRIMARY KEY (id);
CREATE INDEX ix_issue_project ON delivery.issue USING btree (tenant_id, project_id, status);

CREATE TABLE delivery.risk (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    description text,
    impact text NOT NULL DEFAULT 'medium'::text,
    probability text NOT NULL DEFAULT 'medium'::text,
    status text NOT NULL DEFAULT 'identified'::text,
    closed_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_risk_impact CHECK (
        impact = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text])
    ),
    CONSTRAINT ck_risk_probability CHECK (
        probability = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text])
    ),
    CONSTRAINT ck_risk_status CHECK (
        status = ANY (ARRAY['identified'::text, 'mitigating'::text, 'closed'::text])
    ),
    CONSTRAINT ck_risk_closed_complete CHECK (
        status <> 'closed' OR closed_at IS NOT NULL
    )
);

ALTER TABLE delivery.risk ADD CONSTRAINT risk_pkey PRIMARY KEY (id);
CREATE INDEX ix_risk_project ON delivery.risk USING btree (tenant_id, project_id, status);

CREATE TABLE delivery.blocker (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    description text,
    blocked_task_id uuid REFERENCES delivery.task(id),
    status text NOT NULL DEFAULT 'open'::text,
    cleared_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_blocker_status CHECK (
        status = ANY (ARRAY['open'::text, 'cleared'::text])
    ),
    CONSTRAINT ck_blocker_cleared_complete CHECK (
        status <> 'cleared' OR cleared_at IS NOT NULL
    )
);

ALTER TABLE delivery.blocker ADD CONSTRAINT blocker_pkey PRIMARY KEY (id);
CREATE INDEX ix_blocker_project ON delivery.blocker USING btree (tenant_id, project_id, status);

-- 统一事项注册表的 target 类型域扩展（0051 原约束仅 task/requirement/adr）。
ALTER TABLE core.work_item DROP CONSTRAINT ck_work_item_target;
ALTER TABLE core.work_item ADD CONSTRAINT ck_work_item_target CHECK (
    target_type = ANY (ARRAY['task'::text, 'requirement'::text, 'adr'::text,
                             'issue'::text, 'risk'::text, 'blocker'::text])
);
