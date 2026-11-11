# AI-02 · Context Management（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：让用户管理 AI 当前使用的业务上下文。

## Figma Prompt

Create Context Management inside the same AI-01 four-region layout (188 / 518 / 446 / 288), using the locked V2 dark visual system.

Do not change the global dimensions.

### Main Area

Display Requirement Detail.

Example:
REQ-001 · Project Initialization

Show:
- Requirement Description
- Acceptance Criteria
- Current Version
- Status

### AI Chat

User:

"Compare this Requirement with the existing Design."

AI:

"I need to reference the relevant Design and its approved Revision."

### Context Panel

Context tab active.

Create three object groups.

#### Attached Context

Show:
- Object Name
- Object ID
- Object Type
- Revision
- Remove Action

Example:
- REQ-001 · Requirement · v1.0
- KB-003 · Knowledge · v2.1

#### Available Context

Example:
- DESIGN-004
- ADR-003
- TASK-021

Provide Add action.

#### Read-only Context

Display objects that can be referenced but cannot be modified.

### Object Selector

Search and filter by:
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions

Each item shows:
- Name
- ID
- Status
- Version
- Permission

### History

History is not a primary Tab.

Provide View Context History through the History Drawer.

### Prototype

- Add object.
- Remove object.
- Preview object.
- Change reference version.
- Inspect permission.
- Navigate to Sources.
- Open History Drawer.
- Switch Main Area page without losing Attached Context.

Important:
- Available does not mean Attached.
- Read-only does not mean unavailable.
- Every AI execution must preserve the context snapshot it actually used.
