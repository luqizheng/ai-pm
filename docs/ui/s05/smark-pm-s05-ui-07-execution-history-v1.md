# S05-UI-07 — Execution History

> - 记录日期：2026-11-10
> - 状态：已落库，待执行
> - 领域：S05 · AI Agent Execution & Collaboration（页面 7/7，执行审计与回溯）
> - 关联：S05-00 总提示词、S05-UI-01/02/06、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-07 Execution History

## 1. Design Objective

Design the Execution History page: a compact, professional, searchable record of every execution attempt in the project — including past attempts, failures and previous delivery reviews.

It serves audit and post-mortem needs: "which runs touched this repo last month", "how many times did this task fail and why", "what did the reviewer decide last time". One task may have multiple attempts; **history is append-only and never overwritten** — each attempt is a distinct record.

## 2. Global Layout

1440px desktop. Same global frame as S05-UI-01 §2. Secondary nav **History (selected)**. Main column = dense table; right panel = selected record summary (AI Project Lead + quick facts).

## 3. Page Header

- Title: **Execution History** · Subtitle: "Every execution attempt, with its outcome and review verdict."
- Summary chips (compact, single row — not KPI cards): Executions (312) · Success Rate (78%) · Avg Duration (26m) · Awaiting Review (3) · Failures (14).
- Primary action: **Export** (secondary, CSV/JSON) — audit-friendly.

## 4. Filter Bar

- Search: task ID / title / commit SHA / agent name.
- Filters: status (queued / claimed / running / succeeded / failed / timed_out / cancelled) · review verdict (pending / approved / changes_requested / rejected / inconclusive) · agent · repository · branch · date range (with quick presets: today / 7d / 30d).
- Active filters shown as removable chips; "Clear all"; result count.

## 5. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar / project nav / AI Execution secondary nav                           │
├───────────┬──────────────────────────────────────────────┬───────────────────┤
│ Project   │ Execution History        312 executions   [Export]             │
│ nav       │ [search…][status▾][verdict▾][agent▾][repo▾][date▾]  ✕ chips   │
│ AI Exec   ├──────────────────────────────────────────────┤ SELECTED RECORD   │
│ History   │ RUN      TASK              AGENT    STATUS  │ T-043 Run#2       │
│ (here)    │ #3 T-043 Token service     Backend  ✓ ok    │ failed · 2026-11-09│
│           │ #2 T-043 Token service     Backend  ✗ exit  │ ───────────────── │
│           │ #1 T-043 Token service     Backend  ⏱ timeout│ 16m 23s          │
│           │ #1 T-047 Audit log UI     Frontend ✓ ok    │ reason exit_code  │
│           │ #2 T-038 Rate limit       Backend  ✗ fail  │ commits 1         │
│           │ #1 T-052 Billing export   Backend  ⏸ queued│ ───────────────── │
│           │ …                                                  │ REVIEW           │
│           │ columns: run · task · agent/device · status ·  │ changes_requested│
│           │ started · duration · commits · verdict · updated │ reviewer + time │
│           ├──────────────────────────────────────────────┤ feedback (1)      │
│           │ ⤵ expand row → inline summary strip:          │ [Open Run]       │
│           │   scope · outcome reason · files · tests       │ [Open Delivery]  │
├───────────┴──────────────────────────────────────────────┴───────────────────┤
│ AI command bar                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 6. History Table

Columns: **Run** (`#N` + kind badge delivery/review) · **Task** (ID + title, link) · **Agent / Device** (role + readiness dot) · **Status** (run state badge) · **Review verdict** (approved / changes_requested / inconclusive / "—" when not yet reviewed) · **Started** · **Duration** · **Commits** · **Files** · **Updated**.

- Sorted by started desc by default; sticky header; compact row height; tabular numerals for duration/commits/files.
- Run states and review verdicts are **separate columns** — a successful run may still carry a `changes_requested` verdict. This distinction must be visually obvious. Review verdict has exactly three values; a human rejection appears as a task-level outcome on the attempt row, not as a fourth verdict value.
- Row expansion (inline, no navigation): outcome reason, authorized scope vs actual paths, changed file count, test summary, failure category, and links to the run detail / delivery review.
- Failed and cancelled attempts remain visible forever — they are the audit trail.

## 7. Selected Record Panel

Right panel content: run identity (run id, attempt no, task link, plan version) · device/agent · timing (started, ended, duration, deadline) · outcome & reason · scope adherence · commits/files/tests · review verdict + reviewer + feedback history · links: **[Open Run Detail]** (02) · **[Open Delivery Review]** (06) · **[Reopen as New Attempt]** (only for terminal attempts; creates a new attempt — never edits history).

## 8. Components

FilterBar (search + 5 filter groups + chips) · HistoryTable (sticky header, expandable rows) · RunCell · VerdictBadge · DurationCell (tabular) · InlineSummaryStrip · SelectedRecordPanel · ExportButton · EmptyResultState (with filter-reset hint).

## 9. Interaction & States

Prototype: filter by verdict=changes_requested → see the returned deliveries → open one row → expand inline summary → jump to its Delivery Review (06) and to the Run Detail (02) → clear filters → export. States: empty result (neutral, with reset) · loading skeleton · long commit list truncated with "+n more" · mixed verdicts (color-coded without flooding) · a task with 4 attempts (grouped visually by task when sorting by task).

## 10. Visual Style

V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11, tabular numerals throughout numeric columns; compact 36–40px row height; hairline dividers; status via small badge + left status rule rather than filled rows. **No purple, no gradients; no decorative charts** — the page is a dense, auditable ledger.

## 11. Deliverable

Complete high-fidelity page (1440×1000) + connected prototype implementing §9.
