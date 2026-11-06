# S01 QA Acceptance Taskbook — QA Test Execution Prompt V1.0

> **登记日期**：2026-11-06
> **来源**：用户提供《S01 — Project Initialization & Code Discovery / QA Test Execution Prompt V1.0》
> **角色设定**：PM-AI Platform 高级 QA Engineer / Test Lead
> **测试对象**：S01（Project Initialization & Code Discovery）
> **性质**：独立、全面、严格的测试验收——验证 S01 是否具备可投入后续 S02、S03 开发的稳定基础
> **硬约束**：在完成全部阶段之前，不得宣称 S01 已通过验收

---

## 1. 测试原则

1. 必须以代码仓库中的正式需求文档、UX 设计稿、架构裁决记录及 API 契约为测试依据。
2. 不得因为某项功能在 UI 上存在，就认定功能已经实现。
3. 必须检查前端、后端、数据库、API、权限、状态流转及数据一致性。
4. 不得通过 Mock 数据、硬编码数据或伪造响应判定测试通过。
5. 对无法验证的功能必须标记 BLOCKED 或 NOT VERIFIED，不得标记 PASS。
6. 必须检查异常路径、重复操作、并发操作及跨项目数据隔离。
7. 不得擅自修改产品需求或架构裁决。
8. 发现需求文档与代码实现不一致时，记录为 GAP，不得自行选择其中一方作为正确实现。
9. 测试过程中不得直接修改生产代码来让测试通过。
10. 所有缺陷必须提供可重复的操作步骤、预期结果、实际结果及证据。

## 2. S01 测试范围

按照实际存在的正式 S01 文档和功能范围执行以下测试。

### S01-01：Project Baseline Confirmation

重点验证：

- 创建项目后是否能够进入项目初始化流程。
- 项目初始化状态是否由实际 InitItems 推导，而不是依赖一个虚假的整体状态。
- InitItem 是否正确反映项目初始化事项。
- 是否支持 not_started、in_progress、completed、blocked、not_applicable、undetermined 六种展示状态。
- 六种状态是否仅用于 Command Center Read Model，而不是错误地作为数据库核心实体状态。
- 项目基线信息是否可以独立确认和更新。
- 已完成事项是否能够正确保留。
- 未完成事项是否能够继续处理。
- 不适用事项是否不会错误阻塞项目初始化。
- 初始化状态是否能在数据变更后正确更新。
- 页面刷新、重新登录后状态是否保持一致。

特别验证：Project Baseline Confirmation 不是一个简单的项目整体状态，而是由多个初始化事项组成的确认区域。

### S01-02：Project Creation

重点验证：

- 项目创建流程。
- Project Key 自动生成规则。
- Owner 自动分配规则。
- 项目名称、描述及必要属性校验。
- 创建成功后立即进入 Project Initialization。
- 创建失败时是否正确回滚。
- 重复提交是否产生重复项目。
- 不同用户创建项目时的权限隔离。
- 项目列表是否正确展示新项目。
- 项目创建后 AI-PM 是否立即可用。

特别验证：项目创建后，不应该要求用户先配置 MCP、Codex 或其他外部 AI 工具，才能使用内置 AI-PM。

### S01-03：Repository Management

重点验证：

- 一个项目是否允许关联多个 Git Repository。
- 没有 Repository 的项目是否仍然可以正常工作。
- 初始没有 Primary Repository 是否可以接受。
- Repository 添加、修改、移除及重复添加处理。
- Git Provider OAuth / App 接入流程（如果该能力属于当前交付范围）。
- Repository 连接异常处理。
- Repository 权限校验。
- Repository 变更检测。
- 检测到变化后是否要求用户确认。
- 是否存在未经用户授权的自动操作。
- Repository 与 Project 的关联关系是否正确。
- 多个项目之间是否存在 Repository 数据串扰。

特别验证：Repository 是项目工程执行能力的扩展，不是启用 AI-PM 的前置条件。

### S01-04：AI-PM Cognition

重点验证：

- AI-PM 是否能够理解当前项目上下文。
- 是否能够读取项目基本信息。
- 是否能够理解 Project Goal。
- 是否能够进行需求分析。
- 是否能够提出需求建议。
- 是否能够整理项目知识。
- 是否能够识别项目中的未知信息。
- 是否能够区分已确认信息和 AI 推测信息。
- 是否能够正确引用项目知识。
- 是否能够在缺少 Repository 的情况下工作。
- 是否能够在缺少外部 AI 工具的情况下工作。
- AI 建议是否不会未经授权直接变成正式项目事实。
- 不同项目的 AI 上下文是否严格隔离。

### S01-05：Personal Workspace

按照正式裁决及 UX 设计验证：

- 用户登录后的个人工作入口。
- 项目快捷入口。
- Create New Project 快捷操作。
- 用户可访问项目的展示。
- 不同角色和权限下的项目展示。
- 项目状态与实际数据一致。
- 空数据状态。
- 加载状态。
- 异常状态。
- 多项目情况下的操作体验。

不得额外引入未经产品裁决的通用 Workspace 概念。

### S01-06 / S01-07

根据仓库中正式 S01-06、S01-07 需求文档、裁决记录和验收标准执行。

如果找不到正式需求或无法确认其范围：

1. 记录缺失的文档。
2. 标记 NOT VERIFIED。
3. 不得自行猜测功能要求。
4. 继续测试其他可以确认的范围。

## 3. 重点业务规则与架构约束

以下属于本次测试的强制检查项。

### A. AI-PM 独立可用

必须验证：

- 项目创建后，AI-PM 即可使用。
- 不接入 MCP 也能工作。
- 不接入 Codex 也能工作。
- 不接入 Claude Code 也能工作。
- 不配置外部 AI Agent 也不影响原生 AI-PM 的基本能力。

外部工具只扩展工程执行能力，不能成为项目管理功能的启动条件。

### B. Project Goal

验证：

- Project Goal 与 Goal 实体的关系。
- doc_key = project-goal 的使用规则。
- Goal 的创建、读取和更新。
- 不同项目 Goal 的隔离。
- AI 对 Goal 的读取及上下文使用。
- 未确认的 AI 建议不得冒充正式 Goal。

### C. Finding

验证：

- 人工直接提交的 Finding 可以创建为 CONFIRMED。
- Agent 转录产生的 Finding 必须先进入 PENDING。
- Pending Finding 的确认流程。
- 确认、拒绝及重复提交。
- 状态与审计记录一致。
- 不同来源不能错误地使用相同的确认规则。

### D. Project Archive

验证：

- Archive 后禁止新活动。
- 已经运行中的 Run 可以完成或取消。
- Archive 时必须撤销相关 Credential。
- 必须先写入终态，再进行进程清理。
- 不要求等待进程真正终止才写入终态。
- 不允许归档项目继续启动新的 Run。
- 不允许归档项目通过其他 API 绕过限制。
- 归档及取消操作必须留下审计记录。

### E. Repository 权限

按照已确认的产品裁决，不需要将 Git Read、Write、Commit、Push、Merge、Release 拆分成不同的产品权限层级。

但必须验证实际授权边界，不能出现未经授权的跨项目访问或操作。

## 4. 必须执行的跨模块场景

至少完成以下端到端场景：

| # | 场景 | 路径 |
|---|---|---|
| 01 | 新用户创建项目 | 登录 → Personal Workspace → Create New Project → 创建成功 → 进入项目初始化 → 查看 Project Dashboard |
| 02 | 无 Repository 的项目 | 创建项目 → 不添加 Repository → 使用 AI-PM → 分析项目目标 → 提出需求建议 → 检查知识上下文 |
| 03 | 项目初始化 | 创建项目 → 查看 InitItems → 确认部分事项 → 标记不适用事项 → 刷新 → 检查初始化状态 |
| 04 | AI-PM 项目认知 | 创建项目 → 提供 Project Goal → AI 分析项目 → 提出建议 → 人工检查 → 验证未经确认的建议没有成为正式事实 |
| 05 | Repository 接入 | 创建项目 → 添加多个 Repository → 检查关联 → 检测变化 → 请求用户确认 → 验证确认后的结果 |
| 06 | Finding 生命周期 | 人工创建 Finding → 验证直接 CONFIRMED；Agent 创建 Finding → 验证 PENDING → 人工确认 → 验证状态与审计记录 |
| 07 | 项目隔离 | 创建 Project A 和 Project B → 分别写入不同的 Goal、Finding 和知识 → 使用不同用户或权限访问 → 尝试跨项目读取和修改 → 验证隔离 |
| 08 | 项目归档 | 启动 Run → Archive Project → 检查 Credential 撤销 → 检查 Run 终态 → 检查进程清理 → 尝试重新创建活动 |
| 09 | 异常与重复操作 | 重复提交创建请求、重复确认、网络中断、API 超时、并发更新、无效 ID、无权限访问等 |

## 5. 测试技术要求

必须检查：

- Frontend UI
- Backend API
- Database Schema
- Database Constraints
- State Transitions
- Authentication
- Authorization
- Cross-Project Isolation
- Data Consistency
- Audit Trail
- Error Handling
- Concurrency
- Regression

优先使用真实测试数据库及真实 API。

所有测试数据必须使用独立测试项目和测试账号。

测试结束后清理测试数据，不能影响其他开发环境。

## 6. 缺陷分级

| 级别 | 定义 |
|---|---|
| P0 — Critical | 严重安全漏洞、跨项目数据泄漏、核心数据损坏、严重权限绕过 |
| P1 — High | 核心业务流程无法完成、关键状态错误、AI-PM 核心能力不可用 |
| P2 — Medium | 非核心业务流程异常、重要数据展示错误、异常恢复能力不足 |
| P3 — Low | 轻微 UI、文案、交互或不影响业务的体验问题 |

## 7. 测试报告要求

必须输出：

1. S01 测试范围与实际覆盖范围。
2. 测试环境及版本。
3. 测试用例总数。
4. PASS 数量。
5. FAIL 数量。
6. BLOCKED 数量。
7. NOT VERIFIED 数量。
8. P0/P1/P2/P3 缺陷清单。
9. 端到端场景测试结果。
10. API 与数据库检查结果。
11. 权限及跨项目隔离结果。
12. 自动化测试结果。
13. 需求与实现 GAP。
14. 遗留风险。
15. 最终验收结论。

每个失败用例必须包含：

- Test Case ID
- 对应需求
- 前置条件
- 操作步骤
- 预期结果
- 实际结果
- 缺陷等级
- 复现证据
- 相关 API / 数据库记录
- 建议修复方向

## 8. 最终验收标准

S01 只有在以下条件满足时，才可以建议通过：

- 所有 P0 缺陷关闭。
- 所有 P1 缺陷关闭，或经过明确的产品负责人风险豁免。
- 核心端到端场景全部通过。
- AI-PM 在没有 Repository 和外部 AI 工具时仍然可用。
- 项目数据隔离通过。
- Finding 状态规则正确。
- Project Archive 规则正确。
- InitItem 状态推导正确。
- 核心 API 和数据库状态一致。
- 没有未解释的需求与实现 GAP。

注意：测试报告必须客观反映真实结果。不得为了让 S01 显示完成而降低验收标准。

## 9. 执行阶段

| 阶段 | 内容 |
|---|---|
| 第一阶段 | 检查 S01 文档、代码和数据库结构，建立需求追踪矩阵 |
| 第二阶段 | 制定详细测试用例 |
| 第三阶段 | 执行测试 |
| 第四阶段 | 整理缺陷并进行回归 |
| 第五阶段 | 提交正式 S01 QA Acceptance Report |