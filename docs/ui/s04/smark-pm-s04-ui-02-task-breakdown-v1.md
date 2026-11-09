# S04-UI-02 — Task Breakdown

> - 记录日期：2026-11-10
> - 状态：已落库，待执行（合并 2026-11-10 用户补充版）
> - 领域：S04 · Task Management & Planning（页面 2/7，核心 AI 交互）
> - 关联：S04-00 总提示词、S04-UI-01、S04-UI-03、S04-UI-04、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-02 Task Breakdown

## 1. Design Objective

Design the Task Breakdown workspace for S04 — Task Management & Planning: the primary AI-assisted task generation and decomposition interface.

Purpose: transform an **approved Implementation Baseline from S03** into structured engineering tasks. The user can inspect, edit, reorganize and approve AI-generated task proposals.

Core rules:
- **AI-generated tasks are proposals, not automatically approved tasks.**
- Humans retain final approval authority, with bulk review (Accept Selected / Reject Selected / Edit and Accept).
- Every proposal carries its source traceability. Generating tasks never implies starting engineering execution — **S04 organizes; S05 executes.**

## 2. Global Layout

1440px desktop enterprise SaaS. Global frame identical to S04-UI-01 §2 (top nav / left project nav / right AI Project Lead panel 320px / bottom AI command bar). S04 secondary nav: Overview / **Breakdown (selected)** / Backlog / Execution Planning / Execution Readiness.

## 3. Page Header

- Title: **Task Breakdown** · Subtitle: "Transform approved implementation baselines into structured engineering work."
- Context chip: selected Implementation Baseline + version — "BASE-v1.2 (Approved)".
- Actions: **Generate Tasks** · Import Tasks · Create Task.

## 4. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                    │
├──────────┬─────────────────────────────────────────────┬───────────────────┤
│ Project  │ Task Breakdown [BASE-v1.2 Approved▾]        │ AI Project Lead   │
│ nav      │ [Generate Tasks][Import][Create Task]       │ ───────────────── │
│          ├──────────────────────────────┬──────────────┤ · Why this task   │
│ Task Mgmt│ SOURCE BASELINE (collapsible)│              │   exists          │
│  Overview│  REQ-201 · DES-114 · ADR-007 │ AI ANALYSIS  │ · Traceability    │
│  Breakdn │  scope · v1.2 · Approved     │ (for select- │ · Missing work    │
│  Backlog ├──────────────────────────────┤  ed task)    │ · Overlap warning │
│  ExecPlan│ TASK HIERARCHY [⚙ gen config]│              │ · Suggested       │
│  Readiness│ ▾ F-1 User Authentication  ◇AI             │   subtasks        │
│          │   ├ T-001 Auth API    P1 ◇ Pending Review  │ · Risks           │
│          │   │  ├ T-001.1 Request model ◇            │                   │
│          │   │  ├ T-001.2 Login endpoint ◇           │                   │
│          │   │  └ T-001.3 Token validation ◇         │                   │
│          │   ├ T-002 Frontend login ◇                 │                   │
│          │   └ T-003 Auth tests ◇                     │                   │
│          │ ☑ 4 selected · [Accept Selected][Reject][Edit & Accept]        │
│          │ Proposals 14 · Accepted 6 · Rejected 1 · Pending 7             │
├──────────┴──────────────────────────────┴──────────────┴───────────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"                 │
└────────────────────────────────────────────────────────────────────────────┘
```

## 5. Source Baseline Panel (top of workspace, collapsible)

- Implementation Baseline name · **version** · approval status badge · related Requirements / Technical Designs / ADRs (clickable chips) · Implementation Scope · Last Updated.
- **View Source** action (opens the S03 baseline).
- Only **approved** baselines are eligible for formal task generation (unapproved → selector disabled with reason).

## 6. AI Task Generation Workflow

User selects an approved baseline and configures generation:
- Task granularity (coarse / standard / fine).
- Include testing tasks · include documentation tasks · include infrastructure tasks.
- Consider repository boundaries · identify dependencies · generate acceptance criteria.

Primary button: **Generate Task Breakdown**. States: configuration → **AI processing** (streaming skeleton rows, never implying execution) → **completed proposals** (all rows Pending Review). Proposal history and source traceability are preserved.

## 7. Task Hierarchy (main focus)

Professional hierarchical tree; example to render:

```
Feature: User Authentication
├ T-001 Implement Authentication API
│  ├ T-001.1 Define authentication request model
│  ├ T-001.2 Implement login endpoint
│  └ T-001.3 Implement token validation
├ T-002 Implement Frontend Login
│  ├ T-002.1 Build login form
│  └ T-002.2 Integrate authentication API
└ T-003 Add Authentication Tests
   ├ T-003.1 API unit tests
   └ T-003.2 Integration tests
```

Row anatomy: checkbox · expand/collapse caret · Task ID · title · TaskTypeTag · PriorityTag · estimated effort · RepositoryChip · dependency count · **proposal status badge**.

## 8. Task Editing

Support: Add Task · Add Subtask · Edit · Delete · Duplicate · Split Task · Merge Tasks · Move Task · Reorder · Change Parent · Multi-select. Contextual action menu per task; drag-and-drop hierarchy editing with clear drop indicators.

## 9. Right AI Analysis Panel (per selected task)

- **Why this task exists** — business/engineering rationale.
- **Source Traceability** — Requirement · Technical Design section · ADR · Implementation Baseline (each with ID + version).
- **AI Analysis** — necessity · missing work · potential overlap · suggested subtasks · dependencies · technical risks.
- **Suggested Acceptance Criteria** — individually editable rows (add / edit / dismiss before accepting).

## 10. Human Review Workflow

Three proposal states: **Pending Review · Accepted · Rejected**.
Actions: Accept · Reject · **Edit and Accept** · Accept Selected · Reject Selected.
Approval summary bar: Total proposed · Accepted · Rejected · Pending review.
Human approval is required before proposals become approved tasks; every action is logged to task history. AI-proposed rows use Info-blue tint + dashed border + "AI" badge; accepted rows restyle to normal; rejected rows greyed with strikethrough and recoverable via the "Rejected" filter.

## 11. Components
BaselineCard · GenerationConfigPopover · GenerateRunningState (streaming skeleton) · TaskTreeNode · TaskRow (proposal / accepted / rejected / edited variants) · AIBadge · AIAnalysisPanel · SuggestedSubtaskItem · SuggestedCriterionRow · BulkReviewBar · ApprovalSummaryBar · RepositoryChip · DependencyChip · TraceabilityLink · EmptyState (no approved baseline).

## 12. Interaction & States
Prototype: select baseline → generate → hierarchy of proposals streams in → select a task → inspect AI reasoning → edit title/description → add subtask → Accept Selected → reject another proposal → navigate to Task Backlog.
States: no approved baseline (EmptyState + link to S03) · generating · pending review · accepted · rejected · edited ("AI · Edited") · bulk-selected · all-reviewed (banner "Breakdown approved — continue in Execution Planning").

## 13. Visual Style
High-density engineering workspace; the task tree is the visual hero. Subtle indentation (12px steps, 28px rows), clear hierarchy lines, compact rows, distinct AI proposal indicators (Info-blue tint + AI badge), clear human approval indicators, restrained status colors, consistent panel spacing. V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF (light variant per token file). Typography 21/15/13/12/11. **Do not make the page look like a chatbot** — the primary interaction is task structure management with AI as an integrated planning assistant. No purple.

## 14. Deliverable
High-fidelity desktop design (1440×900) + interactive prototype implementing §12.
