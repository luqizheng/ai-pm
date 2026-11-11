# AI Collaboration Workspace V1.0 — 待专家裁决冲突清单

| 项目 | 内容 |
|---|---|
| 日期 | 2026-11-11 |
| 背景 | 设计基线 `docs/product/ai-pm-ai-collaboration-workspace-v1.md` + Figma 提示词包 `docs/ui/ai-collab/`（分支 `feat/ai-collab-v1-review`） |
| 说明 | 以下冲突在包内均已有**临时处置**（标注"可推翻"），专家只需逐条给选项编号或新指示 |
| 结论寄回 | 每条裁决结果回填 §3,即可解除对应"待裁决"状态 |

---

## C1 · 视觉方向：浅色专业蓝 vs V2 深色锁定 【最高优先，未获用户裁决】

| | 主张 | 出处 |
|---|---|---|
| A | 白底 / Very Light Gray + Professional Blue，克制 B2B | 提示词包原稿 §0.3（用户提供的版本） |
| B | V2「Slate & Muted Teal」深色；禁紫禁渐变 | S04/S05 两次裁决「简报浅色蓝方向收敛至 V2 深色」；全应用令牌唯一真源 `docs/design/pm-ai-design-tokens-v2.css`（`:root` 即深色，浅色经 `data-theme="light"`） |

- **临时处置（可推翻）**：包 00 §0.3 已重写为 V2 深色（深色为默认渲染目标，保留浅色变体列），七页锚点行同步。
- **待裁决**：① 沿用 V2 深色？② 若必须浅色：全应用换肤，还是本模块豁免双主题？
- **影响**：七页全部色值、验收截图基准、前端落地成本。

## C2 · 「AI 不得自动审批」硬边界 vs 实现默认放行

| | 主张 | 出处 |
|---|---|---|
| A | 「AI 不得 AutoApply 正式 Requirement，不得自动审批」写作**已确定约束**；场景 6 等待用户确认；AI-04「AI cannot approve its own proposal」 | 基线 §0.3 / §5.3 / 场景 6；AI-04 |
| B | `auto_accept`「无任何层表态时**默认允许**」；`confirm_baseline` 无真人守卫 | `crates/pm-application/src/ai_team_policies.rs:150`（缺陷 DEF-P0-01）；DEF-P0-02 |

- **临时处置**：仅要求基线附录 B 挂账两条缺陷，未改任何行为。
- **待裁决**：① 默认值是否翻转（无表态 = 默认禁止）？② 还是维持「交集 + 显式 require_human_approval 兜底」？③ 基线硬边界是否要求提示词包升级为显式 UI 门（Approval Pending 强拦截态）？
- **影响**：AI-04 / AI-05 / AI-06 的权限审批交互设计强度。

## C3 · 右栏结构：五 Tab vs 四 Tab + History Drawer

| | 主张 | 出处 |
|---|---|---|
| A | 五 Tab：Context / Sources / Results / Actions / **History** | 基线 §3.1、`ai-collab-v1-context-tabs.png` |
| B | 四主 Tab；History 经二级入口 / Drawer，「不增加第五个主区域」 | 用户线框稿说明；提示词包 00 §0.2D、AI-07 |

- **临时处置**：包内按四 Tab + Drawer。
- **待裁决**：定稿哪个？若定四 Tab，基线 §3.1 与五 Tab 配图需同步修订。
- **影响**：AI-01 / AI-02 / AI-07 及 Context Panel 宽度分配。

## C4 · 卡名词表：七卡名 vs 信封八类

| | 主张 | 出处 |
|---|---|---|
| A | 七卡名：Analysis / Proposal / Risk / Question / Decision / Execution / Progress | 基线 §5.1；AI-03 |
| B | 信封 8 类 `RESPONSE_TYPES`：informational / recommendation / proposal / question / decision / **artifact** / **action-request** / execution-result——Risk、Progress 无独立类型 | `crates/pm-application/src/ai_interaction_envelope.rs:36` |

- **临时处置**：包 00 §0.5 映射：Risk → recommendation + severity 徽标；Progress → execution-result；artifact 由卡内嵌展示。
- **待裁决**：① 映射表成立？② 还是实现侧增 risk / progress 类型（动契约，涉及 8→10 类白名单）？
- **影响**：AI-03 组件体系与前端卡实现一一对应。

## C5 · 四区宽度数值不一致（轻微）

| | 主张 | 出处 |
|---|---|---|
| A | 比例 13 / 36 / 31 / 20 @1440 ≈ 190 / 520 / 440 / 290 | 用户确认线框 |
| B | 默认 200 / 500 / 420 / 280（合计 1400，非 1440） | 提示词包原稿 §0.2 |

- **临时处置**：比例为准，px 仅作参考标注。
- **待裁决**：Figma 画板初始值用哪组？
- **附带风险（请专家一并复核）**：Main Area 仅 ~520px——AI-04 的 Original│Proposed 双栏 diff 在此宽度是否可用？AI Chat ~440px 接近主区宽度是否为预期？

## C6 · Global AI 入口 vs 会话强制绑定项目

| | 主张 | 出处 |
|---|---|---|
| A | 两种入口：Global AI「尚未绑定具体 Project」 | 基线 §1.1 |
| B | `ai.conversation.project_id NOT NULL`，无未绑定会话模型（基线 G11 自认） | `migrations/0001_baseline.sql:818` |

- **临时处置**：包内 AI-01 仅呈现 Project AI Workspace，Global AI 入口未出页面。
- **待裁决**：V1 砍掉 Global AI 入口（基线改 §1.1）？还是出设计稿并挂账模型迁移？

---

## 附：非冲突备忘（已处置，无需专家）

- Part 编号冲突：用户口述 Part 2/3 与 90 内部 Part 2~5 撞号 → 落库为 §A/§B。
- G9 笔误：基线「5 字段」括号漏列 project_name。
- 提醒：验收门 §B 场景 3（三处状态一致更新）在现有实现属 G1/G2 断点（右栏读活态、`metadata.injection` 前端零消费）——Figma 可画，落地需先修 G1/G2。
