-- 0001_core: identity and projects (architecture doc §6.1, verbatim)
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE SCHEMA core;
CREATE SCHEMA delivery;
CREATE SCHEMA integration;
CREATE SCHEMA ai;
CREATE SCHEMA governance;

CREATE TABLE core.customer (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE core.project (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    customer_id UUID NOT NULL
        REFERENCES core.customer(id),
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'planning',
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (id, customer_id)
);

CREATE TABLE core.principal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL CHECK (
        kind IN ('human', 'dev_agent', 'ai_pm', 'service')
    ),
    display_name TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE core.project_member (
    project_id UUID NOT NULL REFERENCES core.project(id),
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    role TEXT NOT NULL,
    PRIMARY KEY (project_id, principal_id)
);
