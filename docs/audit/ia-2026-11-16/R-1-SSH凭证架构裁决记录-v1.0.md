# SSH Keys（Git 仓库访问凭证）架构裁决记录 V1.0

- 裁决编号：R-1
- 所属项目：PM-AI
- 日期：2026-11-16
- 状态：**已裁决**
- 适用范围：Team / Project / Repository / Development Environment
- 裁决主题：Git Repository SSH 访问凭证的所有权、授权、绑定及使用边界

## 1. 问题背景

PM-AI 需要支持 AI Agent、Device、Executor、Runner 等工程执行工具通过 SSH 访问 Git Repository，包括 Clone、Fetch、Pull、Push 等操作。

现有前端导航中存在 SSH Keys 相关设置入口，但其归属不明确，需要确定 SSH 凭证应属于 Team、Project 还是 Development Environment。

本次裁决明确：SSH Keys 仅用于 Git Repository SSH 认证，不包含服务器 SSH 登录和用户个人 SSH 公钥管理。

## 2. 正式裁决

### D1：Team 凭证能否授权多个 Project？

**裁决：A — 可以，但每个 Project 必须显式获得授权。**

正式规则：

1. SSH Credential 的所有权归 Team。
2. 一个 Team 可以管理多个 SSH Credential。
3. 一个 SSH Credential 可以授权给多个 Project 使用。
4. 每个 Project 必须获得独立、明确的使用授权。
5. 未获得授权的 Project 不得使用该凭证。
6. 项目授权撤销后，不得继续为该项目签发新的凭证使用许可。

### D2：凭证绑定粒度？

**裁决：A — Repository 级绑定，每个仓库独立选择凭证。**

正式规则：

1. Repository 是 SSH Credential 的实际绑定对象。
2. 一个 Project 可以关联多个 Repository。
3. 每个 Repository 独立选择已获授权的 SSH Credential。
4. 同一个 Project 内不同 Repository 可以使用不同凭证。
5. 不默认将某个凭证应用于项目内全部 Repository。
6. Repository 绑定不得突破 Team 授权范围。

### D3：凭证授权和配置是否允许 AI 自动操作？

**裁决：A — 必须人工创建、授权和撤销；AI 只能提出建议。**

正式规则：

1. SSH Credential 的创建必须由具有相应权限的人工用户完成。
2. Project 使用授权必须由人工用户明确批准。
3. Repository 凭证绑定必须由具有项目管理权限的人工用户完成。
4. 凭证撤销必须由具有相应权限的人工用户执行。
5. AI Agent 可以分析仓库访问需求、提出凭证配置建议，但不能自行创建凭证、授予项目权限、修改授权范围或撤销凭证。
6. AI Agent 不得通过其他执行接口绕过上述人工控制边界。

## 3. 目标架构模型

采用：

**Team-owned Credential + Project-scoped Grant + Repository-level Binding + Execution-time Access**

职责划分：

| 层次 | 所属领域 | 职责 |
|---|---|---|
| SSH Credential | Team | 凭证元数据、生命周期和管理 |
| Credential Secret | Team 安全存储 | 私钥等敏感材料的安全保存 |
| Project Credential Grant | Project 授权关系 | 项目获得凭证使用许可 |
| Repository Credential Binding | Project / Repository | 仓库与凭证绑定 |
| Execution Credential Access | Execution | 执行时按授权使用凭证 |

逻辑关系：

```text
Team
 └── SSH Credential
       ├── Project A Grant
       │     ├── Repository A1 Binding
       │     └── Repository A2 Binding
       │
       └── Project B Grant
             └── Repository B1 Binding

Execution Run
 └── Execution-time Credential Access
       └── Authorized Repository Credential
```

## 4. 安全约束

以下为本裁决架构的安全设计约束，具体存储技术和执行注入机制由后续技术设计确定：

1. SSH 私钥不得明文存储于普通业务数据库。
2. SSH 私钥不得写入 Markdown、Git、日志、任务输出或 AI 对话。
3. 凭证不得因绑定到 Project 或 Repository 而被复制为多个不受控副本。
4. Device、Executor、Runner 是凭证使用者，不是凭证所有者。
5. 执行环境只能取得当前任务、项目及 Repository 已授权的凭证使用能力。
6. 凭证使用必须具备可审计记录。
7. 凭证撤销后必须阻止新的使用，并设计在途凭证失效与清理机制。
8. Repository 的 Read / Write 权限必须明确；不得因为绑定凭证就默认授予写权限。
9. 实际权限必须受到 Git Provider 侧授权能力的约束，不能假定所有 SSH Key 都支持相同的细粒度权限。

## 5. 人工权限边界

| 操作 | 控制要求 |
|---|---|
| 创建 SSH Credential | 人工执行 |
| 更新或撤销 Credential | 人工执行 |
| 授予 Project 使用权限 | 人工批准 |
| 撤销 Project 使用权限 | 人工执行 |
| Repository 绑定凭证 | 人工配置 |
| AI 提出凭证配置建议 | 允许 |
| AI 自动创建或授权凭证 | 禁止 |
| Executor / Runner 执行时使用 | 必须通过授权检查 |

具体 Team 角色权限和 Project 管理权限映射，需在权限设计中落实，不在本次裁决中擅自指定 Owner 以外的具体角色。

## 6. 前端导航归属裁决

### Team 设置

新增：

- Git 凭证（SSH Keys）

负责团队 SSH 凭证的创建、查看元数据、更新、撤销及 Project 使用授权管理。

### Project 设置

Repository 设置负责：

- Repository 凭证选择。
- Repository 与 SSH Credential 绑定。
- 凭证授权状态展示。
- 访问权限与绑定关系管理。

不新增独立的 Project SSH Keys 管理模块。

### Development Environment

保留：

- Devices
- Executors
- Runners

不再将 SSH Keys 作为 Development Environment 的管理页面。

Development Environment 只负责在执行时使用经过授权的凭证，不拥有凭证，也不负责凭证生命周期管理。

## 7. 本次裁决不包含的内容

以下事项不属于本次 R-1 已确认范围：

1. SSH Credential 的具体数据库 Schema。
2. 密钥加密算法、密钥管理服务和 Secret Storage 的技术选型。
3. GitHub / GitLab 等 Provider 的具体授权适配方案。
4. 临时凭证注入、SSH Agent 或临时文件的最终实现方式。
5. 在途任务撤销与密钥轮换的完整执行协议。
6. Team 内具体角色权限矩阵。

上述内容应在后续技术设计中细化，不得反向改变本次已确认的所有权、授权和绑定模型。

## 8. 最终裁决结论

R-1 正式采用：

- D1 = A：Team 凭证可以授权多个 Project，但必须显式授权。
- D2 = A：Repository 级绑定，每个仓库独立选择凭证。
- D3 = A：凭证创建、授权和撤销均由人工控制，AI 只能提出建议。

**最终架构定位：SSH Credential 属于 Team；Project 管理授权和 Repository 绑定；Development Environment 仅在执行时使用凭证。**

本裁决为后续前端导航、凭证管理、权限设计和工程执行凭证访问机制的正式依据。

---

# 附一：全局导航 IA 基线修订（V1.2 建议）

原导航 IA V1.1 的 Team 设置和 Project 设置作如下调整，不改变其他已裁决导航结构：

1. Team 设置

```text
Team Settings
- 团队常规
- 成员与邀请
- AI Provider
- Git 凭证（SSH Keys）   ← R-1 新增归属：团队凭证库、凭证生命周期、Project 使用授权
- 团队公告（适用时）
```

2. Project 设置

```text
Project Settings
- 基础设置
- 成员（规划项）
- 仓库                    ← Repository 列表、仓库配置、SSH Credential 绑定
- 开发环境
  - Devices
  - Executors
  - Runners
- AI 配置与执行策略
```

3. 最终导航树修订

```text
PM-AI
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
│       ├── Git 凭证（SSH Keys）  ← R-1
│       └── 团队公告
│
└── Current Project
    ├── 项目总控台
    └── 项目工作区
        └── 项目设置
            ├── 基础设置
            ├── 成员（规划项）
            ├── 仓库
            │   ├── Repository 列表
            │   └── Repository 凭证绑定  ← R-1
            ├── 开发环境
            │   ├── Devices
            │   ├── Executors
            │   └── Runners
            └── AI 配置与执行策略
```

# 附二：对 Phase 1 前端改造的影响

R-1 解除了一项导航设计阻塞，但不意味着 SSH Credential 管理功能本身需要在 Phase 1 实现。

| 事项 | 处理方式 | 阶段 |
|---|---|---|
| SSH Keys 从开发环境归属中移出 | 纳入导航配置 | Phase 1 |
| Team 设置新增 Git 凭证入口 | 纳入 Team Navigation | Phase 1 |
| Project 仓库页面增加凭证绑定入口 | 预留导航与页面设计位置 | 后续页面改造 |
| Team SSH 凭证管理 UI | 独立功能设计 | 后续 |
| Repository 凭证绑定 UI | 独立功能设计 | 后续 |
| Secret Storage / 加密 / 执行注入 | 技术设计与实施 | 后续 |

**特别注意**：当前核验报告中 `/project/settings/ssh-keys` 的实际页面、路由和 API 归属仍需根据源码确认。R-1 已经裁决业务归属，但不代表现有 SSH Keys 页面已经完成迁移。

# 附三：R-1 对 Phase 1 阻塞状态的影响

| 阻塞项 | 状态 |
|---|---|
| R-1：SSH Keys 归属 | **已解除 BLOCKED**（Team 凭证库；Project 仓库绑定；执行环境按需使用） |
| R-2：导航呈现形态 | **仍然 BLOCKED**（仍需定稿全局导航与项目内导航的挂载关系、工作区切换交互及 Grid 布局） |
| R-5：Graph 页签 | 可延后（Phase 4 前仍不修改 Graph 页签） |
