# PA08-04 — Workspace / Mirror Reclamation Inventory

- **Status:** RUN-WORKTREE + MIRROR RECLAMATION WORKER IMPLEMENTED (DEFAULT-OFF); EXPANDED HOSTED CI VERIFYING
- **Date:** 2026-11-22
- **Branch:** `pa-08/implementation-baseline`
- **Purpose:** Establish ownership, safety gates, retention, and retry semantics before implementing resource reclamation.

## 1. Resource ownership map

| Resource | Current location / identity | Owner | Reclamation policy |
|---|---|---|---|
| Codex per-Run worktree | `<AI_PM_WORKSPACE_ROOT>/<run_id>/<task_run_repository.workspace_path>` | `CodexExecutor` | Reclaim only after terminal Run retention, process cleanup completion, second DB check, canonical path validation, and mirror lock. |
| Shared bare Git mirror | `<AI_PM_WORKSPACE_ROOT>/git-mirror/<repository_id>` | `CodexExecutor` | Reclaim only after repository is archived, no active Run/cleanup reference remains, and Git reports no linked worktrees under the mirror lock. It is a rebuildable cache; Repository rows and Commit records are never deleted. |
| Mirror lock file | `git-mirror/<repository_id>.lock` | `CodexExecutor::lock_mirror` | Never delete an active lock. Existing stale-lock policy remains authoritative; reclamation uses the same lock. |
| Run logs | `<run_root>/logs/{stdout,stderr}.log` | `CodexExecutor` | Preserve. These are execution evidence and may be needed for audit/debugging. |
| Process identity evidence | `<run_root>/process.json`, `executor-events.jsonl` | `CodexExecutor` | Preserve. Process cleanup is a separate lifecycle and must complete before worktree deletion. |
| Run snapshot metadata | `delivery.task_run_repository` | Database | Preserve. It is immutable historical execution context; deleting worktree bytes does not delete the snapshot row. |
| Delivered artifacts / commits / audit / outbox | Database and durable storage | Platform | Out of scope; never delete or rewrite as part of workspace reclamation. |
| Remote Runner user-mapped local repositories | `AI_PM_RUNNER_GIT_MAP` paths consumed by `apps/ai-pm-runner/src/workspace.rs` | User / remote Runner host | Not owned by CodexExecutor. Never scan or delete these paths. |

## 2. Confirmed implementation facts

1. `CodexExecutor::run_root(run_id)` resolves to `root/<run_id>`.
2. `CodexExecutor::checkout` uses a shared bare mirror at `root/git-mirror/<repository_id>`, guarded by `lock_mirror(repository_id)`.
3. Each repository checkout is a Git worktree under the per-Run root. The path is supplied by the immutable `delivery.task_run_repository.workspace_path` snapshot.
4. `CodexExecutor::terminate` terminates the process tree and appends an executor event, but does not remove the worktree or mirror.
5. Run process cleanup is tracked separately in `delivery.task_run_cleanup`. A terminal Run is not sufficient evidence that its process has exited.
6. `run_cleanup::run_cleanup_round` is invoked by `apps/ai-pm-worker/src/main.rs`; it is the existing periodic process-cleanup worker.
7. `apps/ai-pm-runner` has a separate workspace root and user-provided local path mappings. These are not the CodexExecutor filesystem and must not be reclaimed by this worker.

## 3. Safety invariants

### Per-Run worktree

A Run worktree is eligible only when all are true:
- Run status is terminal: `succeeded`, `failed`, `timed_out`, or `cancelled`.
- Terminal timestamp is older than the configured retention period (initial default proposal: 30 days).
- If `executor_session` exists, the matching `task_run_cleanup` ledger row is `completed`. `pending`, `cleaning`, or `needs_human` blocks deletion.
- No active Run status or process-cleanup work references the same Run.
- The target path is a validated relative `workspace_path`, canonicalizes beneath the canonical configured root and Run root, and does not traverse a symlink outside either boundary.
- The operation holds the repository's existing mirror lock while removing the worktree and running `git worktree prune`.

Only the worktree path is deleted. The Run root, logs, process identity file, executor events, task/run rows, task_run_repository snapshot, commits, artifacts, audit and outbox records remain. The immutable workspace_path field remains as historical execution metadata; the separate reclamation ledger must explicitly mark that this path's bytes have been reclaimed. A historical metadata pointer is not treated as an active execution reference after terminal retention and process cleanup are proven.

### Shared bare mirror

A mirror is a cache, not the Repository record. It is eligible only when:
- The corresponding `integration.project_repository.status = 'archived'` and its archived/update timestamp is older than retention (initial default proposal: 30 days).
- No active Run references the repository through `delivery.task_run_repository`.
- No non-completed process cleanup entry belongs to any Run referencing the repository.
- Under `lock_mirror(repository_id)`, `git worktree list --porcelain` confirms no linked worktrees remain.
- The mirror path canonicalizes beneath `<root>/git-mirror` and is not a symlink escape.

Deleting the bare mirror never deletes `integration.project_repository`, repository credentials, commit metadata, or remote history. The repository row remains the canonical reference; only its derived local cache is reclaimed. A future checkout can recreate the mirror from the configured remote. If any check is uncertain, skip and retain the mirror.

### Universal protections

- Dry-run is the default; filesystem deletion requires explicit opt-in.
- Candidate discovery and deletion are separate steps. Recheck all DB eligibility predicates immediately before deletion.
- Acquire the same per-repository mirror lock used by checkout; never race a fetch/clone/worktree operation.
- Do not delete an entire Run root. Preserve logs and process evidence.
- Do not delete remote Runner mapped local paths, repository rows, commits, delivered artifacts, audit history, outbox events, or ProjC mirror/version history.
- Missing paths are idempotent success; path traversal, symlink escape, uncertain process identity, unknown status, or lock timeout are fail-closed skip/error outcomes.
- A cleanup failure is persisted with bounded retry/backoff and eventually becomes queryable as `needs_human`.

## 4. Process cleanup ledger claim defect and implemented correction

The original implementation separated `SELECT ... FOR UPDATE SKIP LOCKED` and the `cleaning` UPDATE across autocommit statements. This released the row lock before the claim state was persisted, and left `next_retry_at` unchanged, so a second Worker could immediately reclaim the same row.

Implemented correction (migration `0054_task_run_cleanup_claim_fencing.sql`):
- A single CTE statement selects and updates due rows atomically under `FOR UPDATE SKIP LOCKED`.
- `next_retry_at` is advanced to a 300-second claim lease expiry while status is `cleaning`.
- Every claim receives a fresh UUID `claim_token`; completion and failure updates require the current token and clear it on success/failure/requeue.
- Expired leases are reclaimable after Worker crash; stale Workers cannot mutate the new claimant's state.
- The token is excluded from serialized cleanup summaries.

Real PostgreSQL tests cover concurrent duplicate claims, unexpired lease protection, expired lease recovery, stale-token rejection, valid failure/backoff/retry, retry exhaustion to `needs_human`, and valid completion. Verified in Hosted CI [Run #117](https://github.com/luqizheng/ai-pm/actions/runs/44973471988).

## 5. Workspace reclamation ledger added

Migration `0055_workspace_reclamation_ledger.sql` adds a separate durable ledger for `run_worktree` and `repository_mirror` resources. It stores only tenant/project/run/repository identity and a validated relative path, plus pending/cleaning/completed/needs_human retry state and fencing-token fields. Partial unique indexes enforce one worktree record per Run/repository and one mirror record per repository. Foreign keys preserve the referenced Run repository snapshot and Repository row; no cascade deletion is used.

`pm_application::workspace_reclamation::stage_candidates` now idempotently stages only eligible candidates (30-day default policy constant): terminal Run worktrees require completed process cleanup when an executor session exists; archived mirrors require no active Run or unfinished cleanup reference. Staging is metadata-only and does not delete files.

Real PostgreSQL tests cover separate worktree/mirror identity, duplicate mirror rejection, invalid worktree shape, terminal retention, process-cleanup gating, archived mirror gating, idempotent staging, fenced claims, and second-check eligibility. Verified in [Run #117](https://github.com/luqizheng/ai-pm/actions/runs/44973471988).

## 6. Filesystem reclamation primitives

`CodexExecutor::reclaim_run_worktree` and `reclaim_repository_mirror` are implemented. The periodic Worker is wired for both Run worktrees and Repository mirrors. Mirror reclamation revalidates and locks the Repository row, holding the transaction through the filesystem adapter so concurrent archive/unarchive and new Run-repository references serialize against deletion. They acquire the existing per-repository mirror lock. Worktree reclamation validates the DB-sourced relative path, rejects symlinks at every path component, canonicalizes beneath the Run root, removes only that repository worktree, and prunes stale Git worktree metadata; it preserves the Run root, logs, process evidence, and database history. Missing worktrees are idempotent and still trigger mirror prune when possible.

Mirror reclamation canonicalizes beneath `git-mirror`, rejects symlinks/non-bare paths, runs Git prune, and refuses deletion if `git worktree list --porcelain` reports any linked worktree other than the bare mirror itself. It deletes only the mirror cache directory.

Unit tests cover traversal rejection, missing worktree idempotency, worktree/mirror symlink escape, mirror-lock symlink rejection, and a real Git lifecycle (linked-worktree refusal, worktree removal, and mirror removal). All passed in [Run #117](https://github.com/luqizheng/ai-pm/actions/runs/44973471988). The application Worker performs a second eligibility check before invoking either filesystem primitive. The expanded mirror Worker and row-lock concurrency tests are being revalidated in [Hosted Run #131](https://github.com/luqizheng/ai-pm/actions/runs/44974426580).

## 7. Worker policy and remaining gates

- Candidate discovery runs periodically and is metadata-only. It uses a 30-day default retention (`AI_PM_WORKSPACE_RECLAMATION_RETENTION_DAYS` can override it).
- File deletion is disabled by default. Enabling requires `AI_PM_WORKSPACE_RECLAMATION_ENABLED=true`, `AI_PM_WORKSPACE_RECLAMATION_SHARED_ROOT=true`, and an absolute `AI_PM_WORKSPACE_ROOT`. The shared-root acknowledgement is required because worktrees are local filesystem resources.
- The Worker claims `run_worktree` and `repository_mirror` rows in separate rounds. Each round revalidates the current fencing token and resource eligibility before filesystem work. Worktree eligibility includes terminal Run status, retention, immutable repository snapshot path, and process-cleanup completion.
- Mirror second-check locks the Repository row and relevant Run/worktree/cleanup rows, retaining the transaction until the filesystem adapter and ledger completion/failure update finish. Repository archive/update writes and new FK-backed Run-repository references therefore serialize against mirror deletion. The existing filesystem mirror lock and Git linked-worktree check provide an additional guard.
- Application operations now list `needs_human` records and explicitly requeue an operator-reviewed record. PA08-06 will add backlog/bytes metrics.

This inventory remains a safety baseline. No Repository/Commit/Run/audit/artifact history is deleted; production deletion remains explicitly opt-in.

## 8. Verified source files

- `crates/pm-infrastructure/src/codex_executor.rs`
- `crates/pm-application/src/run_cleanup.rs`
- `crates/pm-application/src/run_dispatch/port.rs`
- `apps/ai-pm-worker/src/main.rs`
- `apps/ai-pm-runner/src/workspace.rs`
- `migrations/0001_baseline.sql`

This document is an inventory and safety baseline. Filesystem deletion is implemented but remains disabled unless the explicit operator opt-ins are present.
