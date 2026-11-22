# PA08-04 — Workspace / Mirror Reclamation Inventory

- **Status:** PROCESS-CLEANUP CLAIM FIX IMPLEMENTED; PostgreSQL CI VERIFICATION PENDING
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

Real PostgreSQL tests now cover concurrent duplicate claims, unexpired lease protection, expired lease recovery, stale-token rejection, valid failure/backoff/retry, and valid completion. Hosted CI verification is pending; no filesystem reclamation is enabled by this change.

## 5. Workspace reclamation ledger added

Migration `0055_workspace_reclamation_ledger.sql` adds a separate durable ledger for `run_worktree` and `repository_mirror` resources. It stores only tenant/project/run/repository identity and a validated relative path, plus pending/cleaning/completed/needs_human retry state and fencing-token fields. Partial unique indexes enforce one worktree record per Run/repository and one mirror record per repository. Foreign keys preserve the referenced Run repository snapshot and Repository row; no cascade deletion is used.

Real PostgreSQL schema tests cover separate worktree/mirror identity, duplicate mirror rejection, and invalid worktree shape rejection. Hosted CI verification is pending.

## 6. Planned implementation sequence

1. Complete Hosted CI verification for process-cleanup fencing and the new reclamation ledger schema tests.
2. Add dry-run candidate discovery for terminal Run worktrees and archived repository mirrors. Do not conflate process termination status with disk deletion status.
3. Add dry-run candidate discovery for terminal Run worktrees and archived repository mirrors.
4. Implement CodexExecutor filesystem operations with canonical path checks, mirror lock reuse, Git worktree removal/prune, and mirror worktree-list verification.
5. Add second-check DB eligibility immediately before deletion, retry/backoff, and `needs_human` visibility.
6. Wire a periodic reclamation round into `ai-pm-worker`; default dry-run and deletion disabled.
7. Validate with real PostgreSQL concurrency tests and filesystem tests (including symlink escape, active Run, pending process cleanup, lock contention, and idempotent missing paths).

## 7. Verified source files

- `crates/pm-infrastructure/src/codex_executor.rs`
- `crates/pm-application/src/run_cleanup.rs`
- `crates/pm-application/src/run_dispatch/port.rs`
- `apps/ai-pm-worker/src/main.rs`
- `apps/ai-pm-runner/src/workspace.rs`
- `migrations/0001_baseline.sql`

This document is an inventory and safety baseline. The independent ledgers are in place, but no filesystem deletion behavior is implemented or enabled.
