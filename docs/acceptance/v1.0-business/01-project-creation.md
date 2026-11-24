# 01 — 阶段 1：项目创建与 AI-PM 初始化

- 日期：2026-11-24
- 操作者：leo（OWNER，团队 PM-AI）
- 证据目录：`evidence/01-project-creation/`

## 1. 实际执行步骤

| # | 步骤 | 结果 |
|---|---|---|
| 1 | 登录态进入 http://127.0.0.1:8092 | ✅ 会话有效（leo，OWNER） |
| 2 | /projects →「新建项目」 | ✅ 表单页：名称*、Key*（自动建议）、描述、负责人（默认创建人，标注「AI 不能指定负责人」） |
| 3 | 填写 TaskFlow / TASKFLOW / 描述 | ✅ Key 依据名称自动建议 |
| 4 | 提交「创建项目」 | ✅ 201，自动跳转项目总控台 |
| 5 | 检查项目首页 | ✅ 见下 |
| 6 | 与 AI-PM 对话 | ❌ **无回复**（见阻断发现） |

- 项目 ID：`74a545ea-379c-4c97-bd0e-0e5dc7c8bb02`
- 创建后状态：`规划中`；成员 1（leo）；仓库 0；项目目标「未建立」（显式信号「系统不会臆测」）
- 初始化清单（必需 2/3）：基本信息 ✅ / 项目目标 ⏳ 待开始 / 人工处理 ✅；仓库接入、代码分析（不适用）、项目认知、项目基线、开发环境（Runner）均**非必需** —— 与基线 4「外部工程工具不是前置条件」一致 ✅
- 表单侧栏 Guidance 明示：仓库 OAuth、AI-PM 认知、Codex/MCP「都不阻塞项目创建」 ✅

截图：`01-command-center-after-create.png`

## 2. AI-PM 对话（先阻断 → 配置后复验通过）

### 2.1 初始阻断

向 AI 助手发送任务书指定话术（「你现在是 TaskFlow 项目的 AI 项目负责人…」），消息 201 入库（会话 `a1266aca-4946-4f0d-85a1-7ccc1a47dfba`），前端持续轮询，**超过 60 秒无任何 AI 回复、无排队/失败提示**。

根因（只读排查）：
1. `ai-pm-worker` 日志：`conversation reply loop (AI_PM_CONVERSATION_REPLY=off|on, default off) mode=Off` —— 第七循环（对话框 AI 回复）**默认关闭**（F22：新计费型生产者默认关）。
2. 同批默认关：`AI_PM_DISCOVERY_ORCHESTRATOR=off`（第八循环）、`AI_PM_RUN_DISPATCH=off`（无人值守派发）。
3. `ai.team_llm_provider` 0 行、`ai.llm_call_record` 0 行 —— 部署从未配置任何 LLM Provider。
4. UI 侧：对话面板只显示用户消息，**无「AI 未配置/未启用」任何提示** —— 项目负责人无法自行发现原因。

### 2.2 解锁（用户授权后操作员动作，透明记录）

1. 用户提供 DeepSeek API key（sk-****853f，仅入本地部署，未进 git）。
2. full-dev `.env` 打开 `AI_PM_CONVERSATION_REPLY=on`、`AI_PM_DISCOVERY_ORCHESTRATOR=on`，重建 ai-pm-worker（部署配置面，非代码改动）。
3. 走产品 UI `/settings/team/ai-providers` 创建 Provider「DeepSeek-deepseek-chat」：类型自动预填地址、Key 密封保存只显末4位、「测试连接」连通（938ms）、「启用」生效 ✅。
4. **卡点**：Provider 启用后 worker 仍报 `general_policy_missing`（三级选模需要模型目录+授权+general 策略）。后端有完整目录 API（`/v1/pm/teams/{id}/llm-models` 等），**前端无任何页面调用**——UI 无法完成此步。经产品自身 REST API（登录态 Bearer）注册模型目录行 + team_wide 授权 + general 策略后，worker 立即回复成功（`replied=1`）。

### 2.3 AI-PM 实际表现（复验）

**第一轮**（07:25 回复）：正确理解 TaskFlow 目标（用户/项目/任务/权限四域 + 可扩展可维护）；识别当前 planning 阶段、无仓库无任务、关键决策未定；建议「先需求澄清与 PRD 初稿」并列出待答问题（角色、权限模型粒度、MVP 边界、约束、成功标准）；**未创建工程任务、未假设代码仓库**；以问题收尾等人类决策。✅ 全部符合基线。

**第二轮**（上下文记忆测试，07:26）：补充「5-50 人、纯 SaaS、多租户隔离、MVP 无外部协作者」后，AI 基于上一轮建议做增量更新（多租户隔离升级为第一决策点），列出 5 项需人类拍板事项并给推荐项，主动提出「按默认假设起草文档、你逐条批注」的协作模式。✅ 上下文持续记忆 + 建议与决策边界清晰。

截图：`02-ai-pm-first-reply.png`

## 3. 缺陷记录

### B01-01（P1，倾向 P0）
- 阶段：1
- 前置条件：full-dev 默认部署（AI_PM_CONVERSATION_REPLY 默认 off；无 LLM provider）
- 复现步骤：创建项目 → AI 面板发送任意消息
- 实际结果：消息入库后永久无回复；UI 无错误、无排队状态、无配置引导
- 预期结果：基线 3「项目创建后，内置 AI-PM 必须立即可用」；至少应给出「AI 未启用，如何开启」的显式提示
- 影响：产品核心价值（AI 项目负责人）开箱不可用，且用户无感知原因
- 严重程度：**P1**（关键业务流程无法闭环；归类=部署默认配置 + 未配置 LLM，叠加产品缺少引导提示的产品缺陷）
- 证据：`evidence/01-project-creation/01-command-center-after-create.png`、worker 日志、DB 只读查询

### B01-02（P3）
- 新建项目后 AI 面板 placeholder 停留「正在切换项目…」，需手动刷新页面才锚定 TaskFlow。

### B01-03（P3，浏览器适配备注)
- 「创建项目」按钮在自动化测试中 Playwright click 超时，需 DOM 直点——不影响真人操作，仅记录。

### B01-04（P1）
- 阶段：1
- 前置条件：任意部署；团队 Owner 已在 UI 配置并启用 LLM Provider
- 复现步骤：UI 创建 Provider → 测试连接 → 启用 → 向 AI-PM 发消息
- 实际结果：AI 仍不回复；worker 报 `general_policy_missing`。模型目录/授权/general 策略的后端 API（`llm-models` / `llm-model-authorizations` / `llm-model-policies`）**前端零调用**，无任何 UI 可完成此配置
- 预期结果：Provider 启用后 AI 即可用；至少 UI 提供模型目录/策略管理或自动落目录
- 影响：普通用户走 UI 永远无法激活 AI-PM；「内置 AI-PM 开箱可用」在 UI 层面不成立
- 严重程度：**P1**（归类=产品缺陷：UI 缺功能面）
- 证据：worker 日志、前端 grep（api/llm.ts 无 catalog 函数）、API 配置过程

### B01-05（P3）
- 在 `/settings/team/ai-providers` 点击「测试连接」后，SPA 无故跳转回项目总控台（复现 2 次），需手动返回查看测试结果。测试本身成功（200，连通正常 938ms）。

### B01-06（P2）
- AI 面板消息重复渲染：用户消息在对话区出现两次（一次截断预览 + 一次全文），视觉上像发送了两条。

## 4. 阶段结论

**部分通过（部署配置修正后通过）**：
- 项目创建、初始化清单、外部工具非必需设计 ✅
- AI-PM 理解力、阶段判断、建议质量、上下文记忆、不越权 ✅（DeepSeek-chat 驱动）
- 开箱即用性 ❌（B01-01 + B01-04 两条配置断链，P1×2）

