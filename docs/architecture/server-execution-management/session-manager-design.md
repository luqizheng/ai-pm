# Session Manager Design — SEM-02E

- Status: Review Pending.
- Normative baseline: [SEM-02E](./sem-02e-architecture-convergence.md).

## Placement

Session Manager is a logical SEM runtime module, not a separate microservice and not a Platform DB reader. Detached/Reconciling authority is SEM-owned persistent execution state. SEM cannot project these states by directly querying Platform PostgreSQL; it obtains business Lease/Generation facts through Platform HTTPS API.

## Three distinct continuity mechanisms

1. **Process Reattachment**: same OS process survives Runner/connection interruption; requires same device authentication, boot identity, process creation identity, active OS supervision handle, current Platform Lease/Generation and SEM connection epoch.
2. **Interaction Continuation**: checkpoint + user response injected into a new Harness process/Fresh Session. This creates a new process identity sequence within the same Run+Generation when permitted; it is not Native Resume.
3. **Harness Native Resume**: harness-specific native session restoration. Default false; only enabled after exact CLI/version repeatable test. Native session ID alone, especially across devices, does not restore remote CODEX_HOME.

## Process identity sequence

Run+Generation may contain multiple sequential processes. Every start has process_sequence, boot_id, PID + process creation identity, harness kind and launch reason. Interaction continue must emit continuation.consumed and register the new sequence. Lease renewal validates the current active sequence. Late exit evidence for an older sequence cannot terminate a newer process.

## Storage

SEM stores local process/session observations and reconnect reconciliation state in SEM DB. Platform remains authoritative for Run/Lease/Generation. Native session artifacts remain on the execution host; no remote CODEX_HOME assumption.

## Recovery

Active → Detached → Reconciling → Active/Terminated/Quarantined. Unknown identity or missing evidence is E5 + Task Barrier/HIR. Session Manager does not clear Barrier.
