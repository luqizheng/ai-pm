# S04-UI-01 — Task Overview

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S04 · Task Management & Planning（页面 1/7）
> - 关联：S04-00 总提示词、S02-UI-01 Requirements Workspace、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-01 Task Overview

## 1. Design Objective

Design the Task Overview page for an enterprise AI R&D Project Management Platform.

Module: **S04 — Task Management & Planning**. This is the landing page for project-level engineering task management.

The page must let a Project Lead understand at a glance:
- What engineering tasks exist and how much work is planned?
- What needs human attention right now?
- What is blocked, and what is ready for engineering execution?
- What does the AI Project Lead recommend for the plan?

Hard boundary: **S04 organizes and prepares engineering work; S05 executes it.** Never show code, commits, agent runs, CI or test execution on this page.

## 2. Global Workspace Frame (identical across all S04 screens)

1440px desktop, dark theme. Layout:

- **Top bar 48px**: product logo · team selector · project selector · global search · notifications · user avatar.
- **Left project nav 220px**: Project Command Center / Requirements / Technical Design / **Task Management (selected)** / Knowledge / Project Settings.
- **S04 secondary nav**: Overview (**selected**) / Breakdown / Backlog / Execution Planning / Execution Readiness.
- **Main content**: this screen's working surface.
- **Right context panel 320px** (collapsible): "AI Project Lead" — planning context (approved Implementation Baseline + related requirements/designs), AI suggestions, planning risks, pending human review actions.
- **Bottom AI command bar**: persistent natural-language input, placeholder "Ask AI Project Lead about task planning…", with a context chip of the current scope.

## 3. Page Header

- Breadcrumb: Project / Task Management / Overview.
- Title: **Task Overview** (21px) · Subtitle: "Organize, review and prepare engineering work."
- Primary action: **Create Task**. Secondary: **Generate Task Breakdown** (navigates to Breakdown).

## 4. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                    │
├──────────┬─────────────────────────────────────────────┬───────────────────┤
│ Project  │ Breadcrumb                                  │ AI Project Lead   │
│ nav      │ Task Overview   [Create Task][Generate ▸]   │ ───────────────── │
│          ├─────────────────────────────────────────────┤ · Planning context│
│ Task Mgmt│ [Total 128][Pending Review 12][Blocked 4]   │ · AI suggestions  │
│  Overview│ [Ready 31][Unplanned 9]   ← metric cards    │ · Risks           │
│  Breakdn │                                             │ · Review actions  │
│  Backlog │ Human Attention Required [All|Critical|     │                   │
│  ExecPlan│                           Needs Review|Blkd]│                   │
│  Readiness│ ◦ AI tasks awaiting approval (12)   [Review]│                  │
│          │ ◦ Missing acceptance criteria (5)   [Review]│                   │
│          ├───────────────────────────┬─────────────────┤                   │
│          │ Task Planning Overview    │ Upcoming Tasks  │                   │
│          │ status ▓▓▓░░ priority ▓▓░ │ table rows …    │                   │
│          ├───────────────────────────┴─────────────────┤                   │
│          │ AI Planning Insights  [Accept][Reject][View]│                   │
├──────────┴─────────────────────────────────────────────┴───────────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"                 │
└────────────────────────────────────────────────────────────────────────────┘
```

## 5. Section Specifications

### 5.1 Planning Status Summary
Five compact metric cards in one row (not oversized dashboard tiles):
- **Total Tasks · Pending Human Review · Blocked Tasks · Ready for Execution · Unplanned Work**.

Each card: metric label (caption 12px), count (32px stat, tabular numerals), delta hint ("+3 since last review"), one-line helper ("tasks missing acceptance criteria"). Every card is clickable → Backlog pre-filtered accordingly.

### 5.2 Human Attention Required — the most important section
Filter chips: All / Critical / Needs Review / Blocked. Item categories:
- AI-generated tasks awaiting approval (count badge)
- Tasks missing acceptance criteria
- Tasks with unresolved dependencies
- Tasks without repository assignment
- Conflicting or duplicated tasks
- Tasks requiring clarification

Item anatomy: category icon · issue title · related task link (TASK-041) · priority tag · why attention is needed (one line) · recommended action · **[Review]** button. Support bulk selection when the fix is identical.

### 5.3 Task Planning Overview
Two compact distributions side by side, clickable segments → filtered Backlog:
- By planning status: Draft / Pending Review / Approved / Ready for Execution / Blocked / Completed / Cancelled (segmented bar + legend).
- By priority: P0 Critical / P1 High / P2 Medium / P3 Low.

Caption under the card: **"Planning readiness — not engineering execution progress."**

### 5.4 Upcoming Tasks
Table columns: Task ID · Task Name · Priority · Status · Assignee · Repository · Planned Date · Related Requirement.
Sortable headers, 8–10 rows, row click → Task Detail.

### 5.5 AI Planning Insights
Recommendation cards, each with: statement, explanation, source reference (e.g. BASE-v1.2 §4.2), impact, actions **[Accept] [Reject] [Review Details]**. Render these examples:
- "Three tasks may be missing from the approved implementation baseline."
- "Two tasks can be executed independently."
- "One task has an unresolved dependency."
- "Acceptance criteria are incomplete for two tasks."

AI suggestions must never silently modify approved tasks.

## 6. Components
MetricCard (5 variants) · AttentionListItem (6 category variants) · StatusBadge (7 planning statuses; Ready uses status-ready color #8DC5AF) · PriorityTag (P0–P3) · TaskTypeTag · RepositoryChip · TraceabilityLink (REQ / DES / ADR / BASE chips) · AIProposalCard (reason / source / impact / Accept-Reject-Edit) · SegmentedDistribution (clickable) · DataTable · FilterChipRow · EmptyState.

## 7. Interaction & States
1. Metric card click → Backlog with matching filter.
2. Task row / task link click → Task Detail (full page, S04-UI-03).
3. Generate Task Breakdown → Breakdown page.
4. AI insight: [Accept] → inline confirm, collapses to "Applied"; [Reject] → optional reason, state "Rejected"; [Review Details] → expands in place.
5. Attention row [Review] → Task Detail with offending field highlighted.
6. Secondary nav: Execution Readiness reachable in one click.

Render these states: default; one metric card at 0; attention list filtered to Critical; AI insight in Accepted state; empty Upcoming Tasks (first-run project) with EmptyState + [Generate Task Breakdown].

## 8. Visual Style
V2 "Slate & Muted Teal" tokens (single source of truth `pm-ai-design-tokens-v2.css`):

| Role | Dark (default) | Light variant |
|---|---|---|
| Canvas | #171C24 | #F5F8FA |
| Sidebar | #1D2631 | #EDF2F5 |
| Surface (cards/tables) | #222A34 | #FFFFFF |
| Elevated (badges/hover/nested) | #2B3541 | #EAF0F3 |
| Border | #374452 | #DCE4E9 |
| Text primary / secondary | #E7EDF2 / #A8BBC9 | per token file |
| Accent | #75A9A4 (hover #8AB9B4, pressed #63948F) | same hue |
| Info / Success / Warning / Danger | #6EA8FE / #34C08B / #E0B15E / #F07E88 | per token file |
| Status Ready | #8DC5AF | #3F8F70 |

Typography: Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11. **No purple, no gradients** — AI content = Info-blue tint + "AI" badge. The page should feel like a serious engineering control center, not a business analytics dashboard.

## 9. Deliverable
High-fidelity desktop screen (1440×900) + connected interactive prototype implementing §7.
