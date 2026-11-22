-- PA08-06: retain logical byte accounting for completed local-resource reclamation.
-- This is a logical file-size estimate, not a guarantee of physical blocks freed.
ALTER TABLE delivery.workspace_reclamation
    ADD COLUMN reclaimed_bytes bigint NOT NULL DEFAULT 0
    CONSTRAINT workspace_reclamation_reclaimed_bytes_check CHECK (reclaimed_bytes >= 0);

COMMENT ON COLUMN delivery.workspace_reclamation.reclaimed_bytes IS
    'Logical file bytes removed by successful reclamation; not a physical disk-block accounting guarantee.';
