# S05-UI-01 — Execution Center

> - 记录日期：2026-11-10
> - 状态：已落库，待执行（重点迭代页 ①，第二轮单独深化交互）
> - 领域：S05 · AI Agent Execution & Collaboration（页面 1/7，模块日常入口）
> - 关联：S05-00 总提示词、S05-UI-02/04/05/06、S04-UI-07 Execution Readiness（上游出口）、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-01 Execution Center

## 1. Design Objective

Design the Execution Center: the primary landing page of the **AI Execution** module. It answers, at a glance: what needs ME right now, what is running, what is queued, what has been delivered, and what recently happened.

It is NOT a KPI dashboard. Human attention comes first, then active execution, then delivery review. Metrics are secondary compact context.

Upstream context: tasks arrive here from S04 "Execution Readiness" handoff and wait in the **authorization** queue until a human authorizes them.

## 2. Global Layout

1440px desktop. Global frame: top navigation; left project nav (Project Command Center / Product · Requirements / Product Cognition / Project Cognition / Design / Tasks / **AI Execution (selected)** / Knowledge Base / Project Settings); secondary nav **Overview (selected) / Collaboration / Interventions / Deliveries / History**; right AI Project Lead panel 320px collapsible; bottom AI command bar.

## 3. Page Header

- Title: **AI Execution** · Subtitle: "Monitor agent activities, manage execution tasks and review deliveries."
- Context chips: Active Runs (6) · Awaiting Human (4) · Devices Ready (2/3).
- Primary actions: **Authorize Execution** (enabled only when eligible tasks exist; disabled shows "No eligible tasks — complete S04 readiness") · **View Execution History** (secondary).

## 4. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                      │
├───────────┬──────────────────────────────────────────────┬───────────────────┤
│ Project   │ AI Execution   [Authorize Execution][History]│ AI Project Lead   │
│ nav       ├──────────────────────────────────────────────┤ ───────────────── │
│           │ Overview Collaboration Interventions Deliveries History        │
│ Tasks     │ ⚠ HUMAN ATTENTION REQUIRED (4)          [View all]             │
│ AI Exec   │ ┌ ⚠ Awaiting clarification  T-041 Login API  Test Agent      │
│ Overview  │ │   needs_info · waiting 2h   [Answer]      P1                │
│ (here)    │ ├ ⛔ Failed execution       T-038 Rate limit Backend Agent   │
│           │ │   exit_code · Run #2       [Decide]       P0                │
│           │ ├ 🔑 Authorization required T-052 Billing export — P1        │
│           │ │   ready, 5/5 checks passed [Authorize]                     │
│           │ └ 👁 Delivery awaiting review T-043 Token service — P0       │
│           │    verdict pending          [Review]                          │
│           ├──────────────────────────────────────────────┤ · T-041 question  │
│           │ ▶ CURRENTLY EXECUTING (3)                                   │
│           │ T-043 Token service   Run #3 running  Backend Agent  ▓▓▓░ 70%│
│           │   current: implementing refresh flow · started 40m · core-api│
│           │ T-047 Audit log UI    Run #1 running  Frontend Agent ▓▓░░ 55%│
│           │ T-049 E2E harness     Run #2 running  Test Agent     ▓░░░ 25%│
│           ├──────────────────────────────────────────────┤                   │
│           │ ⏸ EXECUTION QUEUE (2)                                        │
│           │ T-052 Billing export  ready · awaiting authorization          │
│           │ T-053 Cleanup job      ready · blocked by T-038 failure       │
│           ├──────────────────────────────────────────────┤                   │
│           │ ✦ RECENT DELIVERIES (2)                                      │
│           │ T-043 Token service  submitted · AI-PM check passed · [Review]│
│           │ T-036 Metrics cache  changes_requested · [View review]       │
│           ├──────────────────────────────────────────────┤                   │
│           │ ◷ ACTIVITY TIMELINE (compact, today)                         │
│           │ 14:32 T-043 Run#3 stage→testing · 14:01 terminate acked · …  │
├───────────┴──────────────────────────────────────────────┴───────────────────┤
│ AI command bar: "Ask about executions, agents or deliveries…"                │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 5. Section A — Human Attention Required

Prominent section at top, warning-tinted container. One row per item needing a human:

- Items: execution authorization required · agent waiting for clarification · execution blocked by missing dependencies · failed execution requiring a decision · delivery awaiting review · Run-level deadline expired needing re-authorization.
- Row: task ID + title · priority (P0–P3) · state badges (**Task state ≠ Run state ≠ waiting reason** — show e.g. Task `awaiting_decision` + Run `#2 failed` + reason `needs_info`) · related agent · last updated · **action button** (Authorize / Answer / Decide / Review).
- Waiting reason badge takes one of five values: needs_info / needs_decision / needs_approval / external_dependency / no_executor_available — each with its own micro-copy so the human knows what kind of input is expected.
- Sort: priority, then age. Click row → Execution Detail; action button → the corresponding focused flow (05/04/06).

## 6. Section B — Currently Executing

Rows for running executions: task ID + title · Run chip (`Run #N`) with progress bar · agent or agent group (name + role + device readiness dot) · current activity (latest `run_stage_changed` event text) · started time / elapsed · repository + branch. Click → Execution Detail (02).

## 7. Section C — Execution Queue

Tasks waiting: authorization pending (link to 05) · queued run waiting for an available execution environment (reason `no_executor_available` — show which device is missing or not ready) · blocked by dependency/failure (show blocking reference). Empty queue state: neutral explanation, not an error.

## 8. Section D — Recent Deliveries

Completed executions: task · Run # · delivery submitted time · **AI-PM verification state** (manifest reconciliation: passed / pending / failed) · review verdict (approved / changes_requested / pending) · **[Review]**. Make the three-stage separation visible: submitted ≠ verified ≠ accepted.

## 9. Section E — Execution Activity Timeline

Compact chronological feed of important events (from the execution event log): task authorized · run claimed / started · stage changed · waiting raised / answered · command issued/acked · run finished · delivery submitted · review decided. Each line: time · event · task/Run reference · source (agent / platform / human). Filter by type; link to detail. Never raw logs here.

## 10. Empty & Edge States

No executions at all → "No execution tasks yet. Complete S04 readiness to hand work over." with a link to S04 Readiness. All-clear state (nothing needing attention) → subtle success line "Nothing needs your attention", sections remain visible.

## 11. Components

AttentionRow (type icon + priority + dual status badges + action) · ExecutingRow (progress + agent group + activity) · QueueRow (blocked reason) · DeliveryRow (verification + verdict badges) · ActivityTimelineItem · StatusBadge(Task/Run/Waiting-reason) · AgentChip(name+role+device dot) · SummaryChips · SectionHeader(with count).

## 12. Interaction & States

Prototype journey: land on page → authorize T-052 (→05) → answer T-041 question (→04) → click running T-043 (→02) → review delivery T-043 (→06). States: authorizations empty (primary disabled + reason) · attention section collapsed/expanded · device not ready (queue row shows readiness) · all-clear · timeline filters.

## 13. Visual Style

V2 "Slate & Muted Teal" (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11, tabular numerals. Attention section uses warning tint sparingly; progress bars thin and neutral; **no purple, no gradients, no decorative agent avatars**.

## 14. Deliverable

Complete high-fidelity page (1440×1000) + connected prototype implementing §12.
