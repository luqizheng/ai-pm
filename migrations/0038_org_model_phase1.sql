-- 0038: P4-ORG Phase 1 组织关系数据模型（基线 V0.1 §2；DR-ORG-01~05，2026-11-19 签发）
-- Team（长期组织单元，无解散终态）/ TeamMember(owner|member, 多 Owner) /
-- Product（归属唯一 Team，无成员名单）/ Product↔Project 多对多关联。
-- 无存量数据：直接建表，不做数据搬运与兼容（AGENTS.md 规则 1）。

CREATE TABLE core.team (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    name text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT fk_team_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

-- PK 单独声明（沿 0001 基线惯例：建表后补 <table>_pkey）。
ALTER TABLE core.team ADD CONSTRAINT team_pkey PRIMARY KEY (id);

CREATE TABLE core.team_member (
    team_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    role text NOT NULL,
    granted_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_team_member_role CHECK ((role = ANY (ARRAY['owner'::text, 'member'::text]))),
    CONSTRAINT fk_team_member_team FOREIGN KEY (team_id) REFERENCES core.team(id) ON DELETE CASCADE,
    CONSTRAINT fk_team_member_principal FOREIGN KEY (principal_id) REFERENCES core.principal(id),
    CONSTRAINT fk_team_member_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.team_member ADD CONSTRAINT team_member_pkey PRIMARY KEY (team_id, principal_id);

CREATE TABLE core.product (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    team_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    name text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT fk_product_team FOREIGN KEY (team_id) REFERENCES core.team(id),
    CONSTRAINT fk_product_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.product ADD CONSTRAINT product_pkey PRIMARY KEY (id);

CREATE TABLE core.product_project_link (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    product_id uuid NOT NULL,
    project_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_product_project_link_pair UNIQUE (product_id, project_id),
    CONSTRAINT fk_product_project_link_product FOREIGN KEY (product_id) REFERENCES core.product(id) ON DELETE CASCADE,
    CONSTRAINT fk_product_project_link_project FOREIGN KEY (project_id) REFERENCES core.project(id),
    CONSTRAINT fk_product_project_link_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.product_project_link ADD CONSTRAINT product_project_link_pkey PRIMARY KEY (id);

CREATE INDEX idx_team_tenant ON core.team (tenant_id);
CREATE INDEX idx_team_member_principal ON core.team_member (principal_id);
CREATE INDEX idx_team_member_tenant ON core.team_member (tenant_id);
CREATE INDEX idx_product_team ON core.product (team_id);
CREATE INDEX idx_product_tenant ON core.product (tenant_id);
CREATE INDEX idx_product_project_link_product ON core.product_project_link (product_id);
CREATE INDEX idx_product_project_link_project ON core.product_project_link (project_id);
