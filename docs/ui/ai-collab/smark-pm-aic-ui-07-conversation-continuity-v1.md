# AI-07 · Conversation Continuity

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：确保用户在切换业务页面、切换上下文、恢复历史对话时，不会丢失工作状态。

## Figma Prompt

Create a Conversation Continuity experience inside the existing Smark PM four-region workspace.

Reuse exactly the same AI-01 layout and locked V2 dark visual system.

### Main Area

Display different Project business pages:
- Project Overview
- Requirement Detail
- Design Detail
- Task Detail

The user should be able to switch between these pages.

### AI Chat Assistance

Display an ongoing conversation.

Example:

User:
"Check whether this Design covers the Requirement we discussed earlier."

AI:
"I have retained the previous Requirement reference. Please attach the current Design to continue the comparison."

The conversation should remain visible when Main Area changes.

### Right Context Panel

Context tab active.

Display:

Current Conversation:
- Conversation ID
- Project ID
- Conversation Title
- Last Activity
- Current Context Status

Attached Objects:
- REQ-001
- DESIGN-004

Available Objects:
- TASK-021
- ADR-003

Conversation history is accessed through the History Drawer (secondary entry), not a fifth tab.

### Conversation Management

Provide:
- New Conversation
- Rename Conversation
- Pin Conversation
- Archive Conversation
- Restore Conversation
- View Conversation History

### Switching Project

When the user switches Project, show a clear confirmation dialog:

"You're switching to another Project. What would you like to do with the current conversation?"

Options:
1. Keep Current Conversation and Start a New One.
2. Switch Project and Create a New Conversation.
3. Cancel.

Never silently move the current conversation to another Project.

### Context Continuity

When the user switches Main Area pages:
- Preserve the Conversation.
- Preserve explicitly Attached Objects.
- Add the new page object to Available Context.
- Do not automatically attach every newly opened object.
- Allow the user to attach it explicitly.

### Historical Execution Context

When viewing an old AI result, display the exact input context snapshot used at that time.

Do not replace historical context with current context.

### Prototype Requirements

1. Switch Main Area page.
2. Verify Conversation remains.
3. Attach a newly opened business object.
4. Remove an existing attached object.
5. Create a new Conversation.
6. Restore an archived Conversation.
7. Switch Project and display confirmation dialog.
8. Open an old AI result and inspect its historical context.
