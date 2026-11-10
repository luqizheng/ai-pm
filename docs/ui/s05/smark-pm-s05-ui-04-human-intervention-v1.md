# S05-UI-04 — Human Intervention

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S05 · AI Agent Execution & Collaboration（页面 4/7，人工介入工作台）
> - 关联：S05-00 总提示词、S05-UI-01/02/03/05、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-04 Human Intervention

## 1. Design Objective

Design the Human Intervention workbench: the dedicated surface where humans handle everything an execution cannot decide by itself. Five scenarios must all be first-class:

1. **Execution Authorization** — approved task ready to run (covered by the focused flow S05-UI-05; here it appears as a queue item with a deep link).
2. **Agent Waiting for Clarification** — the agent asked a real question; the human answers with context; execution resumes as a **continuation of the same run** (not a new task, not a new attempt).
3. **Execution Blocked** — blocking reason, affected subtasks, suggested resolutions.
4. **Execution Failure** — failure summary, error context, failed stage, blast radius; decide Retry / Modify & Retry / Cancel / escalate.
5. **Permission Required** — exactly which permission, why, on which resource (never a generic write-permission toggle).

Semantics: a waiting task shows Task state `awaiting_decision` + **waiting reason** (five values: needs_info / needs_decision / needs_approval / external_dependency / no_executor_available) + the interaction lifecycle (**pending → responded / timed_out / cancelled → consumed**). The human reply is persisted and never overwritten; deadline semantics are explicit (a timed-out interaction creates a new interaction, it never silently resumes).

## 2. Global Layout

1440px desktop. Same global frame as S05-UI-01 §2. Secondary nav **Interventions (selected)**. Two zones: left = intervention queue; right/main = focused resolution workbench for the selected item. Deep-linked from Execution Center (attention rows), Collaboration nodes, Execution Detail banners.

## 3. Page Header

- Title: **Interventions** · Subtitle: "Resolve what agents cannot decide on their own."
- Queue summary chips: Clarifications (2) · Blocked (1) · Failures (1) · Permissions (1) · Authorizations (2).
- Filters: scenario · priority · age.

## 4. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar / project nav / AI Execution secondary nav                           │
├───────────┬───────────────────────────────────┬──────────────────────────────┤
│ Project   │ QUEUE                             │ WORKBENCH — Clarification     │
│ nav       │ ⚠ needs_info  T-041 Login  P1 2h │ T-041 Login API · Run#2 ·    │
│ AI Exec   │ ⛔ failed      T-038 Rate   P0 1h │ Test Agent @ dev-2 (ready)   │
│ Interv.   │ 🔑 auth        T-052 Bill   P1 —  │ ─────────────────────────    │
│ (here)    │ ⚠ needs_dec.  T-047 UI pa… P2 4h │ QUESTION (agent, verbatim):  │
│           │                                   │ "Should refresh tokens be    │
│           │ (row: scenario icon · reason ·    │  single-device or allow      │
│           │  task · Run# · age · priority)    │  concurrent sessions? The    │
│           │                                   │  design §5 is ambiguous."    │
│           │                                   │ CONTEXT: design DES-08 §5    │
│           │                                   │ excerpt · requirement REQ-12 │
│           │                                   │ acceptance criteria · code   │
│           │                                   │ refs the agent inspected     │
│           │                                   │ ─────────────────────────    │
│           │                                   │ YOUR ANSWER (persisted)      │
│           │                                   │ [textarea] [Attach context]  │
│           │                                   │ ☑ Resume same execution      │
│           │                                   │   (continuation of Run#2)    │
│           │                                   │ [Send Answer] [Cancel Exec]  │
├───────────┴───────────────────────────────────┴──────────────────────────────┤
│ AI command bar                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 5. Scenario Workbenches (right zone switches by selection)

- **Clarification (needs_info)**: agent's question verbatim · supporting evidence (design excerpt, requirement link, code refs the agent already inspected) · answer editor (persisted, auditable) · attach context · explicit note "answering resumes the same execution (continuation), deadline unchanged" · secondary **Cancel Execution** with confirm.
- **Failure (decision)**: failure summary · category (execution / infrastructure / timeout) · failed stage · error context (structured, expandable to raw) · impact list ("blocks Frontend T-060c, Test harness unaffected") · actions **[Retry]** (new Run attempt) · **[Modify & Retry]** (edit task instructions/scope first) · **[Cancel]** · **[Escalate]** (assign to another human with note).
- **Blocked (external_dependency / needs_decision)**: blocking reason · affected subtasks (from collaboration view) · suggested resolutions (with links: resolve ADR, complete predecessor, adjust scope) · **[Resolve]** or **[Reassign wait reason]**.
- **No execution environment (no_executor_available)**: the task is ready but no Device is available/ready — show available devices with their readiness chain, and offer to authorize an execution once one is ready (→05) or to adjust the agent assignment. No generic retry button that would loop.
- **Permission required**: exact permission name · why it is needed · affected resource (repository / path scope / device) · requester (agent + run) · **[Grant]** (scoped, logged, auditable) · **[Deny with reason]**. No global toggles.
- **Authorization**: compact card (see S05-UI-05) → **[Open Authorization Flow]**.

## 6. Queue Rows

Scenario icon (clarification / failure / blocked / permission / authorization) · waiting reason badge · task + title · Run chip · age (time waiting) · priority · assignee avatar-chip (human owner) · quick action. Sorted by priority then age. Selected row highlighted with accent border.

## 7. Right Context Panel

AI Project Lead per item: suggested answer draft (Info tint + AI badge, editable — human decides) · risk notes ("answering X narrows requirement scope; consider REQ update") · similar past decisions ("T-036 answered similarly — reused policy") · impact preview of each action.

## 8. Components

InterventionQueueRow (scenario icon + reason + age) · ClarificationWorkbench (question card + evidence + answer editor) · FailureWorkbench (summary + impact list + decision buttons) · BlockedWorkbench · PermissionRequestCard (resource-scoped) · ImpactList · AnswerDraftCard (AI) · DeadlineNote · ConfirmDialog.

## 9. Interaction & States

Prototype Journey B end-to-end: queue → select clarification → read question + evidence → (optional AI draft) → type answer → Send → row leaves queue with success note "Answer delivered — execution resuming (continuation)" → Execution Detail shows the agent resuming. Journey C: select failure → review impact → Retry → confirm → new Run attempt starts (old kept). States: empty queue ("Nothing needs you — executions are flowing") · deadline-expired item (re-authorization required badge, links to ACP-07 re-auth flow) · answered (transient state, auto-consumed) · denied permission (logged with reason) · human handoff escalated.

## 10. Visual Style

V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11. Question/evidence cards read as quoted material (bordered, elevated); answers read as human-authored records (accent border). **No purple, no gradients; nothing auto-sends** — every human action has explicit confirm where material.

## 11. Deliverable

Complete high-fidelity page (1440×1000) with all five scenario workbenches designed + connected prototype implementing §9.
