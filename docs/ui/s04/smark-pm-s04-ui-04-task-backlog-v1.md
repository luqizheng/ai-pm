# S04-UI-04 — Task Backlog

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S04 · Task Management & Planning（页面 4/7）
> - 关联：S04-00 总提示词、S04-UI-03 Task Detail、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-04 Task Backlog

## 1. Design Objective

Design the Task Backlog page: the central task inventory of a project.

It enables Project Managers and team members to browse, search, filter, assign, organize and bulk-manage engineering tasks, and to navigate to task details. It manages engineering **work**, not engineering **execution** — **S04 organizes; S05 executes.**

## 2. Global Layout

1440px desktop enterprise SaaS. Global frame identical to S04-UI-01 §2 (top nav / left project nav / right AI panel / bottom AI command bar). S04 secondary nav: Overview / Breakdown / **Backlog (selected)** / Execution Planning / Execution Readiness.

## 3. Page Header

- Title: **Task Backlog** · Subtitle: "Manage and organize all project engineering tasks."
- Summary chips: Total Tasks (128) · Unassigned (9) · Pending Review (12) · Blocked (4).
- Primary: **Create Task**. Secondary: Import Tasks · Generate Task Breakdown · Export.

## 4. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                    │
├──────────┬─────────────────────────────────────────────┬───────────────────┤
│ Project  │ Task Backlog (128 · 9 unassigned)           │ AI Project Lead   │
│ nav      │ [Create Task] [Import ▸][Generate ▸][Export]│ ───────────────── │
│          │ [Table|Grouped|Kanban|Tree] 🔍 [Filters ▾]  │ · Duplicate found │
│ Task Mgmt│ Saved: ⚑ Needs review · ⚑ This sprint       │ · Tasks w/o REQ   │
│  Overview├─────────────────────────────────────────────┤ · Missing AC      │
│  Breakdn │ ☑ ID    Title          Type Prio Plan  Appr │   [Review]        │
│  Backlog │ ☑ T-041 Login endpoint Dev  P1  PendR  —    │                   │
│  (here)  │ ☐ T-042 Login form     Feat P1  Draft  —    │                   │
│  ExecPlan│ ☐ T-043 Token valid.   Dev  P0  Approv ✓Rdy │                   │
│  Readiness│ ☐ T-045 Rate limit     Infr P2  Blocked —   │                   │
│          │ Bulk: 2 selected [Priority][Assign][Repo][Status][Export]       │
├──────────┴─────────────────────────────────────────────┴───────────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"                 │
└────────────────────────────────────────────────────────────────────────────┘
```

## 5. Search & Filtering

Search across: Task ID · title · description · Requirement ID · Design ID.
Filters: Task Status · Approval Status · Priority · Task Type · Assignee · Repository · Requirement · Technical Design · Implementation Baseline · Updated Date.
Support: Clear Filters · Save Filter · Saved Views. Active filters render as removable chips; filter state is URL-addressable.

## 6. View Switcher — four modes

- **A. Table (default)**: columns — Selection · Task ID · Task Name · Type · Priority · **Planning Status** · **Approval Status** · Assignee · Repository · Estimated Effort · Dependency Count · Updated At. Compact rows (36px), sticky header, sortable, column show/hide/reorder. Readiness marker (✓ Ready / ○ Not ready + missing-field tooltip) where relevant — planning status and execution readiness stay separate concepts.
- **B. Kanban**: columns by planning status — Draft / Pending Review / Approved / Ready for Execution / Blocked / Completed. Card: ID, title, priority, assignee, repository. Drag changes planning status only.
- **C. Hierarchical**: parent/subtask tree with expand-collapse (same model as Breakdown).
- **D. Grouped**: group by Priority / Assignee / Repository / Task Type / Status; collapsible headers with counts.

## 7. Bulk Operations

Multi-select → contextual bulk toolbar: Change Priority · Assign Owner · Change Repository · Change Planning Status · Add Labels · Export Selected.
Approval actions must list the tasks being approved; never silently change approved task specifications. All bulk actions confirmed and logged.

## 8. Task Row Interaction

Click → Task Detail (full page, S04-UI-03). Context menu per row: Open · Edit · Duplicate · Add Subtask · View Dependencies · Archive or Cancel. Requirement / Design references render as compact traceability chips.

## 9. AI Assistance Panel

Insight examples: identify duplicate tasks · tasks without linked requirements · tasks with incomplete acceptance criteria · unassigned tasks · inconsistent priorities · better grouping suggestions.
Each suggestion: explanation · affected tasks · recommended action · **[Review]**. AI never auto-modifies approved tasks.

## 10. Components
SummaryChip · ViewSwitcher · FilterBar + FilterChip · SavedViewMenu · DataTable (sticky/sortable/customizable) · StatusBadge (planning) · ApprovalBadge · ReadinessMarker · PriorityTag · TaskTypeTag · RepositoryChip · DependencyChip · TraceabilityLink · AssigneeAvatar · KanbanColumn/Card · BulkActionBar · Pagination · EmptyState / NoResultsState.

## 11. Interaction & States
Prototype: search → multi-filter → switch Table↔Kanban → select several → change priority → open Task Detail → save custom filter → review AI recommendation.
States: default 128 tasks · filtered/no results · empty project (EmptyState + [Import from Baseline]) · bulk bar active · Kanban drag blocked at "Ready for Execution" when readiness checks fail (reason popover) · loading skeletons.

## 12. Visual Style
V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF (light variant per token file). Typography 21/15/13/12/11, tabular numerals. High information density, clean table, restrained status colors, neutral Kanban columns (**no rainbow boards**), minimal decoration. No purple; AI-origin tasks carry the Info-tint "AI" badge.

## 13. Deliverable
High-fidelity desktop screens (Table primary) + interactive prototypes for all four view modes implementing §11.
