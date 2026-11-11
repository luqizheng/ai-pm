# AI-07 · Conversation Continuity（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：保证 Conversation、Project、Context 和历史执行结果之间的连续性（DR-C6：会话必须绑定 Project）。

## Figma Prompt

Create Conversation Continuity inside the same AI-01 four-region layout (188 / 518 / 446 / 288), using the locked V2 dark visual system.

### Main Area

Allow switching between:
- Project Overview
- Requirement Detail
- Design Detail
- Task Detail

### AI Chat

Display ongoing conversation.

Example:

User:
"Check whether this Design covers the Requirement we discussed earlier."

AI:
"I retained the previous Requirement reference. Please attach the current Design to continue the comparison."

### Context Panel

Context tab active.

Display:

Current Conversation:
- Conversation ID
- Project ID
- Conversation Title
- Last Activity
- Context Status

Attached Objects:
- REQ-001
- DESIGN-004

Available Objects:
- TASK-021
- ADR-003

### Conversation Rules

Every Conversation must have a Project binding.

Do not create a Conversation without a Project.

### Global AI Entry

When no Project is selected:

Show Project Selection.

Options:
- Open Recent Project
- Select Project
- Create New Project

Only after Project selection can the user create or restore a Conversation.

### Project Switching

Display:

"You're switching to another Project."

Options:
1. Keep Current Conversation and Start a New One in the Target Project.
2. Cancel.

The existing Conversation remains bound to its original Project.

Do not transfer Context silently.

### Context Continuity

When switching Main Area:
- Preserve Conversation.
- Preserve explicitly Attached Objects.
- Add the new page object to Available.
- Do not automatically attach the new object.

### Historical Context

When viewing an old AI result:
- Display its original input context snapshot.
- Display original object revisions.
- Display historical source references.
- Indicate if the source is now outdated.

Do not replace historical context with current Context.

### History Drawer

Provide:
- Conversation History
- AI Action History
- Context Change History
- Approval History
- Execution History

History is accessed through a secondary entry, never a fifth primary Tab.

### Prototype

- Switch Main Area.
- Preserve Conversation.
- Attach new object.
- Remove object.
- Create new Conversation.
- Restore archived Conversation.
- Open History Drawer.
- Inspect historical Context.
- Switch Project.
- Verify Project isolation.
