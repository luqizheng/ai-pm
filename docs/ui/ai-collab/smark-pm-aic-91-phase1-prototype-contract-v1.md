# 91 · 第一阶段 Prototype 契约（AI-01 + AI-02）

> 本文件是第一阶段的**原型状态与交互契约 + 演示场景 + 验收标准**，配合 [AI-01 v1-3](smark-pm-aic-ui-01-project-ai-workspace-v1-3.md) 与 [AI-02 v1-3](smark-pm-aic-ui-02-context-management-v1-3.md) 使用。
> 视觉与全局规则见 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md)，裁决见 [95 裁决记录](smark-pm-aic-95-decision-record-v1.md)。

---

## 实施进度（2026-11-11 · 前端）

| 阶段 | 状态 | 提交 |
|---|---|---|
| ① 四栏容器 + AI Chat 常驻列 | **已落地** | `53ddf93` |
| ② 会话能力并入常驻列 | **已落地** | `64c8e40` |
| ③ Context Panel 四 Tab + History Drawer | **已落地** | `846abed` |
| ④ 专注模式 + 切项目提示（G12） | **已落地** | `53ddf93`（专注模式）· `f68c060`（切项目确认） |

已落地的关键事实（以代码为准）：

- 四栏主体工作区：Navigation 188 / Main 518（1fr，1440 基准正好 518）/ AI Chat 446 / Context 288；52px TopBar 保留在上方。
- **会话机制单一实现**：`ai-frontend/src/composables/useAssistantConversation.ts`——会话页与常驻列共用，不重复建设会话体系（DR-C8 §3）。
- 指令栏以 `embedded` 模式成为列内 Composer（就地发送）；独立模式行为不变（入队交接仍服务顶栏助手入口）。
- 完整会话页路由 `/project/assistant` 时 AI Chat 列收起，避免同屏重复会话面。
- 专注模式开关在**顶栏**（列收起后顶栏是唯一退出入口），退出逐项还原进入前的右栏开合态。
- 右栏四一级 Tab + History Drawer 已落地（阶段③）：派生数据由**单一写者**
  （`useAssistantConversation`）发布到 `stores/assistant.ts`，右栏与常驻列读同一份，
  **右栏不另起 fetch**；Sources 只展示**真实注入记账**（后端 `metadata.injection`），
  未收到回复即显示未注入，**不冒充注入面**（G2 断点的正解）。
- **切项目确认已落地**（G12 · DR-C6）：有活跃会话时切换项目先确认——确认 = 保留当前
  会话并在目标项目另起（历史会话保留原项目归属），取消 = 维持现状；两个切换入口
  （顶栏 / 左导航）走同一道门；文案明说「不会跟着转移」。
- **仍未落地**：对象级上下文（Attach / Remove / 双轴状态机）属 AI-02 阶段，
  **需后端扩展锚点白名单**（`0003` 白名单现为 5 键：project_id / project_name /
  repository_name / default_branch / repository_role，装不下对象级上下文 +
  版本）；审批与执行的**服务端读接口** V1 无（`/interaction` 仅 POST 写回），
  Actions / History 的审批与执行类别目前由客户端派生。

---

## Part C — 原型状态与交互契约

### C1. 全局状态

原型必须建立以下共享状态：

```
selectedProject
selectedConversation
activeContextTab
contextItems
sourceItems
resultItems
actionItems
historyDrawerOpen
historyCategory
selectedHistoryEvent
selectedContextItem
pendingContextChanges
```

**所有相关组件必须读取同一份状态。** 不要为不同页面建立互不关联的重复状态。

### C2. Context 状态变化

实现以下可操作流程：

1. Add Context
2. Context 从不存在变成 Pending 或 Active（按原型设定的确认流程）
3. Enable Context
4. Disable Context
5. Remove Context
6. 查看状态变化历史

每次操作都应：
- 更新列表。
- 更新统计。
- 更新详情。
- 更新 History。
- **保持其他 Context 不受影响。**

### C3. Project 与 Conversation 边界

V1 不支持没有 Project 绑定的 Conversation（DR-C6）。

全局 AI Collaboration 入口只能作为 **Project Selector**。用户选择 Project 后，才能创建或恢复对应 Conversation。

切换 Project 时：
- 不转移 Context。
- 不混用 Sources。
- 不混用 Results。
- 不混用 Actions。
- 不混用 Conversation History。

**必须让用户清楚当前 Conversation 属于哪个 Project。**

### C4. 权限与审批边界

默认拒绝未明确授权的操作（DR-C2）。有效权限：

```
Effective Permission = Team Policy ∩ Project Policy ∩ Action Policy
```

涉及正式审批的操作必须经过人工批准。AI 不得批准自己的提案，也不得正式批准或发布 Requirement / Revision / Knowledge / Decision。

必须区分：
- **Accept Draft**（进草稿）
- **Approve Formal Change**（正式审批）

接受 AI 草稿不等于正式审批。

### C5. Main Area 专注模式（Focus Mode · DR-C8）

518px 是常态工作宽度，复杂编辑场景不永久受限。

| 规则 | 要求 |
|---|---|
| 进入 | 有可见、可键盘触达的入口；进入后 AI Chat 与 Context Panel 收起，业务工作区扩大 |
| 退出 | 恢复原来的四栏布局（188/518/446/288） |
| 状态 | **不改变**当前 Project、Conversation、Context |
| 内容 | **不丢失**正在编辑的内容或对话状态 |
| 范围 | 只影响布局，不影响任何业务语义；Context / Sources / Results / Actions 数据不变 |

专注模式下的 Diff 呈现：默认 Inline / Unified Diff，**不强制**左右并排；必要时可展开独立比较视图。

### C6. Shell 渐进迁移（实施顺序 · DR-C8）

**不要一次性把现有三栏全部推倒。**三步推进：

1. **建立四栏容器与 AI Chat 列** —— Shell 骨架就位；52px TopBar 与 V2 视觉系统不动。
2. **迁移现有会话能力进常驻 AI Chat 列** —— 复用既有路由与对话机制（`/project/assistant` 的核心对话能力），**不重复建设会话体系**；原底部指令栏升级为 AI Chat 列内的输入组件。
3. **升级 Context Panel** —— 单张上下文卡 → 288px 四 Tab + History Drawer。

每一步都应可独立验收、独立回退；任一步不得破坏既有 CommandBar、Conversation 与 Context 数据机制。

---

## Part D — 原型演示场景

在 Figma Prototype 中建立一条可完整操作的演示路径：

| # | 操作 | 期望结果 |
|---|---|---|
| 1 | 进入 AI Collaboration | 打开 Project Selector（无 Project 时） |
| 2 | 选择 Project | 当前 Project 确立，modules 导航出现 |
| 3 | 打开或创建 Project-bound Conversation | Conversation 与 Project 绑定 |
| 4 | 在 AI Chat 查看 AI Analysis Card | Analysis 卡渲染，来源可查 |
| 5 | 切换 Context Tab | 已有 Context 列表可见 |
| 6 | 点击 Add Context | 打开多选选择面板 |
| 7 | 多选 Requirement 与 Knowledge | 选中态明确，已添加项标灰 |
| 8 | 添加 Context | 列表新增，Overview 计数变化，结果提示 |
| 9 | Disable 一条 Context | 该 Context 变为 Inactive（文字明示），其余不变 |
| 10 | 打开 Sources Tab | 展示 AI 当前引用的来源（Available / Referenced / Unavailable 分列） |
| 11 | 打开 Results Tab | 展示 AI 生成的 Draft 及其状态 |
| 12 | 打开 Actions Tab | 展示 Context 变更记录与 AI 请求/用户授权/执行结果的区分 |
| 13 | 打开 History Drawer | 五类记录可筛选 |
| 14 | 筛选 Context Change | 仅显示上下文变更记录 |
| 15 | 查看一条历史事件详情 | 显示前后状态、触发者、关联对象 |
| 16 | 关闭 Drawer | 恢复打开前的 Tab 与滚动位置 |
| 17 | 切换 Project | 验证 Context、Sources、Results、Actions、History 都不串项目 |
| 18 | 进入 Main Area 专注模式 | AI Chat 与 Context Panel 收起，工作区扩大；Context 与对话状态不变 |
| 19 | 退出专注模式 | 四栏布局恢复，编辑内容与对话不丢 |

**所有演示操作必须真实改变 Prototype 状态，不允许只做视觉上可点击的按钮。**

---

## Part E — 交付与验收标准

### AI-01 验收

- [ ] 1440px 下四区域尺寸准确（188 / 518 / 446 / 288，合计 1440）。
- [ ] **52px TopBar 保留并横跨主体工作区**，四栏位于 TopBar 下方。
- [ ] 四区域之间无未计入宽度的固定边距，视觉分隔由内边距与分隔线承担。
- [ ] 四区域职责明确。
- [ ] AI Chat 为**常驻独立列**，Composer 固定在列底；不随业务页面切换消失。
- [ ] AI Chat 与业务页面并行共存，无需跳转独立页面。
- [ ] Context Panel 固定四个主 Tab。
- [ ] 288px 内只展示摘要与状态，完整内容进详情 Drawer。
- [ ] History 使用 Drawer，而不是第五个 Tab。
- [ ] Project Selector 与 Project-bound Conversation 流程正确。
- [ ] **专注模式**：可进入可退出，退出恢复四栏，Project / Conversation / Context 不变，编辑与对话不丢。
- [ ] 四区域滚动互不干扰。
- [ ] 不存在区域重叠或宽度漂移。
- [ ] 视觉符合 V2 深色令牌（禁紫、禁渐变），且未改动既有 TopBar 与视觉系统。

### AI-02 验收

- [ ] Context 列表为**摘要级**呈现（名称 / 类型 / 状态 / 一行摘要 / 操作入口），完整内容在 Drawer。
- [ ] Context 状态清晰，**文字明示，不靠颜色**；归属轴与生命周期轴不混淆。
- [ ] Add Context 可用（多选、已添加标记、不可用项说明原因）。
- [ ] Enable / Disable 可用；Disable 不等于 Remove。
- [ ] Remove 有确认步骤。
- [ ] Context Details Drawer 可查看，含两轴状态。
- [ ] Sources、Results、Actions 三个 Tab 可用。
- [ ] History Drawer 可筛选、可查看详情，关闭后恢复原 Tab 与滚动位置。
- [ ] 状态变化同步更新（列表 / 统计 / 详情 / History 四处一致）。
- [ ] Overview 计数由原型状态驱动，非固定装饰数字。
- [ ] 项目之间的 Context 严格隔离。
- [ ] 专注模式进出时 Context / Conversation / Attached 项不受影响。

### 阶段门禁

- [ ] **AI-01 与 AI-02 全部通过后，才允许将 Shell 复制到其余五个页面。**
- [ ] 其余页面只能复用 Shell，不得自行修改四区域尺寸、主 Tab 结构、History 入口规则。

---

## 附：与既有裁决的关系

| 本文件条款 | 对应裁决 | 说明 |
|---|---|---|
| C3 Project/Conversation 边界 | DR-C6 | 与 00 §0.7、AI-01/AI-07 一致 |
| C4 权限与审批边界 | DR-C2 | 与 00 §0.6、AI-04/05/06 一致 |
| AI-01 七类 Card | DR-C4 | Artifact 内嵌，Risk 需结构化字段 |
| 四区域尺寸 | DR-C5 | 188/518/446/288 |
| History Drawer 五类 | DR-C3 | 不设第五主 Tab |
| **Context 双轴状态模型** | **DR-C7（已确认）** | 专家明确赞成保留双轴；**后端契约评审修正**：存储态只有 attached/detached，**Available 是候选查询结果不入库**；`read_only` 为独立可写性属性 |
| 四栏 Shell 结构升级 · 专注模式 · 密度规则 · 渐进迁移 | **DR-C8（已裁定）** | 见 95 裁决记录 DR-C8 |

**DR-C7 摘要**：Active / Inactive / Pending / Unavailable 是**生命周期轴**，与 Available / Attached / Read-only **归属轴**正交，不是替换关系。AI 实际使用的是「Attached + Active」。

**DR-C8 摘要**：四栏 Shell（188/518/446/288，**52px TopBar 保留在其上方**）为 V1.0 目标基线——在现有三栏壳上渐进升级，不另起视觉系统；Main Area 提供**专注模式**（收起 AI Chat 与 Context Panel，退出恢复四栏，状态与内容不丢）；288px 只放摘要、详情入 Drawer；实施按「① 四栏容器与 AI Chat 列 → ② 迁移现有会话能力 → ③ 升级 Context Panel」三步推进，每步可独立验收与回退。
