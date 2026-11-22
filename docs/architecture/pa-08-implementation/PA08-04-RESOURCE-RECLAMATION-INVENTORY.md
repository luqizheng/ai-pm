# PA08-04 — Workspace / Mirror Reclamation Inventory

- **Status:** INVENTORY BASELINE (no filesystem deletion code changed)
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

## 4. Process cleanup ledger issue requiring a real-PostgreSQL test

Static inspection found a concrete risk in `run_cleanup::claim_batch`:
- The `SELECT ... FOR UPDATE SKIP LOCKED` uses `fetch_all(pool)` (autocommit), then the lock is released before a separate UPDATE marks rows `cleaning`.
- The selection predicate includes both `pending` and `cleaning`, while the UPDATE does not advance `next_retry_at`. Therefore a row marked `cleaning` remains immediately eligible and can be claimed again by a second Worker even without a process crash.

This must be reproduced by a real PostgreSQL test before changing the code. Expected corrected semantics:
- Claim and status/token update are atomic.
- A `cleaning` row has an explicit claim lease expiry; it cannot be reclaimed before expiry.
- Each claim has a unique fencing token. `mark_completed` / `mark_failed` must compare the token so a stale Worker cannot overwrite a newer claimant's result.
- Worker crash recovery remains supported after claim lease expiry.

## 5. Planned implementation sequence

1. Add a real PostgreSQL test reproducing duplicate `task_run_cleanup` claims; then fix claim lease + fencing token and update the existing Worker.
2. Add a separate workspace-reclamation ledger. Do not conflate process termination status with disk deletion status.
3. Add dry-run candidate discovery for terminal Run worktrees and archived repository mirrors.
4. Implement CodexExecutor filesystem operations with canonical path checks, mirror lock reuse, Git worktree removal/prune, and mirror worktree-list verification.
5. Add second-check DB eligibility immediately before deletion, retry/backoff, and `needs_human` visibility.
6. Wire a periodic reclamation round into `ai-pm-worker`; default dry-run and deletion disabled.
7. Validate with real PostgreSQL concurrency tests and filesystem tests (including symlink escape, active Run, pending process cleanup, lock contention, and idempotent missing paths).

## 6. Verified source files

- `crates/pm-infrastructure/src/codex_executor.rs`
- `crates/pm-application/src/run_cleanup.rs`
- `crates/pm-application/src/run_dispatch/port.rs`
- `apps/ai-pm-worker/src/main.rs`
- `apps/ai-pm-runner/src/workspace.rs`
- `migrations/0001_baseline.sql`

This document is an inventory and safety baseline. No filesystem deletion behavior is implemented or enabled by this commit.
