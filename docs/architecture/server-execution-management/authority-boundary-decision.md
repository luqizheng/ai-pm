# AI-PM ↔ SEM Authority Boundary Decision

- Status: **User-confirmed architecture direction; cross-document and wire-protocol replacement in progress**.
- Date: 2026-11-25.
- Related canonical SEM document: [SEM Authority Boundary and Execution Message Model](https://github.com/luqizheng/smark-sem/blob/docs/sem-authority-boundary-convergence/docs/architecture/SEM-AUTHORITY-BOUNDARY-AND-EXECUTION-MESSAGE-MODEL.md).
- This decision supersedes SEM-02E–02J and Protocol V1 wherever they assign AI-PM business workflow authority to SEM. Historical audit findings remain evidence, but their proposed control-plane ownership must be re-evaluated.

## 1. Decision

SEM is an independent execution-message transport and runtime-management service. It is not a second business execution control plane.

AI-PM Platform remains the sole business authority for Project, Task, business Run state, Task status, approvals, HIR, Barrier, business commands, business results, retry/successor decisions, and business audit.

SEM receives AI-PM execution messages, routes opaque prompt bodies to Runners, supervises Harness processes and infrastructure resources, and reports infrastructure facts. It must not interpret prompt text, fetch Task context, mutate Task state, judge business success, or mirror business workflow state.

## 2. Canonical responsibility flow

1. AI-PM sends SEM an idempotent execution request containing an opaque execution ID, prompt body and infrastructure routing/profile metadata.
2. SEM selects an eligible Runner using infrastructure availability/capacity and delivers the opaque prompt through the authenticated Runner channel.
3. Runner checks/starts the configured AI Harness and injects the prompt.
4. Harness uses its independently authenticated AI-PM MCP integration to fetch the Task, requirements, dependencies and other business context.
5. Harness performs Task state transitions and submits progress/results directly through AI-PM MCP. AI-PM validates and commits all business changes.
6. AI-PM decides when the Task is complete and sends SEM an explicit release instruction for the corresponding execution ID.
7. SEM stops/cleans up the Harness process, releases infrastructure reservations when safe, and reports the cleanup/process outcome.
8. AI-PM records infrastructure outcome separately; it never treats process exit or successful cleanup as proof of business completion.

## 3. Two independent lifecycles

The AI-PM Task lifecycle and SEM Execution lifecycle must not be merged.

- Task completed + SEM releasing: business work is complete; runtime cleanup remains.
- Task non-terminal + SEM failed: process failed; AI-PM decides retry, intervention or other business handling.
- Process exit code zero: infrastructure fact only, not Task completion.
- Runner disconnected: infrastructure uncertainty, not proof of process death or Task failure.

SEM-local statuses must describe only delivery, assignment, process and resource lifecycle. They must not reuse Task status names as if they were business authority.

## 4. Harness MCP security boundary

Installing AI-PM MCP in the Harness is a required integration prerequisite, not an authorization grant by itself.

- Harness authenticates independently to AI-PM MCP.
- Credentials are scoped to the intended tenant/project/principal and have bounded lifetime/revocation.
- AI-PM enforces authorization on every MCP read/write.
- SEM service credentials must never be forwarded as a human/project principal.
- SEM correlation IDs and execution IDs are not business credentials.

## 5. Superseded assumptions

The following old assumptions are no longer normative and must be removed from current design and implementation plans:

- SEM calls Platform Claim to create or advance business Run authority.
- SEM forwards Runner Lease Renewal as the business execution authority path.
- SEM ACKs or mutates Platform business commands/Task state.
- SEM owns Task/HIR/Barrier/Consultation/successor/retry/completion semantics.
- Runner polls Platform MCP for dispatch/control.
- SEM derives business intent or Task identity by parsing prompt content.

Any remaining Platform-owned authorization primitive must be called directly through a Platform-owned contract and must not turn SEM into a business workflow proxy.

## 6. SEM still owns real infrastructure responsibilities

This decision does not reduce SEM to a stateless byte pipe. SEM remains responsible for authenticated Runner connectivity, infrastructure-level Runner eligibility, dispatch delivery ledger, duplicate-start prevention, process supervision coordination, resource telemetry/admission, graceful/forced stop execution, process-exit observations, orphan reconciliation, and durable infrastructure recovery.

It owns those infrastructure facts and resources only—not their business interpretation.

## 7. Required next work

- Replace Protocol V1 with the opaque execution request/delivery/release/runtime-observation protocol.
- Map existing AI-PM Claim, Lease, WorkerCommand, MCP, Runner and Worker code paths to retain/move/retire.
- Specify Harness MCP identity provisioning and project authorization.
- Define execution ID/generation fencing, idempotency and duplicate/late-message handling.
- Define process start/exit evidence and cleanup reconciliation.
- Reconcile all SEM-02E–02J documents and ADRs; keep unresolved lock audit findings as source evidence, but do not treat old SEM-owned business control assumptions as approved.
- Add contract and failure-injection tests before implementation.

No business code, migration, API implementation or Phase 0 work is authorized by this decision record.
