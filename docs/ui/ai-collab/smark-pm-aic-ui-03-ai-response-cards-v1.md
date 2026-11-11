# AI-03 · AI Response Cards

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：建立统一的 AI 输出组件体系。不要让所有 AI 输出都表现为普通聊天文本。不同类型的 AI 输出应该有不同的交互 Card。
> **词表**：卡名 ↔ 信封类型映射见 00 §0.5；本页卡状态是全包状态设计基准。

## Figma Prompt

Create an AI Response Component Showcase inside the existing Smark PM four-region workspace.

Reuse AI-01 global layout and the locked V2 "Slate & Muted Teal" dark visual system.

The Main Area should display a Requirement Detail page.

AI Chat should contain multiple AI response types.

### Card Type 1 · Analysis Card

Display:
- Analysis Title
- Summary
- Key Findings
- Evidence References
- Confidence / Completeness Indicator

Actions:
- View Details
- Ask Follow-up
- Open Source

### Card Type 2 · Proposal Card

Display:
- Proposal Title
- Proposed Changes
- Original Content
- Proposed Content
- Change Highlights

Actions:
- Accept
- Reject
- Modify
- Compare

Accept means accept into a working draft, not automatically publish.

### Card Type 3 · Risk Card

Display:
- Risk Title
- Severity
- Impact
- Likelihood
- Related Objects
- Recommended Mitigation

Actions:
- Confirm
- Dismiss
- Create Task
- View Related Object

### Card Type 4 · Question Card

Display:
- AI Question
- Why This Information Is Needed
- Multiple Choice Options
- Custom Answer Input

Actions:
- Submit Answer
- Skip
- Ask AI

### Card Type 5 · Decision Card

Display:
- Decision Title
- Decision Background
- Alternatives
- Recommendation
- Consequences

Actions:
- Edit
- Submit for Human Confirmation
- Reject

### Card Type 6 · Execution Card

Display:
- Execution Goal
- Execution Plan
- Required Permission
- Steps
- Execution Status

Actions:
- Authorize
- Pause
- Cancel
- View Execution

### Card Type 7 · Progress Card

Display:
- Current Step
- Completed Steps
- Remaining Steps
- Blocked Items
- Last Updated

Actions:
- View Details
- Retry
- Resume

### Right Context Panel

Results tab active.

Show:
- Current AI Result
- Result Type
- Related Objects
- Source References
- Result Status

### Visual Rules

Each Card must have a consistent visual structure: same header (card type badge + AI badge + status), same body rhythm, same footer action row.

Use subtle color indicators per the Global status mapping:
- Information → Info tint #6EA8FE
- Proposal → Accent teal tint #253735
- Warning → Warning #E0B15E
- Pending Approval → Warning
- Execution → Info
- Completed → Success #34C08B
- Failed → Danger #F07E88

Avoid excessive colors. All AI-generated cards carry the "AI" badge.

### Prototype Requirements

1. Click a Card to open detailed result in Context Panel.
2. Click Accept on Proposal to create a Draft state.
3. Click Modify to open editable draft.
4. Click Reject to display a rejection reason dialog.
5. Click Create Task on Risk Card to open task creation draft.
6. Click Authorize on Execution Card to open permission confirmation.
7. Every interaction updates the corresponding Card state.
