#!/usr/bin/env python3
"""SEM Callback Inbox acceptance harness — real PostgreSQL, two connections.

TEST-ONLY. Creates an ephemeral database, applies fixture_schema.sql
(candidate §14 DDL + mirrored baseline constraints), executes the INBOX/BIND/
UNBOUND acceptance scenarios with independent connections and explicit
barriers, then drops the database.

This harness validates the candidate DDL and transaction algorithm from
docs/architecture/server-execution-management/sem-callback-inbox-ddl-and-
concurrency-design.md. It is NOT the production adapter, NOT an ai-pm
migration, and NOT bilateral live HTTP integration.

Usage:
    python run_acceptance.py [--dsn postgres://aipm:aipm@127.0.0.1:16433/postgres]
"""
from __future__ import annotations

import argparse
import json
import sys
import threading
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, Callable, Optional

import psycopg2
from psycopg2 import errors as pg_errors

TENANT = str(uuid.uuid4())
PROJECT = str(uuid.uuid4())
TASK = str(uuid.uuid4())
SEM_PRINCIPAL = str(uuid.uuid4())
RUNNER = "55555555-5555-4555-8555-555555555555"
SUPERVISOR = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
BOOT = "boot-01"
DELIVERY = "77777777-7777-4777-8777-777777777777"
UUID_RE = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"


def jcs(value: Any) -> bytes:
    """RFC 8785 canonicalization for the ASCII/integer subset used here."""
    if isinstance(value, dict):
        body = ",".join(
            json.dumps(k, ensure_ascii=False) + ":" + jcs(v).decode("utf-8")
            for k, v in sorted(value.items(), key=lambda kv: kv[0])
        )
        return ("{" + body + "}").encode("utf-8")
    if isinstance(value, list):
        return ("[" + ",".join(jcs(v).decode("utf-8") for v in value) + "]").encode("utf-8")
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def sha256_hex(value: Any) -> str:
    import hashlib

    return hashlib.sha256(jcs(value)).hexdigest()


def reject_duplicate_members(pairs):
    seen = set()
    for key, _ in pairs:
        if key in seen:
            raise ValueError(f"duplicate JSON member: {key}")
        seen.add(key)
    return dict(pairs)


def parse_strict(body: str) -> Any:
    return json.loads(body, object_pairs_hook=reject_duplicate_members)


def envelope(
    event_type: str,
    execution_id: str,
    sequence: int,
    payload: dict,
    *,
    event_id: Optional[str] = None,
    occurred_at: str = "2026-11-28T12:00:00Z",
    schema_version: int = 1,
) -> dict:
    return {
        "event_id": event_id or str(uuid.uuid4()),
        "event_type": event_type,
        "schema_version": schema_version,
        "occurred_at": occurred_at,
        "execution_id": execution_id,
        "execution_event_sequence": sequence,
        "payload": payload,
    }


def binding_payload(epoch: int, *, runner=RUNNER, key_version=1, boot=BOOT, supervisor=SUPERVISOR, delivery=DELIVERY) -> dict:
    return {
        "binding_epoch": epoch,
        "runner": {
            "runner_id": runner,
            "runner_key_version": key_version,
            "boot_id": boot,
            "supervisor_instance_id": supervisor,
        },
        "delivery_id": delivery,
        "reserved_capacity": {"cpu_millicores": 1000, "memory_bytes": 1073741824, "workspace_bytes": 1073741824},
        "established_at": "2026-11-28T12:00:00Z",
    }


def binding_runner_tuple(payload: dict) -> tuple:
    """binding_established nests the tuple under payload.runner; the other four
    binding-scoped event types carry the identity at the payload top level."""
    if "runner" in payload:
        runner = payload["runner"]
        return (
            runner["runner_id"],
            runner["runner_key_version"],
            runner["boot_id"],
            runner["supervisor_instance_id"],
            payload["delivery_id"],
        )
    return (
        payload["runner_id"],
        payload["runner_key_version"],
        payload["boot_id"],
        payload["supervisor_instance_id"],
        None,
    )


def unbound_payload(release_id: str, fence_id: str) -> dict:
    return {
        "release_id": release_id,
        "dispatch_fence_id": fence_id,
        "disposition": "released_unbound",
        "applied_at": "2026-11-28T12:00:00Z",
    }


class Rejected(Exception):
    """Application-level rejection: 404/409/422 semantics; no writes."""

    def __init__(self, code: str, detail: str = ""):
        super().__init__(f"{code}: {detail}")
        self.code = code


class IntegrityFailure(Exception):
    """Generic integrity failure: rollback, never ACK."""


@dataclass
class Ctx:
    conn: psycopg2.extensions.connection
    cur: psycopg2.extensions.cursor
    execution_id: str


def fetch_projection(cur, execution_id: str) -> Optional[dict]:
    cur.execute(
        "SELECT dispatch_state, binding_epoch, runner_id, runner_key_version, boot_id,"
        " supervisor_instance_id, delivery_id, stop_fence_id, process_exit_evidence_id,"
        " host_isolation_disposition_id, dispatch_fence_id, binding_event_id, binding_event_sequence,"
        " last_event_id, last_event_sequence, updated_at"
        " FROM delivery.task_run_sem_execution WHERE task_run_id = %s",
        (execution_id,),
    )
    row = cur.fetchone()
    if row is None:
        return None
    cols = [
        "dispatch_state", "binding_epoch", "runner_id", "runner_key_version", "boot_id",
        "supervisor_instance_id", "delivery_id", "stop_fence_id", "process_exit_evidence_id",
        "host_isolation_disposition_id", "dispatch_fence_id", "binding_event_id",
        "binding_event_sequence", "last_event_id", "last_event_sequence", "updated_at",
    ]
    return dict(zip(cols, row))


def projection_row_for_update(cur, execution_id: str) -> Optional[dict]:
    cur.execute(
        "SELECT task_run_id FROM delivery.task_run_sem_execution WHERE task_run_id = %s FOR UPDATE",
        (execution_id,),
    )
    if cur.fetchone() is None:
        return None
    return fetch_projection(cur, execution_id)


def validate_binding_tuple(proj: dict, payload: dict, event: dict) -> None:
    """§5 step 2 / §14.2: compare the event's binding identity against the
    current accepted binding. Only events whose binding identity matches the
    projected binding may apply. delivery_id participates only when the event
    carries it (binding_established)."""
    incoming = binding_runner_tuple(payload)
    fields = ["runner_id", "runner_key_version", "boot_id", "supervisor_instance_id"]
    if "delivery_id" in payload:
        fields.append("delivery_id")
    incoming = incoming[: len(fields)]
    current = tuple(proj[f] for f in fields)
    if incoming != current:
        raise Rejected("BINDING_IDENTITY_MISMATCH", f"incoming tuple {incoming} != current {current}")
    if payload["binding_epoch"] != proj["binding_epoch"]:
        raise Rejected("STALE_BINDING_EPOCH", f"event epoch {payload['binding_epoch']} != projection {proj['binding_epoch']}")


def apply_binding_established(ctx: Ctx, event: dict) -> None:
    payload = event["payload"]
    epoch = payload["binding_epoch"]
    if epoch < 1:
        raise Rejected("INVALID_EPOCH", "binding_epoch >= 1 required")
    proj = projection_row_for_update(ctx.cur, ctx.execution_id)
    if proj is None or proj["dispatch_state"] == "eligible":
        # case 1: first bind
        if epoch != 1:
            raise Rejected("FIRST_BIND_EPOCH_MUST_BE_1", f"got {epoch}")
        runner = payload["runner"]
        ctx.cur.execute(
            "INSERT INTO delivery.task_run_sem_execution"
            " (task_run_id, tenant_id, project_id, dispatch_state, binding_epoch, runner_id,"
            "  runner_key_version, boot_id, supervisor_instance_id, delivery_id,"
            "  binding_event_id, binding_event_sequence, last_event_id, last_event_sequence,"
            "  first_bound_at, updated_at)"
            " VALUES (%s,%s,%s,'bound',%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,now(),now())",
            (
                ctx.execution_id, TENANT, PROJECT, epoch,
                runner["runner_id"], runner["runner_key_version"], runner["boot_id"],
                runner["supervisor_instance_id"], payload["delivery_id"],
                event["event_id"], event["execution_event_sequence"],
                event["event_id"], event["execution_event_sequence"],
            ),
        )
        return
    # existing binding
    if proj["dispatch_state"] == "released_unbound":
        # INBOX-53 / UNBOUND-04: tombstone is permanent
        raise Rejected("EXECUTION_RELEASED_UNBOUND", "later binding after released_unbound tombstone")
    if proj["dispatch_state"] in ("uncertain",):
        # case 7: uncertain blocks rebind
        raise Rejected("PRIOR_BINDING_UNCERTAIN", "rebind forbidden while prior binding uncertain")
    if epoch == proj["binding_epoch"]:
        # case 3: same epoch must be the exact same tuple
        validate_binding_tuple(proj, payload, event)
        raise Rejected("DUPLICATE_SEQUENCE_OWNED", "same epoch re-delivered under a new event id")
    if epoch < proj["binding_epoch"]:
        # case 4: stale
        raise Rejected("STALE_BINDING_EPOCH", f"event epoch {epoch} < projection {proj['binding_epoch']}")
    # case 5/6/8/9: higher epoch requires positive release proof or isolation
    if proj["dispatch_state"] == "released":
        if proj["process_exit_evidence_id"] is None and proj["stop_fence_id"] is None:
            raise IntegrityFailure("released projection without evidence")
        _rebind(ctx, event, proj)
        return
    if proj["dispatch_state"] == "host_isolated":
        if proj["host_isolation_disposition_id"] is None:
            raise IntegrityFailure("host_isolated without disposition")
        _rebind(ctx, event, proj)
        return
    raise Rejected(
        "REBIND_NOT_AUTHORIZED",
        f"prior state {proj['dispatch_state']} provides no positive release/isolation proof",
    )


def _rebind(ctx: Ctx, event: dict, proj: dict) -> None:
    payload = event["payload"]
    runner = payload["runner"]
    if payload["binding_epoch"] <= proj["binding_epoch"]:
        raise Rejected("STALE_BINDING_EPOCH", "rebind epoch must be strictly greater")
    ctx.cur.execute(
        "UPDATE delivery.task_run_sem_execution SET"
        " dispatch_state='bound', binding_epoch=%s, runner_id=%s, runner_key_version=%s,"
        " boot_id=%s, supervisor_instance_id=%s, delivery_id=%s,"
        " launch_id=NULL, containment_id=NULL, process_start_identity=NULL, systemd_invocation_id=NULL,"
        " stop_fence_id=NULL, process_exit_evidence_id=NULL, host_isolation_disposition_id=NULL,"
        " binding_event_id=%s, binding_event_sequence=%s, last_event_id=%s, last_event_sequence=%s,"
        " first_bound_at=now(), released_at=NULL, updated_at=now()"
        " WHERE task_run_id=%s",
        (
            payload["binding_epoch"], runner["runner_id"], runner["runner_key_version"],
            runner["boot_id"], runner["supervisor_instance_id"], payload["delivery_id"],
            event["event_id"], event["execution_event_sequence"],
            event["event_id"], event["execution_event_sequence"],
            ctx.execution_id,
        ),
    )


def apply_infrastructure_uncertain(ctx: Ctx, event: dict) -> None:
    payload = event["payload"]
    if payload.get("reservation_retained") is not True:
        raise Rejected("UNCERTAIN_MUST_RETAIN_RESERVATION", "reservation_retained must be true")
    proj = projection_row_for_update(ctx.cur, ctx.execution_id)
    if proj is None:
        raise Rejected("NO_BINDING_FOR_UNCERTAINTY", "uncertainty requires an existing binding")
    validate_binding_tuple(proj, payload, event)
    if proj["dispatch_state"] in ("released", "released_unbound"):
        raise Rejected("UNCERTAINTY_AFTER_TERMINAL", "cannot uncertaintify a terminal state")
    ctx.cur.execute(
        "UPDATE delivery.task_run_sem_execution SET dispatch_state='uncertain',"
        " last_event_id=%s, last_event_sequence=%s, updated_at=now()"
        " WHERE task_run_id=%s",
        (event["event_id"], event["execution_event_sequence"], ctx.execution_id),
    )


def apply_release_status(ctx: Ctx, event: dict) -> None:
    payload = event["payload"]
    state = payload["release_state"]
    proj = projection_row_for_update(ctx.cur, ctx.execution_id)
    if proj is None or proj["dispatch_state"] in ("eligible", "released_unbound"):
        raise Rejected("RELEASE_WITHOUT_BINDING", "release_status requires a bound execution")
    validate_binding_tuple(proj, payload, event)
    fence = payload.get("stop_fence_id")
    exit_ev = payload.get("process_exit_evidence_id")
    new_state = proj["dispatch_state"]
    if state == "requested":
        new_state = "release_pending"
    elif state == "stop_fenced":
        if not fence:
            raise Rejected("STOP_FENCE_ID_REQUIRED", "post-fence state requires stop_fence_id")
        new_state = "release_pending"
    elif state == "process_exited":
        if not exit_ev:
            raise Rejected("EXIT_EVIDENCE_REQUIRED", "process_exited requires process_exit_evidence_id")
        new_state = "release_pending"
    elif state == "released":
        if not (exit_ev or fence):
            raise Rejected("RELEASE_PROOF_REQUIRED", "released requires exit evidence or stop-fence proof")
        new_state = "released"
    elif state in ("release_uncertain", "cleanup_failed"):
        new_state = "uncertain"
    elif state == "stale_rejected":
        pass  # observation only
    else:
        raise Rejected("UNKNOWN_RELEASE_STATE", state)
    ctx.cur.execute(
        "UPDATE delivery.task_run_sem_execution SET"
        " dispatch_state=%s,"
        " stop_fence_id=COALESCE(%s, stop_fence_id),"
        " process_exit_evidence_id=COALESCE(%s, process_exit_evidence_id),"
        " last_event_id=%s, last_event_sequence=%s,"
        " released_at=CASE WHEN %s='released' THEN now() ELSE released_at END,"
        " updated_at=now()"
        " WHERE task_run_id=%s",
        (new_state, fence, exit_ev, event["event_id"], event["execution_event_sequence"], new_state, ctx.execution_id),
    )


def apply_process_observed(ctx: Ctx, event: dict) -> None:
    payload = event["payload"]
    proj = projection_row_for_update(ctx.cur, ctx.execution_id)
    if proj is None:
        raise Rejected("PROCESS_OBSERVATION_WITHOUT_BINDING", "process observation requires a binding")
    validate_binding_tuple(proj, payload, event)
    observation = payload["observation"]
    if observation == "started":
        ident = payload["process_identity"]
        if proj["launch_id"] is not None and proj["launch_id"] != ident["launch_id"] if "launch_id" in ident else False:
            raise Rejected("PROCESS_IDENTITY_REWRITE", "process identity is immutable within one binding epoch")
        ctx.cur.execute(
            "UPDATE delivery.task_run_sem_execution SET launch_id=%s, containment_id=%s,"
            " process_start_identity=%s, systemd_invocation_id=%s,"
            " last_event_id=%s, last_event_sequence=%s, updated_at=now()"
            " WHERE task_run_id=%s AND launch_id IS NULL",
            (
                ident["launch_id"], ident["containment_id"], ident["process_start_identity"],
                ident["systemd_invocation_id"],
                event["event_id"], event["execution_event_sequence"], ctx.execution_id,
            ),
        )
        if ctx.cur.rowcount != 1:
            raise Rejected("PROCESS_IDENTITY_REWRITE", "identity already recorded for this epoch")
    elif observation in ("exited", "exit_status_unknown"):
        evidence = payload["exit_evidence"]
        # exit facts are recorded in the observation; projection-level evidence
        # columns are set only through release_status (DDL state CHECK).
        ctx.cur.execute(
            "UPDATE delivery.task_run_sem_execution SET"
            " last_event_id=%s, last_event_sequence=%s, updated_at=now()"
            " WHERE task_run_id=%s",
            (event["event_id"], event["execution_event_sequence"], ctx.execution_id),
        )
        _ = evidence
    elif observation == "start_failed":
        ctx.cur.execute(
            "UPDATE delivery.task_run_sem_execution SET"
            " last_event_id=%s, last_event_sequence=%s, updated_at=now()"
            " WHERE task_run_id=%s",
            (event["event_id"], event["execution_event_sequence"], ctx.execution_id),
        )
    else:
        raise Rejected("UNKNOWN_OBSERVATION", observation)


def apply_unbound_release(ctx: Ctx, event: dict) -> None:
    payload = event["payload"]
    if payload.get("disposition") != "released_unbound":
        raise Rejected("BAD_TOMBSTONE_DISPOSITION", "disposition must be released_unbound")
    for banned in ("binding_epoch", "runner_id", "boot_id", "stop_fence_id", "process_exit_evidence_id"):
        if banned in payload:
            raise Rejected("TOMBSTONE_CARRIES_IDENTITY", f"sixth event must not carry {banned}")
    proj = projection_row_for_update(ctx.cur, ctx.execution_id)
    if proj is not None and proj["dispatch_state"] != "eligible":
        # INBOX-52 / UNBOUND-03: any real binding contradicts the tombstone
        raise Rejected("BINDING_ALREADY_EXISTS", "unbound release contradicts existing binding")
    if proj is None:
        ctx.cur.execute(
            "INSERT INTO delivery.task_run_sem_execution"
            " (task_run_id, tenant_id, project_id, dispatch_state, dispatch_fence_id,"
            "  last_event_id, last_event_sequence, released_at, updated_at)"
            " VALUES (%s,%s,%s,'released_unbound',%s,%s,%s,now(),now())",
            (
                ctx.execution_id, TENANT, PROJECT,
                payload["dispatch_fence_id"],
                event["event_id"], event["execution_event_sequence"],
            ),
        )
    else:
        ctx.cur.execute(
            "UPDATE delivery.task_run_sem_execution SET dispatch_state='released_unbound',"
            " dispatch_fence_id=%s, last_event_id=%s, last_event_sequence=%s,"
            " released_at=now(), updated_at=now()"
            " WHERE task_run_id=%s",
            (payload["dispatch_fence_id"], event["event_id"], event["execution_event_sequence"], ctx.execution_id),
        )


EVENT_HANDLERS: dict[str, Callable[[Ctx, dict], None]] = {
    "execution.binding_established": apply_binding_established,
    "execution.infrastructure_uncertain": apply_infrastructure_uncertain,
    "execution.release_status": apply_release_status,
    "harness.process_observed": apply_process_observed,
    "execution.unbound_release_applied": apply_unbound_release,
}


def receive_sem_event(conn, event: dict, *, fail_after: Optional[str] = None) -> str:
    """§13 transaction algorithm with the frozen lock order:
    Run row → execution projection → Inbox → Run observation → cursor CAS → COMMIT.
    Returns 'accepted' or 'duplicate' (the ACK disposition)."""
    body = json.dumps(event, separators=(",", ":"))
    parsed = parse_strict(body)
    event_id = parsed["event_id"]
    execution_id = parsed["execution_id"]
    sequence = parsed["execution_event_sequence"]
    cur = conn.cursor()
    ctx = Ctx(conn, cur, execution_id)

    cur.execute("BEGIN")
    try:
        # 1. lock the Run row; identity derived only from it
        cur.execute(
            "SELECT tenant_id, project_id, task_id, run_kind, lease_generation, lease_client_id,"
            " execution_event_sequence FROM delivery.task_run WHERE id = %s FOR UPDATE",
            (execution_id,),
        )
        run = cur.fetchone()
        if run is None or run[3] != "delivery":
            raise Rejected("NOT_FOUND", "unknown or non-delivery execution")
        tenant_id, project_id, task_id, _kind, lease_generation, lease_client_id, cursor_seq = run

        # 3. global Inbox event_id identity check (before sequence ownership)
        cur.execute(
            "SELECT source, event_type, idempotency_key, execution_id, execution_event_sequence,"
            " payload_hash, event_envelope_hash, tenant_id, project_id"
            " FROM integration.inbound_event WHERE id = %s",
            (event_id,),
        )
        existing = cur.fetchone()
        payload_hash = sha256_hex(parsed["payload"])
        envelope_hash = sha256_hex(parsed)
        if existing is not None:
            (e_source, e_type, e_idem, e_exec, e_seq, e_ph, e_eh, e_t, e_p) = existing
            if e_source != "sem" or e_exec != execution_id:
                raise Rejected("IDEMPOTENCY_CONFLICT", "event id owned by another execution/source")
            if (e_t, e_p) != (tenant_id, project_id):
                raise IntegrityFailure("stored scope mismatch")
            if (
                e_type == parsed["event_type"]
                and e_idem == event_id
                and e_seq == sequence
                and e_ph == payload_hash
                and e_eh == envelope_hash
            ):
                # exact duplicate: verify sequence owner resolves to this row
                cur.execute(
                    "SELECT id FROM integration.inbound_event"
                    " WHERE execution_id=%s AND execution_event_sequence=%s AND source='sem'",
                    (execution_id, sequence),
                )
                owner = cur.fetchone()
                if owner is None or owner[0] != event_id:
                    raise IntegrityFailure("duplicate sequence resolves to another row")
                if cursor_seq < sequence:
                    raise IntegrityFailure("cursor below committed sequence")
                conn.rollback()
                return "duplicate"
            raise Rejected("IDEMPOTENCY_CONFLICT", "same event id with changed canonical identity")

        # 4. sequence ownership + contiguity
        cur.execute(
            "SELECT id FROM integration.inbound_event"
            " WHERE execution_id=%s AND execution_event_sequence=%s AND source='sem'",
            (execution_id, sequence),
        )
        if cur.fetchone() is not None:
            raise Rejected("EVENT_SEQUENCE_CONFLICT", "sequence owned by another event id")
        if sequence != cursor_seq + 1:
            raise Rejected(
                "EVENT_SEQUENCE_GAP" if sequence > cursor_seq + 1 else "EVENT_SEQUENCE_CONFLICT",
                f"expected_sequence={cursor_seq + 1}",
            )

        # 2/5. projection lock/mutation (frozen lock order: after Run, before Inbox insert)
        handler = EVENT_HANDLERS.get(parsed["event_type"])
        if handler is None:
            raise Rejected("UNSUPPORTED_EVENT_TYPE", parsed["event_type"])
        runner_obj = parsed["payload"].get("runner") if isinstance(parsed["payload"].get("runner"), dict) else {}
        detail = {
            "event_id": event_id,
            "execution_event_sequence": sequence,
            "sem_event_type": parsed["event_type"],
            "schema_version": parsed["schema_version"],
            "occurred_at": parsed["occurred_at"],
            "event_envelope_hash": envelope_hash,
        }
        # binding_epoch is recorded only for the five binding-scoped events;
        # the unbound-release tombstone never carries one (2026-11-28 harness
        # finding: a fabricated 0 or a required key breaks the tombstone contract).
        binding_epoch = parsed["payload"].get("binding_epoch") or runner_obj.get("binding_epoch")
        if binding_epoch is not None:
            detail["binding_epoch"] = binding_epoch
        for key, source in (
            ("runner_id", runner_obj.get("runner_id", parsed["payload"].get("runner_id"))),
            ("runner_key_version", runner_obj.get("runner_key_version", parsed["payload"].get("runner_key_version"))),
            ("boot_id", runner_obj.get("boot_id", parsed["payload"].get("boot_id"))),
            ("supervisor_instance_id", runner_obj.get("supervisor_instance_id", parsed["payload"].get("supervisor_instance_id"))),
        ):
            if source is not None:
                detail[key] = source
        handler(ctx, parsed)

        # 5. Inbox insert
        cur.execute(
            "INSERT INTO integration.inbound_event"
            " (id, tenant_id, project_id, actor_id, source, event_type, idempotency_key,"
            "  payload, payload_hash, execution_id, execution_event_sequence, event_envelope_hash)"
            " VALUES (%s,%s,%s,%s,'sem',%s,%s,%s,%s,%s,%s,%s)"
            " ON CONFLICT DO NOTHING RETURNING id",
            (
                event_id, tenant_id, project_id, SEM_PRINCIPAL, parsed["event_type"],
                event_id, json.dumps(parsed["payload"]), payload_hash,
                execution_id, sequence, envelope_hash,
            ),
        )
        if cur.fetchone() is None:
            raise IntegrityFailure("inbox insert lost the race without a visible winner")
        if fail_after == "inbox":
            raise IntegrityFailure("injected failure after Inbox insert")

        # 6. Run observation
        cur.execute(
            "INSERT INTO delivery.task_run_event"
            " (tenant_id, project_id, task_id, run_id, client_id, event_type,"
            "  lease_generation, dedupe_tag, occurred_at, detail)"
            " VALUES (%s,%s,%s,%s,%s,'sem_infrastructure_observed',%s,%s,%s,%s)",
            (
                tenant_id, project_id, task_id, execution_id, lease_client_id,
                lease_generation, str(sequence), parsed["occurred_at"], json.dumps(detail),
            ),
        )
        if cur.rowcount != 1:
            raise IntegrityFailure("observation insert failed")
        if fail_after == "observation":
            raise IntegrityFailure("injected failure after Run observation")

        # 7. cursor CAS
        cur.execute(
            "UPDATE delivery.task_run SET execution_event_sequence=%s"
            " WHERE id=%s AND tenant_id=%s AND project_id=%s AND execution_event_sequence=%s"
            " RETURNING execution_event_sequence",
            (sequence, execution_id, tenant_id, project_id, cursor_seq),
        )
        if cur.fetchone() is None:
            raise IntegrityFailure("cursor CAS lost")
        if fail_after == "cursor":
            raise IntegrityFailure("injected failure before commit")

        conn.commit()
        return "accepted"
    except Rejected as rejected:
        conn.rollback()
        raise rejected
    except IntegrityFailure:
        conn.rollback()
        raise
    except Exception:
        conn.rollback()
        raise
    finally:
        cur.close()


def count_rows(cur, execution_id: str) -> tuple[int, int, int, Optional[str]]:
    cur.execute("SELECT count(*) FROM integration.inbound_event WHERE execution_id=%s", (execution_id,))
    inbox = cur.fetchone()[0]
    cur.execute(
        "SELECT count(*) FROM delivery.task_run_event WHERE run_id=%s AND event_type='sem_infrastructure_observed'",
        (execution_id,),
    )
    obs = cur.fetchone()[0]
    cur.execute("SELECT execution_event_sequence FROM delivery.task_run WHERE id=%s", (execution_id,))
    cursor = cur.fetchone()[0]
    proj = fetch_projection(cur, execution_id)
    return inbox, obs, cursor, (proj or {}).get("dispatch_state")


class Harness:
    def __init__(self, admin_dsn: str):
        self.admin_dsn = admin_dsn
        self.dbname = f"sem_inbox_acceptance_{int(time.time())}"
        self.dsn = None
        self.passed: list[str] = []
        self.failed: list[tuple[str, str]] = []

    def setup(self):
        admin = psycopg2.connect(self.admin_dsn)
        admin.autocommit = True
        cur = admin.cursor()
        cur.execute(f'DROP DATABASE IF EXISTS "{self.dbname}"')
        cur.execute(f'CREATE DATABASE "{self.dbname}"')
        cur.close()
        admin.close()
        parts = self.admin_dsn.rsplit("/", 1)
        self.dsn = parts[0] + "/" + self.dbname
        conn = psycopg2.connect(self.dsn)
        conn.autocommit = True
        cur = conn.cursor()
        schema_sql = (
            __import__("pathlib").Path(__file__).parent / "fixture_schema.sql"
        ).read_text(encoding="utf-8")
        cur.execute(schema_sql)
        cur.execute("INSERT INTO core.principal (id, kind) VALUES (%s, 'service')", (SEM_PRINCIPAL,))
        cur.close()
        conn.close()

    def teardown(self):
        admin = psycopg2.connect(self.admin_dsn)
        admin.autocommit = True
        cur = admin.cursor()
        cur.execute(f'DROP DATABASE IF EXISTS "{self.dbname}" WITH (FORCE)')
        cur.close()
        admin.close()

    def check(self, name: str, fn: Callable[[], None]):
        try:
            fn()
            self.passed.append(name)
            print(f"  PASS {name}")
        except Exception as exc:  # noqa: BLE001 - harness records everything
            self.failed.append((name, f"{type(exc).__name__}: {exc}"))
            print(f"  FAIL {name}: {type(exc).__name__}: {exc}")

    def new_run(self, conn, run_id: str, *, kind="delivery", cursor=0):
        """Create/overwrite the attempt row in its own committed transaction so
        the harness never leaks locks into the scenario transactions."""
        with conn.cursor() as cur:
            cur.execute(
                "INSERT INTO delivery.task_run (id, project_id, tenant_id, task_id, attempt_no, run_kind, execution_event_sequence)"
                " VALUES (%s,%s,%s,%s,1,%s,%s) ON CONFLICT (id) DO UPDATE SET execution_event_sequence=%s",
                (run_id, PROJECT, TENANT, TASK, kind, cursor, cursor),
            )
        conn.commit()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dsn", default="postgres://aipm:aipm@127.0.0.1:16433/postgres")
    args = parser.parse_args()

    harness = Harness(args.dsn)
    harness.setup()
    print(f"ephemeral database: {harness.dbname}")

    conn1 = psycopg2.connect(harness.dsn, options="-c lock_timeout=15000")
    conn2 = psycopg2.connect(harness.dsn, options="-c lock_timeout=15000")
    audit = psycopg2.connect(harness.dsn)
    audit.autocommit = True
    acur = audit.cursor()

    try:
        run_scenarios(harness, conn1, conn2, acur)
    finally:
        for c in (conn1, conn2, audit):
            c.close()
        summary_pass = len(harness.passed)
        summary_fail = len(harness.failed)
        harness.teardown()
        print(f"\nRESULT: {summary_pass} passed, {summary_fail} failed (total {summary_pass + summary_fail})")
        if harness.failed:
            for name, detail in harness.failed:
                print(f"  FAILED: {name}: {detail}")
    return 1 if harness.failed else 0


def run_scenarios(h: Harness, conn1, conn2, acur) -> None:
    print("\n== Group 1: baseline Inbox transaction (INBOX-01..09) ==")

    def inbox01():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        assert receive_sem_event(conn1, ev) == "accepted"
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1), f"{inbox},{obs},{cursor}"
        assert state == "bound"
    h.check("H-01 INBOX-01 first event commits inbox+observation+cursor atomically", inbox01)

    def inbox02():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        assert receive_sem_event(conn1, ev) == "accepted"
        assert receive_sem_event(conn1, ev) == "duplicate"
        inbox, obs, cursor, _ = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1)
    h.check("H-02 INBOX-02 exact replay is duplicate without second observation", inbox02)

    def inbox03():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        receive_sem_event(conn1, ev)
        changed = json.loads(json.dumps(ev))
        changed["payload"]["established_at"] = "2026-11-28T13:00:00Z"
        try:
            receive_sem_event(conn1, changed)
            raise AssertionError("changed payload must conflict")
        except Rejected as r:
            assert r.code == "IDEMPOTENCY_CONFLICT", r.code
        inbox, obs, cursor, _ = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1)
    h.check("H-03 INBOX-03 same event id changed body is IDEMPOTENCY_CONFLICT", inbox03)

    def inbox05():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        e1 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        e2 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        assert receive_sem_event(conn1, e1) == "accepted"
        try:
            receive_sem_event(conn2, e2)
            raise AssertionError("same sequence different event id must conflict")
        except Rejected as r:
            assert r.code == "EVENT_SEQUENCE_CONFLICT", r.code
        inbox, obs, cursor, _ = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1)
    h.check("H-04 INBOX-05 same sequence different event ids: one commits, loser conflicts", inbox05)

    def inbox05_backstop():
        """Unique-index backstop: two raw concurrent-shaped inserts bypass the
        application checks entirely; the partial unique index must keep
        exactly one `(execution_id, sequence)` owner. The second insert waits
        on the first's commit exactly as it would under two connections."""
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        e1 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        e2 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        c1, c2 = conn1.cursor(), conn2.cursor()

        def raw_insert(cur, ev):
            cur.execute("BEGIN")
            cur.execute(
                "INSERT INTO integration.inbound_event"
                " (id, tenant_id, project_id, actor_id, source, event_type, idempotency_key,"
                "  payload, payload_hash, execution_id, execution_event_sequence, event_envelope_hash)"
                " VALUES (%s,%s,%s,%s,'sem',%s,%s,%s,%s,%s,%s,%s) ON CONFLICT DO NOTHING RETURNING id",
                (
                    ev["event_id"], TENANT, PROJECT, SEM_PRINCIPAL, ev["event_type"], ev["event_id"],
                    json.dumps(ev["payload"]), sha256_hex(ev["payload"]),
                    run_id, 1, sha256_hex(ev),
                ),
            )
            return cur.fetchone()

        try:
            w1 = raw_insert(c1, e1)
            conn1.commit()
            w2 = raw_insert(c2, e2)  # serialized behind the unique index
            conn2.commit()
            assert w1 is not None and w2 is None, f"index must admit one owner, got {w1}/{w2}"
        finally:
            c1.close()
            c2.close()
            conn1.rollback()
            conn2.rollback()
        cur = conn1.cursor()
        cur.execute("SELECT count(*) FROM integration.inbound_event WHERE execution_id=%s", (run_id,))
        assert cur.fetchone()[0] == 1, "exactly one inbox row must survive"
        cur.close()
    h.check("H-05 INBOX-05 backstop: partial unique index admits exactly one sequence owner", inbox05_backstop)

    def inbox07():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 2, binding_payload(1))
        try:
            receive_sem_event(conn1, ev)
            raise AssertionError("gap must be rejected")
        except Rejected as r:
            assert r.code == "EVENT_SEQUENCE_GAP", r.code
        inbox, obs, cursor, _ = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (0, 0, 0)
    h.check("H-06 INBOX-07 sequence gap rejected without writes", inbox07)

    def inbox08():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        receive_sem_event(conn1, ev)  # commit; response lost
        assert receive_sem_event(conn1, ev) == "duplicate"
        inbox, obs, cursor, _ = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1)
    h.check("H-07 INBOX-08 commit-before-ACK replay returns duplicate, cursor stable", inbox08)

    def inbox09():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        try:
            receive_sem_event(conn1, ev, fail_after="inbox")
            raise AssertionError("injected failure must abort")
        except IntegrityFailure:
            pass
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor, state) == (0, 0, 0, None)
    h.check("H-08 INBOX-09 crash after Inbox insert rolls back everything", inbox09)

    def inbox20_21():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        for point in ("observation", "cursor"):
            try:
                receive_sem_event(conn1, json.loads(json.dumps(ev)), fail_after=point)
                raise AssertionError(f"injected failure at {point} must abort")
            except IntegrityFailure:
                pass
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor, state) == (0, 0, 0, None)
    h.check("H-09 INBOX-20/21 failure after observation/cursor rolls back all writes", inbox20_21)

    print("\n== Group 2: ordered concurrency (INBOX-19) ==")

    results: dict[str, str] = {}

    def worker(conn, ev, barrier, *, retries=False):
        barrier.wait()
        key = ev["event_id"]
        for attempt in range(3):
            try:
                results[key] = receive_sem_event(conn, ev)
                return
            except Rejected as r:
                results[key] = r.code
                if retries and r.code == "EVENT_SEQUENCE_GAP" and attempt < 2:
                    time.sleep(0.5)
                    continue
                return
            except Exception as exc:  # record everything, never hang silently
                results[key] = f"{type(exc).__name__}: {exc}"
                return

    def inbox19():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        results.clear()
        ev1 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        ev2 = envelope("execution.infrastructure_uncertain", run_id, 2, {
            "binding_epoch": 1, "uncertainty_code": "wss_disconnected", "uncertainty_scope": "delivery",
            "runner_id": RUNNER, "runner_key_version": 1, "boot_id": BOOT,
            "supervisor_instance_id": SUPERVISOR, "since": "2026-11-28T12:00:00Z",
            "reconciliation_required": True, "reservation_retained": True,
        })
        barrier = threading.Barrier(2)
        t1 = threading.Thread(target=worker, args=(conn1, ev1, barrier), kwargs={"retries": True})
        t2 = threading.Thread(target=worker, args=(conn2, ev2, barrier), kwargs={"retries": True})
        t1.start(); t2.start(); t1.join(30); t2.join(30)
        assert not t1.is_alive() and not t2.is_alive(), "concurrent workers hung"
        assert results[ev1["event_id"]] == "accepted", results
        assert results[ev2["event_id"]] == "accepted", results
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (2, 2, 2), f"{inbox},{obs},{cursor}"
        assert state == "uncertain"
        acur.execute(
            "SELECT dedupe_tag FROM delivery.task_run_event WHERE run_id=%s"
            " AND event_type='sem_infrastructure_observed' ORDER BY occurred_at, dedupe_tag",
            (run_id,),
        )
        assert [r[0] for r in acur.fetchall()] == ["1", "2"]
    h.check("H-10 INBOX-19 concurrent N and N+1 serialize to ordered cursor N+1", inbox19)

    print("\n== Group 3: binding projection rules (BIND-*) ==")

    def bind03():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        same_epoch_other_tuple = envelope(
            "execution.infrastructure_uncertain", run_id, 2,
            {
                "binding_epoch": 1, "uncertainty_code": "heartbeat_stale", "uncertainty_scope": "delivery",
                "runner_id": "66666666-6666-4666-8666-666666666666", "runner_key_version": 1,
                "boot_id": BOOT, "supervisor_instance_id": SUPERVISOR,
                "since": "2026-11-28T12:00:00Z", "reconciliation_required": True,
                "reservation_retained": True,
            },
        )
        try:
            receive_sem_event(conn1, same_epoch_other_tuple)
            raise AssertionError("same epoch different tuple must reject")
        except Rejected as r:
            assert r.code == "BINDING_IDENTITY_MISMATCH", r.code
        _, obs, cursor, state = count_rows(acur, run_id)
        assert (obs, cursor, state) == (1, 1, "bound")
    h.check("H-11 BIND-03 same epoch changed runner tuple rejects (identity mismatch)", bind03)

    def bind04():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        # release with exit evidence -> released
        receive_sem_event(conn1, envelope("execution.release_status", run_id, 2, {
            "release_id": str(uuid.uuid4()), "binding_epoch": 1, "runner_id": RUNNER,
            "runner_key_version": 1, "boot_id": BOOT, "supervisor_instance_id": SUPERVISOR,
            "release_state": "requested", "observed_at": "2026-11-28T12:00:01Z",
        }))
        receive_sem_event(conn1, envelope("execution.release_status", run_id, 3, {
            "release_id": str(uuid.uuid4()), "binding_epoch": 1, "runner_id": RUNNER,
            "runner_key_version": 1, "boot_id": BOOT, "supervisor_instance_id": SUPERVISOR,
            "release_state": "released", "process_exit_evidence_id": str(uuid.uuid4()),
            "observed_at": "2026-11-28T12:00:02Z",
        }))
        # stale epoch-1 binding replay under a new event id
        late = envelope("execution.binding_established", run_id, 4, binding_payload(1))
        try:
            receive_sem_event(conn1, late)
            raise AssertionError("late epoch-1 binding must reject")
        except Rejected as r:
            assert r.code == "DUPLICATE_SEQUENCE_OWNED", r.code
        _, obs, cursor, state = count_rows(acur, run_id)
        assert state == "released"
        assert cursor == 3
    h.check("H-12 BIND-04/BIND-11 lower/stale epoch event after release rejects without regression", bind04)

    def bind05():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        receive_sem_event(conn1, envelope("execution.release_status", run_id, 2, {
            "release_id": str(uuid.uuid4()), "binding_epoch": 1, "runner_id": RUNNER,
            "runner_key_version": 1, "boot_id": BOOT, "supervisor_instance_id": SUPERVISOR,
            "release_state": "released", "process_exit_evidence_id": str(uuid.uuid4()),
            "observed_at": "2026-11-28T12:00:02Z",
        }))
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 3, binding_payload(2)))
        inbox, obs, cursor, state = count_rows(acur, run_id)
        proj = fetch_projection(acur, run_id)
        assert (inbox, obs, cursor) == (3, 3, 3)
        assert state == "bound" and proj["binding_epoch"] == 2
        assert proj["process_exit_evidence_id"] is None  # rebind clears old evidence
    h.check("H-13 BIND-05 higher epoch after positive release proof atomically replaces tuple", bind05)

    def bind07():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        receive_sem_event(conn1, envelope("execution.infrastructure_uncertain", run_id, 2, {
            "binding_epoch": 1, "uncertainty_code": "host_unreachable", "uncertainty_scope": "runner_host",
            "runner_id": RUNNER, "runner_key_version": 1, "boot_id": BOOT,
            "supervisor_instance_id": SUPERVISOR, "since": "2026-11-28T12:00:00Z",
            "reconciliation_required": True, "reservation_retained": True,
        }))
        rebind = envelope("execution.binding_established", run_id, 3, binding_payload(2))
        try:
            receive_sem_event(conn1, rebind)
            raise AssertionError("rebind while uncertain must reject")
        except Rejected as r:
            assert r.code == "PRIOR_BINDING_UNCERTAIN", r.code
        _, obs, cursor, state = count_rows(acur, run_id)
        assert (obs, cursor, state) == (2, 2, "uncertain")
    h.check("H-14 BIND-07 higher epoch while prior binding uncertain rejects and retains quarantine", bind07)

    def bind08():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        results.clear()
        e1 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        e2 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        barrier = threading.Barrier(2)
        t1 = threading.Thread(target=worker, args=(conn1, e1, barrier))
        t2 = threading.Thread(target=worker, args=(conn2, e2, barrier))
        t1.start(); t2.start(); t1.join(30); t2.join(30)
        assert not t1.is_alive() and not t2.is_alive(), "concurrent workers hung"
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1), f"{inbox},{obs},{cursor}"
        assert state == "bound"
        assert sorted(results.values()) == ["EVENT_SEQUENCE_CONFLICT", "accepted"], results
    h.check("H-15 BIND-08 concurrent first-binding serializes; exactly one wins", bind08)

    def bind13():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        results.clear()
        e1 = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        e2 = envelope("execution.binding_established", run_id, 2, binding_payload(2))
        barrier = threading.Barrier(2)
        t1 = threading.Thread(target=worker, args=(conn1, e1, barrier))
        # epoch2 carries sequence 2: a GAP against an uncommitted epoch1 is the
        # correct first outcome; the at-least-once retry then surfaces the
        # semantic rejection (no positive release/isolation proof for epoch 2).
        t2 = threading.Thread(target=worker, args=(conn2, e2, barrier), kwargs={"retries": True})
        t1.start(); t2.start(); t1.join(30); t2.join(30)
        assert not t1.is_alive() and not t2.is_alive(), "concurrent workers hung"
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1), f"{inbox},{obs},{cursor}"
        assert state == "bound" and fetch_projection(acur, run_id)["binding_epoch"] == 1
        assert results[e1["event_id"]] == "accepted", results
        assert results[e2["event_id"]] == "REBIND_NOT_AUTHORIZED", results
    h.check("H-16 BIND-13 two epochs in flight: epoch1 wins, epoch2 has no release proof", bind13)

    def bind09():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        try:
            receive_sem_event(conn1, ev, fail_after="cursor")
            raise AssertionError("pre-commit failure must abort")
        except IntegrityFailure:
            pass
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor, state) == (0, 0, 0, None)
        assert receive_sem_event(conn1, ev) == "accepted"  # exact replay succeeds
    h.check("H-17 BIND-09 mid-transaction failure rolls back all four writes; replay succeeds", bind09)

    def bind06():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        disposition = str(uuid.uuid4())
        with conn1.cursor() as cur:
            cur.execute("BEGIN")
            cur.execute("SELECT task_run_id FROM delivery.task_run_sem_execution WHERE task_run_id=%s FOR UPDATE", (run_id,))
            cur.execute(
                "UPDATE delivery.task_run_sem_execution SET dispatch_state='host_isolated',"
                " host_isolation_disposition_id=%s, last_event_sequence=last_event_sequence,"
                " updated_at=now() WHERE task_run_id=%s",
                (disposition, run_id),
            )
            conn1.commit()
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 2, binding_payload(2)))
        proj = fetch_projection(acur, run_id)
        assert proj["dispatch_state"] == "bound" and proj["binding_epoch"] == 2
        assert proj["host_isolation_disposition_id"] is None  # cleared by rebind
    h.check("H-18 BIND-06 rebind after explicit host-isolation disposition reference", bind06)

    def bind12():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        stale_key = envelope(
            "execution.infrastructure_uncertain", run_id, 2,
            {
                "binding_epoch": 1, "uncertainty_code": "delivery_ack_unknown", "uncertainty_scope": "delivery",
                "runner_id": RUNNER, "runner_key_version": 99, "boot_id": BOOT,
                "supervisor_instance_id": SUPERVISOR, "since": "2026-11-28T12:00:00Z",
                "reconciliation_required": True, "reservation_retained": True,
            },
        )
        try:
            receive_sem_event(conn1, stale_key)
            raise AssertionError("stale runner key version must reject")
        except Rejected as r:
            assert r.code == "BINDING_IDENTITY_MISMATCH", r.code
        _, obs, cursor, _ = count_rows(acur, run_id)
        assert (obs, cursor) == (1, 1)
    h.check("H-19 BIND-12 stale runner_key_version observation rejects, cursor unchanged", bind12)

    print("\n== Group 4: unbound release / sixth event (INBOX-50..54, UNBOUND-*) ==")

    def unbound01():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        assert receive_sem_event(conn1, ev) == "accepted"
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1)
        assert state == "released_unbound"
        proj = fetch_projection(acur, run_id)
        assert proj["dispatch_fence_id"] is not None and proj["binding_epoch"] is None and proj["runner_id"] is None
    h.check("H-20 INBOX-50 first sixth event creates released_unbound tombstone", unbound01)

    def unbound02():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        receive_sem_event(conn1, ev)
        before = fetch_projection(acur, run_id)
        assert receive_sem_event(conn1, ev) == "duplicate"
        after = fetch_projection(acur, run_id)
        assert before["updated_at"] == after["updated_at"] and before["last_event_id"] == after["last_event_id"]
        inbox, obs, cursor, _ = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1)
    h.check("H-21 INBOX-51 duplicate sixth event ACKs without rewriting projection", unbound02)

    def unbound03():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.binding_established", run_id, 1, binding_payload(1)))
        ev = envelope("execution.unbound_release_applied", run_id, 2, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        try:
            receive_sem_event(conn1, ev)
            raise AssertionError("sixth event after binding must reject")
        except Rejected as r:
            assert r.code == "BINDING_ALREADY_EXISTS", r.code
        _, obs, cursor, state = count_rows(acur, run_id)
        assert (obs, cursor, state) == (1, 1, "bound")
    h.check("H-22 INBOX-52/UNBOUND-03 sixth event after binding rejects", unbound03)

    def unbound04():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        receive_sem_event(conn1, envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4()))))
        late_binding = envelope("execution.binding_established", run_id, 2, binding_payload(7))
        try:
            receive_sem_event(conn1, late_binding)
            raise AssertionError("binding after tombstone must reject")
        except Rejected as r:
            assert r.code == "EXECUTION_RELEASED_UNBOUND", r.code
        _, obs, cursor, state = count_rows(acur, run_id)
        assert (obs, cursor, state) == (1, 1, "released_unbound")
    h.check("H-23 INBOX-53/UNBOUND-04 binding after released_unbound tombstone rejects regardless of epoch", unbound04)

    def unbound05():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        ev = envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        try:
            receive_sem_event(conn1, ev, fail_after="observation")
            raise AssertionError("mid-transaction failure must abort")
        except IntegrityFailure:
            pass
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor, state) == (0, 0, 0, None)
        assert receive_sem_event(conn1, ev) == "accepted"
    h.check("H-24 UNBOUND-10 sixth-event mid-transaction failure rolls back all four writes; replay idempotent", unbound05)

    def inbox54():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        results.clear()
        binding = envelope("execution.binding_established", run_id, 1, binding_payload(1))
        tombstone = envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        barrier = threading.Barrier(2)

        def tagged_worker(conn, ev, tag):
            barrier.wait()
            try:
                results[tag] = receive_sem_event(conn, ev)
            except Rejected as r:
                results[tag] = r.code
            except Exception as exc:
                results[tag] = f"{type(exc).__name__}: {exc}"

        t1 = threading.Thread(target=tagged_worker, args=(conn1, binding, "binding"))
        t2 = threading.Thread(target=tagged_worker, args=(conn2, tombstone, "tombstone"))
        t1.start(); t2.start(); t1.join(30); t2.join(30)
        assert not t1.is_alive() and not t2.is_alive(), "concurrent workers hung"
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor) == (1, 1, 1), f"{inbox},{obs},{cursor}"
        dispositions = sorted(results.values())
        assert dispositions in (["BINDING_ALREADY_EXISTS", "accepted"], ["EVENT_SEQUENCE_CONFLICT", "accepted"]), results
        assert state in ("bound", "released_unbound")
    h.check("H-25 INBOX-54/UNBOUND-09 release-vs-binding race yields exactly one terminal state", inbox54)

    def unbound08():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        leaked = envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        leaked["payload"]["binding_epoch"] = 3  # D04-F05: tombstone must not carry identity
        try:
            receive_sem_event(conn1, leaked)
            raise AssertionError("tombstone carrying binding_epoch must reject")
        except Rejected as r:
            assert r.code == "TOMBSTONE_CARRIES_IDENTITY", r.code
        # DDL backstop: direct released_unbound insert without fence violates tuple CHECK
        cur = conn1.cursor()
        try:
            cur.execute("BEGIN")
            cur.execute(
                "INSERT INTO delivery.task_run_sem_execution"
                " (task_run_id, tenant_id, project_id, dispatch_state, last_event_id, last_event_sequence, released_at)"
                " VALUES (%s,%s,%s,'released_unbound',%s,1,now())",
                (run_id, TENANT, PROJECT, str(uuid.uuid4())),
            )
            raise AssertionError("tuple CHECK must reject fenceless released_unbound")
        except pg_errors.CheckViolation:
            conn1.rollback()
        finally:
            cur.close()
    h.check("H-26 INBOX-56/UNBOUND-08 tombstone shape enforced by app check + DDL tuple CHECK", unbound08)

    def inbox57():
        run_id = str(uuid.uuid4())
        h.new_run(conn1, run_id)
        bad = envelope("execution.unbound_release_applied", run_id, 1, unbound_payload(str(uuid.uuid4()), str(uuid.uuid4())))
        bad["payload"]["disposition"] = "released"
        try:
            receive_sem_event(conn1, bad)
            raise AssertionError("wrong disposition must reject")
        except Rejected as r:
            assert r.code == "BAD_TOMBSTONE_DISPOSITION", r.code
        inbox, obs, cursor, state = count_rows(acur, run_id)
        assert (inbox, obs, cursor, state) == (0, 0, 0, None)
    h.check("H-27 INBOX-57 sixth event with non-tombstone disposition rejects before transaction writes", inbox57)

    print("\n== Group 5: cross-run/global identity (INBOX-10/25/26) + parse strictness ==")

    def inbox25():
        run_a, run_b = str(uuid.uuid4()), str(uuid.uuid4())
        h.new_run(conn1, run_a)
        h.new_run(conn1, run_b)
        shared_id = str(uuid.uuid4())
        ev_a = envelope("execution.binding_established", run_a, 1, binding_payload(1), event_id=shared_id)
        ev_b = envelope("execution.binding_established", run_b, 1, binding_payload(1), event_id=shared_id)
        assert receive_sem_event(conn1, ev_a) == "accepted"
        try:
            receive_sem_event(conn1, ev_b)
            raise AssertionError("cross-run event id reuse must conflict")
        except Rejected as r:
            assert r.code == "IDEMPOTENCY_CONFLICT", r.code
        inbox_a, _, cursor_a, _ = count_rows(acur, run_a)
        inbox_b, _, cursor_b, _ = count_rows(acur, run_b)
        assert (inbox_a, cursor_a) == (1, 1) and (inbox_b, cursor_b) == (0, 0)
    h.check("H-28 INBOX-25/26 same global event id from another run conflicts without cross-run writes", inbox25)

    def dup_members():
        bad_body = '{"event_id": "%s", "event_id": "%s"}' % (str(uuid.uuid4()), str(uuid.uuid4()))
        try:
            parse_strict(bad_body)
            raise AssertionError("duplicate members must be rejected")
        except ValueError:
            pass
    h.check("H-29 INBOX-24/34 duplicate JSON members rejected before persistence", dup_members)

    def nonsem_check():
        cur = conn1.cursor()
        try:
            cur.execute("BEGIN")
            cur.execute(
                "INSERT INTO integration.inbound_event"
                " (id, tenant_id, project_id, actor_id, source, event_type, idempotency_key,"
                "  payload, payload_hash, execution_id, execution_event_sequence, event_envelope_hash)"
                " VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)",
                (
                    str(uuid.uuid4()), TENANT, PROJECT, SEM_PRINCIPAL,
                    "generic", "generic.event", "generic-key-1",
                    "{}", sha256_hex({}),  # SEM-only columns must stay NULL for non-SEM
                    str(uuid.uuid4()), 1, sha256_hex({}),
                ),
            )
            raise AssertionError("non-SEM row with SEM columns must violate the source CHECK")
        except pg_errors.CheckViolation:
            conn1.rollback()
        finally:
            cur.close()
    h.check("H-30 INBOX-36..38 source CHECK truth table: non-SEM row with SEM fields rejects", nonsem_check)


if __name__ == "__main__":
    sys.exit(main())
