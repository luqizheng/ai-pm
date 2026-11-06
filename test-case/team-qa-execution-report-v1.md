# Team Feature QA · Execution Report V1.0

> 2026-11-06 · master @ `3680138` · Plan: `team-qa-test-plan-v1.md` · Cases and results: `team-qa-test-cases-v1.md` · Evidence: `test-case/evidence/team-qa/`

## 1. Summary

**Release recommendation: NOT READY. One P0 and one P1 must be fixed first.**

- **P0, DEF-01:** master cannot run migrations against a clean database. pm-api cannot start on a new deployment, and every `sqlx::test` fails.
- **P1, DEF-02:** a removed member keeps read access to the team roster.

All results except the precheck were produced in a copy outside the repo, with `0044_s02_i3` renumbered to 0045. Under that assumption, the core team logic is sound:
- Team claiming, invitation and link lifecycles, concurrency (claim race, invitation race, TDM-011 link quota), the permission matrix, cross-tenant isolation, and dissolve/rebuild all pass.
- The other defects are in removed-member handling and in frontend error rendering.

| Layer | Suite | Result |
|---|---|---|
| Precheck | Unique migration versions | **FAIL** (DEF-01) |
| L1 regression | team_onboarding (existing) | 16/16 PASS |
| L1 regression | tenant_members (existing) | 12/12 PASS |
| L1 regression | project_members (existing) | 15/15 PASS |
| L1 new | team_onboarding_qa | 28 PASS / **2 FAIL** / 3 pending decision |
| L2 UI E2E | team-onboarding.spec.ts (real backend) | 15 PASS / **4 FAIL** / 1 pending decision |

Total test cases: 88 = 73 PASS / 7 FAIL / 4 PENDING-DECISION / 3 NOT IMPLEMENTED / 1 environment record.

**Harness validity check.** I deliberately broke one passing assertion in each layer:
- backend: `owners == 2`
- UI: wrong team name

Both runs went red as expected, and I reverted both changes afterwards.

## 2. Defects

| ID | Severity | Summary | Reproduction / evidence | Suspected cause |
|---|---|---|---|---|
| DEF-01 | **P0** | Migration version 0044 is duplicated:<br>• `0044_s02_i3_source_traceability.sql` (5936105)<br>• `0044_team_onboarding.sql` (c2b6199) | Clean DB: `duplicate key … _sqlx_migrations_pkey (version)=(44)`.<br>Default-mode run: project_members 0/15.<br>(`run-20261106-221117.log`, `backend-20261106-221117.log`) | Parallel work lines claimed the same number. The ledger lists only team_onboarding as 0044. Under the ledger rule "the later implementer renumbers", one of the two must move to 0045+. **Which one moves, and how databases that already applied a 0044 get repaired, is the user's call.** Only clean databases were verified. |
| DEF-02 | **P1** | A member removed via `DELETE /tenants/{id}/members/{pid}` can still read the roster and team profile with the same JWT | `qa_b05_02`: returns 200 | Removal deletes only the `tenant_member` row; `principal.is_active` stays true. `guard_tenant` checks only that tid matches, and the missing role defaults to member. This violates design §6.3 "Verify Active Membership". |
| DEF-03 | P2 | The removed member's `/me` is self-contradictory: `onboarding_required=false`, `team` present, `memberships=[]` | `qa_b05_03` | `principals.rs` derives `onboarding_required` only from `is_active` and tenant activity, not from membership. The frontend therefore lets the user into the dashboard, and they cannot reclaim the tenant (409). Same root cause as DEF-02. |
| DEF-04 | P2 | When accepting an invitation fails, the page always shows "Invitation invalid". It never shows email mismatch, tenant mismatch, expired, revoked, link exhausted, already accepted or team suspended | B09-08 / B09-10: backend returns 403 `email_mismatch` / `invitation_tenant_mismatch`, UI shows `data-state=invalid` | `TeamInvitationAcceptView.vue:146` passes the `HttpError` object to `translateError(code: string)`, so `invitationErrorKey` always falls through to its default. `strict: false` hides the type error, and the unit tests do not cover failure paths (OBS-2). |
| DEF-05 | P3 | The "default role" field on the invitation-link screen is blank | B09-19 | `OnboardingInvitationLinkView` uses `<Input value="MEMBER">`, but the component only binds `modelValue`. |
| DEF-06 | P3 | Refreshing or deep-linking into `/onboarding/invite` shows the banner as "新团队" (the fallback "New team" name), and Send does nothing (no request, no error) | B09-20 | The view does not call `teamStore.load()` on mount, so `tenantId` is empty and `submitInvitations` returns early. This is the same class of bug the team settings pages already fixed. |

## 3. Pending decisions (not counted as defects; tests are written to the design spec and marked `#[ignore]` / `fixme`)

| ID | Spec vs. current behaviour | Test |
|---|---|---|
| PD-1 | Design §6.1 and the direct-grant path say admin cannot grant admin. Via invitations, an admin **can** create a role=admin invitation (201). | `qa_b02_18` |
| PD-2 | An invitation already persisted as EXPIRED, when accepted again, returns `invitation_already_accepted` (status gate at L717) instead of `invitation_expired`. | `qa_b02_19` |
| PD-3 | In the already-member branch, the response `role` echoes the invitation's role, not the actual role. The stored role is unchanged, so there is no privilege escalation. | `qa_b02_14` |
| PD-4 | The onboarding gate exists only on the sign-in hand-off page; there is no route-level guard. A user without a team who deep-links to `/dashboard` is not redirected. The backend still rejects their team-area calls via `require_active`. | B09-18 |

## 4. Environment findings (not product defects)

- **ENV-1:** `template1` on the shared PG container `infrastructure-aipm-pg-1` has been pre-seeded with migrations from another checkout. Migration 0025's checksum differs because of CRLF vs LF line endings. As a result, every `sqlx::test` pointed at `:16433` fails with `VersionMismatch(25)`. Fix: rebuild the template from the current checkout, or drop `_sqlx_migrations` from `template1`. I did not touch it in this run.
- **ENV-2:** `infrastructure-pm-api-1` is in a restart loop with `VersionMissing(40)`. The image dates from 2026-11-05 and is older than the database, which has migrations up to 42 applied. Fix: rebuild the image.

## 5. Coverage against acceptance criteria

- **TDM (DB design §15):** 001, 002, 005–016 are covered.
  - 012 and 013 are covered by existing tenant_members tests.
  - 016 is indirectly covered by the concurrent claim test (the losing side leaves no audit row or profile).
  - 003, 004, 017 and 018 are not done in V1.
- **Spec §9:** items 1–4, 10, 11 and 12 pass. Items 5–9 are V1-excluded or out of this repo's scope (see cases B11).
- **Not covered:** migration rollback (TDM §15 L966); real IAM login flow (this run used self-signed tokens); email delivery (not done in V1).

## 6. Reproduction

```bash
# In ai-pm/. A dedicated QA PG container aipm-teamqa-pg:16499 is created on demand.
bash test-case/evidence/team-qa/run-team-qa.sh                  # default: run against repo migrations (red while DEF-01 is open)
bash test-case/evidence/team-qa/run-team-qa.sh --shim --ignored # renumbering assumed + pending-decision probes
```
Once DEF-01 is fixed, the default mode should produce the same results as `--shim`, and `--shim` can be retired.
