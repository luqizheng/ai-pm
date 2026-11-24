# R3 — AI 开箱体验与模型目录（B01-01 / B01-04 修复）

- 日期：2026-11-24
- 分支：`remediation/r3-ai-onboarding-model-catalog`
- 对应缺陷：**B01-04（P1）** 模型目录/授权/策略后端齐全但前端零调用；**B01-01（P1，倾向 P0）** 默认部署 AI 三循环 off + 无 Provider，开箱不可用且无引导
- 基线：`00-baseline.md` §3 B01-04 / B01-01

## 1. 根因

- **B01-04**：激活链的第二步（登记模型 → team_wide 授权 → general 策略）只有后端 REST（`/v1/pm/teams/{id}/llm-models*`，Team Owner/Admin），前端 `api/llm.ts` 只有 Provider 面——Provider 启用后 worker 报 `general_policy_missing`，普通用户走 UI 永远无法激活 AI。授权面此前连 GET 都没有（只写不读）。
- **B01-01**：默认部署 `AI_PM_CONVERSATION_REPLY=off`（worker 进程）+ 0 Provider 行 + 目录空；前端发送恒 201、轮询永不报错、无任何状态端点可查——「AI 未启用」对用户完全不可见。

## 2. 修复方案

### R3-A 模型目录前端接通（B01-04）

1. `api/llm.ts` 新增目录三面 + 授权清单 + 词表常量（与后端 ROLE_KEYS/CAPABILITIES 同源）：listModels/createModel/updateModel(CAS)/listAuthorizations/upsertAuthorizations/listTeamPolicies/setTeamPolicies。
2. **后端补 `GET /v1/pm/teams/{id}/llm-model-authorizations`**（`list_authorizations`，只回 team_wide 行）——UI 反映真实授权态的必要读取面（此前后端只写不读）。
3. 新组件 `TeamModelCatalog.vue`（接入 `SettingsTeamAiView.vue`）：
   - 登记模型行（provider+model_id+display_name+capabilities+context_window）；
   - 目录行表：启停 toggle（CAS revision）、授权/撤销（真实命令，撤销带影响面提示=后端「只报告不自动改写」语义）；
   - 七 Role 策略编辑（general 兜底缺失时显式警告 `general_policy_missing` 语义）；
   - **不伪造**：模型行必须真实登记（无静态清单兜底显示）；授权态=后端清单回填。

### R3-B 开箱引导（B01-01）

1. **新端点 `GET /v1/pm/projects/{id}/ai-readiness`**（项目成员可读，无密钥）：DB 四项数据条件（enabled Provider 数 / enabled 模型数 / 指向 enabled 模型的 team_wide 授权数 / general 策略行）+ 本进程路由开关 + `ready` 四项全绿。
2. chat 面板（常驻列 + 会话页共用）：composable 载入项目时探针一次（失败静默）；`ready=false` 时渲染 `AssistantReadinessBanner`——缺项原因清单 + 团队设置入口链接；`ready=true`/探针失败不渲染（不制造噪音）。
3. 未完成配置不影响其他项目功能（横幅只在聊天面板，不阻断）。

**诚实边界（不伪造「已就绪」）**：回复/访谈两个 worker 循环开关在 ai-pm-worker 进程内，API 进程如实**不观测**——`ready=true` 表示 worker 选模的数据条件齐备（`resolve_model_for_role` 末级兜底的通过前提），端点文档与横幅措辞均不宣称「一定有回复」。worker 开关状态的产品化观测（如心跳落库）为后续项，本批次不做假信号。

## 3. 安全基线

- 目录/授权/策略面权限不变（Team Owner/Admin；member 403 有测试）；readiness 面项目成员可读且只含计数与布尔（无凭据列）。
- 不开启任何有外部副作用的自动化循环（开关不动）；controlled automation 原则遵守——general 策略只影响选模，不绕人工审批。
- CAS 乐观锁保留（updateModel/授权 revision）；审计沿用后端既有 catalog 审计（`llm_model_authorization.changed` 等）。

## 4. 变更清单

**后端**
- `crates/pm-application/src/repo/ai_team_monitor.rs`：+`llm_readiness_counts`（trait）
- `crates/pm-infrastructure/src/repo_ai_team_monitor.rs`：实现（1 条聚合 SQL，4 标量子查询）
- `apps/pm-api/src/ai_team_monitor.rs`：+`GET /v1/pm/projects/{id}/ai-readiness`（readiness_handler）
- `crates/pm-application/src/team_llm_catalog.rs`：+`list_authorizations`
- `apps/pm-api/src/team_llm_catalog.rs`：授权面 +GET

**前端**
- `src/api/llm.ts`：目录三面 + 授权清单 + 词表常量 + `getAiReadiness`
- `src/views/settings/TeamModelCatalog.vue`（新）：目录/授权/策略管理
- `src/views/settings/SettingsTeamAiView.vue`：接入目录组件
- `src/components/shell/AssistantReadinessBanner.vue`（新）：未就绪引导横幅
- `src/components/shell/AssistantColumn.vue` / `src/views/project/assistant/AssistantView.vue`：渲染横幅
- `src/composables/useAssistantConversation.ts`：就绪探针（load/reset 生命周期）
- i18n：assistant.yaml +settings.yaml（zh/en，check:i18n 对齐）

## 5. 测试

**后端（全绿）**
- `ai_team_monitor` 套件 8/8（新 `ai_readiness_reflects_llm_data_plane`：全零→ready=false；四项种子→ready=true；跨租户 404）
- `team_llm_catalog` 套件（新 `authorization_list_reflects_current_team_wide_rows`：grant→清单含；revoke→清单清空；member 403）

**前端（全绿）**
- `SettingsTeamAiView.test.ts` 11 例（+4 目录用例：目录行/授权态渲染、授权真实命令、登记模型 createLlmModel、general 缺口警告+七 Role 保存）
- `AssistantView.test.ts` 28 例（+2 横幅用例：未就绪渲染原因+入口链接、就绪不渲染）
- 全量 vitest 回归 + vue-tsc + check:i18n

## 6. 验收标准对照（任务书 §六 R3）

- 找到现有 API/检查未接通服务/真实接入 ✅（三面 + 授权清单补读）
- 展示模型与元数据/选择模型/状态展示 ✅（目录行 + 启停 + 授权态 + 加载/失败文案）
- 模型配置影响后续请求 ✅（策略→`resolve_model_for_role` 既有链路，四项齐备 ready=true）
- 不用静态列表假装接通 ✅（无静态模型清单兜底）
- 不暴露 Provider 密钥 ✅（全部面 has_credential/hint 模式，探针零密钥）
- 新用户知道 AI 是否可用/为什么/入口/验证 ✅（横幅原因清单 + 设置链接 + 目录区 general 警告）
- 无 Provider 不误导/基础功能不受影响 ✅（横幅只在聊天面板；其他功能零改动）
- 不擅自开启外部副作用的循环 ✅（部署开关不动；审批闸门零改动）

## 7. 业务场景验证（运行态回归批次执行）

1. 全新部署（无 Provider）：chat 面板出现未就绪横幅（缺 Provider/模型/授权/策略四行）
2. UI 配置 Provider → 目录登记模型 → 授权 → general 策略 → 横幅消失
3. 发消息真实得到 AI 回复（worker 开关 on 的部署态）
