# Team LLM Provider Management — Phase 1 现有代码核验报告 V1.0

| 属性 | 内容 |
|---|---|
| 依据设计稿 | docs/ui/smark-pm-team-llm-provider-management-v1.md（2eda3b1，DEC-LLM-001~004 Approved） |
| 核验日期 | 2026-11-06 |
| 代码基线 | master db466ab（工作树含并行会话在途批：approver 0040 等，已单独标注） |
| 核验范围 | 设计稿 §19 实施前必须核验的六项 |
| 方法 | 三路并行只读侦察（migration SQL + pm-domain/pm-application/pm-ai/pm-mcp + apps/pm-api/worker） |
| 状态 | 核验完成，待用户裁决 §9 后方可进迁移设计 |

---

## 0. 结论速览

| # | 核验项 | 结论 |
|---|---|---|
| ① | Team 数据模型 | **Team ≡ tenant**（core.tenant），无独立 Team 实体 |
| ② | Team RBAC | 租户三级 owner/admin/member 已备；**Project Admin 角色不存在**；现有 LLM 端点**零角色守卫（P0）** |
| ③ | project_ai_config | **存在**（ai.project_ai_config，0003 建 0019 租户化），但逐项目单模型、strategy 锁死、无 API 可改模型字段 |
| ④ | AI-PM 调用链 | 无统一 Gateway（三处独立构造 /chat/completions）；**ai_analysis 生产主链仍是 FakeModel**；无 llm_call_record、无 token 统计 |
| ⑤ | 模型/Provider 实现 | 仅 OpenAI-Compatible 单适配器（无 streaming/tool calling）；ai.llm_provider 为**全局表**（无 tenant_id）；连接测试已有、模型发现没有 |
| ⑥ | 审计与密钥设施 | AES-256-GCM + env 派生主密钥可用（无 KMS/Vault）；审计四列+覆盖闸门测试成熟；SSRF 防护缺失；乐观锁有先例 |

**总体判断**：设计稿可落地，但 §14「复用 project_ai_config」成立的同时，`ai.llm_provider` 的形态（全局单表、每类型一行）与设计稿的 Team-owned 多实例模型**结构性冲突**，需重建而非加列；另有 1 个 P0 安全缺口（LLM 端点无守卫）应在同批修复。

---

## 1. 核验①：Team 数据模型

**事实：**

1. 全仓无 `teams` 建表；migrations grep "team" 仅命中 `tenant_*`。**Team ≡ tenant**（`core.tenant`，migrations/0019_tenant.sql:21-29：id UUID PK、provider TEXT（V1 仅 'lmms'/'system'）、external_key（上游 claims.tid）、display_name、is_active、UNIQUE(provider,external_key)）。
2. `core.tenant` 由鉴权层按上游 tid 自动 1:1 镜像创建（`ensure_tenant`，apps/pm-api/src/auth.rs:450-488，注释 Q1：上游 tid 1:1 镜像为 core.tenant，自动建即生效）。
3. 租户级成员表 `core.tenant_member`（0019_tenant.sql:31-39）：PRIMARY KEY (tenant_id, principal_id)、`role CHECK IN ('owner','admin','member')`、granted_by FK。0019 将 39 张业务表统一加 tenant_id 并建 15 组 UNIQUE(tenant_id,id) 复合键 + 组合 FK（0019_tenant.sql:154-409）。
4. Rust 侧：`TenantRole { Owner, Admin, Member }`（crates/pm-domain/src/tenant.rs:19-23）；无 Tenant/TenantMember 领域实体结构体；tenant_member CRUD 由 pm-api handler 裸 SQL 单事务实现（apps/pm-api/src/tenant_members.rs:267-371）。
5. 新主体默认 `role='member'` 且未激活；仅 env 冷启动对（AI_PM_FIRST_ADMIN_TENANT+SUBJECT）命中的首个管理员直接 'owner'（auth.rs:493-504）。

## 2. 核验②：Team RBAC 与权限面

**事实：**

1. 权限模型为混合：租户/项目 HTTP 面是「硬编码角色判断 + 每请求查库」；MCP 面是 JWT `permissions` claim 权限码（唯一能力码机制，仅服务 /mcp）。
2. HTTP 守卫：`AuthedPrincipal { tenant_id, tenant_role, is_admin }`，每请求查 `core.tenant_member.role`（auth.rs:165-175, 418-439）；中间件链 require_bearer → require_active（auth.rs:208, 291）。无独立 `require_tenant_admin` 中间件，判定散在 handler。
3. `core.tenant_member.role` 是租户角色唯一事实源，token `roles` 不参与授权（crates/pm-domain/src/tenant.rs:3-4）。Owner 与 Admin 可区分：admin 不能授 admin（tenant_members.rs:499-501）、不能动 owner（:526-528）；仅 owner 可转让所有权（POST /v1/pm/tenants/{tenant_id}/owner-transfer，tenant_members.rs:48-51）；最后 owner 保护 OWNER_LAST（:537-539）。
4. Project 角色：`core.project_member.role` 一列双语义 CHECK ('owner','member','ai_pm','architect','developer','tester')（0001_core.sql:41-44 + 0030_project_member_role.sql:8-9）；真人访问角色仅 `ProjectMemberRole { Owner, Member }`（pm-domain/src/project_member.rs:14-19），Agent 职能值不是访问等级（:47-62）。`is_project_manager` = 租户 owner/admin **或** 项目 owner（project_member.rs:178-209）。
5. 项目成员必须先是租户成员（project_member.rs:132-133；tenant_members.rs:332-348 级联撤）。项目归属固定 caller.tenant_id（pm_lists.rs:130-166，T-022）。
6. 路由惯例：团队级 = **`/v1/pm/tenants/{tenant_id}/...`**（members/owner-transfer，tenant_members.rs:34-52）；全仓无任何 `/teams/` 前缀。项目级 = /v1/pm/projects/{project_id}/...。MCP 20 个工具全部项目域，无团队级操作（crates/pm-mcp/src/caller.rs:36-60）。
7. **P0 缺口：现有 LLM 端点零角色守卫**。GET/PUT `/v1/pm/llm/providers[/{provider}]`、POST `.../test`（apps/pm-api/src/llm_configs.rs:27-29，挂载 routes.rs:185）三个 handler 均不取 `Extension<AuthedPrincipal>`、无任何 is_admin/tenant_role 判断——任何持有效 JWT 的激活主体可读**和写**全部 Provider 配置（含覆盖 API Key 密文、外发探测请求）。应用层 `llm_config::{list_providers, update_provider, load_provider_credentials}` 同样无 caller/role 参数（crates/pm-application/src/llm_config.rs）。
8. 无任何 llm/model/ai 相关权限码；现有 MCP 码全集仅 PM_MCP_READ / PM_MCP_TASK_UPDATE（未分配）/ PM_MCP_DOCUMENT_WRITE（auth.rs:576-649）。

## 3. 核验③：project_ai_config

**事实：**

1. **存在**：`ai.project_ai_config`（migrations/0003_platform.sql:52-70）——project_id PK（FK core.project）、ai_principal_id FK、`model_provider TEXT`、`model_name TEXT`、`model_config JSONB`、`strategy CHECK 只允许 'single_strong_model'`、`automation_mode CHECK (ai_lead|controlled)`、policy_version INT、**config_revision BIGINT**、enabled、updated_at。0019 租户化（0019_tenant.sql:123 加 tenant_id、:301-302 组合 FK）。
2. 全局供应商配置 `ai.llm_provider`（migrations/0007_llm_provider.sql:7-25）：`provider TEXT PK CHECK IN ('deepseek','openai','glm','minimax','ollama')`、base_url、model（单值）、`api_key_ciphertext BYTEA`、api_key_hint、enabled（种子 5 行全 false）、timeout_secs 1..600。**0019 给全表加租户列时未包含它——它是全局表，无 tenant_id**。
3. 消费点：start_run 读 model_name/model_config/policy_version 快照（crates/pm-ai/src/context.rs:112,169）；orchestrator 读 ai_principal_id 作执行主体（orchestrator.rs:223）；repo 链读 model_provider 选模型（repo_model.rs:247）；job claim 过滤 enabled AND automation_mode='ai_lead'（pm-infrastructure/src/job_queue.rs:41-43）。
4. 写入点：仅 bootstrap 种子 ('fake','fake',...)（pm-application/src/bootstrap.rs:289-296）+ 监督冻结 automation_mode（supervision/freeze.rs:43）。**没有任何 API 能更新 model_provider/model_name**（pm-api 全目录无 project_ai_config 引用）。
5. 无按用途/角色选模型：每项目单组 model_provider/model_name；skill_router 按事件路由 skill 但所有 skill 共用同一模型实例（skill_router.rs:9-25 + orchestrator.rs:211 单次 model.analyze）。

## 4. 核验④：AI-PM 调用链

**事实：**

1. 入队：MCP → pm-api /mcp → `ingestion::submit_event` 单事务写 inbound_event + job(job_type='ai_analysis')（pm-application/src/ingestion.rs:99-105）。消费：ai-pm-worker 60ms 轮询 `job_queue::claim`（FOR UPDATE SKIP LOCKED + 租约 + freeze 过滤，job_queue.rs:39-71；ai-pm-worker/src/main.rs:212-217）；失败退避 max_attempts=5。
2. pm-ai 模块：model.rs（`AnalysisModel` trait + `ModelError{Timeout,Unavailable,BadOutput}`，model.rs:22-27）、openai_adapter.rs、repo_model.rs（第二套端口 `RepoAnalysisModel`）、context.rs、skill_router.rs、orchestrator.rs（八步链，:118-376）、self_review.rs、fake_model.rs。**无统一 gateway/policy/queue 模块**。
3. **P0 级事实：ai_analysis 生产主链仍接 FakeModel**——apps/ai-pm-worker/src/main.rs:59-63 `Arc::new(FakeModel::from_fixtures())`，注释 TODO(Task 17)。`OpenAiCompatibleModel` 全仓仅测试构造（pm-ai/tests/openai_adapter.rs:31）；生产真实调用仅 repo_import 链（main.rs:236 → repo_model.rs:242-283）。
4. 模型/端点/Key 全在 DB（ai.llm_provider），env 仅有解密主密钥 `AI_PM_SECRET_KEY`。请求 `{base_url}/chat/completions`、`.no_proxy()`、timeout 绑进 client（openai_adapter.rs:65-76,139）。
5. **三处独立构造 /chat/completions**：openai_adapter.rs:139（分析链，生产未接线）、repo_model.rs:209（仓库链）、apps/pm-api/src/llm_configs.rs:112-122（test 探针自建 reqwest）——即无统一 LLM Gateway，架构稿规划的 Model Gateway（docs/ai-pm-architecture-principles.md:134,1415）未落成。
6. 策略引擎 `PolicyEngine` 是**命令治理**（AutoApply/RequirePreConfirmation/Escalate/Deny，YAML policies/ai-pm-ai-lead-v2.yaml，pm-domain/src/policy.rs:8-13；AiPm 执行入口 orchestrator.rs:510-513 走 PgCommandExecutor，红线落 governance.approval_request :591-601），与「模型选择」无关。
7. 调用记录：`ai.analysis_run`（0003:107-127，status/model_name/policy_version/config_snapshot/result/failure_code/attempt_count）按 event 粒度；失败码 model_timeout/unavailable/bad_output（orchestrator.rs:687-693）。**无请求级 llm_call_record、零 token 用量统计**（全仓 grep token/usage 零命中）。审计仅覆盖「已执行命令」（command_executor/audit.rs:24）+ RunCompleted 补写（outbox_handlers.rs:280）。
8. pm-mcp 不构造 LLM 请求（crates/pm-mcp/src/lib.rs:1-15，无 reqwest）。

## 5. 核验⑤：现有模型配置与 Provider 实现

**事实：**

1. 唯一真适配器 = `OpenAiCompatibleModel`（openai_adapter.rs:46-51,115-176），兼容任意 /chat/completions 面（OpenAI/GLM/DeepSeek/vLLM）；能力：structured output 仅 response_format:json_object（:127-134）；**无 streaming、无 tool calling**。错误分类：5xx/connect→Unavailable、4xx→BadOutput、timeout→Timeout（:7-13,104-112）。
2. 无 async-openai/anthropic SDK；外部依赖仅 reqwest 0.12、aes-gcm 0.10、wiremock 0.6（测试）。
3. Provider 形态是 **DB 行非代码**：ai.llm_provider 全局表（见 §3.2）；「5 provider 全 disabled」即种子行 enabled=false；代码白名单常量 `PROVIDERS: [&str;5]`（pm-application/src/llm_config.rs:17）+ DB CHECK + 前端联合类型三处双写死（frontend/src/api/llm.ts:4-10、ai-frontend/src/api/llm.ts 同构）——扩展 anthropic/gemini 需三处同改。
4. 连接测试**已有**：POST /v1/pm/llm/providers/{provider}/test（llm_configs.rs:84-156），服务端发 "ping"/max_tokens 16 探活，返回 {ok,latency_ms,message}，错误截断 200 字符，不回显密钥。
5. 模型发现**不存在**：无任何 GET /models 调用；model 字段是手填自由文本（0007:12）。
6. Provider 选择兜底：项目配置 model_provider 优先（非 'fake'），否则**全局第一条 enabled**，再取不到回退 Fake（repo_model.rs:242-283）——租户化后此兜底语义必须重做。

## 6. 核验⑥：审计与密钥管理设施

**事实：**

1. 审计表：`governance.audit_log`（0003:188-203 + 0004 source_kind/source_ref + 0018 actor_identity_id/actor_client/actor_auth_mode + 0027 claimed_device_id）；租户级镜像 `governance.tenant_audit_log`（0019:51-65）+ 同事务写入先例（tenant_members.rs:364-366,634-659）。
2. 写入口：命令路径 `insert_audit`（command_executor/audit.rs:7-48）；人工写路径 `append_audit`（pm_write/audit.rs:40-102）。T-088 四列 = actor_identity_id/actor_client/actor_auth_mode/claimed_device_id，清单闸门测试 `audit_actor_coverage.rs:13-46`——**新增审计 INSERT 必须写齐四列并登记清单，否则 CI 失败**。action 为自由 TEXT 无枚举。
3. 加密设施：`AI_PM_SECRET_KEY` env → sha256 → AES-256-GCM（llm_config.rs:62-71；secret.rs:36-44 同口径）；密文格式 nonce(12)||ciphertext，hint 只留末 4 位 `****xxxx`（:73-78）。两套工具：llm_config（encrypt/decrypt_api_key）+ secret.rs（seal_text/open_text，**AAD 绑定 project_id** 防跨项目搬运；ed25519 经 ssh-keygen 即焚生成）。
4. 无 KMS/Vault：主密钥是单一 env，.env.prod:26 存 dev 明文；架构稿 7.1 自己已声明「应放密钥管理系统」为未实现（ai-pm-architecture-principles.md:555）。
5. 加密存第三方凭证先例两个：LLM key（ai.llm_provider.api_key_ciphertext）+ 仓库 SSH 私钥（repository.repo_ssh_key.private_key_ciphertext，0008:2-11，明文永不回显 repository/mod.rs:7-8）。
6. 脱敏回显已达标（has_api_key + hint）；SSRF 防护**缺口**：base_url 仅校验 http(s) 前缀（llm_config.rs:136-141），无私网/元数据黑名单、无 DNS 固定解析。
7. 配置变更审计与乐观锁**缺口**：update_provider 与 test 端点均不写 audit_log；ai.llm_provider 无 version 列（last-write-wins，llm_config.rs:179-197）。乐观锁先例成熟可抄：CompensableTable 显式声明乐观锁列 + RevisionConflict 409（command_executor/mod.rs:110-130、executor.rs:297-305）；pm_write FOR UPDATE + revision+1（pm_write/task.rs:100-109）；project_ai_config 自带 config_revision BIGINT 可直接复用。
8. 无 config crate（全部 std::env::var 散点读取）；JWT 无本地签名密钥（lmms token-service 签发，pm-api 仅 RS256 验证，auth.rs:4-6,34）。

---

## 7. 与设计稿的偏差清单（R-LLM-xx）

| ID | 级别 | 偏差 | 说明 |
|---|---|---|---|
| R-LLM-01 | **P0 安全** | 现有 LLM 端点零角色守卫 | 任何激活 JWT 可读写 Provider 凭证（§2.7）；违反设计稿 §12 权限矩阵与 §17 后端校验要求，落地时同批修复 |
| R-LLM-02 | **P0 结构** | ai.llm_provider 全局表 vs Team-owned 多实例 | 全局单表、每类型一行、无 tenant_id、无实例 ID/Name/状态生命周期（§3.2）；与 DEC-LLM-001/002 结构性冲突，建议新表重建而非加列（处置见 §8） |
| R-LLM-03 | P1 | 无统一 LLM Gateway | 三处独立构造请求（§4.5）；AnalysisModel 与 RepoAnalysisModel 双端口 + test 探针第三处；AC-09 要求收敛 |
| R-LLM-04 | P1 | AI Role 策略解析链不存在 | project_ai_config 单模型 + strategy 锁死 single_strong_model；skill 不分模型（§3.5）；DEC-LLM-003 七 Role 全新建设 |
| R-LLM-05 | P1 | ai_analysis 主链仍是 FakeModel | TODO(Task 17)（§4.3）；AC-09「所有 Agent 通过统一 Gateway」的前置是先完成真模型接线 |
| R-LLM-06 | P1 | llm_call_record 与 token/cost 统计缺失 | 仅 run 级 analysis_run（§4.7）；AC-14（P1）全新增 |
| R-LLM-07 | P1 | Project Admin 角色不存在 | 真人项目访问角色仅 owner/member（§2.4）；设计稿 §12 的 Project Admin 需映射为 project owner 或扩展枚举（待裁决） |
| R-LLM-08 | P2 | Provider 类型清单不一致 | 现有 deepseek/openai/glm/minimax/ollama vs 设计稿 OpenAI/Anthropic/Gemini/DeepSeek/OpenAI-Compatible（§5.3 三处双写死） |
| R-LLM-09 | P2 | 模型发现（API 自动发现）不存在 | model 手填（§5.5）；设计稿 §8.3 的 API 自动发现全新增 |
| R-LLM-10 | P2 | SSRF 防护缺失 | 仅 http(s) 前缀校验（§6.6）；§17 要求 Endpoint 安全校验 |
| R-LLM-11 | P2 | 无 KMS/Vault | 主密钥为 env 派生（§6.4）；设计稿 §7 要求 KMS/Vault 管主密钥——V1 口径待裁决 |
| R-LLM-12 | P2 | Provider 变更无审计、无乐观锁 | update_provider 不写审计、无 version（§6.7）；现有 tenant_audit_log + config_revision 先例可直接套用 |

**达标项（可直接复用）**：AES-256-GCM 凭证加密 + 脱敏回显（hint 末 4 位）、连接测试端点、审计四列与覆盖闸门、租户级审计表、乐观锁先例、Team(tenant) 三级角色与 owner 转移/最后 owner 保护规则、Team Scope 组合 FK 模式。

## 8. 实施影响与建议

1. **Team ≡ tenant 映射**：设计稿「Team」落地为 `core.tenant`，路由对齐现有惯例 `/v1/pm/tenants/{tenant_id}/llm-*`（不新造 `/teams/`），资源表一律带 tenant_id + UNIQUE(tenant_id,id) 复合键（照抄 0019 模式）。
2. **ai.llm_provider 处置**：现表数据仅 5 条 disabled 种子、主链 FakeModel——建议新建 `ai.team_llm_provider`（等七实体）后**退役** 0007 表（drop 或保留只读过渡），repo_import 的「全局第一条 enabled」兜底同批改租户作用域解析。不做加列改造（PK 是 provider TEXT，无法承载同类型多实例）。
3. **project_ai_config 复用**：按设计稿 §14 要求复用现表作为 Project 覆盖载体——扩展 strategy 枚举（解锁 single_strong_model CHECK）或另建 project_ai_model_policy 表、现表保留自动化/主体配置；config_revision 直接作乐观锁。倾向后者（策略与运行配置分离，删改面小）。
4. **Gateway 落地点**：以 `crates/pm-ai` 新增 gateway 模块收敛三处请求构造为单一 Credential Resolver + Adapter 面；同批完成 Task 17（main.rs:61 FakeModel → 真适配器），否则 AC-09/AC-10 无验证载体。
5. **权限修复顺序**：R-LLM-01 不必等新模块——可为现有三个端点先接 AuthedPrincipal + is_tenant_admin（一行守卫级改动），随本特性第一批落地。
6. **迁移编号**：0040 已被并行会话 approver 批占用（未提交但在工作树），本特性自 **0041** 起；动手前需再核对最新工作树。
7. **审计接入**：新审计点须同步登记 audit_actor_coverage.rs 清单（§6.2），否则 CI 红。
8. **Adapter 扩展**：OpenAI-Compatible 适配器可覆盖现有五家；Anthropic/Gemini 需新适配器（无 SDK 依赖，手写 reqwest 面）；streaming/tool calling 现状不支持，V1 能力矩阵按实际填充（§8.2 要求「来源于实际能力或验证过的配置」可满足）。

## 9. 待用户裁决项

| # | 问题 | 建议 |
|---|---|---|
| Q-1 | Team ≡ tenant 映射与路由命名（/v1/pm/tenants/... 而非 /teams/） | 接受为实现映射 |
| Q-2 | Project Admin ≈ project owner（不新造角色）还是扩展 project_member 枚举 | 映射为 project owner；未来需要再立迁移 |
| Q-3 | ai.llm_provider 退役 vs 双表并存过渡 | 直接重建退役（数据仅 disabled 种子） |
| Q-4 | KMS/Vault V1 口径：沿用 AI_PM_SECRET_KEY env 派生 + 挂账，还是本期引入外部 KMS | V1 沿用现状 + 密钥轮换支持（替换机制已在设计稿 §7），KMS/Vault 挂账独立裁决 |
| Q-5 | DEC-LLM-003 七 Role 与现有 skill_router 技能集（requirement_analysis/risk_detection 等）及项目职能（ai_pm/architect/developer/tester）的映射 | Phase 3 设计稿时给出映射表 |
| Q-6 | Usage & Budget（AC-14，P1）本期是否只建 llm_call_record 表 + 基础计数，预算管控后置 | 表随 Phase 1 建，管控后置 |
