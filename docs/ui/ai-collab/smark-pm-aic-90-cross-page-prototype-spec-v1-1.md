# 90 · 跨页 Prototype 交互契约 + 演示流 + 验收清单（v1-1）

> 本文件是 AI-01~07 七个页面必须共同遵守的 Prototype 规范与验收标准。视觉与布局契约见 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md)，裁决依据见 [95 裁决记录](smark-pm-aic-95-decision-record-v1.md)。
> 编号说明：本文件内部沿用 Part 编号；执行顺序（§A）与成败场景（§B）为包级章节。

---

## §A · 实际执行顺序

**不要一次性让 Figma AI 生成七个页面**——容易造成布局漂移、组件不一致和交互断裂。严格按以下十步执行：

| 顺序 | 工作 | 对应 Figma Page | 交付检查 |
|---|---|---|---|
| 1 | Global Design System | Page 00 | 色板对齐 `--ws-*`；字阶 21/15/13/12/11；禁紫禁渐变 |
| 2 | Global Workspace Shell | Page 01 | 四区 188/518/446/288；Resize 不破四区；4 Tab + History Drawer |
| 3 | AI-01 | Page 02 | 主页面成立，是其余六页的布局母版 |
| 4 | AI-02 | Page 03 | Available / Attached / Read-only 三态可辨 |
| 5 | AI-03 | Page 04 | 七类卡结构一致，状态色符合 00 映射 |
| 6 | AI-04 | Page 05 | Approve ≠ 发布，正式确认走真人，Approval Pending 强拦截 |
| 7 | AI-05 | Page 06 | 授权前不可执行，失败步骤可见，授权 ≠ 审批 |
| 8 | AI-06 | Page 07 | 六异常场景各有恢复动作 + 未授权审批尝试 |
| 9 | AI-07 | Page 08 | 切页/切项目对话与上下文保持，Project 隔离 |
| 10 | Prototype Flows | Page 09 | 16 步端到端流可点击走通 |

**特别建议：先完成 AI-01，再复制其 Workspace Shell 到其他六个页面。不要让 Figma AI 每次重新设计四栏布局。**其余六页提示词中的 "Reuse the same AI-01 four-region layout" 即指复制 AI-01 的 Shell。

---

## Part 4 · Cross-page Prototype Interaction Contract

### 4.1 Global State

Prototype 必须维护以下逻辑状态组：

**Project State**：`project_id` · `project_name` · `current_page` · `project_permission`

**Conversation State**：`conversation_id` · `project_id` · `message_history` · `input_draft` · `conversation_status`

**Context State**：`available_objects` · `attached_objects` · `readonly_objects` · `context_snapshot` · `context_version`

**AI Action State**：`action_id` · `response_type` · `action_status` · `source_references` · `result_reference`

**Approval State**：`approval_id` · `requested_by` · `approver` · `approval_status` · `approval_result`

**Execution State**：`run_id` · `execution_status` · `completed_steps` · `failed_steps` · `recovery_status`

> These are Prototype state concepts, not a mandate to introduce new backend database entities.

### 4.2 State Persistence

**Main Area navigation**：改变当前页 · 不重置 Conversation · 不清空 Attached Context。

**Context modification**：更新当前 Context · **不重写历史快照**。

**Project switching**：保留旧 Conversation · 为目标 Project 创建或恢复 Conversation · **阻止跨 Project Context 泄漏**。

### 4.3 Approval State Machine

```text
PROPOSED
    |
    v
DRAFT_CREATED
    |
    v
SUBMITTED
    |
    v
APPROVAL_PENDING
    |
    +------> REJECTED
    |
    v
HUMAN_APPROVED
    |
    v
OFFICIAL_REVISION
```

An AI Agent cannot transition directly from PROPOSED or DRAFT_CREATED to HUMAN_APPROVED.

### 4.4 Permission State Machine

```text
REQUESTED
    |
    v
POLICY_EVALUATION
    |
    +------> DENIED
    |
    v
APPROVAL_REQUIRED
    |
    v
HUMAN_APPROVAL
    |
    +------> REJECTED
    |
    v
AUTHORIZED
    |
    v
EXECUTION
```

If no explicit policy allows the action, the result is DENIED.

For protected operations, approval is mandatory regardless of permissive lower-level policies.

### 4.5 Response Rendering Contract

| Envelope | UI Rendering |
|---|---|
| informational | Analysis / Information |
| recommendation | Recommendation / Risk |
| proposal | Proposal |
| question | Question |
| decision | Decision |
| artifact | Embedded Artifact |
| action-request | Execution / Authorization Request |
| execution-result | Progress / Execution Result |

All eight Envelope types must be supported. **Do not invent a ninth or tenth Envelope type.**

### 4.6 Context Panel Contract

Exactly four primary Tabs：`Context | Sources | Results | Actions`

History is a Drawer. **Drawer must not change the four-region layout.**

### 4.7 Panel Resizing

1440px default：`188 / 518 / 446 / 288`。

Support：Resize Navigation（适当处）· Resize Main Area · Resize AI Chat · Resize Context Panel · Collapse Context Panel · Restore default layout。

**Preserve a usable minimum width.**

### 4.8 Responsive Behavior

1440px 标准四区 · 1280px 保持四区、压缩留白 · 1024px Context Panel 默认可折叠、AI Chat 可扩展 · **不得横向溢出**。

---

## §B · 最终验收重点（三成败场景）

除 §C 的完整清单外，**以下三个交互是整个设计的成败关键**。任一场景跑不通，即判定整体返工，不以其余条目合格补救：

### 场景 1 · 页面切换，对话与上下文不丢

用户从 Requirement 切换到 Design：
- AI Chat 不丢失（消息、草稿、状态原样）。
- Attached Context 不被清空。
- 新打开的 Design 只进 Available，不自动 Attach。

（主验页面：AI-07；联动 AI-02）

### 场景 2 · Proposal 生成后三区分工

AI 生成 Proposal 后，用户可以：
- 在 Main Area 编辑（518px 内采用 Inline / Unified Diff，不强制双栏）；
- 在 Context Panel 检查来源（对象 + 精确 Revision）；
- 在 Chat 继续讨论（追问、要求修改）。

（主验页面：AI-04；联动 AI-03）

### 场景 3 · 正式确认后三处状态一致更新

用户正式确认后：
- AI Chat 显示提交/审批结果；
- Main Area 显示新 Revision 并保留旧版本；
- Context Panel 刷新对象状态与版本。

三处状态必须一致，不允许出现「Chat 说已生效、Main Area 还是旧版」类撕裂。

（主验页面：AI-04；联动 AI-02 / AI-07 历史快照）

**这三个场景跑通，才算真正实现 Project、AI Chat 和 Context 的联动。**

---

## Part 5 · Complete Prototype Demonstration

**Business Scenario**：Analyze a Requirement, compare its Design, prepare a Revision, obtain human approval, and create implementation Tasks.

| Step | Page | User Action | Expected Result |
|---|---|---|---|
| 1 | AI-01 | Open Project | Project context loaded |
| 2 | AI-02 | Attach Requirement | Requirement attached |
| 3 | AI-03 | Request Analysis | Analysis Card |
| 4 | AI-02 | Attach Design | Design attached |
| 5 | AI-03 | Compare Objects | Mismatch identified |
| 6 | AI-04 | Generate Draft | Draft created |
| 7 | AI-04 | Modify Draft | Updated Draft |
| 8 | AI-04 | Submit | Approval Pending |
| 9 | AI-06 | Attempt unauthorized approval | Permission Denied |
| 10 | AI-04 | Human approval | Official Revision |
| 11 | AI-05 | Generate Task Plan | Execution Plan |
| 12 | AI-05 | Authorize | Execution starts |
| 13 | AI-06 | Simulate failure | Exception displayed |
| 14 | AI-06 | Retry | Recovery |
| 15 | AI-07 | Switch business page | Conversation preserved |
| 16 | AI-07 | Inspect historical result | Historical Context restored |

---

## Part 6 · Figma File Organization

**Page 00 — Design System**：Color Tokens（V2 Slate & Muted Teal，对齐 `--ws-*` 命名）· Typography · Spacing · Icons · Buttons · Inputs · Badges · Cards（含 AI Badge）· Dialogs

**Page 01 — Global Workspace Shell**：Navigation · Main Area · AI Chat · Context Panel · Four Tabs · History Drawer · Resize States

**Page 02 — AI-01 Project AI Workspace**
**Page 03 — AI-02 Context Management**
**Page 04 — AI-03 AI Response Cards**
**Page 05 — AI-04 Draft & Review**
**Page 06 — AI-05 AI Execution Control**
**Page 07 — AI-06 Exception & Permission**
**Page 08 — AI-07 Conversation Continuity**

**Page 09 — Prototype Flows**：Main Business Flow · Context Flow · Approval Flow · Execution Flow · Exception Recovery · Conversation Continuity · Project Isolation

---

## Part 7 · Design Acceptance Criteria

The Figma prototype is accepted only when all of the following are demonstrated.

### Layout

- [ ] Four-region layout exists on all seven pages.
- [ ] Desktop default dimensions match 188 / 518 / 446 / 288.
- [ ] Context Panel has exactly four primary Tabs.
- [ ] History uses Drawer.
- [ ] Main Area supports readable Diff layouts at 518px.

### Context

- [ ] Available is distinct from Attached.
- [ ] Read-only is clearly indicated.
- [ ] Attach and Remove work.
- [ ] Page switching does not erase explicit Context.
- [ ] Historical Context remains unchanged.

### Response Cards

- [ ] Seven UI Card types are supported.
- [ ] Eight Envelope types remain unchanged.
- [ ] Risk is rendered from structured recommendation data.
- [ ] Progress is rendered from execution-result.
- [ ] Artifact is embedded.

### Approval

- [ ] Approval Pending is a hard blocking state.
- [ ] AI cannot approve its own Proposal.
- [ ] Unauthorized users cannot approve.
- [ ] Formal Revision requires authorized human approval.
- [ ] No successful state is shown before approval succeeds.

### Execution

- [ ] Authorization is explicit.
- [ ] Execution status is visible.
- [ ] Partial failure is visible.
- [ ] Retry eligibility is respected.
- [ ] Execution results can be inspected.

### Conversation

- [ ] Every Conversation is Project-bound.
- [ ] Global AI requires Project selection.
- [ ] Project switching does not transfer Context.
- [ ] Conversation continuity works.
- [ ] Historical results preserve their original context.

---

## Part 8 · Implementation Boundary

The Figma prototype is a design and interaction demonstration. **It does not prove backend enforcement.**

以下需独立代码验证，**UI 成功不得当作后端正确性证明**：

1. DEF-P0-01：Default-deny Policy
2. DEF-P0-02：Human approval guard
3. Effective permission intersection
4. Protected action approval enforcement
5. Response Envelope compatibility
6. Conversation Project binding
7. Context snapshot integrity
8. Approval audit trail

---

## Final Product Design Principle

The user is working on a Project.
AI works alongside the user.
The Main Area shows the business object being worked on.
AI Chat shows the collaboration process.
Context Panel shows what AI knows, what it references, what it produces, and what actions are available.
The user remains the authority for formal business decisions.

**AI can analyze, propose, prepare, and execute authorized work. It cannot replace the human authority responsible for formal approval.**
