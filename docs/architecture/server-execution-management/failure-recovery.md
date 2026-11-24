# SEM V1 Failure and Recovery Model

Priority order: prevent duplicate execution; preserve authoritative state/audit; restore service. Availability is subordinate to execution safety.

## 1. Failure matrix

| Failure | Required behavior | Forbidden behavior |
|---|---|---|
| SEM restart, runtime volume intact | Reconcile DB Runs against OS process identity and persisted evidence | Treat lost in-memory child map as proof of death |
| SEM workspace volume lost | Shield active Runs lacking E1/E2; create/associate HIR; rebuild Git cache only after isolation | Infer process death from missing PID/event files |
| SEM cannot reach its own DB | Stop new dispatch/claim attempts that require durable local state; preserve/observe local processes safely | Use volatile in-memory state as durable truth |
| SEM cannot reach Platform API | Stop new claims and authoritative Run mutations; preserve/observe local processes, queue retryable reports locally and reconcile after API recovery | Self-issue Lease or change Run state locally |
| Platform cannot reach SEM API | Keep Outbox events pending/retryable; do not mark dispatch complete until SEM durably accepts the event | Drop dispatch event or assume SEM accepted it |
| Platform PostgreSQL unavailable | Platform rejects/defers claim and authoritative mutations; SEM preserves local execution evidence and retries API operations after recovery | Treat SEM DB/cache as Run/Lease authority |
| Runner offline / heartbeat lost | E4 shield + HIR; await reconnect or human resolution | Automatic re-dispatch |
| Runner returns with stale generation | Reject heartbeat/result/command mutation | Revive old lease |
| Cancel command delivered but no process exit observed | Not E3; remain shielded | Reclaim |
| Harness Resume unsupported/fails | Fresh Attempt only after old process termination proof | Reuse uncertain old workspace/session |
| Git origin unavailable | Pause reconstruction and retry later | Treat empty workspace as successful checkout |
| Disk pressure | Stop new claims before exhaustion; reclaim only with E1/E2/E3 | Delete active/uncertain workspace |

## 2. Recovery ordering

1. Restore PostgreSQL connectivity and confirm authoritative generation/lease state.
2. Confirm SEM host and process supervisor identity.
3. Inventory active Runs from DB.
4. Reconcile each Run with process evidence.
5. Apply E1/E2/E3 guarded recovery; apply E4/E5 shield + HIR.
6. Rebuild Git Mirror/worktree only for a safe future Attempt from the single authoritative origin.
7. Verify no duplicate active executor before reopening claims.

## 3. Volume loss

Workspace, Worktree and Git Mirror are explicitly not backed up. Loss is a safety incident, not a backup restore workflow:
- DB Run/lease/generation/audit state remains authoritative.
- Missing PID file or event log is not process-death proof.
- Active execution without positive termination evidence is shielded and escalated.
- Rebuild Git Mirror from the Repository's sole authoritative origin for future safe work.
- A new Attempt uses a new workspace and starts only after the old process is proven terminated or safely contained by an approved procedure.

## 4. Recovery acceptance

Fault injection must prove every ambiguous failure results in no automatic second executor. Recovery may be delayed indefinitely when termination cannot be proven. This is an intentional safety property.


## 7. Cross-service recovery additions (ADR-SEM-25; authentication/revocation per ADR-SEM-22/23)

- Inbox event is acknowledged only after SEM DB commit. SEM DB outage means no ACK and Platform Outbox retry.
- Duplicate events are deduplicated by `event_id`; duplicate command deliveries by command ID + Run + Generation + target Runner.
- Claim operation is persisted before Platform call and uses a stable `claim_request_id`. If the response is lost, retry the same request and retrieve the same Platform idempotency result; never create a second claim because of timeout.
- If Claim succeeded but WSS delivery did not, keep the same target Runner binding. Do not redirect the lease to another Runner. Recover the same grant while valid or request guarded cancellation/isolation through Platform.
- Outbound evidence/ACKs are persisted in SEM `platform_delivery` and retried with the same idempotency key.
- Device revocation events close active WSS sessions; periodic authorization revalidation bounds missed-event exposure. If Platform cannot revalidate before the configured freshness deadline, stop dispatch and close the connection.
- SEM DB loss or WSS disconnection does not prove process death. Uncertain active Runs remain shielded and enter E4/E5/HIR handling.