-- PA08-06: preserve cumulative reclaimed bytes across repeated reclamation
-- attempts of a ledger resource. The per-row field remains the latest operation;
-- this table is the monotonic aggregate used by Prometheus counters.
CREATE TABLE IF NOT EXISTS delivery.workspace_reclamation_totals (
    resource_kind TEXT PRIMARY KEY
        CHECK (resource_kind IN ('run_worktree', 'repository_mirror')),
    total_reclaimed_bytes BIGINT NOT NULL DEFAULT 0
        CHECK (total_reclaimed_bytes >= 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE delivery.workspace_reclamation_totals IS
    'Monotonic cumulative logical file bytes reclaimed by resource kind; not physical disk-block accounting.';
