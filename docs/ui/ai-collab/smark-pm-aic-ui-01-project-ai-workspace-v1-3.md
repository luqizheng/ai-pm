# AI-01 · Global Workspace Shell（v1-3 · 第一阶段）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **阶段**：第一阶段第 1 步。**AI-01 通过验收后，才允许复制其 Shell 到 AI-02 及其余五个页面。**
> **裁决依据**：[95 裁决记录](smark-pm-aic-95-decision-record-v1.md)（DR-C1~C8）· 视觉 V2 深色锁定（DR-C1）· 四栏结构升级（DR-C8）。

## Figma Prompt

Create a professional, compact, AI project collaboration desktop workspace for Smark PM.

Use a 1440 × 1000px canvas with the locked V2 "Slate & Muted Teal" dark design system from the Global Design Prompt (canvas #171C24, sidebar #1D2631, surface #222A34, elevated #2B3541, border #374452, text #E7EDF2 / #A8BBC9, accent teal #75A9A4; no purple, no gradients).

Visual direction: Professional B2B SaaS · restrained and clean · high information density · clear hierarchy · neutral background · minimal decorative elements · no oversized cards · no excessive whitespace.

AI Chat is a **persistent resident column**, not a temporary popover and not a bottom command strip. The Context Panel is the visual management area of AI's current working context, not a generic settings panel.

### Shell structure

Keep the existing 52px TopBar spanning the top of the work area. The four-region layout applies to the work area **below** the TopBar.

| Region | Width | Responsibility |
|---|---:|---|
| Navigation | 188px | Global navigation, project entry |
| Main Area | 518px | Current business page and primary work content |
| AI Chat | 446px | Resident AI conversation |
| Context Panel | 288px | Context, Sources, Results, Actions |

Total = 1440px.

Layout constraints:
- Do NOT add fixed margins or gutters between the four regions that are not counted in these widths. Establish separation with internal padding, dividers, and content spacing.
- Do NOT shrink AI Chat or Context Panel to squeeze the Main Area.
- Each region scrolls independently. The page must never shift or misalign because one region has long content.
- The TopBar, the four regions, and the V2 visual system are **inherited from the existing app shell** — this is a structural upgrade of the work area, not a visual redesign of the application.

### Navigation — 188px

Display the grouped navigation (matches the shipped app shell navigation tree):

Group PERSONAL:
- Personal Home
- Inbox

Group WORKSPACE:
- Dashboard
- Projects

Group MODULES (visible only when a Project is selected):
- Discovery
- Requirements
- Technical Design
- Task Management
- Execution
- AI Assistant
- Knowledge
- Dev Environment

Group ADMINISTRATION (collapsed by default):
- Settings
- Members
- Announcements
- AI Providers

Show the current Project context explicitly in the navigation when a Project is selected. Show current user information at the bottom.

The AI Assistant entry opens the AI Collaboration area. In V1 the user MUST select a Project first — never create a Conversation without a Project binding.

### Main Area — 518px

Primary business region for the current page. In AI-01 display:
- Current Project name
- Current Conversation title
- Conversation status
- Current working content
- Business content placeholder for the current page

The Main Area must not be squeezed by AI Chat content and must not carry Context Panel management responsibilities.

**Focus Mode** — 518px is the normal working width, but complex editing is not permanently constrained:

Provide a visible, keyboard-reachable control to enter Focus Mode. In Focus Mode:
- AI Chat and Context Panel collapse, expanding the business work area.
- The four-region layout is restored on exit.
- The current Project, Conversation, and Context are unchanged.
- In-progress edits and conversation state are not lost.

### AI Chat — 446px, resident column

Persistent AI conversation column containing:
- Conversation Header
- Message Timeline
- AI Response Cards
- User Input Composer (message input, attachment, context reference, Send / Stop / Retry)

The Composer sits permanently at the bottom of the AI Chat column — it is the existing bottom command strip promoted into this column, no longer docked to the Main Area.

The AI Chat column coexists with the business page: entering a Project keeps the AI Chat in place; the user does not navigate away to a separate conversation page.

Must support these seven UI Response Cards (DR-C4):

| UI Card | Backend Response Envelope |
|---|---|
| Analysis | informational |
| Proposal | proposal |
| Risk | recommendation |
| Question | question |
| Decision | decision |
| Execution | action-request |
| Progress | execution-result |

Artifact is embedded content; it does not add a new main Card type.

`action-request` is only an operation request — it does not mean authorization was granted. AI may propose, draft, and request execution, but it cannot bypass permission control.

### Context Panel — 288px

Exactly four primary tabs pinned at the top:
1. Context
2. Sources
3. Results
4. Actions

Do not add a fifth History tab.

Content of each tab must be readable and operable within 288px. Switching tabs must not re-lay-out the shell.

**Information density rule**: within 288px show only the high-value summary and status (name, type, status and similar key facts). Full details belong in a detail Drawer or expanded view — do not force complete content into the list.

### History Drawer

History is not a fifth primary tab.

Place the History entry in the Context Panel header or an adjacent utility area. Clicking it opens a Drawer from the Context Panel side.

History Drawer contains these categories:
- Conversation
- AI Action
- Context Change
- Approval
- Execution

Interaction requirements:
- Filter by category.
- Browse by time.
- Open a single record detail.
- Clicking a record shows the corresponding event.
- Closing the Drawer returns to the previously active Context tab.
- Opening History does not change the current Conversation.
- History must never overwrite or replace the four primary tabs.

The History Drawer overlays the Context Panel's visual area; it must not change the four-region base widths.

### Responsive rules

1440px is the primary design and acceptance baseline.

At narrower desktop widths:
- Keep the four regions and their responsibilities.
- Allow Navigation to collapse.
- Allow Context Panel to collapse temporarily.
- Never allow AI Chat and Main Area content to overlap.
- Never turn the four Context tabs into separate page entries.

This round prioritizes the 1440px desktop experience; a full mobile workflow is out of scope.

### Prototype

- Switch business pages — Main Area changes only, AI Chat and Context Panel stay mounted.
- Preserve Attached Context across page switches.
- Open an AI result detail from the Results tab.
- Open and close the History Drawer.
- Open the Project Selector from AI Assistant when no Project is selected, then create a Project-bound Conversation.
- Enter and exit Focus Mode — the four-region layout is restored and no state is lost.
- Confirm each region scrolls independently.
