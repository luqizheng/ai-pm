# PM-AI — Execution Environments（Runner Management）UI/UX Design

> **性质**：用户供稿的 Figma AI 设计提示词原文（英文），逐字转录，未改写、未裁决。
> **供稿时间**：2026-11-08 18:38（GMT+8）。
> **主题**：Execution Environments / Runner 管理模块的 UX/UI 设计依据；覆盖 Overview、Runner Detail、Add Wizard、My Devices、Project Setting → Runners 六类 Figma 交付物与交互状态。
> **关联产物**：`D:/kudi-project/templates/runner-ui-design/`（仓库外，2026-11-07 生成）—— `frame-01-list.html`（执行环境列表 EE-01）、`frame-02-detail-ready.html`（设备详情 / Execution Ready）、`_probe.html`（英文设计探针）。
> **同类文档**：`docs/team-onboarding/team-onboarding-uiux-design-v1.md`（同为整篇一段式 Figma 提示词）；`docs/architecture/ai-pm-mcp-executor-positioning-v1.md`（执行端定位基线）。
> **状态**：仅登记归档；未提交 git，未据此改动任何实现。提示词自身的产品裁定（如 Team 级注册 / Project 级绑定）尚未与代码现状对账。

---

## 1. Product Context

Design the Execution Environments management module for PM-AI, an AI-powered R&D and Project Management platform.

Execution Environments are the actual computing environments where AI coding agents and engineering tools execute development tasks.

Examples include:
- Developer's Windows PC
- Developer's Mac
- Dedicated development machine
- Remote execution machine

Important architecture principles:

1. Runner represents an execution environment, not an AI agent.
2. MCP is a communication interface, not an execution runtime.
3. AI-PM can operate independently of external Runner installations.
4. Runners extend engineering execution capabilities; they do not enable or disable the core project management functionality.
5. A Runner can be registered at Team level and authorized for use by multiple Projects through explicit project bindings.
6. Team-level registration/authorization and Project-level binding/configuration must be clearly distinguished.

## 2. Design Goals

Create a professional enterprise-grade Runner management experience.

The interface should help administrators and developers:

- Understand which execution environments are registered.
- Monitor Runner connectivity and health.
- Inspect available execution capabilities.
- Manage project bindings.
- Register new devices.
- Install and configure Runner components.
- Review execution history.
- Revoke device authorization when necessary.

Use a restrained, professional SaaS design language consistent with PM-AI.

Desktop-first design, compact information density, clear navigation and strong status visibility.

## 3. Information Architecture

Create the following pages:

### Page A — Execution Environments Overview

Display all registered execution environments.

Main elements:

- Page title: Execution Environments
- Short description explaining the purpose of execution environments.
- Add Execution Environment action.
- Runner list/table.
- Search and filtering.

Runner table columns:

- Name
- Device / Machine
- Operating System
- Connection Status
- Health Status
- Available Capabilities
- Project Bindings
- Last Seen
- Actions

Support these states:

- Online
- Offline
- Connecting
- Unhealthy
- Revoked

Provide useful empty states when no Runner has been registered.

Do not use excessive KPI cards. Prioritize the actual Runner list and operational information.

### Page B — Runner Detail

Clicking a Runner opens its detail page.

Organize information into the following sections:

1. Overview
   - Runner name
   - Device identification
   - Operating system
   - Connection status
   - Health status
   - Last heartbeat
   - Registration time

2. Health Check
   - Connection health
   - Runtime availability
   - Last successful health check
   - Error information
   - Diagnostics

3. Capabilities
   - Available execution tools
   - Supported development environments
   - Supported operations
   - Capability availability

4. Project Bindings
   - Projects authorized to use this Runner
   - Binding status
   - Project-specific configuration
   - Add / Remove Project Binding

5. Execution History
   - Recent execution tasks
   - Task ID
   - Project
   - Execution status
   - Start time
   - Duration
   - Error details

6. Authorization
   - Device registration information
   - Authorization status
   - Revoke authorization action

### Page C — Add Execution Environment

Design a guided device registration and installation wizard.

Step 1: Select Environment Type

- Windows
- macOS
- Linux
- Remote Machine

Step 2: Register Device

- Device name
- Environment information
- Registration instructions

Step 3: Installation

Provide installation instructions and the required bootstrap package or command.

Explain how the user installs and starts the Runner on the target machine.

Step 4: Connect and Verify

Show:

- Connection progress
- Registration verification
- Health check
- Capability detection

Step 5: Complete

Show successful registration and provide next steps, including project binding.

The wizard must also support:

- Installation failure
- Connection timeout
- Registration failure
- Permission problems
- Retry and recovery

Do not assume a device is online merely because its installation command was generated.

### Page D — My Devices

Provide a personal device view.

Show devices associated with the current user, their registration status and connection state.

Allow users to inspect their own devices and access relevant installation or troubleshooting instructions.

Keep this view distinct from the Team-wide Runner administration view.

## 4. Key Interaction Requirements

### Runner Registration

- Add Runner opens the registration wizard.
- Registration and installation are separate steps.
- A device becomes available only after successful registration and connection verification.

### Project Binding

- A Team-registered Runner must be explicitly authorized and bound to a Project before that Project can use it.
- Project binding must not be confused with device registration.
- Support multiple Project bindings where permitted.
- Show clear binding status and permission feedback.

### Health Monitoring

- Show online/offline state prominently.
- Distinguish connectivity from actual execution health.
- Provide diagnostic details for unhealthy environments.
- Show the last heartbeat timestamp.

### Revocation

- Revoking authorization requires confirmation.
- Explain the impact on future execution.
- Show the resulting revoked state.
- Distinguish revocation from temporary disconnection.

## 5. Navigation and Layout

Use a consistent PM-AI desktop application shell.

The management area should provide navigation for:

- Execution Environments
- My Devices

Within a Runner detail page, use secondary navigation or tabs for:

- Overview
- Health
- Capabilities
- Project Bindings
- Execution History
- Authorization

Project-specific Runner configuration should also be accessible through:

Project Command Center → Project Setting → Runners

When entering from Project Setting, automatically use the current Project context. Do not ask the user to select the Project again.

## 6. Visual Design Requirements

- Enterprise SaaS product style.
- Professional and restrained.
- Desktop-first, approximately 1440px viewport.
- Consistent sidebar, header, typography and spacing.
- Compact but readable tables.
- Clear status indicators.
- Reusable components.
- Consistent empty, loading, success, warning and error states.
- Avoid excessive decorative illustrations.
- Avoid presenting Runner as a chat interface or AI agent management screen.

## 7. Deliverables

Create a coherent, connected Figma prototype covering:

1. Execution Environments Overview
2. Runner Detail
3. Add Execution Environment Wizard
4. My Devices
5. Project Setting → Runners
6. Important interaction states and error handling

All pages should share consistent components and visual language.

Prioritize functional navigation, realistic sample data and meaningful interaction flows over isolated attractive mockup screens.
