# S05-UI-06 — Delivery Review

> - 记录日期：2026-11-10
> - 状态：已落库，待执行（重点迭代页 ③，第二轮单独深化交互）
> - 领域：S05 · AI Agent Execution & Collaboration（页面 6/7，关键业务闭环）
> - 关联：S05-00 总提示词、S05-UI-01/02/05、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-06 Delivery Review

## 1. Design Objective

Design the Delivery Review page: the evidence-based review interface where a human decides whether to accept what an agent delivered.

Core rule: **agent execution completion ≠ project task acceptance.** The page must let a reviewer inspect commits, test evidence, the execution report and the acceptance checklist **before** deciding. The review verdict (approved / changes_requested / inconclusive) and the human acceptance are distinct outcomes, and the UI shows them distinctly.

The reviewer should be able to answer, without leaving the page: what changed, was it tested, does it meet the acceptance criteria, and what remains unresolved.

## 2. Global Layout

1440px desktop. Same global frame as S05-UI-01 §2. Secondary nav **Deliveries (selected)**. Full-width main column + right AI Project Lead panel. Reached from Execution Center "Recent Deliveries" → [Review], Execution Detail → [Review Delivery], and History.

## 3. Page Header

- Title: **Delivery Review** · Subtitle: "Inspect the evidence, then decide."
- Context line: task title + ID · Run chip (`Run #3`) · agent(s) · delivery submitted time · execution duration · final run status.
- Stage strip (three steps, explicitly separated): ① **Execution completed** (✓ run succeeded) → ② **AI-PM verification** (manifest reconciliation: passed / pending) → ③ **Human acceptance** (current, pending). The reviewer is at step ③.
- Actions: **Accept Delivery** (primary, guarded) · **Request Changes** · **Reject Delivery**.

## 4. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar / project nav / AI Execution secondary nav                           │
├───────────┬──────────────────────────────────────────────┬───────────────────┤
│ Project   │ Delivery Review · T-043 Token service          │ AI Project Lead   │
│ nav       │ ① Execution completed ✓  ② AI-PM check ✓  ③ Human pending      │
│ AI Exec   │ Run#3 · Backend Agent · 41m · succeeded        │ ───────────────── │
│ Deliveries├──────────────────────────────────────────────┤ · AC 2/3 covered  │
│ (here)    │ A. EXECUTION SUMMARY                          │ · AC-3 no test    │
│           │  agent Backend@dev-2 · device ✓ready           │   evidence yet   │
│           │  commits 3 · files 7 · tests 12 ✓ / 1 ✗       │ · suggest require │
│           │  scope honored: core-api crates/auth/** ✓     │   test evidence  │
│           ├──────────────────────────────────────────────┤   before accept   │
│           │ B. CODE CHANGES                               │                   │
│           │  core-api @ task/52b91e8f                     │                   │
│           │  a1f3c9d  feat(auth): refresh token rotation  │                   │
│           │    7 files · +412 −96 · [View diff] [Open]    │                   │
│           │  7b2e104  fix(auth): null-guard refresh path  │                   │
│           │  9c1a55b  test(auth): add rotation expiry case │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ C. TEST EVIDENCE                              │                   │
│           │  package:auth  ✓ 12 passed  8.4s   [logs]     │                   │
│           │  package:api  ✗ 1 failed  (e2e-refresh)      │                   │
│           │    failure: expected 200, got 401 · [logs]    │                   │
│           │  coverage  auth 87% (+2.1%)   [report]        │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ D. EXECUTION REPORT (agent-authored)          │                   │
│           │  Completed · Decisions (rotate-on-refresh)    │                   │
│           │  Technical changes · Known limitations (e2e   │                   │
│           │  not run in CI — needs manual) · Unresolved   │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ E. DELIVERY CHECKLIST                         │                   │
│           │  ☑ Acceptance criteria 1 — token refresh API  │                   │
│           │  ☑ Acceptance criteria 2 — rotation ≤15min    │                   │
│           │  ⛔ Acceptance criteria 3 — e2e regression     │                   │
│           │  ☑ Implementation scope  ☑ Code changes        │                   │
│           │  ⚠ Test evidence (1 failed)  ☑ Docs  ⚠ Risks  │                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ [Reject Delivery]  [Request Changes…]  [Accept Delivery]        │
│           │  Request Changes → feedback editor + scope of rework            │
└───────────┴──────────────────────────────────────────────┴───────────────────┘
```

## 5. Section A — Execution Summary

Task · Run (attempt number, final status, duration) · agents and devices involved · workspace/branch · scope adherence indicator (authorized scope vs actual changes — e.g. "all changes inside crates/auth/**" or a warning for out-of-scope paths) · commit/file/test counts.

## 6. Section B — Code Changes

Per repository: branch · commit list (SHA short + message + author agent + files changed + +/- counts) · changed-file list with per-file diff preview (condensed, expandable) · link to open in repository. Commit SHAs are monospace, tabular.

## 7. Section C — Test Evidence

Per test suite: command · passed/failed counts · duration · status badge · **test logs (expandable, not the default view)** · coverage when available (before/after delta). Failed tests are listed first with the failure reason (expected vs actual) and a link to the failing run's logs. Absence of evidence is stated explicitly ("no test evidence submitted") — never implied success.

## 8. Section D — Execution Report

Agent-authored report, marked with an **AI** badge and Info tint: work completed · implementation decisions (with rationale) · technical changes · known limitations · unresolved issues. Read-only to the reviewer here; reviewer feedback goes through §Request Changes.

## 9. Section E — Delivery Checklist

Explicit list with per-item state: acceptance criteria (each mapped to its verification evidence: satisfied / failed / no evidence) · implementation scope adherence · code changes present · test evidence present · documentation changes · outstanding risks. Each failing or unevidenced row explains what is missing and links to the relevant section.

## 10. Human Actions

- **Accept Delivery** — enabled only when no blocking checklist item is failed; confirm dialog summarizes what is being accepted and which evidence supports it; on accept: Task moves to completed (delivery state updated), requirement/knowledge status update is queued by PM-AI (not by the agent).
- **Request Changes** — requires feedback text and optionally selects affected acceptance criteria; the review verdict becomes `changes_requested` (one of exactly three verdict values) and the task returns for further execution; the new attempt appears in history while the previous attempt and its review stay intact. Feedback history is retained.
- **Reject Delivery** — requires reason; distinct from Request Changes: the task is not continued, it is terminated by human decision (task ends in a terminal state, not `changes_requested`). The review verdict column shows `changes_requested` / `approved` / `inconclusive` — a rejection is a human task decision, not a fourth verdict value; label it accordingly (e.g. "Rejected by reviewer" on the attempt row).

## 11. Right Context Panel

AI Project Lead: coverage of acceptance criteria · deviation from design · risk assessment · suggestion of what to request (AI recommends, human decides) · links to requirement/design/knowledge references.

## 12. Components

StageStrip (three steps) · ExecutionSummaryCard · CommitRow (sha + message + stats) · FileDiffRow · TestSuiteCard · TestLogViewer · CoverageDelta · AgentReportCard (AI badge) · ChecklistItem (3 states) · EvidenceLink · ReviewActionBar · RequestChangesDialog (feedback editor) · AcceptConfirmDialog · RejectDialog.

## 13. Interaction & States

Prototype Journey A: open from Execution Center → scroll evidence sections → expand one failing test's logs → open Accept dialog → confirm → success state (restrained, verdict `approved`, task completed). Journey D: on a delivery with a failed criterion → Request Changes → write feedback → confirm → state becomes `changes_requested`, feedback shown, link to the new attempt once it starts. States: all-passed (clean review) · failed-evidence (danger on the failing rows, accept disabled with reason) · no-evidence (neutral warning "evidence missing", accept disabled) · already-reviewed (read-only with verdict and reviewer + time) · inconclusive verdict.

## 14. Visual Style

V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11, tabular numerals for SHAs/counts/durations. Evidence sections read like a technical audit: monospace SHAs, hairline dividers, restrained badges. **No purple, no gradients; no celebratory green floods** — a clean review still looks like a professional document, and the accept button is the only strong accent.

## 15. Deliverable

Complete high-fidelity page (1440×1000) + connected prototype implementing §13 (focus page for second-round interaction iteration).
