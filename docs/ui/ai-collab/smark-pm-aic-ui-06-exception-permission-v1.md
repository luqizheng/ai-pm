# AI-06 · Exception & Permission

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：设计 AI 协作过程中所有异常、权限拒绝、冲突和人工介入场景。这是保证系统可靠性的重要页面。

## Figma Prompt

Create an Exception and Permission Management experience inside the existing Smark PM four-region workspace.

Reuse AI-01 global layout and the locked V2 dark visual system (failure/blocked → Danger #F07E88, waiting for human → Warning #E0B15E).

### Main Area

Display the current business object related to the exception.

Support different examples:
- Requirement Permission Denied
- Context Conflict
- AI Provider Unavailable
- Execution Failed
- Approval Pending

### AI Chat Assistance

AI must explain:
- What happened
- Why it happened
- What is affected
- What can be done next

Avoid generic messages such as "Something went wrong".

### Right Context Panel

Display an exception detail view.

Include:

#### Exception Information
- Exception ID
- Exception Type
- Severity
- Occurred Time
- Related Project
- Related Object

#### Permission Information
- Current User
- Requested Action
- Required Permission
- Current Permission
- Approval Requirement

#### Recovery Information
- Completed Steps
- Failed Step
- Remaining Steps
- Available Recovery Actions

### Exception Scenarios

Scenario A: Context Missing

Show missing context objects and Add Context action.

Scenario B: Context Conflict

Show conflicting versions and Compare Versions action.

Scenario C: Provider Unavailable

Show provider status and Retry action.

Scenario D: Execution Failed

Show failed step, error summary and Retry / Cancel options.

Scenario E: Permission Denied

Show the required permission and Request Authorization action.

Scenario F: Approval Pending

Show pending approver, requested change and approval status.

### Important Rules

Never silently bypass permissions.

Never treat an AI suggestion as user authorization.

Never hide partial execution results.

Never discard recoverable work after failure.

### Prototype Requirements

Create interactive transitions for all six exception scenarios.

Each scenario must have:
- Initial state
- Error state
- Explanation
- Recovery action
- Recovered or terminal state
