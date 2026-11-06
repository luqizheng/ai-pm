-- S02-I3 · Source & Traceability（docs/s02-i3-source-traceability-implementation-v1.md）
-- 契约：S02-04 V1.2 §9（Q-4/R5）+ §5.4/S02D-15/19（Q-18 选项 A）+ §7.4 职责表。
-- Q-20：旧列 source_artifact_id 已于 0039 DROP，无继承关系。
-- 本切片裁决：repository_change_intake 候选延后 I7（无消费方不建空表，克制原则）；
--             Decision 治理关联 = ai.change_proposal.target_type 扩值，列无 CHECK 零迁移。

CREATE SCHEMA IF NOT EXISTS source;

-- 来源材料（输入侧实体；五类枚举 R-13/C-1，intake 类随 S02-I7 启用）
CREATE TABLE source.source_input (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL,
    project_id  UUID NOT NULL,
    input_kind  TEXT NOT NULL
                CHECK (input_kind IN ('human_input','discovery','external_feedback','intake','import')),
    title       TEXT NOT NULL,
    content     TEXT,
    source_ref  TEXT,
    created_by  UUID NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id)
);
CREATE INDEX idx_source_input_project ON source.source_input (tenant_id, project_id, created_at DESC);

-- 来源↔需求 多对多（来源属需求级、修订继承——S02D-19，不挂 revision_id）
CREATE TABLE delivery.requirement_source_input (
    requirement_id UUID NOT NULL,
    input_id       UUID NOT NULL,
    tenant_id      UUID NOT NULL,
    project_id     UUID NOT NULL,
    relation       TEXT NOT NULL DEFAULT 'source',
    created_by     UUID NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (requirement_id, input_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id),
    FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id),
    FOREIGN KEY (project_id, input_id) REFERENCES source.source_input(project_id, id)
);
CREATE INDEX idx_req_source_input_req ON delivery.requirement_source_input (requirement_id);

-- Finding↔需求 多对多（§9.2-2，relation 单值域 supports；不复制 Finding 内容进正文）
CREATE TABLE delivery.requirement_finding (
    requirement_id UUID NOT NULL,
    finding_id     UUID NOT NULL,
    tenant_id      UUID NOT NULL,
    project_id     UUID NOT NULL,
    relation       TEXT NOT NULL DEFAULT 'supports' CHECK (relation = 'supports'),
    created_by     UUID NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (requirement_id, finding_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id),
    FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id),
    FOREIGN KEY (tenant_id, project_id, finding_id)
        REFERENCES discovery.discovery_finding(tenant_id, project_id, id)
);
CREATE INDEX idx_req_finding_req ON delivery.requirement_finding (requirement_id);
CREATE INDEX idx_req_finding_finding ON delivery.requirement_finding (finding_id);