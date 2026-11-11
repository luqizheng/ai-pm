# AI-06 · Exception & Permission（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：建立统一异常处理、权限控制和人工介入体验（DR-C2 默认拒绝 + 强审批门）。

## Figma Prompt

Create Exception and Permission Management inside the same AI-01 four-region layout (188 / 518 / 446 / 288), using the locked V2 dark visual system (failure/blocked → Danger #F07E88, waiting for human → Warning #E0B15E).

### Main Area

Display the affected business object.

Support:
- Requirement Permission Denied
- Context Conflict
- AI Provider Unavailable
- Execution Failed
- Approval Pending

### AI Chat

AI explains:
- What happened
- Why it happened
- What is affected
- What can be done next

Avoid generic error messages.

### Context Panel

Use the four primary Tabs.

When an exception is selected, display its detail in the relevant tab.

Exception details:
- Exception ID
- Type
- Severity
- Time
- Project
- Related Object

Permission details:
- Current User
- Requested Action
- Required Permission
- Effective Permission
- Approval Requirement

Recovery details:
- Completed Steps
- Failed Step
- Remaining Steps
- Recovery Actions

### Six Exception Scenarios

A. Context Missing

Display missing objects and Add Context.

B. Context Conflict

Display conflicting versions and Compare Versions.

C. Provider Unavailable

Display provider status and Retry.

D. Execution Failed

Display failed step and Retry / Cancel.

E. Permission Denied

Display required permission and Request Authorization.

F. Approval Pending

Display approver, requested change and approval state.

### Hard Approval Boundary

Approval Pending must be a blocking state.

AI cannot bypass it through:
- Retry
- Resume
- Auto Accept
- Alternative Action
- Direct API action

The backend remains the final authority for enforcement.

### Prototype

For every exception, demonstrate:

Initial State → Error State → Explanation → Recovery Action → Recovered or Terminal State.

Include explicit unauthorized approval attempts.
