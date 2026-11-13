# S05-00 — AI Agent Execution & Collaboration · 模块级设计总提示词

> - 记录日期：2026-11-10
> - 状态：已落库（v1，对齐 V2 设计令牌；简报 §13 浅色方向按 S04 同款裁决收敛至 V2 深色）
> - 领域：S05 · AI Agent Execution & Collaboration
> - 关联：本目录 INDEX.md、七份页面级提示词（S05-UI-01…07）、`docs/design/pm-ai-design-tokens-v2.css`、`docs/scenarios/s05/S05-00-code-facts-and-gap-analysis-v1.md`（后端数据面事实）

---

# S05 — AI Agent Execution & Collaboration
## AI R&D Workspace — High-Fidelity UI/UX Design Prompt

### 1. Product Context

Design the S05 module of an enterprise-grade AI R&D Project Management Platform (SMARK).

S05 is called **AI Agent Execution & Collaboration**. It is where previously approved engineering tasks are actually executed: a human authorizes an execution, external engineering agents (Codex / Claude Code / Trae / zcode profiles) perform the work in managed workspaces, PM-AI coordinates multiple agents, humans intervene when agents wait or fail, and humans formally review and accept the delivery.

This is NOT an AI chat interface and NOT a CI/CD dashboard. It is a **human-governed AI engineering execution management interface**.

Platform lifecycle:

- S01 — Project Initialization & Code Discovery
- S02 — Requirements Management
- S03 — Technical Design & Implementation Baseline
- S04 — Task Management & Planning
- **S05 — AI Agent Execution & Collaboration (this module)**

S05 consumes approved tasks and execution bases. It does NOT create formal requirements, confirm designs, define knowledge authority, or manage repository configuration (that lives in Project Settings).

### 2. Core Product Principles

- Humans own projects; they authorize execution, make key decisions, and give final acceptance.
- AI agents execute authorized tasks; PM-AI coordinates and consolidates.
- Multiple agents may execute different subtasks in parallel; knowledge-base writes always go through the unified PM-AI knowledge process.
- MCP is the communication interface between agents and the platform — never shown or described as an execution runtime.
- Execution must be observable, auditable and traceable: events, commits, tests, evidence.
- Waiting and failure states are first-class citizens, clearly communicated with reasons.
- Execution environments have a lifecycle: Device identity + readiness (installed / connected / authenticated / ready / degraded / failed). A task must not be dispatched to an unavailable device.
- Permissions follow the chain: Team Membership → Project Membership → Device authorization → Task execution authorization.

### 3. Global Workspace Layout

Desktop-first SaaS, canvas 1440×1000. Same global frame as all other modules (S04 pages).

**Top navigation**: product logo · team selector · project selector · global navigation · notifications · user profile.

**Project navigation** (left): Project Command Center / Product · Requirements / Product Cognition / Project Cognition / Design / Tasks / **AI Execution (selected)** / Knowledge Base / Project Settings.

**Secondary navigation inside AI Execution**: Overview / Collaboration / Interventions / Deliveries / History.

**Main content**: execution center sections, execution detail, collaboration graph, intervention workbench, delivery review, history tables.

**Right context panel** (320px, collapsible): AI Project Lead — current execution context, task/requirement/design references, agent activity digest, risks, human actions. Identical placement and styling across all modules.

**Bottom AI command bar**: persistent natural-language input, context-aware (e.g. "Which executions are blocked and why?").

Compact information density, clear hierarchy, subtle borders, neutral surfaces, restrained accent. No oversized KPI dashboards, decorative illustrations, gradients, or chat-centric layouts.

### 4. Domain Model — Do Not Conflate

The UI must always be explicit about which of these it is showing:

| Concept | Meaning | Visual treatment |
|---|---|---|
| **Project Task** | The engineering work item (13-state lifecycle: draft, ready, claimed, running, awaiting_decision, blocked, submitted, reviewing, completed, changes_requested, failed, cancelled, superseded) | Task status badge |
| **Execution Run** | One execution attempt of a task (7 states: queued, claimed, running, succeeded, failed, timed_out, cancelled; attempt_no per kind) | "Run #N" chip + run status |
| **Agent** | An AI worker (role + Device carrier with readiness) | Agent badge (name + role + device dot) |
| **Subtask** | A decomposed unit inside collaboration view | Node in dependency graph |
| **Delivery** | The output of an execution: per-repo commits, test evidence, report, checklist | Delivery card |
| **Review** | Human verdict on a delivery: approved / changes_requested / inconclusive | Review verdict badge |

- One task → many runs (retries create new attempts; history is never overwritten).
- One execution may involve multiple agents (collaboration view).
- An execution can complete while delivery review is still pending.
- Agent waiting is `awaiting_decision` + a waiting reason (**five values**: needs_info / needs_decision / needs_approval / external_dependency / no_executor_available) — never a "WAITING_FOR_X status". (`no_executor_available` = no ready execution environment; it is the reason shown when a task waits for an available device.)
- **Three-stage completion separation** (must be visually distinguishable everywhere): ① Agent claims completion (run succeeded, delivery submitted) → ② AI-PM verification passes (manifest reconciliation + review verdict) → ③ Human formally accepts (task completed). Only ③ changes project delivery status. Review verdict has exactly three values — approved / changes_requested / inconclusive; "not yet reviewed" is an empty verdict, not a fourth value.

### 5. Screens

Seven screens (page-level prompts in this directory):

1. **01 Execution Center** — daily landing: human attention queue, currently executing, execution queue, recent deliveries, activity timeline.
2. **02 Execution Detail** — one task's execution: timeline, current agent activity, task context, tabs (Overview / Timeline / Agent Activity / Logs / Changes / Deliverables).
3. **03 Multi-Agent Collaboration** — parallel execution graph: subtasks, dependencies, agents, progress, blocked/failed work.
4. **04 Human Intervention** — intervention workbench: clarification answers, blocked resolution, failure decisions, permission requests.
5. **05 Execution Authorization** — controlled authorization flow consuming an S04 handoff: scope, repositories, proposed agents, device readiness, risks.
6. **06 Delivery Review** — evidence-based review: execution summary, code changes, test evidence, execution report, checklist; Accept / Request Changes / Reject.
7. **07 Execution History** — searchable execution records and past reviews.

### 6. AI Interaction Design

AI Project Lead is a coordinator, not a chatbot.

- Execution events and agent reports are structured cards with sources — raw logs are never the default view (logs live in a tab, with filters and severity).
- AI proposals (retry plans, scope adjustments, risk notes) carry reasoning + references; humans confirm material actions.
- Human control points are explicit: **Authorize / Answer / Decide / Retry / Accept** — each guarded, each with disabled-state reasons.
- Waiting items surface the agent's actual question with its supporting context, so a human can answer without leaving the flow.

### 7. Visual Design Requirements

Locked to the V2 design system **"Slate & Muted Teal"** (`docs/design/pm-ai-design-tokens-v2.css` — single source of truth).

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

Execution status mapping: Ready → `#8DC5AF` · Running/In-progress → Info `#6EA8FE` · Waiting/Awaiting human → Warning `#E0B15E` · Failed/Blocked → Danger `#F07E88` · Completed/Accepted → Success `#34C08B` · Neutral (queued/cancelled/draft) → secondary text grey `#A8BBC9` + Elevated fills. AI-generated content = Info tint + "AI" badge.

Typography: Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11; tabular numerals for IDs, SHAs, durations.

**No purple, no gradients anywhere.** Avoid agent avatars as decoration, KPI-only dashboards, chat-first layouts, and log-dump interfaces.

### 8. Interaction & Prototype Requirements

Connected prototype covering four journeys:

1. **Journey A (main loop)** — Approved Task → Authorization (05) → Execution (01/03) → Delivery Submission → Human Review (06) → Acceptance.
2. **Journey B (clarification)** — Execution → agent raises question → Waiting (04) → human answers → execution resumes as a continuation of the same run.
3. **Journey C (failure & retry)** — Parallel execution (03) → one agent fails → intervention (04) → retry (new run attempt) → delivery.
4. **Journey D (return for changes)** — Delivery Review (06) → Request Changes with feedback → new execution attempt → re-review.

Required UI states (each visually distinct): no executions · ready for authorization · running · parallel agents · waiting for clarification · device unavailable/not ready · execution failed · delivery awaiting review · delivery accepted · delivery returned for changes.

### 9. Final Deliverables

Seven high-fidelity desktop screens + reusable components: global navigation, execution status badges (Task/Run/Agent three-layer), event timeline, agent activity cards, dependency graph, intervention forms, delivery evidence sections, review verdict controls, history table.

All screens read as one coherent enterprise AI R&D product. The design must communicate:

**S04 organizes and prepares work. S05 executes it under human governance: agents do the engineering, humans authorize, decide and accept.**
