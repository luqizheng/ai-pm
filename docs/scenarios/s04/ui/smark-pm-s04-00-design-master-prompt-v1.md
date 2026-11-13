# S04-00 — Task Management & Planning · 模块级设计总提示词

> - 记录日期：2026-11-10
> - 状态：已落库（v1，对齐 V2 设计令牌）
> - 领域：S04 · Task Management & Planning
> - 关联：本目录 INDEX.md、七份页面级提示词（S04-UI-01…07）、`docs/design/pm-ai-design-tokens-v2.css`

---

# S04 — Task Management & Planning
## AI R&D Workspace — High-Fidelity UI/UX Design Prompt

### 1. Product Context

Design the S04 module of an enterprise-grade AI R&D Project Management Platform.

S04 is called **Task Management & Planning**. Its purpose is to transform approved implementation baselines into structured, executable, traceable engineering tasks.

Platform lifecycle:

- S01 — Project Initialization & Code Discovery
- S02 — Requirements Management
- S03 — Technical Design & Implementation Baseline
- S04 — Task Management & Planning
- S05 — Engineering Agent Execution & Delivery

S04 organizes and prepares engineering work. It does NOT execute code, run tests, create commits, or operate engineering agents — that is S05. The primary outcome of S04 is an approved, organized, execution-ready task plan.

### 2. Design Philosophy

- Enterprise-grade SaaS, desktop-first (1440px).
- Professional, restrained, information-dense.
- AI-assisted planning with explicit human control.
- Prioritize actionable information over vanity metrics.
- Avoid generic project-management dashboard templates and decorative elements.
- Use the established AI R&D Workspace design language (V2 "Slate & Muted Teal", see §11).

### 3. Global Workspace Layout

**Top Navigation**: product logo · team selector · project selector · global navigation · notifications · user profile.

**Left Navigation**: Project Command Center / Requirements / Technical Design / **Task Management (selected)** / Knowledge / Project Settings.

Secondary navigation inside Task Management: Overview / Breakdown / Backlog / Execution Planning / Execution Readiness.

**Main Content Area**: tables, task trees, dependency graphs, planning boards.

**Right Context Panel** (320px, collapsible): AI Project Lead — current planning context, related requirements and technical designs, AI suggestions, risks and unresolved issues, human review actions.

**Bottom AI Command Bar**: persistent natural-language input (e.g. "Break down the approved implementation baseline into engineering tasks."), context-aware.

The right panel and bottom bar stay visually consistent with all other project modules.

### 4. Page A — Task Overview

Answer: what engineering work exists, how much is planned, what needs human attention, what is blocked, is the project ready for execution.

Sections:
- **A. Planning Summary** — Total Tasks / Unplanned Work / Awaiting Review / Blocked / Execution Ready.
- **B. Human Attention Required** — AI-generated tasks awaiting approval; tasks missing acceptance criteria; unresolved dependencies; tasks without repository assignment; conflicting or duplicated tasks; tasks requiring clarification.
- **C. Task Progress** — distribution by Draft / Pending Review / Approved / Ready for Execution / Blocked / Completed / Cancelled. Planning readiness ≠ engineering execution status.
- **D. Upcoming Work** — prioritized tasks and planned work.
- **E. AI Planning Insights** — decomposition suggestions, dependency optimization, parallelization, missing tasks, planning risks. Each supports Review / Accept / Reject / Edit.

### 5. Page B — Task Breakdown

AI-assisted decomposition workspace transforming an approved Implementation Baseline into a task hierarchy.

- **Top — Source Baseline**: baseline name + version, related Requirements / Technical Designs / ADRs, implementation scope, approval status. Primary action "Generate Task Breakdown".
- **Main — Task Hierarchy** (tree): each row shows Task ID, title, type, priority, estimated effort, repository, dependencies, review status. Operations: add / edit / delete / move / change hierarchy / split / merge / reorder / multi-select / accept-reject AI suggestions.
- **AI Decomposition Panel**: why the task is needed; which requirement it implements; which design section it follows; which ADR constrains it; suggested subtasks; missing acceptance criteria; potential risks.

AI-generated tasks are proposals until a human approves them.

### 6. Page C — Task Backlog

Professional task table. Views: Table / Grouped / Kanban / Hierarchical.

Columns: Task ID / Title / Type / Priority / Status / Assignee / Repository / Estimated Effort / Dependencies / Related Requirement / Related Design / Updated At.

Filters: Status / Priority / Type / Repository / Assignee / Requirement / Technical Design / Approval State. Plus search, sorting, multi-select, bulk operations, saved filters, customizable columns.

Planning status and execution readiness are represented separately.

### 7. Page D — Execution Planning

Three complementary visualizations:

- **A. Dependency Graph** — directed graph: predecessors, successors, blocking relationships, parallelizable tasks, critical paths.
- **B. Timeline / Gantt** — planned start/completion, duration, dependencies, milestones, workload distribution. Engineering sequencing, not a corporate planner.
- **C. Parallel Execution Groups** — independent tasks that can run in parallel: group name, tasks, dependency conditions, repository distribution, potential conflicts.

AI may recommend parallelization; humans can modify the plan.

### 8. Page E — Task Detail

Page or side drawer with sections:
1. **Header** — ID, title, status, priority, assignee, repository, approval state.
2. **Description** — Objective / Background / Scope / Out of Scope / Expected Result.
3. **Source & Traceability** — Requirement, Technical Design, ADR, Implementation Baseline, source references + versions (clickable).
4. **Dependencies** — Depends On / Blocks / Related Tasks.
5. **Execution Specification** — repository, target module, implementation instructions, technical constraints, required context, expected deliverables. Defines future instructions; executes nothing.
6. **Acceptance Criteria** — explicit, independently verifiable; add / edit / remove; AI-suggested + human confirmation.
7. **Delivery Requirements** — code changes, tests, documentation, implementation evidence.
8. **History** — authorship (AI/human), changes, approvals, versions.

### 9. Page F — Execution Readiness

Verify the task plan is complete enough for S05. Readiness checklist: decomposition completed; clear objectives; requirements traceable; designs linked; ADRs resolved; dependencies valid; repositories assigned; execution instructions complete; acceptance criteria explicit; deliverables defined; human approvals completed.

Three categories: **Ready** / **Needs Attention** / **Blocked**. Plus readiness summary, detailed issue list, AI recommendations, human review actions.

Primary action: **"Prepare for S05 Execution"** — a controlled handoff. Must not run code or launch an agent.

### 10. AI Interaction Design

AI is an integrated Project Lead, not a chatbot.

- **AI Proposal**: show proposed changes, reason, source references; Accept / Reject / Edit.
- **Multi-selection Review**: bulk approval/rejection of AI-generated tasks, with clearly identified selections.
- **AI Recommendations**: missing tasks, dependency problems, parallelization, ambiguous acceptance criteria, duplicate tasks.
- **Human Control**: AI never silently modifies approved plans; significant changes require confirmation; full change history and traceability.

### 11. Visual Design Requirements

Locked to the V2 design system **"Slate & Muted Teal"** (`docs/design/pm-ai-design-tokens-v2.css` — the single source of truth; components in Figma follow this file).

| Role | Dark (default render target) | Light variant |
|---|---|---|
| Canvas | #171C24 | #F5F8FA |
| Sidebar / nav | #1D2631 | #EDF2F5 |
| Surface (cards, tables) | #222A34 | #FFFFFF |
| Elevated (nested fills, badges, hover) | #2B3541 | #EAF0F3 |
| Border / separator | #374452 | #DCE4E9 |
| Text primary / secondary | #E7EDF2 / #A8BBC9 | dark equivalents per token file |
| Accent (primary actions, selection) | #75A9A4 · hover #8AB9B4 · pressed #63948F | same hue |
| Accent tint surface | #253735 | — |
| Info / Success / Warning / Danger | #6EA8FE / #34C08B / #E0B15E / #F07E88 | per token file |
| Status "Ready" | #8DC5AF | #3F8F70 |

Typography: Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11; tabular numerals for IDs and counts.

Visual hierarchy must distinguish: human-approved information · AI-generated proposals · unresolved issues · blocked tasks · execution-ready tasks.

- **No purple and no gradients anywhere** — V2 removed the purple brand accent; AI-generated content uses the Info-blue tint + "AI" badge.
- Avoid oversized dashboard cards, decorative illustrations, unnecessary charts, colorful task boards, chat-centric layouts.

### 12. Interaction & Prototype Requirements

Connected interactive prototype covering four journeys:

1. **Generate Tasks** — Breakdown → select approved baseline → AI generates → review hierarchy → edit/reject → approve.
2. **Organize Execution Plan** — Execution Planning → review dependencies → adjust sequence → identify parallel groups → check repositories → save.
3. **Review a Task** — Task Detail → traceability → execution spec → acceptance criteria → approve or return.
4. **Prepare S05 Handoff** — Readiness → checklist → resolve gaps → confirm plan → handoff.

### 13. Final Deliverables

Seven high-fidelity desktop screens, numbered as the page-level prompts in this directory: 01 Task Overview · 02 Task Breakdown · 03 Task Detail · 04 Task Backlog · 05 Execution Planning (Dependency Graph + Parallel Groups) · 06 Execution Planning (Timeline) · 07 Execution Readiness.

Plus reusable components: global navigation, task components, status/priority components, AI proposal review components, dependency visualization, task detail sections, approval interactions.

All screens must read as one coherent enterprise AI R&D product. The design must clearly communicate:

**S04 organizes and prepares engineering work. S05 executes engineering work.**
