# AI-02 · Context Management

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：让用户明确控制 AI 正在使用什么上下文。这是整个 AI Collaboration 的核心能力。

## Figma Prompt

Create the Context Management experience within the existing Smark PM four-region workspace.

Reuse exactly the same global layout and locked V2 "Slate & Muted Teal" dark visual system from AI-01.

Do not redesign the Navigation, AI Chat column or Main Area shell.

### Main Area

Display a Requirement Detail page.

Example:
- REQ-001
- Project Initialization
- Status: Confirmed

Provide a business object header and content preview.

### AI Chat Assistance

Example conversation:

User:
"Compare this requirement with the existing design."

AI:
"I need to reference the relevant Design and its approved revision."

### Right Context Panel

The Context tab is active.

Create three clearly differentiated object groups:

#### 1. Attached Context

Objects explicitly included in the current AI working context.

Display:
- Object Icon
- Object Name
- Object ID
- Version
- Remove Action

Example:
- REQ-001 · Requirement · v1.0
- KB-003 · Knowledge · v2.1

#### 2. Available Context

Objects discovered from the current Project or current page.

Display:
- Object Name
- Object Type
- Add Button

Example:
- DESIGN-004
- ADR-003
- TASK-021

#### 3. Read-only Context

Objects that can be referenced but cannot be modified through the current operation.

Display a read-only indicator.

### Context Interaction

Provide:
- Attach
- Remove
- Preview
- Pin
- View Version
- Change Reference Version

When the user attaches an object, show a small confirmation notification.

When the user removes an object, update the attached object count.

### Context Selector

Create a searchable object selector.

Filters:
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions

Each result displays:
- Object Name
- Object ID
- Status
- Version
- Permission Indicator

### Important Rules

Available does not mean attached.

Read-only does not mean unavailable.

Attached objects must be version-aware.

Context changes must not silently rewrite previous AI execution records.

The user must be able to inspect the exact context used for each AI result.

### Prototype Requirements

1. Click Add to attach an object.
2. Click Remove to detach it.
3. Click an object to preview its details.
4. Switch Context and Sources tabs.
5. Return to AI-01 without losing the Conversation.
6. Change Main Area page while preserving explicitly attached context.
