# ai-pm 后端重构体检与任务书 V1

- 分支：`refactor/backend`（worktree `.worktrees/refactor-backend`）
- 基线：master `69c91724`，nextest fast profile **1507 passed / 4 skipped（322s）**
- 日期：2026-11-11
- 状态：已裁决（2026-11-11）：**D1=真仓储层；D2=全做 A–G；D3=全量+门禁**

## 1. 体检结论

### 1.1 规模分布（后端 Rust 共 ~199k loc）

| crate/app | 文件 | loc | 占比 | 判断 |
|---|---|---|---|---|
| pm-application | 233 | 106,587 | 53% | 事实上的「应用层+DAL」合体，god-crate |
| pm-api | 121 | 47,380 | 24% | 37/57 handler 文件直写 SQL |
| pm-ai | 33 | 18,458 | 9% | discovery 线集中，有双份常量/重复 HTTP 调用 |
| pm-infrastructure | 35 | 12,863 | 6% | 不是仓储层，是 trait 适配器+后台件 |
| ai-pm-runner | 9 | 3,131 | 2% | 独立 PC 客户端，零服务端依赖，干净 |
| pm-domain | 18 | 2,724 | 1% | 仅核心状态机（TaskStatus/allowed_edges），贫血 |
| pm-mcp | 9 | 5,274 | 3% | tools.rs 1838 行装 24 工具，但解耦良好 |
| kb-pc-contracts | 9 | 2,333 | 1% | 契约 crate，零内部依赖，被 4 方共享，干净 |

### 1.2 核心问题：分层泄漏（不是想象的三层）

1. **pm-api 直写 SQL 普遍**：57 个 handler 文件中 37 个含 `sqlx::query*`，handler 直接拿 `State(pool)` 查库。最重：`pm_lists.rs`（19 处，:113 起多处手写 LIMIT/OFFSET）、`pm_baseline.rs`（18）、`pm_cognition.rs`（15）、`auth.rs`（14）、`tenant_members.rs`（13）。
2. **pm-application = 事实 DAL**：117 个文件持 `PgPool`，约 1325 处 `sqlx::query`（已实证）。`Cargo.toml:35` 还依赖 pm-infrastructure。
3. **pm-infrastructure 名不副实**：实为 outbox/docproc/kb_pc 事件后台件 + trait 适配实现（`ArtifactStore` 等），自身也持 pool 直写 SQL（`kb_pc_handlers.rs`）。
4. **已统一的部分**（保留）：`apps/pm-api/src/support.rs` 的 `ApiError`/`Paging`，中间件 `AuthedPrincipal` + handler 统一 `Extension(caller)`。
5. **pm-domain 是真守卫但覆盖窄**：Task 状态流转有 domain 校验（`task_transition.rs:32` `allowed_edges()`，调用点 `pm_write/task.rs:198` 等）；其余不变量散落 application。

### 1.3 重复模式 Top5（均已 grep 实证）

| # | 模式 | 位置 |
|---|---|---|
| 1 | 修订生命周期四件套（lock/fetch_revision/content_hash_of/transition_or_conflict）逐行同构 | design.rs:701-844 ≈ task_plan.rs:154-307；requirement.rs:270 第三份 lock |
| 2 | `tenant_audit_log` INSERT 样板 | src 4 份：team_llm.rs:298、team_onboarding.rs:271、team_llm_catalog.rs:153、device_credential.rs:159；**pm-api 还有 2 份**：device_claim.rs、tenant_members.rs（handler 越权写审计） |
| 3 | 审批请求流（begin→lock→校验→insert approval→audit） | design.rs:1489、design.rs:1541、task_plan.rs:678 |
| 4 | `content_hash` 独立实现 | design.rs:814、task_plan.rs:208、knowledge.rs:701、kb_service/memory.rs:249、policy_composition.rs:105 |
| 5 | 测试夹具 | `seed_project` 在 16 个 tests 文件各写一份；`INSERT INTO core.project` 手写 26 处；无 tests/common |

另：pm-ai 内 **prompt 预算常量双份**（`discovery_context.rs:61` 与 `discovery_orchestrator.rs:95` 各定 `36_000`+各一份 DROP_ORDER）、**6 处角色各自手写 reqwest+/chat/completions**（repo_model.rs:177、snapshot_model.rs:272、conversation_model.rs:112、writeback_model.rs:191、conversation_router_model.rs:86、retrieval_planner.rs:63）、prompt 常量散落 7 文件 9 处。

### 1.4 巨型文件与函数

| 文件 | 行数 | 混杂的域 |
|---|---|---|
| pm-application/src/design.rs | 3212 | 设计 CRUD + ADR 全生命周期 + Baseline 冻结/废弃 + 审批裁决 + readiness 门禁 + 聚合读，≥5 域 |
| command_center.rs | 1594 | 仪表盘把 8 个域查询内联（load_risk_summary 173 行手工 SQL） |
| team_llm.rs (+catalog) | 1284+1074 | CRUD + AES-GCM 加密(:156-199) + SSRF 校验(:228-266) + HTTP 探活(:791) |
| intake.rs | 1177 | 提交流程 + 任务状态机(:492-568) + 信号路由(:633) |
| task_plan.rs | 1385 | 计划 + 修订四件套副本 + 审批 |

>200 行函数（拆解优先级）：`pc/proposal.rs:448` create_proposal_in（268 行）、`command_center.rs:648` load_initialization_progress（241）、`design.rs:1841` decide_approval（238）、`design.rs:2901` freeze_baseline（234）、`design.rs:469` evaluate_readiness（236）、`run_executor.rs:43` claim_and_start（207）。

### 1.5 错误处理：类型化但碎片化

- 45+ 个模块级 `enum Error`，无 anyhow——方向对，代价是样板：design.rs 单文件 130 处 `map_err(PmWriteError::Db)`；跨域错误靠手写 match 翻译（kb_service/memory.rs:623-636）；device_credential.rs 用 `Db(e.to_string())` 丢类型。

### 1.6 测试组织

- tests/ 60 文件 ~3.7 万行，**477 个 `#[sqlx::test]` 全走真库，零 mock**——这是重构最大的安全网。
- 无 tests/common 夹具库（见 1.3 #5）。

### 1.7 杂项发现

- **master 上 `bash scripts/clippy.sh --all` 是红的**：`-D warnings` 下 pm-application lib 6 处先挡死。全量画像（不判死）：全仓共 ~25 处——type_complexity ×5（pm-application lib 为主）、unused variable ×5、never-read field ×3、`.err().expect()` ×2、其余 singles；pm-api lib 2 处，余散在 tests。多数可 `cargo clippy --fix` 批量处理。（clippy.sh 是门禁脚本，说明全量 lint 不在常用门禁路径上。）
- `services/notification-service`、`services/token-service` 只有 config；`docker-compose-services.yml:153,260` 引用的 Dockerfile 不存在——坏占位。
- 依赖方向总体健康：pm-ai→pm-application、pm-infrastructure→pm-application（注释明示倒置）、pm-mcp→pm-application，无生产双向环（application↔ai 仅 dev 测试环）。

## 2. 重构原则

1. **行为不变是默认约束**：477 个真库用例是安全网；每批结束 `bash scripts/test.sh` 全绿。
2. **不写兼容层/过渡层**（AGENTS.md #1）：直接挪、直接改契约。
3. **小步提交**：每批 1~N 个独立 commit，批内每步可编译可测。
4. **门禁固化防回潮**：如批 B 完成后加「pm-api/src 禁 sqlx::query」grep 门禁进 release-guard 或 CI 脚本。

## 3. 批次提案

### 批 C 执行记录（✅ C-1~C-3 已完成 2026-11-12，commit a3995ac2 + f163e4fd + 628a8b12）

| 刀 | 文件 | 结果 |
|---|---|---|
| C-1 | design.rs 3207 行 | design/{mod,readiness,technical,finding,approval,adr,baseline} 7 文件（最大 734）；对外路径不变 |
| C-2 | command_center.rs 1594 行 | {mod,project,work,action,risk,activity} 6 文件；8 个 load_* 按 §4.x 归 5 子模块 |
| C-3 | intake.rs 1177 行 | {mod,task,signal,read} 4 文件（P3-3 状态机/RT 路由/P3-4 聚合读分离） |

- **守卫真实拦截 ×2**：C-2 拆分触发 ADR-06a 容器守卫（白名单单文件条目失效），按守卫语义收敛为 `command_center/project.rs`（两表引用唯一落点）；C174/C180 审计守卫在批 A 同样拦截并按单点归并收敛清单。守卫机制工作正常。
- **挂账**：V1.1 §4.7 使用方清单回填（守卫注释指向的文档条目未能定位，未擅自改契约文档）。
- **team_llm.rs（1277 行）判定不拆**：混杂是叶子助手级（加密 44/校验 93/探活 90 行，被 CRUD 调用），拆出后主文件仍 ~1000 行且 CRUD 本身职责单一——收益不抵 churn。留待批 B 触及时随域处理。
- 其余 1000+ 行文件（pc/proposal 1580、kb_service/memory 1196、run_executor 1116、requirement 1110、task_read 1046、run_dispatch/claim 1043、delivery/mod 1017、project_import/lifecycle 1013）：转为持续纪律——批 B 逐域迁移时随触随拆，不设独立批次。
- 批 C 每刀验证：workspace 零警告 + clippy --all PASS + 针对性/全量测试绿。

| 批 | 内容 | 风险 | 预估规模 |
|---|---|---|---|
| **F0** | 修绿 clippy --all（type_complexity ≥6 处，建 type 别名/拆结构体） | 低 | 小 |
| **A** | 共享基建归并：修订四件套→共享模块；tenant_audit 助手；content_hash 归一；审批流模板；tests/common 夹具库 | 低 | 中 |
| **C** | 巨型文件拆分：design.rs→design/{crud,adr,baseline,approval,readiness}；team_llm 拆 crypto/ssrf/probe；intake 拆流程/状态机/路由；6 个 200+ 行函数拆解 | 低（纯挪动） | 中 |
| **B** | pm-api 数据访问收口：37 个 handler 文件直写 SQL 下沉 pm-application（按域建 read 模块），先收 top5（pm_lists/pm_baseline/pm_cognition/auth/tenant_members），完成后加 grep 门禁 | 中 | 大 |
| **D** | pm-ai 收敛：6 处 HTTP 调用统一走 OpenAiCompatibleModel；预算常量单一真源；prompt 常量集中 | 中 | 中 |
| **E** | 错误处理收敛：`From<sqlx::Error>` impl 消灭 map_err 海（design.rs 130 处起步）；跨域错误统一翻译 | 低 | 大（机械） |
| **G** | 清理：services/ 坏占位处置（compose 引用缺失 Dockerfile）；pm-infrastructure 定位重命名或文档澄清 | 低 | 小 |

推荐顺序：**F0 → A → C → B → D**，E/G 随行穿插。A 在 C 前面：先把四件套归并，design.rs 拆分时顺手少搬一份副本。

### 批 A 明细（✅ 已完成 2026-11-12，commit 6e3156f0 + beb8e535）

实施记录（与原计划的偏差已标注）：

1. ~~`pm-application/src/support/revision.rs`~~ → **实际落点 `pm_write/support.rs`**（`hex_sha256` 已在此，三域 import 路径零新增）：`is_unique_violation`/`content_hash_of`/`transition_or_conflict`（泛型版）/`next_code` 单一实现；`lock_head_row`/`lock_revision_row` 通用行锁（三域锁函数留 1 行转发壳，调用点零改动）；requirement_revision 行锁顺带补租户过滤。全量 1507 绿。
2. **A2 完成**：`insert_tenant_audit`（target_type 参数化）归并 pm-write 单点 + 4 转发壳。**pm-api 侧 2 处（device_claim/tenant_members）形态不同**——写 `target_principal_id` 而非 `target_type/target_id`，actor 是 pm-api 本地类型，归批 B 随域收口。审计守卫 C174/C180 清单同步收敛（4 项撤销、pm_write/support.rs 登记 1 项）。
3. **A3 完成**：knowledge/memory/policy_composition 三处手写 digest→hex 循环换 `pm_write::hex_sha256`（digest 层单源，各自业务规范化保留）；删 3 份 sha2 导入 + policy_composition 本地 hex helper。
4. **A4 完成**：`insert_design_approval`（design.rs，pub(crate)）——三域同构 INSERT + pending 唯一冲突映射单源；前置校验与尾部 commit/读视图留调用方。
5. **A5 部分完成**：`seed_project` 实测散布 **43 个测试文件**（原体检低估为 16）。新建 `crates/pm-application/tests/common/mod.rs`（`bootstrap_system_project` + `ensure_tenant_member`），转换 pm-application 侧 7 个最简形文件；`retrieval_negative_matrix`/`retrieval_traversal` 是自建 principal 变体、pm-api 侧已有自己的 common——其余 30+ 文件随批 B/C 触及时顺手收编。

**批 A 验证**：clippy --all PASS + 全量 1507 passed / 4 skipped ×2 轮。

## 4. 决策点（已裁决）

- **D1｜批 B 的归宿 = 真仓储层**：重建 pm-infrastructure 为 trait+实现的仓储层，SQL 从 pm-application（1325 处）与 pm-api（37 文件）全量迁入。
  - 分层契约：**trait（port）放 pm-domain 或 pm-application，impl 放 pm-infrastructure**——沿用既有先例（application 定义 `ArtifactStore`，infrastructure/docproc_store.rs 实现）。
  - 实施方式：按聚合/域建仓储（如 ProjectRepo/TaskRepo/DesignRepo/…），逐域迁移，每域一批全绿提交；不做大爆炸。
- **D2｜范围 = 全做 A–G**，顺序 F0→A→C→B→D，E/G 随行穿插。
- **D3｜批 B 覆盖面 = 全量+门禁**：pm-api 37 文件全收口，完成后加「pm-api/src 禁 sqlx::query、pm-application/src 禁 sqlx::query」grep 门禁防回潮。

### 批 B 实施细化（按 D1 裁决展开）

1. **先立骨架**：pm-infrastructure 建 `repo/` 模块与 `PgRepo` 公共基建（连接、分页、错误映射 sqlx::Error→RepoError）；pm-domain 定义核心聚合 trait，复杂读模型 trait 放 pm-application。
2. **迁移顺序**（按 handler/服务热度）：pm_lists → pm_baseline → pm_cognition → auth → tenant_members → 其余 32 文件；pm-application 侧随批 C 拆分时同步迁移该域。
3. **纪律**：每域迁移 = trait + impl + 调用方改接 + 该域测试全绿 + commit；迁移完成的模块立即删原 SQL（无兼容层，AGENTS.md #1）。
4. **完成门禁**：`grep -r "sqlx::query" apps/pm-api/src crates/pm-application/src` 计数为 0，固化进 CI/release-guard。

### 批 B 执行记录（骨架 + 首域 ✅ 2026-11-12，commit 92858396）

**骨架落地**（与原计划的偏差）：
- port 放 `pm-application/src/repo/`（trait + 复用 `crate::rows` 行类型；pm-domain 不掺和——行 DTO 全在 application，动 pm-domain 只会多一层搬运）；impl 放 `pm-infrastructure/src/repo_lists.rs`（PgPmListRepo，`#[derive(Clone)]` 持 PgPool——PgPool 内部是 Arc，无生命周期体操）。
- **错误首版直接 `sqlx::Error`**：不预设 RepoError——ApiError 已有 From 映射，语义按方法名表达（find_*_row vs list_*）；需要 NotFound/Conflict 的域迁移时逐步引入。
- 分页：仓储侧 `ListPage { size, offset }`，HTTP 层 Paging 到 ListPage 的转换留在 handler（一层解耦）。
- async-trait（仓库既有依赖），非 AFIT——保持 dyn 兼容余地。

**首域 pm_lists**：`PmListRepo` trait（8 列表 + revisions + set_project_description + find_project_row）+ PgPmListRepo 实现；pm-api handler 改接仓储，**22 处 SQL 归零**，SQL 文本原样迁移。`principals` 端点的平台级语义（无租户过滤）原样保留。

**守卫战果 ×3**：C-2 ADR-06a 容器守卫、批 A C174/C180 审计守卫、C-3 后审计守卫（intake 路径，拆目录时只跑了针对性测试漏网、全量时被守卫抓住）——源码级守卫体系持续有效，教训：**拆文件类改动必须过全量**（守卫按路径扫描）。

**进度（收尾盘点 2026-11-12，域 12 后）**：pm-api 37 文件 SQL 已收口 **26/37**（pm_lists 22 + pm_baseline 18 + pm_cognition 15 + auth 14 + tenant_members 13 + pm_runs 11 + knowledge_services 9 + pm_designs 8 + ai_team_monitor 8 + customers 6 + 守卫合并两波 20+7 = 151 处归零）。剩余 **11 文件 21 处**：

- 验收/工件读列表 6 处：pm_acceptance(3) + pm_artifacts(2) + pm_reviews(1) → AcceptanceReadRepo（新 port）
- 审计 INSERT 3 处：intakes(1) + device_claim(1) + outbox_ops(1) → 迁仓储时**必须同步改 C174/C180 守卫清单**
- 管理面 4 处：principals(1 UPDATE) + admin_config(2 embedding 配置) + team_llm_guard(1) → AdminRepo
- docproc 3 处：文档存在性/版本链 → DocprocGuardRepo
- support 横切 2 处：require_project_visible 底层 EXISTS + task→project_id（project_visible 被全站 require_project_visible 依赖，收口收益低、风险高，**可判定保留**）

**仓储体系现状**（pm-application/src/repo/ 12 个 port + pm-infrastructure/repo_* 12 个实现）：
PmListRepo / BaselineRepo / CognitionRepo / IdentityRepo / MemberRepo / RunRepo / KbPcQueryRepo / DesignGuardRepo / AiTeamMonitorRepo / CustomerRepo / GuardQueryRepo + AiTeamMonitorRepo（行类型）。
构造器统一走 `pm-api/src/support.rs::repos`（handler 只依赖 trait）。

**错误分层定案**：纯读域 = sqlx::Error（ApiError::Internal）；写事务域 = 域错误枚举（Db/Conflict/NotFound/RuleRejected），HTTP 层三行映射。错误码契约（OWNER_LAST 等）经测试锁定。

**守卫战果（批 B 累计）**：审计守卫 ×2（intake 路径、tenant_members 路径——均为拆迁后清单随迁）、ADR-06a 容器守卫 ×1（documents → repo_guard_query）。

**✅ 批 D 完成收窄版（2026-11-12，d9b37e22）**：体检报告的「6 处手写 HTTP」实测仅 2 处残留（writeback/router/retrieval 此前已复用 conversation_model::complete）。抽 complete_raw（单次无重试，temperature 参数化）为全 crate HTTP 单源，repo/snapshot 两处迁移（temperature=0 保持原确定性值；各自 ModelError 别名 match 翻译）；complete 在其外包 retry 循环。TOTAL_PROMPT_CHARS 数值单源化；两份 DROP_ORDER 语义对齐由新测试锁定。pm-ai 171 用例全绿。

**✅ 批 G 完成（e0e4f596）**：compose 两服务 build 段（引用不存在的 Dockerfile）移除，services/README.md 澄清定位。compose config 语法验证通过。

**批 E（错误收敛）与 pm-application 侧 1325 处 SQL 迁移**：移交后续会话——样板已齐（repo port/impl/handler 改接/错误分层/守卫清单同步五件套），按任务书纪律逐域推进。

**✅ 批 B pm-api 全清（2026-11-12，收尾批 03688633）**：37 文件 179 处 → **1 处判定保留**（support.rs 的 project_visible EXISTS——全站 require_project_visible 底层，豁免上限 1）。收尾批新增 AcceptanceReadRepo（验收读 6 处，4 行类型上移）与 PlatformOpsRepo（admin/docproc/杂项守卫/审计 INSERT 共 14 处）；三处审计 INSERT 迁仓储时 C174/C180 守卫清单同步合并。门禁固化：**scripts/guard-pm-api-sql.sh**（grep 计数 + support.rs 豁免上限 1，违规非零退出；纳入验收门禁）。仓储体系终态：pm-application/src/repo/ 14 个 port ↔ pm-infrastructure/repo_* 14 个实现，构造器统一 support::repos。验证：workspace 零警告 + clippy PASS + 全量 1507 passed + 守卫 10/10。

**操作事故记录**：收尾盘点时误执行 `git checkout master -- .` 污染工作区（master 的 viz 前端文件混入 index），已 `git checkout HEAD -- . && git reset --hard HEAD` 完整恢复并重跑全量验证。教训：**统计类操作禁止用 checkout 检出对比**，用 `git show branch:path` 或 `git diff branch --stat`。

（pm_lists 22 处 + pm_baseline 18 处归零）。首域样板（读模型 trait，错误=sqlx::Error）；第二域升级出**写事务域样板**：`BaselineRepoError { Db, Conflict, NotFound }` 域语义错误在仓储事务内翻译，HTTP 层只映射状态码（`repo_err()` 三行映射）+ 保留授权门。后续域照此二分：纯读域用 sqlx::Error，有唯一性/状态语义的写域用域错误枚举。剩余：pm_cognition(15) → auth(14) → tenant_members(13) → 其余 32 文件。

## 5. 验收门禁（每批必过）

```bash
bash scripts/test.sh          # 1507 全绿
bash scripts/clippy.sh --all  # F0 之后须全绿
cargo fmt --check             # 归一（注意 cargo fmt 波及无关文件，提交前 diff 审查）
```
