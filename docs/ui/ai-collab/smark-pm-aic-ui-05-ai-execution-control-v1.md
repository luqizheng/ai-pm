# AI-05 · AI Execution Control

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：将 AI 从分析和建议扩展到受控执行。执行过程必须可见、可授权、可暂停、可追踪。

## Figma Prompt

Create an AI Execution Control experience inside the existing Smark PM four-region workspace.

Reuse the exact AI-01 global layout and locked V2 dark visual system. Execution status colors follow the Global mapping (Running → Info, Waiting/Awaiting human → Warning, Failed/Blocked → Danger, Completed → Success).

### Main Area

Display a Task Planning or Execution page.

Example:

"Break down the approved Design into implementation Tasks."

Show:
- Target Design
- Related Requirements
- Proposed Task List
- Task Dependencies
- Acceptance Criteria

### AI Chat Assistance

Display AI-generated execution plan.

Example:

"I have prepared 5 implementation tasks based on the approved Design and confirmed Requirements."

Show an Execution Card.

### Right Context Panel

Actions tab active.

Display:

Execution Plan:
- Plan Name
- Target Project
- Input Objects
- Execution Scope
- Required Permissions

Execution Steps:
1. Read approved Design.
2. Verify related Requirements.
3. Generate Task breakdown.
4. Validate dependencies.
5. Create Tasks.

### Permission Confirmation

Before execution, display:
- Requested Operation
- Target Objects
- Permission Required
- Expected Changes
- User Confirmation

Provide:
- Authorize
- Reject
- Cancel

### Execution Monitoring

After authorization, show:

- Queued
- Running
- Completed
- Failed
- Blocked
- Cancelled

Show each step individually.

### Execution Results

Display:
- Created Tasks
- Updated Objects
- Failed Operations
- Execution Logs
- Result References

### Important Rules

Do not automatically authorize actions.

Do not hide failed steps.

Do not claim success before the operation succeeds.

Allow retry only for eligible failed steps.

Maintain execution history.

### Prototype Requirements

1. Generate execution plan.
2. Open plan details.
3. Authorize execution.
4. Show running state.
5. Simulate partial success.
6. Simulate one failed step.
7. Retry the failed step.
8. Show final execution report.
