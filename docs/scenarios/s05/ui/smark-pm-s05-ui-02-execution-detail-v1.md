# S05-UI-02 — Execution Detail

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S05 · AI Agent Execution & Collaboration（页面 2/7，单任务执行审计）
> - 关联：S05-00 总提示词、S05-UI-01（入口）/03（并行视图）/04（介入）/06（交付审查）、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-02 Execution Detail

## 1. Design Objective

Design the Execution Detail page: the auditable record of one task's execution. A lead must be able to reconstruct **what was authorized, which agents did what, when each stage happened, what changed, and what was delivered** — without reading raw logs.

Core rule: execution detail must be readable and auditable; structured events are the default, raw logs are a tab. The page distinguishes **Task state** (lifecycle), **Run state** (this attempt, `Run #N`), and **Agent activity** (who is doing what now). A task with multiple runs shows an attempt selector — history is never overwritten.

## 2. Global Layout

1440px desktop. Same global frame as S05-UI-01 §2 (project nav, AI Execution secondary nav, right AI Project Lead panel, bottom AI command bar). Reached by clicking any task/run row in Execution Center, Collaboration or History.

## 3. Page Header

- Title: task title · ID chip (`T-043`) · attempt selector (**Run #3** current, #1 #2 collapsed with terminal badges).
- Status strip — three layers explicitly separated: Task `running` · Run `#3 running` · Agent `Backend Agent @ dev-device-2 (ready)`.
- Meta chips: requirement link (`REQ-12`) · design link (`DES-08` / ADR) · repository + branch (`core-api @ task/52b91e8f`) · execution scope · environment (Device + workspace) · started 14:02 · elapsed 41m · authorized by (human, time).
- Actions (contextual): **Intervene** (opens 04) · **Cancel Execution** (guarded confirm) · **Review Delivery** (enabled when delivery submitted; → 06).

## 4. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar / project nav / AI Execution secondary nav                           │
├───────────┬──────────────────────────────────────────────┬───────────────────┤
│ Project   │ T-043 Token service  T-043  Run#3 ▾(#1✗ #2✗) │ AI Project Lead   │
│ nav       │ Task running · Run running · Backend@dev ✓   │ ───────────────── │
│ AI Exec   │ REQ-12 · DES-08 · core-api@task/52b91e8f     │ · matches design  │
│ Overview  │ [Intervene] [Cancel]         [Review Delivery]│   §4 auth flow    │
│ (Detail)  ├──────────────────────────────────────────────┤ · no blocker on   │
│           │ Tabs: Overview│Timeline│Agent Activity│Logs│ │   dependencies    │
│           │       Changes│Deliverables                   │                   │
│           │ TIMELINE (default tab)                       │                   │
│           │ ●14:02 Task authorized         human         │                   │
│           │ ●14:03 Run#3 claimed           platform      │                   │
│           │ ●14:03 Context loaded          platform      │                   │
│           │ ●14:04 Agent started working   Backend Agent │                   │
│           │ ●14:20 Stage → implementing    agent         │                   │
│           │ ●15:11 Code changes pushed     agent         │                   │
│           │ ●15:31 Tests executed ✓        agent         │                   │
│           │ ●15:32 Delivery submitted      agent         │                   │
│           │  each: timestamp + source (human/platform/agent)                  │
│           ├──────────────────────────────────────────────┤                   │
│           │ CURRENT ACTIVITY: implementing refresh flow  │                   │
│           │ progress ▓▓▓░ 70% · last heartbeat 2m ago    │                   │
├───────────┴──────────────────────────────────────────────┴───────────────────┤
│ AI command bar                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 5. Tabs

- **Overview** (default): status strip + header meta + current activity + last events + delivery/review snapshot.
- **Timeline**: full event list (see §6).
- **Agent Activity**: current and past agents on this run (see §7).
- **Logs**: execution logs with severity filter, search, agent filter, timestamps; collapsible raw view; **never the default tab**; empty state "Logs are written by the executor — none yet".
- **Changes**: per-repository changed files and commits (link to 06 for review).
- **Deliverables**: delivery checklist + verification state (link to 06).

## 6. Execution Timeline

Structured events, each: timestamp · event type · summary · **source** (human / platform / agent) · optional reference (commit SHA, command id, waiting reason). Event vocabulary mirrors the execution event log (eight persisted types): `run_claimed` / `run_started` / `run_stage_changed` / `run_finished` / `command_issued` / `command_delivered` / `command_acked` / `command_failed` — plus platform-side milestones that are not events but must appear (task authorized, waiting raised, waiting answered, delivery submitted, review decided). Waiting raised/answered maps to the interaction lifecycle **pending → responded / timed_out / cancelled → consumed**; a command event names its type (`agent.process.terminate` / `agent.task.continue`). Terminal events styled with outcome color; retried runs show a "new attempt" marker linking back to the previous run (history preserved).

## 7. Current Agent Activity

Card per agent on the run: agent name + role (Architecture / Backend / Frontend / Test / Review) · Device carrier + readiness dot · current activity text · current subtask · progress · status (working / waiting for human / idle) · last heartbeat. A waiting agent shows the actual question inline with **[Answer]** (→ 04). Multiple agents → group by agent with parallel lanes.

## 8. Right Context Panel — Task Context

- Original requirement (linked, with version).
- Acceptance criteria (each with verification state from evidence: unverified / satisfied / failed).
- Technical constraints + related design/ADR (linked, versions pinned).
- Related knowledge references.
- Authorized execution scope (repositories, write access, path scope — from the authorization snapshot).
- AI Project Lead digest: risks, deviations from design, suggested actions.

## 9. Attempt History (multi-run)

Attempt selector in header: `Run #1 failed (timeout)` · `Run #2 failed (exit_code)` · `Run #3 running`. Selecting a past run switches Timeline/Logs/Changes to that attempt, with a persistent banner "Viewing historical run — read-only". Delivery review always anchors to the delivered run.

## 10. Components

StatusStrip (three-layer) · AttemptSelector · EventTimelineItem (source icon) · AgentActivityCard · WaitingQuestionInline · LogViewer (filters) · ChangesList · CriteriaEvidenceList · ContextCard · RunChip · DeviceDot.

## 11. Interaction & States

Prototype: open from Execution Center → switch attempts → answer a waiting question inline → jump to Changes → open Delivery Review. States: running · waiting (warning banner + inline question) · failed (danger header + retry actions → 04) · completed-awaiting-review (CTA "Review Delivery") · accepted (restrained success + read-only) · returned-for-changes (shows feedback and the new attempt link) · device lost (agent card shows readiness failure).

## 12. Visual Style

V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11, tabular numerals for timestamps/SHAs. Timeline uses a thin neutral rail with colored event dots; logs monospace at Micro size inside a bordered surface. **No purple, no gradients; logs never become the landing view.**

## 13. Deliverable

Complete high-fidelity page (1440×1000) with all six tabs designed + connected prototype implementing §11.
