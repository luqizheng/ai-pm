# AI-02 · Context Management（v1-3 · 第一阶段）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **阶段**：第一阶段第 2 步。**复用 AI-01 的完整 Shell**（复制，不重新设计），只深化 Context 管理内容与交互。
> **约束**：不得修改四区域尺寸、主 Tab 结构、History 入口规则。
> **裁决依据**：[95 裁决记录](smark-pm-aic-95-decision-record-v1.md) DR-C3 / C5 / C7 / C8。

## Figma Prompt

Create the Context Management page inside the **exact same AI-01 four-region workspace** (Navigation 188 / Main Area 518 / AI Chat 446 / Context Panel 288, below the 52px TopBar), using the locked V2 "Slate & Muted Teal" dark visual system.

Do not redesign the Navigation, the Main Area shell, the AI Chat column, or the Context Panel chrome. Copy the AI-01 Shell frame and deepen only the Context Panel content and interactions.

### Context model

Context = the working context the AI may use for the current Conversation.

The user must be able to understand at a glance:
- Which information AI is currently using.
- Where each Context item came from.
- Whether each Context item is currently effective.
- How Context affects AI's work.
- Which results were produced from the current Context.
- Which Context-related actions AI executed.

### Context Tab

The Context tab is the main work area of this page.

#### Header
- Context
- Current Conversation
- Context count
- Add Context button

#### Context Overview
Show:
- Active Context count
- Inactive Context count
- Pending Context count
- Current Context last-updated time

**These numbers must be driven by prototype state — never fixed decorative numbers.**

#### Context List — summary only

Within 288px, the list shows **only the high-value summary and status**. Show:
- Context Name
- Context Type
- Current Status (text, not colour alone)
- A one-line summary / source hint
- Action entry

**Do not force complete content into the list.** Summary text, full source description, usage information, timestamps and related-object details belong in the detail Drawer or an expanded row.

Context types:
- Project
- Requirement
- Knowledge
- Document
- Repository
- Conversation
- User Provided

### Context status model — two axes (DR-C7, confirmed)

Status is expressed on **two orthogonal axes** — do not flatten them into one list:

**Axis 1 · Attachment (relation to the current Conversation)**
- **Available** — discoverable, not yet part of this Conversation's working context
- **Attached** — added to this Conversation's working context
- **Read-only** — referenceable but not modifiable (a modifier tag, combinable with Attached)

**Axis 2 · Lifecycle (state of the item itself)**
- **Active** — currently available to the AI
- **Inactive** — exists but not participating in the current work (result of Disable)
- **Pending** — awaiting user confirmation or handling
- **Unavailable** — currently unreadable or unusable

Rules:
- Status must have explicit text, never colour alone.
- **AI actually uses an item only when it is Attached + Active**, and only if permission policy allows and the item is genuinely readable.
- **Pending must never be silently treated as Active.**
- **Unavailable must show the reason** it is unavailable.
- **Disable ≠ Remove.** Disable moves an item to Inactive and it stays in the list; Remove deletes it (with confirmation).
- Status changes must reflect simultaneously in the list, the Overview counts, the detail view, and History.
- An Attached + Active item is what the AI actually uses. Attached + Inactive is attached but switched off.

### Context operations

Support:
- View Details
- Add Context
- Enable
- Disable
- Remove
- View Source

### Add Context interaction

Clicking Add Context opens a selection panel.

Source categories:
- Project Information
- Requirements
- Knowledge
- Documents
- Repositories
- Previous Conversation
- User Provided Content

Each candidate shows:
- Name
- Source type
- Short summary
- Whether it is already added
- Whether it is available

Multi-select is supported.

On Add:
- Selected items enter the Context List.
- Overview counts update.
- The add result is shown.
- **No other existing Context item changes state.**

If a candidate is unavailable, the reason must be stated explicitly.

### Context Details

Clicking a Context item opens a **detail Drawer** (the 288px list carries summaries only). The Drawer shows:
- Context Name
- Type
- Source
- Description
- Current Status (both axes)
- Added At
- Last Updated
- Related Object
- Usage Information

Enable, Disable, and Remove are available from the Drawer.

**Do not mix the Context item's own source information with the Context item itself.**

### Sources Tab

Shows the information sources AI references in its current work.

Each Source shows:
- Source Name
- Source Type
- Source object
- Citation location or content summary
- Referenced time
- Relation to the current Conversation

Distinguish:
- Available Source
- Referenced Source
- Unavailable Source

A Source is a record of an information origin — it is **not** the same thing as Context status.

### Results Tab

Shows the results produced in the current Conversation:
- AI Analysis
- Proposals
- Drafts
- Documents
- Decisions
- Execution Results

Each Result shows:
- Result Type
- Title
- Status
- Created Time
- Related Context
- Related Source

Clicking a Result opens its detail.

**Never present all AI output as formal knowledge or already-approved content.**

### Actions Tab

Shows the actions related to the current Conversation:
- Context Added
- Context Removed
- Context Enabled
- Context Disabled
- AI Requested Action
- User Approved Action
- Action Rejected
- Execution Completed
- Execution Failed

Each Action shows:
- Action Name
- Initiator
- Status
- Time
- Related Object
- Details

Must distinguish an AI request, a user authorization, and an actual execution result.

### History Drawer — full interaction

The History entry stays visible across all four tabs. Clicking it opens the Drawer.

Five record categories:
1. Conversation
2. AI Action
3. Context Change
4. Approval
5. Execution

Each record shows:
- Event Name
- Event Category
- Actor
- Timestamp
- Related Object
- Event Status

Clicking an event shows its detail:
- What happened
- Who triggered it
- Which object was affected
- State before the operation
- State after the operation
- Related Conversation / Context / Action

Closing the Drawer restores the previously active tab and scroll position.

### Prototype

- Add objects.
- Remove objects.
- Preview objects in the detail Drawer.
- Change reference version.
- Inspect permission.
- Navigate to Sources.
- Open History Drawer.
- Switch Main Area page without losing Attached Context.
- Enable / Disable a Context item and watch the list, counts, and detail view update together.
- Enter and exit Main Area Focus Mode — Context, Conversation, and attached items must be unchanged.

Important:
- Available does not mean Attached.
- Read-only does not mean unavailable.
- Every AI execution must preserve the context snapshot it actually used.
- Disable is reversible; Remove requires confirmation.
- Lists carry summaries; full details live in the Drawer.
