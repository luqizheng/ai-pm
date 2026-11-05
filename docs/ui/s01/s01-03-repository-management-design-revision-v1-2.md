# S01 设计修订 · 第三份：S01-03 · Repository Management 详细设计稿 V1.2（专家供稿记录）

> **转交日期**：2026-11-05。本文件是对用户转交的专家设计修订稿的**忠实记录**（以转交文本为准）。
> 文档性质（原稿自带）：S01-03 设计修订稿。修订目标：收编已确认的用户裁决，明确 Repository Management 的**长期管理定位**、功能边界、交互流程和验收标准。
> 上游关系：S01-02 Project Creation。下游关系：S01-06 Codex / MCP Bootstrap、项目代码分析、工程执行能力。
> **状态（原稿自带）：修订建议稿，待基于现有代码和最终 UX 核验后冻结**；§14「建议以 V1.2 作为 S01-03 的 UX/产品设计基线候选」，技术实现基线不因设计稿默认已有能力。
> 章节号完整（1~15，无缺号）。按既定协作约定：供稿阶段只记录不评审；评审须用户明确要求。

---

## 1. 产品定位

### 1.1 核心定义

Repository Management 是 Project 下**持续存在的工程资源管理能力**。

它负责管理：

- Project 与代码仓库之间的关联关系；
- 仓库身份及 Provider；
- 仓库访问授权与访问验证；
- 仓库在 Project 中承担的用途和角色；
- 默认 Branch 及其变化；
- 仓库代码发现、分析和重新扫描的入口；
- 仓库关联、解绑、恢复和变更的操作记录。

**Repository Management 不只是项目初始化的一步。** 首次接入 Repository 是一个常见场景，但不是它的全部生命周期。

### 1.2 产品原则

**Project 创建成功即可工作。**

用户没有接入任何 Repository 时：

- Project 仍然可以正常使用；
- 内置 AI-PM 仍然可以进行需求分析和需求建议；
- 可以整理项目知识；
- 可以进行人工决策和项目治理；
- 可以逐步建立项目目标、需求和项目基线。

Repository 的作用是**扩展**工程上下文和代码操作能力，**不是启用 AI-PM 的前置条件**。

### 1.3 与其他模块的边界

| 模块 | 负责内容 | 不负责内容 |
| --- | --- | --- |
| Project Creation | 创建 Project 主体 | 不负责仓库接入 |
| Project Command Center | 项目日常工作入口、待处理事项和初始化进度 | 不承担完整仓库管理操作 |
| Repository Management | 仓库关联、授权、验证、角色、Branch 和代码发现管理 | 不负责外部 Agent 运行环境 |
| AI-PM Cognition | 项目理解、需求分析、知识整理和建议 | 不依赖 Repository 才能启用 |
| Codex / MCP Bootstrap | 外部 Agent、MCP 和工程执行环境接入 | 不替代 Repository 管理 |
| Project Baseline | 项目正式基线管理 | 不由仓库 HEAD 变化自动改写 |

Repository 与 MCP 是不同维度的工程能力，**不应合并**为一个接入状态或一个配置表单。

---

## 2. 用户已确认的裁决

### 2.1 R-03-01：Project 支持多 Repository

**用户裁决：一个 Project 可以关联多个 Repository。**

例如：Backend API / Frontend / Shared Library / Infrastructure / Documentation。

每个 Repository 独立维护其身份、访问授权、Branch 和代码发现信息。

不得把一个 Project 与一个 Repository 设计成固定的一对一关系。

### 2.2 R-03-02：允许没有 Primary Repository

**用户裁决：可以暂时没有 Primary Repository。**

设计约束：

- 新建 Project 时不要求指定 Primary Repository；
- Project 可以关联多个 Repository，但暂时没有 Primary；
- Primary 是可选的项目级角色，不是仓库接入成功的必要条件；
- 不得因为没有 Primary Repository 将 Project 标记为不可用；
- 需要 Primary Repository 的具体业务场景，应明确提示用户，而不是在后台静默选择。

Primary Repository 的设置和变更必须经过**用户明确操作**。

### 2.3 R-03-03：优先 Provider OAuth / App 授权

**用户裁决：优先使用 Provider OAuth / App 授权。**

设计约束：

- 优先引导用户通过 GitHub、GitLab 等代码平台的授权流程；
- 授权范围应尽可能遵循最小权限原则；
- 展示授权主体、授权范围和授权状态；
- 授权失效时提示用户重新授权；
- 不得在 UI 中展示或明文保存用户的敏感凭据；
- 其他授权方式是否支持，应以 Provider 能力和现有后端实现为准。

不得将个人 Access Token 作为默认的首选体验。

### 2.4 R-03-04：Branch HEAD 变化由人类确认

**用户裁决：Branch HEAD 变化仅提示，由人类确认更新。**

设计约束：

- 系统可以检测当前 Branch HEAD 与已记录 Commit 的差异；
- 系统可以提示存在新的 HEAD；
- 不得仅因 HEAD 发生变化就自动更新用户已确认的基准；
- 用户确认后，才更新相应的当前基准信息；
- 已经被 Baseline 或 Agent Run 引用的历史 Commit 必须保留其历史含义；
- 不得因为更新当前 Branch HEAD 而覆盖历史执行所使用的 Commit。

---

## 3. Repository 关系模型

### 3.1 基本关系

建议模型：**Project 1 —— N ProjectRepository**

一个 Project 可以有零个、一个或多个 Repository。每个 Repository 关联一个 Project，并具有独立的访问状态和工程上下文。

> 注意（原稿自带）：此处描述的是 Project 与其关联 Repository 的产品关系。具体数据库实体、外键和约束必须与当前代码核验后确定。

### 3.2 Repository 信息

建议在 Repository 管理视图中呈现：

| 信息 | 说明 |
| --- | --- |
| Repository Name | 仓库名称 |
| Provider | GitHub、GitLab 等 |
| Repository URL | 仓库地址 |
| Repository Role | 在 Project 中承担的用途 |
| Primary | 是否为 Primary Repository |
| Default Branch | 当前配置的默认 Branch |
| Access Status | 仓库访问状态 |
| Last Verified | 最近一次访问验证时间 |
| Current HEAD | 最近读取到的 Branch HEAD |
| Confirmed Commit | 用户确认的当前 Commit 基准 |
| Discovery Status | 代码发现/分析状态 |
| Last Scanned | 最近一次代码扫描时间 |

不要把以上所有信息都塞进列表列中。**列表优先显示身份、用途、访问状态和 Branch**；其余信息在详情中查看。

### 3.3 Repository Role

建议支持以下用途分类：

- Backend
- Frontend
- Shared Library
- Infrastructure
- Documentation
- Other

这些是**业务用途标签**，不应与 Git Provider 类型混淆。用途分类应允许修改，不应把它设计成不可变的仓库类型。

---

## 4. 生命周期与操作

### 4.1 首次接入

推荐流程：

1. 用户进入 Project 的 Repositories 页面；
2. 点击 Connect Repository；
3. 选择代码平台；
4. 通过 Provider OAuth / App 完成授权；
5. 选择授权范围内的目标 Repository；
6. 系统读取 Repository 基本信息；
7. 用户确认该 Repository 在 Project 中的用途；
8. 用户选择或确认 Default Branch；
9. 系统执行访问验证；
10. 接入成功后显示 Repository 详情。

如果 Provider 暂不支持授权式选择仓库，可提供**受控的备用接入路径**，但不能因此把所有用户都引导到 Token 表单。

### 4.2 重复接入

如果目标 Repository 已经关联到当前 Project：

- 不得创建重复关联；
- 显示该仓库已经存在；
- 提供查看现有关联的入口；
- 如果用户是希望更新用途、Branch 或授权，应进入对应的管理流程。

如果该 Repository 已关联到**另一个 Project**，系统应按当前产品的跨 Project 共享规则处理。在跨 Project 规则未核验前，**不得假定**可以直接复用同一关联记录，也**不得静默**建立重复绑定。

### 4.3 修改 Repository

允许修改的内容建议包括：

- Repository Role；
- 用途说明；
- Default Branch；
- Primary Repository 标记；
- 授权状态（通过重新授权流程处理）。

修改 Repository 的业务角色，**不应改变 Repository 的真实身份**。

修改 Default Branch 后，应重新读取该 Branch 的 HEAD，并明确显示新的基准信息；**不得把旧 Commit 历史覆盖掉**。

### 4.4 解绑 Repository

建议采用 **Disconnect / Remove from Project** 的语义，而不是 Delete Repository。

解绑含义：

- 解除该 Repository 与当前 Project 的关联；
- 不删除代码平台上的真实仓库；
- 不应自动删除历史 Agent Run、Baseline、Discovery 或审计记录；
- 对已有历史记录，应保留当时引用的 Repository 身份和 Commit 信息；
- 如果解绑的是 Primary Repository，应提示用户确认，并允许项目暂时没有 Primary Repository。

是否支持恢复已解绑关联，应以现有数据模型和审计恢复能力核验后确定。**建议优先采用可审计的软解绑，而非不可逆删除。**

### 4.5 重新验证

用户可以主动发起 Verify Access。系统应展示：

- 验证是否成功；
- 验证时间；
- 失败原因；
- 是否需要重新授权；
- 当前授权是否仍有访问目标仓库的权限。

**验证失败不应直接将 Project 标记为 Failed。**

### 4.6 重新扫描 / 重新分析

Repository 的代码发现和分析属于**可重复执行**的操作。应区分：

- Repository 已连接；
- Repository 访问验证通过；
- Repository 已完成代码发现；
- 最近一次代码分析成功；
- 是否存在新的 HEAD。

这些**不是同一个状态**。重新扫描不等于重新接入，也**不应要求用户重新执行 Project 初始化**。

---

## 5. Branch 与 Commit 基准治理

### 5.1 三个不同概念

必须区分：

1. **Default Branch**：仓库当前配置的默认分支；
2. **Observed HEAD**：系统最近观察到的 Branch HEAD；
3. **Confirmed Commit**：用户确认采用的 Commit 基准。

Observed HEAD **可以**自动检测变化。Confirmed Commit **不得**因为检测到变化而自动更新。

### 5.2 HEAD 变化提示

建议交互：

- 当前确认基准：`abc1234`
- 检测到新 HEAD：`def5678`
- 状态：New commits detected
- 操作：Review changes / Confirm update

用户确认前：

- 当前 Confirmed Commit 保持不变；
- Baseline 不自动变化；
- 正在运行或历史 Agent Run 的 Commit 引用不变化；
- 系统可以展示新旧 Commit 的差异信息。

用户确认后：

- 更新对应的当前确认基准；
- 记录确认人和确认时间；
- 保留旧 Commit 的历史记录；
- 对受影响的 Baseline 或执行上下文提供明确提示。

### 5.3 执行时固定版本

**设计要求：任何工程执行任务都必须能够还原实际使用的 Repository 和 Commit。**

不能只记录「使用了 payment-service 仓库」或「使用 main 分支」。应记录实际执行时的 Commit SHA，以及任务使用的 Repository 集合。

具体快照结构、是否支持单个 Run 多仓库，需与现有执行模型核验。

---

## 6. 页面信息架构

Repository Management 是 Project 下的**长期管理页面**。

建议入口：

```
Project Navigation
- Project Command Center
- Requirements
- Discovery
- Knowledge
- Repositories
- Agent / Execution
- Settings
- Audit
```

Repositories 是**独立页面，不是创建 Project 的必经步骤**。

Project Command Center 可以显示 Repository 接入进度摘要，但完整的添加、修改、验证、解绑和扫描操作都在 Repositories 页面完成。

### 6.1 页面布局

页面分为：

1. 页面标题区；
2. 仓库概览；
3. 搜索与筛选；
4. Repository 列表；
5. Repository 详情面板；
6. 接入、验证、扫描等操作入口。

### 6.2 页面标题区

- 标题：Repositories
- 副标题：Manage repositories connected to this project.
- 主操作：Connect Repository
- 辅助操作：Refresh

**不要**在标题区显示「初始化步骤 3/5」一类流程进度。Repository Management 是长期功能，不是只能完成一次的向导。

### 6.3 概览区

建议显示：

- Linked Repositories
- Access Verified
- Needs Attention
- Discovery Activity

这些是**管理摘要**，不是 Project 的整体状态。

没有 Repository 时，显示明确的空状态：

> "No repositories connected"
>
> "Your project is ready to work. Connect a repository when you need code context or engineering execution capabilities."

提供 Connect Repository 操作，同时说明**需求管理和内置 AI-PM 不受影响**。

### 6.4 Repository 列表

建议列：

| 列 | 内容 |
| --- | --- |
| Repository | 名称、Provider、URL |
| Role | Backend / Frontend 等 |
| Branch | Default Branch |
| Access | Connected / Needs attention |
| Discovery | Not started / Running / Completed / Failed |
| Last Verified | 最近验证时间 |
| Actions | 更多操作 |

筛选条件：Repository Name / URL；Provider；Role；Access Status；Discovery Status。

**不要**把 Commit SHA、所有验证细节和历史扫描记录塞进列表。

### 6.5 Repository 详情

点击仓库进入详情面板或详情页。建议分区：

- Overview：仓库身份、用途、Primary 标记；
- Access：授权主体、授权范围、访问验证；
- Branches：Default Branch、Observed HEAD、Confirmed Commit；
- Discovery：代码发现与分析记录；
- Activity：关联变更、授权、验证和扫描历史。

### 6.6 更多操作菜单

建议包含：

- View Details
- Verify Access
- Change Role
- Change Default Branch
- Set as Primary
- Remove from Project

操作是否可用由当前权限和 Repository 状态决定。

---

## 7. 与 Project Command Center 的关系

Project Command Center 是项目创建后的默认工作入口。

Repository Management **不应取代 Command Center，也不应成为进入项目工作的前置条件**。

Command Center 中可以有 Repository 状态摘要：

- No Repository Connected
- Repository Connected
- Access Needs Attention
- New HEAD Detected
- Discovery In Progress

摘要只承担**提醒和导航**职责。例如："2 repositories connected · 1 needs attention"，点击后进入 Repositories 页面。

初始化进度可以引用 Repository 接入状态，但**不能将 Repository 的未完成状态解释为 Project 尚不可工作**。

---

## 8. 权限、审计与安全

以下操作必须进行权限校验：

- Connect Repository；
- Disconnect Repository；
- Set / Change Primary Repository；
- Change Repository Role；
- Change Default Branch；
- Reauthorize Provider；
- Confirm Commit Update；
- Trigger Discovery / Rescan。

建议审计记录：操作类型；Project ID；Repository ID；操作人；操作时间；操作前后关键值；操作结果；失败原因（如有）。

安全要求：

- Provider 凭据不得明文展示；
- 授权范围应最小化；
- 访问验证失败时不得泄露仓库内部信息；
- Repository URL 和 Provider 返回内容均按不可信输入处理；
- 解绑不得意外删除远端仓库；
- 不能因仓库 HEAD 更新而绕过人工确认。

---

## 9. 状态模型

Repository Management **不得把多个维度合并成一个简单的 Connected / Disconnected 状态**。

建议分维度表达：

| 维度 | 示例 |
| --- | --- |
| Association | Linked / Removed |
| Authorization | Authorized / Expired / Revoked |
| Access Verification | Verified / Failed / Not Verified |
| Discovery | Not Started / Running / Completed / Failed |
| HEAD | Up to Date / New HEAD Detected |
| Primary Role | Primary / Non-primary |

这些是 **UX 语义建议**，具体状态枚举必须核验现有后端模型后再冻结。

特别注意：

- Project Status 与 Repository 状态独立；
- Initialization Status 与 Repository 状态独立；
- Baseline Status 与 Repository 状态独立；
- Agent / MCP Connection Status 与 Repository 状态独立；
- 不存在 Repository 不等于项目失败；
- Access Verified 不等于 Discovery Completed；
- Discovery Completed 不等于 Execution Ready。

**Execution Ready 应是执行条件的派生判断，不应由 Repository 页面手动设置。**

---

## 10. 与现有代码的核验清单

以下内容**不得仅凭 UX 原型认定为现有能力**，必须由技术侧核验：

1. 当前 Project Repository 数据模型是否支持多仓库；
2. Repository 是否允许暂时没有 Primary；
3. 当前 Provider 支持范围；
4. GitHub / GitLab OAuth 或 App 授权是否已经实现；
5. 当前授权凭据的存储和撤销机制；
6. Repository URL 的重复关联检测；
7. Repository 与 Project 的跨项目关系约束；
8. Disconnect 是否已有软删除或归档语义；
9. Repository Role 是否已有持久化字段；
10. Default Branch 与 HEAD 的读取方式；
11. HEAD 检测是否已有独立机制；
12. Confirmed Commit 是否有明确的数据承载位置；
13. Baseline / Agent Run 是否保存不可变 Commit 引用；
14. Discovery 是否可以独立重跑；
15. 多 Repository Discovery 是否支持部分失败和单仓库重试；
16. Repository 的所有变更是否都有审计；
17. 页面统计数字是否有可靠的数据接口。

### 10.1 特别关注的已有代码风险

历史代码核验资料中曾指出：

- Repository 的 created / updated / deleted / archived / verified 操作存在审计记录；
- Repo Import 的创建、取消和 apply 操作曾存在审计缺口；
- 多 Repository Import 的数据模型允许多个 Source，但一个 Source 失败可能导致整个 Import 失败；
- Source 级独立重试能力曾被标记为不存在；
- Import 重跑可能覆盖同一 Source 的 Commit 基准。

这些是历史核验资料中的代码事实线索，**不应直接推断为当前代码仍然如此**。技术侧需要在当前分支重新核验。

尤其**不能把「多仓库关联」误认为「多仓库代码发现已经具备完整的部分成功、独立重试能力」**。

---

## 11. Figma AI · UX/UI 设计提示词

> 请设计一个企业级 AI Project Management 产品中的 Project Repository Management 页面。

**产品定位**：

Repository Management 是 Project 下长期存在、可以反复执行的代码仓库管理功能，**不是一次性初始化向导**。

**设计必须遵循**：

- Project 创建后即可工作；
- Repository 是可选工程集成；
- 一个 Project 可以关联多个 Repository；
- 可以暂时没有 Primary Repository；
- 优先使用 GitHub / GitLab Provider OAuth 或 App 授权；
- Branch HEAD 变化只提示用户，由人类确认更新；
- Repository 管理与 MCP / Codex / 外部 Agent 接入分离；
- 不引入 Workspace 业务概念。

**页面结构**：

1. 全局 Project Navigation；
2. 页面标题 Repositories；
3. 页面说明 Manage repositories connected to this project；
4. 主按钮 Connect Repository；
5. 概览指标 Linked Repositories、Access Verified、Needs Attention、Discovery Activity；
6. 搜索框和 Provider / Role / Status 筛选；
7. Repository 数据表；
8. Repository Detail 侧边面板；
9. Connect Repository 弹窗或分步授权流程；
10. Branch HEAD Change 人工确认提示；
11. No Repository Connected 空状态。

**Repository 列表至少展示**：Repository Name / Provider / Role / Default Branch / Access Status / Discovery Status / Last Verified / Actions。

**详情面板展示**：Repository Identity / Authorization / Access Verification / Default Branch / Observed HEAD / Confirmed Commit / Discovery History / Activity Log。

**连接流程**：Select Provider → Authorize Provider App → Select Repository → Assign Role → Select Default Branch → Verify Access → Connected。

如果已有重复关联，必须展示提示，不创建重复关联。

**Branch HEAD 变化提示**：显示当前 Confirmed Commit 与新 Observed HEAD 的差异，并提供 Review Changes 和 Confirm Update。**禁止自动更新 Confirmed Commit**。

**视觉要求**：

- 专业、克制、清晰的 SaaS 管理后台；
- 深色与浅色主题均需适配；
- 信息密度适中，仓库身份和访问状态优先；
- 状态使用清晰但不过度醒目的标签；
- 详情面板承载低频信息；
- 空状态明确传达项目仍然可工作；
- 桌面端优先，同时适配平板和移动端。

**禁止**：

- 不要把 Repository Management 设计成初始化 Wizard；
- 不要要求用户必须先连接 Repository 才能进入 Project；
- 不要把 MCP、Codex、Agent 环境配置放进 Connect Repository 表单；
- 不要把 Project Status、Initialization Status、Repository Status 合并；
- 不要自动把 Branch HEAD 更新为 Confirmed Commit；
- 不要引入 Workspace 作为业务实体；
- 不要使用虚构的执行就绪状态作为人工可编辑字段。

**请交付**：

- Repository Management 主页面高保真 UI；
- No Repository Connected 空状态；
- Connect Repository 授权流程；
- Repository Detail；
- Branch HEAD Change 人工确认状态；
- Remove from Project 确认弹窗。

---

## 12. 关键线框

**主页面**：

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Project Navigation                                                       │
├──────────────────────────────────────────────────────────────────────────┤
│ Projects / payment-platform / Repositories                              │
│                                                                          │
│ Repositories                            [Refresh] [ + Connect Repository]│
│ Manage repositories connected to this project.                          │
│                                                                          │
│ ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌─────────────┐ │
│ │ Linked Repos   │ │ Access Verified│ │ Needs Attention│ │ Discovery   │ │
│ │       2        │ │       2        │ │       0        │ │ In Progress │ │
│ └────────────────┘ └────────────────┘ └────────────────┘ └─────────────┘ │
│                                                                          │
│ [Search repositories...] [Provider ▼] [Role ▼] [Status ▼]               │
│                                                                          │
│ Repository          Role       Branch   Access      Discovery     ...    │
│ ──────────────────────────────────────────────────────────────────────── │
│ payment-service     Backend    main     Verified    Completed     ⋯      │
│ payment-web         Frontend   main     Verified    Running       ⋯      │
│                                                                          │
│                                                                          │
│ Repository Baseline Information                                         │
│ Each repository has its own Branch and Commit context.                  │
│ Execution records preserve the actual Commit used.                       │
└──────────────────────────────────────────────────────────────────────────┘
```

**无仓库状态**：

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Repositories                                           [+ Connect Repo]  │
│                                                                          │
│                              [Repository Icon]                           │
│                                                                          │
│                       No repositories connected                         │
│                                                                          │
│       Your project is ready to work. Connect a repository when           │
│       you need code context or engineering execution capabilities.       │
│                                                                          │
│                         [Connect Repository]                             │
│                                                                          │
│       Requirements and built-in AI-PM remain available without           │
│       a connected repository.                                            │
└──────────────────────────────────────────────────────────────────────────┘
```

**HEAD 变化提示**：

```
┌────────────────────────────────────────────────────────────┐
│ New commits detected                                       │
│                                                            │
│ Repository: payment-service                                │
│ Branch: main                                                │
│                                                            │
│ Confirmed Commit       abc1234                              │
│ Observed HEAD          def5678                              │
│                                                            │
│ The branch has advanced. Your confirmed baseline has       │
│ not been changed.                                          │
│                                                            │
│ [Review Changes]                       [Confirm Update]     │
└────────────────────────────────────────────────────────────┘
```

---

## 13. 验收标准

| 编号 | 验收项 | 通过标准 |
| --- | --- | --- |
| AC-01 | 无仓库项目 | 没有 Repository 时，Project 仍可正常工作 |
| AC-02 | 多仓库 | 一个 Project 可关联多个 Repository |
| AC-03 | 无 Primary | 没有 Primary Repository 时不阻断项目使用 |
| AC-04 | Provider 授权 | 首选 OAuth / App 授权流程 |
| AC-05 | 重复关联 | 不创建重复 Repository 关联 |
| AC-06 | 用途维护 | 可以修改 Repository Role |
| AC-07 | Branch 维护 | 可以管理 Default Branch |
| AC-08 | HEAD 变化 | 检测变化后只提示，不自动更新确认基准 |
| AC-09 | 人工确认 | 只有用户确认后才更新当前确认基准 |
| AC-10 | 历史 Commit | 不覆盖历史 Baseline / Agent Run 使用的 Commit |
| AC-11 | 解绑 | 解除 Project 关联，不删除远端仓库 |
| AC-12 | 重复执行 | 可以多次验证、扫描和管理，不需要重跑 Project 初始化 |
| AC-13 | 状态隔离 | Repository 状态不等于 Project / Initialization / Baseline 状态 |
| AC-14 | 权限审计 | 关键操作经过授权并留下审计记录 |
| AC-15 | Command Center | 只显示状态摘要，完整管理仍在 Repositories 页面 |

---

## 14. 本稿冻结建议与待核验项

**可以作为设计基线收编的已裁决事项**：

- 多 Repository；
- Primary Repository 可为空；
- 优先 Provider OAuth / App 授权；
- Branch HEAD 变化只提示，由人类确认更新；
- Repository Management 是长期可重复执行的管理能力；
- Repository 不是项目可工作的前置条件；
- Repository 与 MCP / Codex Bootstrap 分离。

**设计建议，尚不等同于用户裁决**：

- Disconnect / Remove from Project 的具体交互；
- Repository Role 的枚举；
- 详情页的信息分区；
- 无仓库空状态文案；
- 是否支持解绑后的恢复；
- 具体审计字段；
- 具体状态枚举。

**必须由技术侧核验**：

- 现有数据库与 API 是否支持以上关系；
- OAuth / App 授权当前实现情况；
- Primary Repository 的现有字段和约束；
- Branch HEAD 检测与确认基准的现有数据结构；
- 多仓库 Import 的部分失败和独立重试；
- 历史 Commit 快照与审计完整性。

**修订结论**：建议以 V1.2 作为 S01-03 的 UX/产品设计**基线候选**。技术实现基线需等待现有代码核验，**不应因设计稿而默认已有全部能力**。

---

## 15. 本轮修订中需要特别纠正的原型问题

> 我检查了此前的 `s01-repository-management-prototype.html`，它有可复用的基础：多仓库列表、筛选、接入弹窗、Repository 基准信息和详情侧栏。但有几处需要调整：
>
> - 原型出现了 **Workspace Mode**，应删除；产品没有 Workspace 业务概念。
> - 原型中的 Execution Ready 提示方向是对的，但**不能让用户在仓库页手动设置**。
> - 目前原型将 Access Status 和 Discovery Activity 分开展示，这一点**应保留**。
> - 原型的 Connect Repository 仍以手工填写 URL 为主，应**调整为 Provider OAuth / App 优先**。
> - 原型没有完整体现 HEAD 变化后由人类确认的流程，需要**补充专门状态**。
>
> 这些判断基于已找到的原型内容。

---

## S01-03 当前结论（原稿自带）

产品设计方向已经明确，但**不建议现在就把技术实现冻结**。尤其是 OAuth/App 授权、Primary Repository、Confirmed Commit 和多仓库扫描的部分失败处理，需要技术侧逐项核验。

---

## 附 A · 记录者附注：与既有文档的关系登记（事实登记，非评审）

| # | 既有文档/事实 | 关系登记 |
| --- | --- | --- |
| A-1 | 既有稿 `s01-03-repository-management-ux-v1-1.md` | **四裁决正文已定位**：该稿 :48 §3.1 多仓库模型、:68-88 §3.2 Primary 规则（可零个/最多一个/不得自动提升第一个接入仓库/断开后不擅自选择替代对象）。本稿 R-03-01~04 与既有稿裁决同源一致；V1.2 新增=长期管理定位、Branch/Commit 三概念治理、页面信息架构、Figma 提示词、线框、AC-01~15、17 项核验清单、§15 原型纠正 |
| A-2 | Role 词表三方差异（待收敛） | 本稿 §3.3 建议 Backend/Frontend/Shared Library/Infrastructure/Documentation/Other；后端既有 `project_repository.role ∈ main/docs/service/infra`（S01-01 总览稿 §3.2.1 已登记）；前端既有词表 `primary|supporting|infrastructure|test`（总览稿 §3.2.3 已登记为词表冲突）。本稿「用途分类应允许修改」与后端 CHECK 约束的关系=§10-9 核验项 |
| A-3 | 状态维度 vs 后端既有枚举 | 本稿 §9 六维度中，后端已有 `project_repository.status ∈ pending/verified/unreachable/archived`（0009 迁移，总览稿 §3.2.1 已登记）——`archived` 与本稿 §4.4「软解绑优先」建议同向（对应 §10-8 核验项）；Authorization 维度当前后端无 git-providers 表与端点（总览稿 §3.2.2 已登记为 G-04 缺口）→ R-03-03 的落点依赖 G-04 |
| A-4 | Primary 无后端落点 | 总览稿 §3.2.3 已登记后端无 `is_primary`/`primary_repository_id`（G-03 待裁决：新增持久化字段 vs `role='main'` 派生）→ 本稿 R-03-02 与 §6.6 Set as Primary 的实施依赖 G-03 裁决 |
| A-5 | HEAD 人工确认无承载位置 | 总览稿 §3.2.4 已登记「人类确认更新」无确认记录表/端点（G-05）→ 本稿 §5.2 Confirm Update 流程的实施依赖 G-05 |
| A-6 | 原型文件位置 | §15 所指 `s01-repository-management-prototype.html` **未在 templates 树内检索到**（含并行 worktree ai-pm-nav）；S01-03 Gen UI 交互 HTML 此前经 Gen UI 渠道交付、未入仓。专家判断基于其自有检索到的原型内容，原件归档位置待确认 |
| A-7 | Command Center / 执行就绪 | 本稿 §7「摘要+导航、完整操作在 Repositories 页」与第一份修订稿（Command Center V1.1）§3.5/§6 一致；「Execution Ready 是派生判断，不可手动设置」与既有 Codex/MCP Bootstrap 稿及 S01-01 总览稿 §3.5.2「执行就绪是派生量」裁决同向 |
| A-8 | 工作包编号 | 本稿保持 S01-03=Repository Management，与第一份修订稿 §九 顺序表一致 |

## 附 B · 待基线确认后的执行挂账

1. G-03（Primary 落点）/ G-04（OAuth 数据与端点）/ G-05（HEAD 确认承载）三个后端缺口是本稿四裁决中三项的实施前提；本稿 §10 的 17 项核验清单与 S01-01 总览稿 §14 缺口表须对齐合并，避免两套清单漂移。
2. §15 原型修订五点（删 Workspace Mode、Execution Ready 改派生、保留 Access/Discovery 分维、OAuth 优先接入流、补 HEAD 人工确认态）待原型原件定位后执行。
3. 本稿状态=修订建议稿：UX 基线候选待用户基线确认；技术实现基线待 §10 清单核验；记录阶段不执行冻结、不改既有稿状态。
4. §9 状态枚举六维度按原稿约束「核验现有后端模型后再冻结」。
