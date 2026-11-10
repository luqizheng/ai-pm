# S04 · Task Management & Planning — UI 设计提示词索引

> - 记录日期：2026-11-10
> - 领域：S04 任务管理与执行规划（Figma AI 高保真页面级提示词）
> - 状态：v1 已落库；01–04 已按提示词落地（总览/任务池接真，拆解/详情 mock 预览）；05–07 已落地为 mock 预览（依赖图/时间线/执行准备，后端聚合端点待补）。进度见 `ai-frontend/src/views/project/tasks/`
> - 职责边界（2026-11-08 确认）：S01 项目初始化与代码认知 / S02 需求管理 / S03 技术设计与实施基线 / **S04 任务拆解、研发组织与执行规划** / S05 工程 Agent 调度、代码执行与交付
> - S04 不执行代码：把 S03 已批准的实施基线转化为可执行、可追踪、有明确交付标准的研发任务计划

## 视觉对齐裁决

- 全部页面锁定 V2 设计系统「Slate & Muted Teal」（`docs/design/pm-ai-design-tokens-v2.css`，全应用视觉唯一真源）：**深色主题为默认渲染目标**，浅色为 token 映射变体。
- 与早期草稿（浅底 + 蓝色 accent）的差异以本目录为准：主强调色为雾蓝青 `#75A9A4`；**全站禁止紫色**——AI 生成内容用 Info 蓝 tint + 「AI」徽标表达。
- 字阶：Page 21 / Section 15 / Body 13 / Caption 12 / Micro 11。

## 文件清单

| 编号 | 文件 | 页面 | 核心作用 |
|---|---|---|---|
| 00 | [smark-pm-s04-00-design-master-prompt-v1.md](smark-pm-s04-00-design-master-prompt-v1.md) | 模块级总提示词 | 整体语境与设计哲学 |
| 01 | [smark-pm-s04-ui-01-task-overview-v1.md](smark-pm-s04-ui-01-task-overview-v1.md) | Task Overview | 任务管理总览 |
| 02 | [smark-pm-s04-ui-02-task-breakdown-v1.md](smark-pm-s04-ui-02-task-breakdown-v1.md) | Task Breakdown | AI 拆解与人工确认 |
| 03 | [smark-pm-s04-ui-03-task-detail-v1.md](smark-pm-s04-ui-03-task-detail-v1.md) | Task Detail | 单个任务的完整信息 |
| 04 | [smark-pm-s04-ui-04-task-backlog-v1.md](smark-pm-s04-ui-04-task-backlog-v1.md) | Task Backlog | 任务查询与日常管理 |
| 05 | [smark-pm-s04-ui-05-execution-planning-dependency-graph-v1.md](smark-pm-s04-ui-05-execution-planning-dependency-graph-v1.md) | Dependency Graph（含并行执行组） | 任务依赖关系 |
| 06 | [smark-pm-s04-ui-06-execution-planning-timeline-v1.md](smark-pm-s04-ui-06-execution-planning-timeline-v1.md) | Execution Timeline | 执行时间规划 |
| 07 | [smark-pm-s04-ui-07-execution-readiness-v1.md](smark-pm-s04-ui-07-execution-readiness-v1.md) | Execution Readiness | 执行准备与 S05 移交 |

每份页面提示词内嵌全局框架与令牌，**独立可投喂 Figma AI，不依赖上下文**；均含低保真布局、组件清单、交互状态三件套。

## 连续工作流（七页不是七个独立模块）

```
Task Overview      发现需要处理的任务与规划问题
  → Task Breakdown AI 生成任务，人类审核确认
  → Backlog/Detail 管理任务、补充上下文与验收要求
  → Dep Graph/Timeline 组织依赖、执行顺序与并行关系
  → Readiness      完成质量检查，准备移交
  → S05            正式进入工程执行
```

**生成顺序**：先生成 01 → 02 → 03（确定整体设计语言与任务交互模型）；再 04 → 05 → 06；最后 07 完成 S04 闭环。

## 三个关键设计不变量

1. **来源追溯**：Task 必须可回链 Requirement / Technical Design / ADR / Implementation Baseline（含版本）。
2. **AI 提案 ≠ 正式任务**：AI Proposal → Human Review（支持批量）→ Approved Task；AI 不得静默修改已批准计划。
3. **Approved ≠ Ready**：Execution Readiness 是 S04 唯一出口；Ready 要求仓库分配、依赖有效、执行说明、验收标准齐备。

## 共享词汇

- 规划状态机：Draft → Pending Review → Approved → Ready for Execution；另有 Blocked / Completed / Cancelled。规划状态 ≠ 工程执行状态。
- 任务类型：Feature / Development / Refactoring / Testing / Documentation / Infrastructure / Bug Fix。
- 优先级：P0 Critical / P1 High / P2 Medium / P3 Low。
- AI 提案生命周期：Proposed →（Accept / Edit / Reject）→ Applied，全程留痕。
