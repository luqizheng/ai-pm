# AI-03 · AI Response Cards（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：建立七类统一 UI Card，并遵循八类后端 Envelope（DR-C4）。本页卡状态是全包状态设计基准。

## Figma Prompt

Create the AI Response Card Library inside the same AI-01 four-region layout (188 / 518 / 446 / 288), using the locked V2 dark visual system.

The Main Area displays Requirement Detail.
The AI Chat displays all seven Card types.

### 1. Analysis Card

Envelope: informational

Fields:
- Title
- Summary
- Findings
- Evidence
- Source References

Actions:
- View Details
- Ask Follow-up
- Open Source

### 2. Proposal Card

Envelope: proposal

Fields:
- Proposal Title
- Proposed Changes
- Original Content
- Proposed Content

Actions:
- Accept into Draft
- Reject
- Modify
- Compare

Never represent Draft Acceptance as Formal Approval.

### 3. Risk Card

Envelope: recommendation

Display only when structured risk information exists.

Fields:
- Severity
- Impact
- Likelihood
- Affected Objects
- Mitigation
- Status

Actions:
- Confirm
- Dismiss
- Create Task
- View Object

### 4. Question Card

Envelope: question

Fields:
- Question
- Reason
- Options
- Custom Answer

Actions:
- Submit Answer
- Skip
- Ask Follow-up

### 5. Decision Card

Envelope: decision

Fields:
- Decision Background
- Alternatives
- Recommendation
- Consequences

Actions:
- Edit
- Submit for Human Confirmation
- Reject

### 6. Execution Card

Envelope: action-request

Fields:
- Execution Goal
- Plan
- Target Objects
- Required Permission
- Expected Changes

Actions:
- Request Authorization
- Reject
- View Details

An action request is not an authorized execution.

### 7. Progress Card

Envelope: execution-result

Display:
- Queued
- Running
- Partially Completed
- Failed
- Completed
- Cancelled

Fields:
- Current Step
- Completed Steps
- Remaining Steps
- Failed Steps
- Last Updated

Actions:
- View Details
- Retry Eligible Step
- Resume Eligible Execution

### Artifact

Artifact is embedded content, not a new main Card type.

Examples:
- Requirement Draft
- Design Document
- Analysis Report
- Execution Report

### Context Panel

Exactly four Tabs.

Results tab displays the selected Card and its source references.

### Prototype

- Open Card.
- Inspect details.
- Accept Proposal into Draft.
- Modify Draft.
- Reject Proposal.
- Create Task from Risk.
- Request Execution Authorization.
- Display Progress.
- Display Failed and Completed states.

Do not expand the eight backend Envelope types.
