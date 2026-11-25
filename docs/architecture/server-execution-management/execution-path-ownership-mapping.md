# SEM Cutover — Existing Execution Path Ownership Mapping

- Status: **Read-only source mapping / candidate disposition; review required**.
- Source baseline: ai-pm master at d0e72fa6805aebeadf1b282e430deba493072fd6.
- Related decision: [AI-PM ↔ SEM Authority Boundary](./authority-boundary-decision.md).
- No application code, database migration, API or Runner behavior was changed.

## 1. Current source-level architecture

The current repository contains two execution paths:

1. **Runner self-claim path (Model A):** apps/ai-pm-runner directly calls AI-PM MCP to discover/claim work, read Run context, renew the Run lease, report interactions, poll/ACK commands, and submit results. It also launches and supervises the local Harness process.
2. **Platform Worker executor path (Model B):** apps/ai-pm-worker calls pm-application run_executor to claim Runs, start/poll/terminate executors and reconcile process state. pm-infrastructure CodexExecutor currently owns actual OS process and workspace operations on the Worker host.

These are historical implementations. They do not match the new single direction: AI-PM sends an opaque execution request to SEM; SEM routes it to Runner; Runner launches/attaches Harness; Harness uses AI-PM MCP for business context and state; AI-PM sends SEM a release instruction after its business decision.

## 2. File-level mapping

| Current source | Verified current responsibility | New disposition |
|---|---|---|
| apps/ai-pm-runner/src/runner_loop.rs | Direct MCP discovery/claim, Run-context fetch, local process spawn/reap/terminate, heartbeat/lease renewal, interaction marker reporting, command polling/ACK, result submission | Retain the local Runner/Harness process responsibilities in Runner. Remove direct Runner MCP dispatch/claim/lease/command polling control. Keep only business MCP operations that Harness itself needs; move business interaction/progress/result to the Harness MCP path. |
| apps/ai-pm-runner/src/mcp.rs | MCP client exposes pm_list_tasks, pm_claim_task, pm_get_run_context, pm_report_progress, pm_report_interaction, pm_poll_commands, pm_ack_command, pm_discover_runs, pm_reacquire_lease and pm_report_consumption | Split the client surface. Keep authorized Task/context/business operations for Harness. Retire Runner-only claim/discover/reacquire/command transport tools after cutover. Split progress from lease heartbeat and split business interaction persistence from infrastructure stop delivery. |
| crates/pm-mcp/src/server.rs | Exposes Task context, task discovery, interaction reporting, command polling/ACK and other business MCP tools | Preserve Harness business MCP. Update descriptions and tool contracts so Task/context/progress/result operations do not imply Runner dispatch/claim. Remove or fail-closed legacy Runner control tools only after all clients migrate. |
| crates/pm-mcp/src/tools.rs | MCP tool application layer; get_task_context uses the project-scoped read service; other tools invoke AI-PM business operations | Candidate business context read surface. Audit completeness of get_task_context and any richer project/task context needs; do not use this read operation as a hidden claim. |
| crates/pm-application/src/run_dispatch/claim.rs | Platform transaction creates/claims Run, issues lease token/generation and applies Task/Project eligibility | Business authority remains in AI-PM. The old Runner/Worker Claim handshake is not the SEM dispatch contract. Decide whether AI-PM still needs an internal Run/attempt allocation operation, but SEM must not call or own it. |
| crates/pm-application/src/run_executor.rs | Orchestrates claim_and_start, poll_active_runs, cancel_run, reconcile_orphans and executor-driven Run lifecycle writes | Split Platform business-state/audit decisions from process lifecycle. Platform retains Task/Run business transitions and policy. Local process spawn/poll/terminate/reap moves to Runner. |
| crates/pm-infrastructure/src/codex_executor.rs | Implements RunExecutor with local Git workspace and OS process operations, including spawn, process identity, terminate and reap | Do not move this process adapter into SEM. Its local OS process responsibilities belong on Runner. Extract reusable behavior only after a Runner-side contract and security review. |
| apps/ai-pm-worker/src/main.rs | Runs Platform Outbox and business worker loops, but also starts run_executor orphan reconciliation, claim_and_start, poll_active_runs and process cleanup loops | Keep Platform analysis/business workers and the single Platform Outbox consumer. Retire the local process-execution loop only after Runner/SEM cutover. Do not move the whole Worker to SEM. |
| crates/pm-application/src/worker_command.rs | Stores business command lifecycle and command receipt state for terminate/continue delivery | Split business intent from transport. AI-PM retains user decision, interaction/continuation state and authorization. SEM/Runner handle delivery and local process action facts. Old MCP poll/ACK must not remain a second Runner control channel. |

## 3. MCP tool disposition

| Existing MCP tool | Current meaning | Candidate new disposition |
|---|---|---|
| pm_list_tasks | Read ready Tasks and latest queued Run hint; description tells caller to claim next | Keep only if useful for Harness business discovery; remove the implied claim-next contract |
| pm_claim_task | Runner-facing business Run claim and Lease/Generation grant | Retire from Runner/Harness MCP dispatch path. AI-PM creates/associates its business execution record when sending the execution request |
| pm_get_run_context | Lease-bound Run execution contract/context | Reassess. Replace or supplement with a Task/project context read that is authorized independently of a SEM/Runner Claim |
| pm_get_task_context | Project-scoped Task context read | Candidate to retain and expand for Harness context acquisition; verify requirements, dependencies, acceptance criteria and source links are complete |
| pm_report_progress | Current implementation couples progress/stage report with Lease renewal | Split business progress reporting from Runner/SEM infrastructure liveness. Harness MCP must not be the process heartbeat/lease channel |
| pm_submit_result | Business result/delivery submission | Keep as AI-PM MCP business operation; AI-PM validates and decides resulting Task state |
| pm_report_interaction | Persists business interaction and currently issues terminate command in the same transaction | Keep the AI-PM business interaction/HIR decision. Emit a separate Platform→SEM stop/release instruction through Outbox; do not write Runner transport command state as the business transaction's SEM proxy |
| pm_poll_commands / pm_ack_command | Runner polls and acknowledges terminate/continue commands | Retire as Runner transport. SEM WSS delivers infrastructure control; Harness retrieves business context/replies via AI-PM MCP |
| pm_discover_runs / pm_reacquire_lease | Runner-side dispatch/recovery control | Retire from Runner MCP after cutover; AI-PM and SEM reconcile through their own authority boundaries |
| pm_report_consumption | Continuation/process-consumption acknowledgement | Reclassify: business continuation state stays in AI-PM; Runner process-start/input-injection observations are infrastructure facts reported via SEM |

Tool names above describe the source baseline, not an approved deletion list. Before removal, audit every caller, permission grant, test, CLI and external integration.

## 4. Critical contract splits

1. **Business progress vs infrastructure liveness:** a Harness may report meaningful Task progress through MCP; Runner reports process/connection/resource liveness through SEM. Neither signal substitutes for the other.
2. **Business interaction vs process stop:** AI-PM records the interaction/HIR/approval state and independently emits a stop/release instruction. A failed stop does not roll back the business interaction.
3. **Business result vs process exit:** AI-PM validates result and Task completion; Runner reports exact process exit. Either may arrive first.
4. **Run/attempt vs SEM Execution:** AI-PM may retain a Run/attempt record and maps it to an opaque execution_id. SEM does not mirror Task/Run state and does not use Task ID as its authority key.
5. **Runner vs SEM process ownership:** Runner Supervisor owns local OS process creation, identity, termination and exit evidence. SEM transports commands, tracks delivery/binding/resource reservation and records Runner reports.

## 5. Required read-only follow-up before implementation

- Enumerate every call site and authorization entry for the MCP tools in §3.
- Inspect exact get_task_context and get_run_context payloads to determine the minimum complete Harness business context.
- Trace pm_report_interaction and continuation paths to separate business writes from transport/process side effects.
- Inventory all Worker run_executor and cleanup loop dependencies, including workspace reclamation and audit/outbox side effects.
- Define the AI-PM execution record ↔ SEM execution_id mapping and its retry/attempt semantics.
- Specify Harness MCP identity provisioning and verify cross-project negative authorization.
- Update product/onboarding/runbook docs that currently instruct users to install a Runner that self-claims via Platform MCP.

No code, Migration, API, Runner behavior, or Phase 0 work is authorized by this mapping.
