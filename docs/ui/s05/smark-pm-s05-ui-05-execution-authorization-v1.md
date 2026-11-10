# S05-UI-05 — Execution Authorization

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S05 · AI Agent Execution & Collaboration（页面 5/7，S04→S05 受控移交的接收端）
> - 关联：S05-00 总提示词、S05-UI-01/04/06、S04-UI-07 Execution Readiness（上游出口）、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-05 Execution Authorization

## 1. Design Objective

Design the Execution Authorization flow: the controlled gate where a human authorizes one approved task to actually run.

It is the receiving end of S04's "Prepare S05 Handoff" package. The page must let a reviewer verify **what will run, where, with which agent, under what permissions, and what could go wrong** — then authorize, reject, or narrow the scope.

Core rule: authorization is **task-level and one-time**. It is not a global permission switch and not a per-step approval. The Run-level deadline re-authorization (ACP-07) is a different mechanism and must not appear here.

## 2. Global Layout

1440px desktop. Same global frame as S05-UI-01 §2. This is a **focused flow page** (full-width main column, right AI Project Lead panel retained). Reached from Execution Center "Authorize Execution" / attention row, and from S04 Readiness handoff action.

## 3. Page Header

- Title: **Authorize Execution** · Subtitle: "Confirm scope and permissions before an agent starts engineering work."
- Context line: task title + ID · requirement/design links · handed-off plan version (e.g. "Handoff package v3").
- Actions: **Authorize Execution** (primary, guarded) · **Reject** · **Modify Execution Scope**.

## 4. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar / project nav                                                        │
├───────────┬──────────────────────────────────────────────┬───────────────────┤
│ Project   │ Authorize Execution · T-052 Billing export   │ AI Project Lead   │
│ nav       │ Handoff package v3 · handed over 2026-11-10  │ ───────────────── │
│ AI Exec   ├──────────────────────────────────────────────┤ · scope matches   │
│ Authorize │ ① ELIGIBILITY (5/5 checks passed)            │   plan v3         │
│ (here)    │  ✓ task ready      ✓ plan approved           │ · dev-2 is the    │
│           │  ✓ deps completed  ✓ acceptance criteria set  │   only ready      │
│           │  ✓ repo binding valid                         │   device          │
│           ├──────────────────────────────────────────────┤ · no conflict     │
│           │ ② EXECUTION SCOPE                             │   with T-047      │
│           │  Repositories  core-api   RW  path crates/**  │                   │
│           │  Branch       task/52b91e8f (platform-generated)│                  │
│           │  Agent        Backend Agent @ dev-device-2     │                   │
│           │  Mode         MCP (agent self-executes)       │                   │
│           │  Timeout      1800s · deadline 3600s          │                   │
│           │  [Edit scope] (per-repo write access + path)  │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ ③ ENVIRONMENT READINESS                       │                   │
│           │  dev-device-2  ✓ ready (installed/connected/  │                   │
│           │                authenticated/ready) 2m ago    │                   │
│           │  dev-device-1  ⚠ degraded — auth expired     │                   │
│           │  (agent assigned to dev-2 only)              │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ ④ PERMISSIONS REQUESTED                       │                   │
│           │  scope: write core-api (crates/billing/**)   │                   │
│           │  scope: read  core-api (whole repo)           │                   │
│           │  scope: run tests (package:billing)           │                   │
│           │  (resource-scoped list — no generic toggles) │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ ⑤ RISKS & PRECONDITIONS                      │                   │
│           │  ⚠ branch task/52b91e8f already exists       │                   │
│           │  · T-047 runs in same repo (queue after it)   │                   │
│           │  · 2 acceptance criteria depend on T-038 fix │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │  [Reject]  [Modify Scope]   [Authorize Execution]                 │
│           │  ← authorize records: who · when · scope snapshot               │
├───────────┴──────────────────────────────────────────────┴───────────────────┤
│ AI command bar                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 5. Eligibility Gate (§①)

Five checks, each with pass/fail and a link to the evidence: task ready · source plan approved · predecessor dependencies completed · acceptance criteria present · repository binding valid. **Authorize is disabled when any check fails**, and the disabled state lists the failing checks with links (never a silent disabled button).

## 6. Execution Scope (§②)

- Repositories: each with name · read/write access · path scope · base branch/commit snapshot.
- Branch: platform-generated task branch (read-only label — the platform owns branch naming).
- Agent: proposed agent + role + Device carrier.
- Execution mode (two real values, shown with plain labels): **human** = the agent executes the work itself in its own process (MCP self-execution; PM-AI only coordinates) · **spawn** = the platform launches the agent process and supervises it.
- Timeouts: execution timeout and absolute deadline, with the note that re-claim does not extend the deadline.
- **Modify Execution Scope**: narrow write access / path scope / repository set / timeout. Changes are previewed as a diff against the handed-off scope, and require confirm.

## 7. Environment Readiness (§③)

Per Device: readiness chain (installed → connected → authenticated → ready / degraded / failed) · last check time · agent assignment compatibility. Devices that are not ready are shown but **cannot be silently selected** — choosing one requires explicit override with a reason, or the authorization is blocked. This is the "device unavailable" state.

## 8. Permissions Requested (§④)

Resource-scoped list, never a generic toggle: for each permission — scope (repo + path / command family), why it is needed (linked evidence), who requested it. Human grants per scope; denial requires a reason.

## 9. Risks & Preconditions (§⑤)

Warnings that do not block but should be acknowledged: existing task branch, concurrent runs in the same repository, acceptance criteria depending on an unfinished fix, missing device for a proposed agent. Each with severity and a suggested mitigation.

## 10. Right Context Panel

AI Project Lead: scope-vs-plan consistency check · agent suitability (role vs task type) · risk summary · suggested scope adjustments · note on expected duration. AI may recommend; only the human authorizes.

## 11. Components

EligibilityChecklist (5, clickable evidence) · ScopeCard (repos/branch/agent/mode/timeouts) · ScopeDiffView · DeviceReadinessRow (6-state chain) · PermissionScopeRow · RiskRow (severity) · AuthorizeConfirmDialog (summary + snapshot hash + who/when) · RejectDialog (reason required) · ModifyScopeDrawer.

## 12. Interaction & States

Prototype: open from Execution Center → review eligibility → inspect device readiness → narrow scope (modify) → confirm diff → Authorize → success state ("Execution authorized — Run queued") with link to Execution Center. Branch states: eligibility failed (authorize disabled + reasons) · no ready device (blocked + override path with reason) · reject with reason (task returns to queue, audit recorded) · modify scope (diff confirm) · authorization already exists (idempotent state).

## 13. Visual Style

V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11, tabular numerals for SHAs and timeouts. Checklist gates use restrained ✓/⚠/⛔ markers, not oversized badges. **No purple, no gradients; the authorize button carries the only strong accent** — the page's job is deliberation, not conversion.

## 14. Deliverable

Complete high-fidelity page (1440×1000) + connected prototype implementing §12.
