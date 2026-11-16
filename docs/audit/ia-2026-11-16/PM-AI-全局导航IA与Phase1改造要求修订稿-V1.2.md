# PM-AI 全局导航 IA 与 Phase 1 改造要求修订稿 V1.2

> **文档类型**：架构裁决记录 + 导航 IA 基线修订 + 前端实施要求  
> **修订日期**：2026-11-16  
> **文档状态**：R-2 已正式裁决；导航 IA V1.2 基线生效  
> **前置基线**：全局导航 IA V1.1、Phase 0 前端导航改造源码核验报告 V1.0、R-1 SSH Keys 架构裁决记录 V1.0  
> **本次修订范围**：R-2 导航呈现架构、导航 IA、Phase 1 实施要求  
> **重要说明**：本文件是产品与架构基线修订，不代表前端代码已经完成相应改造。

---

# 第一部分：R-2 导航呈现架构正式裁决记录

## 1. 裁决结论

用户正式确认 R-2 五项裁决全部选择方案 A：

| 裁决编号 | 裁决事项 | 正式裁决 | 结果 |
|---|---|---|---|
| R2-D1 | 全局导航与项目导航呈现方式 | A：共用现有左侧导航栏，上下分层 | 已裁决 |
| R2-D2 | 是否增加第二条常驻侧栏 | A：不增加，沿用现有侧栏 | 已裁决 |
| R2-D3 | 项目上下文切换方式 | A：复用 Project Switcher 和现有切换守卫 | 已裁决 |
| R2-D4 | 项目导航显示条件 | A：存在当前项目时显示，无当前项目时隐藏 | 已裁决 |
| R2-D5 | 项目导航组织方式 | A：项目总控台置顶、业务模块居中、项目设置置底 | 已裁决 |

**R-2 最终架构结论：**

PM-AI 采用单一左侧导航栏承载 Personal、Team 和 Current Project 三种导航上下文。三种上下文在同一导航栏内分层呈现，不增加第二条常驻侧栏，不引入独立的工作区进入/退出模式。

项目上下文由现有 `shell.currentProjectId` 管理。项目切换必须继续使用 `requestProjectSwitch`，保留现有两阶段切换状态和活跃会话守卫。

## 2. 五项裁决的详细定义

### R2-D1：全局导航与项目导航共用现有左侧栏

**裁决：A**

Personal、Team 和 Current Project 不再被设计为三套彼此独立的导航容器，而是在现有左侧导航栏内形成三个具有明确边界的导航区域。

导航层次：

1. Personal：个人级入口。
2. Team：团队级入口。
3. Current Project：当前项目级入口。

三种上下文的菜单项不得扁平混排，也不得因为视觉共用一个侧栏而混淆其权限和数据作用域。

### R2-D2：不增加第二条常驻侧栏

**裁决：A**

继续使用现有 Shell 左侧导航栏：

| 状态 | 宽度 |
|---|---:|
| 展开 | 232px |
| 折叠 | 60px |

不得新增独立 Project Sidebar，不得将 Shell 改造为五栏常驻布局。

保留现有 AI Chat、Context Panel 和中央内容区域的布局关系。

实现时应复用现有 Shell Grid 和 `use-shell-chrome.ts` 的状态派生机制，避免引入没有必要的新布局状态和 Grid 模板分支。

### R2-D3：复用 Project Switcher 和项目切换守卫

**裁决：A**

项目切换是上下文切换，不是进入或退出一个独立工作区。

交互规则：

- 从项目列表选择项目：调用现有项目切换流程，切换成功后进入该项目总控台。
- 从 Current Project 区域切换项目：使用 Project Switcher。
- 切换操作必须调用 `requestProjectSwitch`。
- 必须保留现有活跃会话确认守卫。
- 切换期间 `currentProject` 应按现有机制置空，防止旧项目数据继续显示。
- 不得绕过现有两阶段项目切换状态。
- 不新增独立的"进入项目工作区""退出项目工作区"按钮或模式。

### R2-D4：有当前项目才显示项目导航

**裁决：A**

当 `shell.currentProjectId` 有效时，显示 Current Project 导航区域。

当没有当前项目时：

- 隐藏 Current Project 导航区域。
- Personal 和 Team 导航继续显示。
- 保留项目列表入口。
- 用户可通过项目列表或 Project Switcher 选择项目。
- 不显示一套没有实际项目上下文的空项目菜单。

不得以某个业务页面的路由是否匹配来代替项目上下文状态判断。

### R2-D5：项目导航采用固定层次

**裁决：A**

项目导航分为三个层次：

1. 项目总控台：置顶。
2. 项目业务模块：居中。
3. 项目设置：置底。

不得将项目设置与主要业务模块混排，也不得将所有项目模块作为无层次的平铺菜单。

## 3. 裁决依据

本次架构选择建立在 Phase 0 源码核验结果之上：

- 现有侧栏展开宽度为 232px，折叠宽度为 60px。
- 现有 Shell 已包含 AI Chat 446px、Context Panel 288px。
- Shell 已有五种 Grid 模板分支。
- 新增第二侧栏会增加布局模板、状态和响应式适配复杂度。
- 现有 `shell.currentProjectId` 是项目上下文运行时单一来源。
- 现有 `requestProjectSwitch` 已实现项目切换守卫。
- 现有 `ProjectSwitcher` 机制可以复用。
- 左栏项目上下文头曾因展开后占用空间过大而被注释停用，因此新设计必须采用紧凑形式，而不是直接恢复原来的大尺寸项目头。

## 4. 架构约束

以下约束从 R-2 裁决生效：

| 编号 | 架构约束 |
|---|---|
| R2-C01 | 不新增第二条常驻项目侧栏 |
| R2-C02 | 不新增独立工作区进入/退出状态 |
| R2-C03 | 项目上下文继续以 `shell.currentProjectId` 为运行时单一来源 |
| R2-C04 | 项目切换必须经过 `requestProjectSwitch` |
| R2-C05 | 保留活跃会话守卫和两阶段切换机制 |
| R2-C06 | 无当前项目时隐藏 Current Project 导航 |
| R2-C07 | 项目总控台置顶，项目设置置底 |
| R2-C08 | 不混淆 Personal、Team、Current Project 三种导航上下文 |
| R2-C09 | 不引入 Workspace Switcher |
| R2-C10 | 保持现有 232px / 60px 侧栏尺寸基线，除非后续独立裁决 |

---

# 第二部分：全局导航 IA 基线 V1.2

## 5. 导航结构

V1.2 将 R-1 和 R-2 的正式裁决合并到同一导航结构中。

```text
PM-AI
│
├── Personal
│   ├── 个人主页
│   └── 通知中心
│
├── Team
│   ├── 团队总览
│   ├── 项目列表
│   └── 团队设置
│       ├── 团队常规
│       ├── 成员与邀请
│       ├── AI Provider
│       ├── Git 凭证（SSH Keys）
│       └── 团队公告（适用时）
│
└── Current Project（有当前项目时显示）
    │
    ├── [紧凑 Project Switcher]
    │
    ├── 项目总控台
    │
    ├── 项目业务模块
    │   ├── 需求
    │   ├── 技术设计
    │   ├── 任务管理
    │   ├── AI 执行中心
    │   ├── 代码发现
    │   ├── Analysis（分析）
    │   ├── KB（知识库）
    │   ├── Product Cognition（产品认知）
    │   ├── Project Cognition（项目认知）
    │   └── 人工审核中心
    │
    └── 项目设置
        ├── 基础设置
        ├── 成员（规划项）
        ├── 仓库
        │   └── Repository 凭证绑定
        └── 开发环境
            ├── Devices
            ├── Executors
            └── Runners
```

## 6. 左侧导航的视觉与交互规则

### 6.1 导航区域

左侧栏由以下区域组成：

1. 产品标识区。
2. Personal 区域。
3. Team 区域。
4. Current Project 区域（条件显示）。
5. 底部账户或全局操作区域（沿用现有 Shell 能力）。

区域之间使用轻量分隔线或间距，不新增独立容器或第二个侧栏。

### 6.2 Project Switcher

Current Project 区域顶部采用紧凑项目上下文头：

- 显示当前项目名称。
- 提供下拉切换入口。
- 项目名称过长时截断显示。
- 下拉内容支持项目搜索和选择。
- 不展示大型项目描述、统计指标或多行元信息。
- 不重复创建第二个项目状态来源。

Project Switcher 必须复用现有项目切换逻辑，不允许直接修改 `currentProjectId`。

### 6.3 折叠状态

展开状态：

- 显示区域标题。
- 显示导航图标和文字。
- Project Switcher 显示项目名称及切换入口。
- 项目设置可展开子菜单。

折叠状态：

- 侧栏保持 60px。
- 只显示图标。
- 使用 Tooltip 提供完整菜单名称。
- Project Switcher 采用紧凑图标或等效入口。
- 不得因为折叠而丢失当前项目识别能力。
- 不得在折叠状态下新增第二层侧栏。

### 6.4 当前项目为空

当没有当前项目时：

- Personal 和 Team 保持可用。
- 项目列表保持可见。
- Current Project 整个区域不显示。
- 不显示灰色禁用的项目业务菜单。
- 不生成伪造的默认项目上下文。

## 7. R-1 与 R-2 合并后的凭证导航归属

R-1 已裁决 SSH Keys 是 Git 仓库访问凭证，采用：

**Team-owned Credential + Project-scoped Grant + Repository-level Binding + Execution-time Access**

导航必须遵守：

| 功能 | 导航归属 | 说明 |
|---|---|---|
| SSH Keys 凭证管理 | Team 设置 | Team 拥有凭证 |
| Project Credential Grant | Team / Project 授权流程 | 每个 Project 必须显式授权 |
| Repository Credential Binding | Project 设置 → 仓库 | 每个仓库独立选择凭证 |
| 执行时凭证访问 | Execution 运行流程 | Device / Executor / Runner 是使用者，不是凭证所有者 |

Development Environment 只包含 Devices、Executors、Runners，不再放置 SSH Keys。

前端不得把凭证私钥材料展示在普通页面，也不得将私钥写入普通 DB 明文、Markdown、Git、日志、任务输出或 AI 对话。

## 8. 四域导航规则

四域保持独立入口：

- Analysis：拥有分析报告和报告版本的正式审阅页面。
- KB：拥有知识来源、文档、知识审核和 Memory 等 KB 领域对象。
- Product Cognition：维护正式产品认知。
- Project Cognition：维护项目技术与项目认知。

导航入口独立不代表可以将四域对象合并。

人工审核中心只负责聚合待处理事项和跳转，不接管领域审批语义。

Analysis 报告审阅必须在 Analysis 页面完成；人工审核中心只提供待审阅入口和状态提示。

旧深链兼容要求继续有效：

- `?doc=` → KB 新页面。
- `?object=` → Product Cognition 或对应目标页面。
- `?report=` → Analysis 新页面。
- `/project/knowledge` → 最终 KB 入口，迁移期保留兼容。
- `/project/cognition` → 保留原路径。
- `/project/baseline` → 保留，并可从总控台 Baseline 卡片进入。

---

# 第三部分：Phase 1 前端导航改造要求修订

## 9. Phase 1 目标

Phase 1 的目标是建立可运行的三层导航模型，并完成单侧栏内的导航分层。

Phase 1 必须交付：

1. Global Navigation 配置模型。
2. Project Navigation 配置模型。
3. Settings Navigation 配置模型。
4. 多配置源路由派生。
5. 单一左侧栏分层呈现。
6. 紧凑 Project Switcher。
7. 项目上下文切换复用。
8. AI 助手隐藏规则迁移及回归测试。
9. `/inbox` 清理和兼容跳转。
10. Phase 0 已确认的必要注释和导航消费点修订。

**Phase 1 不负责完成四域所有业务页面拆分，也不负责重构后端项目权限模型。**

## 10. Phase 1 实施任务

### P1-01：导航配置模型拆分

**要求：**

将当前 `navigation.ts` 中的导航配置按上下文拆分为：

- Global Navigation Config
- Project Navigation Config
- Settings Navigation Config

注意：

- 三套配置仍服务于同一个左侧导航栏。
- 不得把三套配置理解成三套独立 Sidebar。
- `buildRoutes()` 必须能够消费多个配置源。
- `flatNavItems` 必须同步更新，保持命令面板与侧栏路由一致。
- 保持现有路由名称、权限和兼容路径。
- 不得因为拆分配置而改变业务页面的数据作用域。

**重点核验：**

`navigation.ts` 当前有 `navGroups` 和 `flatNavItems` 两类消费结构；路由由 `buildRoutes()` 遍历导航配置派生。拆分后必须检查所有消费方，不能只修改侧栏。

### P1-02：实现单侧栏上下分层

**要求：**

在现有 SideNav 中呈现：

- Personal
- Team
- Current Project

三个区域。

不得新增第二条常驻侧栏。

必须保持：

- 232px 展开宽度。
- 60px 折叠宽度。
- 现有 AI Chat 和 Context Panel 列。
- 现有 Shell Grid 的布局状态机制。

不得为 Project Navigation 单独创建新的 Shell Grid 列。

### P1-03：Project Switcher 与项目上下文

**要求：**

- 在 Current Project 区域顶部实现紧凑 Project Switcher。
- 当前项目名称来自现有项目上下文。
- 项目切换复用 `requestProjectSwitch`。
- 保留两阶段切换状态。
- 保留活跃会话守卫。
- 切换过程中避免旧项目数据泄漏。
- 不创建独立工作区进入/退出模式。
- 无当前项目时隐藏 Current Project 区域。

**禁止：**

- 直接绕过项目切换服务修改 `shell.currentProjectId`。
- 在侧栏组件中重新实现一套项目切换守卫。
- 用路由匹配结果替代当前项目状态。
- 在切换未完成时继续显示旧项目的导航选中状态和业务数据。

### P1-04：项目导航菜单顺序

**要求：**

严格按 V1.2 导航结构实现：

- 项目总控台置顶。
- 项目业务模块居中。
- 项目设置置底。

项目设置下：

- 基础设置
- 成员（规划项）
- 仓库
- 开发环境

开发环境下：

- Devices
- Executors
- Runners

Team 设置下：

- Git 凭证（SSH Keys）

Repository 凭证绑定放在 Project 设置 → 仓库中，不得放入 Development Environment。

### P1-05：AI 助手隐藏规则迁移

**要求：**

继续遵守根 `AGENTS.md` 规则 5：

- 助手可见性单一来源为 `use-shell-chrome.ts`。
- 路径判定通过 `isAssistantHiddenPath()`。
- 管理路径判定继续使用 `isManagementPath()`。
- 不允许业务页面自行增加 `v-if` 隐藏助手。
- 新增或迁移禁用路径时，必须更新 `ASSISTANT_HIDDEN_PREFIXES` 或相应管理路径判定。

由于 Team 设置的路由归属发生调整，必须重新核验：

- `/settings/team` 或最终实际 Team 设置路径。
- `/project/settings/*`。
- 项目设置子页面。
- 原 `/admin/*` 页面。

不能因为导航归属迁移就默认原有管理页面权限或助手隐藏规则仍然正确。

**回归要求：**

- AssistantColumn。
- ContextPanel。
- Shell 根节点属性。
- 顶栏助手入口。

四处可见性必须保持一致。

按 `ai-frontend/AGENTS.md` 要求，更新 `WorkspaceShell.test` 参数化路径用例，并执行 Vitest 和 Typecheck。

### P1-06：`/inbox` 清理与兼容

**要求：**

- 删除 `/inbox` 作为正式独立入口。
- 将旧 `/inbox` 跳转至 `/notifications`。
- 清理 `searchCommands` 中 `open-inbox` 命令。
- 清理对应 i18n key。
- 保持通知中心原有入口和业务行为。

### P1-07：导航相关消费点同步

必须同步核验：

- SideNav。
- Router `buildRoutes()`。
- `flatNavItems`。
- Command Palette。
- `searchCommands`。
- `use-shell-chrome.ts`。
- `WorkspaceShell`。
- TopBar。
- ContextPanel 的 `availableObject` 路由推导。
- ReviewCenter 的路由跳转。
- Global Search 的目标路由。

不得只完成导航视觉层而遗漏其他入口。

### P1-08：修正 Phase 0 已发现的注释漂移

允许顺手修正：

- `router/index.ts` 中 task breakdown 相关过期注释。
- `stores/shell.ts` 中过期 sidebar width 注释。

以 `tokens.css` 中的实际布局变量为唯一视觉数值来源。

不得借此扩大为无关的样式重构。

## 11. Phase 1 明确不做的事项

以下事项不属于 Phase 1 的交付范围：

1. 不增加第二条常驻侧栏。
2. 不新增进入/退出项目工作区模式。
3. 不重构项目切换底层机制。
4. 不改变 `shell.currentProjectId` 的单一来源定位。
5. 不修改 Shell 的 AI Chat / Context Panel 列结构。
6. 不删除 ContextPanel Graph Tab。
7. 不删除 `/project/visualizations`。
8. 不提前完成 KB、Analysis、PC、Project Cognition 的所有页面拆分。
9. 不迁移尚未确认归属的 `/admin/settings` 和 `/admin/announcements`。
10. 不在本阶段修改数据库 Schema。
11. 不将 Team Membership 直接当作 Project Member。
12. 不把 SSH Keys 放进 Development Environment。

## 12. Phase 1 Commit 切分建议

| Commit | 内容 | 前置条件 |
|---|---|---|
| C1 | 导航配置模型拆分 + 多源路由派生 | Phase 0 已完成 |
| C2 | Global Navigation 呈现 | C1 |
| C3 | Project Navigation 单侧栏分层 + Project Switcher | C1、C2 |
| C4 | Team 设置归属与路径迁移 + `isManagementPath` 更新 | C1 |
| C5 | AI 助手隐藏规则回归与 `/inbox` 清理 | 对应路径变更完成 |

Commit 应保持可独立审查，禁止将无关的存量 Prettier、i18n 或其他质量问题混入导航改造提交。

---

# 第四部分：验收标准与实施门禁

## 13. Phase 1 功能验收

| 编号 | 验收项 | 通过标准 |
|---|---|---|
| P1-AC-01 | 三层导航 | Personal、Team、Current Project 分层显示 |
| P1-AC-02 | 单侧栏 | 不存在第二条常驻侧栏 |
| P1-AC-03 | 侧栏尺寸 | 展开 232px、折叠 60px |
| P1-AC-04 | 项目显示条件 | 无当前项目时隐藏 Current Project |
| P1-AC-05 | Project Switcher | 能显示和切换当前项目 |
| P1-AC-06 | 项目切换安全 | 复用 `requestProjectSwitch` 和现有守卫 |
| P1-AC-07 | 项目导航顺序 | 总控台置顶、设置置底 |
| P1-AC-08 | SSH Keys 归属 | Team 设置下管理凭证 |
| P1-AC-09 | Repository 绑定 | 项目设置 → 仓库 |
| P1-AC-10 | Development Environment | 只包含 Devices、Executors、Runners |
| P1-AC-11 | AI 助手隐藏 | 四处可见性一致 |
| P1-AC-12 | `/inbox` 兼容 | 旧路径正确跳转通知中心 |
| P1-AC-13 | 命令面板 | 导航和命令入口一致 |
| P1-AC-14 | 深链 | 既有兼容路径不回归 |
| P1-AC-15 | ContextPanel | Graph Tab 和原有功能不受影响 |

## 14. 安全与状态验收

必须验证：

1. 从项目 A 切换至项目 B 时，项目 A 的数据不会在切换过程中残留。
2. 活跃会话存在时，切换仍触发原有确认守卫。
3. 用户取消切换后，原项目上下文恢复正确。
4. 项目切换失败时，不会留下错误的导航选中状态。
5. 无当前项目时，不能通过残留路由展示旧项目菜单。
6. Team 设置导航不构成权限授权；实际访问仍由后端权限控制。
7. SSH Credential 的授权、撤销和 Repository 绑定仍由人工操作控制。
8. 不因菜单显示而推导用户拥有相应业务权限。

## 15. 测试与质量门禁

Phase 0 基线：

| 检查项 | Phase 0 基线 |
|---|---:|
| Prettier | 21 files 存在格式问题 |
| i18n hardcode | 39 |
| i18n namespace | 47 namespaces / 5370 keys |
| Layout Test | 1 failed / 13 passed |
| SeeCode | 3 项既往 flaky 记录，尚未复跑 |

Phase 1 验收原则：

- 不新增存量质量红项。
- 导航改造引入的新增失败必须修复。
- 存量质量问题单独建任务，不混入导航提交。
- 新增或修改的视图必须有对应 Vitest。
- 执行 Typecheck。
- 执行 WorkspaceShell 导航和助手隐藏回归测试。
- 执行路由兼容测试。
- 执行项目切换安全测试。
- 执行 Release Guard。
- 确认生产构建不包含 Mock/fixtures/demo 内容。

## 16. Phase 1 完成定义

Phase 1 只有在以下条件全部满足时才可标记完成：

1. 三种导航上下文在同一侧栏正确分层。
2. 不新增第二条常驻侧栏。
3. Project Switcher 复用原有切换机制。
4. 项目切换守卫及防止旧项目数据泄漏的机制通过测试。
5. Team 设置、项目设置、开发环境导航归属符合 V1.2。
6. AI 助手隐藏规则完成迁移并通过回归。
7. `/inbox` 兼容和命令面板清理完成。
8. 所有相关导航消费点同步。
9. 深链兼容通过。
10. 测试、Typecheck 和 Release Guard 通过。
11. 没有新增未解释的质量红项。
12. 提交源码变更清单、测试结果、遗留问题和验收记录。

---

# 第五部分：后续阶段边界

## 17. Phase 1 与后续 Phase 的关系

| 阶段 | 主要工作 | 与 R-2 的关系 |
|---|---|---|
| Phase 0 | 源码核验 | 已完成 |
| Phase 1 | 三层导航模型与单侧栏实现 | R-2 直接约束 |
| Phase 2 | 项目总控台与 Baseline | 使用 Current Project 导航 |
| Phase 3 | Devices / Executors / Runners | 使用项目设置 → 开发环境 |
| Phase 4 | 四域独立页面和旧 KnowledgeView 拆分 | 使用项目业务模块导航 |
| Phase 5 | 人工审核中心 | 使用独立项目模块入口 |
| Phase 6 | 项目设置与成员规划 | 使用项目设置置底结构 |

Phase 1 不应等待 Phase 2～6 的所有业务页面完成。可以先建立稳定的导航结构，再逐步迁移具体页面。

## 18. 仍需单独处理的事项

本次 R-2 裁决不代表以下事项已经解决：

- `/admin/settings`、`/admin/announcements` 的最终归属，仍需 NAV-0103 证据核验。
- ContextPanel Graph Tab 的长期定位仍未裁决，当前必须保留。
- Product Cognition 中期向 Product 资产归属演进的身份模型尚未定义。
- Project Members 目前仍为规划项，不等于已有完整项目成员权限模型。
- SSH Credential 的 Schema、KMS、Provider 适配、临时注入和在途撤销协议仍需独立设计。
- 四域页面拆分及旧深链迁移仍按后续阶段实施。

---

# 第六部分：正式裁决登记

## 19. 裁决状态

| 决策编号 | 状态 | 后续处理 |
|---|---|---|
| P-01～P-10 | 已裁决 | 按既有导航 IA 执行 |
| R-1 SSH Keys | 已裁决 | Team Credential / Project Grant / Repository Binding |
| R2-D1 | 已裁决 A | 单侧栏上下分层 |
| R2-D2 | 已裁决 A | 不增加第二侧栏 |
| R2-D3 | 已裁决 A | 复用 Project Switcher 和切换守卫 |
| R2-D4 | 已裁决 A | 有当前项目才显示项目导航 |
| R2-D5 | 已裁决 A | 总控台置顶、设置置底 |
| NAV-0103 | 待核验 | 继续核实管理页面归属 |
| R-5 Graph | 待裁决 | 当前保持现状 |

## 20. 正式生效声明

本次 R-2 五项裁决均由用户明确选择 A，属于正式架构裁决，不再作为待讨论建议。

自本版本起：

- 《PM-AI 全局导航 IA 基线 V1.2》取代 V1.1，作为导航结构的当前基线。
- 《PM-AI 前端导航改造说明书 V1.0》中的 Phase 1 实施要求，应以本文件为修订依据。
- 若旧说明书与本文件冲突，以本文件中的 R-1、R-2 裁决及 Phase 1 修订要求为准。
- 后续开发 Agent 不得自行增加第二侧栏、工作区进入/退出模式或绕过项目切换守卫。
- 本次裁决只确定目标架构，不代表代码实现、测试或验收已经完成。

**最终架构原则：一个左侧导航栏，三个清晰导航上下文；项目上下文由现有机制管理，项目业务导航随当前项目动态呈现。**
