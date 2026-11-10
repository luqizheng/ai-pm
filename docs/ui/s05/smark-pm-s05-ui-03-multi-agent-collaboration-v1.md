# S05-UI-03 — Multi-Agent Collaboration

> - 记录日期：2026-11-10
> - 状态：已落库，待执行（重点迭代页 ②，第二轮单独深化交互）
> - 领域：S05 · AI Agent Execution & Collaboration（页面 3/7，核心差异化）
> - 关联：S05-00 总提示词、S05-UI-01/02/04、S04-UI-05 Dependency Graph（上游规划视图）、V2 设计令牌
> - 使用方式：整份复制投喂 Figma AI，独立可执行

---

# Figma AI Prompt — S05-03 Multi-Agent Collaboration

## 1. Design Objective

Design the Multi-Agent Collaboration view: a dependency-oriented execution picture of one decomposed task being executed by several agents in parallel.

It must make immediately obvious: who is working on what, which subtasks run in parallel, which are blocked by dependencies, which failed, and where human attention is needed. It is an execution coordination view — NOT a chat window between agents, NOT an org chart of cute robot avatars.

Boundary: this view shows execution coordination only. Agents never write to the authoritative project knowledge base independently — knowledge updates go through the unified PM-AI knowledge process (show knowledge writebacks as platform-mediated events, e.g. "knowledge update proposed by Test Agent → pending review", never as direct agent writes).

## 2. Global Layout

1440px desktop. Same global frame as S05-UI-01 §2. Secondary nav **Collaboration (selected)**. Reachable from Execution Center (collaboration chip on a running task) and Execution Detail (agent activity → open graph).

## 3. Page Header

- Title: parent task title + ID (`T-060 Checkout revamp`) · Run chip · overall progress.
- Summary chips: Subtasks (5) · Running (3) · Waiting (1) · Failed (1) · Completed (0) · Agents (4).
- Filters: state (Ready / Running / Waiting / Failed / Completed) · agent · repository.
- Actions: **Intervene** (→04) · **Open Execution Detail** (→02).

## 4. Low-Fidelity Wireframe

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Top bar / project nav / AI Execution secondary nav                           │
├───────────┬──────────────────────────────────────────────┬───────────────────┤
│ Project   │ T-060 Checkout revamp  Run#1  ▓▓░░ 45%       │ AI Project Lead   │
│ nav       │ 5 subtasks · 3 running · 1 waiting · 1 failed│ ───────────────── │
│ AI Exec   ├──────────────────────────────────────────────┤ · T-060c blocked  │
│ Collab    │ DEPENDENCY EXECUTION GRAPH (left→right lanes)│   by T-060b fail  │
│ (here)    │                                              │                   │
│           │ ┌T-060a Architecture ✅ done──────────┐      │ · suggest retry   │
│           │ │ Arch Agent · 12m · commits 1        │      │   T-060b after    │
│           │ └───────┬──────────────┬──────────────┘      │   fix lands       │
│           │ ┌T-060b Backend API ⛔ failed─────────┐      │                   │
│           │ │ Backend Agent · Run#1 exit_code 1   │      │                   │
│           │ │ [Decide: Retry / Modify / Cancel]   │      │                   │
│           │ └───────┬──────────────┘  ┌T-060c Frontend⏸ waiting┐       │
│           │         │                 │ blocked by T-060b     │       │
│           │ ┌T-060d Test harness ▶ running════╗                            │
│           │ │ Test Agent · 35% · in parallel  ║ ┌T-060e Review ○ ready    │
│           │ └─────────────────────────────────╝ │ after a,b,c,d done      │
│           ├──────────────────────────────────────────────┤                   │
│           │ AGENT LANES (right or below, synchronized)   │                   │
│           │ Backend ▶▶▶⛔  Frontend ⏸  Test ▶▶▶▶▶  Arch ✅│                   │
│           ├──────────────────────────────────────────────┤                   │
│           │ COORDINATION EVENTS: conflict guard, lock notes, knowledge     │
│           │ writeback proposals (platform-mediated)                        │
├───────────┴──────────────────────────────────────────────┴───────────────────┤
│ AI command bar                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

## 5. Collaboration Graph (main area)

- Left-to-right dependency lanes (topological): each node = subtask card.
- Node card: subtask ID + title · **state badge** (Ready `#8DC5AF` / Running Info `#6EA8FE` / Waiting Warning `#E0B15E` / Failed Danger `#F07E88` / Completed Success `#34C08B`) · assigned agent (name + role + device dot) · progress bar · duration · commit count.
- Edges: dependency arrows (blocked-by), animated subtly while predecessor runs; a blocked node shows its blocking predecessor highlighted.
- Failed node: inline decision actions **[Retry] [Modify & Retry] [Cancel]** → deep-link into 04 with the failure preloaded.
- Waiting node: reason (needs_info / needs_decision / needs_approval / external_dependency) + **[Answer]** deep link.
- Parallel runs render as concurrent lanes; a running edge may show a subtle flow. Selection syncs with the agent lane and the side panel.
- Repository tagging on nodes: when two subtasks touch the same repository, show a small shared-repo chip (coordination hint: workspace conflict risk is platform-guarded).

## 6. Agent Lanes

Horizontal lanes per agent, time-synchronized with the graph: segments = working (info) / idle (grey) / waiting (warning) / failed (danger). Lane header: agent name + role + Device readiness dot + current activity micro-copy. Lanes make parallelism legible at a glance without animation.

## 7. Coordination Events

Compact feed under the graph: subtask completed · conflict guard applied (workspace lock note) · command issued/acked (e.g. terminate a stale subtask) · waiting raised/answered · knowledge writeback proposed (platform-mediated, pending review — with link to Knowledge module). Each row: time · source (agent/platform/human) · reference.

## 8. Right Context Panel

AI Project Lead: parallelization health ("Test harness runs ahead of API — consider rebalancing") · blocked-chain explanation ("Frontend waits on Backend API failure") · suggested interventions with reasoning · affected upstream requirement/design links.

## 9. Components

DependencyNode (state + agent + progress + repo chip) · DependencyEdge (incl. blocked highlight) · AgentLane (segments) · LaneHeader · CoordinationEventRow · ParallelSummaryChips · StateFilter · InlineDecisionButtons · SharedRepoChip · KnowledgeWritebackRow.

## 10. Interaction & States

Prototype Journey C: open graph → Backend fails (node turns danger, edge to Frontend highlighted as blocked) → click [Retry] → intervention sheet (04) with failure context → confirm retry → node becomes Running with `Run #2` chip (old attempt kept) → Test Agent completes → Review node becomes Ready. States to render: all-parallel-running · one-failed-with-blocked-chain · waiting-for-clarification (warning node with question preview) · completed (all nodes success, graph static) · empty ("No parallel execution — this task runs as a single execution").

## 11. Visual Style

V2 "Slate & Muted Teal" tokens (truth: `pm-ai-design-tokens-v2.css`): Canvas #171C24 / Surface #222A34 / Elevated #2B3541 / Border #374452 / Text #E7EDF2·#A8BBC9 / Accent #75A9A4 / Info #6EA8FE / Success #34C08B / Warning #E0B15E / Danger #F07E88 / Ready #8DC5AF. Typography 21/15/13/12/11, tabular numerals. Graph on canvas background with thin borders; nodes compact (≤3 lines); edges 1px with restrained arrowheads. **No purple, no gradients, no robot avatars, no gamified animations** — clarity of parallelism and blocking is the whole point.

## 12. Deliverable

Complete high-fidelity page (1440×1000) + connected prototype implementing §10 (focus page for second-round interaction iteration).
