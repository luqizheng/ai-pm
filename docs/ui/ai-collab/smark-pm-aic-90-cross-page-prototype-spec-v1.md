# 90 · 跨页 Prototype 交互规范 + 端到端演示流 + Figma 文件组织

> 本文件是 AI-01~07 七个页面必须共同遵守的 Prototype 规范与验收标准。视觉与布局契约见 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1.md)。
> 编号说明：本文件内部沿用 Part 2~5；执行顺序（§A）与最终验收重点（§B）为包级章节，避免与内部 Part 编号冲突。

---

## §A · 实际执行顺序

**不要一次性让 Figma AI 生成七个页面**——容易造成布局漂移、组件不一致和交互断裂。严格按以下十步执行：

| 顺序 | 工作 | 对应 Figma Page | 交付检查 |
|---|---|---|---|
| 1 | Global Design System | Page 00 | 色板对齐 `--ws-*` 命名；字阶 21/15/13/12/11；禁紫禁渐变 |
| 2 | Global Workspace Shell | Page 01 | 四区比例 13/36/31/20；Resize 不破四区；4 Tab + History Drawer |
| 3 | AI-01 | Page 02 | 主页面成立，是其余六页的布局母版 |
| 4 | AI-02 | Page 03 | Available / Attached / Read-only 三态可辨 |
| 5 | AI-03 | Page 04 | 七类卡结构一致，状态色符合 00 映射 |
| 6 | AI-04 | Page 05 | Accept ≠ 发布，正式确认走人工 |
| 7 | AI-05 | Page 06 | 授权前不可执行，失败步骤可见 |
| 8 | AI-06 | Page 07 | 六异常场景各有恢复动作 |
| 9 | AI-07 | Page 08 | 切页/切项目对话与上下文保持 |
| 10 | Prototype Flows | Page 09 | 14 步端到端流可点击走通 |

**特别建议：先完成 AI-01，再复制其 Workspace Shell 到其他六个页面。不要让 Figma AI 每次重新设计四栏布局。**其余六页的提示词中 "Reuse the exact AI-01 global layout" 即指复制 AI-01 的 Shell，而不是重新生成。

---

## Part 2 · Cross-page Prototype Interaction Rules

### 1. Global Navigation

All seven pages share one persistent navigation.

Navigation changes Main Area only.

AI Chat and Context Panel remain mounted.

### 2. Main Area Navigation

Clicking:
- Overview
- Requirements
- Tasks
- Designs
- Knowledge
- Decisions

must change the business content without resetting the Conversation.

### 3. AI Chat State

Maintain:
- Conversation ID
- Project ID
- Message History
- Current Draft
- Input Draft
- AI Response State

Input draft must survive ordinary page navigation.

### 4. Context State

Maintain three object states:

- Available
- Attached
- Read-only

Each object must have:
- Object ID
- Object Type
- Project ID
- Version / Revision
- Permission Status

Context changes should update the Context Panel immediately.

### 5. AI Response State

All AI Response Cards must support appropriate state transitions.

Examples:

Proposal:
Draft → Modified → Accepted into Draft → Submitted → Approved / Rejected

Execution:
Planned → Awaiting Authorization → Authorized → Running → Completed / Failed / Cancelled

Question:
Pending → Answered / Skipped

Risk:
Detected → Confirmed / Dismissed / Converted to Task

### 6. Context and Result Synchronization

When a user selects an AI Result:

- Highlight the relevant object.
- Show source references.
- Show the exact Revision.
- Allow navigation to the related business object.

When the user updates a business object:

- Refresh its current state.
- Preserve historical AI results.
- Indicate if a previous result is based on an outdated version.

### 7. Permission Interaction

All sensitive operations require a visible authorization state.

Use explicit confirmation dialogs for:
- Formal approval
- Official business object modification
- Task execution
- Destructive actions
- Cross-project operations

Never simulate an authorized operation as completed without the confirmation transition.

### 8. Panel Interaction

Right Context Panel supports:
- Expand
- Collapse
- Resize
- Tab Switching

AI Chat supports:
- Conversation History
- New Conversation
- Pin
- Resize

Panel resizing must not break the four-region layout.

### 9. Responsive Behavior

Primary target: Desktop 1440px.

Also design behavior for:
- 1280px desktop
- 1024px compact desktop

At smaller widths:
- Preserve the four-region information hierarchy.
- Allow Context Panel to collapse.
- Allow AI Chat to expand.
- Do not create horizontal overflow.

---

## Part 3 · Complete Prototype Demonstration Flow

Create one clickable end-to-end prototype connecting all seven pages.

### Demo Scenario

"Analyze a Requirement, compare its Design, improve its acceptance criteria, approve the Revision, and create implementation Tasks."

### Prototype Sequence

| Step | Page | User Action | Expected Result |
|---|---|---|---|
| 1 | AI-01 | Open Project | Project context loaded |
| 2 | AI-02 | Attach Requirement | Requirement attached |
| 3 | AI-03 | Request Analysis | Analysis Card generated |
| 4 | AI-02 | Attach Design | Design added to context |
| 5 | AI-03 | Compare Objects | Analysis identifies mismatch |
| 6 | AI-04 | Accept Draft | Requirement draft created |
| 7 | AI-04 | Modify and submit | Revision awaits confirmation |
| 8 | AI-04 | Human approves | Official Revision updated |
| 9 | AI-05 | Generate Task Plan | Task plan generated |
| 10 | AI-05 | Authorize | Execution starts |
| 11 | AI-06 | Simulate failure | Exception displayed |
| 12 | AI-06 | Retry | Failed step recovered |
| 13 | AI-07 | Switch page | Conversation preserved |
| 14 | AI-07 | Inspect history | Historical context restored |

### Prototype Acceptance Criteria

The final prototype is accepted only when:

1. All seven pages share one consistent four-region layout.
2. Navigation works across all pages.
3. AI Chat maintains conversation continuity.
4. Context Panel correctly reflects Attached Objects.
5. Available Objects do not automatically become Attached.
6. AI Results can be inspected.
7. Proposal and Draft transitions work.
8. Formal confirmation requires human interaction.
9. Execution authorization is visible.
10. Exception and recovery flows work.
11. Project switching does not silently transfer context.
12. Historical results retain their original context.

---

## §B · 最终验收重点（三成败场景）

上面 12 条是完整清单，但**以下三个交互是整个设计的成败关键**。任一场景跑不通，即判定整体返工，不以其余条目合格补救：

### 场景 1 · 页面切换，对话与上下文不丢

用户从 Requirement 切换到 Design：
- AI Chat 不丢失（消息、草稿、状态原样）。
- Attached Context 不被清空。
- 新打开的 Design 只进 Available，不自动 Attach。

（主验页面：AI-07；联动 AI-02）

### 场景 2 · Proposal 生成后三区分工

AI 生成 Proposal 后，用户可以：
- 在 Main Area 编辑（结构化对比 + 内联编辑）；
- 在 Context Panel 检查来源（对象 + 精确版本/Revision）；
- 在 Chat 继续讨论（追问、要求修改）。

三区同时可用、互不遮蔽。

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

## Part 4 · Figma File Organization

Organize the Figma project as follows:

**Page 00 — Design System**
- Color Tokens（V2 Slate & Muted Teal，对齐 `--ws-*` 命名）
- Typography
- Spacing
- Icons
- Buttons
- Inputs
- Badges
- Cards（含 AI Badge）
- Dialogs

**Page 01 — Global Workspace Shell**
- Navigation
- Main Area
- AI Chat
- Context Panel
- Panel States

**Page 02 — AI-01 Project AI Workspace**

**Page 03 — AI-02 Context Management**

**Page 04 — AI-03 AI Response Cards**

**Page 05 — AI-04 Draft & Review**

**Page 06 — AI-05 AI Execution Control**

**Page 07 — AI-06 Exception & Permission**

**Page 08 — AI-07 Conversation Continuity**

**Page 09 — Prototype Flows**
- Main Business Flow
- Context Flow
- Approval Flow
- Execution Flow
- Exception Recovery
- Conversation Continuity

---

## Part 5 · Final Design Instruction

The seven pages must look and behave like one integrated product.

Do not generate seven unrelated dashboard designs.

Do not create seven different navigation systems.

Do not change the width or position of the AI Chat column between pages.

Do not move Context Panel into the Main Area.

Do not turn the AI Chat into a full-screen chatbot.

Do not automatically publish AI-generated content.

The main design objective is to make the user feel that:

**"I am working on my Project. AI is working alongside me. The Context Panel tells me what AI knows, what it has produced, and what I can safely do next."**

The result should be a cohesive, professional, interactive AI Project Collaboration Workspace for Smark PM.
