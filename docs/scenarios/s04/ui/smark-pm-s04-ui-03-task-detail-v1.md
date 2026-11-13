# S04-UI-03 — Task Detail

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S04 · Task Management & Planning（页面 3/7）
> - 关联：S04-00 总提示词、S04-UI-02 Breakdown、S04-UI-04 Backlog、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-03 Task Detail

## 1. Design Objective

Design a comprehensive Task Detail page for S04 — Task Management & Planning.

Task Detail is the authoritative workspace for understanding, editing, reviewing and approving an individual engineering task. It must clearly communicate:
- What needs to be done, and why the task exists.
- Which requirements it implements; which technical designs and ADRs constrain it.
- What other tasks it depends on; where it should be implemented.
- What constitutes successful delivery.

This page defines execution instructions but does not execute code. **S04 organizes and prepares engineering work; S05 executes it.**

## 2. Global Layout

1440px desktop enterprise SaaS. Open as a **full-page view** with clear back navigation to Task Backlog (deep-linkable). Two-column content layout: main ≈70% · right context panel ≈30%.

Global frame identical to S04-UI-01 §2 (top bar / left nav / bottom AI command bar). S04 secondary nav highlights **Backlog** (detail is reached from it).

## 3. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                │
├────────┬───────────────────────────────────────────────┬───────────────┤
│ Project│ ← Back to Backlog   Project / Task Mgmt / T-001│ AI Project    │
│ nav    │ T-001  Implement login endpoint   [Dev][P1]   │ Lead          │
│        │ Planning: Pending Review · Approval: —        │ ───────────── │
│ Backlog│ [Edit][More ▾]        [Approve][Return f.Rev.]│ · Task under- │
│ (here) ├───────────────────────────────────────────────┤   standing   │
│        │ A Objective & Scope          (inline editable)│ · Missing     │
│        │ B Source Traceability  REQ-201 · DES-114 ·    │   information │
│        │                        ADR-007 · BASE-v1.2    │ · Risks       │
│        │ C Dependencies      ▸ depends on 2 · blocks 1 │ · Consistency │
│        │ D Execution Specification (repo/module/…)     │ · Suggestions │
│        │ E Acceptance Criteria   AC-1 AC-2 AC-3 [+]    │   [Apply]     │
│        │ F Delivery Requirements  code/tests/docs      │   [Ignore]    │
│        │ G History & Audit   ○ ○ ○ timeline            │   [Discuss]   │
├────────┴───────────────────────────────────────────────┴───────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"             │
└────────────────────────────────────────────────────────────────────────┘
```

## 4. Task Header

- Breadcrumb: Project / Task Management / Backlog / T-001.
- Task ID · title · TaskTypeTag · PriorityTag · **planning status** · **approval status** (two separate badges; planning status ≠ execution status).
- Actions: Edit · More Actions (⋯) · **Approve** · **Return for Revision**.
- Metadata row: Assignee · Repository · Created By · Created At · Updated At · AI-generated badge (when applicable).

## 5. Section A — Objective & Scope
Fields: Objective · Background · Description · In Scope · Out of Scope · Expected Result. Structured, labeled sub-blocks — never one oversized text field. Inline editing with explicit Save / Cancel.

## 6. Section B — Source Traceability
Linked source objects as compact cards: Related Requirements · Technical Designs · ADRs · Implementation Baseline.
Each card shows: Object ID · Title · **Version** · relationship type · approval status. Clickable → source object. Multiple references per type supported; version references are pinned (show "BASE-v1.2", not just "BASE").

## 7. Section C — Task Dependencies
- Depends On · Blocks · Related Tasks — compact dependency cards (Task ID · title · current status · dependency type).
- Small visual dependency preview (mini node-edge sketch around this task).
- Add / remove relationships; **warn on circular dependencies** and invalid references.

## 8. Section D — Execution Specification
Engineering preparation section: Target Repository · Target Module · Target Component · Implementation Instructions · Technical Constraints · Required Context · Relevant Documentation.
Must carry enough context for a future engineering Agent or developer. **No controls that execute code.**

## 9. Section E — Acceptance Criteria
Structured criteria editor. Each criterion: Criterion ID · Description · Verification Method · Required Evidence · Mandatory/Optional.
Support: Add · Edit · Delete · Reorder · **AI Suggest** · Confirm. Criteria must be independently verifiable ("given/when/then" or equivalent).

## 10. Section F — Delivery Requirements
Expected deliverables: Code Changes · Unit Tests · Integration Tests · Documentation · Configuration Changes · Implementation Evidence. Each marked Required or Optional by the human owner.

## 11. Section G — History & Audit
Chronological activity timeline: creation event · AI-generated proposal · human modifications · approval history · version changes · status transitions. **AI actions and human actions are visually distinct** (AI badge vs avatar).

## 12. Right AI Project Lead Panel
Displays: task understanding · missing information · potential risks · requirement consistency · design consistency · suggested improvements. Example callout to render:
> "Acceptance criteria do not currently specify how expired authentication tokens should be handled."

Actions per suggestion: **Apply Suggestion / Ignore / Discuss**. AI changes to approved task specifications require explicit human confirmation.

## 13. Components
TaskHeaderBar · MetadataRow · TraceCard (4 object types) · DependencyCard + MiniDependencyGraph · AcceptanceCriterionRow · DeliveryChecklist (required/optional) · ActivityTimeline (AI / human variants) · AISuggestionCard · ApprovalActionBar · InlineEditor · SectionDivider.

## 14. Interaction & States
Prototype journeys: open detail → edit objective → open a related Requirement → add dependency → edit a criterion → request AI suggestions → review AI changes → approve the task.
Render states: default (Pending Review) · **Approved** (header banner; material edits require new revision + re-approval) · Returned for Revision (warning banner) · missing information highlights (sections flagged, linking to Execution Readiness) · loading skeleton · not found.

## 15. Visual Style
V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Sidebar #1D2631 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF (light variant per token file). Typography 21/15/13/12/11. Structured sections with subtle dividers, consistent spacing, compact metadata, clear source links, explicit approval states; avoid excessive cards. **No purple** — AI content = Info-blue tint + AI badge.

## 16. Deliverable
High-fidelity desktop page (1440×900) + interactive prototype implementing §14.
