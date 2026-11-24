# SEM V1 Security Model

Status: security design baseline.

## 1. Principals

- Human user: may view/resolve HIR and select Runner within authorized tenant/project scope.
- Platform services: act under service identity and explicit DB permissions.
- SEM worker: manages execution lifecycle only within authorized scope.
- Client Runner: device identity plus current lease proof; scoped to claimed Run/generation.
- Harness process: receives only execution-scoped credentials.
- Git remote: one authoritative origin per Repository.

## 2. Authorization

- Preserve exact lease_client_id/device identity binding.
- Every mutation checks tenant/project scope, Run identity, generation and current lease proof.
- HIR operations require user authorization and audit.
- Runner supervision changes require authorized user/admin action.
- SEM DB credentials are least-privilege; no runtime superuser.

## 3. Lease and fencing

- Store token verifier/hash, not plaintext.
- Issue fresh token on claim and rotate on new generation.
- Reject stale generation and mismatched client identity.
- Generation fencing protects DB state, not Git/network/other external side effects.

## 4. Harness and Git credentials

- Use per-Run or per-Repository minimum-scope credentials where supported.
- Prefer short-lived, revocable credentials; do not inject long-lived personal tokens into generic Harness processes.
- SSH keys use restrictive permissions and temporary-file lifecycle.
- Environment, process arguments, stdout/stderr, executor events, exceptions and HIR details must not expose secrets.
- Revocation status/time is auditable. DB token rotation does not revoke a credential already held by an OS process.
- If an old process may still hold a credential, assume it can still perform external actions until process termination or credential-side revocation is confirmed.

## 5. E3 trust

E3 is accepted only when Runner supervisor authentication is valid; Run/generation/command/process identity match; supervisor observed OS process termination; and exit observation/time are included. Duplicate delivery is idempotent.

A process's own self-report that it is stopping is insufficient. A generic HTTP response is insufficient.

## 6. Workspace isolation

- One active writer per workspace.
- Every replacement Attempt gets a new workspace.
- Resolve workspace paths under configured root; reject traversal and symlink escape.
- Do not delete/reuse a workspace while process termination is uncertain.
- Runtime-persistent Workspace volume is not backed up.

## 7. Audit

Audit claim, generation rotation, shield creation/release, termination evidence, credential issuance/revocation, HIR transitions, Runner supervision changes and handoff. Redact secrets before persistence.
