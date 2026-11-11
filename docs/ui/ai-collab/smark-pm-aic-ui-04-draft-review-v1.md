# AI-04 · Draft & Review

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：建立 AI 建议转化为正式业务对象的审核流程。核心原则：AI 负责提出和准备，用户负责正式确认。

## Figma Prompt

Create a Draft and Review experience inside the existing Smark PM four-region workspace.

Reuse the exact AI-01 global layout and locked V2 dark visual system.

### Main Area

Display Requirement Editor.

Example:
REQ-001 · Project Initialization

The user is reviewing AI-generated modifications.

Show:
- Requirement Title
- Description
- Acceptance Criteria
- Business Rules
- Exception Handling

### AI Chat Assistance

Example conversation:

User:
"Please improve the acceptance criteria and exception handling."

AI:
"I prepared an updated draft. Please review the changes."

Display a Proposal Card.

### Main Area Editing Experience

Provide a structured comparison:

Original Version | Proposed Version

Highlight:
- Added Content
- Modified Content
- Removed Content

Allow inline editing.

### Right Context Panel

Results tab active.

Display:

Draft Information:
- Draft ID
- Target Object
- Based-on Version
- Created By AI
- Creation Time
- Current Status

Source References:
- Related Requirement
- Related Design
- Related Knowledge
- Related Decisions

### Review Actions

Provide:

1. Accept Draft
2. Modify Draft
3. Reject Draft
4. Submit for Confirmation

After Submit for Confirmation:
- Display confirmation summary.
- Display the approving user's identity.
- Require explicit confirmation.

After successful approval:
- Update the official object version.
- Display the new Revision.
- Preserve the previous Revision.
- Update the corresponding Context Panel.

### Important Business Rules

AI cannot approve its own proposal.

Accepting a suggestion is not equivalent to publishing an official Requirement.

Formal confirmation requires an authorized human user.

Rejecting a proposal must preserve the rejection record.

Concurrent modification must trigger a version conflict warning.

### Prototype Requirements

1. Select a proposed change.
2. Edit its content.
3. Accept the draft.
4. Submit for confirmation.
5. Confirm the operation.
6. Show successful Revision creation.
7. Simulate version conflict and provide a reload/compare option.
