# SEM Cutover — Harness Identity, Business Fencing and Completion Review

- Status: **Read-only source audit / architecture requirements; review pending**.
- Source baseline: ai-pm master at d0e72fa6805aebeadf1b282e430deba493072fd6.
- Related: [execution path ownership mapping](./execution-path-ownership-mapping.md).
- No application code, migration, API or Runner behavior was changed.

## 1. P0 — Existing MCP identity is not a delegated execution identity

### Verified current implementation

1. apps/pm-api/src/auth.rs accepts the lmms human-token path and maps the token sub to a human core.principal. Service JWTs are explicitly rejected from business endpoints (IAM_PERM_3003).
2. The cpk_ Client Credential path resolves a registered device and injects an AuthedPrincipal whose principal_id is the device owner's human principal. It also injects principal.human_actor().
3. AuthedPrincipal::human_actor() always produces HumanActor.
4. crates/pm-mcp/src/caller.rs::caller_from_ctx requires that HumanActor to exist. authorize() checks the human principal's project membership, then resolves the project's dev_agent or ai_pm execution/audit principal. The MCP call is not authenticated as an independent Harness execution.
5. docs/governance/ai-pm-principal-identity-v1.md explicitly records the existing decision: no independent Agent credential; the human holding the API Key is the authentication principal and the Agent is an attribution principal.

### Consequence

The existing model can attribute an MCP action to a human who supplied a key and a project dev_agent actor, but it cannot prove that a particular Harness invocation is authorized for exactly one Task/Run/attempt. A device cpk_ proves a registered client/device identity; it is not, by itself, a per-execution business grant.

Do not solve this by:
- putting a reusable human API Key in the prompt or SEM message;
- reusing SEM service JWT or Runner WSS credentials as MCP credentials;
- accepting a caller-supplied Task ID or correlation ID as authorization;
- treating dev_agent project attribution as proof of a currently authorized execution.

### Required design: Delegated Execution Identity

Introduce a distinct, short-lived **delegated execution grant** (the name is a proposal, not an approved schema). It should represent one AI-PM-authorized execution attempt, not impersonate a human.

Minimum claims/bindings:
- issuer and audience = AI-PM MCP;
- tenant_id, project_id, task_id and AI-PM Run/attempt identity;
- immutable execution_id plus Platform-owned authorization_generation;
- permitted MCP operations / capability set;
- initiating/authorizing human principal (if applicable) as delegated_by, separate from the Harness executor principal;
- Runner/device binding where required by the trust model;
- issued_at, not_before, expiry, unique token ID and revocation/consumption state.

AI-PM must authorize every MCP call against the active execution attempt and its allowed operations. The grant must not authorize human approval, formal acceptance, project administration, unrelated Task access, or arbitrary status mutation.

Audit should preserve at least:
- delegated_by (human or authorized Platform actor);
- executor_principal / Harness identity;
- execution_id and authorization generation;
- client/Runner identity and credential channel.

A suitable application-level caller model must support HumanCaller and DelegatedExecutionCaller explicitly. Do not fabricate a HumanActor for the delegated execution path. Existing human-only REST and approval guards must remain human-only.

Credential delivery remains an open security design item. Prefer a one-time, Runner-bound bootstrap exchange with AI-PM over placing a reusable MCP bearer token in a prompt body. If any bootstrap ticket traverses SEM, it must be cryptographically bound to the intended Runner and unusable by SEM or another Runner; define expiry, one-time redemption, revocation and secret redaction. A Runner-local credential broker/proxy may inject the short-lived MCP token into Harness requests without exposing a reusable human key.



### 1.1 Existing MCP permission and resource-scope gap

The current pm-api MCP permission gate requires PM_MCP_READ for every MCP request and adds a small per-tool permission map. In apps/pm-api/src/auth.rs, PM_MCP_TASK_UPDATE is explicitly described as unassigned until a task-update tool exists. The current map does not list pm_report_progress or pm_submit_result, even though these tools mutate Run/Task business state; therefore they pass the generic PM_MCP_READ gate rather than a dedicated task-update permission. The new delegated grant must enforce operation-level capabilities in the AI-PM application service, not merely rely on a broad transport permission.

The current pm_get_task_context implementation calls ensure_member(project_id) and then reads the supplied task_ref. This is project-member scope, not a per-execution Task grant. The new Harness context read must validate the exact task_id/run_id/execution_id/authorization_generation tuple on every call and must not allow a project member or Harness to select an unrelated Task by changing task_ref.

The formal review endpoint pm_reviews explicitly calls require_human_principal; this human-only gate must remain in force. A delegated Harness grant must never be accepted as a human review/approval identity.

## 2. P0 — Two independent fencing domains are required

The replacement protocol currently defines a SEM execution_epoch for Runner binding/release fencing. That epoch is an infrastructure fact and cannot replace AI-PM business authorization fencing.

Define two separate values:

| Value | Authority | Protects |
|---|---|---|
| authorization_generation (or equivalent immutable attempt grant generation) | AI-PM | Harness MCP reads/writes for the authorized Task/Run attempt; revokes stale Harness business authority |
| binding_epoch / execution_epoch | SEM | SEM→Runner delivery/release messages for the current Runner binding |

Rules:
1. AI-PM validates the active execution attempt and authorization generation on every delegated Harness MCP operation.
2. SEM validates the current binding epoch on every Runner-directed delivery/release operation.
3. Neither epoch is accepted as a substitute for the other.
4. A new business retry is a new AI-PM attempt/execution identity and invalidates the old grant.
5. Rebinding the same business attempt to a different Runner must revoke/fence the old Harness grant before a new Harness can mutate business state.
6. A stale Harness may still run local code after losing MCP authorization. Therefore, revoking its MCP grant alone is not proof that it can no longer mutate a Git remote, invoke external tools, or hold local resources.

## 3. P0 — Runner loss must not cause unsafe reassignment

If a Runner disconnects, SEM cannot infer that its Harness process exited. Before dispatching the same business work to another Runner, require:
- positive process-exit evidence from the original Runner; or
- an explicitly designed isolation/fencing procedure that makes the old process unable to mutate protected external resources.

Otherwise retain infrastructure_uncertain, quarantine the old binding/resource and do not start a competing Harness for the same attempt. AI-PM may revoke the old MCP grant to prevent further business writes, but this alone does not stop arbitrary local code or Git/network side effects.

Tests must cover: old Runner disconnects while process is alive; old Harness attempts MCP writes after grant revocation; old Runner reconnects and reports stale process; new Runner is not started until safe; stale release/delivery messages cannot affect the new binding.

## 4. P0 — Harness submission is not Task completion

The current Task state machine explicitly distinguishes:
- Running → Submitted: requires a delivery manifest (delivery_submitted=true);
- Submitted → Reviewing;
- Reviewing → Completed: requires complete acceptance evidence (acceptance_evidence_complete=true);
- Ready → Completed: a special manual-task path guarded by manual_completion, with manual_complete_task requiring a human principal and a reason.

The current pm_submit_result MCP implementation submits and validates delivery facts and advances the Task to submitted; D4 explicitly says it does not decide final Task status. It is not a Harness self-acceptance operation.

Therefore:
- Harness may report progress and submit a delivery/result through typed AI-PM MCP operations.
- AI-PM validates the submission and advances to submitted where valid.
- Review/acceptance decides completed through the existing guarded business workflow, preserving human approval rules.
- Harness must not call a generic set_task_status(completed) operation.
- AI-PM sends SEM release when the business execution no longer needs the runtime (for example, after accepted completion, cancellation, or a decision to stop while awaiting human input). The release reason is an AI-PM business decision; SEM sees only the opaque execution identity and stop instruction.

## 5. P1 — Release must be transactionally coupled to the Platform decision

When an AI-PM business transition makes an execution no longer required, persist the release intent in the same AI-PM database transaction as the business decision via Platform Outbox. Delivery to SEM is at-least-once and idempotent. This avoids a crash window where the Task is completed/cancelled but no release request is ever emitted.

SEM's accepted/released status remains infrastructure-only. A failed or delayed release does not roll back the business decision; it creates an infrastructure cleanup obligation and visible uncertainty.

## 6. P1 — Existing runbook and remote workspace contract need reconciliation

The current docs/executor/ai-pm-executor-onboarding-runbook-v1.md still describes the old human API Key + pm_claim_task Model A flow and states that pm_submit_result is Unsupported. The current source baseline contains a real D4 pm_submit_result implementation. The runbook is historical/stale for the new architecture and must not be used as the cutover contract.

The current D4 verifier supports both:
- local workspace verification (requires the Platform process to access the Run worktree); and
- a remote-carrier path for client: lease owners, which verifies the declared SHA against remote branch heads and ancestry using a temporary bare clone.

The new Runner topology must explicitly choose and test the remote verification contract. It must not assume AI-PM can inspect a Runner-local workspace. Preserve the distinction between remote Git evidence and local workspace HEAD evidence; do not claim the latter if only remote branch evidence is available.

## 7. Required acceptance cases

1. Human API Key is not accepted as a reusable Harness execution credential by default.
2. SEM service JWT and Runner WSS credential are rejected by AI-PM MCP.
3. Delegated grant is scoped to one tenant/project/task/attempt and a strict operation allow-list.
4. Cross-project, cross-task, stale-generation, expired, revoked and replayed grants are rejected.
5. Harness cannot call human approval, formal acceptance, project admin or unrestricted Task status mutation.
6. Human initiator and delegated Harness executor are separately visible in audit.
7. Releasing an execution revokes or invalidates its delegated grant according to explicit policy.
8. Old Harness with revoked MCP grant cannot change Task state or submit delivery.
9. Runner disconnect with process still alive does not trigger competing reassignment.
10. Running→Submitted→Reviewing→Completed is enforced; Harness cannot self-complete an execution Task.
11. Business transition + Platform Outbox release intent is atomic; duplicate outbox delivery produces one SEM release effect.
12. Remote Git verification is tested with a Runner workspace unavailable to the Platform host.

No implementation or migration is authorized by this review.


## 6.1 P1 — Remote Git evidence is weaker than local workspace verification

The current D4 remote verifier is selected by lease_owner starting with client:. It checks that the submitted SHA is a remote branch head and that the locked baseline is an ancestor (using a temporary bare clone). Because the Platform host cannot inspect the Runner-local worktree, this path cannot prove the Runner-local working tree HEAD. It also currently echoes the submitted changed_files count rather than calculating the diff from the remote clone.

The new architecture removes the old Runner Claim/lease_owner identity as the natural selector for this path. It must define an explicit execution workspace evidence mode; do not infer it from a credential string prefix. For a remote Runner, the contract should require a unique execution branch/ref to be pushed before submission, bind the submitted SHA to that ref, verify the ref and ancestry on the remote, and calculate changed-file facts from the remote object graph rather than trusting the Harness declaration. If a commit is not pushed or cannot be fetched, submission must be rejected or remain explicitly pending; do not silently fall back to an unverified local claim.

If the product requires proof of the exact local worktree HEAD rather than remote branch state, define a separate Runner-signed attestation and trust model. A plain Harness JSON claim is not independent evidence. Tests must run with AI-PM on a host that cannot access the Runner workspace.


## 8. P1 — Keep Task, AI-PM Run and SEM Execution as three lifecycles

The new design must not equate Task completion with process exit or SEM release. There are three distinct lifecycles:

| Lifecycle | Authority | Meaning |
|---|---|---|
| Task | AI-PM | Business work state: Running → Submitted → Reviewing → Completed, plus guarded blocked/cancelled/failed paths |
| Run / attempt | AI-PM | One business execution attempt, its authorization generation, delivery association and Platform-decided outcome |
| SEM Execution | SEM | Infrastructure delivery/binding/process/release state |

Harness MCP may perform guarded business transitions such as starting (Task=running) and submitting a validated delivery (Task=submitted). It cannot set completed. Review/acceptance decides completion. Runner process start/exit is an infrastructure observation; AI-PM correlates it with its Run and decides the Run outcome. A zero exit code is not business success.

SEM release should occur when AI-PM decides the specific Run/attempt no longer needs a runtime. This may be after accepted delivery while Task is still submitted/reviewing, after cancellation, or while awaiting human input. Do not trigger release only from Task=completed. Persist the release intent in the same Platform transaction as the relevant Run/Task decision through Outbox.


## 9. Companion SEM contract draft

The downstream SEM repository now contains a detailed, review-pending proposal for the four connected contracts: [Execution Identity, Lifecycle, Fencing and Delivery Contract](https://github.com/luqizheng/smark-sem/blob/docs/sem-authority-boundary-convergence/docs/protocol/EXECUTION-IDENTITY-LIFECYCLE-FENCING-DELIVERY-CONTRACT-DRAFT.md).

Its proposed two-stage Runner-bound bootstrap is a candidate design, not an approved credential architecture. AI-PM must retain authority over delegated grant issuance, exact Task/Run/attempt scope, operation allow-list, authorization_generation and per-call MCP enforcement. SEM transports the opaque bootstrap ticket and prompt; it must not be able to redeem the ticket or inspect the resulting MCP credential.

The SEM contract also standardizes binding_epoch as the SEM-side Runner-binding fence, separate from AI-PM authorization_generation. Runner disconnect without positive process-exit or independently verified isolation evidence remains infrastructure_uncertain and prohibits competing reassignment. The detailed contract is cross-repository design input only; it does not change the verified current AI-PM source behavior or authorize implementation.


## 10. Follow-up source verification: current device credential is not a Runner-bound execution identity

Read-only source baseline checked on 2026-11-25 (AI-PM master at the review branch's source baseline; relevant files and symbols):

- `apps/pm-api/src/pm_devices.rs`: device registration accepts device metadata and `is_runner`; there is no public-key/certificate enrollment field in the registration request. Device credential issue/revoke/permission management is performed by the device owner or tenant admin.
- `crates/pm-application/src/device_credential.rs`: current `cpk_` credential is a reusable 256-bit random static secret, stored as SHA-256 hash in `devenv.device`, issued once and revoked by updating `credential_revoked_at`. `resolve` checks device active state, revocation and owner principal active state on each request. It does not bind the credential to a Task, Run, attempt, execution_id, authorization_generation, Runner key or operation-specific grant.
- `apps/pm-api/src/auth.rs`: successful `cpk_` authentication constructs an `AuthedPrincipal` and injects `HumanActor` for MCP. This is device attribution under the existing user-owned device model, not a delegated Harness principal.
- `crates/pm-mcp/src/caller.rs` and `crates/pm-mcp/src/tools.rs`: caller authorization still uses the owner human principal/project membership and broad project-member checks; task_ref is a selector and current checks are not a per-execution grant boundary.

Consequences:
1. Existing `cpk_` must not be reused as the delegated Harness credential. Its static, device-wide, owner-attributed semantics are incompatible with the required per-execution scope and non-impersonation rule.
2. Existing device enrollment does not establish a cryptographic Runner public-key identity. The two-stage bootstrap cannot be implemented securely until enrollment/attestation and key rotation/revocation are designed.
3. The new design needs a distinct delegated execution grant and a separate Harness executor audit identity; do not silently repurpose `devenv.device` / `cpk_` without an explicit domain/security review.
4. The current permission vocabulary (`PM_MCP_READ`, `PM_MCP_AGENT_REPORT`, `PM_MCP_COMMAND`) is not sufficient proof of exact Task/Run/attempt scope. The grant must be enforced by the AI-PM application-service boundary for every operation.

This is a source finding, not an implementation instruction. It reinforces the credential/enrollment design gate; it does not authorize a schema migration or compatibility change.
