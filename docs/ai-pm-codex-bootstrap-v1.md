# Codex 自动初始化与开发环境接入（Codex Bootstrap）设计稿 V1

> **标注图例**：〔用户供稿〕= 2026-11-01 用户提供的设计供稿整理；〔原型〕= 用户提供的界面原型图转写（原图为微信临时文件，已失效，本文转写为文字线框）；〔整理注〕= 落盘时的格式性说明，不含设计裁决。

| 项 | 内容 |
| --- | --- |
| 文档性质 | 设计供稿整理（ADR + 安装协议 + 界面原型），**未过稿** |
| 版本 | V1（用户供稿，2026-11-01） |
| 记录 | 司南（产品经理 Agent）按用户供稿整理落盘 |
| 状态 | **草稿待评审**——实施前须按 todo.md §0.1 完成设计定稿（§10.7 T-087），经用户过稿后方可进入实施 |
| 上位约束 | `ai-pm-mcp-executor-positioning-v1.md`、`ai-pm-executor-onboarding-runbook-v1.md`、`ai-pm-adr-0001-tenant-agent-executor-identity.md`、`ai-pm-principal-identity-v1.md`、`ai-pm-multi-repo-model-v1.md`、`ai-pm-slice-d-execution-pipeline-v1.md` |
| 关联条目 | todo.md §10.7 M10（T-087~T-092）；M8 T-080（onboarding URL，本稿的直系前身） |

---

## 〇、一句话结论

把「Codex 接入」正式纳入 PM-AI 的 **Codex 接入与自动初始化（Codex Bootstrap）**：用户复制一条初始化指令交给 Codex，Codex 读取安装说明后**自动**完成 Worker 安装、MCP 配置、身份认证与项目关联，最后执行自检，确认可以正常领取 Task、回写执行状态——而不是让用户手动复制 MCP JSON、下载 Worker、修改配置文件。

> **核心原则**〔用户供稿〕：**安装说明负责指导安装，安装程序负责保证安装正确，PM-AI 服务端负责验证安装结果。** 不要让 AI 仅凭一段 Markdown 自己猜测应该如何修改配置文件。

---

## 一、建议的用户体验

在 PM-AI 项目管理页面增加一个入口（图1 原型）：用户只需要把这段文字交给 Codex，Codex 接下来完成安装。

**图1 原型转写〔原型〕**——「Codex 环境接入」卡片：

```
┌──────────────────────────────────────────────────────┐
│  Codex 环境接入                       [支持自动初始化] │
│  将以下指令发送给 Codex，即可初始化当前开发环境。       │
│                                                      │
│  PM-AI Bootstrap                                     │
│  请阅读以下安装说明，并为当前 Codex 环境初始化          │
│  PM-AI Worker 和 MCP：                               │
│                                                      │
│  https://pm-ai.example.com/install/codex             │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │              ⧉  复制初始化指令                  │  │
│  └────────────────────────────────────────────────┘  │
│  示例地址，正式环境请替换为 PM-AI 的实际部署地址。      │
└──────────────────────────────────────────────────────┘
```

---

## 二、先明确：PM-AI 需要安装的是什么？

把三个东西分开，避免架构混淆〔用户供稿〕：

| 层 | 组件 | 职责 |
| --- | --- | --- |
| **平台端** | PM-AI Worker | 负责 Run 调度、租约、超时回收、执行实例管理、结果接收。部署在 PM-AI 服务端，**不应该每个 Codex 都安装一套** |
| **开发机端** | PM-AI Runner / Local Worker | 负责连接 PM-AI、准备 Workspace、管理本地仓库、启动 Codex 执行实例、收集 Commit 和执行证据 |
| **Codex 端** | PM-AI Skill + MCP | Skill 提供工作规范和执行流程；MCP 提供 Task、Knowledge、Run 等平台接口 |

**关键架构判断**〔用户供稿〕：**MCP 不等于 Worker。**

- MCP 是 Codex 与 PM-AI 通信的接口；Worker 是负责执行任务的运行时。
- 用户在 Codex 中手动开发：Codex 通过 MCP 领取 Task。
- PM-AI 无人值守执行：平台 Runner 在开发机上启动 Codex，Codex 再通过 MCP 与平台交互。
- 两者使用**相同的 Skill、MCP 协议和权限模型**。

〔整理注〕这与既有架构一致：Task 是业务任务，Run 是执行实例，Workspace 隔离执行环境，Project 与 Repository 解耦，一个 Project 可以有多个 Repository。

---

## 三、推荐的安装技术方案

**推荐**〔用户供稿〕：**Codex Plugin + Bootstrap Skill + 安装程序 + Remote MCP**。

为什么不是单纯提供一个 MCP URL？因为 MCP URL 只能解决通信配置，不能可靠地完成本地 Worker 安装、Workspace 配置和环境检测。

> OpenAI 当前已经支持把 Skill 和 MCP 配置一起打包成 Plugin，也支持配置安装后的 onboarding Skill，可以把初始化流程纳入插件安装体验。

建议 PM-AI 发布以下安装包〔用户供稿〕：

```text
pm-ai-codex/
├── .codex-plugin/
│   └── plugin.json
│
├── .mcp.json
│
├── skills/
│   ├── pm-ai-bootstrap/
│   │   ├── SKILL.md
│   │   └── scripts/
│   │       ├── install-worker.ps1
│   │       ├── install-worker.sh
│   │       └── verify-install.ps1
│   │
│   ├── pm-task-execution/
│   │   └── SKILL.md
│   │
│   ├── pm-learn/
│   │   └── SKILL.md
│   │
│   └── pm-knowledge/
│       └── SKILL.md
│
└── manifest.json
```

| 组件 | 职责 |
| --- | --- |
| Plugin | 分发 Skill 和 MCP 配置 |
| Bootstrap Skill | 指导 Codex 执行初始化 |
| Worker Installer | 确定性安装 Runner |
| MCP | 连接 PM-AI 服务 |
| Verify | 检查安装完整性 |

MCP 配置示意：

```json
{
  "mcpServers": {
    "pm-ai": {
      "type": "http",
      "url": "https://pm-ai.example.com/mcp"
    }
  }
}
```

> Codex 支持 Streamable HTTP MCP 和 Bearer Token、OAuth 等认证方式。MCP 配置可以在用户级 Codex 配置中共享，也可以在受信任项目中使用项目级配置。

用户倾向〔用户供稿〕：使用 **Remote MCP**，而不是让每台机器运行一个本地 MCP Server。

---

## 四、自动初始化的完整流程（Bootstrap Lifecycle）

〔用户供稿〕八步：

1. **获取安装包**——Codex 通过 Plugin 或安装 URL 获取 PM-AI Bootstrap Skill。
2. **识别开发环境**——检查操作系统、Codex 版本、Rust/Node 等依赖（按实际 Worker 实现需要检测），检查是否已经安装 Worker。
3. **安装 Local Worker**——下载对应平台的二进制文件，校验 SHA-256 和版本，安装到用户目录。
4. **连接 MCP**——配置 PM-AI Remote MCP，执行 OAuth 授权，获取用户身份。
5. **注册开发环境**——向 PM-AI 注册 Device / Runner，建立租户与开发环境的关联。
6. **绑定项目与仓库**——用户选择 Project、Repository，检查 Git 状态和访问权限。支持一个 Project 绑定多个 Repository。
7. **运行自检**——测试 MCP、Worker、Runner 心跳、Workspace 创建、仓库访问和任务读取权限。
8. **完成初始化**——PM-AI 将该开发环境标记为 Ready，Codex 可以开始领取和执行 Task。

> **注意**〔用户供稿〕：下载程序、修改系统配置、安装服务、启动后台进程等操作，仍然需要遵循 Codex 的权限和审批机制。不能假设一段提示词就能绕过用户授权。

---

## 五、身份认证与安全设计

**不要让用户在安装指令里直接放一个长期有效的 API Token**，使用一次性初始化凭证〔用户供稿〕：

```text
Codex
  │
  │ 1. 请求初始化
  ▼
PM-AI Bootstrap API
  │
  │ 2. 返回一次性授权码
  ▼
Codex 打开授权页面
  │
  │ 3. 用户登录并确认授权
  ▼
PM-AI
  │
  │ 4. 注册 Device / Runner
  ▼
颁发短期访问凭证及刷新凭证
  │
  ▼
Worker + MCP 正式连接
```

建议的服务端对象〔用户供稿〕：

| 对象 | 作用 |
| --- | --- |
| device | 用户的一台开发设备 |
| runner | 设备上的一个执行运行时 |
| runner_registration | 设备注册与授权记录 |
| runner_heartbeat | 在线状态与最后心跳 |
| runner_capability | 支持的操作系统、执行能力 |
| runner_project_binding | Runner 被允许访问的项目 |

权限仍然遵循已有的租户、项目、Repository 和 principal 模型。

**特别注意**〔用户供稿〕：

- Device 不等于 Tenant Member。
- 一个用户可以有多台 Device。
- 一个 Device 可以服务多个 Project。
- 一个 Project 可以有多个 Repository。
- 撤销设备授权后，必须**立即阻止新的任务领取**。
- 已运行的 Run 应进入受控终止或失效处理，不能继续凭旧凭证回写结果。

---

## 六、PM-AI 页面还应该提供什么

建议增加一个独立的「开发环境」管理页面（图2 原型）。这意味着 PM-AI 不再只是管理 Task，还能知道**哪些真实开发环境可以执行任务**。

**图2 原型转写〔原型〕**——「开发环境」管理页：

```
┌──────────────────────────────────────────────────────┐
│  开发环境                                  [2 台在线] │
│  ┌────────────────────────────────────────────────┐  │
│  │ Windows Development PC                [Online] │  │
│  │ Runner v1.2.0 · Codex · Windows 11             │  │
│  │ 绑定项目：World Map Editor、PM-AI               │  │
│  │ 最后心跳：刚刚                                  │  │
│  │ [MCP 正常] [Worker 正常]                        │  │
│  │                        查看详情 · 撤销授权      │  │
│  ├────────────────────────────────────────────────┤  │
│  │ MacBook Pro                           [Online] │  │
│  │ Runner v1.2.0 · Codex · macOS                  │  │
│  │ 绑定项目：PM-AI                                 │  │
│  │ [MCP 正常] [Worker 正常]                        │  │
│  └────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────┐  │
│  │              +  添加开发环境                    │  │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

---

## 七、验收标准（Slice F — Codex Environment Bootstrap）

〔用户供稿〕作为一个独立 Slice，验收要求：

| 编号 | 验收要求 |
| --- | --- |
| BOOT-01 | 提供一个可分发的 Codex Plugin 或安装入口 |
| BOOT-02 | 仅通过初始化指令和安装 URL 即可启动安装流程 |
| BOOT-03 | 自动识别操作系统及 Worker 版本 |
| BOOT-04 | 支持 Windows、macOS、Linux |
| BOOT-05 | 自动下载、校验和安装 Worker |
| BOOT-06 | 自动配置 PM-AI MCP |
| BOOT-07 | 支持 OAuth 设备授权 |
| BOOT-08 | 自动注册 Device / Runner |
| BOOT-09 | 支持 Project 与多个 Repository 绑定 |
| BOOT-10 | 自动完成环境自检 |
| BOOT-11 | 支持升级、卸载、重新初始化 |
| BOOT-12 | 支持服务端撤销设备授权 |
| BOOT-13 | 安装失败可以恢复，不破坏已有 Codex 配置 |
| BOOT-14 | 安装和升级均有版本记录及审计日志 |

---

## 八、实施阶段建议

〔用户供稿〕分两个阶段实施：

| 阶段 | 范围 |
| --- | --- |
| **V1：Codex 手动启动 + 自动初始化** | 用户复制一段指令，Codex 执行 Bootstrap Skill，安装 Worker、配置 MCP、授权并注册设备 |
| **V2：无人值守 Runner** | 用户安装一次 Worker，后续 PM-AI 可以自主调度 Run，Runner 自动创建 Workspace、启动 Codex、执行 Task、收集 Commit 和 Delivery Manifest、回传证据 |

两阶段**共用同一套 MCP 协议和身份体系**，避免未来推翻重做。

另一个值得考虑的设计〔用户供稿〕：将 PM-AI 的**安装包版本与服务端 API 版本分离**，支持 Worker 自动升级和 MCP 能力协商。

---

## 九、ADR 概述：Codex 自动初始化与开发环境接入

### 目标

PM-AI 必须支持通过简单的安装指令和安装地址，让 Codex 自动完成开发环境初始化，无需用户手动配置 MCP、下载 Worker 或编辑配置文件。

### 核心决策

1. 使用 Codex Plugin + Bootstrap Skill 作为安装分发入口。
2. 使用 Remote MCP 作为 Codex 与 PM-AI 的标准通信接口。
3. 使用独立 Local Runner / Worker 负责本地执行环境管理和 Codex 实例调度。
4. 平台端 Worker 负责任务调度、租约、Run 生命周期管理，两者不得混为一谈。
5. 使用 OAuth 或一次性授权流程完成设备注册，**不在安装指令中传递长期 Token**。
6. 通过 Device、Runner、Project Binding 管理开发环境及项目授权。
7. Skill、MCP、Runner 采用独立版本管理，支持升级、兼容性检查和安装自检。

### 设计约束

- 安装过程必须幂等。
- 不得覆盖用户已有 Codex 配置。
- 不得未经授权安装后台服务或修改系统安全设置。
- 设备授权撤销必须立即阻止新的任务领取。
- Worker 不拥有绕过平台 Policy Engine 的权限。
- 所有任务状态变更仍由 PM-AI 后端统一处理。
- 一个 Project 支持多个 Repository，一个 Device 支持多个 Project。

### 实施阶段

- **V1**：Codex Bootstrap、MCP 配置、Runner 安装、设备授权、自检。
- **V2**：Runner 常驻运行、无人值守 Run 调度、自动升级和故障恢复。
