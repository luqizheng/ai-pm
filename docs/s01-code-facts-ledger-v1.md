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
| 3.4 | `task_repository.write_access` 是**布尔**，与 I3 的六能力模型不等价 | `[FACT]` | `0011_run_dispatch.sql:90` |
| 3.5 | **仓库级 Git 能力授权零实现**：无枚举、无字段、无策略、无判定层 | `[DEFECT]` | `grep "enum .*Permission"` 无匹配；`migrations/*.sql` 无权限字段 |
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
| G-07 | 仓库删除 CASCADE，历史引用不保护 | 基线 §5.1 | 迁移 `0031`：RESTRICT | ⚠️ 部分（应用层 409 映射未落地） |
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
| **G-19** | **无 Evidence 回传通道的 MCP 步骤**（instruction.md 只三步） | 基线 §5.3 | 已有 `pm_submit_evidence` 工具；执行端指令文件补第四步 | I6-B-05 |
| **G-20** | **Discovery 侧无 Worker 分析作业**（事实只能外部送入） | 基线 §2「I1～I6 六个工作域」 | 记入 I6-B-02 后续扩展 | I6-B-02 |

### 10.3 结构性观察（非缺陷，供后续裁决参考）

| # | 观察 | 说明 |
|---|---|---|
| O-01 | `core.project_member` 一列两义（真人角色 / Agent 职能） | 当前靠「值域白名单 + 类型层判定」分离；I3-B 引入 Git 能力后建议评估拆列 |
| O-02 | `task_repository.write_access` 布尔 vs I3 六能力 | I3-B 需换轨（能力集合），不可复用该布尔 |
| O-03 | 提案发布（`ai.document_proposal`）与 Discovery 提案（`discovery.discovery_proposal`）并存 | 基线 §5.4 要求区分「草稿、提案与正式知识」——当前两者语义确实不同，**不得合并** |

### 10.4 差异裁决（基线与专项设计稿冲突时的裁定记录）

> 依基线 §1「基线优先级」处理：专项设计稿与专家裁决（第 3 条）高于正式基线（第 5 条）。

| # | 冲突 | 裁决 | 依据与理由 |
|---|---|---|---|
| **D-C-01** | 基线 §4.2 写「Evidence 的 Import 外键包含 `tenant_id, project_id, import_id`」；而 0025 的 Q1「唯一关联方向」要求 **discovery 域不得引用 `integration.repo_import`**（单向：repo_import → discovery_session） | **不加直接的 Evidence → `repo_import` 外键**，改为链式实现：Evidence → `repo_import_source(t,p,import,repository)` → `repo_import(t,p,id)`，以及 Evidence → `repo_import_source_attempt(t,p,import,repository,attempt_no)`。两条链的父键均带 `project_id`，等价满足基线要求的「同 Project 归属由数据库保证」 | ① 0025 属专项设计稿（优先级第 3 条）高于正式基线措辞（第 5 条）；② 合并规格 §2.3 的「全链归属图」中 Evidence 无直连 Import 的边，链路经 Source/Attempt；③ 直接外键在该模型下**冗余**（已被链式约束蕴含）；④ `crates/pm-infrastructure/tests/discovery_schema.rs::repo_import_link_is_single_direction` 是该不变量的机械断言，加直连外键会使其失败——说明这是**被测试固化的设计不变量**，不是疏漏 |
| **D-C-02** | 基线 §4.2 要求保留 `UNIQUE (tenant_id, import_id, repository_id)`；迁移 `0031` 的 Source 主键恰为该三元组 | **判定为已满足**，不额外建唯一约束（主键已提供同等唯一性，且可被 FK 引用） | 避免同一三元组同时存在主键与唯一约束的冗余；四列键 `(tenant_id, project_id, import_id, repository_id)` 另建，供 Attempt / Evidence 组合 FK 引用 |

---

## 11. 复核方式

- 事实条目可用 `grep` / `sed` / 文件行号在 `feat/s01-implementation` 分支复核。
- `[DEFECT]` 条目的闭合证据以 `docs/s01-acceptance-matrix-v1.md` 的验收条目与提交记录为准。
- 本台账随实施推进增量更新；**结论变更必须带证据**，不得只改结论。
