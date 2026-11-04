# S01 代码事实与差异台账 V1.0

> 文档性质：S01 正式交付体系第六份文件（`docs/s01-implementation-baseline-v1.md` §9）
> 职责：**代码事实与差异台账**——记录「设计 / 基线要求」与「真实代码、Migration、调用链」的对照结论
> 核验等级：沿用基线 §3.2 —— `[FACT]` / `[PARTIAL]` / `[MISSING]` / `[DEFECT]` / `[NEW]` / `[DECIDED]`
> 核验基准：仓库 `ai-pm`，分支 `feat/s01-implementation`（基线 master `aaee6e5` + S01 批次），Migration 0001～0031
> 版本：V1.0　日期：2026-11-03

---

## 0. 使用规则

1. 本台账**只记录事实与差异**，不重复设计内容；设计以专项设计稿为准。
2. 每条事实带证据（文件:行号 / Migration 号 / 端点 / 工具名）。
3. 发现与基线或设计冲突 → 登记到 §10 差异台账，并按基线 §1 的优先级顺序处理：
   用户裁决 > ADR > 专项设计稿与专家裁决 > 代码事实 > 正式基线 > 候选建议。
4. `[DEFECT]` 条目必须落到 TASK（`docs/s01-task-registry-v1.md`）并有验收条目。
5. 本台账**不自动成为裁决**：实施方不得据本台账自行改变已冻结设计（基线 §10）。

---

## 1. 身份、租户与主体

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 1.1 | `core.principal` 为全局共享表，无 `tenant_id`；`kind ∈ {human, dev_agent, ai_pm, service}` | `[FACT]` | `migrations/0001_core.sql:31-36` |
| 1.2 | `core.principal_identity` 为 JWT → 真实主体映射；`UNIQUE (provider, tenant_id, subject)` | `[FACT]` | `0018_principal_identity.sql:10-26` |
| 1.3 | 租户成员是唯一授权事实源：每次请求查库 `core.tenant_member.role`，不读 token claims | `[FACT]` | `apps/pm-api/src/auth.rs:395-416` |
| 1.4 | 首次请求即 provision：非首管主体建成 **inactive**，需 owner 显式激活 | `[FACT]` | `auth.rs:472-520` |
| 1.5 | 三层身份（HumanActor / DeviceClaim / CallerContext）生产点唯一、消费点只读 | `[FACT]` | `crates/pm-application/src/caller.rs:1-28` |
| 1.6 | Device ID 仅作归因，**不进入** `principal_id` / `identity_id`，不影响授权结果 | `[DECIDED]` | Codex Bootstrap V1.3 §5.1.1（`docs/ai-pm-codex-bootstrap-v1.md`） |
| 1.7 | 租户成员管理齐备（增删改、角色变更、owner 转移、审计） | `[FACT]` | `apps/pm-api/src/tenant_members.rs`（6 端点） |
| 1.8 | Project 成员此前**只读**（`core.project_member` 全仓零写入路径） | `[DEFECT]` | 已由 S01-W13 修复（`pm-application/src/project_member.rs`） |
| 1.9 | 租户撤权此前**不级联**到项目成员（退出租户仍可访问其项目） | `[DEFECT]` | 已由 S01-W13 修复（级联撤销 + `PROJECT_OWNER_DEPENDENCY`） |

## 2. Project、成员与角色

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 2.1 | `core.project` 具备 `UNIQUE (tenant_id, id)`（0019），是所有组合 FK 的父键锚点 | `[FACT]` | `0019_tenant.sql:155` |
| 2.2 | Project 成员角色**复用同一列**承载两类语义：真人角色（owner/member）与 Agent 职能（ai_pm/architect/developer/tester） | `[FACT]` | `0001_core.sql:41-46`；`bootstrap.rs:282-286` |
| 2.3 | PK 为 `(project_id, principal_id)` → **一人一 role** | `[FACT]` | `0001_core.sql:45` |
| 2.4 | `role` 值域原先无 CHECK；已由迁移 `0030` 收敛为 6 值白名单 | `[FACT]` | `0030_project_member_role.sql` |
| 2.5 | 真人 Project 角色只两档（owner/member）；`owner` 只能通过所有权转移获得 | `[DECIDED]` | S01-W13 设计稿 §2.1 |
| 2.6 | 无「单项目详情」GET 端点（仅列表 + 创建） | `[MISSING]` | `apps/pm-api/src/pm_lists.rs:83` |
| 2.7 | `core.project.status` 消费点仅 `initializing` 一处 | `[PARTIAL]` | `lifecycle.rs` start_import；归档语义属 I1-D |

## 3. Repository 与 Git 能力

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 3.1 | Repository 是独立实体，`project_repository` 为项目↔仓库关联表 | `[FACT]` | `0008_repositories.sql:18-33` |
| 3.2 | `project_repository.role ∈ {main, docs, service, infra}` 是**仓库类型标签**，不是权限等级 | `[FACT]` | `0008_repositories.sql:23-24` |
| 3.3 | `project_repository` 无 `(tenant_id, project_id, id)` 组合唯一键 → 已由 `0031` 补 | `[FACT]` | `0031_s01_discovery_import_v2.sql` 第 0 项 |
| 3.4 | `task_repository.write_access` 是**布尔**；I3 V1.1 简化后**不再决定访问档位**（仅作调度前置与交付规则输入） | `[FACT]` → 已改判 | `0011_run_dispatch.sql:90`；I3 设计稿 §0.0.5 |
| 3.5 | **仓库级 Git 能力授权零实现 → 2026-11-04 简化裁决后不再需要**：授权表 `integration.git_capability_grant` 由 `0032` 建、由 `0033` 撤下；判定层改为单一 `read_write` 档位 | `[DEFECT]` → 已闭合（简化） | `0032` / `0033`；`pm-domain::repo_access` |
| 3.6 | Git 访问层只允许只读命令（`ls-remote` / `clone` / `log` / `show` / `ls-tree`），**物理上无法 push/commit** | `[FACT]` | `crates/pm-application/src/git_access.rs:1-4` |
| 3.7 | 仓库删除原为 CASCADE（历史引用不保护）；已由 `0031` 换轨 RESTRICT | `[DEFECT]` → 已修 | `0031` 第 11 项 |

## 4. Import / Source / Attempt / 草案

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 4.1 | `repo_import` 10 态 CHECK；`project_id` 已有 | `[FACT]` | `0009_repo_import.sql:31-34` |
| 4.2 | `repo_import` 无 `source` 列（`WEB` 属 `discovery_session.source`） | `[FACT]` | `0009_repo_import.sql:27-49` |
| 4.3 | 原 `draft` 为**单列 JSONB 覆盖写**，旧版本不可追溯 | `[DEFECT]` | `0009:38`；`lifecycle.rs` 原 UPDATE；已由 `0031` 拆为 `repo_import_draft_version` |
| 4.4 | 原 `repo_import_source` 无 Project 作用域键，事实与计数被覆盖 | `[DEFECT]` | `0009:60-74`；已由 `0031` 整表重建 |
| 4.5 | `repo_import_source` 原无 `UNIQUE (tenant_id, id)`（0019 的 15 表清单遗漏） | `[DEFECT]` | `0019_tenant.sql:154-168` |
| 4.6 | Import 名额索引原为 `repo_import_one_in_progress`（仅 `project_id`，未租户化） | `[DEFECT]` | `0009:52-54`；已由 `0031` 重建为 `repo_import_one_active_per_project (tenant_id, project_id)` |
| 4.7 | **取消状态会被 Worker 覆盖**：`set_stage` 的 UPDATE 无状态守卫 | `[DEFECT]` | `lifecycle.rs:492-510`（基线 §3.1 事实 #7） |
| 4.8 | **首个仓库失败即中止**：clone/采集失败即 `fail_import` 并整体返回，其余仓库不再处理 | `[DEFECT]` | `lifecycle.rs` run_import_job 循环（基线 §3.1 事实 #5；场景稿 §6「部分仓库失败保留已成功仓库」） |
| 4.9 | **失败路径不释放初始化名额**：`fail_import` 只更新 Import，未释放 `project_initialization_operation` 的 active 行 | `[DEFECT]` | `lifecycle.rs:513-524`（`0031` 引入） |
| 4.10 | Import 失败/取消/驳回/无人认领路径下，Project 初始化状态恢复不完整 | `[DEFECT]` | `fail_import` / `cancel_import` / 驳回路径分别处理，无统一收口 |

## 5. Discovery / Finding / Evidence

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 5.1 | `discovery.discovery_session` 已存在（5 态），不可重设计 | `[FACT]` | `0025_discovery.sql:12-30` |
| 5.2 | `discovery.discovery_finding_repository` 已存在且有真实读写 | `[FACT]` | `0025:67-76`；`discovery/finding.rs` |
| 5.3 | `discovery_finding` 原**无 `project_id`**，Finding↔Session 跨项目不一致无法 DB 级发现 | `[DEFECT]` | `0025:48-62`；已由 `0031` 加列 + 换轨 FK |
| 5.4 | `discovery_finding_repository` 的仓库 FK 只锚 `(tenant_id, repository_id)` → 同租户跨项目仓库可挂 Finding | `[DEFECT]` | `0025:74-75`；已由 `0031` 补-3 升格 |
| 5.5 | **无 Evidence 实体表**，证据只是 `discovery_finding.evidence` JSONB 数组 | `[DEFECT]` | `0025:57`；已由 `0031` 新建 `discovery_finding_evidence` |
| 5.6 | Discovery Session 状态机与 Task 状态机**并行且无交叉校验** | `[DEFECT]` | `discovery/session.rs:161-176` vs `pm-domain` Task 13 态 |
| 5.7 | Discovery 侧**无 Worker 分析作业**：事实只能由 MCP/REST 外部送入 | `[MISSING]` | `apps/ai-pm-worker` 无 discovery 作业 |
| 5.8 | Discovery 会话由 Web 与 Codex `/pm-learn` 两个 producer 共用 | `[FACT]` | `0025` source CHECK `WEB/CODEX/SYSTEM` |

## 6. Task / Run / 租约 / Codex 执行

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 6.1 | Task 13 态状态机 + 受守卫流转（`apply_transition`） | `[FACT]` | `pm-domain/src/task_transition.rs`；`run_dispatch/claim.rs:395` |
| 6.2 | Run 领取原子：`FOR UPDATE OF r SKIP LOCKED`，锁序 run→task | `[FACT]` | `run_dispatch/claim.rs:272-300, 346-353` |
| 6.3 | 租约有世代（`lease_generation`）防旧 token 复活；续租严格向前 | `[FACT]` | `run_context.rs:129-175` |
| 6.4 | 租约过期回收与领取同路径（无独立 reaper）；启动后失联走 `isolate_started_run` | `[FACT]` | `run_dispatch/lease.rs:61-114` |
| 6.5 | Codex 执行适配器已实现（进程隔离 / 工作区校验 / MCP 配置注入 / 指令文件） | `[FACT]` | `pm-infrastructure/src/codex_executor.rs` |
| 6.6 | **无 Codex bootstrap 层**：无固定子命令、无参数契约校验、无版本/能力探测 | `[MISSING]` | `codex_executor.rs:469-490`；执行端只有技能包 |
| 6.7 | 指令文件只含三步（取上下文 / 报进度 / 报结果），**无「回传证据」步骤** | `[MISSING]` | `codex_executor.rs:367-380` |
| 6.8 | MCP 18 → 19 个工具（新增 `pm_submit_evidence`）；权限码表只覆盖 5 个写工具 | `[DEFECT]` | `apps/pm-api/src/auth.rs:562-568`；lmms 只有 3 个 MCP 码 |
| 6.9 | Run 写侧只在 MCP（REST 侧 Run 只读） | `[FACT]` | `pm_runs.rs`（2 个 GET） |

## 7. Knowledge / Proposal / Coordinator

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 7.1 | `ai.document_proposal`（8 态发布管线）与 `discovery.discovery_proposal`（4 态审核）**并存且不合并** | `[FACT]` | `0014_knowledge_v2.sql:21`；`0025:113-133` |
| 7.2 | 项目级 advisory lock + `base_revision` 乐观并发 + `knowledge_write_operation` 幂等账本已建成 | `[FACT]` | `knowledge_write_coordinator.rs:68, 153, 241-268` |
| 7.3 | Coordinator 位于 pm-application（不在 Worker 内），Worker 不参与知识写入 | `[FACT]` | `knowledge_write_coordinator.rs` |
| 7.4 | `create_document` 无 REST 入口（仅 MCP） | `[MISSING]` | `knowledge.rs:281` |

## 8. Delivery / Review / Audit

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 8.1 | 交付清单与逐仓 commit 事实已建成（`task_repository_commit` + 视图） | `[FACT]` | `0022_delivery_manifest.sql` |
| 8.2 | Review 链与失败判定已建成 | `[FACT]` | `0026_delivery_review.sql`；`review/` |
| 8.3 | 交付清单**只有任务级**读模型，无项目级聚合 | `[MISSING]` | `pm_delivery.rs:21-25` |
| 8.4 | 审计四列 + `claimed_device_id` 统一口径 | `[FACT]` | `pm_write/audit.rs:40-68` |
| 8.5 | 仓库删除被拒（RESTRICT）尚缺应用层 409 映射 | `[PARTIAL]` | `0031` 第 11 项已加约束；错误映射未落地 |

## 9. Device / Bootstrap / Install

| # | 事实 | 等级 | 证据 |
|---|---|---|---|
| 9.1 | `devenv.device` / `devenv.device_project_binding` 已建成（跨租户组合 FK） | `[FACT]` | `0027_devenv.sql` |
| 9.2 | `/install`、`/install/{tool}`、`/health` 公开 | `[FACT]` | `auth.rs:288-294` |
| 9.3 | Host 白名单可配置（跨机暴露） | `[FACT]` | `mcp_hosts.rs:18-37` |
| 9.4 | T-090 Bootstrap 自检闭环的两机实测仍待安排环境 | `[PARTIAL]` | `todo.md` §10.7 |

---

## 10. 差异台账（设计 / 基线要求 ↔ 代码现状）

> 每条差异给出：**差异内容 → 依据 → 处置 → 归属 TASK**。

### 10.1 已在本轮之前登记、现已闭合

| # | 差异 | 依据 | 处置 | 状态 |
|---|---|---|---|---|
| G-01 | `core.project_member` 零写入路径 | 基线 §5.1（DB 与服务端双重保障） | S01-W13：写侧 + 4 端点 + 审计 | ✅ 闭合 |
| G-02 | 租户撤权不级联到项目成员 | 基线 §5.1 租户隔离 | S01-W13：级联撤销 + owner 依赖拒绝 | ✅ 闭合 |
| G-03 | 草案单列覆盖写 | 基线 §4.1 D-1a | 迁移 `0031`：`repo_import_draft_version` | ✅ 闭合 |
| G-04 | Evidence 无实体、跨项目可关联 | 基线 §4.2 D-2a | 迁移 `0031`：Evidence 表 + 组合 FK + CHECK | ✅ 闭合 |
| G-05 | Source 缺 Project 作用域键 | 基线 §4.2 D-2a | 迁移 `0031`：Source 整表重建 + 四列键 | ✅ 闭合 |
| G-06 | Finding 无 `project_id` | 基线 §4.2 D-2a | 迁移 `0031`：加列 + 换轨 FK | ✅ 闭合 |
| G-07 | 仓库删除 CASCADE，历史引用不保护 | 基线 §5.1 | 迁移 `0031`：RESTRICT + 应用层 `HasHistory` → 409（见 G-16） | ✅ 闭合 |
| G-08 | Import 名额索引未租户化 | 基线 §5.1 | 迁移 `0031`：索引重建 | ✅ 闭合 |

### 10.2 本轮新发现、待处置

| # | 差异 | 依据 | 处置 | 归属 |
|---|---|---|---|---|
| **G-09** | **基线 §4.2 明确要求保留 `UNIQUE (tenant_id, import_id, repository_id)`（Source 归属约束）** | 基线 §4.2 + §4.3「增加 Source 三列组合唯一键」 | ✅ **已满足**：迁移 `0031` 的 Source 主键即 `(tenant_id, import_id, repository_id)`，与该唯一约束等价；四列键 `(tenant_id, project_id, import_id, repository_id)` 另供 Attempt / Evidence 组合 FK 引用 | I6-B-08（已闭合） |
| **G-10** | **基线 §4.2 要求 Evidence 的 Import 外键包含 `tenant_id, project_id, import_id`** | 基线 §4.2 | ✅ **链式实现**：Evidence → `repo_import_source(t,p,import,repository)`（四列）→ `repo_import(t,p,id)`，以及 Evidence → `repo_import_source_attempt(t,p,import,repository,attempt_no)`（五列）；两条链的父键都带 `project_id`，等价保证「Evidence 的 Import 与其 Project 同 Project」。**不加直接的 Evidence → `repo_import` 外键**——0025 的 Q1「唯一关联方向」不变量要求 discovery 域不得引用 `integration.repo_import`，且专项设计稿优先级（基线 §1 第 3 条）高于正式基线措辞（第 5 条），直接外键在此模型下亦属冗余。差异裁决见 §10.4 D-C-01 | I6-B-08（已闭合） |
| **G-11** | **取消状态被 Worker 覆盖**（基线 §3.1 事实 #7） | 基线 §3.1 + §5.4 | ✅ `set_stage` 增加状态守卫：`cancelled` / `failed` / `rejected` / `applied` / `proposed` 的导入不再被 Worker 推进 | I6-B-01（已闭合） |
| **G-12** | **失败路径不释放初始化名额**（`fail_import` 未释放 operation active 行 → 该 Project 后续 `start_import` 触发 `uq_project_initialization_operation_active` 唯一违例，Project 卡死） | 基线 §5.1 + §5.4 | ✅ `fail_import` 在同一事务内释放初始化操作（条件 UPDATE + `rows_affected`）并写 `repo_import.failed` 审计（含 `initialization_released`） | I6-B-01（已闭合） |
| **G-13** | **首个仓库失败即中止**（基线 §3.1 事实 #5） | S01 场景稿 §6「部分仓库失败保留已成功接入的仓库」 | ✅ `run_import_job` 改为**逐仓失败隔离**：失败仓记 failed Attempt 并继续其余仓库；全部仓库失败才终止导入（`all_repositories_failed`） | I6-B-02（已闭合） |
| **G-14** | **基线 §7 的 TASK 字段缺 `code_facts` / `ui_scope` / `test_scope` / `evidence` 四列** | 基线 §7 表格 | ✅ `docs/s01-task-registry-v1.md` §1 字段表已按 §7 补齐四列并标注基线依据 | S01-W06（已闭合） |
| **G-15** | **基线 §8 的五级验收（L1～L5）未在验收矩阵中映射** | 基线 §8 | ✅ `docs/s01-acceptance-matrix-v1.md` 新增 §0 五级验收表 + §0.1 七类测试→五级映射 | S01-W07（已闭合） |
| **G-16** | **仓库删除被拒的应用层 409 映射缺失**（G-07 的应用层半边） | 基线 §3 / §4.3「错误码和业务错误映射正确」 | ✅ `repository::delete_repository` 捕获 FK 违例 → 新错误变体 `HasHistory` → HTTP 409 `repository has history and cannot be deleted` | I6-B-08（已闭合） |
| **G-17** | **基线 §6 要求首批页面以 Project Dashboard 交付**；当前实现顺序未对齐（I5 总控台排在最后） | 基线 §6.2/§6.3 | 记入 TASK Registry 的依赖顺序：Dashboard 相关切片前置 | I5-C/I5-D |
| **G-18** | **MCP 权限码只覆盖 5 个写工具**（缺陷 D-11）；lmms 只有 3 个 MCP 权限码，无发现族专用码 | 基线 §5.2「REST 与 MCP 必须遵循相同领域业务规则」 | **跨仓协同**：lmms 发码 → ai-pm 收紧 → 回归；单仓收紧会打断现网执行链路 | 跨仓待办 |
| **G-19** | **无 Evidence 回传通道的 MCP 步骤**（instruction.md 只三步） | 基线 §5.3 | `pm_submit_evidence` 工具已就位；执行端 `instruction.md` 补第四步（codex_executor.rs:367-380） | I6-B-05（工具侧闭合，指令文件待） |
| **G-20** | **Discovery 侧无 Worker 分析作业**（事实只能外部送入） | 基线 §2「I1～I6 六个工作域」 | 记入 I6-B-02 后续扩展 | I6-B-02 |

### 10.2b 2026-11-04 批次闭合

| # | 差异 | 依据 | 处置 | 归属 |
|---|---|---|---|---|
| **G-21** | **迁移 `0031` 的「升级路径与环境一致性」未演练**（基线 §4.4 第 6 项） | 基线 §4.4 / §11 准入 | ✅ `scripts/upgrade-drill-0028-to-0031.sh`：0028 → 0030 → 0031 升级库与新全序库的表/列/约束/索引**完全一致** + 0031 十项关键对象抽查全过。基线 §11 准入 12 项全绿，`0031` 冻结 | I6-B-09（已闭合） |
| **G-22** | **无项目级交付聚合**（缺陷 D-13） | 总体计划 §9.1 W60 / §15.E | ✅ `GET /v1/pm/projects/{id}/delivery`：只读投影 `v_task_delivery_manifest` + 任务清单 + pending 审批数；任务级卷起四态（delivered/partial/missing/no_contract）。注意视图逐仓 `delivery_state` 已是任务级口径，卷起是对状态组归类而非逐仓求交集 | S01-W60（已闭合） |
| **G-23** | **Finding 创建初态可被外部指定**（违反 I2-026）且可信 human 直建 user_confirmed 落 PENDING（违反 I2-023） | I2 V1.1 §15.1；I2-D 双入口铁律 | ✅ `NewFinding` 创建契约删除 `verification_status`；新增 `FindingCreator` 通道（HumanDirect / AgentRelayed）：human 直建 user_confirmed → CONFIRMED（I2-023）、Agent 转述 → PENDING（I2-024）；REST 与 MCP 同批改造防分叉 | I2-B/D（已闭合） |

| **G-24** | **旧决定路径先盲更新再校验 Session 归属（I2-029 违反）、无真人校验（I2-025）、无审计、无幂等；`set_finding_verification` 是可绕过审计/锁的公开原语（I2-035）** | I2 V1.1 §15.1/§15.4；I2-C | ✅ `record_finding_verification` 重写为单事务统一决定服务：先校验后写（I2-029/I2-030）、真人门（I2-025）、条件更新（I2-020）、审计 `finding.decided`（before/after）、operation_id 幂等（N-4 先例）；删除裸导出 `set_finding_verification`，状态写点收窄为 `pub(crate)` 仅决定服务可调。REST 契约保持兼容（I2-034），新增可选 `operation_id` | I2-C（已闭合） |

| **G-25** | **「待补充」展示态无推导依据**（§8.2：不能从 open question 推导，须从最近一次有效人工决定审计推导） | I2 V1.1 §8 / I2-033 | ✅ 决定服务新增 `REVISION_REQUESTED`（不改状态，落设计稿指定审计 action `discovery_finding_revision_requested`）；`DiscoveryFindingView` 增加 `display_state`（§8.1 四态映射，读取路径批量取最近决定审计，DISTINCT ON + created_at DESC, id DESC）；决定服务加归档守卫（project 行锁 + archived 拒绝） | I2-033（已闭合） |

| **G-26** | **Project Goal 无落地机制**（Q1/Q2 已裁但未实施；Goal 提案会被质量门自动提交，违反 Q2） | I4 §3/§4；用户裁决 Q1/Q2 | ✅ `project_goal.rs`（doc_key='project-goal'，唯一性由既有 UNIQUE 保证；ensure 幂等引导）；**submit_proposal 对 Goal 提案强制红线**（Q2 结构性保证：Agent 提案含初稿只挂起不发布）；RunContract 增 goal 快照（I4-E，缺失=null 显式信号）；REST GET /projects/{id}/goal；**I4-F 归档门禁**：知识写入四入口（create/submit/confirm/finalize）统一检查 Project 仍 active，与归档事务共享 `core.project` 行锁 → 提交与归档线性化；归档后 Goal/提案/修订历史仍可读 | I4-B/C/E/**F**（已闭合；I4-F 验收 5 例全绿） |

| **G-27** | **Git 能力授权零实现**（缺陷 D-7：无枚举、无字段、无策略、`git_access.rs` 物理只读） | I3 §3.1~§3.3；B-6 默认（方案 A）；**2026-11-04 用户简化裁决**（「不区分，都可以读写。因为这部分太复杂。」） | ✅ **V1.0 方案（2026-11-04 上午，已回退）**：迁移 `0032` 的 `task_run.policy_snapshot_hash TEXT` + `integration.git_capability_grant`（六能力 CHECK）+ `pm-domain::git_capability` 判定层 + REST capabilities 三端点。<br>✅ **V1.1 简化方案（同日定稿并落地）**：授权表由迁移 `0033` **撤下**、六能力/依赖闭包/逐主体授权与 REST 端点**全部废止**；判定层改 `pm-domain::repo_access`（单一 `read_write` 档位，4 例单测）；`pm-application::repo_access` 提供快照构建（Task 范围 ∩ verified）+ SHA-256 内容寻址 + 执行期强校验（快照 ∩ 现状：仓库失验 / 项目归档 → 拒绝）；claim 落快照；执行器物化仓库前按快照过滤；12 例集成验收 | I3-B/C/D/E（**已闭合，简化后**）；I3-F（直接 Git 绕过 / 凭据隔离）仍待 T-090 两机实测 |

| **G-28** | **归档生命周期零实现**（`core.project.status` 消费者为零；queued 无取消路径；`cancel_run` 无外部入口；残留进程无告警——I1 核验稿三入口事实） | I1 设计稿；B-1 默认（P1~P4 并入） | ✅ `project_gate.rs` 统一门禁（三入口接入：claim_core / TaskReadyHandler / enqueue_review）+ `archive.rs` 归档事务（P1 queued 条件取消、N-2 Import 终态化、P4 名额释放）+ REST 归档与 `POST /task-runs/{id}/cancel`（P2，reason 协议化）+ P3 残留进程告警（run.orphan_suspected，1h 去重）；I1 正文回填 V1.1 | I1-A/I1-D/P1~P4（已闭合；I1-B/C 两阶段候选、I1-E Executor 握手、I1-F 全链回归仍待） |

### 10.3 结构性观察（非缺陷，供后续裁决参考）

| # | 观察 | 说明 |
|---|---|---|
| O-01 | `core.project_member` 一列两义（真人角色 / Agent 职能） | 当前靠「值域白名单 + 类型层判定」分离；I3 V1.1 简化后不再新增 Git 能力主体，拆列的紧迫性下降（仍建议后续裁决） |
| O-02 | `task_repository.write_access` 布尔 vs I3 六能力 | **V1.1 简化后此张力消失**：六能力模型废止，该布尔不再承担访问档位，仅作调度前置与交付规则输入 |
| O-03 | 提案发布（`ai.document_proposal`）与 Discovery 提案（`discovery.discovery_proposal`）并存 | 基线 §5.4 要求区分「草稿、提案与正式知识」——当前两者语义确实不同，**不得合并** |

### 10.4 差异裁决（基线与专项设计稿冲突时的裁定记录）

> 依基线 §1「基线优先级」处理：专项设计稿与专家裁决（第 3 条）高于正式基线（第 5 条）。

| # | 冲突 | 裁决 | 依据与理由 |
|---|---|---|---|
| **D-C-01** | 基线 §4.2 写「Evidence 的 Import 外键包含 `tenant_id, project_id, import_id`」；而 0025 的 Q1「唯一关联方向」要求 **discovery 域不得引用 `integration.repo_import`**（单向：repo_import → discovery_session） | **不加直接的 Evidence → `repo_import` 外键**，改为链式实现：Evidence → `repo_import_source(t,p,import,repository)` → `repo_import(t,p,id)`，以及 Evidence → `repo_import_source_attempt(t,p,import,repository,attempt_no)`。两条链的父键均带 `project_id`，等价满足基线要求的「同 Project 归属由数据库保证」 | ① 0025 属专项设计稿（优先级第 3 条）高于正式基线措辞（第 5 条）；② 合并规格 §2.3 的「全链归属图」中 Evidence 无直连 Import 的边，链路经 Source/Attempt；③ 直接外键在该模型下**冗余**（已被链式约束蕴含）；④ `crates/pm-infrastructure/tests/discovery_schema.rs::repo_import_link_is_single_direction` 是该不变量的机械断言，加直连外键会使其失败——说明这是**被测试固化的设计不变量**，不是疏漏 |
| **D-C-02** | 基线 §4.2 要求保留 `UNIQUE (tenant_id, import_id, repository_id)`；迁移 `0031` 的 Source 主键恰为该三元组 | **判定为已满足**，不额外建唯一约束（主键已提供同等唯一性，且可被 FK 引用） | 避免同一三元组同时存在主键与唯一约束的冗余；四列键 `(tenant_id, project_id, import_id, repository_id)` 另建，供 Attempt / Evidence 组合 FK 引用 |
| **D-C-03** | I3 V1.0 设计稿（专项设计稿，优先级第 3 条）要求「六能力 + 四层授权交集 + 逐主体授权表」；而 S01 实施期用户裁决为「**不区分，都可以读写。因为这部分太复杂**」 | **以用户裁决为准**：I3 走 V1.1 简化模型——`可访问仓库 = Task 关联仓 ∩ 仓库状态 verified`，档位恒 `read_write`；六能力枚举、依赖闭包、逐主体授权表（迁移 `0033` 撤下）与 REST 授权端点**一并废止**。保留的访问边界（租户/项目隔离、Task 范围、仓库状态、Run 快照、归档与取消门禁）不受影响 | ① 用户裁决是**最高优先级的需求输入**，高于任何既有设计稿；② 专项设计稿可由用户显式修订（本次为 V1.0 → V1.1 修订，保留原文作决策留痕）；③ 六能力模型在 S01 无真实业务用例，却引入一张授权表、一套闭包判定、一组端点、一次撤权语义与 24 条验收——**成本远超收益**；④ 简化后仍保留全部**安全边界**（范围/状态/归档/取消/快照可追溯），未放宽到"无约束"；⑤ 详细映射见 I3 设计稿 §0.0（删除项/保留项/副作用/验收映射） |
| **D-C-04** | I4 设计稿 §12 验收表中，归档相关条目是 **I4-019（归档只读）/ I4-020（归档竞争）**；而 `docs/s01-task-registry-v1.md` 与 `docs/s01-acceptance-matrix-v1.md` 的 I4-F 行登记为「I4-022~024」（历史恢复 / 上下文追溯 / 不重复存储） | **以设计稿 §12 验收表为准**：I4-F 覆盖 **I4-019/020**；I4-022~024 归属不明（历史恢复与上下文追溯由既有修订/快照机制满足，不重复存储由「Goal = 既有知识表」结构性保证），台账两处登记已更正 | ① 设计稿 §12 是 I4 的**编号权威**（专项设计稿优先级高于本台账的二次登记）；② 两份台账的 I4-F 归属是**实施期映射**时的连带笔误（与 I2 双重占号同型）；③ 差异不改变实施内容——I4-F 的实际工作项（写入入口检查、确认入口检查、归档后拒绝新版本、历史查询保留、并发验证、审计完整）**逐条都在设计稿 §13 I4-F 的六项里** |
| **D-C-05** | I5 V1.1 §6.4 要求「Goal 是否确认 → I4 Goal 状态，**不允许**根据文档是否存在推断」；但 I4 实施后**领域内不存在 Goal 确认状态字段**（`project_goal.rs` 仅 4 个函数，`OfficialGoal` 只有 `published_at`，Goal 复用通用知识审计 action） | **I5 读模型不下「是否确认」结论**：只输出两个事实 `goal.present`（是否有正式 published 修订）与 `goal.pending_proposal`（是否有待确认修改），由人判断；**不得**新增臆造状态列，**不得**用「文档存在」反推「已确认」 | ① 设计稿 §6.4 的意图是「展示值必须可追溯到权威领域状态」，在领域无该状态时**正确做法是不下结论**，而不是造一个派生状态；② 与 I4 的既有裁决一致（Q1/Q2：Goal 的正式性由 published 修订 + 人工确认发布共同保证，但「确认动作」本身未落独立状态）；③ 若将来 I4 补了 Goal 确认状态/专用审计 action，本读模型应随之扩展 `goal.confirmed` 字段（登记为 I5-G-14 的后续） |
| **D-C-06** | I5 V1.1 §4.5「只有所有**适用且必需**的初始化事项均已完成，才可以显示基础初始化已完成」；但本仓**没有项目类型或初始化要求配置**（I5-A 全域核验无此数据源） | **初始化完成度输出 `completion = "not_determinable"`**，只展示「项目接入概况」与逐事项状态，**不宣称初始化已完成**（设计稿 §4.5 末句的明文出口）；必需事项集合保守取三项（项目基本信息 / 项目目标 / 人工处理），仓库/分析/认知/环境一律 `required=false`（零仓库合法） | ① 设计稿 §4.5 末句已预留该出口：「如果尚无项目类型或初始化要求配置……页面展示项目接入概况，不宣称项目已经完成全部初始化」；② 把仓库/环境定义为必需项会与 §4.7「零仓库是合法状态」冲突；③ 对应验收 I5-029「初始化必需项未定义 → 不伪造初始化完成状态」 |

---

## 11. 复核方式

- 事实条目可用 `grep` / `sed` / 文件行号在 `feat/s01-implementation` 分支复核。
- `[DEFECT]` 条目的闭合证据以 `docs/s01-acceptance-matrix-v1.md` 的验收条目与提交记录为准。
- 本台账随实施推进增量更新；**结论变更必须带证据**，不得只改结论。
