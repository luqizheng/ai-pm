# AI-05 · AI Execution Control（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：让 AI 执行过程可见、可授权、可暂停和可追踪（DR-C2：授权 ≠ 审批）。

## Figma Prompt

Create AI Execution Control using the same four-region layout (188 / 518 / 446 / 288) and the locked V2 dark visual system. Execution status colors follow the Global mapping (Running → Info #6EA8FE, Waiting/Awaiting human → Warning #E0B15E, Failed/Blocked → Danger #F07E88, Completed → Success #34C08B).

### Main Area

Display Task Planning.

Example:

"Break down the approved Design into implementation Tasks."

Show:
- Target Design
- Related Requirements
- Proposed Tasks
- Dependencies
- Acceptance Criteria

### AI Chat

Display Execution Card.

Example:

"I prepared five implementation Tasks based on the approved Design and confirmed Requirements."

### Context Panel

Actions tab active.

Display:

Execution Plan:
- Plan ID
- Target Project
- Input Objects
- Execution Scope
- Required Permissions

Execution Steps:
1. Read approved Design.
2. Verify Requirements.
3. Generate Task breakdown.
4. Validate dependencies.
5. Create Tasks.

### Authorization

Before execution display:
- Requested Operation
- Target Objects
- Required Permission
- Expected Changes
- Current Authorization State

Provide:
- Authorize
- Reject
- Cancel

### Mandatory Approval Boundary

Differentiate:
1. Authorization to execute an operational task.
2. Approval of a formal business change.

An execution authorization does not replace a required formal human approval.

If execution includes protected formal business changes, the mandatory human approval gate remains in force.

### Execution States

- Queued
- Running
- Completed
- Failed
- Blocked
- Cancelled

### Execution Results

Display:
- Created Tasks
- Updated Objects
- Failed Operations
- Logs
- Result References

Do not display successful completion before actual completion.

### Prototype

- Generate execution plan.
- Inspect plan.
- Request authorization.
- Simulate permission denial.
- Authorize execution.
- Show running.
- Simulate partial success.
- Simulate failure.
- Retry eligible step.
- Show final result.
