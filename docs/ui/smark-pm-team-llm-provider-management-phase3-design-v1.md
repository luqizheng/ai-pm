# Team LLM Provider Management — Phase 3 实施设计任务书 V1.0

| 属性 | 内容 |
|---|---|
| 产品模块 | Team Settings / AI Configuration（Model Catalog · Authorization · Default Model Policies） |
| 文档版本 | V1.0 |
| 文档状态 | 待用户裁决（§12 五项） |
| 上游基线 | docs/ui/smark-pm-team-llm-provider-management-v1.md（V1.0 Approved，DEC-LLM-001~004） |
| 现状核验 | docs/ui/smark-pm-team-llm-provider-management-code-facts-v1.md（§10 实施批注）+ 2026-11-11 全仓盘点 |
| 迁移编号 | 0018（checkpoint 模式：0001_baseline.sql 为全量快照，0002~0017 为增量，下一号 0018） |
| 范围 | 3+1 张新表 · 8 组端点 · 解析链四级化 · 九链 Role 映射 · UI 03/04/07 解锁 |

---

## §1 背景与范围

Phase 1-2 已落地（0041 纵切 + 2026-11-11 D-1/D-3 修复）：Provider CRUD/test/credential/status、AES-256-GCM、权限矩阵、`resolve_repo_call_config` 三级过渡解析。**本任务书兑现设计文档 §14/§16/§15 的全部缺失项**：

| 缺口（对照盘点） | 本任务书交付 |
|---|---|
| `ai.team_llm_model` 缺失（模型仅 `configuration->>'model'` 自由文本） | §4.1 模型目录表 + §5 API |
| `ai.team_llm_model_authorization` 缺失（AC-05 无载体） | §4.2 授权表 + §5.4 授权端点 |
| `ai.team_llm_model_policy` 缺失（AC-06 无载体） | §4.3 团队 Role 策略表 |
| 项目覆盖无 Role/Model 维度（D-2） | §4.4 项目 Role 覆盖表 |
| 解析链无 Role 两级（§10.3 偏差） | §6 四级解析链 + 九链映射（Q-5 兑现） |
| `models/sync` 无实现（R-LLM-09） | §5.5 + §7 Adapter `discover_models` |
| 审计缺 3 事件 | §8.2 补全 |
| UI 03/04/07 被阻塞 | §9 前端契约 |

**不在本任务书范围**：Usage & Budget 管控（Q-6 已裁决后置）；KMS/Vault（Q-4 已裁决挂账）；DNS 解析后 SSRF 校验（随 Gateway 挂账）。

## §2 现状基线（代码事实，写作时点 2026-11-11）

1. `ai.team_llm_provider`：Team-owned 实例，status 七态 CHECK，revision CAS，`configuration->>'model'` 为当前唯一模型载体（baseline.sql:1163-1180）。
2. `ai.project_ai_model_policy`：PK(tenant_id, project_id) 单行 → provider_id（**项目 Provider 约束**，解析第一级「严格不回退」），无 model/role 维度（baseline.sql:1038-1045）。
3. `ai.llm_call_record`：error_kind CHECK 七类词表（0041/0004）——`configuration` 已在词表，`analysis_error_kind` 已产 `"configuration"`（2026-11-11 D-1）；执行链 401/429 仍折入 `provider_error`（openai_adapter.rs:153-161）。
4. 解析函数：`resolve_repo_call_config`（pm-application/team_llm.rs:1195+）三级过渡口径，9 条调用链消费。
5. AI Role 两套词表并存：
   - 项目 AI Role 目录（`pm-domain/src/ai_role.rs:59-65`）：`project_manager / requirement_analyst / architect / developer / tester`（执行职能，含不可禁用约束）；
   - 设计文档 §10.1 七 Role（分析职责）：General / AI-PM Lead / Requirement Analysis / Task Analysis / Risk Analysis / Architecture Analysis / Knowledge Management。
6. repo 链（`repo_model::model_from_llm_provider`）Fake 回退挂账 Phase 3 兑现（2026-11-11 裁决）。

## §3 裁决 D-P3-1：Role 词表（兑现 code facts Q-5）

**裁决建议：`team_llm_model_policy.role_key` 采用设计文档 §10.1 七 Role，snake_case；与项目 AI Role 目录（执行职能）正交。**

```text
general | ai_pm_lead | requirement_analysis | task_analysis | risk_analysis
| architecture_analysis | knowledge_management
```

理由（唯一推荐）：
1. 模型策略的解析维度是「调用用途」（哪类分析用哪种模型），项目 AI Role 目录的维度是「执行主体」（哪个 Agent 角色）——两套语义正交，混用会让 `project_manager` 的「不可禁用」等执行约束污染模型策略层。
2. `general` 为兜底键（设计文档 §10.3 解析末级），未单独配置的 Role 继承它。
3. 九条调用链到 Role 的映射是机械归档（§6.3），不依赖执行职能词表；未来若需按执行职能细分，可在执行主体→用途 Role 之间加映射层，不在 V1。

**与现有 5 职能词表的关系**：零改动。`ai.project_ai_role` 体系（目录/启停/版本）继续服务 Agent 编排；Phase 3 只新增模型策略维度。

## §4 数据库设计（迁移 0018）

惯例：schema `ai.`、UUID PK、TIMESTAMPTZ、`UNIQUE(tenant_id, id)` 组合键 + 组合 FK（0019 模式）、`revision bigint` CAS、snake_case。敏感凭证不落任何 JSONB（沿用现有 credential 表，模型表零凭证）。

### §4.1 `ai.team_llm_model`（模型目录）

| 列 | 类型 | 约束 |
|---|---|---|
| id / tenant_id | uuid | PK(tenant_id,id) 组合，FK→team_llm_provider(tenant_id,id) ON DELETE CASCADE |
| provider_id | uuid | FK 同上；**Team ID 一致性由组合 FK 保证**（设计文档 §14 约束 2） |
| model_id | text | NOT NULL；`UNIQUE(tenant_id, provider_id, model_id)` |
| display_name | text | NOT NULL |
| capabilities | text[] | DEFAULT '{}'；词表 §4.1a |
| context_window | integer | NULL（未知不得编造，§8.3） |
| enabled | boolean | DEFAULT false（**发现≠启用**，§8.3） |
| source | text | CHECK ('discovered','manual') |
| last_verified_at | timestamptz | NULL |
| revision / created_by / created_at / updated_at | | 惯例 |

§4.1a capabilities 词表（设计文档 §8.2 六能力）：`text_generation | reasoning | structured_output | tool_calling | vision | long_context`。未知能力 = 空数组（UI 显示「未验证」），禁止编造。

### §4.2 `ai.team_llm_model_authorization`（授权，AC-05）

| 列 | 类型 | 约束 |
|---|---|---|
| id / tenant_id | uuid | 组合 PK/FK→team_llm_model |
| model_id | uuid | `UNIQUE(tenant_id, model_id)`（一模型一授权行） |
| authorization | text | CHECK ('team_wide','revoked')；V1 两态（§9「V1 默认团队内项目可用」；`restricted` 按项目限制列为 additive 扩展，V1 不建关联表——AGENTS 规则 1） |
| granted_by / granted_at / revision / updated_at | | 惯例 |

语义：模型 `enabled=false` = Team 层停用（能力面）；`authorization='revoked'` = 撤销项目使用许可（许可面）。撤销不删目录行、不改 enabled——两者独立（设计文档 §9 撤销不得静默改写项目配置）。

### §4.3 `ai.team_llm_model_policy`（Team 按 Role 默认模型，AC-06）

| 列 | 类型 | 约束 |
|---|---|---|
| tenant_id / role_key | | PK(tenant_id, role_key)；role_key CHECK ∈ §3 七值 |
| model_id | uuid | 组合 FK→team_llm_model；**应用层校验 enabled + authorized** |
| updated_by / revision / updated_at | | 惯例 |

未配置的 Role = 无行 → 解析继承 `general`（§6）。`general` 行删除 = Team 未配 General → 解析末级 Configuration Error（显式，§6.1 reason=general_policy_missing）。

### §4.4 `ai.project_ai_model_override`（项目 Role 覆盖，AC-08，修 D-2）

| 列 | 类型 | 约束 |
|---|---|---|
| tenant_id / project_id / role_key | | PK 三元；role_key CHECK ∈ §3 七值 |
| model_id | uuid | 组合 FK→team_llm_model；应用层校验 enabled + authorized |
| updated_by / revision / updated_at | | 惯例 |

删除行 = 恢复继承 Team（现 `project_ai_model_policy` 的 None 语义平移）。**现表 `project_ai_model_policy` 保留原语义（项目 Provider 约束，§6.2），零改动**——策略与运行配置分离（code facts §8.3 同款倾向）。

## §5 API 设计

路由惯例 `/v1/pm/tenants/{tenant_id}/...`（Q-1 已裁决映射）；全部端点 `guard_tenant + is_admin`（§12 矩阵：Team Owner/Admin），项目覆盖端点 `guard_project_manager`。写端点全部 CAS（`expected_revision`）。

| Method | Endpoint | 说明 | 对应设计稿 §16 |
|---|---|---|---|
| POST | `/tenants/{tid}/llm-providers/{pid}/models/sync` | API 自动发现（§5.5） | `.../models/sync` |
| POST | `/tenants/{tid}/llm-models` | 手动登记（§5.5 双通道之二） | 新增（增补端点） |
| GET | `/tenants/{tid}/llm-models?provider_id=&enabled=&q=` | 目录（含授权状态、能力、上下文窗） | `GET .../llm-models` |
| PATCH | `/tenants/{tid}/llm-models/{mid}` | display_name/capabilities/context_window/enabled；CAS | `PATCH .../llm-models/{id}` |
| PUT | `/tenants/{tid}/llm-model-authorizations` | 批量 `{items:[{model_id, action: grant\|revoke}]}`；撤销响应含影响面（引用该模型的项目覆盖行数 + Team 策略行数——只报告不自动改写） | `PUT .../llm-model-authorizations` |
| GET | `/tenants/{tid}/llm-model-policies` | 七 Role 当前策略（含未配置→inherit_general 标记） | `GET .../llm-model-policies` |
| PUT | `/tenants/{tid}/llm-model-policies` | 批量 `{items:[{role_key, model_id\|null}]}`；null=删行继承 general | `PUT .../llm-model-policies` |
| GET/PUT | `/projects/{pid}/ai-model-policies` | **语义扩展**：返回 `{provider_constraint, overrides:{role_key→model}, team_default_ref}`；PUT 接受 overrides 批量（model 必须 enabled+authorized）+ provider_constraint 原语义 | 现有两端点（升级） |

### §5.5 模型发现双通道

- **自动发现**：`discover_models` Adapter trait（§7.1）。V1 仅 `OpenAiCompatibleModel` 实现：GET `{endpoint}/models`（OpenAI 标准面，Bearer 认证）→ upsert `team_llm_model`（source='discovered'，enabled **不自动置 true**）。
- **手动登记**：POST 端点全类型可用（anthropic/gemini/deepseek 无适配器时不阻塞目录建设）。
- 适配器缺失/类型不支持 → 显式 `AdapterUnavailable`（复用现有探针错误路径），禁止静默空结果。

## §6 解析链（AC-09 核心）

### §6.1 四级解析（设计文档 §10.3 完整兑现）

```text
resolve_model_for_role(pool, tenant_id, project_id, role_key)
  1. project_ai_model_override(role_key)        → model   （项目 Role 覆盖）
  2. team_llm_model_policy(role_key)            → model   （Team Role 策略）
  3. team_llm_model_policy('general')           → model   （Team General）
  4. Configuration Error                                  （显式，禁止降级）

每一级命中后的严格校验（任一失败 → Configuration Error，reason 区分）：
  model.enabled = true ∧ authorization = 'team_wide'
  ∧ model.provider.status = 'enabled' ∧ 凭证可用
  ∧ （若项目有 Provider 约束行：model.provider_id = 约束.provider_id）
```

reason 词表：`no_model_for_role` / `general_policy_missing` / `model_disabled` / `authorization_revoked` / `provider_not_enabled` / `credential_missing` / `provider_constraint_violation`。

**返回结构**：复用 `ResolvedProvider`（provider_id/endpoint/model/api_key/timeout_secs）——新增 `model_ref`（uuid）供记账归因。`resolve_repo_call_config` 退役：其 9 个消费方全部切换 `resolve_model_for_role`（§6.3），函数本体在 3B 末删除（无兼容层，AGENTS 规则 1）。

### §6.2 项目 Provider 约束（现表语义升级）

`project_ai_model_policy` 单跳从「解析入口」降为「候选过滤器」：约束行存在时，四级链解析出的模型所在 Provider 必须等于约束 Provider，否则显式 `provider_constraint_violation`——与现「项目指定严格不回退」语义连续（项目的显式管理员决策不被 Team 策略静默绕过）。

### §6.3 九链 Role 映射（Q-5 兑现，⚑待裁决 TQ-2）

| 调用链 | role_key | 依据 |
|---|---|---|
| ai_analysis 主链（RoutedAnalysisModel） | `ai_pm_lead` | 设计文档 §10.1「AI-PM Lead 默认高能力模型」 |
| conversation_reply | `general` | 对话兜底用途 |
| conversation_router_model（意图分类） | `general` | 轻量分类，跟随对话 |
| retrieval_planner | `general` | 检索规划轻量用途 |
| writeback_model | `knowledge_management` | 写回 = 知识维护 |
| snapshot_model | `knowledge_management` | 快照分析同域 |
| repo_model（import 分析） | `general` | 导入摘要用途；**Fake 回退同批兑现为显式 Configuration Error**（2026-11-11 挂账） |
| discovery_orchestrator（编排轮） | `requirement_analysis` | 编排产出需求候选 |
| discovery writeback（任务分解） | `task_analysis` | 任务拆解用途 |

### §6.4 记账与故障分类补全

- `llm_call_record` 增 `model_ref uuid NULL`（additive；引用 ai.team_llm_model(tenant_id,id) 组合 FK）——token/费用按模型归因（AC-14 前置）。
- openai_adapter 错误分类升级：401/403 → `authentication`、429 → `rate_limit`（现折入 BadOutput）；`analysis_error_kind`/`repo_error_kind` 相应透出词表值。词表零迁移（0041 CHECK 已含全七类）。

## §7 Adapter 层

§7.1 `LlmProviderAdapter` trait 扩展：`async fn discover_models(&self, cfg) -> Result<Vec<DiscoveredModel>, AdapterError>`。V1 实现：OpenAICompatible（GET /models）。anthropic/gemini 返回 `AdapterError::Unavailable`（显式）。

§7.2 anthropic 适配器（Phase 2 挂账）**不阻塞本任务书**：手动登记兜底；适配器落地后 sync 自动覆盖。是否前置为独立切片 ⚑TQ-5。

## §8 权限与审计

§8.1 权限：§5 全表——目录/授权/策略 = Team Owner/Admin；项目覆盖 = Project Admin（guard_project_manager 现惯例，Q-2 已裁决映射）。Member 无入口 403；跨租户 404（team_llm_guard 惯例）。

§8.2 审计（governance.tenant_audit_log，target_type 补 `llm_model`；四列身份 + audit_actor_coverage 登记，CI 闸门）：

| 事件 | 触发 |
|---|---|
| llm_model.created / synced / updated / enabled / disabled | 目录面 |
| llm_model_authorization.changed | grant/revoke（payload 含 action+影响面计数） |
| llm_team_policy.changed | Team 策略 PUT |
| llm_provider.project_policy_changed | 已有，payload 扩 role 维度 |

## §9 前端契约（UI 03/04/07 解锁 + 05 完整版 + 01 增列）

`ai-frontend/src/api/llm.ts` 扩展契约（tenant 路径/CAS/授权状态）；页面按设计稿七页框架接线：

| 页 | 后端依赖 | 本批交付 |
|---|---|---|
| 03 Model Catalog | GET/PATCH llm-models + sync | 全量（能力标签/上下文窗/启用态/授权状态/详情抽屉；未验证能力显示「未验证」） |
| 07 Model Authorization | PUT authorizations | 授权/撤销 + 撤销前影响面展示（不自动改写） |
| 04 Default Model Policies | GET/PUT llm-model-policies | 七 Role 卡片（Provider+模型+继承标记；选择器只列授权模型——AC-07 UI 面） |
| 05 Project AI Configuration | 项目 ai-model-policies 升级 | 覆盖集 + Team 默认参照 + 恢复继承；无凭证/Provider 管理动作 |
| 01 Provider Management | 目录 COUNT | 「已配置模型数量」列从 `configuration->>'model'` 单值改目录计数 |

i18n 走 `settings.team.ai.*` 命名空间扩展；门禁 check-prism-tokens/check-i18n 照常。

## §10 实施切片

| 切片 | 内容 | 依赖 |
|---|---|---|
| 3A | 迁移 0018（4 新表 + llm_call_record.model_ref）+ 服务层（目录/授权/策略 CRUD）+ 8 组端点 + 审计 + pm-api 集成测试 | 无 |
| 3B | `resolve_model_for_role` 四级链 + 九链迁移 + `resolve_repo_call_config` 退役 + repo 链 Fake 兑现 + 测试 | 3A |
| 3C | adapter `discover_models` + sync 端点实现 + 401/429 分类升级 + 前端 5 页接线 + 门禁 | 3A（3B 可并行） |

每个切片独立可验证；3B 是 AC-09/AC-10 的达成点（统一解析 + 显式失败）。

## §11 验收标准（对照设计文档 §18）

| AC | 达成方式 | 验证 |
|---|---|---|
| AC-04 多模型 | §4.1 目录 + UNIQUE(tenant,provider,model_id) | 3A 集成测试：同 Provider 多模型/同模型多 Provider |
| AC-05 授权管理 | §4.2 + PUT 端点 + 撤销影响面 | 3A 测试：grant/revoke/撤销不静默改写 |
| AC-06 按 Role 配模型 | §4.3 + 七 Role 端点 | 3A 测试：七值 CHECK/继承语义 |
| AC-07 项目只能选授权模型 | §6.1 严格校验 | 3B 测试：revoked/disabled 模型 → Configuration Error |
| AC-08 项目覆盖 | §4.4 + 恢复继承 | 3B 测试：覆盖优先/删行回 Team |
| AC-09 统一 Gateway | §6.3 九链全量切换 + resolve_repo_call_config 退役 | 3B：全仓 grep 无直连 resolve 旧函数 |
| AC-10 显式失败 | §6.1 reason 词表 + repo 链兑现 | 3B：无配置 → configuration error_kind 落账 |
| AC-13 审计 | §8.2 全事件 | audit_actor_coverage CI 绿 |
| AC-14（P1） | §6.4 model_ref 归因 | 落账面就绪，管控后置（Q-6） |

## §12 待用户裁决项

| # | 问题 | 建议 |
|---|---|---|
| TQ-1 | §3 七 Role 词表（snake_case 七值）与项目职能词表正交 | 按 §3 采纳 |
| TQ-2 | §6.3 九链映射表（discovery 两链的归档可再议） | 按 §6.3 采纳 |
| TQ-3 | §6.2 项目 Provider 约束 = 解析结果严格校验（非候选过滤） | 按 §6.2 采纳（延续现「严格不回退」语义） |
| TQ-4 | 授权 V1 两态（team_wide/revoked），`restricted` 留 additive 扩展 | 按 §4.2 采纳 |
| TQ-5 | anthropic 适配器是否前置为独立切片 | 建议非阻塞（手动登记兜底，3C 后独立切片） |

---

**版本记录**：V1.0（2026-11-11）——初稿，基于 code facts V1.0 §10 批注 + 2026-11-11 全仓盘点 + D-1/D-3 修复后的解析语义。