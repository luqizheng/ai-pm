# SEM V1 State Machines

Status: target behavior, not a claim that all states exist in code.

## 1. Run / Attempt

- queued → claimed only through successful claim.
- claimed → running after executor/process identity is confirmed.
- running → terminal result only through guarded transition handling.
- claimed/running with uncertain liveness → durable Scheduling Shield plus evidence/HIR.
- Shielded Run → evidence-backed terminalization/recovery or authorized human resolution.
- Replacement Attempt is a new task_run row with lineage, never a reset of the old row.

Forbidden: lease expiry alone → queued; E4/E5 → automatic new Attempt; late heartbeat → shield removal; stale generation → accepted result.

## 2. Lease

- Unowned → Owned(generation, client, token verifier, expiry) on claim.
- Owned → Renewed only with matching client identity, token proof and generation.
- Owned → Expired when time passes. Expiry is a trigger, not proof of process death.
- Expired → Reclaimed only after E1/E2/E3 and guarded transaction.
- Expired + uncertain process → Shielded; lease may expire naturally while shield remains.
- New claim increments generation and rotates token verifier.
- Never rewrite a lease owner from Runner A to Runner B.

## 3. Termination evidence

| Evidence | Meaning | Permitted action |
|---|---|---|
| E1 | SEM observed process exit result from OS poll/wait | Confirm exact process dead; guarded recovery may proceed |
| E2 | PID/process identity checked against OS and original process absent | Confirm exact process dead; guarded recovery may proceed |
| E3 | Authenticated Runner supervisor observed target process exit and sent generation-bound confirmation | Confirm exact remote process dead; guarded recovery may proceed |
| E4 | Runner heartbeat absent beyond configured grace | Shield + HIR only |
| E5 | Process state unknown or contradictory | Retain shield, alert, no automatic reclaim |

Evidence applies only to the exact process identity and generation it describes.

## 4. Runner

Supervision mode is unattended or supervised. Missing value at first registration defaults to unattended. supervised requires pre-claim user confirmation. Availability (online/offline/busy/unavailable) is independent.

## 5. HIR

Target lifecycle: Created → Notified → AwaitingUserAction or AwaitingRunnerSelection → Accepted → InProgress → Resolved → Closed.

A self-resolved HIR may close only when the original condition is proven safe; record evidence. Reopen must preserve prior history. HIR status is workflow state, not Run state; HIR cannot grant a lease or clear a shield by itself.

## 6. Handoff

1. User authorizes/selects target Runner when policy requires.
2. Old execution is stopped or remains safely isolated.
3. Old lease/token becomes unusable through guarded expiry/revocation and generation change.
4. New Runner explicitly performs normal claim.
5. Generation increments and fresh token is issued.
6. Target Runner creates its own workspace.
7. New execution begins only after successful claim.

No direct lease-owner rewrite and no weakening of lease_client_id checks.
