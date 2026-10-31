-- 0005_audit_notify: fan out new audit rows over PG LISTEN/NOTIFY.
-- pm-api keeps a dedicated LISTEN connection on channel 'pm_audit' and replays
-- each payload onto its in-process SSE broadcast. Deliberately trigger-based:
-- every writer (pm_write, command_executor, MCP, seed scripts, parallel
-- sessions) is covered without touching business code.
-- Payload carries only the contract fields (no before/after JSONB), keeping it
-- far below the 8000-byte pg_notify limit.

CREATE OR REPLACE FUNCTION governance.notify_audit_insert()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM pg_notify(
        'pm_audit',
        json_build_object(
            'id', NEW.id::text,
            'project_id', NEW.project_id::text,
            'actor_id', NEW.actor_id::text,
            'action', NEW.action,
            'target_type', NEW.target_type,
            'target_id', NEW.target_id::text,
            'created_at', to_char(NEW.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')
        )::text
    );
    RETURN NEW;
END;
$$;

CREATE TRIGGER audit_log_notify
    AFTER INSERT ON governance.audit_log
    FOR EACH ROW
    EXECUTE FUNCTION governance.notify_audit_insert();