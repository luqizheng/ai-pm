# PA08-05 — Product Lifecycle Integrity Inventory

**Status:** IMPLEMENTED; Hosted CI verification in progress.

## 1. Authority and scope

PA-07 is authoritative for the Product execution lifecycle. The lifecycle is separate from Task, Run, Project, and Workflow lifecycles:

- `normal`
- `suspended`
- `resuming`
- `archived` (history retained; no ordinary restoration)

Relevant baseline: PA-07 §§4, 11, and 12. In particular, a Product leaving `suspended` must enter `resuming` for Task Portfolio Review before returning to `normal`; `archived` is not an ordinary reversible state.

## 2. Enforced transition matrix

| Source | Action | Target | Result |
|---|---|---|---|
| normal | suspend | suspended | Allowed |
| normal | abandon | archived | Allowed |
| suspended | resume | resuming | Allowed |
| suspended | abandon | archived | Allowed |
| resuming | complete_resume | normal | Allowed |
| resuming | suspend | suspended | Allowed (a new pause event) |
| resuming | abandon | archived | Allowed |
| archived | any ordinary lifecycle action | — | Rejected |

All other source/action combinations are rejected with `product_lifecycle_transition_invalid`. Repeating an action whose target equals the current state remains a `product_lifecycle_noop` conflict. Unknown actions return validation error.

The legacy `restart` action was removed. It previously allowed `archived → normal`, contradicting PA-07 §12. It must not be used as a shortcut around the mandatory `suspended → resuming → normal` recovery process. No archived Product restoration API is introduced.

## 3. Transaction and authorization boundary

- Product lifecycle changes remain Product Owner-only.
- The application service locks the Product row with `SELECT ... FOR UPDATE` before checking the current state.
- Source-state validation, status update, and `product.lifecycle_changed` audit insertion occur in the same transaction.
- Rejected transitions return before DML or audit insertion.
- This does not batch-update Projects, Tasks, Runs, or Workflows and does not reopen terminal execution entities.
- The Product active gate remains fail-closed for non-`normal` states.

## 4. Tests added

- Existing API integration coverage still verifies Owner-only lifecycle authority, archived Product write gating, and audit behavior; its legacy `restart` expectation now verifies that archived Products cannot be restored.
- New API integration coverage exercises `normal → suspended → resuming → normal`, rejects a resume from `normal`, archives the Product, rejects completion from `archived`, and verifies rejected transitions create no audit rows.
- Hosted CI run: [PA-08 Run #133](https://github.com/luqizheng/ai-pm/actions/runs/44974739861) — pending at the time of this inventory update.

## 5. Remaining PA08-05 verification

- Confirm the current Hosted run completes successfully.
- Add/verify concurrency coverage for simultaneous Product lifecycle transitions and the existing Run claim/start Product gate; preserve the lock ordering already established by PA08-03.
- Verify a repeated pause while `resuming` produces a separate audited lifecycle event only when the transition is valid.
- Confirm PA-07 regression remains green.

No change is made to PA-06 authorization boundaries or PA-07 lifecycle isolation.
