# S01 总体实施计划 V1.0

> 文档性质：总体实施组织基线  
> 所属项目：AI R&D Workspace / smark-pm  
> 版本：V1.0  
> 日期：2026-11-03  
> 状态：实施组织基线，待 S01-0 基线核验完成后正式冻结  
> 实施原则：人类负责项目，AI Agent 协助执行；多仓库、多 Agent 并行；知识库统一协调写入。

---

## 0. 文档目的

S01 的目标是建立完整的项目初始化与代码发现闭环，而不是单独完成 Repository Import。

最终需要实现：

1. 人类创建和管理 Project。
2. 一个 Project 可以关联多个 Repository。
3. 平台负责身份、租户、角色、权限和任务管理。
4. 多个 AI Agent 可以并行领取和执行 Task。
5. Codex CLI 在授权的执行环境中完成代码分析、开发、测试和提交。
6. MCP 负责 Agent 与平台之间的任务上下文、执行进度和证据交换。
7. Worker 负责调度、后台任务和执行生命周期管理。
8. Discovery 负责代码事实发现和证据追溯。
9. Knowledge Coordinator 负责统一协调知识写入。
10. 人类负责重要决策、稿件审核和最终发布。

### 0.1 总体架构原则

- Project 是项目管理边界。
- Repository 是独立管理的代码仓库实体。
- Task 是平台分配的工作单位。
- Run 是一次具体的 Agent 执行实例。
- Attempt 是 Import Source 的一次执行轮次，不与 Run 混为一谈。
- Evidence 是可以追溯来源的证据。
- Proposal 是待审核的知识或发现提案。
- Knowledge Coordinator 是项目知识写入的统一协调入口。

### 0.2 明确不做

本阶段不引入：

- 第二套任务调度系统。
- 通用事件总线或完整事件溯源系统。
- Agent 直接修改正式知识库的权限。
- 自动合并、自动发布等尚未批准的策略。
- 为了统一概念而推翻现有状态机。
- 没有代码事实依据的大规模数据库重构。

---

## 1. S01 总体实施阶段

| 阶段 | 名称 | 主要目标 | 阶段出口 |
|---|---|---|---|
| S01-0 | 统一实施基线 | 汇总 I1–I6、代码和 Migration | 实施任务体系 |
| S01-1 | 身份与权限基础 | 建立可靠管理边界 | 身份和授权可用 |
| S01-2 | Project / Repository | 支持一个项目多个仓库 | 多仓库管理可用 |
| S01-3 | Discovery / Import | 完成代码发现和证据追溯 | Discovery 闭环 |
| S01-4 | Agent 执行基础 | 打通 Bootstrap、MCP、Codex、Worker | Agent 执行闭环 |
| S01-5 | Knowledge 协调 | 统一知识写入和稿件发布 | 知识闭环 |
| S01-6 | 交付与审计 | 验收、恢复、审计 | S01 端到端通过 |

阶段划分是实施组织，不代表必须按阶段整体开发完毕才能进入下一阶段。满足接口契约和前置条件的任务可以并行。

> 勘误 2026-11-03（S01-0）：**I5（项目总控台）必须最后实施**，顺序为 `I1 → I2 → I3 → I4 → I6 → I5`（依据 `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-i5-project-command-center-v1-1.md` §11.2）。本表阶段顺序未表达此约束。

---

## 2. TASK 编号与管理规则

### 2.1 编号原则

保留已经存在的设计任务编号，不重新给已有任务改名。

例如：

- D1：Run 调度与领取。
- D2：MCP 任务上下文接口。
- D3：Codex 执行适配器。
- D4：多仓库交付与结果回写。
- D5：Review、失败恢复及知识回写。
- I6-B-01～I6-B-09：I6-B 数据模型和实施任务。（勘误 2026-11-03：原写至 08；I6-B 合并规格 §4/§6 与核验反馈 C-3 均引用 I6-B-09「空库初始化 + 0028 升级验证 + DB 级负向测试」，补入本表并同步 §6.4。）

这些编号不得被新的 S01 编号覆盖。

对于尚未找到原始编号的任务，先使用 `S01-Wxx` 作为实施工作包编号。S01-Wxx 不是正式代码 TASK ID，待 S01-0 核验后再关联到平台 Task。

### 2.2 TASK 必须包含的字段

| 字段 | 说明 |
|---|---|
| task_key | 原始 TASK 编号 |
| stage | 所属 S01 阶段（S01-0～S01-6）。勘误 2026-11-03：原字段说明为「所属 S01 阶段」，与工作包编号混用，现拆为两字段。 |
| work_package | 所属工作包（S01-Wxx） |
| title | 任务名称 |
| objective | 目标 |
| source_design | 来源设计稿及版本 |
| dependencies | 前置 TASK |
| migration_scope | 涉及的 Migration |
| rest_scope | REST 改动 |
| mcp_scope | MCP 改动 |
| worker_scope | Worker 改动 |
| code_scope | 代码模块 |
| acceptance | 验收标准 |
| parallelism | 串行、接口依赖、可并行 |
| freeze_gate | 是否存在设计或迁移冻结门禁 |
| status | 当前实施状态 |

### 2.3 TASK 状态管理

TASK 状态、Run 状态和 Delivery 状态必须独立管理。

不得将 Agent 执行成功等同于 TASK 已经完成，也不得将 Commit 成功等同于交付验收通过。

---

## 3. S01-0：统一实施基线

目标：在不重新进行架构讨论的前提下，将 I1–I6 转化为可执行的任务体系。

### 3.1 工作包

| 工作包 | 工作内容 | 交付物 |
|---|---|---|
| S01-W01 | 汇总 I1–I6 设计稿 | 设计索引 |
| S01-W02 | 设计稿与代码事实核对 | 代码事实矩阵 |
| S01-W03 | 数据库 Migration 盘点 | Migration Registry |
| S01-W04 | REST/MCP/Worker 接口梳理 | Interface Matrix |
| S01-W05 | 状态机和权限边界核对 | State & Permission Matrix |
| S01-W06 | TASK 依赖关系梳理 | TASK Registry |
| S01-W07 | 统一验收规范 | Acceptance Matrix |

### 3.2 必须完成的核验

1. I1–I6 是否有唯一有效设计稿。
2. 各设计稿之间是否存在状态机冲突。
3. 是否存在重复的数据模型或重复职责。
4. 已有 Migration 是否已经实现部分设计。
5. 哪些设计已经实施，哪些只有设计尚未实施。
6. 哪些缺陷必须在 S01 修复。
7. 哪些问题允许延后，不影响当前实施。
8. 哪些 TASK 依赖同一个数据库变更。

### 3.3 S01-0 验收

- 所有设计稿有明确版本。
- 已确认裁决有正式记录。
- 代码事实有文件路径、Migration 和必要的行号。
- Migration 变更有明确归属。
- 每个正式 TASK 都有验收条件。
- 不存在无法解释的关键接口依赖。

---

## 4. S01-1：身份、租户、Project 权限基础

目标：建立项目管理和 Agent 执行的统一授权边界。

### 4.1 工作范围

| 工作包 | 内容 | 交付 |
|---|---|---|
| S01-W10 | 身份上下文和调用人识别 | Principal 上下文 |
| S01-W11 | Tenant 成员管理 | 成员管理能力 |
| S01-W12 | 角色和权限管理 | 授权管理能力 |
| S01-W13 | Project 成员授权 | Project 权限 |
| S01-W14 | Agent 执行授权 | Agent Principal 校验 |
| S01-W15 | 权限撤销即时生效 | 撤权测试 |

正式 TASK 编号、Migration 和 API 路径由 S01-0 从 I1–I3 原始设计稿中映射，不另行创造。

### 4.2 核心约束

- Tenant 是成员和角色管理边界。
- Project 是项目资源访问边界。
- Agent 身份不能替代人类授权。
- Agent 必须具有明确的执行身份。
- 权限撤销必须能够及时影响后续执行。
- 历史 Run 保留执行时授权快照。
- 不能仅凭历史授权快照继续执行当前已撤销的操作。

Git 权限（**2026-11-04 用户简化裁决后**）：

`可访问仓库 = Task 关联仓库 ∩ 仓库状态 = verified`；档位单一 `read_write`（可读即可写，写操作不再按类型分级）。

（勘误 2026-11-04：此前经 2026-11-03 勘误为 `read / modify / commit / push / create_pr / merge`
（以 I3 §3.1 为准），现按**用户简化裁决**「不区分，都可以读写。因为这部分太复杂。」
废止六能力与四层授权交集模型——I3 走 V1.1 简化模型，
详见 `docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-i3-git-permission-and-repo-write-authorization-v1.md` §0.0。
保留的边界不变：租户/项目隔离、Task 仓库范围、仓库状态、Run 授权快照、归档与取消门禁。）

策略开启不等于权限授予。

### 4.3 验收

- 正向：合法成员可以访问授权 Project。
- 负向：跨 Tenant、跨 Project 访问被拒绝。
- 并发：权限变更与执行领取并发时结果明确。
- 幂等：重复成员授权不会产生重复关系。
- 恢复：权限服务异常时默认拒绝执行。
- 审计：成员授权、角色变更和撤权均可追溯。

---

## 5. S01-2：Project / Repository 管理

目标：建立真正的多仓库 Project 模型。

### 5.1 工作范围

| 工作包 | 内容 | 交付 |
|---|---|---|
| S01-W20 | Project 管理 | Project 生命周期 |
| S01-W21 | Repository 管理 | Repository CRUD |
| S01-W22 | Project-Repository 关联 | 多仓库关联 |
| S01-W23 | Repository 验证 | Git 连接验证 |
| S01-W24 | Repository 权限 | 仓库级授权 |
| S01-W25 | Git 基准管理 | Commit 基准 |

### 5.2 数据模型原则

Project 与 Repository 必须独立。

一个 Project 可以关联多个 Repository。

Repository 必须有独立的：

- 仓库身份。
- 访问配置。
- 验证状态。
- Git 基准。
- 生命周期。
- 历史引用关系。

不能将 Repository 简化为 Project 表中的一个 URL 字段。

### 5.3 验收

- 一个 Project 可以关联多个 Repository。
- 不同 Repository 可以具有不同授权。
- 不同 Repository 可以独立验证。
- 仓库失效不导致其他仓库失效。
- 仓库归档不会破坏历史执行记录。
- 删除被历史 Task、Run 或 Import 引用的 Repository 时，必须有明确的保护行为和业务错误。

---

## 6. S01-3：Discovery / Import

目标：将代码发现过程转化为可追溯、可恢复的业务流程。

### 6.1 工作范围

| 工作包 | 内容 | 交付 |
|---|---|---|
| S01-W30 | Discovery Session | Session 生命周期 |
| S01-W31 | Import 生命周期 | Import 状态管理 |
| S01-W32 | Source 与 Attempt | 仓库分析执行记录 |
| S01-W33 | Finding | 发现事实 |
| S01-W34 | Question / Answer | 人工补充信息 |
| S01-W35 | Evidence | 证据追溯 |
| S01-W36 | Proposal | 发现结果提案 |
| S01-W37 | Worker 恢复 | 失败与取消恢复 |

### 6.2 领域边界

必须区分：

- `discovery.discovery_session`
- `repo_import`
- `repo_import_source`
- Attempt
- Finding
- Question
- Evidence
- `discovery.discovery_proposal`
- `ai.document_proposal`

这些对象不能因为名称相似就合并状态机。

Discovery Session 负责发现过程；Import 负责仓库接入和分析；Finding 负责发现事实；Evidence 负责事实依据；Proposal 负责待处理结果。

### 6.3 已有代码事实

当前代码基线中：

- `discovery.discovery_session` 已存在，Migration 0025。
- `discovery.discovery_finding_repository` 已存在。
- `repo_import` 已有既定实现。
- Discovery MCP 已有 6 个工具。
- Repository 和 Import 已有 REST 接口。
- Knowledge Write Coordinator 已存在。

因此，I6 的实施是对现有实现进行修复、补强和模型收敛，而不是从零创建整套 Discovery 系统。

### 6.4 I6-B 专项工作

I6-B V1.1 必须先完成设计正文修订，再允许 Migration 冻结。

任务安排：

| 任务 | 工作内容 | 冻结要求 |
|---|---|---|
| I6-B-01 | Project 初始化操作、占有与释放、generation、active 唯一性、取消及终态恢复 | DDL 审查 |
| I6-B-02 | Source / Attempt 数据归属和重试语义 | DDL 审查 |
| I6-B-03 | Source 集合固化和 Commit 基准 | DDL 审查 |
| I6-B-04 | Finding / Evidence 模型及证据类型判别 | DDL 审查 |
| I6-B-05 | Discovery 权限、提案及接口收敛 | 接口审查 |
| I6-B-06 | 审计和幂等 | 测试审查 |
| I6-B-07 | 历史数据和兼容性 | 数据审查 |
| I6-B-08 | 复合外键、跨 Project 约束和 Migration 集成 | 最终 DDL 审查 |
| I6-B-09 | 空库初始化 + 0028 升级验证 + DB 级负向测试 1~5 | 数据审查（DDL 冻结后实测） |

具体任务归属以 I6-B V1.1 正文最终映射为准，不得因本计划而擅自改变原有任务定义。

### 6.5 I6-B 强制冻结条件

1. D-1、D-2 正式进入 V1.1 正文。
2. Attempt / Evidence 复合外键进入 DDL。
3. `repo_import_source` 补充所需复合唯一键。
4. 跨 Project 关联约束采用已经确认的数据库级方案。
5. 代码证据和非代码证据 CHECK 约束明确。
6. 历史数据兼容性检查通过。
7. 旧环境升级通过。
8. 新环境初始化通过。
9. 并发、幂等、恢复及负向约束测试通过。

D-1a：

- `draft_ready` 可以重试。
- 重试成功后重新生成草稿，`draft_revision + 1`。
- 进入 `proposed` 后禁止重试。

D-2a：

- 采用数据库级 Project 归属约束。
- Finding、Import、Evidence 之间的关联必须保持 Project 一致。

特别注意：复合 FK 不应仅仅在设计文档中写明，必须验证 PostgreSQL 中引用列具有相应的唯一约束，且历史数据满足新约束。

### 6.6 I6-B 实施策略

I6-B 不阻塞已经具备实施条件的身份、Project、Repository、执行引擎等模块。

但依赖 I6-B 新增字段或约束的代码任务，必须等待对应 Migration 冻结。

---

## 7. S01-4：Codex Bootstrap、执行引擎与 MCP

目标：建立 Agent 从平台领取 Task 到实际执行并回传结果的完整链路。

### 7.1 已有执行设计任务

保留原有 D1–D5 编号：

| TASK | 工作内容 | 交付物 |
|---|---|---|
| D1 | Run 调度与领取 | 原子领取、租约和调度 |
| D2 | MCP 任务上下文接口 | 上下文、权限和结果回传 |
| D3 | Codex 执行适配器 | Codex CLI 执行能力 |
| D4 | 多仓库交付和结果回写 | Delivery 和逐仓结果 |
| D5 | Review、恢复和知识回写 | 验收及知识闭环 |
| D3.1 | D3 可靠性与启动协议加固（载体编号 T-032） | 启动持久化、凭据下发、lease_generation、进程回收 |

### 7.2 执行架构

平台：

- 决定 Task 是否可以执行。
- 决定由谁执行。
- 决定仓库和权限范围。
- 决定什么条件算完成。

调度器：

- 原子领取 Run。
- 检查 Task 和 Repository 基准。
- 校验有效策略。
- 维护租约。
- 处理领取失败和重试。

MCP：

- 下发 Task 上下文。
- 提供授权范围内的知识和仓库信息。
- 接收进度。
- 接收证据和结果。
- 不负责任务调度。
- 不允许 Agent 直接修改 Task / Run 权威状态。

Codex：

- 实际读取代码。
- 修改代码。
- 执行测试。
- 生成 Commit。
- 汇报执行结果。

Worker：

- 调度 Run。
- 管理执行生命周期。
- 维护租约。
- 回收失效进程。
- 处理 Outbox 和后台作业。

### 7.3 必须遵守的执行不变量

1. 同一个 Task 同时最多一条活跃 Run。
2. Task 状态只能由平台领域服务校验并变更。
3. 执行端不能直写 Task 或 Run 状态。
4. 失败重试必须创建新的 Run。
5. 失败历史不可覆盖。
6. 每次执行记录多仓库基准。
7. 权限快照与当前授权检查分开。
8. 领取、启动、心跳、失败和完成都必须可审计。

### 7.4 Bootstrap 工作范围

Codex Bootstrap 必须能够：

- 检查运行环境。
- 检查 Codex CLI。
- 检查必要配置。
- 安装或配置 MCP。
- 建立平台身份认证。
- 获取 Project 和 Repository 上下文。
- 验证调用权限。
- 进行连接自检。
- 提供明确的安装结果。

Bootstrap 不得将用户的本地凭证写入知识库或普通 Task 日志。

### 7.5 验收

- 两个 Worker 并发领取同一个 Task，不会生成两条活跃执行。
- Worker 领取后崩溃，租约过期可以恢复。
- Codex 启动失败不会误记为正常执行。
- 取消可以传播到实际执行进程。
- 重试保留历史 Run。
- 多仓库执行结果可以分别记录。
- MCP 越权调用被拒绝。
- MCP 重复回传具备幂等保护。

---

## 8. S01-5：Knowledge Coordinator 与稿件发布

目标：实现多 Agent 并行生产、统一协调写入。

### 8.1 工作范围

| 工作包 | 内容 | 交付 |
|---|---|---|
| S01-W50 | 执行结果收集 | Result Collection |
| S01-W51 | Evidence 汇总 | Evidence Aggregation |
| S01-W52 | Knowledge Coordinator | 统一写入 |
| S01-W53 | Document Proposal | 稿件管理 |
| S01-W54 | 质量门禁 | Quality Gates |
| S01-W55 | 人工审核 | Review |
| S01-W56 | 发布和回滚 | Knowledge Lifecycle |

### 8.2 核心架构约束

多个 Agent 可以并行执行：

```text
Agent A ──┐
Agent B ──┼──> 执行结果 / Evidence
Agent C ──┘
                  │
                  ▼
          Knowledge Coordinator
                  │
                  ▼
          Proposal / Revision
                  │
                  ▼
             Quality Gates
                  │
                  ▼
             Human Review
                  │
                  ▼
             Knowledge Publish
```

统一协调器负责：

- Project 级并发控制。
- 文档基准版本校验。
- Evidence 关联。
- Proposal 汇总。
- 冲突检测。
- 质量门禁。
- 审核和发布协调。

### 8.3 已有机制复用

不重复建设已有能力：

- `ai.document_proposal`：8 态发布管线。
- `discovery.discovery_proposal`：独立的 Discovery 提案状态机。
- `base_revision_id`：知识版本基准。
- `check_base_revision`：乐观并发校验。
- Project 级 advisory lock：统一知识写入锁。
- `knowledge_write_coordinator.rs`：现有统一写入服务。

注意：知识稿件发布状态机与 Discovery 提案审核状态机是两套不同的业务生命周期。

### 8.4 验收

- 两个 Agent 同时提交不同知识变更。
- 同一文档出现版本冲突时能够识别。
- 重复提交不会重复发布。
- 未通过质量门禁不能发布。
- 未经人工确认的稿件不能绕过审核。
- 发布失败能够恢复。
- 回滚不删除历史记录。
- 每次发布都能追溯到 Agent、Run、Evidence 和审核人。

---

## 9. S01-6：Delivery、恢复与审计

目标：让整个执行过程能够被人类验收和管理。

### 9.1 工作范围

| 工作包 | 内容 | 交付 |
|---|---|---|
| S01-W60 | 项目级 Delivery 聚合 | Delivery Read Model |
| S01-W61 | 交付验收页面 | Acceptance UI |
| S01-W62 | 失败恢复 | Recovery |
| S01-W63 | 全链路审计 | Audit |
| S01-W64 | 端到端验收 | E2E Report |

### 9.2 交付验收必须支持

按 Project 展示：

- 待验收任务。
- 可验收任务。
- 存在阻塞的任务。
- 逐仓库交付情况。
- 预期 Commit。
- 实际 Commit。
- 测试结果。
- 交付证据。
- Review 状态。

### 9.3 审计要求

能够追溯：

- 谁创建了 Project。
- 谁关联 Repository。
- 谁创建了 Task。
- 哪个 Agent 领取了 Task。
- 哪个 Worker 执行了 Run。
- 执行使用了哪个 Commit。
- 执行修改了哪些文件。
- 哪些测试通过或失败。
- 哪些 Evidence 被提交。
- 谁审核了交付。
- 谁批准知识发布。

---

## 10. Migration Registry

### 10.1 管理原则

Migration 不以 TASK 编号为唯一依据，也不允许 Codex 随意修改已冻结的历史 Migration。

每项数据库变更必须记录：

| 字段 | 说明 |
|---|---|
| migration_id | Migration 编号 |
| source_design | 来源设计稿 |
| source_task | 来源 TASK |
| dependencies | 前置 Migration |
| upgrade | 旧环境升级 |
| fresh_install | 新环境初始化 |
| constraints | 数据库约束 |
| data_backfill | 历史数据处理 |
| rollback | 回滚或前向修复 |
| test | 迁移测试 |
| freeze_status | 冻结状态 |

### 10.2 当前已知 Migration 关联

以下是已经核实的关键关联，不代表完整 Migration 清单：

| Migration | 已知关联 | 实施要求 |
|---|---|---|
| 0009 | Repo Import 早期模型 | 核对现有约束及后续扩展 |
| 0011 | Task Run 执行模型 | 保留既有状态和并发约束 |
| 0014 | Document Proposal | 保留知识稿件生命周期 |
| 0016 | Policy Composition | 复用现有策略合成 |
| 0019 | Repository Import Source 相关结构 | 核对历史数据及删除行为 |
| 0022 | 多仓库执行关联 | 核对 Task / Run 仓库关系 |
| 0025 | Discovery Session、Finding 关联等 | 复用现有 Discovery 模型 |

实际实施前必须对 0001–0028 完整盘点，并从当前仓库重新核验 Migration 文件、执行顺序和数据库状态。

> 盘点已于 2026-11-03 完成（S01-0），结论见 `docs/s01-全局主菜单与信息架构设计基线/docs/s01-migration-registry-v1.md`。后续变更以该 Registry 为准。

### 10.3 三类 Migration 状态

- `draft`：设计尚未完成。
- `reviewed`：DDL、兼容性和测试已经审查。
- `frozen`：允许进入正式实施。

已经冻结的 Migration 不应直接修改；需要修正时优先新增增量 Migration。

### 10.4 新旧环境双路径

每项 Migration 必须验证：

**旧环境升级**

```text
现有数据库
    ↓
执行新增 Migration
    ↓
历史数据检查
    ↓
约束验证
    ↓
应用启动
    ↓
回归测试
```

**新环境初始化**

```text
空数据库
    ↓
按顺序执行全部 Migration
    ↓
初始化必要数据
    ↓
启动应用
    ↓
完整测试
```

两条路径必须得到一致的 Schema 和必要的数据约束。

---

## 11. REST / MCP / Worker 职责矩阵

| 领域 | REST | MCP | Worker |
|---|---|---|---|
| Tenant | 管理成员和角色 | 不负责管理 | 不负责管理 |
| Project | 项目管理 | 获取授权上下文 | 后台任务 |
| Repository | 仓库管理 | 获取授权仓库上下文 | 验证、分析 |
| Task | 创建和管理任务 | 读取任务、执行回报 | 调度 |
| Run | 查询和管理执行记录 | 执行进度回传 | 生命周期管理 |
| Discovery | 创建和管理发现流程 | 发现结果提交 | 分析执行 |
| Evidence | 查询和管理 | 证据回传 | 处理与归档 |
| Knowledge | 管理和审核 | 不直接发布 | 协调后台流程 |
| Delivery | 验收和查询 | 提交交付证据 | 聚合和处理 |

以上是职责边界。实际端点、MCP 工具名称和 Worker handler 必须以当前代码为准，不能因为矩阵中存在一项就假定相应接口已经实现。

---

## 12. 并行实施规则

### 12.1 可以并行

满足接口和数据模型稳定的前提下：

- Project 管理与部分 Repository 管理。
- Bootstrap 与执行环境检测。
- MCP 契约与 Codex CLI 适配器。
- 前端页面与已经稳定的 REST Read Model。
- 不依赖 I6-B 新约束的其他模块测试。

### 12.2 必须串行

- 身份上下文先于依赖它的授权接口。
- Project/Repository 数据模型先于多仓库执行。
- Task/Run 状态机先于调度器。
- MCP 契约先于执行端正式接入。
- Source/Attempt 模型先于 Evidence 复合外键。
- Knowledge Proposal 规则先于发布集成。
- Migration 审查先于依赖该 Migration 的代码合并。

### 12.3 冲突控制

同一时间：

- 一个数据库 Migration 变更应只有一个主责 TASK。
- 一个领域状态机应只有一个权威写入服务。
- 多 Agent 不得各自修改同一个知识发布基线。
- 多 Agent 修改同一个文件或模块时，必须提前建立文件级协调。
- TASK 之间通过明确的接口契约交付，不通过口头约定依赖。

---

## 13. 统一 TASK 验收规范

所有正式 TASK 必须包含以下测试：

| 类型 | 验收要求 |
|---|---|
| 正向 | 正常业务流程成功 |
| 负向 | 越权、错误状态、非法参数被拒绝 |
| 并发 | 同一资源多请求竞争 |
| 幂等 | 重复提交不会重复产生业务结果 |
| 恢复 | 进程退出、Worker 重启、租约过期 |
| 数据 | 外键、唯一约束、CHECK、状态一致性 |
| 审计 | 操作主体和操作结果可追溯 |

### 13.1 TASK 完成定义

TASK 只有满足以下条件才可以进入 Completed：

1. 实施符合已批准设计。
2. 代码已经提交。
3. 测试已经执行并通过。
4. Migration 已经过适用的审查。
5. 代码审查通过。
6. Delivery 证据完整。
7. 没有未处理的 P0/P1 问题。
8. 平台验收通过。

Agent 的自我报告不能替代平台验收。

---

## 14. Codex TASK 执行协议

统一执行过程：

```text
TASK Ready
    ↓
Agent Claim
    ↓
平台校验身份、权限、仓库和基准
    ↓
创建 Run
    ↓
MCP 获取上下文
    ↓
Codex Bootstrap / CLI
    ↓
代码实施和测试
    ↓
Commit
    ↓
MCP 回传执行结果
    ↓
平台生成 Delivery
    ↓
人类或授权验收流程
    ↓
TASK Completed
    ↓
Knowledge Coordinator
    ↓
Proposal / Review / Publish
```

### 14.1 重新执行

重新执行必须：

- 保留原 Run。
- 记录修改意见。
- 重新检查仓库基准。
- 创建新的 Run。
- 记录新的执行身份和权限快照。
- 重新产生 Delivery。

不允许覆盖原执行历史。

### 14.2 Agent 职责边界

Agent 可以：

- 领取已授权任务。
- 读取任务上下文。
- 修改授权范围内的代码。
- 运行测试。
- 创建 Commit。
- 提交证据。
- 报告执行结果。

Agent 不可以：

- 自行变更权限。
- 自行改变 Task 权威状态。
- 绕过 MCP 修改平台状态。
- 绕过 Knowledge Coordinator 发布知识。
- 擅自修改已冻结的架构决策。

---

## 15. S01 总体验收标准

S01 完成必须同时满足：

### A. 设计基线

- I1–I6 之间无未解决的关键接口冲突。
- 权限边界统一。
- 状态机边界统一。
- 数据模型关系明确。
- 未决事项有明确责任和门禁。

### B. 数据库

- Migration 顺序完整。
- 旧环境升级通过。
- 新环境初始化通过。
- 关键 FK、UNIQUE、CHECK 均有测试。
- I6-B 复合外键约束已正式落地。

### C. 执行闭环

- REST 管理边界明确。
- MCP 上下文和回传完整。
- Worker 可恢复。
- Codex CLI 可以实际执行。
- 多仓库基准和交付可追溯。

### D. 知识闭环

- 多 Agent 可以并行生产。
- Knowledge Coordinator 统一协调。
- 冲突可识别。
- 审核和发布可追溯。
- 失败可恢复。

### E. 交付闭环

- 项目级交付可查询。
- 逐仓库验收。
- 失败 Run 可恢复。
- 审计链完整。
- 端到端测试通过。

---

## 16. 本版本的冻结与未决事项

### 16.1 本版本确认的原则

- 人类负责项目，Agent 协助执行。
- Project 可以关联多个 Repository。
- 多 Agent 并行执行。
- 知识库统一协调写入。
- Task 与 Run 分离。
- 重试创建新的 Run。
- REST、MCP、Worker、Codex 职责分离。
- I6-B 不阻塞其他已经具备实施条件的模块。
- I6-B Migration 必须经过 V1.1 正文和 DDL 审查后才能冻结。

### 16.2 本版本不冻结的内容

- I1–I5 的具体 TASK 编号与代码文件映射。
- 尚未完成核验的 Migration 归属。
- I6-B V1.1 最终 DDL。
- 未经代码核实的 REST 路径和 MCP 工具映射。
- 尚未完成接口审查的 Worker handler。
- 前端尚未确认的具体页面交互。

### 16.3 S01-0 的退出条件

S01-0 完成后必须产生以下五份正式文件：

1. `docs/s01-全局主菜单与信息架构设计基线/docs/s01-implementation-plan-v1.md`
2. `docs/s01-全局主菜单与信息架构设计基线/docs/s01-task-registry-v1.md`
3. `docs/s01-全局主菜单与信息架构设计基线/docs/s01-migration-registry-v1.md`
4. `docs/s01-全局主菜单与信息架构设计基线/docs/s01-interface-matrix-v1.md`
5. `docs/s01-全局主菜单与信息架构设计基线/docs/s01-acceptance-matrix-v1.md`

并形成一份待决事项清单。

---

## 17. 启动决议

本计划建议正式进入 S01-0。

实施顺序：

1. 完成 I1–I6 设计稿索引和版本核验。
2. 完成现有 Migration 0001–0028 盘点。
3. 将原有 TASK 编号映射至 S01 阶段。
4. 核验 REST / MCP / Worker 真实调用链。
5. 生成正式 TASK Registry。
6. 冻结不依赖 I6-B 的实施任务。
7. 推进 S01-1 和具备条件的 S01-2 工作。
8. 并行推进 I6-B V1.1 修订与 DDL 审查。
9. 按依赖逐步推进 Discovery、Codex 和 Knowledge 闭环。

**最终原则：先建立统一实施基线，再按 TASK 实施；先打通底层基础，再打通端到端闭环。**

S01 的成功标准不是所有模块分别完成，而是人类能够通过平台管理 Project，Agent 能够安全执行跨仓库任务，平台能够验收交付，并由统一协调器将经过确认的结果写入项目知识体系。
