# S02-02 · 需求领域运行时核验报告 V1.0

> **阶段**：S02-02（运行时核验 / 缺陷复现 / 证据收集）
> **前置依据**：`docs/scenarios/s02/ui/smark-pm-s02-01-requirement-domain-asset-inventory-v1-1.md`（V1.1，§13.1 V-1~V-5）
> **任务书**：《S02-02 需求领域运行时核验任务书 V1.0》（专家提供，V-1~V-6）
> **本报告性质**：运行时证据记录。所有判定来自真实 HTTP 请求、数据库查询或测试执行；静态结论一律标注来源，不冒充运行时结果。
> **评审收编（2026-11-04，专家）**：按评审完成三处精确化——① V-2 判定分维（策略/执行器机制 FAIL、真模型端到端 BLOCKED、DEF-02 整体 P0 FAIL）；② V-4 身份证据边界三分（运行时实证 / 静态证据 / 未验证）；③ 结论 7 关闭语义。另补：V-1 范围限定、DEF-02 修复方向精确化（独立人类确认约束，非通用红线）、V-6 上升架构约束、DEF-01 处理面扩展、缺陷独立关闭语义。全文其余判定不变。
> **本阶段未做**：业务代码修改（零改动）、数据库迁移（零变更，测试库独立）、正式设计裁决（留 S02-04）。
> **完成日期**：2026-11-04。

---

# §0. 核验环境（可复现）

| 项 | 值 |
|---|---|
| 代码基线 | worktree `D:\kudi-project\templates\ai-pm-s01`，`feat/s01-implementation` @ `807ddae`（需求域文件最后改动 `38742eb`，与镜像一致） |
| 被测服务 | 容器 `pm-api-s02v`（镜像 `aipm/pm-api:0.1.0`，2026-11-03 构建），`AI_PM_AUTH_MODE=off`，宿主端口 **8091** |
| 数据库 | 共享容器 `aipm-pg`（postgres:16）上**独立新建**库 `aipm_s02v`，迁移 0001–0028；与在跑演示栈（`aipm` 库）完全隔离，零污染 |
| 身份 | auth-off 注入固定 `dev-off` principal（`...000ff`，human），系统租户 `...0010`，租户 owner；**项目 A（`...b001`）成员（owner）**，**项目 B（`...b002`）非成员** |
| 种子数据 | customer `...a001`；项目 A/B；需求 `REQ-V1`（c001，项目A）、`REQ-V3`（c002，项目A，双 draft）、`REQ-V4`（c003，**项目B**）及各自修订 |
| 证据目录 | `D:\kudi-project\templates\.s02v-evidence\`（`v1.log` / `v3.log` / `v4.log` / `v6_rest_audit_shape.log` / `v2_policy_test.log` / `test_harness_probe.log`） |
| 复现方式 | 容器与库保留未删；`docker logs pm-api-s02v` + 证据目录内日志可重放全部步骤 |

**身份与证据边界（2026-11-04 专家评审口径）**：本次运行时身份为租户 owner + `is_admin=true`、项目 A 成员、**项目 B 非成员**。三个层次分开表述——

- **运行时实证**：租户内具有 owner/admin 身份、但**非目标项目成员**的主体，可以越权写入目标项目需求（V-4.3/4.4）。
- **静态证据**（代码路径分析）：写授权缺失成员校验是路径级事实——`requirement_in_tenant` 仅查 `tenant_id`（`pm_writes.rs:54-65`）；执行器路径无任何 membership 查询（V-4.5 夹具佐证）。
- **未验证**：其他具体身份类型（普通成员、其他角色、AI-Agent 主体经 REST/MCP 的授权表现）的运行时行为，应由后续权限测试覆盖。

V-4 判定综合前两者成立：**读侧有成员校验、写侧没有**。

---

# §1. 逐项核验记录

## V-1 REST 需求修订生命周期 —— **PASS**

| 字段 | 内容 |
|---|---|
| 核验编号 | V-1 |
| 核验目标 | 起草→定稿→ superseded→指针迁移→审计的完整 REST 链路是否真实可用 |
| 测试环境 | `pm-api-s02v` :8091（auth-off）；库 `aipm_s02v` |
| 前置数据 | 需求 c001（REQ-V1，项目A）：v1 approved 为当前版（种子），`row_revision=1` |
| 执行步骤 | ① `GET /v1/pm/requirements?project_id=A` ② `GET .../c001/revisions` ③ `POST .../c001/revisions`（body `{"content":{"markdown":"# v2 draft created at runtime"}}`）④ `POST .../c001/revisions/{v2}/approve` ⑤ 再查修订列表 ⑥ 查 requirement 行 ⑦ 查审计 |
| 预期结果 | 201 建 draft（version 自增）；200 approve；v1 转 superseded；指针指向 v2；两条审计 |
| 实际结果 | ③→**201**（`version=2`，`state=draft`，`content_hash=3d6e4f2f…`）；④→**200**（`state=approved`，`approved_by=...00ff`）；⑤ v2 approved + v1 **superseded**，total=2；⑥ `status='approved'`，`current_revision_id=7bdf6a7f…`（=v2），`row_revision=2`；⑦ `requirement.propose_revision` + `requirement.approve_revision` 各一条 |
| 数据库证据 | 见 v1.log 尾部 SQL 查询输出（requirement 行 + audit 行） |
| 日志证据 | `.s02v-evidence/v1.log`（全部 HTTP 请求/响应原文） |
| 判定 | **PASS**——状态转换符合约束、指针正确、历史查询与库一致、无重复定稿 |

**环境备注（非缺陷）**：步骤③首次尝试携带中文 JSON body 返回 400 `invalid unicode code point`——根因是 Git Bash curl 以 GBK 编码发出请求体，服务端**正确拒绝**了非法 UTF-8。改 ASCII body 后成功。此为测试工具编码问题，不是服务缺陷。

**范围限定（2026-11-04 专家评审）**：本项证明的是 REST 链路可运行（起草/审批/版本翻转/指针迁移/审计生成），**不**证明完整 Requirement 生命周期（拒绝/撤回/归档未复验，见结论 1 限定）。且本次 approve 由 dev-off（**human** 主体）完成——**V-1 不能用于证明 AI 定稿权限合理，也不能证明人类确认机制已强制执行**（后者是 V-2 / DEF-02 的对象）。

## V-2 AI 自动定稿与人工确认 —— **BLOCKED（真模型支路）/ 机制级 FAIL 证据成立**

| 字段 | 内容 |
|---|---|
| 核验编号 | V-2 |
| 核验目标 | AI 能否在没有人类确认的情况下完成需求定稿 |
| 测试环境 | 同上 + 测试执行环境（`cargo test`，DATABASE_URL 指向共享 PG 宿主端口 5435，`sqlx::test` 独立临时库） |
| 前置数据 | 策略文件 `policies/ai-pm-ai-lead-v2.yaml`（内嵌镜像）；执行器测试夹具（无任何 project_member 行） |
| 执行步骤 | ① 跑策略测试全量（10 例）② 跑既有执行器测试 `approve_requirement_revision_supersedes_old_and_compensates`（`ActorKind::AiPm` + `CommandSource::Bootstrap`）③ 查 `ai.llm_provider` 配置评估真模型可达性 |
| 预期结果 | （任务书：不能仅凭策略文件推断实际执行） |
| 实际结果 | ① **10/10 ok**，其中 `test_8_command_kind_unknown_to_policy_auto_applies` 运行时断言 `ApproveRequirementRevision → PolicyDecision::AutoApply` 且 `is_redline=false`；② 既有测试**实跑通过**（444.87s，全量迁移耗时所致）——**AI 主体执行定稿命令成功**，指针迁移 + 补偿规格落库（`CompensationSpec::Restore` 三条目）——该夹具**不含任何 project_member 行**，同时证明执行器无成员校验；③ 5 个 provider 全部 `enabled=false`、`api_key_ciphertext=\x454d505459`（字面 "EMPTY"）→ 真实模型链路不可用 |
| 数据库证据 | 临时测试库（sqlx::test 自建自毁）；llm_provider 查询见 §0 证据目录 |
| 日志证据 | `v2_policy_test.log`（test_8 输出）、`test_harness_probe.log`（444.87s ok） |
| 判定 | **按维度分列（2026-11-04 专家评审口径）**：① 策略机制 **FAIL**（运行时实证——`ApproveRequirementRevision → AutoApply` 且非红线）；② 执行器机制 **FAIL**（运行时实证——AiPm 主体执行定稿成功）；③ 真实模型端到端 **BLOCKED**（无可用 provider）；④ **DEF-02 整体判定为 P0 FAIL，不因③的 BLOCKED 而降为 BLOCKED**——机制级绕过已获运行时证据，且直接违反 S02-04 已确认的 Q-2 裁决方向（正式需求必须由人类完成确认、AI 不允许自动确认）及 S02-03 §5.3 两条防绕过规则 |
| 缺陷编号 | **DEF-02（P0 FAIL）** |

**按任务书口径的明确回答**：机制层面，**AI 可以在没有人类确认的情况下完成需求定稿**——策略判定（AutoApply）与执行器放行（AiPm 主体执行成功）均为运行时实证，非策略文件推断。端到端（真实模型产出提案→AutoApply 执行）因环境无模型配置记 BLOCKED；但链路上**不存在**任何会拦截它的闸门（V1.1 §7.1 五段静态证据链 + 本轮两段运行时证据）。

## V-3 并发定稿 —— **FAIL**

| 字段 | 内容 |
|---|---|
| 核验编号 | V-3 |
| 核验目标 | 并发定稿是否产生矛盾状态 / 静默覆盖 / 部分写入 |
| 测试环境 | 同 V-1 |
| 前置数据 | 需求 c002（REQ-V3）：v1、v2 两个 draft（种子），`row_revision=1` |
| 执行步骤 | 场景1：**并行** approve v1 与 v2；场景2：**并行**双 approve 同一修订 v3（运行时新建）；场景3：对已 superseded 的 v1 重复 approve；场景4：过期版本继续操作（API 无 `expected_revision` 字段，与场景1同型） |
| 预期结果 | （通过标准）不产生两个互相矛盾的当前版本；失败事务不留部分状态；**并发冲突有明确、可识别的错误结果** |
| 实际结果 | **场景1：两个请求双双 200**。终态：v1 superseded / v2 approved、指针→v2、`row_revision=3`（两次自增）、approve 审计两条。无任何一方收到冲突错误——第一个定稿人被**静默失效**。场景2：200 + 409（后者 `not a draft revision`，符合预期）；场景3：409（符合预期）；场景4：同场景1（无版本校验可谈） |
| 数据库证据 | v3.log 内三轮 SQL 输出（含终态：v3 approved、v1/v2 superseded、指针→v3、`row_revision=4`） |
| 日志证据 | `.s02v-evidence/v3.log` |
| 判定 | **FAIL**——通过标准第 3 条被违反：不同修订并发定稿双双成功、静默丢失更新、无可识别冲突结果。同修订并发与终态重试行为正确 |
| 缺陷编号 | **DEF-03（P1）** |

**机理对照**：与 V1.1 §3.1 的静态推断一致——REST 路径读修订状态无 `FOR UPDATE`，并发双方都读到目标仍为 draft、先后通过检查、交错写入；`row_revision` 只增不校。本轮为该推断的**运行时首次实证**。

## V-4 跨项目授权 —— **FAIL**

| 字段 | 内容 |
|---|---|
| 核验编号 | V-4 |
| 核验目标 | 无项目访问权限的成员能否读/写该项目需求；REST 与 Executor 授权边界是否一致 |
| 测试环境 | 同 V-1；身份 dev-off = 项目A 成员、**项目B 非成员**（同租户） |
| 前置数据 | 项目B + 需求 c003（REQ-V4）+ v1 draft（种子） |
| 执行步骤 | ① A 成员（即本身份）GET B 的需求列表 ② GET B 需求的修订列表 ③ POST 在 B 建草稿 ④ POST 定稿 B 的草稿 ⑤ Executor 侧成员校验检查（既有测试夹具证据） |
| 预期结果 | （通过标准）无权限成员不能读或改；写不能仅凭 tenant_id 通过；REST/Executor 边界一致 |
| 实际结果 | ① **404** `project not found`（读列表受保护 ✅）；② **200 但 `items:[]`**（修订列表有成员过滤，无泄漏 ✅，且与"未知需求返回空列表"的既有设计一致）；③ **201 建稿成功** ❌；④ **200 定稿成功** ❌——DB：REQ-V4 `status='approved'`、指针→v1、审计两条（propose + approve，actor=dev-off）。⑤ 既有执行器测试夹具**不含任何 project_member 行**而 approve 实跑成功（444s ok）→ **Executor 同样无成员校验** |
| 数据库证据 | v4.log 内 requirement 行 + 审计行查询输出 |
| 日志证据 | `.s02v-evidence/v4.log` |
| 判定 | **FAIL**——读侧有成员校验、写侧完全没有（`requirement_in_tenant` 仅查 tenant）；两条入口一致地"敞开"。租户内跨项目写越权成立（S02-01-033 的运行时实证） |
| 缺陷编号 | **DEF-01（P0）** |

**读/写不对称一览**：

| 操作 | 端点 | 校验 | 结果 |
|---|---|---|---|
| 读需求列表 | GET /v1/pm/requirements | 租户 + 成员（EXISTS） | 404 ✅ |
| 读修订列表 | GET …/revisions | 租户 + 成员（EXISTS） | 空列表 ✅ |
| 写修订 | POST …/revisions | **仅租户** | 201 ❌ |
| 定稿 | POST …/approve | **仅租户** | 200 ❌ |

## V-5 RequirementsPage.vue 前端行为 —— **BLOCKED**

| 字段 | 内容 |
|---|---|
| 核验编号 | V-5 |
| 核验目标 | 前端需求管理闭环（列表/修订/起草/定稿/刷新/错误态） |
| 测试环境 | 当前栈：仅 pm-api（jwks 模式，token-service 不在）+ PG + workers；无 lmms gateway / prue 容器 |
| 前置数据 | 需要可用登录链与 DB 菜单后端 |
| 执行步骤 | 环境评估：① 8091 auth-off 实例可绕过 API 鉴权，但前端路由守卫要求 `auth.authenticated`（`guard.ts:64-71`）+ DB 菜单装载；② 登录链依赖 lmms gateway（`.env.development` `VITE_IAM_BASE_URL=/gw` → :3000，未运行）；③ DB 菜单端点依赖 prue 容器（既有环境记录：无 prue → 503 circuit_open）；④ `VITE_USE_MOCK=1` 可起页面但数据全部来自 fixtures（`api/mock/fixtures.ts`），不构成对真实契约的运行时验证 |
| 预期结果 | —— |
| 实际结果 | 真实模式 E2E 无法在当前环境成立；mock 模式按核验原则第 1 条不采信 |
| 数据库/日志证据 | 无（未执行浏览器操作） |
| 判定 | **BLOCKED**——阻塞原因：登录链（gateway/token-service）与菜单后端（prue）在当前栈缺失；非 mock 模式下前端无法到达需求页 |
| 缺陷编号 | 无（环境阻塞，非产品缺陷） |

**契约层旁证（非运行时证据，仅记录）**：`frontend/src/api/pm.ts` 的四个客户端方法与 V-1 实测端点逐一对应——`listRequirements`→GET /v1/pm/requirements、`listRevisions`→GET …/revisions、`createRevision`→POST …/revisions（body 形态一致：`{"content":{"markdown"}}`）、`approveRevision`→POST …/{rev}/approve。请求/响应结构与 V-1 实测响应字段一一吻合。

## V-6 REST 与 Executor 双入口一致性 —— **FAIL**

| 字段 | 内容 |
|---|---|
| 核验编号 | V-6 |
| 核验目标 | 同一业务语义（需求定稿）经两个入口的权限/并发/审计/回滚差异；区分接口层差异与业务语义分叉 |
| 测试环境 | REST：`pm-api-s02v` 实测；Executor：既有测试实跑（V-2 步骤②） |
| 前置数据 | REST：需求 c001（V-1）；Executor：测试夹具 |
| 执行步骤 | 分别取两条路径的审计行与行为特征做对照 |
| 预期结果 | （任务书要求明确哪些是接口层差异、哪些导致业务语义不一致） |
| 实际结果 | 见下表 |
| 数据库证据 | `v6_rest_audit_shape.log`（REST 审计四列 + before/after + compensation 实查）；执行器侧断言见 `test_harness_probe.log` 对应测试 |
| 日志证据 | 同上 |
| 判定 | **FAIL**——存在业务语义级分叉（并发保护与可回滚性随入口不同） |
| 缺陷编号 | **DEF-04（P1）** |

**双入口对照表（全部为实测/实跑取得）**：

| 维度 | REST（实测） | Executor（实跑） | 差异级别 |
|---|---|---|---|
| 审计动作名 | `requirement.approve_revision` | `approve_requirement_revision` | 接口层（可观测性） |
| 审计载荷 | `before_data=NULL, after_data=NULL`（实测四列齐：actor_id/auth_mode='off'） | before/after JSON 快照 + 补偿规格 | 接口层（可观测性） |
| 补偿回滚 | `compensation_command IS NOT NULL → f`（**不可回滚**） | `CompensationSpec::Restore` 三条目（可回滚） | **业务语义** |
| 并发保护 | 无锁（V-3.1 双 200 实证） | `FOR UPDATE` 行锁 | **业务语义** |
| 成员授权 | 无成员校验（V-4 实证） | 无成员校验（V-4.5 实证） | 一致（共同缺失 → DEF-01） |
| 操作人记录 | 四列身份齐（auth_mode='off'，client 空） | `AuditActor`（ctx.actor_id） | 接口层 |

**上升为 S02 架构约束（2026-11-04 专家评审）**：当前存在两条都能改变 Requirement Revision 正式状态的写入路径（REST Revision Service 与 Command Executor），各自实现并发控制、审计、补偿与权限校验——今后每增加一条规则都有两套不一致的风险。故裁定方向：**Requirement 的所有正式业务写操作必须经过同一业务应用服务与统一状态迁移规则；REST、MCP、Executor 只作适配入口，不得独立实现需求生命周期规则**（与 S02-04 Requirement Application Service 方向一致；落 S02-04 裁决 / S02-05 I-Slice）。

---

# §2. 缺陷清单

| 编号 | 严重度 | 缺陷描述 | 复现条件 | 影响范围 | 相关代码位置 | 建议处理方向 |
|---|---|---|---|---|---|---|
| **DEF-01** | **P0** | 需求写路径无项目成员校验：同租户非项目成员可创建并定稿任意项目的需求修订（读侧有校验、写侧没有） | 租户内任一已认证主体 + 目标项目需求 ID；`POST /v1/pm/requirements/{id}/revisions`（201）→ `POST .../approve`（200）；V-4.3/4.4 实录（身份=租户 owner/admin、目标项目非成员；其他角色待后续权限测试覆盖） | 全部项目的需求写完整性；与 I2 V1.1 G9/G10 同型 | `apps/pm-api/src/pm_writes.rs:54-65`（`requirement_in_tenant` 仅查 tenant）；执行器路径无 membership 查询（既有测试夹具无成员行仍成功） | **不止补一处查询——统一 REST/Executor 项目授权**，覆盖六点：项目成员角色校验 / 需求创建权限 / Revision 修改权限 / Revision 审批权限 / AI-Agent 作为执行主体时的项目授权 / 双入口授权一致性（I2-D 铁律）→ Q-9 |
| **DEF-02** | **P0 FAIL** | 需求定稿无人类确认强制：策略黑名单模型下 `approve_requirement_revision` 非红线 → AutoApply；执行器接受 AI 主体执行 | 策略测试 `test_8` 运行时断言 AutoApply；执行器测试以 `ActorKind::AiPm` 实跑通过。真实模型端到端 BLOCKED（5 个 provider 全部 `enabled=false`、key EMPTY）——**不影响本缺陷 FAIL 判定** | 「经过人类确认的需求基线」这一 S02 核心结果不成立；违反 S02-04 Q-2 裁决方向 | `policies/ai-pm-ai-lead-v2.yaml:16-39`（名单缺项）；`policy_engine.rs:316-321`；`executor.rs:52-56`（仅红线才拦非人类）；`openai_adapter.rs:31`（prompt 列其为合法命令） | **正式 Revision 的 approve 命令必须具备独立的人类确认约束，不得仅依赖通用 redline 黑名单**——红线策略与正式需求审批不是同一概念，需求审批的安全性不能寄托在策略 YAML 名单；`approved_by` 写入主体必须为真人主体 → Q-2；与 DEF-01 同批收双入口 |
| **DEF-03** | **P1** | 不同修订并发定稿双双 200：静默丢失更新，先定稿方被无声失效，无冲突错误 | 对同一需求的两个 draft 并发 approve（V-3.1 实录：双 200，`row_revision` 1→3，两条审计，无错误响应） | 数据一致性；审计可信度（成功日志与最终状态矛盾） | `pm_write/revision.rs:77-140`（读修订无 `FOR UPDATE`）；API 无 `expected_revision` 字段（`pm_write/mod.rs:109-112`） | 行锁或乐观并发校验；与 Q-1（权威路径裁决）一并收敛 |
| **DEF-04** | **P1** | 双入口业务语义分叉：同一"定稿"语义，REST 不可回滚/无锁/审计无快照，Executor 可回滚/有锁/审计带快照；审计动作词表两套 | 对同一操作分别走两入口并对比审计行（V-6 对照表，全部实测） | 依据入口不同获得不同等级的一致性保证；审计追溯口径分裂 | `pm_write/revision.rs` vs `command_executor/executor.rs:809-971` | 裁决唯一权威路径，另一条收敛或降级为薄封装（Q-1） |

**分级说明**：P0=权限或人工确认绕过（任务书定义）；P1=数据一致性或并发；本阶段无 P2/P3 实证项。V-1 步骤③的 400 编码拒绝为测试工具问题，不入缺陷清单。**DEF-01、DEF-02 为 S02 P0 阻塞项（专家 2026-11-04 确认）：不得为推进后续设计而降级；S02-04 最终基线的通过条件必须包含二者的约束性修复裁决（见 §3 结论 7）。**
**关闭语义（专家 2026-11-04）**：四项缺陷**相互独立关闭**——P0 修复不使 P1 自动解决；统一入口（DEF-04）的裁决不自动解决并发缺陷（DEF-03 是 REST 路径实际缺陷、DEF-04 是双入口未统一保证，二者各有主体）；**关闭 = 后续修复提交 + 针对性回归证据**，本阶段不热修代码（任务书约束）不构成任何一项的关闭或降级。

---

# §3. 核验结论（任务书七问）

1. **当前需求生命周期是否真实可用？**
   核心链路真实可用：起草→定稿→版本翻转→指针迁移→审计，端到端无阻断（V-1 PASS）。但可用性伴随三项缺陷：写侧越权（DEF-01）、无人工确认（DEF-02）、并发不安全（DEF-03）；且拒绝/撤回/归档路径不存在（静态结论，S02-01 §3.4，本轮未复验）。

2. **AI 是否能够绕过人工确认定稿？**
   **机制层面：能**（运行时实证——策略 AutoApply + AI 主体执行成功，非文件推断）。真实模型端到端：BLOCKED（无模型配置），但链路上无任何闸门。

3. **是否存在跨项目越权？**
   **存在（P0，已实证）**：非项目成员可创建（201）并定稿（200）其他项目的需求；读侧有校验、写侧没有。REST 与 Executor 一致地缺失成员校验。

4. **并发定稿是否安全？**
   **不安全（不同修订并发）**：双双 200、静默丢失更新（V-3.1）。同修订并发与终态重试有 409 保护。无乐观并发校验（`row_revision` 只写不校、API 无 `expected_revision`）。

5. **REST 与 Executor 是否存在业务语义分叉？**
   **存在**：并发保护（无锁 vs 行锁）与可回滚性（不可 vs 可补偿）为**业务语义级**分叉；审计词汇与载荷粒度为**接口层**分叉。成员授权两侧一致（共同缺失）。

6. **前端是否真正具备需求管理闭环？**
   运行时 E2E **BLOCKED**（登录链与菜单后端缺失）。契约层与实测端点一一吻合（旁证），页面与四个客户端方法存在。闭环成立与否待环境补齐后核验。

7. **哪些问题必须在 S02 设计前解决？**
   按 S02-02 任务书，本阶段不修改代码。**DEF-01、DEF-02 是 S02-04 最终设计基线必须作出强制修复裁决的 P0 项**（→ Q-9 授权模型 / Q-2 人类确认约束）；**DEF-03、DEF-04 是必须纳入 S02-05 / S02-06 的 P1 项**（→ Q-1 权威路径 + 统一并发控制 + 统一业务应用服务）。**所有四项均不得因本阶段未实施修复而视为关闭；关闭状态必须由后续修复提交及针对性回归证据确认。**本阶段不热修代码是任务书约束（只核验不修复），不构成对任何缺陷的降级或关闭。

---

# §4. 完成条件自查

- [x] V-1~V-6 均有明确判定：PASS ×1（V-1）、FAIL ×3（V-3 / V-4 / V-6）、BLOCKED ×2（V-2 真模型支路 / V-5）
- [x] 所有 FAIL 均有可重复证据（HTTP 日志 + SQL 输出 + 测试名与断言）
- [x] 所有 BLOCKED 均说明阻塞原因（V-2：无模型配置；V-5：登录链与菜单后端缺失）
- [x] 静态结论与运行时结果全程分离标注（V-1.1 §14.4 口径延续）
- [x] 零业务代码修改、零迁移变更（测试用独立库 `aipm_s02v`；`cargo test` 用 sqlx::test 临时库）
- [x] 本报告提交后等待 S02 评审，不自动进入实施

**遗留物**：容器 `pm-api-s02v`、库 `aipm_s02v`、证据目录 `.s02v-evidence/` 保留以便复现；评审结束后可清理（`docker rm -f pm-api-s02v` + `DROP DATABASE aipm_s02v`）。
