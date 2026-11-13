# S04-UI-05 — Execution Planning · Dependency Graph（含并行执行组）

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S04 · Task Management & Planning（页面 5/7）
> - 关联：S04-00 总提示词、S04-UI-06 Timeline（同属 Execution Planning）、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-05 Execution Planning · Dependency Graph

## 1. Design Objective

Design the Execution Planning workspace for S04 — Task Management & Planning: organize approved tasks into a logical engineering execution plan.

Focus: task dependencies · execution order · parallelization · critical paths · repository distribution. **This is planning only — it never launches Agents or executes code. Actual execution belongs to S05.**

This prompt covers two of the three Execution Planning views: **Dependency Graph** (primary) and **Parallel Execution Groups**. The Timeline view is specified in S04-UI-06.

## 2. Global Layout

1440px desktop. Global frame identical to S04-UI-01 §2. Execution Planning has a **view sub-switcher**: Dependency Graph (selected) / Timeline / Parallel Groups. Main workspace occupies most of the width; compact right AI Project Lead panel.

## 3. Page Header

- Title: **Execution Planning** · Subtitle: "Organize engineering tasks, dependencies and execution sequence."
- Metrics: Planning Version (v3) · Last Updated · Total Approved Tasks (47) · Tasks with Dependencies (31) · Parallelizable Tasks (12) · Unresolved Dependencies (2).
- Actions: **Save Plan** · Validate Dependencies · Optimize Plan · Compare Versions.

## 4. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                    │
├──────────┬─────────────────────────────────────────────┬───────────────────┤
│ Project  │ Execution Planning  v3   [Save][Validate]   │ AI Project Lead   │
│ nav      │ [Graph*|Timeline|Parallel Groups]  ⤢ 🔍 ▦   │ ───────────────── │
│          ├─────────────────────────────────────────────┤ · Bottleneck:     │
│ Task Mgmt│                                              │   T-011           │
│  Overview│      ┌───────┐        ┌───────┐             │ · Parallelize     │
│  Breakdn │      │ T-001 │───────▶│ T-003 │             │   T-003 ∥ T-005  │
│  Backlog │      │ P1 ●  │        │ P0 ⚑  │             │   [Accept][Rej]  │
│  ExecPlan│      └───┬───┘        └───┬───┘             │ · Conflict        │
│  Readiness│          ▼                ▼                │   T-014/T-015    │
│  (here)  │      ┌───────┐        ┌───────┐             │                   │
│          │      │ T-002 │⋯⋯⋯⋯⋯⋯▶│ T-004 │  ⚑ critical │                   │
│          │      └───────┘        └───────┘             │                   │
│          │ Legend: ● blocked  ⚑ critical  ◇ parallel   │                   │
├──────────┴─────────────────────────────────────────────┴───────────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"                 │
└────────────────────────────────────────────────────────────────────────────┘
```

## 5. Planning Toolbar

View tabs (Dependency Graph / Timeline / Parallel Groups) + controls: Zoom In/Out · Fit to Screen · Filter (repo/status/priority) · Group By · Highlight Critical Path · Show Repository Boundaries.

## 6. Dependency Graph (primary visualization)

Interactive directed graph; node = task, edge = dependency.

Node anatomy: Task ID · title (truncated) · PriorityTag · planning status dot · RepositoryChip · dependency state (blocked indicator).

Visual treatments: normal · **blocked** (danger tint) · **critical path** (accent outline, ⚑) · **parallelizable** (◇ marker, dashed halo grouping) · requires attention (warning dot).

Support: pan · zoom · select node · inspect details · add dependency (drag between nodes) · remove dependency · reposition nodes (manual layout persisted with plan version).

On node select: highlight predecessors (upstream) and successors (downstream); dim the rest. **Circular dependencies are prevented** at creation; invalid relationships show inline warnings.

## 7. Parallel Execution Groups (view 3)

Groups of tasks that can potentially execute independently. Example to render:

- **Group A — Authentication Foundation**: T-001 Authentication API · T-002 Token Validation.
- **Group B — Frontend**: T-003 Login UI · T-004 Login Form Validation.

Each group card: Group Name · Tasks Included · Required Preconditions · Repository Distribution · Potential Conflicts (e.g. "T-014/T-015 modify the same module") · Parallelization Rationale. Users can reorganize groups (move tasks between groups, split, merge). AI-suggested groups carry the AI badge and require human review.

## 8. Dependency Inspector (side panel on selection)

Shows: Selected Task · Upstream Dependencies · Downstream Tasks · Dependency Reason · Potential Blocking Impact · Suggested Changes. Actions: Add Dependency · Remove Dependency · View Task · Review AI Recommendation.

## 9. AI Planning Assistant

Recommendation examples: "Two tasks can be executed in parallel." · "One dependency may create an unnecessary bottleneck." · "A critical path has been identified." · "Two tasks may conflict because they modify the same module." · "Repository boundaries may affect parallel execution."

Each: reasoning · impact · source tasks · suggested plan change · **[Accept] [Reject]**. AI must not silently modify the approved plan.

## 10. Planning Validation

**Validate Dependencies** checks: circular dependencies · missing predecessors · invalid task references · unresolved upstream conditions · conflicting relationships · impossible sequencing. Results open a dedicated issue panel (severity · affected tasks · reason · jump-to-node).

## 11. Components
GraphCanvas (pan/zoom/minimap) · TaskNode (5 state variants) · DependencyEdge (arrow, blocked/critical styles) · CriticalPathOverlay · ParallelGroupCard · RepositoryLane (optional background swimlanes) · DependencyInspector · ValidationIssuePanel · PlanningVersionChip · AISuggestionCard · ViewSubSwitcher · Legend.

## 12. Interaction & States
Prototype: open graph → select node → inspect upstream → add dependency → trigger circular-dependency warning → switch to Parallel Groups → reorganize a group → review AI parallelization suggestion → validate → save planning version.
States: empty plan (EmptyState + link to approved tasks in Backlog) · validation running · validation failed with issues · unsaved changes indicator · compare-versions overlay (added/removed/changed dependencies highlighted).

## 13. Visual Style
V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 (graph canvas may use a slightly darker base #171C24 with subtle dot grid) / Node Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 (critical path, selection) / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF (light variant per token file). Typography 15/13/12/11 inside nodes. Large readable canvas, precise connectors, compact nodes, restrained colors. Avoid generic Gantt-only PM templates — the dependency graph is the hero. No purple.

## 14. Deliverable
High-fidelity desktop screens (Dependency Graph primary + Parallel Groups view) + interactive prototype implementing §12.
