# S05 · 输入简报（参考件）· 原始需求与收敛映射

> - 记录日期：2026-11-10
> - 性质：**参考输入，非设计真源**。本文件归档原始需求简报原文，供追溯"为什么这样设计"。
> - **设计真源**：`../INDEX.md` + `../smark-pm-s05-00-design-master-prompt-v1.md` + 各页提示词 + `docs/design/pm-ai-design-tokens-v2.css`（V2 令牌）。
> - 原则：**以我们的设计为主，简报作为参考**（用户裁决 2026-11-10）。

---

## 一、收敛映射（简报条目 → 我们的裁决 → 出处）

| 简报条目 | 我们的裁决 | 出处 |
|---|---|---|
| §13「Neutral light background / restrained blue accent」 | **不采纳**——该方向已被 `docs/ui/s04/INDEX.md` 视觉对齐裁决否决（早期草稿）。统一 V2「Slate & Muted Teal」深色 + 雾蓝青 `#75A9A4`、禁紫色与渐变 | 各页 §视觉风格；INDEX 视觉对齐裁决 |
| 简报「S04 = 项目认知与知识管理」 | **不采纳**——沿用五阶段既有语义（S04 = 任务拆解与执行规划）；简报的边界清单（不建需求/不确认设计/不定义知识权威/不管 Git 配置）全部保留 | INDEX 职责边界行 |
| §4 项目导航（Command Center / Product·Requirements / Product Cognition / Project Cognition / Design / Tasks / AI Execution / Knowledge / Settings） | 采纳，但补上 S04 模块入口（Tasks）作为 AI Execution 的上游，顺序不变 | 00 §3；各页 §2 |
| §4「Do not create a generic Workspace concept」 | 采纳——S05 二级导航固定为 Overview / Collaboration / Interventions / Deliveries / History | INDEX 文件清单；各页 §2 |
| §5 Execution Center 五段式（待处理 / 执行中 / 队列 / 交付 / 活动时间线） | 采纳五段式，但**重排优先级**：待处理置顶、时间线殿后；并加"三个 KPI 芯片"而非 KPI 卡片区 | 01 §3–§9 |
| §6 Execution Detail 三栏 + 六 Tab | 采纳；六 Tab 定为 Overview / Timeline / Agent Activity / Logs / Changes / Deliverables，**默认 Overview/Timeline 而非日志**（简报已要求，落地为硬约束） | 02 §5 |
| §7 Multi-Agent Collaboration 图谱 | 采纳依赖图谱，**改为执行依赖图 + Agent 时间泳道双视图**；明确禁止"Agent 聊天窗口"与装饰性头像；知识写入改为"平台中介的提案" | 03 §5–§7 |
| §8 Human Intervention 五场景 | 采纳五场景；每个场景补上**后端既有契约**（waiting reason 五值、interaction 生命周期、resource-scoped 权限） | 04 §5 |
| §9 Delivery Review 五段 + 三动作 | 采纳五段 A–E 与三动作；**修正 verdict 口径**：verdict 只有三值，"未评审"为空值；Reject 是任务级人工终止，不等于第四个 verdict | 06 §10 |
| §10 Execution History 筛选维度 | 采纳筛选维度；补 review verdict 与 run status **分列**、展开行内摘要、历史不可覆盖 | 07 §6 |
| §11 十种交互状态 | 采纳，并落到具体页面（每页 §Interaction & States），且补"设备不可用/未就绪"与"交付被退回"两态 | 各页 |
| §12 六大概念不得混淆 | **提升为全局不变量**，并补库级真值表（13 态 / 7 态 / 5 值 reason / 3 值 verdict / 6 态 readiness） | INDEX 领域语义裁决 + 契约真源表 |
| §14 七页 + 四条旅程 | 采纳；生成顺序按用户裁决：先整批 01→07，第二轮单深 01/03/06 | INDEX 连续工作流 |
| §13 参考风格（Linear / GitHub / 企业项目管理） | 采纳为气质参照，但**色板与字阶一律走 V2 令牌**（不照搬浅底蓝 accent） | 各页视觉风格 |

---

## 二、原始简报原文（归档）

### S5 — AI Agent Execution & Collaboration

AI 研发平台 · Project Level

1. **Execution Center（执行中心）** — 查看项目所有 Agent 执行任务、运行状态和待处理事项。
2. **Execution Detail（执行详情）** — 查看单个任务的执行过程、Agent 活动、执行日志和产出。
3. **Agent Collaboration（Agent 协作）** — 展示多个 Agent 的并行工作、依赖关系和协作进度。
4. **Human Intervention（人工介入）** — 处理 Agent 等待授权、等待信息、执行阻塞和异常等情况。
5. **Delivery Review（交付审查）** — 审查 Commit、执行报告、测试证据和交付清单，最终由人类验收。

**核心产品原则**

1. AI Agent 负责执行，人类负责授权、关键决策和最终验收。
2. 多 Agent 可以并行执行，但知识库写入必须统一协调。
3. MCP 是 Agent 与 PM 系统的通信接口，不是执行运行环境。
4. Codex 等外部工具负责工程执行，PM-AI 负责任务管理、过程跟踪和结果收敛。
5. 不能把任务状态、Agent 状态、Run 状态混为一谈。
6. 执行环境必须考虑 Device 生命周期和 Readiness 状态。
7. 权限必须遵循团队成员、项目成员、Device 和任务授权链。

**屏幕清单**：Screen A 执行中心 / B 执行详情 / C 多 Agent 协作 / D 人工介入（5 场景：执行授权、等待澄清、执行阻塞、执行失败、权限请求）/ E 交付审查 / F 执行历史。

**关键交互**：Journey A 批准任务 → 授权 → Agent 执行 → 交付提交 → 人工审查 → 验收；Journey B 等待澄清 → 人工回复 → 恢复执行；Journey C 并行执行 → 一 Agent 失败 → 人工介入 → 重试 → 交付；Journey D 执行完成 → 交付审查 → 退回修改 → 新一轮执行。

**避免项**：ChatGPT 式全屏聊天、超大彩色 AI 插画、过多 KPI 卡片、无谓动画、过度嵌套卡片、把 Agent 当装饰头像、把执行日志当唯一信息来源。

---

**归档说明**：以上为原始简报的要点还原（原文为 Figma AI 英文提示词块，因与我们的页面级提示词内容重叠，此处保留决策要点而非逐字复制；逐字稿以会话记录为准）。本文件只作参考，实际投喂 Figma AI 请使用 `../smark-pm-s05-ui-0X-*.md` 七份页面级提示词。
