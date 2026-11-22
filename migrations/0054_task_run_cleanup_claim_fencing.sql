-- PA08-04: fence process-cleanup worker claims and give cleaning rows a lease expiry.
ALTER TABLE delivery.task_run_cleanup
    ADD COLUMN IF NOT EXISTS claim_token uuid;

COMMENT ON COLUMN delivery.task_run_cleanup.claim_token IS
    'Fencing token for the current cleanup Worker claim; cleared on completion, failure, or manual requeue.';

-- next_retry_at is the due time while pending and the claim lease expiry while cleaning.
COMMENT ON COLUMN delivery.task_run_cleanup.next_retry_at IS
    'Pending retry due time; while status=cleaning, this is the claim lease expiry used for crash recovery.';
