# 91 · 第一阶段 Prototype 契约（AI-01 + AI-02）

> 本文件是第一阶段的**原型状态与交互契约 + 演示场景 + 验收标准**，配合 [AI-01 v1-2](smark-pm-aic-ui-01-project-ai-workspace-v1-2.md) 与 [AI-02 v1-2](smark-pm-aic-ui-02-context-management-v1-2.md) 使用。
> 视觉与全局规则见 [00 主提示词](smark-pm-aic-00-design-master-prompt-v1-1.md)，裁决见 [95 裁决记录](smark-pm-aic-95-decision-record-v1.md)。

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

**所有演示操作必须真实改变 Prototype 状态，不允许只做视觉上可点击的按钮。**

---

## Part E — 交付与验收标准

### AI-01 验收

- [ ] 1440px 下四区域尺寸准确（188 / 518 / 446 / 288，合计 1440）。
- [ ] 四区域之间无未计入宽度的固定边距，视觉分隔由内边距与分隔线承担。
- [ ] 四区域职责明确。
- [ ] AI Chat 可持续显示，不随业务页面切换消失。
- [ ] Context Panel 固定四个主 Tab。
- [ ] History 使用 Drawer，而不是第五个 Tab。
- [ ] Project Selector 与 Project-bound Conversation 流程正确。
- [ ] 四区域滚动互不干扰。
- [ ] 不存在区域重叠或宽度漂移。
- [ ] 视觉符合 V2 深色令牌（禁紫、禁渐变）。

### AI-02 验收

- [ ] Context 列表完整（类型 / 来源 / 摘要 / 状态 / 更新时间 / 关联对象 / 操作入口）。
- [ ] Context 状态清晰，**文字明示，不靠颜色**；归属轴与生命周期轴不混淆。
- [ ] Add Context 可用（多选、已添加标记、不可用项说明原因）。
- [ ] Enable / Disable 可用；Disable 不等于 Remove。
- [ ] Remove 有确认步骤。
- [ ] Context Details 可查看。
- [ ] Sources、Results、Actions 三个 Tab 可用。
- [ ] History Drawer 可筛选、可查看详情，关闭后恢复原 Tab 与滚动位置。
- [ ] 状态变化同步更新（列表 / 统计 / 详情 / History 四处一致）。
- [ ] Overview 计数由原型状态驱动，非固定装饰数字。
- [ ] 项目之间的 Context 严格隔离。

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
| **Context 双轴状态模型** | **DR-C7（新增）** | 见 95 裁决记录 DR-C7 |

**DR-C7 摘要**：Phase 1 引入的 Active / Inactive / Pending / Unavailable 是**生命周期轴**，与既有的 Available / Attached / Read-only **归属轴**正交，不是替换关系。AI 实际使用的是「Attached + Active」。详见 [95 裁决记录](smark-pm-aic-95-decision-record-v1.md)。
