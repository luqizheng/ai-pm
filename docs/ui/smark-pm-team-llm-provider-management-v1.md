# Team LLM Provider Management

## 正式产品裁决记录与详细设计稿 V1.0

| 属性 | 内容 |
|---|---|
| 产品模块 | Team Settings / AI Configuration |
| 文档版本 | V1.0 |
| 文档状态 | Approved |
| 裁决状态 | 四项全部确认 |
| 产品归属 | Team |
| 适用范围 | PM-AI Platform |
| 补充意见 | 无 |

---

# Part I：正式产品裁决记录

## 1. 裁决背景

PM-AI 是多 Team、多 Project 的 AI R&D / Project Management 平台。

LLM Provider 为 AI-PM 提供基础模型服务能力。为了支持团队独立管理模型资源，同时保证项目之间的资源共享与权限隔离，需要建立 Team-level LLM Provider Management。

经过产品讨论，以下四项进入正式产品裁决。

## 2. 正式裁决

### DEC-LLM-001：Provider 凭证归属

**裁决：A — Team 独立配置和拥有凭证。**

规则：

1. 每个 Team 独立管理自己的 LLM Provider。
2. API Key、Endpoint、认证信息归属于 Team。
3. 不同 Team 之间禁止共享凭证。
4. System 不代替 Team 管理客户的 Provider 凭证。
5. 同一个 Team 可以配置多个相同类型的 Provider。
6. Provider 凭证只能通过后端安全机制使用，不允许前端读取完整密钥。

裁决结果：Provider 是 Team-owned Resource。

### DEC-LLM-002：Project 模型选择权限

**裁决：A — Team Admin 配置，Project Admin 只能在授权范围内选择。**

规则：

1. Team Admin 负责管理 Provider 和 Model。
2. Team Admin 决定哪些模型可以被 Project 使用。
3. Project Admin 只能从团队授权模型中选择。
4. Project 不得创建自己的 Provider 凭证。
5. Project 不得使用未授权或已禁用的模型。
6. Team 撤销模型授权后，相关 Project 必须收到配置失效提示。

裁决结果：Team 控制资源边界，Project 在边界内自主配置。

### DEC-LLM-003：默认模型策略

**裁决：A — 按 AI Role 分配模型。**

规则：

1. Team 支持按 AI Role 配置默认模型。
2. 每个 Role 可以指定独立模型。
3. 未单独配置的 Role 继承 General 模型。
4. Project 可以在团队授权范围内覆盖模型策略。
5. AI-PM Lead 默认使用团队指定的高能力模型。
6. 模型策略必须能够追踪最终解析结果。

初始 AI Role：

- General
- AI-PM Lead
- Requirement Analysis
- Task Analysis
- Risk Analysis
- Architecture Analysis
- Knowledge Management

裁决结果：模型策略以 AI 职责为中心，而不是简单地以项目为中心。

### DEC-LLM-004：Provider 故障处理

**裁决：A — 明确失败并提示管理员，不自动切换。**

规则：

1. Provider 调用失败时，返回明确错误。
2. 不自动切换其他 Provider。
3. 不自动降低模型能力。
4. 不静默使用其他模型。
5. 向 Team Admin 提供故障信息。
6. 用户可以在故障恢复后重新执行任务。
7. 已发生的失败必须保留执行记录。

如果未来增加备用模型机制，必须经过独立产品裁决，不能由技术实现自行引入。

裁决结果：模型调用的可预测性优先于自动降级的可用性。

---

## 3. 最终产品原则

| 原则 | 最终规则 |
|---|---|
| Resource Ownership | Team |
| Provider Credentials | Team 独立拥有 |
| Model Authorization | Team Admin |
| Project Model Selection | Project Admin，在授权范围内 |
| Default Policy | 按 AI Role |
| Failure Handling | Fail Fast，明确告警 |
| Automatic Fallback | 禁止 |
| Cross-Team Sharing | 默认禁止 |

以上四项为 V1.0 强制产品约束。

---

# Part II：修订后的详细设计稿

## 4. 产品架构

### 4.1 三层架构

System：

- Provider Type Registry
- 平台安全策略
- 全局能力定义

Team：

- Team LLM Provider
- Team Credentials
- Team Model Catalog
- Model Authorization
- Team Default Model Policies

Project：

- Project AI Configuration
- Project Model Policies
- Agent Model Assignment

### 4.2 资源关系

```text
System
  └── Provider Type Registry

Team
  ├── LLM Providers
  │    ├── Provider A
  │    │    ├── Credential
  │    │    └── Models
  │    └── Provider B
  │         ├── Credential
  │         └── Models
  │
  ├── Model Authorization
  │
  ├── Team Default Model Policies
  │
  └── Projects
       ├── Project A
       │    └── Project AI Configuration
       └── Project B
            └── Project AI Configuration
```

一个 Team 可以拥有多个 Provider，一个 Provider 可以包含多个 Model。

多个 Project 可以使用同一团队授权的 Model。

## 5. 功能模块

V1.0 包含：

| 模块 | 功能 | 优先级 |
|---|---|---|
| LLM Providers | Provider 生命周期管理 | P0 |
| Credentials | 凭证安全管理 | P0 |
| Model Catalog | 模型发现与登记 | P0 |
| Model Authorization | 团队模型授权 | P0 |
| Connection Testing | 连接测试 | P0 |
| Default Model Policies | 按 AI Role 配置模型 | P0 |
| Project AI Configuration | 项目模型覆盖 | P0 |
| Usage & Budget | 调用量和成本 | P1 |

## 6. Provider Management

### 6.1 初始 Provider 类型

- OpenAI
- Anthropic
- Google Gemini
- DeepSeek
- OpenAI-Compatible

Provider 类型采用可扩展 Adapter 机制。

### 6.2 Provider 实例

每个 Provider 实例包含：

- ID
- Team ID
- Name
- Provider Type
- Endpoint
- Credential Reference
- Configuration
- Status
- Created By
- Created At
- Updated At

同一个 Team 可以创建多个相同 Provider Type 的实例。

### 6.3 Provider 生命周期

```text
Not Configured
       ↓
Configured
       ↓
Connection Testing
       ↓
Connected / Failed
       ↓
Enabled
       ↓
Disabled
       ↓
Archived
```

测试成功不等于自动启用，启用必须经过用户明确操作。

## 7. Credential Management

Credential 独立于 Provider 配置保存。

安全要求：

1. API Key 加密存储。
2. 使用 KMS 或 Vault 管理加密主密钥。
3. 不向前端返回完整 API Key。
4. 修改凭证采用替换机制。
5. 支持密钥轮换。
6. 凭证访问必须通过后端受控服务。
7. 日志不得记录明文凭证。

即使 Team Owner 也不能重新读取已保存的完整密钥。

## 8. Model Catalog

### 8.1 Model 属性

- Model ID
- Display Name
- Provider ID
- Capabilities
- Context Window
- Pricing
- Availability
- Enabled Status
- Last Verified At

### 8.2 Model 能力

- Text Generation
- Reasoning
- Structured Output
- Tool Calling
- Vision
- Long Context

能力信息必须来源于实际 Provider 能力或经过验证的配置。

### 8.3 模型发现

支持：

- API 自动发现。
- 手动登记。

模型自动发现不代表自动授权。新增模型必须经过团队启用和授权。

## 9. Model Authorization

这是本次裁决新增的明确设计要求。

Team Admin 必须能够控制哪些 Model 可以被 Project 使用。

建议建立独立的 Model Authorization 管理区域。

授权关系：

```text
Team Model
    ↓
Team Authorization
    ↓
Authorized Projects
    ↓
Project Model Selection
```

建议支持：

- Team-wide availability：团队范围可用。
- Restricted availability：只允许指定项目使用。
- Revoked：撤销授权。

V1.0 默认允许已授权模型被团队内项目使用，同时保留未来按项目限制的扩展能力。

Project Admin 不能修改授权范围。

## 10. Team Default Model Policy

### 10.1 AI Role 配置

| AI Role | Default Model |
|---|---|
| General | Team selected model |
| AI-PM Lead | Team selected model |
| Requirement Analysis | Inherit General |
| Task Analysis | Inherit General |
| Risk Analysis | Inherit General |
| Architecture Analysis | Inherit General |
| Knowledge Management | Inherit General |

模型必须属于团队已启用、已授权的模型。

### 10.2 Project Model Override

Project Admin 可以：

- 选择团队授权模型。
- 为 AI-PM Lead 指定模型。
- 为其他 Agent 指定模型。
- 恢复继承 Team Default 的策略。

Project Admin 不可以：

- 创建 Provider。
- 修改 Provider Endpoint。
- 修改 API Key。
- 使用未授权模型。
- 修改 Team Default Policy。

### 10.3 模型解析顺序

```text
Project Role Override
        ↓
Team Role Policy
        ↓
Team General Policy
        ↓
Configuration Error
```

如果指定模型不可用或已撤销授权，不允许静默降级。

必须产生明确的模型配置错误。

## 11. Provider 故障处理

故障必须区分：

- Authentication Error
- Connection Error
- Timeout
- Rate Limit
- Provider Service Error
- Model Unavailable
- Configuration Error

处理规则：

| 错误 | 系统行为 |
|---|---|
| Authentication Error | 终止调用，提示检查凭证 |
| Connection Error | 终止调用，提示检查连接 |
| Timeout | 终止本次调用，保留执行记录 |
| Rate Limit | 明确返回限流状态，不切换模型 |
| Provider Error | 返回服务错误 |
| Model Unavailable | 返回模型不可用 |
| Configuration Error | 提示检查 Team / Project 配置 |

允许对明确可重试的临时网络错误进行有限次数的同一 Provider 请求重试，但不能通过切换 Provider 或 Model 实现所谓自动恢复。

对于存在不确定执行结果的请求，必须考虑重复计费和重复执行风险。

## 12. 权限矩阵

| 操作 | Team Owner | Team Admin | Project Admin | Member |
|---|---|---|---|---|
| 查看 Provider | ✓ | ✓ | — | — |
| 管理 Provider | ✓ | ✓ | — | — |
| 替换 Credential | ✓ | ✓ | — | — |
| 测试 Provider | ✓ | ✓ | — | — |
| 管理 Model Catalog | ✓ | ✓ | — | — |
| 授权 Model | ✓ | ✓ | — | — |
| 修改 Team Policy | ✓ | ✓ | — | — |
| 选择 Project Model | ✓ | 授权后 | ✓ | — |
| 使用模型 | ✓ | ✓ | ✓ | ✓ |

Project Admin 的项目模型配置权限仅在其项目权限范围内生效。

## 13. UI/UX 信息架构

Team Settings：

```text
Team Settings
 ├── Team Members
 ├── Roles & Permissions
 ├── AI Configuration
 │    ├── LLM Providers
 │    ├── Model Catalog
 │    └── Default Model Policies
 └── AI Usage & Budget
```

LLM Providers 页面：

- Provider 列表
- Add Provider
- Provider 状态
- Model 数量
- Last Tested
- Edit / Test / Disable / Archive

Model Catalog 页面：

- Model Name
- Model ID
- Provider
- Capabilities
- Context Window
- Status
- Project Authorization

Default Model Policies 页面：

- AI Role
- Selected Model
- Provider
- Inheritance
- Last Updated

Project AI Configuration：

- 显示 Team Default。
- 显示当前实际使用模型。
- 提供已授权模型选择器。
- 支持恢复继承。
- 不显示 API Key 和 Provider 凭证。

## 14. 数据库设计

建议逻辑实体：

| 实体 | 职责 |
|---|---|
| team_llm_provider | Team Provider 实例 |
| team_llm_credential | 加密凭证 |
| team_llm_model | 团队模型目录 |
| team_llm_model_authorization | 模型授权 |
| team_llm_model_policy | Team AI Role 策略 |
| project_ai_model_policy | Project 模型覆盖 |
| llm_call_record | 模型调用记录 |

数据约束：

1. 所有团队资源必须具有 team_id。
2. Provider、Credential、Model 之间必须验证 Team ID 一致。
3. Project Policy 只能引用同一 Team 已授权的 Model。
4. 删除 Provider 前必须检查模型和项目引用。
5. 配置修改采用乐观锁或版本控制。
6. 敏感凭证不得保存在普通 JSONB 配置中。

现有 project_ai_config 必须优先复用，不能在未核对现有结构前重复创建项目 AI 配置体系。

## 15. 后端架构

采用统一 LLM Gateway：

```text
AI-PM Agents
      ↓
LLM Gateway
      ↓
Model Resolver
      ↓
Team Policy
      ↓
Project Policy
      ↓
Authorization Check
      ↓
Credential Resolver
      ↓
Provider Adapter
      ↓
LLM API
```

所有 AI-PM Agent 通过统一 Gateway 调用模型。

Agent 不直接管理 Provider Client，也不直接读取 API Key。

Gateway 负责：

- 模型策略解析。
- Team / Project 权限验证。
- Model Authorization 验证。
- Credential Resolution。
- Provider Adapter 调用。
- 错误分类。
- Token 和费用记录。
- 调用审计。

## 16. API 设计

| Method | Endpoint | 说明 |
|---|---|---|
| GET | `/teams/{team_id}/llm-providers` | Provider 列表 |
| POST | `/teams/{team_id}/llm-providers` | 新增 Provider |
| PATCH | `/teams/{team_id}/llm-providers/{id}` | 修改 Provider |
| DELETE | `/teams/{team_id}/llm-providers/{id}` | 删除 Provider |
| POST | `/teams/{team_id}/llm-providers/{id}/test` | 测试连接 |
| POST | `/teams/{team_id}/llm-providers/{id}/models/sync` | 同步模型 |
| GET | `/teams/{team_id}/llm-models` | 模型目录 |
| PATCH | `/teams/{team_id}/llm-models/{id}` | 修改模型 |
| PUT | `/teams/{team_id}/llm-model-authorizations` | 管理模型授权 |
| GET | `/teams/{team_id}/llm-model-policies` | 获取团队策略 |
| PUT | `/teams/{team_id}/llm-model-policies` | 更新团队策略 |
| GET | `/projects/{project_id}/ai-model-policies` | 获取项目模型策略 |
| PUT | `/projects/{project_id}/ai-model-policies` | 更新项目模型策略 |

所有 API 必须通过现有认证、Team Scope 和项目权限中间件。

## 17. 安全与审计

必须实现：

- Team 数据隔离。
- Credential 加密。
- SSRF 防护。
- Endpoint 安全校验。
- API Key 脱敏。
- 权限后端校验。
- 模型授权检查。
- Provider 删除保护。
- 配置变更审计。
- 调用失败审计。

审计事件：

- Provider Created / Updated / Disabled / Archived
- Credential Rotated
- Connection Tested
- Model Enabled / Disabled
- Model Authorization Changed
- Team Policy Changed
- Project Policy Changed
- LLM Call Failed

## 18. 验收标准

| ID | 验收条件 | 优先级 |
|---|---|---|
| AC-01 | Team 可以配置多个 Provider | P0 |
| AC-02 | 凭证安全加密 | P0 |
| AC-03 | Provider 连接测试 | P0 |
| AC-04 | 支持多个 Model | P0 |
| AC-05 | Team 可以管理模型授权 | P0 |
| AC-06 | Team 可以按 AI Role 配置模型 | P0 |
| AC-07 | Project 只能选择授权模型 | P0 |
| AC-08 | Project 可以覆盖 Team 默认模型 | P0 |
| AC-09 | 所有 Agent 通过统一 Gateway | P0 |
| AC-10 | Provider 故障明确失败 | P0 |
| AC-11 | 不允许自动切换模型 | P0 |
| AC-12 | 跨团队访问隔离 | P0 |
| AC-13 | 配置变更可审计 | P0 |
| AC-14 | Token / Cost 统计 | P1 |

## 19. 实施阶段

| Phase | 工作内容 |
|---|---|
| Phase 1 | 现有代码核验、数据模型、权限和凭证安全 |
| Phase 2 | Provider Registry、Adapter |
| Phase 3 | Model Catalog、Authorization、Policy Resolver |
| Phase 4 | Team Settings UI |
| Phase 5 | Project AI Configuration 集成 |
| Phase 6 | AI-PM Gateway 集成及端到端测试 |

实施前必须核验：

- 现有 Team 数据模型。
- 现有 Team RBAC。
- 现有 project_ai_config。
- 现有 AI-PM 调用链。
- 现有模型配置与 Provider 实现。
- 现有审计和密钥管理设施。

不得在未完成核验前直接实施数据库迁移。

---

# Part III：最终基线

## 20. 最终产品定义

Team LLM Provider Management 是 PM-AI 的团队级 AI 基础设施管理模块。

它负责为团队提供统一、安全、可控的大模型资源管理能力。

**最终职责边界：**

- System：管理平台规则。
- Team：拥有和管理 LLM Provider、Credentials、Models 及授权。
- Project：在授权范围内选择和使用模型。
- AI-PM Agent：通过统一 LLM Gateway 使用模型。

## 21. 版本变更记录

| 版本 | 变更 |
|---|---|
| V0.1 | 初始产品设计建议 |
| V1.0 | 完成四项正式产品裁决，确立产品基线 |

**V1.0 状态：Approved**

后续技术实现、UI/UX 设计、API 设计及数据库迁移均以本文件为依据。
