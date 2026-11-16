# PM-AI 前端导航改造说明书 V1.0

> **项目：** PM-AI  
> **改造对象：** ai-frontend  
> **基线依据：**《PM-AI 全局导航 IA 基线 V1.1》  
> **裁决依据：** P-01～P-10 正式裁决记录  
> **编制日期：** 2026-11-16  
> **文档状态：** 实施设计基线  
> **改造性质：** 前端导航架构、路由组织、Shell 布局及页面入口调整

---

# 1. 改造目标

## 1.1 总体目标

将 PM-AI 当前以单一全局左侧导航承载所有项目模块的结构，调整为：

**全局导航 + 项目内二级导航 + 领域独立页面入口。**

最终实现三个清晰的导航层次：

1. Personal：个人范围。
2. Team：团队范围。
3. Current Project：当前项目范围。

进入项目后，使用独立的项目内二级导航访问项目模块。

四个业务领域必须独立呈现：

- Analysis
- KB
- Product Cognition
- Project Cognition

## 1.2 本次改造解决的问题

| 编号 | 当前问题 | 目标 |
|---|---|---|
| NAV-01 | 项目模块全部放在全局左栏 | 建立项目内二级导航 |
| NAV-02 | 项目总控台缺少稳定导航入口 | 增加项目总控台入口 |
| NAV-03 | Baseline 页面孤立 | 总控台常驻 Baseline 状态卡片 |
| NAV-04 | KB、PC、Analysis 同页混合 | 四域独立入口 |
| NAV-05 | Project Cognition 无入口 | 建立独立项目认知入口 |
| NAV-06 | 开发环境归属不清晰 | 项目设置下独立开发环境 |
| NAV-07 | Devices、Executors、Runners 区分不足 | 分开呈现 |
| NAV-08 | 报告审阅与人工审核中心职责重叠 | Analysis 承载报告审阅，审核中心聚合跳转 |
| NAV-09 | Team 和 Project 导航边界不清 | 明确双层上下文 |
| NAV-10 | 现有管理组与 Team 设置分散 | 逐项核实并收敛 Team 配置 |

## 1.3 非目标

本次不实施：

- 四域后端对象模型重构。
- 数据库 Schema Migration。
- Analysis Report 正文迁移。
- KB Markdown/Git 权威迁移。
- Product Cognition 的 Product 资产归属迁移。
- Project Cognition 的业务生命周期重构。
- 后端权限体系整体重构。
- AI Agent 执行机制重构。
- 需求、设计、任务等业务模块的内部功能重构。

前端可以调整页面组织，但不得改变既有领域对象权威和审批语义。

---

# 2. 实施前置条件

## 2.1 必须读取的仓库文件

开发 Agent 开始修改代码前，必须读取以下原始文件。

### 项目规则

- 仓库根目录 `AGENTS.md`
- `ai-frontend/AGENTS.md`（如果存在）
- 相关子目录中的 `AGENTS.md`（如果存在）

重点核对：

- 规则 4：release-guard。
- 规则 5：Management Path 与 AI Assistant 隐藏规则。
- 测试命令与前端代码规范。
- 页面、路由和导航修改的约束。

禁止只根据审计报告中的规则摘要推断实现方式。

### 导航与路由

- `ai-frontend/src/router/navigation.ts`
- `ai-frontend/src/router/index.ts`
- `ai-frontend/src/components/shell/SideNav.vue`
- `ai-frontend/src/components/shell/TopBar.vue`
- `ai-frontend/src/components/shell/WorkspaceShell.vue`
- `ai-frontend/src/components/shell/use-shell-chrome.ts`
- `ai-frontend/src/components/shell/ContextPanel.vue`

### 项目页面

- `ai-frontend/src/views/project/command-center/`
- `ai-frontend/src/views/project/baseline/BaselinePage.vue`
- `ai-frontend/src/views/project/knowledge/KnowledgeView.vue`
- `ai-frontend/src/views/project/knowledge/AnalysisReportsPanel.vue`
- `ai-frontend/src/views/project/cognition/CognitionPage.vue`
- `ai-frontend/src/views/project/devenv/BootstrapView.vue`
- `ai-frontend/src/views/project/review-center/`

### 设置页面

- `ai-frontend/src/views/settings/nav.ts`
- `ai-frontend/src/views/settings/`
- `ai-frontend/src/locales/zh-CN/shell.yaml`

### 测试

检查：

- `ai-frontend` 中导航相关单元测试。
- 路由测试。
- Shell 布局测试。
- 页面入口测试。
- Playwright E2E 测试。

## 2.2 开工前必须形成源码核验记录

至少回答：

1. 当前导航是否仍然由 `navigation.ts` 单一数据源派生？
2. `NavItem`、`NavGroup` 是否支持嵌套结构？
3. 路由如何从导航配置生成？
4. 当前 `project_id` 是如何传递和保持的？
5. 当前项目切换如何影响页面状态？
6. `isManagementPath` 的真实用途是什么？
7. `ASSISTANT_HIDDEN_PREFIXES` 的实际判定规则是什么？
8. `AGENTS.md` 规则 5 对管理页面有什么具体要求？
9. `use-shell-chrome.ts` 如何控制四列布局？
10. Context Panel 是否依赖当前路由或页面类型？
11. Baseline 页面是否已有可复用的状态 API？
12. `/project/devenv` 是否已有设备管理以外的能力？

核验结果需要记录：

- 文件路径。
- 相关符号。
- 当前行为。
- 目标行为。
- 是否存在兼容风险。
- 具体改造建议。

未完成源码核验，不允许进入大规模重构。

---

# 3. 目标前端架构

## 3.1 导航结构

全局导航：

Personal
- 个人主页
- 通知中心

Team
- 团队总览
- 项目列表
- 团队设置
  - 团队常规
  - 成员与邀请
  - AI Provider
  - 团队公告（适用时）

Current Project
- 当前项目
- 项目总控台
- 进入项目工作区

项目内导航：

- 项目总控台
- 需求
- 技术设计
- 任务管理
- AI 执行中心
- 代码发现
- Analysis
- KB
- Product Cognition
- Project Cognition
- 人工审核中心
- 项目设置
  - 基础设置
  - 成员（规划项）
  - 仓库
  - 开发环境
    - Devices
    - Executors
    - Runners
  - AI 配置与执行策略

## 3.2 推荐组件结构

建议建立以下组件：

```text
components/shell/
├── WorkspaceShell.vue
├── GlobalNavigation.vue
├── GlobalNavigationGroup.vue
├── GlobalNavigationItem.vue
├── ProjectNavigation.vue
├── ProjectNavigationGroup.vue
├── ProjectNavigationItem.vue
├── TopBar.vue
├── ContextPanel.vue
└── use-shell-chrome.ts
```

以上为目标组件建议，不要求机械地按照文件名拆分。

开发 Agent 应优先复用现有组件，不应为了组件数量而过度拆分。

## 3.3 导航配置模型

优先扩展现有导航单一数据源。

建议将导航配置区分为：

- Global Navigation Config
- Project Navigation Config
- Project Settings Navigation Config

每项导航至少包含：

- 稳定 ID。
- 标题。
- 图标。
- 路由名称或路径。
- 上下文类型。
- 子项。
- 是否默认入口。
- 是否需要当前项目。
- 是否属于规划项。
- 可见性策略标识。

示意：

```ts
type NavigationScope =
  | 'personal'
  | 'team'
  | 'project'

interface NavigationItem {
  id: string
  title: string
  icon?: string
  routeName?: string
  path?: string
  scope: NavigationScope
  children?: NavigationItem[]
  requiresProject?: boolean
  planned?: boolean
}
```

该接口仅为设计示意，最终应适配仓库已有类型，不应直接覆盖已有类型定义。

---

# 4. 分阶段实施任务

## Phase 0：源码核验与改造准备

### 任务 NAV-0001：读取项目规则

目标：

确认 AGENTS.md 的实际约束。

交付：

- AGENTS 规则核验记录。
- 规则 4 和规则 5 影响分析。
- 需要修改的文件清单。

验收：

- 规则 4、5 已读取原文。
- 明确 AI 助手隐藏逻辑的正确扩展方式。
- 明确 release-guard 的执行要求。

### 任务 NAV-0002：导航与路由现状核验

目标：

确认当前导航、路由、Shell 的真实结构。

交付：

- 当前导航树。
- 当前路由清单。
- 当前项目上下文传递方式。
- 当前 Shell 列布局关系。
- 改造影响矩阵。

验收：

- 所有涉及的导航入口均有明确旧路径和新归属。
- 不存在未经核实的路径假设。

### 任务 NAV-0003：建立路由兼容清单

目标：

记录需要保留的旧路由与深链。

至少包含：

- `?doc=`
- `?object=`
- `?report=`
- `/project/knowledge`
- `/project/cognition`
- `/project/baseline`
- `/project/devenv`
- `/project/settings/*`
- 仓库和环境相关旧路由。

验收：

每个旧路由都有明确处理策略：

- 保留。
- 重定向。
- 解析旧参数后跳转。
- 暂时兼容。
- 经过明确确认后下线。

不得在没有迁移策略时直接删除旧入口。

---

## Phase 1：全局导航与项目内导航

这是第一优先级实施阶段。

### 任务 NAV-0101：建立全局导航与项目导航分层

目标：

将当前单一左栏改造为两个明确的导航层次。

要求：

- 未进入项目工作区时显示全局导航。
- 进入项目工作区后显示项目内导航。
- 当前 Team 和 Project 在顶栏始终可识别。
- 项目内导航不再与 Personal、Team 菜单平铺。
- 不引入 Workspace Switcher。

涉及文件：

- `navigation.ts`
- `SideNav.vue`
- `WorkspaceShell.vue`
- `TopBar.vue`
- `use-shell-chrome.ts`

验收：

- 全局导航不包含全部项目业务模块。
- 项目内导航能够显示全部项目模块。
- 切换项目时项目内导航正确更新。
- 项目导航折叠和展开正常。
- 刷新页面后当前项目上下文保持一致。

### 任务 NAV-0102：实现项目总控台入口

目标：

将 `/project/command-center` 作为项目总控台的正式入口。

要求：

- 项目总控台在项目导航中可见。
- 项目切换后能够进入当前项目总控台。
- 无当前项目时提供合理的选择项目引导。
- 页面入口不能依赖仅存在于代码注释中的默认落地行为。

特别要求：

现有 `api/mock/command-center.ts` 预览分支必须核验。

如果总控台作为正式主入口：

- 明确区分开发预览与正式运行。
- 按 AGENTS 规则 4 执行 release-guard。
- 禁止 Mock 数据误进入生产路径。
- 不能把 Mock 预览分支的存在误认为真实业务能力已经完成。

验收：

- 项目总控台可从项目导航直接进入。
- URL、当前项目和页面数据上下文一致。
- 生产环境不使用未经授权的 Mock 预览分支。

### 任务 NAV-0103：修复 Team 导航归属

目标：

将 Team 级页面归入 Team 导航和 Team 设置。

处理：

- 团队总览。
- 项目列表。
- 团队常规设置。
- 成员与邀请。
- AI Provider。
- 适用时的团队公告。

约束：

- V1 不建立 Platform Administration 固定分区。
- 不得只根据 `/admin/*` 路径判定业务归属。
- 必须逐页面确认数据范围和后端权限。
- 不得把所有现有 admin 页面未经核实地直接迁入 Team。

特别注意：

`/admin/settings`、`/admin/announcements` 当前业务归属和权限证据不足，应在源码与后端核验后逐项确定。

验收：

- Team 页面集中在 Team 导航体系。
- 不存在未经解释的 Team 页面跨组。
- 不存在把租户级配置错误标记为平台级的情况。
- 页面/API 权限仍由服务端控制。

### 任务 NAV-0104：处理 AI 助手隐藏规则

目标：

确保导航重构后 AI 助手和 AI Chat 的显示规则不发生意外变化。

必须检查四处：

1. 顶栏助手按钮。
2. AI Chat 列。
3. 右侧 Context Panel。
4. Shell 根节点相关属性。

实施约束：

- 必须先读取 AGENTS.md 规则 5。
- 必须理解 `isManagementPath` 的实际语义。
- 必须核验 `ASSISTANT_HIDDEN_PREFIXES`。
- Team 设置迁移后，旧管理路径判定不得失效。
- 不得通过硬编码前缀简单绕过既有规则。

验收：

对所有管理页面执行显示/隐藏回归测试。

---

## Phase 2：项目总控台与 Baseline

### 任务 NAV-0201：Baseline 状态卡片

目标：

在项目总控台常驻展示项目基线状态。

卡片至少包含：

- 当前基线状态。
- 已完成项目。
- 待确认项目。
- 阻塞项目。
- 最近更新。
- 进入基线确认入口。

状态必须来自真实数据。

不得使用静态演示数字冒充真实状态。

### 任务 NAV-0202：Baseline 确认入口

目标：

让用户能够从项目总控台进入现有 Baseline 确认流程。

要求：

- 卡片可进入 `/project/baseline`。
- 保留原有确认流程。
- 保留既有权限校验。
- 保留人工确认硬门。
- AI 只能建议，不能代替人工确认。
- 不允许导航入口绕过 `pm_baseline` 相关安全门。

验收：

- 用户可以从总控台到达 Baseline。
- 未授权主体不能完成正式确认。
- AI 不能通过页面操作绕过人工确认门。
- 页面刷新后状态一致。

---

## Phase 3：开发环境导航调整

### 任务 NAV-0301：开发环境归入项目设置

目标：

将 `/project/devenv` 归入项目设置下的「开发环境」。

建议导航：

项目设置
- 基础设置
- 成员
- 仓库
- 开发环境
- AI 配置与执行策略

### 任务 NAV-0302：Devices 独立呈现

目标：

独立呈现设备管理。

主要内容：

- 设备列表。
- 设备注册。
- 设备身份。
- 设备健康状态。
- 设备连接状态。
- 设备详情。

保留当前 BootstrapView 的真实设备 API 能力。

### 任务 NAV-0303：Executors 独立呈现

目标：

为 Executors 建立独立导航和页面入口。

注意：

必须先核实当前 Executor 对象、API 和页面能力。

若当前没有完整实现：

- 可以提供规划状态入口。
- 不得用 Devices 页面伪装 Executor 管理。
- 不得创造未经确认的后端 API。

### 任务 NAV-0304：Runners 独立呈现

目标：

保留现有 Runners 管理页面。

涉及现有页面：

- `/project/settings/runners`
- `/project/settings/runners/:deviceId`

要求：

- 保留原有路由。
- 保留 Runner 详情深链。
- 通过开发环境导航访问。
- 不与 Devices 合并。

验收：

Devices、Executors、Runners 三个入口语义独立，旧路由兼容。

---

## Phase 4：四域独立入口

这是较大的独立迁移阶段，应拆分执行。

### 任务 NAV-0401：KB 独立页面

目标：

将 KnowledgeView 中属于 KB 的内容明确归入 KB。

需要整理：

- Source。
- Document。
- Document Revision。
- Memory。
- Memory Proposal。
- Knowledge Review。

注意：

现有 KB 功能不一定全部具备页面。

缺少的功能标记为 PLANNED，不得通过空白占位页声称已经完成。

### 任务 NAV-0402：Analysis 独立页面

目标：

将分析报告和报告审阅能力从 KnowledgeView 拆出。

建议页面：

- Analysis Overview。
- Analysis Runs。
- Reports。
- Report Detail。
- Report Revision History。

具体页面层级可在详细 UX 设计中确定。

必须遵守：

- Report 属于 Analysis。
- Report Revision 属于 Analysis。
- 报告正文以 Git 为权威。
- DB 保存 Revision 身份、状态和 Git 引用。
- Report Revision 不依附 KB Document Revision。

本次只调整前端页面组织，不执行后端正文迁移。

### 任务 NAV-0403：Product Cognition 独立入口

目标：

将 Product Cognition 从 KnowledgeView 的页签中独立出来。

要求：

- 独立导航入口。
- 独立页面上下文。
- 独立对象详情。
- 保留 Proposal 审批入口。
- 不与 Project Cognition 共用一个无领域区分的对象列表。

V1 仍按照项目级使用。

### 任务 NAV-0404：Project Cognition 独立入口

目标：

将 `/project/cognition` 从孤立路由变成正式项目导航入口。

要求：

- 页面上下文必须是当前 Project。
- 保留原有 API。
- 保留现有认知三区结构。
- 保留冲突和确认相关语义。
- 不迁入 KB Memory。
- 不迁入 PC Cognition Object。

### 任务 NAV-0405：旧 KnowledgeView 拆分与兼容

目标：

逐步将原 `/project/knowledge` 页面拆分到各自领域。

必须建立参数映射：

| 旧参数 | 新目标 |
|---|---|
| `?doc=` | KB Document |
| `?object=` | Product Cognition 或其明确对应对象 |
| `?report=` | Analysis Report |

要求：

- 兼容历史链接。
- 保留对象 ID。
- 保留项目上下文。
- 保留必要的筛选和定位状态。
- 不允许参数映射错误。
- 不允许跨项目对象访问。

旧 `/project/knowledge` 可暂时作为兼容路由或过渡入口，但最终应明确其作为 KB 入口的定位。

---

## Phase 5：人工审核中心

### 任务 NAV-0501：审核中心聚合展示

目标：

将审核中心定位为跨领域待处理事项聚合入口。

展示：

- 来源领域。
- 待处理类型。
- 对象名称。
- 优先级。
- 当前状态。
- 摘要。
- 进入领域页面处理。

### 任务 NAV-0502：Analysis 审阅操作归位

目标：

将报告审阅操作全部归入 Analysis。

审核中心不得重复提供：

- 报告决定。
- 报告关闭。
- 报告批量发布。
- 报告 Revision 操作。

验收：

- 审核中心只聚合与跳转。
- Analysis 是唯一报告审阅操作入口。
- 不存在两个页面对同一报告执行不同决定的情况。

### 任务 NAV-0503：领域审核语义保持独立

目标：

保持各领域自己的审批语义。

例如：

- Analysis Report Review。
- KB Revision Confirmation。
- Product Cognition Proposal Approval。
- Project Cognition Conflict Resolution。

审核中心只统一入口和展示表达，不统一各领域的内部生命周期。

---

## Phase 6：项目设置与规划项

### 任务 NAV-0601：项目设置导航整理

目标：

统一项目设置的子导航。

建议：

- 基础设置。
- 成员（规划项）。
- 仓库。
- 开发环境。
- AI 配置与执行策略。

### 任务 NAV-0602：Project Members 规划入口

目标：

保留项目成员管理的导航规划位置。

约束：

- 不将 Team Members 当作 Project Members。
- 不将 Team Membership 直接解释为项目角色。
- 当前若无完整业务实现，应标记 PLANNED。
- 不得增加虚假的成员管理能力。

---

# 5. 路由改造规范

## 5.1 路由与导航单一数据源

尽量延续当前导航派生路由的架构。

要求：

- 导航项与路由定义保持一致。
- 不允许导航显示但路由不存在。
- 不允许路由存在但没有明确的可达入口（明确设计为深链的路由除外）。
- 不允许多个页面重复注册相同业务入口。

## 5.2 Project Context

必须统一当前项目上下文。

建议目标：

`shell.currentProjectId` 作为运行时当前项目上下文。

URL 中的 `project_id` 用于：

- 首次进入项目时初始化上下文。
- 分享和恢复项目链接。
- 旧链接兼容。
- 页面刷新恢复。

项目切换时：

1. 更新当前项目上下文。
2. 更新 URL。
3. 重新加载项目范围数据。
4. 清理不属于新项目的页面状态。
5. 阻止旧项目数据残留。

具体实现必须核验现有 store 和 URL 同步逻辑后确定。

## 5.3 路由命名

新路由应使用稳定、明确的命名。

建议语义：

- `project-command-center`
- `project-analysis`
- `project-analysis-report`
- `project-kb`
- `project-product-cognition`
- `project-cognition`
- `project-baseline`
- `project-devenv-devices`
- `project-devenv-executors`
- `project-devenv-runners`

实际命名以仓库现有规范为准。

## 5.4 深链兼容

迁移旧路由时，必须有兼容处理。

兼容逻辑应：

- 识别旧路由。
- 读取旧参数。
- 校验当前项目。
- 定位目标对象。
- 跳转到新的领域页面。
- 保留对象定位信息。

不得简单把所有旧链接重定向到目标页面首页。

---

# 6. Shell 与布局要求

## 6.1 全局 Shell

全局 Shell 负责：

- 顶栏。
- 全局导航。
- 全局搜索。
- 通知。
- AI 助手。
- 账户菜单。
- 右侧 Context Panel。

## 6.2 项目 Shell

项目 Shell 负责：

- 当前项目标题。
- 项目内导航。
- 项目页面内容。
- 项目上下文。
- 右侧 Context Panel。

## 6.3 桌面端布局

1440px 作为主要设计验收基准。

要求：

- 全局导航保持紧凑。
- 项目导航有独立宽度。
- 内容区优先保证可读性。
- Context Panel 可独立展开与收起。
- 项目导航支持折叠。
- 不因新增导航层次造成内容区过度压缩。

建议将现有列布局做成可配置布局，不应将列宽硬编码到各页面中。

## 6.4 响应式

本次首先保证桌面端。

窄屏下必须保证：

- 导航可以通过受控方式展开。
- 项目上下文可识别。
- 页面内容不被导航遮挡。
- 当前项目切换可用。

如果当前没有移动端抽屉实现，应单独标记，不得将未实现的响应式行为视为验收通过。

---

# 7. 权限与安全要求

## 7.1 导航可见性不等于权限

菜单隐藏不能代替后端权限校验。

前端必须区分：

- Navigation Visibility。
- Route Access。
- API Authorization。

## 7.2 项目上下文隔离

所有项目范围页面必须确认：

- 当前 Project ID。
- 用户是否有权访问该项目。
- 页面 API 是否使用正确项目上下文。
- 路由参数是否可能被恶意替换。

## 7.3 管理页面

管理页面应依据真实 Team 角色和后端权限显示。

不允许只通过路径前缀判断用户权限。

## 7.4 受保护操作

以下操作不能因页面拆分而改变原有安全门：

- Project Baseline Confirmation。
- KB Revision Confirmation。
- Product Cognition Proposal Approval。
- Project Cognition 决策。
- 其他受保护的人工审批。

---

# 8. 测试与验收

## 8.1 导航测试

至少覆盖：

- Personal 导航。
- Team 导航。
- Project 导航。
- Team 切换。
- Project 切换。
- 项目导航折叠。
- 项目导航展开。
- 刷新后导航恢复。
- 直接访问项目深链。

## 8.2 路由测试

至少覆盖：

- 新路由可访问。
- 旧路由兼容。
- `?doc=` 参数兼容。
- `?object=` 参数兼容。
- `?report=` 参数兼容。
- 无项目上下文处理。
- 错误项目 ID 处理。
- 无权限项目访问处理。

## 8.3 四域测试

必须验证：

- KB 不显示 PC 的正式对象管理操作。
- Product Cognition 和 Project Cognition 不混淆。
- Analysis Report 不再由 KB 页面作为正式审阅入口。
- 四域页面都能正确加载当前项目的数据。
- 跨域跳转保留对象来源和定位信息。

## 8.4 安全测试

至少验证：

- 未登录访问。
- 非项目成员访问。
- 错误项目 ID。
- 角色不足访问。
- 受保护操作被非授权主体调用。
- AI 主体尝试执行人工确认。
- 项目切换后的旧数据残留。

## 8.5 回归测试

必须检查：

- 顶栏助手入口。
- AI Chat 列。
- Context Panel。
- 管理页面 AI 助手隐藏规则。
- 通知跳转。
- 全局搜索跳转。
- 项目总控台跳转。
- 仓库和开发环境旧路由。
- 账户菜单。
- 新建项目入口。

---

# 9. 交付物要求

每个 Phase 必须交付：

1. 实施代码。
2. 变更文件清单。
3. 路由变更清单。
4. 导航变更清单。
5. 旧链接兼容清单。
6. 测试用例。
7. 测试执行结果。
8. 未完成项。
9. 风险与遗留问题。
10. 截图或录屏（涉及 UI 的任务）。

每个任务必须明确：

- 状态：NOT_STARTED / IN_PROGRESS / BLOCKED / COMPLETED。
- 实际修改文件。
- 测试命令。
- 测试结果。
- 未完成原因。

不能仅通过代码提交或页面截图宣称任务完成。

---

# 10. 实施顺序与依赖

| Phase | 内容 | 优先级 | 前置条件 |
|---|---|---|---|
| Phase 0 | 源码核验、AGENTS 检查、路由清单 | P0 | 无 |
| Phase 1 | 全局导航、项目导航、Team 导航 | P0 | Phase 0 |
| Phase 2 | 总控台与 Baseline | P0 | Phase 0；可与 Phase 1 部分并行 |
| Phase 3 | Devices / Executors / Runners | P1 | Phase 0；依赖开发环境对象核验 |
| Phase 4 | 四域独立页面 | P1 | Phase 0；依赖领域页面迁移设计 |
| Phase 5 | 人工审核中心 | P1 | Phase 4 的 Analysis 页面操作归位 |
| Phase 6 | 项目设置与成员规划 | P2 | Phase 0 |

说明：

- Phase 1 是首要实施阶段。
- Phase 2 可以并行开展，但 Baseline 确认安全门不能改变。
- Phase 4 应按领域分批实施。
- Phase 5 依赖 Analysis 审阅操作归位。
- Phase 6 的 Project Members 可以保留规划入口，不阻塞其他 Phase。
- Product 资产归属演进不属于本次前端 V1 实施范围。

---

# 11. 全局验收标准

以下条件全部满足，才可认为导航改造 V1 完成：

| 编号 | 验收项 | 标准 |
|---|---|---|
| AC-01 | 全局导航 | Personal / Team / Project 职责明确 |
| AC-02 | 项目导航 | 具有独立项目内二级导航 |
| AC-03 | 项目总控台 | 有稳定入口 |
| AC-04 | Baseline | 总控台常驻状态卡片与操作入口 |
| AC-05 | 四域 | 四个独立导航入口 |
| AC-06 | Analysis | 拥有报告审阅页面和操作 |
| AC-07 | 审核中心 | 只聚合和跳转 |
| AC-08 | 开发环境 | Devices / Executors / Runners 分开 |
| AC-09 | Team | 不设置固定 Platform Administration 分区 |
| AC-10 | Project Members | 保留规划位置并与 Team Members 区分 |
| AC-11 | 深链 | 旧链接兼容 |
| AC-12 | 权限 | 不降低现有安全门 |
| AC-13 | Shell | 顶栏、助手、Context Panel 正常 |
| AC-14 | 项目上下文 | 切换和刷新后保持一致 |
| AC-15 | 测试 | 关键导航、路由、安全回归测试通过 |

---

# 12. 开发 Agent 执行纪律

1. 严格遵循 IA V1.1，不擅自调整产品裁决。
2. 先读取 AGENTS.md，再修改代码。
3. 先完成 Phase 0，再开始 Phase 1。
4. 每个 Phase 单独提交，便于审查和回滚。
5. 不得擅自进行数据库 Schema Migration。
6. 不得为了 UI 导航方便合并四域对象。
7. 不得将领域审批统一迁入审核中心。
8. 不得移除旧深链而不提供兼容方案。
9. 不得用 Mock 数据冒充生产能力。
10. 不得将规划项显示为已完成能力。
11. 不得以隐藏菜单代替后端权限校验。
12. 所有测试必须实际执行并记录结果。
13. 遇到业务契约不明确时，先列出阻塞项，不得自行创造业务语义。

**最终目标：让 PM-AI 的前端导航结构准确体现产品的 Personal / Team / Project 三层上下文，以及 Analysis / KB / Product Cognition / Project Cognition 四域独立权威。**

---

# 附一：建议的开发任务拆分

为避免一次性改动过大，实际交给 Codex 或其他开发 Agent 时，按以下顺序逐个执行：

**第一批：导航框架（最高优先级）**

- NAV-0001～0003：源码核验
- NAV-0101：全局与项目导航分层
- NAV-0102：项目总控台入口
- NAV-0103：Team 导航归属
- NAV-0104：助手隐藏规则

**第二批：项目入口**

- NAV-0201～0202：Baseline
- NAV-0301～0304：开发环境

**第三批：领域拆分**

- NAV-0401～0405：四域独立页面与深链迁移
- NAV-0501～0503：审核中心调整

**第四批：收尾**

- NAV-0601～0602：项目设置与成员规划
- 全局回归与验收

# 附二：三个特别实施风险

1. **不要把 Phase 1 做成单纯的菜单搬家。**

   最关键的工作是 Shell 上下文切换。全局导航切换为项目导航后，项目 ID、路由、搜索、通知和 Context Panel 必须保持一致，否则视觉上完成了分层，实际使用却可能出现跨项目数据错配。

2. **四域拆分不要一次性删除 KnowledgeView。**

   先让 Analysis、PC、Project Cognition 有独立入口，再逐步迁移旧页面能力。尤其 `?doc=`、`?object=`、`?report=` 必须逐个验证定位结果。

3. **管理路径和助手隐藏规则是 Phase 1 的重点回归项。**

   这是本次导航重构最容易产生意外副作用的地方。必须把 AGENTS.md 规则 5 的原文和对应源码作为开工条件。
