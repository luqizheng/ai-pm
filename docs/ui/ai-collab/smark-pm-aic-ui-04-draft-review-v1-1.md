# AI-04 · Draft & Review（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：建立 AI Proposal → Draft → Human Approval → Official Revision 的正式流程（DR-C2 强审批门）。

## Figma Prompt

Create Draft and Review inside the same AI-01 four-region layout (188 / 518 / 446 / 288), using the locked V2 dark visual system.

### Main Area

Display Requirement Editor.

Example:
REQ-001 · Project Initialization

Provide:
- Title
- Description
- Acceptance Criteria
- Business Rules
- Exception Handling

Show Original and Proposed versions.

Because Main Area is 518px wide, use:
- Inline Diff
- Unified Diff
- Single-column Original / Proposed switch
- Expandable comparison view

Do not force a side-by-side comparison.

### AI Chat

Display Proposal Card.

Example:

"I prepared an updated Requirement draft. Please review the changes."

### Context Panel

Results tab active.

Display:
- Draft ID
- Target Object
- Based-on Revision
- Created By AI
- Current Status
- Source References

### Draft Workflow

```text
AI Proposal
    |
    v
Draft Created
    |
    v
User Modification
    |
    v
Draft Accepted
    |
    v
Submit for Confirmation
    |
    v
Approval Pending
    |
    +------> Rejected
    |
    v
Human Approval
    |
    v
Official Revision Created
```

### Mandatory Approval Gate

Display a prominent Approval Pending panel.

Example:
"Human Approval Required"

Show:
- Target Requirement
- Proposed Revision
- Changes
- Approver Permission
- Approval Status

Before formal confirmation:
- No official approval.
- No official Revision publication.
- No success state.

Only an authorized human can approve.

AI cannot approve its own Proposal.

### Permission Enforcement

If the user lacks approval permission:
- Display Permission Denied.
- Disable the formal approval action.

If approval is pending:
- Block all transitions that would publish the official object.

### Concurrent Modification

Simulate version conflict.

Display:
- Current Version
- Draft Base Version
- Conflicting Changes

Provide Compare and Reload actions.

### Prototype

- Create Draft.
- Modify Draft.
- Submit for Confirmation.
- Display Approval Pending.
- Attempt unauthorized approval.
- Display Permission Denied.
- Complete authorized human approval.
- Create official Revision.
- Update Main Area and Context Panel.
