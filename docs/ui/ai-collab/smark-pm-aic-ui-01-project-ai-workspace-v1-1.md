# AI-01 · Project AI Workspace（v1-1）

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md) 全文，再投喂本文件。
> **页面目标**：建立完整的四区域主工作界面。**这是其他六个页面的唯一布局母版**——AI-01 完成后复制其 Workspace Shell 到其余六页，不得每页重新生成四栏布局。

## Figma Prompt

Create a professional enterprise SaaS Project AI Workspace for Smark PM.

Use a 1440 × 1000px canvas with the locked V2 "Slate & Muted Teal" dark design system from the Global Design Prompt (canvas #171C24, sidebar #1D2631, surface #222A34, accent teal #75A9A4; no purple, no gradients).

Use the exact four-region layout:

- Navigation: 188px
- Main Area: 518px
- AI Chat: 446px
- Context Panel: 288px

Do not introduce a fifth main region.

### Navigation

Display:
- Smark PM Logo
- Projects
- Current Project
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions
- AI Assistant
- Settings
- Help

### Main Area

Display Project Command Center.

Example:
Smark Platform

Tabs:
- Overview
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions

Show:
- Project Summary
- Current Progress
- Recent Activity
- Open Risks
- Pending Actions

### AI Chat

Example user message:
"Analyze the current project risks and identify the most important unresolved issues."

AI response:
- Summary
- Risk Findings
- Suggested Actions
- Source References

Display structured AI Response Cards.

Composer:
- Message Input
- Attachment
- Add Context
- Send

### Context Panel

Exactly four primary tabs:
Context / Sources / Results / Actions

Context:
- Current Project
- Current Page
- Attached Objects
- Available Objects
- Read-only Objects

Sources:
- Requirements
- Knowledge
- Decisions
- Source Versions

Results:
- AI Findings
- Suggestions
- Pending Questions

Actions:
- Pending Approval
- Requested Actions
- Execution Status

History must be accessed through a secondary History Drawer.

### Global AI Entry

The AI Assistant navigation item must open a Project Selection screen when no Project is selected.

Do not create an unbound Conversation.

### Prototype

- Switch business pages.
- Preserve AI Chat.
- Preserve Attached Context.
- Open AI result details.
- Open History Drawer.
- Select Project before creating a Conversation.
