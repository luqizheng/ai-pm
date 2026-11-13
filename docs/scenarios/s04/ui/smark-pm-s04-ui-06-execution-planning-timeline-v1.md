# S04-UI-06 — Execution Planning · Timeline

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S04 · Task Management & Planning（页面 6/7）
> - 关联：S04-00 总提示词、S04-UI-05 Dependency Graph（同属 Execution Planning）、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-06 Execution Planning · Timeline

## 1. Design Objective

Design a dedicated Execution Timeline for S04 — Task Management & Planning: a time-oriented visualization of the engineering task plan.

It answers: when tasks are planned · what must happen first · what can run in parallel · how dependencies affect scheduling · how work distributes over time · which milestones approach.

**This is a planning and scheduling interface, not an execution monitoring page.** Estimated planning dates must be visually distinct from actual execution dates (execution dates belong to S05).

## 2. Global Layout

1440px desktop. Global frame identical to S04-UI-01 §2. Execution Planning view sub-switcher: Dependency Graph / **Timeline (selected)** / Parallel Groups. Main timeline workspace gets most of the width; right AI Project Lead panel compact.

## 3. Page Header

- Title: **Execution Planning — Timeline** · Subtitle: "Visualize task schedules, dependencies and engineering workload."
- Metrics: Planning Version (v3) · Last Updated · Total Scheduled Tasks (44) · Unscheduled Tasks (3) · Critical Dependencies (5) · Planned Milestones (4).
- Actions: **Save Plan** · Validate Schedule · Optimize Schedule · Compare Versions.

## 4. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                    │
├──────────┬─────────────────────────────────────────────┬───────────────────┤
│ Project  │ Execution Planning — Timeline  v3           │ AI Project Lead   │
│ nav      │ [Graph|Timeline*|ParallelGroups]            │ ───────────────── │
│          │ [Day|Week*|Month] ◀ ▶ Today ⤢ 🔍 [Group ▾]  │ · Move T-003      │
│ Task Mgmt├──────────────┬──────────────────────────────┤   earlier → FE    │
│  Overview│ TASK         │ W45    W46    W47    W48     │   unblocks  [Acc] │
│  Breakdn │ Auth Feature ▏▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▏    │ · Repo conflict   │
│  Backlog │  T-001 API   │ ████░░  P1     ◆ Milestone   │   W47 gateway     │
│  ExecPlan│  T-002 Token │   ████████   P0              │ · Bottleneck      │
│  Readiness│  T-003 UI   │      ████████  P1  ↘depends  │   before Testing  │
│  (here)  │  T-004 Tests │         ░░░░░░░ blocked      │   milestone [Acc] │
│          ├──────────────┴──────────────────────────────┤                   │
│          │ Workload: @amy ▃▅█▅ · core-api ▅█▅ · web ▃▃▅ │                   │
├──────────┴─────────────────────────────────────────────┴───────────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"                 │
└────────────────────────────────────────────────────────────────────────────┘
```

## 5. Timeline Toolbar

Time scale: Day / Week / Month. Controls: Previous / Next Period · Today · Zoom In/Out · Fit Timeline · Group By (feature / repository / assignee) · Filter · Show Dependencies · Show Milestones.
Filters: Repository · Assignee · Priority · Task Type · Task Status.

## 6. Main Timeline Canvas

Horizontal Gantt-style timeline, synchronized two-pane layout:
- **Left list**: Task ID · Task Name · Priority · Assignee · Repository — rows stay aligned with bars.
- **Right canvas**: time scale · task bars · dependency connectors · milestone markers · planned date ranges.

Tasks grouped hierarchically, collapsible:
```
Authentication Feature: T-001 Authentication API · T-002 Token Validation · T-003 Login UI · T-004 Integration Tests
User Management Feature: T-005 User Profile API · T-006 Profile UI · T-007 User Management Tests
```

Bars: planned range with estimated duration; neutral surface + accent for critical path; blocked tasks get a hatched/ghost segment; unscheduled tasks appear in a side tray for drag-in.

## 7. Task Scheduling Interactions

- Drag bars → change planned start; resize → adjust estimated duration; move between groups.
- Select → open Task Detail; highlight predecessors/successors.
- Moving a task shows a **preview of downstream impact** on dependent tasks (ghost bars shift) before commit.
- Material schedule changes require explicit confirmation before saving.
- Invalid dependency order is prevented; violations show warnings.

## 8. Dependency Visualization

Directional connectors on the canvas: Finish-to-Start (default) · Start-to-Start where supported. On task select: highlight upstream dependencies and downstream tasks, show potential schedule impact, display dependency warnings. Invalid relationships are not allowed.

## 9. Milestones

Markers on the timeline, each with: Name · Planned Date · Related Tasks · Completion Conditions. Examples: *Authentication Foundation Complete · API Integration Complete · Testing Complete · Implementation Baseline Delivery*. Milestones link to their tasks (click → highlight + filter).

## 10. Workload Distribution

Compact workload panel below or beside the canvas: planned workload by Assignee · Repository · Engineering Area · Time Period. Highlight: overloaded assignments · uneven distribution · potential conflicts · unallocated work. Caption: "Estimated effort is a planning estimate — not actual engineering time."

## 11. AI Scheduling Recommendations

Examples: "Moving T-003 earlier may allow frontend work to proceed independently." · "Three tasks are scheduled in the same period and target the same repository." · "Current scheduling creates a bottleneck before the testing milestone."
Each: recommendation · reasoning · affected tasks · expected impact · **Preview Changes** (ghost-diff on canvas) · **[Accept] [Reject]**. AI never auto-changes the approved schedule.

## 12. Schedule Validation & Versions

**Validate Schedule** checks: missing planned dates · invalid dependency order · circular dependencies · scheduling conflicts · overlapping resource allocation · unscheduled tasks · milestone inconsistencies → expandable validation panel.

Version management: Save Planning Version · Version History · **Compare Versions** (highlight added/removed tasks, changed dates, changed dependencies, changed milestones) · Restore Previous Version. Planning change history is preserved.

## 13. Components
TimelineCanvas (two-pane sync) · TaskBar (planned/blocked/critical/ghost-diff variants) · DependencyConnector (FS/SS) · MilestoneMarker · FeatureGroupRow · UnscheduledTray · ImpactPreviewOverlay · WorkloadPanel · ScaleSwitcher · VersionMenu + CompareDiff · ValidationPanel · AISuggestionCard.

## 14. Interaction & States
Prototype: open timeline → switch Week↔Month → expand a feature → select task → inspect dependencies → drag a task → preview downstream impact → review AI recommendation → validate → save version → navigate to Dependency Graph.
States: empty schedule (drag from unscheduled tray) · validation failed · compare overlay · unsaved-changes banner · loading skeleton.

## 15. Visual Style
V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 (critical path, selection) / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF (light variant per token file). Typography 15/13/12/11. Clean horizontal Gantt: subtle grid lines, compact labels, restrained bar colors, distinct milestone markers. Avoid oversized colorful bars, decoration, and generic business PM templates. No purple.

## 16. Deliverable
High-fidelity desktop UI (1440×900) + fully connected interactive prototype implementing §14.
