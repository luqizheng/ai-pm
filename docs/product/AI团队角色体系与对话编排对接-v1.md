# AI 团队角色体系与对话编排（DK-0）对接 · 终态登记 V1

| 属性 | 值 |
| --- | --- |
| 状态 | ⚠️ **已裁决未实现**（裁决定版 2026-11-08；代码零开工） |
| 登记范围 | AI Team V2 Role 体系定义 + DK-0 对话编排底座与 Role 体系的对接关系 |
| 证据基准 | V2 基线 @ 2026-11-06 裁决；DK-0 设计 V1.0 @ `feat/dk0-dialog-orchestration-design` 72cb7d5 |
| 权威源 | **`docs/project-ai-team-v2/ai-pm-v2-architecture-baseline-v1.md` §四**（Role 体系）；**`docs/product/ai-pm-dialog-orchestration-design-v1.md`**（DK-0，位于 `ai-pm-dk0` worktree）；**`ai-pm-v05-implementation-baseline-v1.md`**（实施序） |

> 维护纪律提醒：本篇两域均为「已裁决、未开工」。任一域进入实施后，须按 00-index.md §0 同步把「状态」行改为已实现并刷新证据基准。

---

## 1. AI Role 体系（AI Team V2）

### 1.1 定位与三条铁律

Role 是**项目专业工作的职责定义，不是运行中的 Agent**（V2 基线 §4 开篇）。三条已裁铁律：

1. **Role ≠ Agent 实例**：Role 不绑定固定 Agent Client，每个 Task 逐次动态分配（裁决 Q3=A）。同一 Role 的不同 Task 可由不同 Client 执行；同一 Client 可承担多个 Role。
2. **执行方式不是 Role 固定属性**：Native（平台内 AI-PM）或 External（外部 Agent Client）是任务层面的选择。例：Requirement Reviewer 可先由 Native 初审、复杂评审再委托外部 Client。
3. **Platform 是 Control Plane 不是多 Agent Runtime**：Role 之间不互相调用，一切协作经任务调度。

### 1.2 Role 预置目录（八个，项目可扩展）

| Role | 主要职责 | 典型执行方式 |
| --- | --- | --- |
| AI Project Manager | 项目理解、规划、拆解、协调 | Platform Native |
| Requirement Reviewer | 需求质量、完整性和冲突检查 | Native 或 External |
| Knowledge Manager | 知识整理、版本协调、知识质量 | Platform 主导 |
| Solution Architect | 技术方案与架构设计 | Native 或 External |
| UI/UX Designer | 信息架构、交互设计、界面设计 | 通常 External |
| Developer | 代码实现与工程修改 | 通常 External |
| QA Engineer | 测试设计、测试执行、质量检查 | Native 或 External |
| Risk Analyst | 风险识别、影响分析 | Native 或 External |

### 1.3 Role 定义结构：结构化记录，提示词只是其中一部分

**Role 不是"靠提示词定义"**。每个 Role 是一条结构化记录（V2 基线 §4.2），按消费方分四类：

| 类别 | 字段 | 谁消费 |
| --- | --- | --- |
| 身份 | Role ID / Name / Description | 人 + 调度器 |
| 职责 | Responsibilities / Skills / **Instructions** | **仅这三个字段是提示词性质**；随 Task 下发给外部 Client |
| 调度 | **Required Capabilities** / Status | 调度器做能力匹配（裁决 Q2=B：按 Role + Capability + 可用性自动选 Client） |
| 契约与护栏 | Input Contract / Output Contract / Acceptance Criteria / Risk Constraints | 平台机器校验交付物与权限，不依赖 LLM 自觉 |

一个只含提示词的 Role 视为「未声明能力」，调度器不会分派任务给它。Role 的 Instructions + Skills 随任务一起交给外部 Client 使用（基线 §4.2 末句）——对外部 Developer 类 Client，这份 Instructions 才是它实际收到的「提示词」。

### 1.4 与既有的区分：Team Role ≠ Project Role ≠ AI Role

- **Team Role**（owner/admin/member，Team≡tenant）：团队治理语义，见 Team 与角色体系登记（待登记章节）。
- **Project Role**（`project_role` 实体）：项目内成员角色，V2 硬前置缺口，尚未建表。
- **AI Role**（本篇 §1）：AI 执行者的职责定义，走 Role 目录 + Capability Registry。
- 定调（2026-11-07 专家第 3 块）：Team Admin 管团队 LLM ≠ 天然有项目业务管理权；Project Admin 不绕过 Team 模型授权。

### 1.5 代码现状（边界）

以下均**未实现**，为 V2 六缺口中的硬前置项：

- `project_role` 实体：不存在（迁移台账：V2 批次暂定 0049 起，落地前必重核台账）。
- Capability Registry：不存在。
- 自动调度策略层③可用性过滤 + ④能力匹配：不存在（当前 assignee 纯人工，七道资格门已有）。
- 现状代码佐证：`crates/pm-ai/src/skill_router.rs` 路由为硬编码 `match`（非 Role 驱动）；worker 主链为 FakeModel（`ai-pm-worker/main.rs`）；`skill_definition` / `project_skill_binding` 为死表（bootstrap 写 5 行零读取）。
- 已有可复用面：MCP 四工具（pm_claim_task / get_run_context / report_progress / submit_result）+ lease_token 是完整 Client 协议面；`delivery.task_run` 即 TaskExecution（禁重建）；project_ai_model_policy（迁移 0043）已存在。

---

## 2. DK-0 对话编排底座（对接的 Native 侧）

### 2.1 定位

DK-0 回答的问题是：**用户在一个对话框里跟 AI-PM 聊天，AI 怎么把对话安全地变成需求、任务和知识**。它不是自由 ChatBot——是给对话装编排规则。11 项裁决 D-DK-1~11 全部已裁（2026-11-08，按推荐全通过）。

设计稿：`docs/product/ai-pm-dialog-orchestration-design-v1.md`（756 行，`ai-pm-dk0` worktree / 分支 `feat/dk0-dialog-orchestration-design`，主 checkout 看不到属正常）。

### 2.2 六块核心裁决（结论级）

1. **对话载体（D-DK-1）**：`ai.conversation` 是唯一线程目录；用户只打一个端点 `POST conversations/{cid}/messages`。消息按段落落地：chat → `conversation_message`，访谈 → `discovery_message`。**单向升级** chat→interview 永不回落（升级只写 SYSTEM 摘要不复制旧轮次）。chat/interview 判定在**服务端**（显式指令优先；LLM 隐式升级由 `AI_PM_CONVERSATION_ROUTER` 控制，默认 off，静默升级禁止）。
2. **需求落点（D-DK-2/3）**：不建提案信封，draft revision 即提案形态。溯源两层：AI 生成草稿时标记 `requirement_revision.content.basis_finding_ids`（零迁移）；正式登记来源（`requirement_finding`/`source_input`）必须人类确认前一步物化——**AI 不得擅自登记来源**（S02D-20 红线）。
3. **访谈编排（D-DK-4/5）**：worker 第八条循环 + `crates/pm-ai/src/discovery_orchestrator.rs`（`AI_PM_DISCOVERY_ORCHESTRATOR` 默认 off）。原则「**系统定骨架，LLM 只回填**」：问题槽位由系统常量定（TASK_DISCOVERY 六槽 / INITIAL_ONBOARDING 五槽 / UPDATE 三槽），LLM 只生成问题文案与回填答案。「问完毕」=三条件与：无 open 且阻塞的问题 + 槽位全绿 + 本轮 LLM 出 0 新问题——全部可机器复算。状态跃迁全用既有状态机边，零迁移。
4. **知识注入（D-DK-6）**：不做检索式（V1 无检索面）；六节固定 system prompt（全只读）+ 36k 字符定值预算 + 低优先级整节裁剪（S5→S4→S2→S3，裁剪记 `llm_call_record.metadata.sections_dropped`）。访谈段不用 20 轮窗口（记忆靠 findings/questions 结构化存储），chat 段维持 CONTEXT_WINDOW=20。
5. **写回通道（D-DK-7）**：AI 写操作全经既有单点服务（V-6 架构约束）。白名单仅：create / draft / update / submit + create_task(draft)。**confirm / reject / transition / link_finding / register_source_input AI 全部无权**；任务 `draft→ready` 仅人类 → TaskReady outbox → Run。
6. **任务分解（D-DK-8）**：需求 approved 修订 + PC/KB 能力面 + 角色版本三源依据；前置关系写 `task_dependency`（不靠标题前缀）；**整批预览一次确认**（逐条确认会产生依赖倒挂）。

迁移：零迁移为目标；唯一候选 **0063** = `ALTER ai.conversation ADD COLUMN discovery_session_id`（+组合 FK，存量 NULL 免回填）。

### 2.3 特别验收（裁决附带）

- **A6**：访谈 COMPLETED 后同线程纯问答回落 chat（防实现写成只查列）。
- **C7**：REST + 编排器双入口 20 轮并发压测零 seq 冲突（`append_message` 序号 = MAX+1 无锁是全仓现状；per-session 锁须覆盖两入口；进程内锁不够可改 append_message 原子序列——单点服务内改动不违 V-6）。

---

## 3. 对接关系：DK-0 ↔ AI Team

### 3.1 一句话

**DK-0 对话编排 = Role 体系里 AI Project Manager（Platform Native）这个角色的执行体**；对话编排管「想清楚并立项」，AI Team 管「派活干活」，两者之间只有一道**人类确认的任务闸门**，不互通对话。

### 3.2 全链路

```
对话框（DK-0 编排，载体 = AI Project Manager / Native）
  │  访谈 → 需求草稿 → 人类 confirm → approved 修订
  │  （AI 全程只有提案权；confirm/reject 人类专属）
  ▼
任务分解（DK-0 ⑥，整批预览一次确认）
  │  三源：approved 需求 + PC/KB 能力面 + 角色版本
  │  AI 只产 draft 任务，前置关系写 task_dependency
  ▼
人类把任务 draft→ready（唯一触发点）
  ▼
TaskReady outbox → Run → V2 调度器
  │  按 Task 所需 role_id + Required Capabilities + Client 可用性
  │  选出 Agent Client；该 Role 的 Instructions/Skills 随任务下发
  ▼
外部 Client 经 MCP 四工具执行并交回
```

### 3.3 对接要点（五条）

1. **编排器本身是第一个 Role 的化身**：AI Project Manager 跑在服务端（第八条 worker 循环 + 既有 orchestrator），不是外部 Client。
2. **对接面 = 任务下发，不是对话互通**：对话框不和外部 Agent 聊天；产物经「需求确认→任务 ready」这道人类闸门后进调度器，外部 Client 才登场。这守住 V2 红线「Platform 是 Control Plane」。
3. **Role 定义到达外部 Client 的方式**：Instructions + Skills 作为任务上下文随 Run 下发。
4. **模型策略按角色走**：`project_ai_model_policy`（迁移 0043）已存在；Role 维度的模型选择是冻结前待裁项（UNIQUE(tenant,provider) 约束）。
5. **V0.5 口径**：对话产品形态 = 结构化交互 + Discussion Memory，非自由 Copilot。

### 3.4 实施序（已裁，均未开工）

- DK 侧：DK-A（底座接线 + 0063）→ DK-B（编排器）→ DK-C（注入）→ DK-D（写回）→ DK-E（场景 2 更正）→ DK-F（demo 真栈联调）；每批四件齐 + 失败用例清单。
- V2 侧硬前置：agent_client（=V-05-1，与 ACP 共同身份基座）/ project_role / Capability Registry / 调度入口设计闭合 / 执行事件持久化最小实现。
- 边界：DK 与 V2 解耦推进（DK 不等 V2 角色体系落地）；S03-01~06 实现不随 V0.5（V0.6 由 PC-I6 Design·ADR 影响分析拉起）。

---

## 4. 未决事项（登记，不在本篇裁决）

| 事项 | 状态 |
| --- | --- |
| 「需求澄清访谈」映射到哪个 discovery kind（加枚举 = ALTER CHECK = 第二迁移，破零迁移声明） | P1，裁决前须补 |
| `discovery_session.project_id NOT NULL` vs 建项目前进对话框的时序 | P1，裁决前须补（推荐：先经既有 REST 建项目再进访谈） |
| a+ 升级后两表扫描集互斥 + 20 轮窗口跨表 UNION 读模型 | P2，裁决前须补读模型节 |
| 36k 字符定值 vs Ollama 本地小窗（8k~32k token）爆窗 | P2，需 provider 感知预算或保护节清单 |
| Role 维度模型选择与 `project_ai_model_policy` UNIQUE(tenant,provider) 约束 | V2 Schema 冻结前必裁 |
| 调度入口六问（谁创建执行请求/选 Role/选模型/决定 Project/分发/失败重试终结） | V2 开工前设计闭合 |
