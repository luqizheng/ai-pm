# SEM V1 Testing and Acceptance

A mock passing does not prove OS process semantics. Process and Runner ACK tests require real process/Runner integration.

## 1. Unit tests

- E1/E2/E3/E4/E5 classification and allowed-action matrix.
- Run lineage and monotonic Attempt numbering.
- HIR transition guards and authorization.
- Scheduling Shield predicate shared by every claim/requeue path.
- Generation/token/client identity validation.
- Runner supervision default and mode changes.
- Harness capability default supports_resume=false.
- Secret redaction and workspace path containment.

## 2. Database and concurrency

- Two claimers cannot own the same Run.
- Shielded Run is excluded from every claim candidate query.
- Expired shielded Run is not requeued by B3-2, B06-D3, orphan reconciliation or retry paths.
- Late heartbeat from old generation cannot clear shield or renew lease.
- New claim increments generation and rotates token verifier.
- lease_client_id mismatch is rejected.
- HIR constraints hold under concurrent creation.
- Empty database migration chain initializes all SEM schema.

## 3. OS process tests

- Live child + expired lease: no E1/E2; no reclaim.
- Child exits: E1 observed; guarded recovery allowed.
- Original process absent with validated process identity: E2; guarded recovery allowed.
- PID reused by unrelated process: never mistake unrelated process for original.
- Worker restart with runtime volume intact: reconcile without duplicate spawn.
- Volume loss while process remains alive: E4/E5 shield, no requeue.
- Cleanup refuses active/uncertain process workspace.

## 4. Runner protocol

- Command receipt ACK differs from termination confirmation.
- Runner sends E3 only after supervisor wait/handle observes process exit.
- ACK with stale generation, wrong command ID or process identity is rejected.
- Duplicate ACK is idempotent.
- Lost/delayed ACK remains E4/E5.
- Runner reconnects with stale token and is rejected.
- Default registration is unattended; supervised requires explicit configuration and pre-claim confirmation.

## 5. Harness Resume

For each exact Harness/version: collect native session ID; controlled supervisor restart; resume same native session; verify context continuity; verify workspace/process prerequisites; inject resume failure and confirm Fresh Attempt only after old process termination proof. Until all tests pass, supports_resume=false.

## 6. Fault injection and E2E

- SEM crash/restart at lifecycle boundaries.
- PostgreSQL outage during claim, heartbeat, termination and result acceptance.
- Runner network partition and loss during active process.
- E4 shield survives service restart and lease expiry.
- Authorized manual HIR resolution cannot bypass process safety.
- Two-Runner handoff with delayed/reconnected old Runner; no overlapping execution.
- Ten concurrent Runs with process and DB evidence assertions.
- Git origin unavailable during workspace reconstruction.
- Credential revocation and log-secret scanning.

## 7. Release gates

- Phase 0: no started Run can be reclaimed without E1/E2/E3; credentials meet minimum-scope/revocation/redaction requirements.
- Phase 1: SEM role registers no outbox handlers; Platform role spawns no execution processes.
- Phase 2: real independent-host execution and restart/volume-loss drill pass.
- Phase 3: Resume remains disabled unless exact-version evidence passes.
- Phase 4: HIR authorization/state tests and Runner ACK semantics pass.
- Phase 5: multi-Runner fault matrix green and audit chronology replayable.
