# Resource-Aware Runner Scheduling — ADR-SEM-26

> **HISTORICAL — SUPERSEDED, NOT AN IMPLEMENTATION CONTRACT (2026-11-25):** This AI-PM-side SEM design copy records the former Platform Claim/Lease/Run/Task/HIR/Barrier control-plane proposal. It is retained for source-audit and migration traceability only. Do not implement its SEM business orchestration, database ownership, state machines or API flows. The current bilateral boundary and active SEM protocol drafts are indexed in [the AI-PM SEM integration review](./README.md) and maintained canonically in [luqizheng/smark-sem](https://github.com/luqizheng/smark-sem).


- Status: Ratified design baseline; SEM-02E consistency alignment pending.
- Resource reports are scheduling signals only.

## Report
Runner Supervisor reports authenticated Runner identity (derived from connection), boot_id, monotonic sample_seq, observed_at, CPU capacity/utilization, memory total/available/pressure, and actual Workspace volume total/free bytes over WSS `runner.resource.report`.

## Admission
SEM stores latest snapshot in runner_connection and resource profile/reservation in dispatch_operation. Reservation is atomic. Unknown/stale/out-of-order resource samples block new dispatch, not active Runs.

## Non-authority
Resource reports do not renew Lease, prove process start/exit, generate E1/E2/E3, release Barrier or establish human presence. RabbitMQ state is likewise not a resource or process fact. RabbitMQ outage does not release reservations.

## Thresholds
Sampling intervals and budgets are calibrated by real Windows/Linux/macOS load tests. No hard-coded universal thresholds without evidence.
