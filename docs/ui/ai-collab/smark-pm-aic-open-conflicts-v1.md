# AI Collaboration Workspace V1.0 — 冲突清单（已全部裁决 · 留档）

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11（提出）→ 2026-11-11（裁决） |
| 状态 | **C1~C6 全部裁决完毕**，留档追溯 |
| 裁决落点 | [95 裁决记录](smark-pm-aic-95-decision-record-v1.md) |
| 提示词包 | `docs/ui/ai-collab/`（v1-1 修订版，已按裁决改写） |

---

## C1 · 视觉方向 —— **已裁决：V2 深色（以本仓设计为准）**

| | 主张 | 出处 |
|---|---|---|
| A | 白底 / Very Light Gray + Professional Blue | 提示词包原稿 §0.3 |
| B | V2「Slate & Muted Teal」深色；禁紫禁渐变 | S04/S05 两次裁决 + 令牌真源 `docs/design/pm-ai-design-tokens-v2.css` |

**裁决（DR-C1）**：采用 **B**。专家稿 Part 2.3 的浅色描述为旧版方向，**不采纳**；专家稿的 C2~C6 结构性裁决全部采纳。
落地：00 §0.3 V2 令牌表（含四区表面映射与状态色映射）。

## C2 · 自动审批硬边界 —— **已裁决：Default Deny + 权限交集 + 真人审批强制**

| | 主张 | 出处 |
|---|---|---|
| A | 「AI 不得自动审批」为已确定约束 | 基线 §0.3 / §5.3 |
| B | `auto_accept` 无表态默认允许；`confirm_baseline` 无真人守卫 | `ai_team_policies.rs:150`（DEF-P0-01）、DEF-P0-02 |

**裁决（DR-C2）**：A 成立，B 为待修缺陷。默认拒绝；权限取三层交集；受保护操作强制真人审批且不可绕过；`auto_accept` 语义拆分为「可进 Draft / 不可正式审批」；DEF-P0-01、DEF-P0-02 需服务端自动化测试验证修复。
落地：00 §0.6 · AI-04（Approval Pending 强拦截 + Permission Denied）· AI-05（授权 ≠ 审批）· AI-06（硬审批边界）· 90 §4.3/§4.4/Part 8。

## C3 · 右栏结构 —— **已裁决：四主 Tab + History Drawer**

| | 主张 | 出处 |
|---|---|---|
| A | 五 Tab（含 History） | 基线 §3.1、`ai-collab-v1-context-tabs.png` |
| B | 四主 Tab + History Drawer | 用户线框稿、专家 DR-C3 |

**裁决（DR-C3）**：采用 B。History 五类历史走 Drawer，支持分类查看，保留发生时状态，不得被当前 Context 覆盖；七页一律遵守。
落地：00 §0.2D · AI-01/02/07 · 90 §4.6。基线 §3.1 已同步改写。

## C4 · 卡名词表 —— **已裁决：八类 Envelope 不变，七类 UI 卡映射**

| | 主张 | 出处 |
|---|---|---|
| A | 七卡名：Analysis / Proposal / Risk / Question / Decision / Execution / Progress | 基线 §5.1 |
| B | 信封八类 `RESPONSE_TYPES`；Risk、Progress 无独立类型 | `ai_interaction_envelope.rs:36` |

**裁决（DR-C4）**：维持 B 的八类契约，A 的七类作为 UI 呈现层。Risk 须有结构化字段（severity/impact/likelihood/affected_objects/mitigation/status），否则显示普通 Recommendation；Progress 须依据真实执行状态（Running / Partially Completed / Failed / Completed / Cancelled），不得由 AI 文本推断成功；artifact 内嵌不设独立主卡；前端须建立统一 Response Rendering Contract，不得扩展白名单。
落地：00 §0.5 · AI-03 · 90 §4.5。

## C5 · 四区宽度 —— **已裁决：188 / 518 / 446 / 288**

| | 主张 | 出处 |
|---|---|---|
| A | 比例 13/36/31/20 @1440 | 用户线框 |
| B | 200/500/420/280（合计 1400） | 提示词包原稿 §0.2 |

**裁决（DR-C5）**：采用 A 的比例并定为**具体像素** 188/518/446/288（合计 1440）。**附带约束**：Main Area 仅 518px，不强制双栏 Original/Proposed Diff，改用 Inline / Unified Diff 或单列切换。
落地：00 §0.2 · AI-01/AI-04 · 90 §4.7/§4.8。

## C6 · Global AI 入口 —— **已裁决：V1 无未绑定会话，Global AI 只是 Project 选择入口**

| | 主张 | 出处 |
|---|---|---|
| A | Global AI 为「尚未绑定 Project」的会话入口 | 基线 §1.1 |
| B | `ai.conversation.project_id NOT NULL` | `0001_baseline.sql:818` |

**裁决（DR-C6）**：采用 B。Global AI 保留导航入口，但流程为「Project 选择 → 创建/恢复该 Project 的 Conversation → 进入 Project AI Workspace」；切项目不转移 Conversation 与 Context；Team-level / Multi-project / Cross-project Search 不纳入 V1。
落地：00 §0.7 · AI-01（Global AI Entry）· AI-07（Project 隔离）· 90 §4.2。

---

## 非冲突备忘（已处置）

- Part 编号冲突：用户口述 Part 2/3 与 90 内部 Part 2~5 撞号 → 落库为 §A/§B。
- G9 笔误：基线「5 字段」括号漏列 project_name。
- **实施提醒**：验收门 §B 场景 3（三处状态一致更新）在现有实现仍属 G1/G2 断点（右栏读活态 `shell.assistantContext`、`metadata.injection` 前端零消费）——Figma 可画，落地需先修 G1/G2。
