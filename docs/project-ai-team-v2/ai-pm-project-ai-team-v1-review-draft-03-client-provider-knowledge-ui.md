# Project AI Team V1.0 产品需求与架构设计基线（评审稿）· 分篇 3/5：Client 管理、模型策略、知识治理与管理界面（§9–§12）

> 状态：评审稿转录 · 未冻结 · Q2–Q4 待确认（见分篇 4）
> 记录日期：2026-11-08 · 来源与拆分说明见 [00-index](ai-pm-project-ai-team-v1-review-draft-00-index.md)

---

## 9. AI Client Management

这一部分需要严格沿用个人级注册、Project 授权、Platform 动态调度的已确认模型。

### 9.1 Client 的管理生命周期

（原文为流程图，粘贴文本中图形丢失，以下按原稿文字项重建。）

```text
User
  |
  v
Register Agent Client
  |
  v
Personal Client Registry
  |
  v
Authorize Project
  |
  v
Project AI Team Client Pool
  |
  v
Capability Discovery
  |
  v
Availability Monitoring
  |
  v
Task Dispatch
```

### 9.2 Client 的基本信息

| 字段 | 说明 |
| --- | --- |
| client_id | 唯一标识 |
| owner_user_id | 所属用户 |
| client_type | Client 类型 |
| display_name | 显示名称 |
| supported_capabilities | 支持能力 |
| connection_status | 连接状态 |
| last_heartbeat_at | 最后心跳 |
| version | Client 版本 |
| authorization_status | 项目授权状态 |

### 9.3 Project Client Binding

个人 Client 授权给 Project 时，应该单独记录：

- Project
- Client
- 授权人
- 授权时间
- 授权状态
- 可执行范围
- 解除授权时间

个人注册、Project 授权、Task 调度不能合并成一个概念。

### 9.4 Client 状态

建议区分：

| 状态 | 含义 |
| --- | --- |
| REGISTERED | 已注册 |
| CONNECTED | 已连接 |
| OFFLINE | 离线 |
| UNAUTHORIZED | 未授权 |
| DISABLED | 已禁用 |
| REVOKED | 已撤销 |

Client 离线不意味着它的所有历史 Task 都应该失败。

正在执行的 Task 应根据实际执行状态决定恢复、等待或终止。

---

## 10. LLM Provider 与模型策略

这一部分继承 Team LLM Provider Management V1.0 的既有裁决。

已确认：

1. Provider 和凭证由 Team 拥有并管理。
2. Project 只能使用 Team 授权的模型资源。
3. 模型策略按 AI Role 分配。
4. Provider 故障时明确失败并提示，不自动静默切换。

建议结构：

```text
Team
  |
  +-- LLM Providers
  |
  +-- Authorized Models
        |
        v
Project
  |
  +-- Project AI Team
        |
        +-- AI Role
              |
              +-- Model Policy
```

需要注意三个不同的选择：

- Role 决定工作职责。
- Model Policy 决定使用什么模型资源。
- Client Dispatch 决定实际由哪个执行资源承担任务。

它们是三个独立的决策。

对于没有外部 Client 的项目，AI Project Manager 仍然使用平台内置 AI 能力。

---

## 11. 知识写入与治理机制

这里必须继承 PM-AI 已有的权威知识模型。

建议形成一个统一流程：

```text
Agent Analysis
      |
      v
Proposed Changes
      |
      v
Platform Validation
      |
      v
Knowledge Coordination
      |
      v
Human Approval (when required)
      |
      v
Authoritative Domain Update
```

不能让外部 Client 直接修改正式权威知识并绕过业务规则。

不同领域分别负责自己的权威内容：

| Domain | 权威对象 |
| --- | --- |
| Product Cognition | 产品定位、能力、业务规则等 |
| Project Cognition | 项目技术架构、技术约定等 |
| Requirement | 正式需求 |
| Design | 正式设计 |
| ADR | 架构决策 |
| Knowledge | 项目知识文档 |
| Task | 任务状态和执行记录 |

AI Team 统一协调，但不统一吞并这些 Domain。

---

## 12. Project AI Team Management UI

建议采用独立的项目设置入口，而不是把 AI Team 管理做成 Project Command Center 的常驻主导航。

入口：

```text
Project Settings → AI Team
```

这样与之前 Project Settings 的设计方向一致。

### 12.1 页面结构

| Project Settings | 内容 |
| --- | --- |
| General | 项目基础配置 |
| Members | 项目成员 |
| Git Repositories | 仓库关联 |
| Runners | Runner 管理 |
| AI Team（当前设计） | Role、Agent Clients、模型和执行策略 |

### 12.2 AI Team 页面导航

建议设置五个子页面：

| 页面 | 内容 |
| --- | --- |
| Overview | 团队概览、状态、运行统计 |
| Roles | Role 列表、启用状态、职责配置 |
| Agent Clients | 已授权 Client、连接状态、能力 |
| Model Policies | Role 模型配置、Team 授权资源 |
| Execution Policies | 调度规则、权限、并发与失败策略 |

Execution Monitor 可以作为 AI Team 的独立子页面，也可以先通过 Overview 进入运行记录。

原稿倾向于在 V1.0 保留独立的 Execution Monitor，因为它不仅用于配置，也用于诊断任务执行问题。

### 12.3 主要页面草图

（原文为页面布局示意图，粘贴文本中图形丢失，以下按原稿文字项重建。）

```text
AI Team Overview
├── 指标卡：Enabled Roles 8 ／ Connected Clients 2 ／ Running Tasks 3 ／ Blocked Tasks 1
├── AI Project Manager（Management Role · Enabled）
├── Active Professional Roles
│     ├── Requirement Analyst（Enabled）
│     ├── Product Cognition Analyst（Enabled）
│     ├── Knowledge Manager（Enabled）
│     └── Solution Architect（Enabled）
└── Recent Executions
      ├── Requirement Analysis（Running）
      └── Architecture Review（Completed）
```

> 以上为布局示意，不代表真实统计数据或已确认的页面字段。
