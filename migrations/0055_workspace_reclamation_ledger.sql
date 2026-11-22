-- PA08-04: persist resource reclamation independently from Run process termination.
-- The ledger tracks only derived local bytes; it never owns Repository/Run history.
CREATE TABLE delivery.workspace_reclamation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    resource_kind text NOT NULL,
    run_id uuid,
    repository_id uuid NOT NULL,
    relative_path text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 8 NOT NULL,
    next_retry_at timestamp with time zone DEFAULT now() NOT NULL,
    claim_token uuid,
    last_error text,
    completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT workspace_reclamation_pkey PRIMARY KEY (id),
    CONSTRAINT workspace_reclamation_kind_check
        CHECK (resource_kind = ANY (ARRAY['run_worktree'::text, 'repository_mirror'::text])),
    CONSTRAINT workspace_reclamation_status_check
        CHECK (status = ANY (ARRAY['pending'::text, 'cleaning'::text, 'completed'::text, 'needs_human'::text])),
    CONSTRAINT workspace_reclamation_resource_shape_check
        CHECK (
            (resource_kind = 'run_worktree' AND run_id IS NOT NULL)
            OR (resource_kind = 'repository_mirror' AND run_id IS NULL)
        ),
    CONSTRAINT workspace_reclamation_path_check
        CHECK (length(btrim(relative_path)) > 0),
    CONSTRAINT workspace_reclamation_attempts_check
        CHECK (attempts >= 0 AND max_attempts > 0),
    CONSTRAINT workspace_reclamation_completed_check
        CHECK (completed_at IS NULL OR status = 'completed'),
    CONSTRAINT fk_workspace_reclamation_tenant_project
        FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE RESTRICT,
    CONSTRAINT fk_workspace_reclamation_project_repository
        FOREIGN KEY (project_id, repository_id) REFERENCES integration.project_repository(project_id, id) ON DELETE RESTRICT,
    CONSTRAINT fk_workspace_reclamation_run_repository
        FOREIGN KEY (run_id, repository_id) REFERENCES delivery.task_run_repository(run_id, repository_id) ON DELETE RESTRICT
);

-- One reclamation record per Run/repository worktree, and one per shared mirror.
CREATE UNIQUE INDEX uq_workspace_reclamation_run_worktree
    ON delivery.workspace_reclamation (run_id, repository_id)
    WHERE resource_kind = 'run_worktree';

CREATE UNIQUE INDEX uq_workspace_reclamation_repository_mirror
    ON delivery.workspace_reclamation (repository_id)
    WHERE resource_kind = 'repository_mirror';

CREATE INDEX idx_workspace_reclamation_due
    ON delivery.workspace_reclamation (next_retry_at, created_at)
    WHERE status IN ('pending', 'cleaning');

COMMENT ON TABLE delivery.workspace_reclamation IS
    'Lifecycle ledger for derived local Run worktrees and shared Git mirrors; never deletes Run, Repository, Commit, artifact, or audit records.';
COMMENT ON COLUMN delivery.workspace_reclamation.relative_path IS
    'Validated relative path: relative to the Run root for run_worktree, or to the configured workspace root for repository_mirror; absolute paths are never persisted.';
COMMENT ON COLUMN delivery.workspace_reclamation.next_retry_at IS
    'Pending retry due time; while cleaning, the claim lease expiry used for crash recovery.';
COMMENT ON COLUMN delivery.workspace_reclamation.claim_token IS
    'Fencing token for the current filesystem reclamation Worker claim.';
