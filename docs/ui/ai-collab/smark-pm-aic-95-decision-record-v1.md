# AI Collaboration Workspace V1.0 — 正式裁决记录（Decision Record）

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11 |
| 状态 | **已确认**（用户正式裁决） |
| 适用范围 | 产品设计 · Figma · Frontend · Backend · Prototype · QA |
| 上位文档 | `docs/product/ai-pm-ai-collaboration-workspace-v1.md`（设计基线） |
| 本包 | `docs/ui/ai-collab/`（Figma AI 提示词包，v1-1 修订版） |

本文件是本模块的**裁决真源**。提示词包与设计基线若与本文件冲突，以本文件为准。

---

## DR-C1 · 视觉方向（用户裁决，非专家稿）

**状态：CONFIRMED（以本仓设计为准）**

采用：**V2「Slate & Muted Teal」深色锁定**——画布 `#171C24`、侧栏 `#1D2631`、表面 `#222A34`、强调色 `#75A9A4`（雾蓝青），深色为默认渲染目标，浅色经 `data-theme="light"` 提供变体；**禁紫、禁渐变**。

- 令牌唯一真源：`docs/design/pm-ai-design-tokens-v2.css`。
- **专家稿 Part 2.3 的「White background / Professional blue primary」为旧版浅色方向，不予采纳**（S04/S05 已两次裁决收敛至 V2 深色，全应用仅一套 token）。
- 专家稿的**结构与交互裁决（C2~C6）全部采纳**；被否决的只有 §2.3 视觉段。

---

## DR-C2 · AI 审批安全边界

**状态：CONFIRMED**

采用：**Default Deny + Permission Intersection + Mandatory Human Approval**。

AI 不得自动批准正式业务对象，不得通过策略配置绕过真人审批。

### 1. 默认拒绝

任何权限策略没有明确表态时，默认禁止。

```text
No Explicit Policy → DENY
```

不允许使用默认放行逻辑。

### 2. 权限交集

```text
Effective Permission = Team Policy ∩ Project Policy ∩ Action Policy
```

任一层拒绝即最终拒绝。未配置策略不得视为允许。

### 3. 真人审批硬约束

以下正式业务操作必须经有权限真人审批：Formal Requirement Approval · Formal Requirement Revision Confirmation · Formal Knowledge Confirmation · Formal Decision Approval · 其他具有正式业务批准或发布性质的操作。

`require_human_approval = true` 是受保护操作的**不可绕过**约束。AI 不得：批准自己生成的 Proposal · 自动确认正式 Requirement · 自动批准正式 Revision · 通过 `auto_accept` 绕过人工审批 · 用低层策略覆盖真人审批要求。

### 4. `auto_accept` 语义必须拆分

| 行为 | 允许性 |
|---|---|
| 自动接受建议进入**非正式 Draft** | 允许，但须有明确权限 |
| 自动修改正式业务对象 | 不允许绕过正式变更流程 |
| 自动批准正式 Revision | 禁止 |
| 自动发布正式 Requirement | 禁止 |

`auto_accept` 不得同时代表 Draft Acceptance 与 Formal Approval。

### 5. 后端缺陷处理

| 缺陷 | 位置 | 要求 |
|---|---|---|
| DEF-P0-01 | `crates/pm-application/src/ai_team_policies.rs:150` | 修复默认放行 → 默认拒绝 |
| DEF-P0-02 | `confirm_baseline` | 增加真人审批守卫 |

两条缺陷目前**仅登记，不代表代码已修复**。修复后必须通过服务端自动化测试，不能只依赖前端按钮限制。

### 6. UI 强制状态

必须提供：**Approval Pending · Human Approval Required · Permission Denied · Approved · Rejected**。Approval Pending 必须是**强拦截态**；AI Action 在审批完成前不得产生正式批准结果。

---

## DR-C3 · Context Panel 信息架构

**状态：CONFIRMED**

采用：**Four Primary Tabs + History Drawer**。

| 主 Tab | 职责 |
|---|---|
| Context | 当前项目、可用对象、已附加对象 |
| Sources | AI 使用的信息来源及版本 |
| Results | AI 输出及其状态 |
| Actions | 待授权操作、执行任务及操作结果 |

History 不属于第五个主 Tab，通过二级入口或 Drawer 访问，七个页面一律遵守，不允许某页单独加 History 主 Tab。

**History Drawer 内容**：Conversation History · AI Action History · Context Change History · Approval History · Execution History；支持分类查看；**历史记录保留发生时的状态，不得被当前 Context 覆盖**。

---

## DR-C4 · 响应信封与 UI 卡映射

**状态：CONFIRMED**

维持后端现有八类 Response Envelope，**不增加 Risk、Progress 两种独立 Envelope**。

| UI Card | Envelope | 渲染要求 |
|---|---|---|
| Analysis | informational | 分析、结论、证据 |
| Proposal | proposal | 修改方案、前后对比 |
| Risk | recommendation | severity / impact / likelihood |
| Question | question | 问题、选项、补充回答 |
| Decision | decision | 决策、备选方案 |
| Execution | action-request | 执行计划、授权请求 |
| Progress | execution-result | 进度、执行状态、结果 |

Artifact 作为可嵌入业务产物（Requirement Draft · Design Document · Analysis Report · Execution Report），不增加独立 UI 主卡类型。

### Risk Card 识别

必须依据结构化字段：`severity` · `impact` · `likelihood` · `affected_objects` · `mitigation` · `status`。**没有风险结构化字段时，显示普通 Recommendation**（不得由 AI 文本推断风险）。

### Progress Card 识别

使用 execution-result，支持 Running / Partially Completed / Failed / Completed / Cancelled；**具体状态必须依据实际执行状态，不得根据 AI 文本推断成功**。

### Action Request 安全要求

`action-request` 仅代表**请求执行**，不代表已获授权；必须经权限检查与必要的人工确认才进入实际执行阶段。

### 契约维护

Frontend 必须建立统一的 **Response Rendering Contract**，**不允许前端自行扩展 Envelope 白名单**。

---

## DR-C5 · 四区桌面布局

**状态：CONFIRMED**

正式画板 **1440 × 1000px**。

| 区域 | 宽度 | 占比 |
|---|---:|---:|
| Navigation | 188px | 13% |
| Main Area | 518px | 36% |
| AI Chat | 446px | 31% |
| Context Panel | 288px | 20% |
| **Total** | **1440px** | **100%** |

设计规则：四区域同时存在 · 不增加第五主区 · 四区支持合理宽度调整 · AI Chat 不因业务页面切换而消失 · Context Panel 支持折叠 · 用户调整宽度不破坏布局。

**Main Area 仅 518px，不允许强制双栏 Original / Proposed Diff。**正式采用：Inline Diff · Unified Diff · 单列 Original / Proposed 切换 · 必要时独立展开的比较视图。

响应式：1440px 标准四区 · 1280px 保持四区、压缩留白 · 1024px Context Panel 默认可折叠、AI Chat 可扩展 · **不得横向溢出**。

---

## DR-C6 · 会话项目绑定

**状态：CONFIRMED**

**V1 不支持未绑定 Project 的 Conversation**，与现有数据库约束 `ai.conversation.project_id NOT NULL` 保持一致。

### Global AI Entry

保留 Global AI 导航入口，但它**只是 Project 选择入口**，不是独立 Global Conversation：

1. 展示 Project 选择界面 → 2. 用户选择 Project → 3. 创建或恢复该 Project 的 Conversation → 4. 进入对应 Project AI Workspace。

### Project 切换

当前 Conversation 不得自动转移 · 当前 Project Context 不得自动带入其他 Project · 用户须进入目标 Project 的 Conversation · 历史 Conversation 保留原 Project 归属。

### 后续版本边界

Team-level AI Conversation · Multi-project Context · Cross-project Knowledge Search **暂不纳入 V1**；未来如需实现，必须单独进行数据模型与权限架构评审。

---

## 落地对照

| 裁决 | 落点 |
|---|---|
| DR-C1 | 00 §0.3（V2 令牌，唯一视觉真源） |
| DR-C2 | 00 §0.6 · AI-04（审批状态机 + 强拦截 + Permission Denied）· AI-05（授权 ≠ 审批）· AI-06（硬审批边界 + 未授权审批尝试） |
| DR-C3 | 00 §0.2D · AI-01/AI-02/AI-07（四主 Tab + History Drawer 五类历史） |
| DR-C4 | 00 §0.5 · AI-03（七卡映射 + Risk 结构化字段 + Progress 状态 + Artifact 内嵌） |
| DR-C5 | 00 §0.2 · AI-01（188/518/446/288）· AI-04（518px 禁双栏）· 90 §4.7 |
| DR-C6 | 00 §0.7 · AI-01（Global AI = Project 选择）· AI-07（绑定规则 + 切项目隔离） |

## 实施边界

本裁决是设计与交互规范，**不证明后端已强制执行**。以下需独立代码验证，UI 成功不得当作后端正确性证明：

1. DEF-P0-01 默认拒绝策略 · 2. DEF-P0-02 真人审批守卫 · 3. 有效权限交集 · 4. 受保护操作的审批强制 · 5. Response Envelope 兼容性 · 6. 会话 Project 绑定 · 7. Context 快照完整性 · 8. 审批审计轨迹。
