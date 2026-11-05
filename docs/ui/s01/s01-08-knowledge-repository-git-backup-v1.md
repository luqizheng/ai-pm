# S01-08 · Project Knowledge Repository & Git Backup
## 正式产品裁决记录及详细设计稿 V1.0

> 文档编号：S01-08  
> 文档名称：Project Knowledge Repository & Git Backup  
> 文档版本：V1.0  
> 文档日期：2026-11-05  
> 文档性质：正式产品裁决 + UX / 业务详细设计  
> 所属阶段：S01 · Project Foundation  
> 裁决状态：已完成用户裁决  
> 实施状态：待技术核验与实施

---

# Part A · 正式产品裁决记录

## A.1 功能背景

AI-PM 在 Project 生命周期中会持续形成大量知识文件，包括：

- 原始需求及需求分析
- 产品需求规格
- 产品设计基线
- UX / UI 设计文档
- 系统架构和技术设计
- 技术决策记录（ADR）
- 代码分析报告
- 项目研究报告
- 人工确认的项目决策
- AI-PM 整理的项目知识

这些知识以文件形式保存，是 Project 长期积累的重要资产。

如果知识文件仅依赖应用服务器本地存储，一旦发生服务器故障、数据损坏或迁移事故，可能造成项目知识资产丢失。

因此，Project 必须支持关联独立的远程 Git 仓库，将知识文件持续同步到用户自己的 Git 服务器，实现异地保存、版本追溯及恢复。

## A.2 用户正式裁决

| 裁决编号 | 问题 | 用户裁决 | 最终规则 |
|---|---|---|---|
| R-S01-08-01 | 知识库与代码仓库的关系 | A | 完全独立，分别配置 Git 地址 |
| R-S01-08-02 | 同步方式 | A | 自动同步为主，支持手动同步 |
| R-S01-08-03 | Git 服务支持范围 | B | 支持 GitHub、GitLab、Gitea、自建 Git 服务 |
| R-S01-08-04 | Project 创建时是否必须配置 | A | 非必需，创建后随时配置 |

### R-S01-08-01：知识库仓库与代码仓库完全独立

正式裁决：

1. Project Code Repository 与 Project Knowledge Repository 是两种不同用途的资源。
2. 两者分别配置 Git 地址、分支及认证信息。
3. 知识库 Git 不参与代码仓库的 Primary Repository 选择。
4. 不允许将知识库仓库自动识别为代码仓库。
5. 两种仓库可以属于同一个 Git Provider，也可以使用不同 Provider。

### R-S01-08-02：自动同步为主，支持手动同步

正式裁决：

1. 知识文件发生变化后，系统自动安排 Git 同步。
2. 支持人工触发 Sync Now。
3. 同步失败不得阻断正常业务。
4. 系统必须保留待同步变更，并提供重试能力。
5. 不得因为同步失败而丢弃知识文件。
6. 不允许未经人工确认覆盖远程冲突内容。

### R-S01-08-03：支持多种 Git 服务

正式裁决：

支持：

- GitHub
- GitLab
- Gitea
- Self-hosted Git Service

设计要求：

- Provider 不应与业务领域模型强耦合。
- 自建服务应尽可能通过标准 Git 协议接入。
- 不得将系统设计限制为某一家 Git 服务商。
- 不同 Provider 的认证方式允许存在差异。

### R-S01-08-04：Project 创建不强制配置知识库 Git

正式裁决：

1. Project 创建不要求提供知识库 Git 地址。
2. 没有远程知识库仓库时，Project 仍然可以正常工作。
3. Project 创建后，可以随时在 Settings 中配置。
4. 尚未配置远程仓库时，知识文件仍然必须保存在系统自己的持久化存储中。
5. 配置远程仓库属于增强能力，不是启动 AI-PM 的前置条件。

## A.3 裁决优先级

以上四项为已确认的产品裁决。

后续技术设计、UX 设计和代码实施不得改变这些业务规则。

如果技术实现存在限制，应提出替代方案并重新提交用户裁决，不得自行降低产品要求。

---

# Part B · 产品定位与领域边界

## B.1 功能定位

Project Knowledge Repository 是 Project 的独立知识资产持久化及版本管理能力。

它解决三个问题：

1. 知识文件可靠保存。
2. 知识文件持续同步到用户自己的 Git 服务器。
3. 知识文件能够追溯、恢复和迁移。

它不是代码仓库管理的子功能，也不是 AI-PM 的外部工具执行能力。

## B.2 与现有 S01 模块的关系

| 模块 | 职责 | 与本模块的关系 |
|---|---|---|
| S01-02 Project Creation | 创建 Project | 不要求配置 Knowledge Repository |
| S01-03 Repository Management | 管理代码仓库 | 与知识仓库完全独立 |
| S01-04 AI-PM Cognition | AI-PM 认知和知识处理 | 产生或使用知识资产 |
| S01-05 Project Baseline | 项目基线管理 | 基线文档可形成知识文件，但正式基线的确认机制不变 |
| S01-06 Codex / MCP Bootstrap | 外部工程工具接入 | 不依赖 Knowledge Repository 启用 |
| S01-07 Initialization Handover | 初始化交接 | 不因知识仓库未配置而阻断交接 |
| S01-08 Knowledge Repository | 知识文件持久化与 Git 备份 | 本模块 |

## B.3 核心领域边界

必须区分以下三个层次：

**第一层：Knowledge Domain**

负责知识内容的形成、管理、检索、组织和人工确认。

**第二层：Knowledge File Storage**

负责知识文件在系统内部的持久化保存。

**第三层：Knowledge Git Backup**

负责将内部知识文件同步到用户指定的远程 Git 仓库。

三者不能混为一谈。

特别是：

- Git Push 成功不等于知识内容已经通过人工确认。
- Git Commit 不等于产品决策。
- Git 历史不能替代产品业务审计记录。
- Git 同步状态不能代替知识文件自身的业务状态。

## B.4 数据权威性

正式采用以下原则：

| 数据 | 权威来源 |
|---|---|
| 当前知识文件内容 | 系统内部持久化存储 |
| 知识文件业务状态 | Knowledge Domain |
| 正式产品裁决 | 产品业务记录 |
| Git Commit 历史 | 远程 Git 仓库 |
| Git 同步状态 | Knowledge Sync Service |
| 文件操作审计 | 系统审计记录 |

远程 Git 是知识文件的版本化备份和恢复来源，不是日常业务运行的唯一数据源。

---

# Part C · 功能详细设计

## C.1 Project Knowledge Repository 配置

配置入口：

Project → Settings → Knowledge Repository

每个 Project 最多关联一个默认 Knowledge Repository。

本版本不引入多个知识仓库的业务管理复杂度。不同类型的知识通过目录和分类组织。

### C.1.1 配置字段

| 字段 | 必需 | 说明 |
|---|---|---|
| Provider | 是 | GitHub / GitLab / Gitea / Self-hosted |
| Repository URL | 是 | 远程 Git 地址 |
| Default Branch | 是 | 默认同步分支 |
| Authentication | 是 | 授权方式 |
| Sync Mode | 是 | Automatic / Manual |
| Connection Status | 系统维护 | 连接状态 |
| Last Successful Sync | 系统维护 | 最近成功同步时间 |
| Last Commit | 系统维护 | 最近成功提交 |
| Pending Changes | 系统维护 | 待同步文件变化数量 |
| Last Sync Error | 系统维护 | 最近同步错误 |

### C.1.2 Provider 认证

优先使用 Provider 官方 OAuth / App 授权能力。

其他情况支持适合该 Provider 的标准认证方式，例如：

- Personal Access Token
- Deploy Key
- SSH Key

安全要求：

1. 认证凭据必须加密保存。
2. 凭据不能以明文返回前端。
3. 日志不得记录 Token、私钥或完整敏感认证信息。
4. 认证失效时应提示重新授权。
5. 用户撤销授权后，应保留内部知识文件。

### C.1.3 配置流程

1. 用户进入 Knowledge Repository 设置。
2. 选择 Git Provider。
3. 提供 Repository URL。
4. 完成身份认证。
5. 选择目标 Branch。
6. 系统执行连接验证。
7. 检查远程仓库及目标分支情况。
8. 用户确认初始化同步策略。
9. 建立关联。
10. 启动自动同步。

如果远程仓库目标分支已有文件，禁止直接覆盖。

必须先识别远程内容并进入安全的初始化处理流程。

空仓库可以进入首次初始化 Push。

非空仓库应提供检查、差异预览及人工确认流程，不允许默认 Force Push。

## C.2 Knowledge File Storage

系统必须先有自己的知识文件持久化能力，再提供 Git 备份。

### C.2.1 文件类型

主要支持：

- Markdown（.md）
- JSON（.json）
- YAML（.yaml / .yml）
- 文本文件（.txt）
- 图片（.png / .jpg / .svg）
- PDF
- 其他经系统允许的附件格式

推荐使用 Markdown 作为知识正文格式。

### C.2.2 建议目录结构

```text
project-knowledge/
│
├── README.md
│
├── requirements/
│   ├── original-requirements/
│   ├── requirement-specifications/
│   └── requirement-changes/
│
├── product/
│   ├── product-baseline/
│   ├── ux-design/
│   └── feature-specifications/
│
├── architecture/
│   ├── system-architecture/
│   ├── domain-model/
│   └── technical-decisions/
│
├── decisions/
│   ├── product-decisions/
│   ├── technical-decisions/
│   └── decision-history/
│
├── analysis/
│   ├── code-analysis/
│   └── research-reports/
│
└── attachments/
    ├── images/
    └── documents/
```

上述目录为建议的默认组织结构，不得将目录路径写死为不可扩展的业务规则。

系统应支持后续增加知识分类。

### C.2.3 文件元数据

知识文件至少需要具备以下逻辑属性：

- file_id
- project_id
- file_path
- file_name
- content_type
- content_hash
- size
- created_at
- updated_at
- created_by
- updated_by
- deleted_at（适用时）

具体是否全部使用独立数据库列，须结合现有 Knowledge Domain 和文件存储实现进行代码核验。

文件内容与元数据可以采用不同的持久化方式。

### C.2.4 文件修改规则

任何知识文件变更：

1. 必须先成功持久化到系统内部。
2. 更新文件元数据和内容 Hash。
3. 产生可追踪的文件变更事件。
4. 通知 Git Sync Service。
5. 进入同步队列。

Git 不可用不能影响第一步。

---

# Part D · Git 同步机制

## D.1 整体架构

```text
AI-PM / Human
      |
      v
Knowledge Domain
      |
      v
Knowledge File Storage
      |
      | File Changed Event
      v
Knowledge Sync Queue
      |
      v
Knowledge Git Sync Service
      |
      +---- Repository Validation
      |
      +---- Change Detection
      |
      +---- Commit Generation
      |
      +---- Git Push
      |
      v
Remote Git Repository
```

Git Sync Service 应与 Knowledge Domain 解耦。

知识文件创建、修改和删除不应直接调用 Git Push。

## D.2 自动同步

当文件发生变化时：

1. 系统保存文件。
2. 产生变更事件。
3. 将变更加入同步队列。
4. 根据批处理策略合并短时间内的变更。
5. 计算文件差异

---

> **[落库注记 · 2026-11-05]** 本稿按用户提供原文完整落库。原文在 Part D.2「自动同步」第 5 步「计算文件差异」处截断，
> D.2 剩余步骤及后续章节（D.3 及以后、Part E 等，如有）尚未到达，待用户补发后补充落库。
> 评审稿见 `s01-08-knowledge-repository-git-backup-review-v1.md`（评审范围以本稿已到内容为限）。