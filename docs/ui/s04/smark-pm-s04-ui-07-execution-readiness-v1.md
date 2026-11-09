# S04-UI-07 — Execution Readiness

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S04 · Task Management & Planning（页面 7/7，S05 移交边界）
> - 关联：S04-00 总提示词、S04-UI-03 Task Detail、S04-UI-05/06 Execution Planning、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S04-07 Execution Readiness

## 1. Design Objective

Design the Execution Readiness page: the planning quality gate that determines whether the approved task plan contains sufficient information to be handed to **S05 — Engineering Agent Execution & Delivery**.

It must NOT start an Agent, execute code, or create commits. It helps humans find and resolve missing task information **before** execution begins, then performs a controlled handoff.

Core rule: **Approved ≠ Ready.** A task only becomes Ready when repository, dependencies, execution instructions and acceptance criteria are complete.

## 2. Global Layout

1440px desktop. Global frame identical to S04-UI-01 §2. S04 secondary nav: Overview / Breakdown / Backlog / Execution Planning / **Execution Readiness (selected)**.

## 3. Page Header

- Title: **Execution Readiness** · Subtitle: "Validate task completeness before handing work over to engineering execution."
- Metrics: Planning Version (v3) · Last Validation (10 min ago) · Total Tasks (128) · Ready (31) · Needs Attention (12) · Blocked (4).
- Primary: **Validate Task Readiness**. Secondary: **Review Issues** (jumps to §7).

## 4. Low-Fidelity Wireframe

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Top bar                                                                    │
├──────────┬─────────────────────────────────────────────┬───────────────────┤
│ Project  │ Execution Readiness  v3   [Validate][Issues]│ AI Project Lead   │
│ nav      ├─────────────────────────────────────────────┤ ───────────────── │
│          │  Ready 31 · 24%  │ Needs Attention 12 │ Blocked 4              │
│ Task Mgmt│  (three summary segments, click = filter)   │ · 3 tasks w/      │
│  Overview├─────────────────────────────────────────────┤   incomplete AC   │
│  Breakdn │ READINESS CHECKLIST  ADefinition BTrace     │ · 2 tasks ref old │
│  Backlog │  CDependency DPreparation EAccept&Delivery  │   baseline rev    │
│  ExecPlan│  FApproval   each: ✓ / ⚠ / ⛔ / n.a.        │   [Review]        │
│  Readiness├─────────────────────────────────────────────┤                   │
│  (here)  │ ☑ T-043 Token valid.  P0  ✓ Ready    core-api                   │
│          │ ⚠ T-041 Login endp.   P1  ⚠ missing AC       [Open]            │
│          │ ⛔ T-045 Rate limit    P2  ⛔ ADR unresolved  [Open]            │
│          ├─────────────────────────────────────────────┤                   │
│          │ HANDOFF  v3 · 31 included · 4 excluded      │                   │
│          │ [Prepare S05 Handoff]  (guarded, confirm)   │                   │
├──────────┴─────────────────────────────────────────────┴───────────────────┤
│ AI command bar: "Ask AI Project Lead about task planning…"                 │
└────────────────────────────────────────────────────────────────────────────┘
```

## 5. Readiness Summary

Three category segments with counts **and** percentages — never a single overall percentage as the only indicator:
- **Ready** (success #34C08B / ready #8DC5AF): satisfies all execution-preparation criteria.
- **Needs Attention** (warning #E0B15E): missing/incomplete information that can be resolved.
- **Blocked** (danger #F07E88): cannot be prepared — unresolved dependencies or upstream decisions.

Each segment clickable → filters the table below.

## 6. Readiness Checklist

Six categories, each item shows state: Passed ✓ / Needs Attention ⚠ / Blocked ⛔ / Not Applicable (n.a.), plus a link to affected tasks.

- **A. Task Definition** — clear objective · scope defined · expected result specified · implementation instructions available.
- **B. Source Traceability** — requirements linked · technical design linked · relevant ADRs resolved · Implementation Baseline valid & approved.
- **C. Dependency Validation** — dependencies defined · no circular dependency · predecessor conditions satisfied · no unresolved blocking relationship.
- **D. Engineering Preparation** — repository assigned · target module/component specified · technical constraints documented · required context available.
- **E. Acceptance & Delivery** — acceptance criteria defined · verification methods specified · deliverables defined · required tests identified.
- **F. Human Approval** — required approvals complete · planning changes confirmed · outstanding human decisions resolved.

## 7. Task Readiness Table

Columns: Task ID · Task Name · Priority · Readiness State · Missing Information (explicit list, e.g. "no repository; 2 acceptance criteria") · Repository · Dependencies · Last Updated.
Filter: All / Ready / Needs Attention / Blocked. Row click → Task Detail (with missing fields highlighted).

## 8. Issue Resolution Panel

Dedicated issue list. Examples: T-001 has no assigned repository · T-002 missing acceptance criteria · T-003 depends on an unresolved ADR · T-004 has an invalid dependency · T-005 has no linked Technical Design.
Each issue: Severity · Affected Task · Reason · Recommended Resolution · **[Open Task]** · **[Mark Resolved]**.
An issue may only be marked resolved when the underlying condition is actually corrected — or legitimately **waived** (waiver requires a reason and is logged).

## 9. AI Project Lead Analysis

Examples: "Three tasks have incomplete acceptance criteria." · "Two tasks reference a Technical Design revision that is no longer the current approved baseline." · "One task cannot be prepared because its predecessor is blocked."
Each: explanation · affected tasks · source references · recommended actions · **[Review]**. AI may suggest corrections; humans confirm material changes.

## 10. S05 Handoff Preparation (bottom section)

Displays: Planning Version · Included Tasks · Excluded Tasks · Readiness Summary · Unresolved Issues · Approval Summary.
Primary action: **"Prepare S05 Handoff"** — enabled **only** when required readiness conditions and human approvals are satisfied (otherwise disabled with the blocking reasons listed).
Before handoff: confirmation dialog explaining which tasks are included, which are excluded (and why), which planning version is handed over, and any warnings.
The action prepares a handoff package to S05 — it does not itself execute engineering work.

## 11. Components
ReadinessSummarySegment (3) · ChecklistCategory (A–F) · ChecklistItem (4 states) · ReadinessTable + ReadinessBadge · MissingInfoList · IssueRow (severity) · WaiverDialog · HandoffSummaryCard · HandoffConfirmDialog · AISuggestionCard · ValidationRunState.

## 12. Interaction & States
Prototype: run validation → summary updates → filter Blocked → open an issue → fix missing acceptance criteria in Task Detail → resolve dependency issue → re-validate → review handoff summary → confirm S05 handoff.
States: validating (skeleton/progress) · all-ready (restrained success state — no confetti, no exaggerated green) · handoff disabled with reasons · handoff confirm dialog · post-handoff banner "Handoff package v3 delivered to S05" with link to the package record.

## 13. Visual Style
Quality-gate language on V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF (light variant per token file). Typography 21/15/13/12/11, tabular numerals.
Restrained success/warning indicators, professional checklist layout, compact tables, clear severity, explicit confirmation dialogs. **Avoid celebratory dashboards or exaggerated green completion indicators** — the page communicates preparation quality, accountability and controlled handoff. No purple.

## 14. Deliverable
Complete high-fidelity page (1440×900) + connected interactive prototype implementing §12.
