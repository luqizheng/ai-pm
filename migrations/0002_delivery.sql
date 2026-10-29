-- 0002_delivery: requirements, revisions, tasks, milestones (architecture doc §6.2–6.3, verbatim)

CREATE TABLE delivery.requirement (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    code TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    current_revision_id UUID,
    row_revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, code),
    UNIQUE (project_id, id)
);

CREATE TABLE delivery.requirement_revision (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    requirement_id UUID NOT NULL,
    project_id UUID NOT NULL,
    version INT NOT NULL CHECK (version > 0),
    content JSONB NOT NULL,
    source_artifact_id UUID,
    state TEXT NOT NULL CHECK (
        state IN ('draft', 'approved', 'superseded')
    ),
    content_hash TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES core.principal(id),
    approved_by UUID REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (requirement_id, version),
    UNIQUE (requirement_id, id),
    FOREIGN KEY (project_id, requirement_id)
        REFERENCES delivery.requirement(project_id, id)
);

ALTER TABLE delivery.requirement
ADD CONSTRAINT fk_current_requirement_revision
FOREIGN KEY (id, current_revision_id)
REFERENCES delivery.requirement_revision(requirement_id, id);

CREATE TABLE delivery.milestone (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    name TEXT NOT NULL,
    planned_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'planned',
    revision BIGINT NOT NULL DEFAULT 1,
    UNIQUE (project_id, id)
);

CREATE TABLE delivery.task (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    milestone_id UUID,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'backlog',
    priority TEXT NOT NULL DEFAULT 'normal',
    assignee_id UUID REFERENCES core.principal(id),
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, milestone_id)
        REFERENCES delivery.milestone(project_id, id)
);

CREATE TABLE delivery.task_requirement (
    project_id UUID NOT NULL,
    task_id UUID NOT NULL,
    requirement_id UUID NOT NULL,
    revision_id UUID NOT NULL,
    PRIMARY KEY (task_id, revision_id),
    FOREIGN KEY (project_id, task_id)
        REFERENCES delivery.task(project_id, id),
    FOREIGN KEY (project_id, requirement_id)
        REFERENCES delivery.requirement(project_id, id),
    FOREIGN KEY (requirement_id, revision_id)
        REFERENCES delivery.requirement_revision(requirement_id, id)
);
