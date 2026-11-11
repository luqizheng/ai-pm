# AI-01 · Project AI Workspace

> **前置**：先投喂 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md) 全文，再投喂本文件。
> **页面目标**：建立整个 AI Collaboration Workspace 的主界面。这是七个页面中最重要的基础页面，其他六个页面都必须继承它的布局。

## Figma Prompt

Create a professional enterprise SaaS Project AI Workspace for Smark PM.

Design a desktop application at 1440px width, using the locked V2 "Slate & Muted Teal" dark design system from the Global Design Prompt (canvas #171C24, sidebar #1D2631, surface #222A34, accent teal #75A9A4; no purple, no gradients).

Use a fixed four-region layout (ratio reference 13 / 36 / 31 / 20):

1. Left Navigation
2. Main Project Area
3. AI Chat Assistance
4. Right Context Panel

Do not create any additional main column.

### Left Navigation

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

Header:
- Project Name: Smark Platform
- Project Status: Active
- Project Breadcrumb

Tabs:
- Overview
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions

Overview content:
- Project Summary
- Project Description
- Recent Activity
- Current Progress
- Open Risks

Keep this region focused on business information.

### AI Chat Assistance

Header:
- AI Assistant
- Current Conversation
- New Conversation
- Pin / Minimize

Conversation:
- User Message
- AI Response
- AI Insight Card
- Referenced Business Objects

Bottom Composer:
- Message Input
- Attachment
- Add Context
- Send

Example user message:

"Analyze the current project risks and identify the most important unresolved issues."

AI response should include:
- Summary
- Risk Findings
- Suggested Actions
- References

### Right Context Panel

Tabs:
- Context
- Sources
- Results
- Actions

Context:
- Current Project
- Current Page
- Attached Objects
- Available Objects

Sources:
- Referenced Requirements
- Referenced Knowledge
- Referenced Decisions

Results:
- AI Findings
- Suggestions
- Pending Questions

Actions:
- View Details
- Create Task
- Generate Report

### Prototype Requirements

- Clicking Requirements changes Main Area only.
- AI Chat remains unchanged.
- Context Panel remains visible.
- Clicking Attach Context opens an object selector.
- Clicking an attached object opens its detail in Main Area.
- AI response cards can be opened in the Results tab.

This page is the global layout foundation for all other AI collaboration pages.
