-- 0004_audit_source: carry the command-source identity on audit rows.
-- Task 6 fix round 1: execute() only persisted AiProposal.run_id (analysis_run_id);
-- RedlineApproval / HumanIntervention ids had no join anchor in the audit row.
-- The Task 15 approval flow joins approvals to their audit rows via
-- (source_kind, source_ref). Nullable: pre-0004 rows stay NULL.

ALTER TABLE governance.audit_log
    ADD COLUMN source_kind TEXT,
    ADD COLUMN source_ref UUID;
