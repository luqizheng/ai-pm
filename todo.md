# TODO — 执行清单

> 本文件是**唯一待办队列**。每条任务自包含，供 AI 直接领取执行。
> 维护规则：状态原地改（`[ ]` → `[~]` → `[x]`），条目只追加不删除；做完的移到 §13 归档。
> 编号只是稳定 ID，**排序以里程碑（§3–§10.6）为准**，不再按编号段位推断优先级。

**本版（v2，2026-11-01）的合并口径**：

- 依据 A：`target.md` —— 技术顾问**只按产品描述**制定的 12 阶段路线（P0–P11），他看不到本仓库实况。
- 依据 B：仓库实测 —— `migrations/`（0001–0018，18 个迁移 / 43 张表）、`crates/`（5 个包：pm-application / pm-domain / pm-ai / pm-infrastructure / pm-mcp）、`apps/`（3 个：pm-api / ai-pm-worker / pm-worker，合计 8 个 workspace 成员）、`frontend/src/shell/views/` + `frontend/src/workspace/`、`git log`。
- 冲突时**以实测为准**：顾问的路线用来「补漏」，不用来「重排已完成的活」。已实现的部分不再重复开工（§1 逐项给了证据）。
- **2026-11-01 增量**：多租户 / Agent / Executor 身份边界由 `docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`（Accepted）裁定；M1 拆为 **T-021a（β，身份归因）** / **T-021b（α+γ+δ，Tenant + Agent + Run 身份列）**；迁移号顺移（T-024 → `0021`、T-041 → `0022`）；ADR 未决项登记为 T-055~T-058，T-021a 遗留登记为 T-059。**T-020 已过稿、T-021a 已实施并验收**（交付见 §13 归档）。
- **2026-11-01 增量（二）**：执行端接入转入待实施——runbook 落盘（`docs/ai-pm-executor-onboarding-runbook-v1.md`，`3811a37`），新增 **§10.5 M8**（T-080 执行端 URL 引导式接入 / T-081 `/mcp` 跨机暴露 / T-082 `pm_list_tasks`）；用户确认 trae / codex / workbuddy / z-code / dsh 均为候选执行端。
- **2026-11-01 增量（三）**：Project Discovery（项目接入与认知建立）设计稿 V1.1 落盘（`docs/ai-pm-project-discovery-v1.md`，`203d11e` + DDL 草案 `4373e55`；已并入用户 Q1–Q7 裁决），新增 **§10.6 M9**（T-083~T-086）。核心边界：Discovery 独立于 Repository Import / Task / Run 四个业务域；Web 与 Codex `/pm-learn` 是同一 `DiscoverySession` 的两个 producer。
- **2026-11-01 增量（四）**：用户提供《Codex 自动初始化与开发环境接入（Codex Bootstrap）》设计供稿并落盘（`docs/ai-pm-codex-bootstrap-v1.md`，`3c614be`，**未过稿**），新增 **§10.7 M10**（T-087~T-092，T-092 为 V2 延后条目）。核心原则：安装说明负责指导安装、安装程序负责保证安装正确、服务端负责验证安装结果；MCP ≠ Worker；凭证走一次性授权码 + 短期凭证，安装指令不传长期 Token；验收矩阵 BOOT-01~14。本稿是 M8 T-080（onboarding URL）的深化延伸，实施前必须先过 T-087 设计定稿。
- **2026-11-01 增量（五）**：T-031 外部评审（六项意见，材料包 `docs/ai-pm-t031-executor-expert-review-pack.md`）；其中 Q2 执行端身份经架构专家裁决落盘 **ADR-0002**（`docs/ai-pm-adr-0002-executor-credential-separation.md`，Accepted：Model A/B 统一 lease_token、修订 D2 R-G4、终止顺序反转、T-051 升级安全前置、新增 execution_mode 列）；T-032 设计稿落盘（`docs/ai-pm-slice-d3-executor-hardening-v1.md`，**待过稿**），登记于 §6 M3。
- **2026-11-01 增量（六）**：`docs/ai-pm-codex-bootstrap-v1.md` 外部专家评审落盘（`docs/ai-pm-codex-bootstrap-review.md`，**有条件过稿**）：总体方向通过；须修正 6 项 P0（V1 与 D3/T-031 职责边界、Device/Runner/Agent Principal 身份关系对齐、三凭证分离、七步撤销协议、Project Binding 与 Repository Access 分层、Ready 服务端状态机）+ 补 3 章节（版本兼容协议、安装失败与重初始化协议、数据模型归属）+ 验收补 BOOT-15~25；专家建议的 T-087~T-092 六任务拆分与 §10.7 既有编号重叠且划分不同，**编号以既有为准**，拆分内容作 T-087 定稿输入；两条新增实施依赖：T-087 须等 T-032 收敛、服务端授权能力先于管理页。**用户确认后评审修订已收编进设计稿 V1.1**（6 项 P0 + 3 章节并入正文，章节重排 §〇~§十二；收编逐条对照见评审记录）。

---

## 0. 开工前置流程（AI 必须先读这一节）

### 0.1 强制：任何任务开工前先走 superpowers `brainstorming`

> **用户裁定（2026-11-01）**：「任务开始前都要按照 superpower 的 /brainstorming 来执行。」
> 触发命令：`/brainstorm`（等价于调用 `superpowers:brainstorming` 技能）。

**适用范围（必须走）**：新增功能、新建组件、改动行为、新增迁移、新增接口、改变既有契约、以及**任何需要做设计决策的任务**——包括本清单里标了「出设计稿」的 T-023 / T-030 / T-040 / T-042 / T-060（已归档的 T-020 同此流程）。**T-021b 的 α 阶段虽已有 ADR 定边界，仍需按 §0.1 走一遍设计确认（逐表迁移方案先评审，不改表）**。

**豁免范围（可跳过，但不得跳过「读依据」）**：纯机械动作，不产生任何设计决策的任务——如已归档的 T-050（修一行注释）。判据：**如果做这件事不需要在两个以上方案之间选择，就不必 brainstorm**。

**流程（按技能原文，逐步执行，不得压缩成一次问答）**：

1. **先看现状再开口**：读相关文件、文档、最近提交（`git log -10`）。禁止只凭本文件的任务描述直接提问或动手。
2. **一次只问一个问题**：优先多选题，一个问题一条消息。话题需要展开时拆成多轮，不要一次抛一串问题。
3. **至少给 2–3 个方案**：每个方案给权衡（成本 / 风险 / 与既有架构的一致性），**先给推荐项并说明理由**，不要只抛选项。
4. **分节呈现设计**：每节 200–300 字，覆盖架构、组件、数据流、错误处理、测试；**每节后停下来确认**，不要一次性输出完整设计。
5. **YAGNI**：设计里主动砍掉非必需功能；发现任务描述与实际需要不符时，先回到第 2 步澄清。
6. **设计定稿后落盘并提交**：写入 `docs/ai-pm-<topic>-v1.md`（本项目既有惯例；superpowers 默认的 `docs/plans/YYYY-MM-DD-<topic>-design.md` 在本项目已由 `docs/superpowers/plans/` 承载，设计依据类文档统一走 `docs/` 根）。用 `docs(...)` 单独提交。
7. **设计经用户确认后**，才可进入实施计划与编码。实施阶段建议接 `superpowers:writing-plans` 与 `superpowers:test-driven-development`。

**与交付定义的关系**：§0.2 第 4 条「设计文档已回填」的前提是第 6 步已完成。**没有经过确认的设计，不得进入实施**——这是硬门禁，不是建议。

### 0.2 读取与执行协议

1. **一次只领一条**。领用前把该行 `[ ]` 改成 `[~]`，并写上日期与执行者。不要并行领多条。
2. **开工前必读「依据」**。依据里的文档是上位约束，冲突时以文档为准，不要凭本文件的一句话自由发挥。
3. **不要越过「不做」边界**。每条都写了不做什么，越界产出等于返工。
4. **交付定义 = 三条同时成立**：① 验收命令全绿；② 设计文档/实现记录已回填；③ 已按提交纪律提交。
5. **提交纪律**（沿用 D1 的 Q10；2026-11-01 用户补充）：`docs(...)` 与 `feat(...)` **必须分开提交**，不得合并成一个 commit；**每完成一个任务条目单独 commit 一次，不得把多个条目的改动整批合并成一个 commit**。提交前清场，工作区不得混入无关改动。
6. **完成后**：把本行改成 `[x]` 并补一行「交付：<commit> + <关键结论>」，同时追加当日 `.workbuddy/memory/YYYY-MM-DD.md`。
7. **任务不存在时**：如果发现要做的活不在本清单里，先补一条到本文件（含目标/依据/验收），再执行，不要直接动手。

**通用验收命令**：

```bash
# 后端（需先启动 dev 库；dev 库常驻 5433）
docker compose -f docker-compose.dev.yml up -d
DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm cargo test --workspace
cargo check --workspace --all-targets

# 前端
cd frontend && npx vue-tsc --noEmit --skipLibCheck && npx vitest run
```

> 环境注意（2026-11-01 实测）：
> - **本机没装 `cargo-nextest`**，`scripts/cargo-test.sh` 会在第一步直接退出 —— 用 `cargo test --workspace`；装了 nextest 再改回脚本。
> - **`TEMP`/`TMP` 保持系统默认**：指向工作区内（如 `target/testtmp`）会让 knowledge 测试报 `knowledge root ... is inside git repository`。
> - 子进程写 `%TEMP%` 偶发 `os error 5`（沙箱噪声），换目录重跑即可。

---

## 1. 顾问路线 ↔ 实测状态（先读这张表再领活）

| 顾问阶段 | 顾问要求（target.md 摘要） | 实测证据 | 判定 | 剩余差距 |
|---|---|---|---|---|
| P0 项目与仓库基础 | 项目、仓库、多对多关联、成员角色、环境/系统配置 | `0001_core` + `0017`（客户）、`0008`（`project_ssh_key`/`project_repository`/verify）、`bootstrap` 落 5 人编制、`0007`/`0003`（LLM 与技能配置）、`0018`（`core.principal_identity` + 审计三列）、`0019`/`0020`（租户 + Agent + Run 身份列） | ✅ 完成 | 身份归因（T-021a）、租户边界（T-021b）、按 `project_member` 过滤的可见性（T-022）均已落地 |
| P1 Task 任务管理核心 | 模型、13 态状态机、依赖、类型、输入、变更记录、事件 | `0002`+`0015`（契约：goal/constraints/验收条目/交付物/知识引用）、`0010`（状态机 + `apply_task_transition` + `task_supersession`/`task_waiting`/`task_policy_override`）、`outbox` TaskReady | 🟡 除依赖外完成 | **前置/后置任务依赖零表零列**（`grep task_dependency` 无命中）→ T-023/T-024；逐字段变更历史未做（仅审计 + supersession） |
| P2 Run 执行管理核心 | 生命周期、调度、领取、租约、恢复、重试、策略快照 | `0011`/`0012`/`0013`、`run_dispatch.rs`（24 用例全绿）、`0016` 四层策略合成 | ✅ 完成 | 无（D1/D2/D2.1 已签收） |
| P3 MCP 接入层 | claim / get_task / context / start_run / heartbeat / progress / submit_delivery / complete_run | `pm-mcp` 11 工具：`pm_claim_task`、`pm_get_task_context`、`pm_get_run_context`、`pm_report_progress`、`pm_submit_result`、`pm_get_submission_status` + 5 个文档工具；run 级 `lease_token` | ✅ 已收敛 | 顾问列的 `start_run`/`heartbeat`/`submit_delivery`/`complete_run` 在 D2 契约里**合并**为 `report_progress`/`pm_submit_result`，不是缺失 |
| P4 Codex 执行引擎 | RunExecutor 端口、Worktree、进程管理、日志、超时、取消、执行器恢复 | **零实现**；`executor_kind`/`executor_session` 列已在 `0011:39-40` | ⬜ 未开始 | 整块 → T-030/T-031 |
| P5 交付与验收 | Delivery Manifest 多仓、Commit 追踪、文件清单、测试证据、Baseline Validation、交付历史 | `0011` 已有 `task_repository`/`task_run_repository`；**缺 `task_workspace`/`task_repository_commit`**、缺校验服务与历史视图 | 🟡 只有骨架 | 整块 → T-040/T-041 |
| P6 知识库与 PageIndex | Markdown 事实源、写入、索引、检索、上下文组装、执行后回写 | Markdown+Git 事实源（`0006`）、提案/质量门/红线（`0014`）、标题树索引（`knowledge_index.rs`）、`knowledge_reference` | 🟡 单仓闭环完成一半 | 缺可替换索引层与**执行后知识回写** → T-042/T-043（D5）、T-060 |
| P7 Review 与审批 | 独立 Reviewer Run、Findings、四值结论、审批引擎、策略分层 | `governance.approval_request`、`policy_engine` + `0016` 内容寻址四层合成；`pm-ai/self_review.rs` **只审导入提案**，不审交付 | 🟡 审批完成，Review 未做 | 独立 Reviewer Run 与 Findings → T-042/T-043 |
| P8 Outbox 可靠异步 | 同事务写入、Worker、幂等、重试、死信、重放、监控 | `0010:123` `integration.outbox_event` 已含 `attempts`/`dead_letter`/`locked_until`/`idempotency_key`；`crates/pm-infrastructure/src/outbox_worker.rs` + `apps/ai-pm-worker/src/main.rs:93` **只注册 `TaskReady`** | 🟡 地基完成 | 业务事件覆盖（RunCompleted/DeliverySubmitted/ReviewCompleted/KnowledgeBaseWriteRequested）、死信重放、监控 → T-061 |
| P9 多 Agent 协作 | Agent 角色、能力、任务分解、协作、上下文传递 | 只有 `project_member` 的 5 个固定角色；无 Agent 运行时 | ❌ 主动不做 | 与已裁定「不造自研多 Agent 运行时」冲突，延后（§12） |
| P10 研发管理工作台 | 六大工作区 + 11 个页面 | 经典页 `frontend/src/shell/views/pm/`：`overview/projects/tasks/requirements/approvals/audit/settings/customers`（8）；工作区 `frontend/src/workspace/`：`console` + `focus` + `pages/`（Team/Execution/Goals/Models/Activity/Requirements/Knowledge/Artifacts/Decisions）；`GET /v1/pm/task-runs` 只读（`apps/pm-api/src/pm_runs.rs:20`） | 🟡 页面骨架齐 | 执行中心接真 Run 数据、交付验收页 → T-070 |
| P11 规模化与自动化 | 多项目调度、并发控制、配额、自动规划 | 无 | ❌ 延后 | 需单仓闭环先稳定（§12） |

**结论（这一版的真实工作量）**：平台已有「任务 + Run + 调度 + 租约 + MCP + 审批 + 知识单仓」的底座，顾问路线里**真正从零开始**的只有 4 块——

1. **多租户/Agent 数据边界**（身份归因已于 T-021a 落地；全库仍零租户列）→ §4（ADR-0001 / T-021b）
2. **Task 依赖**（P1 明确要求，实测没有）→ §5
3. **执行引擎 + 多仓交付**（P4 + P5）→ §6/§7
4. **独立 Review + 失败判定 + 知识回写**（P7 后半 + P6 后半）→ §8

其余（索引层、Outbox 事件覆盖、工作台两页）属于**在既有地基上补齐**，可与主线搭车。P9/P11 维持不做。

---

## 2. 排序理由（为什么不按 P0→P11 顺次做）

- **身份先于执行器**：`governance.audit_log` 的 `actor` 曾由 `owner_actor`（`crates/pm-application/src/pm_write.rs:93`，三级回退）猜出来 —— **T-021a 已改为真实 principal**（token → `core.principal_identity` → `core.principal`，审计三列齐备；`owner_actor` 已退场为 `#[cfg(test)]` 夹具）。D3/D4/D5 每一步都要往审计里写执行证据（进程生命周期、逐仓 commit、验收裁决），身份已就位，后续切片直接受益。
- **租户边界先于执行器**：T-021b（α/γ/δ）提供 `tenant_id`、`agent_id`、`executor_id` 三类列，是 T-030/T-031 落库与 Registry 的前提（ADR-0001 §5/§10）；且 α 必须先于 γ/δ（Agent 与 Run 都挂在租户上）。
- **T-022 只依赖 T-021a**：可见性按 `project_member` 过滤，不需要 `tenant_id`；租户维度过滤在 T-021b 之后叠加，不改变 T-022 语义。
- **Task 依赖先于 D4/D5**：D5 的「完成后解锁下游任务」和 D3 领取前的等待判定都依赖它；现在 `task_waiting` 只能记原因、记不了关系。
- **D 系列硬顺序**：`docs/ai-pm-slice-d-execution-pipeline-v1.md` §3.2 规定 D1→D2→D3→D4→D5，跳步会得到「契约字段无来源」或「基线可被事后篡改」。D1/D2 已完成。
- **工作台跟随真实能力**：target.md 明确「不要先做与实际状态机脱节的假数据页」。执行中心与交付验收页排在 T-031/T-041 之后。

---

## 3. M0 · 清场（已完成，条目见 §13 归档）

> T-001 已交付并归档；工作区干净，本清单从 §4 开始领取。

---

## 4. M1 · 主体身份与多租户边界（P0 权限层 + 数据边界）

> 现状（2026-11-01 更新）：**T-020 / T-021a（β）/ T-021b（α+γ/δ）三条均已实施验收**（按「做完的移到 §13 归档」移出本节，完整记录见 §13）。
> - 身份：token 三元组 → `core.principal_identity(provider, tenant_id, subject)` → `core.principal`，审计三列齐备，未激活 fail-closed，`owner_actor` 已退场。
> - 租户：39 张业务表全部 `tenant_id NOT NULL` + 组合外键；`core.tenant` / `core.tenant_member` / `governance.tenant_audit_log` 就位；`tenant_member.role`（owner/admin/member）是唯一事实源，成员管理 6 端点已开。
> - Agent：`ai.agent_definition` / `core.tenant_agent` 三层结构 + `dev_agent` 租户级复用；`task_run` 已有 `agent_id` / `triggered_by_user_id` / `executor_id`。
>
> **已完成（T-022）**：可见性已按 `core.project_member` 收口（客户/项目/项目维度读端点，非成员 404）。
> **仍未做**：MCP 侧审计三列未落地（T-059）、`tenant_agent.is_enabled=false` 的 MCP 领取门（D3 执行端接入时补，见 T-059）。
>
> 架构边界已裁定：`docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`（Accepted，已按实施结果修订）。设计依据：`docs/ai-pm-tenant-infrastructure-v1.md`。

> §4 队列已清空：T-020 / T-021a / T-021b / T-022 全部归档（见 §13）。

---

## 5. M2 · Task 依赖与解锁（补 P1 的唯一硬缺口）

> T-023（设计稿 `9fbb92a`，用户已过稿）/ T-024（实施）均已交付归档（见 §13）；本节现存追补条目 T-025（编号核实 2026-11-01：T-025~T-029 段位空缺，可用）。

- [ ] **T-025 Task Dependency 并发与解锁正确性加固（T-024 追补）**
  - **任务书**：`docs/ai-pm-t025-task-dependency-hardening-v1.md`（用户最终裁定，2026-11-01）；评审依据 `docs/ai-pm-task-dependency-review.md` §9.1 / §9.2 / §7。
  - **本条按 §0.1 执行**：审查报告先行；修复方案（锁协议选型、Blocked 判定模型）经用户确认后再编码。
  - **做**：专项审查 6 项（并发建边环检测 / 建边与终态迁移竞态 / Blocked 解锁语义 / Run 退避与执行预算 / 终态写入入口 / 租户隔离）→ 审查报告（五分类：确认 Bug / 潜在风险 / 已正确解决 / 建议修改位置 / 迁移是否必要）→ 实施修复 → 新增并发测试 C59–C68（双独立连接 + 显式 barrier，真实 PostgreSQL）。
  - **不做**：不推翻 T-024 架构；不重新设计依赖边表；评审 §9.3 P2 六项不在本次范围；不做随机碰撞式并发测试。
  - **验收**：C59–C68 中 ★ 最低集全绿（C63/C65/C67 无法覆盖须报告说明理由）；`cargo test --workspace` + `cargo check --workspace --all-targets` 全绿；T-023/T-024 实施记录回填；T-025 验收矩阵及测试映射落盘。
  - **红线**：不得为了让测试通过而降低现有状态机、租户隔离和事务一致性要求。
  - **进度（2026-11-01）**：**P0 两项（§一/§二）已修复并验收**——项目级图锁 `pg_advisory_xact_lock`（建边/删边/终态迁移统一取锁、取锁点先于任何行锁、零迁移）+ 并发测试 6 例全绿（73 套件 462 passed 0 failed）；审查五分类与验收映射见任务书 §十。
  - **进度（2026-11-02）**：**P1 §三（Blocked 解锁语义）已按方案 A 修复并验收**——`has_active_waiting` 谓词（活跃 `task_waiting` → 依赖满足也不解锁；因该表枚举无依赖类原因，此谓词即等价「无其它阻塞源」）+ C61 用例（blocked+needs_approval 保持阻塞、0 `TaskReady`；等待解除后一次放行）+ 零迁移。**P1 §五（终态写入入口）已修复并验收**——`pm_write::update_task`（UI 直写）并入「行锁 + 项目图锁」（终态直写先取图锁再 `FOR UPDATE`；非终态不取图锁），解锁判定统一到 `unlocks_downstream`；C68 两例（副作用一致 + 直写等图锁的屏障式定向验证）。`pm-application` 全量 172 passed 0 failed，`pm-api pm_writes` 13 / `pm-mcp contract` 6 无回归。遗留：`update_task` 仍不经 `validate_transition`（待单独裁定）。P1 剩余 §四/§六 待续，详见任务书 §10.5–§10.7。

---

## 6. M3 · Codex 执行引擎（D3）

> T-030（设计稿 `edf230f`，已过稿）/ T-031（实施 `6f3ded4`）已交付归档（见 §13）。T-031 外部评审后新增可靠性加固任务：

- [x] **T-032 实施 D3.1 可靠性与启动协议加固（设计稿 `docs/ai-pm-slice-d3-executor-hardening-v1.md` **已过稿并实施**，2026-11-01 T1–T4 终裁并入；实施记录见设计稿 §10）**
  - **做**：① 启动持久化协议（T1 终裁 ii：**`executor_session` 前置即启动意图，零迁移**；pid 文件原子写；reconcile 三重证据判定「PID 文件不存在 ≠ 进程不存在」；spawn 失败 session 保留 + `session_replaced` 事件留痕）；② 执行端凭据下发（按 ADR-0002：修订 D2 R-G4、`AI_PM_LEASE_TOKEN` 注入、七项 MCP 校验、**T2 终裁 a：`lease_generation`（首次领取=1，重领+1，心跳/失败不递增，MCP 双验证）**）；③ 心跳/存活分离（**T3 终裁 i：`deadline_at` 首次 claim 固化，重领/续租不得重置延长，绝对截止优先于续期** + `last_agent_heartbeat_at`/`last_progress_at`（**心跳 ≠ 进展**）+ 终止顺序反转「先撤凭据后杀进程」+ 终态僵尸兜底）；④ 子进程完整回收（Windows Job Object / Unix Process Group / PID 创建时间比对）；⑤ 补充项：镜像仓锁、workspace_path 二次校验、日志限制脱敏、结构化 ExitOutcome、`executor-events.jsonl`；⑥ CodexLaunchProfile（`AI_PM_CODEX_ARGS` 降级为覆盖项）。**迁移：新增 forward-only `0023_run_executor_hardening.sql`（四列一并；0011/0015 已应用不得回改；0022 号段冲突已解：T-041 保留 0022 / T-083 顺移 0024）**。
  - **不做**：不新增凭据体系（lease_token 唯一）；不动 13 态状态机（starting 以 executor_session 前置表达，无新状态无新枚举）；不落 Executor Registry；不提前 D5；不引容器/远程。
  - **验收（已通过）**：设计稿 §8 十二条 + **§8.1 生命周期一致性总验收**（11 步链路 + 两个故障窗口 + 5 场景必测矩阵 + **端到端五项：启动崩溃恢复 / 旧 generation fencing / 绝对截止 / 心跳与进展分离 / 子进程终止回收**；Q1/Q2/Q3 不独立验收）；`cargo test --workspace` **73 套件 / 465 passed / 0 failed**（T-031 基线 456 → 净增 9：C71 日志脱敏禁止面 / C72 PID 重用身份锚点 / C73 孤儿 Uncertain / c19 generation 递增）；`cargo check --workspace --all-targets` 0 error / 0 warning；T-031/D1/D2 既有用例不回归。
  - **实施口径微调**：starting 载体复用既有 `stage` 列（`dispatch`=意图未确认 / `started`=已确认，B3-1 判定扩展）；`lease_generation` 新行 `DEFAULT 0`（claim 递增后首次=1，符合终裁）。详见设计稿 §10。

---

## 7. M4 · 交付与验收（D4）

> T-040 设计稿已交付并过稿（`6f66305`，归档见 §13）；本节剩 T-041 实施。

- [x] **T-041 实施 D4（前置：T-040 已确认 ✅）**（2026-11-02 领用，执行者：Trae）
  - **做**：迁移 `0022_delivery_manifest.sql`；`pm_submit_result` 的交付段落两表；Baseline Validation（base_commit 与仓库实际 HEAD 对账）；交付清单只读接口（前端页在 T-070）。
  - **不做**：不做自动 merge/推远端；不做审批流改造。
  - **验收**：三仓任务可插入完整链路并逐仓对账；基线漂移被拒；只读仓出现 commit 整单驳回；`cargo test` 全绿。
  - **实施口径**：`delivery.task_repository_commit` + `task_run.integration_status`（交付段落两表）；`pm-application::delivery` 单事务管线（⓪幂等识别 → ①fencing → ②契约仓全集 → ③上报校验 → ④只读仓事实 → ⑤基线比对 → ⑥git 校验 → ⑦落库）；git 校验跑在**当前 Run 的实际工作区**（对象类型 / 后代 / ==HEAD / 变更计数现算）；任务状态迁移统一经 D1 `update_task_status` 通道；`ON DELETE RESTRICT` 保交付历史；视图 `v_task_delivery_manifest` 只陈述事实（分母只计可写仓，`latest_run`/`latest_submitted_run` 分离）；只读接口 `GET /v1/pm/tasks/{task_id}/delivery`。设计稿 §9 实施记录（落地物 / 9 项实施裁定 / C71–C97 逐条落点）。
  - **关键结论**：① 可写仓漏报**不驳回**——D4 只记事实（视图 `partial`），最终裁决归 D5；② `commit_sha` 落**工作区实际 HEAD**，执行端声明只做一致性比对；③ 业务驳回**不是事务失败**（`SubmitOutcome::Rejected` 正常 COMMIT，驳回审计可靠存在、0 行 commit）；④ `task_repository_commit → task_repository` 组合外键**不成立**（`task_repository` 主键含 `task_id`，本表无此列），仓归属由两条 `(project_id, …)` 组合外键保证。
  - **交付**：见本节末尾「交付记录」；`cargo test --workspace` 全绿（74 个测试二进制）、`cargo check --workspace --all-targets` 0 error / 0 warning；C71–C97 27 条 + C11 重写 + C84/C85 MCP 端到端。

---

## 8. M5 · Review、失败恢复与知识回写（D5）

- [~] **T-042 出设计稿 `docs/ai-pm-slice-d5-review-v1.md`**（2026-11-02 CodeArts：七题裁定 Q-D5-1…7 收编 v1 → 评审 P0×6/P1×4 收编 V1.1 → 评审补充场景 10 项收编 → 最终架构判断（有条件通过，暂缓 T-043，先修订再过稿）→ D1–D6 + 补充 4/5 → **V1.3：① Knowledge Proposal 改与 Task completed 同事务强一致（异步集废止；C117/C118/C128 改版 + 新增 C137）② 迁移号核实（0024 归 T-060，D5 取 0026）③ 验收矩阵四要素化 ④ §12.1 明确不做七项 → 用户最终建议确认「修订范围 7 项（4 必须 + 3 建议）全部落实」**；验收矩阵终态 **C98–C137（40 条）**；评审核对见 `docs/ai-pm-slice-d5-review-review.md`；**过稿就绪，待用户过稿**）
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **目标**：形成可审计的闭环——失败判定（B4）、独立 Review Run、知识回写。
  - **依据**：`target.md` §P7 五条边界（Reviewer 是独立 Run、默认只读、Review 失败 ≠ Run 失败、必须区分「需要审批」与「已经审批」、高层强制审批不可被任务级放宽——最后一条 D2.1 已实现）；`delivery.task_acceptance_criterion`（`0015`，逐条留痕已建表）；`0014` 知识提案/质量门（回写只能走提案，不得直写）。
  - **要在稿里裁清的点**：失败判定的输入（退出码/测试证据/超时 vs 人工驳回）与终态映射；Reviewer Run 是否复用 `task_run` 还是独立表；四值结论（Approved/ChangesRequested/Inconclusive/Failed）如何落到 task 状态；逐条验收裁决的存储与「结论≠证据」的区分；知识回写的触发点与去重。
  - **不做**：不做多 Agent 协作；不做自动修复（ChangesRequested 之后由人/新 run 处理）。
  - **验收**：含状态图、裁决表、验收矩阵；用户过稿后才可进 T-043。

- [x] **T-043 实施 D5（前置：T-042 已确认 ✅）**（2026-11-02 领用，执行者：Trae）
  - **做**：失败判定服务；Reviewer Run 编排（只读，写 findings）；逐条 `task_acceptance_criterion` 裁决留痕；完成/失败后的知识回写（走 `knowledge_proposal`）；投递 `ReviewCompleted`/`KnowledgeBaseWriteRequested` 事件。
  - **不做**：不自动修改被审对象；不改 D2 工具面。
  - **验收**：两条闭环用例（交付通过→completed + 知识提案；交付不通过→changes_requested 且原 run 不被判失败）；`cargo test` 全绿。
  - **交付**：见 §13 实施记录（`docs/ai-pm-slice-d5-review-v1.md`）；14 条 D5 用例（`crates/pm-application/tests/review.rs`）+ `cargo test --workspace` 全绿（75 个 `test result: ok`、516 passed、0 failed）；两条闭环（C100 自动完成含 `draft` 提案与两事件 / C103 `integration_status=failed` → changes_requested 且 delivery run 不受影响）均已实测通过。提交链 `333b7e0` feat(review) → `5c8496b` docs(d5) → `chore(todo)` → `chore(memory)`。

---

## 9. M6 · 知识索引与 Outbox 补齐（可与 M4/M5 搭车）

- [~] **T-060 索引层可替换化 + 完整检索（P6 后半）**（2026-11-02 开工：v1.3 已补 Project 级写入协调与并发模型，待复审后实施）
  - **设计稿**：`docs/ai-pm-t060-knowledge-index-v1.md`（Q1–Q4 + A-1~A-6 已裁定；两轮评审均为「修改后过稿」。v1.1 补并发转换原语、`index_version` 边界、状态不变量、故障分级、K-17~K-25；v1.2 收录专家评审全文（§12），补当前生效修订校验、`KnowledgeSearchService` 职责分离、分页语义、索引表级故障回退、清理生命周期、证据契约；v1.3 增补 Project 级 `KnowledgeWriteCoordinator`（§5.8：跨实例会话锁、串行范围覆盖同步重建、`operation_id` 幂等、`base_revision` 校验、K-26~K-30）；待复审）。
  - **本条按 §0.1 执行**（涉及选型与端口设计）。
  - **目标**：把现在的标题树内存扫描换成「可替换索引器端口 + 落库索引状态 + 增量重建」，检索质量与规模可演进。
  - **依据**：`crates/pm-application/src/knowledge_index.rs`（当前为内存扫描）、`knowledge_indexer.rs`、`0014` 的 `ai.document_index_state`、`docs/ai-pm-markdown-knowledge-space.md` §十一（「PageIndex / 向量检索作为可替换的 KnowledgeIndexer 适配留后续」）。
  - **要裁的点**：索引产物落库还是落 Git；增量触发（写入后同步 vs 后台 job）；检索排序（标题/正文/引用）是否引入向量；是否引入外部依赖。
  - **不做**：不引入必须在生产跑的向量数据库；不重写 Markdown 事实源。
  - **验收**：索引可重建且幂等；`document_index_state` 与正文哈希一致；检索接口保持向后兼容；用例覆盖「哈希漂移时索引失效」。

- [ ] **T-061 Outbox 业务事件覆盖与死信重放（P8）**
  - **目标**：把 outbox 从「一个 TaskReady 处理器」补成事件体系。
  - **依据**：`0010:123-144`（`outbox_event` 已含 `attempts`/`dead_letter`/`locked_until`/`idempotency_key`）、`apps/ai-pm-worker/src/main.rs`（只注册 `TaskReady`）、`target.md` §P8 首批事件清单。
  - **做**：为 `RunCompleted`/`DeliverySubmitted`/`ReviewCompleted`/`KnowledgeBaseWriteRequested` 建真实生产者（必须同事务写入）；死信重放接口；事件状态监控（计数/积压/失败率）。
  - **不做**：不为凑事件数造无业务语义的 Handler（target.md 明确）。**能搭车的先搭**：`RunCompleted` 可随 T-031 落，`DeliverySubmitted` 随 T-041，`ReviewCompleted` 随 T-043。
  - **已搭车（2026-11-02，T-043 顺带落地）**：`DeliverySubmitted` 生产者（`delivery/mod.rs` 提交管线同事务）+ 消费者（`DeliverySubmittedHandler` → D5 拍①，默认开启）；`ReviewCompleted` / `KnowledgeBaseWriteRequested` 生产者（completed 同事务，经 `AI_PM_OUTBOX_EVENTS` 开关，V1 按设计不注册消费 handler）。**仍缺**：`RunCompleted` 生产者、死信重放接口、事件监控指标。
  - **验收**：四条事件均有生产端与消费端；重复投递不产生副作用；死信重放幂等；监控页或指标可读。

---

## 10. M7 · 工作台补齐（P10 的两个真空）

- [~] **T-070 执行中心接真数据 + 交付验收页（前置：T-031 / T-041）**（2026-11-02 领用，执行者：Trae · 设计稿阶段，稿：`docs/ai-pm-delivery-acceptance-workbench-v1.md` 待确认）
  - **目标**：把 `workspace/:projectId/execution` 从骨架换成真实 run 数据；新增交付验收视图（Commit / 测试证据 / 审批结论）。
  - **依据**：`GET /v1/pm/task-runs`（`apps/pm-api/src/pm_runs.rs`，现只有读端点）、`target.md` §P10「AI 执行中心 / 交付验收」两行、`docs/ai-pm-task-detail-page-v1.md`（页面契约）。
  - **做**：run 列表与详情（状态、租约、executor_session、日志入口）；交付清单页（多仓 commit + 证据）；空态与错误态。
  - **不做**：不做假数据页（`VITE_USE_MOCK=0` 必须能跑）；不做 Agent 管理页（P9 延后）。
  - **验收**：真库下能看到 T-031 产生的 run 与 T-041 产生的交付；`vue-tsc` 0 error；vitest 全绿。

- [ ] **T-071 待裁 #13 分阶段交付 / 部分通过（partial approval）**：**保持开放，不主动实施**（2026-11-02 T-070 设计裁定，用户确认 V1 明确不支持）。13 态状态机无部分完成语义；部分通过涉及交付范围拆分与任务拆分，是领域模型变更而非 UI 功能。触发条件：业务真出现分阶段交付需求，届时与「任务拆分 / 交付范围 / 验收生命周期」一并出设计稿（依据：`docs/ai-pm-delivery-acceptance-workbench-v1.md` §九）。

---

## 10.5 M8 · 执行端接入与 Onboarding（2026-11-01 用户新增）

> 背景：用户确认 z-code / dsh 均支持 MCP，执行端接入从「设计讨论」转入「待实施」；配套 runbook 已落盘（`docs/ai-pm-executor-onboarding-runbook-v1.md`，`3811a37`）。本节三条为 runbook §十缺口与用户裁定的接入方式的落地条目。

- [ ] **T-080 执行端 URL 引导式接入（onboarding endpoint）设计稿 + 实施**
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **用户描述的流程（2026-11-01 原话口径）**：用户第一次安装工具 → 打开 trae / codex 等执行端 → **粘贴平台 URL** → 对 agent 说「**帮我安装**」→ **agent 根据 URL 提供的安装方法进行安装**（写本工具的 MCP 配置、引导填 key、验证连通），全程无手工编辑配置文件。
  - **目标**：平台暴露一个稳定、agent 可读的接入 URL，返回分工具的安装指引；执行端 agent 拉取后按指引完成配置与自检，把「粘贴 URL + 一句话」变成全部安装操作。
  - **依据**：`docs/ai-pm-executor-onboarding-runbook-v1.md`（§三 配置契约 / §十一 R1 用户已定向）；`docs/ai-pm-mcp-executor-positioning-v1.md` §3.4 A5（transport 无关）、Q9/Q10（首批执行端与预注册档位）；`crates/pm-mcp/src/server.rs`（Streamable HTTP、loopback 现状）。
  - **要在稿里裁清的点**：
    1. URL 形态：静态 Markdown 指引页 vs 结构化 JSON（`?tool=trae` 按工具变体）vs 两者（人看页 + agent 读机器可读）。
    2. 指引内容边界：URL 只含配置片段与验证步骤，**不承载 key**（密钥由用户在安装过程中另行输入，不得出现在 URL 或指引内容里）。
    3. 验证闭环：安装完成的判定 = `GET /health` 200 + `GET /v1/pm/me` 200（含 `is_active` 状态提示，未激活给出下一步指引）。
    4. 与预注册档位（T-051）的关系：onboarding 是否顺带登记 `client_name` 档位。
    5. 人工档租约 / 心跳默认值（runbook G5 / R3）是否随 onboarding 指引下发。
  - **不做**：不在 URL 或指引中传递凭据；不负责安装执行端软件本身（装 trae/codex 是用户自己的事）；不做无人值守 Runner（T-030 范围）；不改 11 个 MCP 工具的契约。
  - **验收**：用 ≥2 个不同执行端（建议 trae + codex，z-code / dsh 跟进）实测「粘贴 URL → 帮我安装 → 配置就位 → `/v1/pm/me` 200」全程无手工编辑配置文件；未激活路径给出明确指引而非报错。

- [ ] **T-081 `/mcp` 跨机暴露（runbook G1；T-080 跨机场景的前置）**
  - **目标**：`/mcp` 从 loopback-only（`server.rs:10-14`，M1 本地单机设计）放开到受控跨机访问：host 白名单配置化 + TLS/反向代理 + 鉴权加固，支撑多 PC / 多人接入。
  - **依据**：`crates/pm-mcp/src/server.rs:10-14`；`docs/ai-pm-executor-onboarding-runbook-v1.md` §十 G1 / §十一 R2。
  - **做**：`allowed_hosts` 从代码默认改为可配置；部署侧 TLS / 反代方案；跨机安全评审（key 传输路径、Host 头校验语义变化、请求来源限制）。
  - **不做**：不做 MCP OAuth（定位文档 M1 scope cut，需另行立项才动）；不动鉴权三层链（bearer → resolve → active）。
  - **验收**：另一台 PC 的执行端可完成 T-080 的安装验证闭环；同机用例与 rmcp 契约测试不回归。

- [ ] **T-082 `pm_list_tasks` MCP 工具（runbook G3）**
  - **目标**：执行端不切 Web 控制台即可发现可领取任务：`project_id` 范围内 `ready` 状态任务的只读列表。
  - **依据**：`docs/ai-pm-executor-onboarding-runbook-v1.md` §十 G3 / §十一 R4；`docs/ai-pm-mcp-executor-positioning-v1.md` §4.3（`task.list` 对应项）。
  - **做**：新增只读工具，复用 T-022 可见性语义（持 key 真人 + 租户内 + `project_member`）；命名跟随现有 `pm_*`（Q1 命名收敛前不引入新命名体系）。
  - **不做**：不做按执行端能力 / 档位过滤（归 T-051）；不改 `pm_claim_task` 契约。
  - **验收**：空项目 / 有 ready 任务 / 非本租户 三态用例；`cargo test --workspace` 全绿。

---

## 10.6 M9 · Project Discovery（项目接入与认知建立，2026-11-01 用户新增）

> **设计依据**：`docs/ai-pm-project-discovery-v1.md`（V1.1，已并入用户 Q1–Q7 裁决；主稿 `203d11e`、DDL 草案 `4373e55`）。
> **核心边界**：Discovery 独立于 Repository Import / Task / Run 四个业务域——Import 接入仓库，Discovery 理解项目，Task 推动变化，Run 执行工作。
> **双入口**：Web（Git URL 自动分析，= 已落地 `repo_import`）与 Codex `/pm-learn`（本地深度分析）是同一 `DiscoverySession` 的两个 producer（`source∈{WEB,CODEX,SYSTEM}`）。
> **前置**：Codex 入口依赖 T-080/T-081（MCP 执行端接入）；Web 入口与数据模型可先做。

- [ ] **T-083 实施 DiscoverySession 数据模型（迁移 `0024_discovery.sql` + 领域层）**
  - **目标**：落 `discovery` schema 7 张表 + `repo_import.discovery_session_id` 单向 FK；`pm-application::discovery` 提供 session / message / finding / question / answer / proposal 的读写与状态机。
  - **依据**：设计稿 §四 + 附录 A DDL 草案；0019 多租户模式（`tenant_id` + 组合 FK + 每表 `(tenant_id,id)` 唯一）。
  - **做**：迁移按附录 A；同步更新 schema 计数 / 租户隔离测试期望（现有 43 张 → +7）；四类 `finding.kind` 的 CHECK 与 `confidence∈[0,1]`；`proposal.base_revision_id`（= Q7 base_version）；`repo_import` 只加单向 FK，不做反向。
  - **不做**：不做 Web UI；不做 MCP 工具；不做跨仓库深度分析（P2）。
  - **验收**：`cargo test --workspace` 全绿；新库从零迁移无错；`information_schema` 断言 7 张新表均有 `tenant_id` 且组合 FK 指向 `core.project(tenant_id,id)`；确认无反向 FK。

- [ ] **T-084 Web 项目访谈与发现卡片（P0）**
  - **目标**：把现有 `repo_import` 的 `draft_ready + unknowns` 升级为 session 驱动的访谈 UI；发现卡片支持四类 finding 展示与用户确认。
  - **依据**：设计稿 §五 / §六；现有 `crates/pm-application/src/project_import/`（lifecycle / proposal）与前端 `Review.vue`；`discovery` 表来自 T-083。
  - **做**：`/v1/pm/projects/{id}/discovery/sessions` 读写端点；发现卡片（待确认徽标 + 分析证据[file+说明] + 四选一确认 + 确认按钮）；用户选择写 `discovery_answer` 并把 `verification_status` 从 `PENDING` 推进；Web 创建的 session 挂 `source=WEB`。
  - **不做**：不做自由聊天；不做 AI 主动 tool-loop；不改现有 import 的 clone/read/analyze；不改 T-022 可见性语义。
  - **验收**：Fake 模型下首轮 `unknowns` 能在卡片呈现；回答后 session 状态与 finding 校验状态正确；`vue-tsc` 0 error；vitest 全绿。

- [ ] **T-085 MCP `pm_discovery_*` 工具（P0）**
  - **目标**：6 个语义工具接入 `pm_mcp`，全部 propose-only。
  - **依据**：设计稿 §八；`crates/pm-mcp/src/tools.rs` 现有 `pm_submit_document` / `propose_document` 风格；Q2 裁定（不实现别名）。
  - **做**：`pm_discovery_get` / `pm_discovery_get_context` / `pm_discovery_submit_findings` / `pm_discovery_submit_answers` / `pm_discovery_submit_proposal` / `pm_discovery_complete`；`submit_proposal` 复用 `propose_document` 链（带 `base_revision_id`）。
  - **不做**：不做命名别名；不做通用写；不改 11 个现有工具契约；不做跨仓库分析。
  - **验收**：工具在 `/mcp` 可列；三态用例（空 session / 有 finding / 非法 kind）；propose-only 断言（不直写 `ai.project_document`）；`cargo test` 全绿。

- [ ] **T-086 `/pm-learn` Skill（P1，executor-neutral）**
  - **前置**：T-085 完成 + T-080/T-081（MCP 执行端接入）可用。
  - **目标**：交付一份 executor-neutral 的 `SKILL.md`，装进 Trae / Codex / Claude Code 等任意 MCP-capable 执行端；走 `pm_discovery_*` 完成「读码 → 澄清 → 提交」。
  - **依据**：设计稿 §七；`docs/ai-pm-mcp-executor-positioning-v1.md` §3.4 A1–A6（工具无关性）；`docs/ai-pm-executor-onboarding-runbook-v1.md`。
  - **做**：SKILL.md（命令流程：取 session → 读已有知识 → 分析本地 Git → 澄清 → 提交 findings/answers/proposal → complete）；在 Trae 实测一轮。
  - **不做**：不写死 Trae 专有 API；不给 skill 知识库最终写入权；不做无人值守。
  - **验收**：≥1 个执行端实测「读本地仓库 → 澄清 → `pm_discovery_submit_*` → Web 确认」闭环；未激活路径给出明确指引而非报错。

> **P2 延后（不在本节领取）**：多仓库架构关联分析（跨仓调用关系 / 依赖图 / 部署拓扑 / 微服务边界）、持续认知自动更新。T-083~T-086 稳定后再评估。

---

## 10.7 M10 · Codex 环境自动初始化（Codex Bootstrap，2026-11-01 用户新增）

> **设计依据**：`docs/ai-pm-codex-bootstrap-v1.md`（V1 用户供稿：ADR + 安装协议 + 界面原型，**未过稿**；用户稿自称 Slice F — Codex Environment Bootstrap，验收矩阵 BOOT-01~14）。
> **核心原则**：安装说明负责指导安装，安装程序负责保证安装正确，PM-AI 服务端负责验证安装结果——不让 AI 仅凭一段 Markdown 猜测如何修改配置文件。
> **三组件分离**：平台端 Worker（Run 调度/租约/回收，属服务端）≠ 开发机端 Runner（本地执行环境管理与 Codex 实例调度）≠ Codex 端 Skill+MCP（工作规范 + 平台接口）。
> **与 M8 的关系**：本稿是 T-080「粘贴 URL → 帮我安装」的深化延伸（Plugin + Worker 安装程序 + 设备授权 + 自检）；前置依赖 T-080/T-081。
> **状态**：**全部条目为未过稿设计**——T-087 定稿过稿前，T-088~T-091 不得开工。
> **评审（2026-11-01）**：外部专家评审已落盘 `docs/ai-pm-codex-bootstrap-review.md`——**有条件过稿**；6 项 P0 修正 + 3 个新增章节 + 验收补 BOOT-15~25 **已收编进设计稿 V1.1**（章节重排 §〇~§十二）；专家建议的任务编号与本节既有编号重叠，以本节为准（对照见评审记录 §九）。

- [ ] **T-087 Codex Bootstrap 设计稿定稿（用户供稿 → 正式设计稿）**
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **评审输入（2026-11-01）**：专家评审 6 项 P0 修正与 3 个新增章节**已收编进设计稿 V1.1**（`docs/ai-pm-codex-bootstrap-v1.md`，正文含〔评审收编〕标注，逐条对照见 `docs/ai-pm-codex-bootstrap-review.md`）；定稿时基于 V1.1 做最终裁决。其中身份关系不得另起认证协议（对齐 ADR-0001 / ADR-0002 / T-032）；V2 须复用 D3 已有 CodexExecutor（BOOT-25）；新增硬依赖——本条的身份与协议裁决须等 T-032 收敛后进行。
  - **目标**：把用户供稿整理为可实施的正式设计稿（沿用 `docs/ai-pm-codex-bootstrap-v1.md` 演进或另立定稿版），BOOT-01~14 映射为项目验收矩阵编号（续 C 系列）。
  - **要在稿里裁清的点**：
    1. 与 `ai-pm-mcp-executor-positioning-v1.md`（executor-neutral）及 §12「不为单一编码工具深度定制」裁定书的边界——`pm-ai-codex/` 包是 Codex 首发目标还是唯一目标，包结构与 Skill 如何保持 executor-neutral 演进（trae / workbuddy / z-code / dsh 复用面）。
    2. 与 T-080 onboarding URL 的合并/分层关系——`/install/codex` 是否就是 T-080 的分工具变体，两条任务线是否收编为一条。
    3. OAuth 设备授权 vs 现行「lmms 签发绑真人 API Key」链路（runbook §二）的关系与取舍；与 ADR-0001 R1（私有部署 provider）的衔接。
    4. 服务端六对象（device / runner / runner_registration / runner_heartbeat / runner_capability / runner_project_binding）与既有 `core.principal` / `core.tenant_agent` / T-021b `executor_id` 的归位——新表还是挂既有表。
    5. V1 是否含 Runner 二进制安装（还是 V1 仅 Skill+MCP 配置 + 设备注册，Runner 二进制归 V2——影响 BOOT-05 的 V1 判定）。
    6. 跨机前提：Remote MCP 依赖 T-081 `/mcp` 跨机暴露，V1 范围内是同机先行还是等 T-081。
  - **不做**：不改 11 个 MCP 工具契约；不动鉴权三层链（T-081 范围除外）；不顺带实施。
  - **验收**：正式稿含授权时序图、DDL 草案、BOOT→验收编号映射表、V1/V2 范围切分表；用户过稿后方可进 T-088~T-091。

- [ ] **T-088 服务端 Bootstrap API + Device/Runner 注册域（前置：T-087 已过稿）**
  - **做**：六张服务端对象表 + 一次性授权码签发/换取；短期访问凭证 + 刷新凭证颁发；设备授权撤销（撤销后立即阻止新任务领取，在途 Run 受控终止，旧凭证回写被拒）；Runner 心跳与能力上报；项目绑定读写；审计与版本记录。
  - **不做**：不做费用/配额（§12 R4 延后）；不改租户/项目/principal 权限模型（Runner 授权挂既有模型）；不改任务状态机（状态变更仍由平台后端统一处理）。
  - **验收**：BOOT-07/08/12/14 对应用例全绿；撤销授权后「新领取被拒 + 旧凭证回写被拒」两条路径有测试；`cargo test --workspace` 全绿。

- [ ] **T-089 安装包与 Worker 安装程序（前置：T-087 已过稿）**
  - **做**：`pm-ai-codex/` 包结构（plugin 配置 + `.mcp.json` + `skills/` + `manifest.json`）；`install-worker.ps1` / `install-worker.sh`（环境识别、下载、SHA-256 与版本校验、装用户目录、幂等重跑、不覆盖用户已有 Codex 配置、失败可恢复）；`verify-install` 自检脚本。
  - **不做**：不在 URL 或指引内容中传递凭据；不绕过执行端自身的权限审批机制；不做执行端软件本身的安装。
  - **验收**：BOOT-01~05/11/13 对应场景（V1 平台范围按 T-087 裁定）；覆盖「已装过 Worker 重跑」「已有 Codex 配置不被破坏」「下载校验失败中断且可重试」三态。

- [ ] **T-090 Bootstrap Skill + MCP 配置 + 自检闭环（前置：T-088 / T-089 完成）**
  - **做**：`pm-ai-bootstrap` SKILL.md（八步 lifecycle：取包 → 环境识别 → 装 Worker → 连 MCP → 注册 → 绑项目/仓库 → 自检 → Ready）；Remote MCP 配置写入；自检（MCP 连通 / Worker 心跳 / Workspace 创建 / 仓库访问 / 任务读取权限）；`pm-task-execution` / `pm-learn`（对接 T-086）/ `pm-knowledge` 各 SKILL.md 归位。
  - **不做**：不做无人值守调度（T-092）；Skill 不绕过 MCP 工具直写平台。
  - **验收**：BOOT-02/06/09/10；≥1 个真实执行端实测「复制初始化指令 → 全自动初始化 → Ready → `pm_list_tasks` 可读」全程无手工编辑配置文件；未激活/未授权路径给出明确指引而非报错。

- [ ] **T-091 前端两页（图1/图2 原型；前置：T-088 完成）**
  - **做**：「Codex 环境接入」入口卡片（复制初始化指令，含部署地址替换提示）；「开发环境」管理页（设备列表 / 在线状态 / Runner 版本与系统 / 绑定项目 / 最后心跳 / MCP·Worker 健康徽标 / 查看详情 / 撤销授权 / 添加开发环境）。
  - **不做**：不做假数据页（`VITE_USE_MOCK=0` 必须能跑）；不进 Agent 管理页（P9 延后）。
  - **验收**：真库下能看到 T-088 注册的设备；撤销授权 UI 操作后服务端即时生效；`vue-tsc` 0 error；vitest 全绿。

- [ ] **T-092 V2 无人值守 Runner（延后，V1 稳定前不要领取）**
  - **范围**：Runner 常驻运行、平台自主调度 Run、自动创建 Workspace、启动 Codex、收集 Commit 与 Delivery Manifest、回传证据、Worker 自动升级与故障恢复、安装包版本与服务端 API 版本分离 + MCP 能力协商。
  - **重启条件**：V1（T-087~T-091）闭环稳定后另立设计稿；稿中必须裁清与 T-030/T-031（平台侧 `CodexExecutor` 本地子进程）的边界与复用关系。

---

## 11. 技术债与已开放待裁（可搭车，勿单独开切片）

- [ ] **T-051 待裁 #12 执行端预注册档位**：V1 已明确接受横向越权面（持有效 bearer 可领取任意项目任务），档位机制待首批执行端接入后再裁。**保持开放，不主动实施**。**ADR-0002 D5 门控升级：per-project 调用方绑定改为「多租户 / 跨机执行上线前置」（与 T-081 跨机暴露同批）；单机 V1（T-041）不受阻。**
- [ ] **T-052 待裁 #13 验收条目版本化**：`task_acceptance_criterion` 已有 `updated_at`；若 D5 证据锚定需要更强不变性，再开 forward-only 追加迁移。**保持开放，等 D5 需求明确**。
- [ ] **T-053 estimation PRD 未落库**：`docs/ai-pm-estimation-prd-v1.md` 有完整 PRD，但零迁移、零代码（`grep estimation migrations/` 无命中）。属独立大块，需先裁 P0 范围再排期。
- [ ] **T-054 逐字段变更历史缺失**（顾问 P1「Task 变更记录」的剩余部分）：现只有 `task_supersession` 与审计行，没有字段级 diff。先观察 D4/D5 是否需要「回放某次交付时的任务原貌」，需要再开表。
- [ ] **T-055 私有部署认证 provider（ADR-0001 R1）**：客户可能没有 lmms。`core.principal_identity.provider` 维度已预留，加 `provider='local'/'oidc'` 的解析器**不改表结构**；属独立任务，不是 T-021a/b 的一部分。**保持开放，等私有部署立项**。
- [ ] **T-056 私有部署首管理员 bootstrap（ADR-0001 R2）**：现只有 env 指定 `AI_PM_FIRST_ADMIN_TENANT/SUBJECT`，私有部署装完无人可激活。候选：`AI_PM_BOOTSTRAP_FIRST_ADMIN=false`（默认关，私有部署建议开，首个登录者 `is_active=true` + `tenant_admin`）。**与 T-055 同批裁**。
- [ ] **T-057 Git 凭据层级（ADR-0001 R3）**：现为项目级 `integration.project_ssh_key`，与 ADR §6「凭据归属平台或 Tenant」不一致。V1 不动，待首批私有部署需求明确后裁。
- [ ] **T-058 `tenant_admin` 跨项目可见性（ADR-0001 R6）**：与 T-022 一起裁。默认**不可见非成员项目**（仍按 `project_member` 过滤）；若运维需要全租户只读视图，另开审计专用入口，不放宽业务列表。
- [ ] **T-059 MCP 审计三列（T-021a 遗留）**：MCP 工具写 `governance.audit_log` 时补 `actor_identity_id` / `actor_client` / `actor_auth_mode`。阻塞：rmcp 3.8 的 `StreamableHttpService::new(factory)` 工厂闭包不接收 `http::Request`（拿不到 axum extensions），且会话在 `tokio::spawn` 子任务运行（task-local 不继承）。候选方案：① 换/包 transport，把请求扩展带进会话；② 自研极薄 `/mcp` 转发层（解析 JSON-RPC 后直接调 `pm_mcp::tools`，绕开 rmcp 会话）；③ 接受 NULL 并只保留 HTTP 路径归因。**保持开放，等 D3 执行端接入时一并裁**（执行端接入后 MCP 归因才有真实使用方）。**2026-11-01 ADR-0002 移交（专家复审 §五，P1）**：本任务追加 `task_run.execution_mode` 列（`human`/`spawn`，`remote`/`container` 预留）+ `triggered_by_user_id` 可空性核查——该子项是普通列迁移，**不受 rmcp transport 阻塞，可先行**；审计三列仍受 rmcp 阻塞。
- [ ] **T-093 Executor 生命周期管理能力（ADR-0002 专家复审 §五 新建，P2）**：执行器**管理面**能力——Run 执行时间线查询（消费 `executor-events.jsonl`）、执行器实例状态展示、手动管理入口。**不含数据面正确性项**（启动持久化协议 / 子进程回收维持 P0 在 T-032）。前置：T-032 交付后按需立项。

---

## 12. 明确延后 / 已放弃 / 已推翻（不要重新提议）

| 项 | 裁定 | 重启条件 |
|---|---|---|
| **P9 多 Agent 协作**（Agent 角色运行时、任务分解、Agent handoff） | ❌ 团队不造自研多 Agent 运行时；顾问也标「暂缓」 | 出现「单 Agent 无法完成的跨角色编排」真实案例，且先解决上下文与审计归因 |
| **P11 规模化与自动化**（多项目并发调度、配额、自动规划） | 🟡 延后 | 单仓闭环（T-031→T-041→T-043）跑通并稳定 |
| 客户 G1–G4（建项客户必选、客户详情页、可见性、客户级 AI 上下文） | 🟡 延后，**非必须完成项**（G3=可见性已并入 T-022） | 出现「按客户聚合跨项目视图」或「客户维度权限隔离」的真实需求；重启前回 `docs/ai-pm-customer-master-data-v1.md` §2 复核 5 条裁定 |
| 项目 tag（标签）归纳轴 | ❌ 放弃，不实施 | 需先解决 4 条阻塞（AI 分类半成品收编、project 属性直写 SQL 无命令通道、词表跨客户泄露面、与项目隔离原则冲突） |
| 平台内置 AI 编程助手 / 为单一编码工具深度定制 | ❌ 已推翻，不再进入设计讨论 | 无（见 `docs/ai-pm-mcp-executor-positioning-v1.md`「明确被推翻的路线」） |
| 租户费用核算/配额聚合（ADR-0001 R4）、`project_member` 的 human/agent 授权状态机（R5） | 🟡 V1 不做（`task_run.tenant_id` 载体由 T-021b 的 α 阶段 `0019_tenant.sql` 预留，**当前未建**） | 出现租户级计费/配额真实需求；或 D3 接入后发现 agent 授权需要独立生命周期 |
| Project Discovery P2（多仓库关联分析 / 持续认知自动更新） | 🟡 延后（设计稿 §十一 已划定 V1 不做范围） | T-083~T-086 完成且单仓 Discovery 闭环稳定后再评估 |

> 注意：客户 G3（可见性）与 T-022 是**同一件事**；T-022 作为身份切片的自然延伸，客户功能本身（G1/G2/G4）仍延后。

---

## 13. 归档

### T-001 提交客户主数据（0017）与延后裁定 —— 2026-11-01 完成

- **提交**：`1fae9e3` feat(api/web): 客户主数据 0017（23 文件）→ `8afc24a` docs(customer) → `cf454bc` chore(memory) → `5298b8c` chore(todo)；D2.1 遗留用例 `d7519d8`。
- **关键结论**：客户升为一等实体（`code` 部分唯一索引、`placeholder` 存量不自动归并）；建项可挂已有客户；G1–G4 延后但仍可重启。
- **实测**：cargo check 0 error、vue-tsc 0 error、customers 8/8、pm_lists 9/9、run_dispatch 24/24。

### T-020 主体身份 v1 设计稿 —— 2026-11-01 完成

- **提交**：`8dcdcc3` docs(principal): 主体身份 v1 设计依据 → `d6a38e4` docs(identity): ADR-0001 + v1 实施记录回填。
- **目标**：一份可实施的 v1 设计依据 —— 身份映射模型、自动 provision 流程、鉴权矩阵、迁移 DDL 草案、验收矩阵、遗留取舍。
- **依据**：`apps/pm-api/src/auth.rs`（验签 + `PM_MCP_READ` 门）、`migrations/0001_core.sql:31-39`（`core.principal`）、`crates/pm-application/src/pm_write.rs:93`（`owner_actor`，T-021a 后已降为 `#[cfg(test)]` 夹具）、`docs/runbook-login-chain.md`（lmms claims：`sub`/`tid`/`roles`/`permissions`）。
- **已裁定的上位约束（不得推翻）**：
  - **方案 C —— 自动 provision 但默认未激活**：首次见到未知 identity 时创建 `core.principal(kind='human', is_active=false)` + 身份映射行，**不自动授予任何项目成员资格**；管理端显式激活后方可使用。建身份与给权限解耦，`is_active` 是现成开关。
  - 映射键必须是三元组 `(provider, claims_tenant_key, subject)`：lmms 是多租户签发方（`tid` 为强制 claim），只用 `sub` 会在跨租户同名 subject 时串号。
  - 未激活身份一律 fail-closed，错误码沿用 lmms 风格（如 `IAM_PERM_3002`），响应体不泄露存在性。
- **已裁清的四个点**：① 载体 = 独立表 `core.principal_identity`（多 provider 演进，不动 `core.principal`）；② 未激活身份读权限 = 全拒，只留 `GET /v1/pm/me` 自查；③ `owner_actor` 去留 = bootstrap/系统内部路径保留，业务写路径必须换真实 principal；④ 无存量回填（方案 C 自带，映射行随首次登录生成）。
- **不做**：不改代码；不动 D3/D4/D5；不顺带做 RBAC 角色体系（V1 只做身份归因 + 成员资格）。
- **验收**：文档含「字段 → 库内来源」映射表与验收矩阵（编号续 D2 的 C28 之后 → C29–C42）；**用户已过稿**（批准进入 T-021a）。
- **关键结论**：身份映射定在独立表 `core.principal_identity`（三元组唯一）；未知身份自动 provision 但**默认未激活**（方案 C），未激活 fail-closed，只留 `/health` 与 `GET /v1/pm/me`；`owner_actor` 双轨制（请求内真实 principal / 请求外 `ai-pm-dispatch`）；上游 `tid` 落 `claims_tenant_key`（**不是**内部 `tenant_id`，留给 T-021b）。V1 范围 = β（身份归因），Tenant/Agent/Run 身份列归 T-021b。
- **上位裁定**：ADR-0001 Accepted（D1–D6），T-021 拆为 T-021a（β）/ T-021b（α+γ+δ）。

### T-021a 主体身份归因与审计三列（β）—— 2026-11-01 完成

- **提交**：`d6a38e4` docs(identity) → `cb633b8` feat(identity)（26 文件：迁移 `0018` + auth 三层 + `/v1/pm/me` + `PATCH /v1/pm/principals/{id}` + 写路径 actor 改造 + 11 例新测试）。
- **目标**：真实调用者身份贯穿全部 HTTP 写路径与审计；未激活身份 fail-closed。MCP 工具 actor 契约**不变**（D2）。
- **依据**：`docs/ai-pm-principal-identity-v1.md`（本文 = β 范围）+ `docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`。
- **做**：迁移 `0018_principal_identity.sql`（`core.principal_identity` + 审计三列；上游 `tid` 落 `claims_tenant_key TEXT`，**不叫 `tenant_id`**）；auth 中间件 claims → 解析/自动 provision → typed extension（不再只塞裸 `Value`）；新增 `GET /v1/pm/me`；HTTP 写路径把 `owner_actor` 换成真实 principal —— `pm_write.rs` 的 `create_revision`/`approve_revision`/`create_task`/`update_task`、`apps/pm-api/src/pm_writes.rs` 的 5 个 handler（41/156/166/179/190）、`pm_repositories.rs`、`repository.rs` 的 9 个写函数（`ensure_ssh_key`/`generate_ssh_key`/`revoke_ssh_key`/`delete_ssh_key`/`create_repository`/`update_repository`/`delete_repository`/`archive_repository`/`verify_repository`）、`project_import.rs::start_import`（189）与 `advance_import_after_decision`（1095）、`command_executor.rs::system_context`（1628）同口径；审计 `actor_id` 写真实身份。（原清单列的 `project_import_advance.rs` **不在模块树** —— `lib.rs` 未声明，文件不参与编译；实际推进函数是 `project_import.rs::advance_import_after_decision`，由 `supervision` 调用。）
- **MCP 工具不替换 actor**（代码事实：`crates/pm-mcp/src/tools.rs:179-183` 解析项目内 `dev_agent`、`:312-332` 与 `:386-398` 解析 `ai_pm`，**无 `owner_actor` 调用点**）；原计划在审计同时写 `actor_identity_id` / `actor_client` / `actor_auth_mode` —— **未落地，受 rmcp transport 限制，转 T-059**。
- **不做**：不改 `core.principal.kind` 取值集合；不建 `core.tenant` / Agent 模型 / Run 身份列（归 T-021b）；`is_admin` 暂取 token `roles`（≤15 分钟窗口，T-021b 收口）；不做 per-project 调用方绑定（D2 待裁 #12 仍开放，见 T-051）。
- **验收**：`docs/ai-pm-principal-identity-v1.md` §11 的 C29–C42；D2 的 C1–C28 不回归；`cargo test` 全绿。
- **实测**：`cargo check --workspace --all-targets` 0 error；`cargo test --workspace` 退出码 0，64 个 `test result: ok`、0 失败。
- **关键结论**：审计 `actor_id` 100% 有来源（真实 principal，`owner_actor` 退场）；三列 `actor_identity_id` / `actor_client` / `actor_auth_mode` 描述身份与 token 形态；服务 token（`roles=["service"]`）与跨租户 token（`ct=true`）业务面 403 且不 provision；`is_admin` 暂取 token `roles`（≤15 分钟窗口，T-021b 收口为读 `tenant_member.role`）。
- **未落地**：MCP 侧审计三列受 rmcp 3.8 transport 限制 → **T-059**（保持开放，等 D3 执行端接入时一并裁）。

### T-050 迁移文件头注释残留 —— 2026-11-01 完成

- **提交**：随 T-021a 的迁移改动一并修掉（`cb633b8`）。
- **内容**：`migrations/0010_task_state_machine.sql` 头注释「0009_task_state_machine」→「0010_task_state_machine」。纯注释，无行为变化。

### T-021b 多租户基础设施 + Agent 三层 + Run 身份归因（α+γ/δ）—— 2026-11-01 完成

- **提交**：`e4d10d4` feat(tenant)（85 文件）→ `c9f99b8` docs(adr)（ADR-0001 按实施结果修订 + 设计稿分类/矩阵校正）。
- **目标**：`core.tenant` / `core.tenant_member` / 39 张业务表 `tenant_id` 一次建对；Agent 三层；Run 身份字段；租户成员角色的正式写入路径。
- **依据**：`docs/ai-pm-tenant-infrastructure-v1.md`（**用户过稿**，含 11 项裁定）+ `docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`。
- **α（`0019_tenant.sql`）**：
  - 新表 `core.tenant` / `core.tenant_member(role IN owner/admin/member)` / `governance.tenant_audit_log`；系统租户幂等种子 `(provider='system', external_key='default')`。
  - 39 张业务表全部 `tenant_id UUID NOT NULL`；四类划分（A 根表 2 / B 直挂 project 33 / C 锚定父行 3 / D 伪根表 1）。
  - 组合外键：B 类统一 `(tenant_id, project_id) → core.project(tenant_id, id)`；C 类锚定各自父行；**跨表 10 处单列 FK 一并换成组合 FK**（否则两份 tenant_id 可以各自不相同）。
  - **I4 四处**全局唯一改租户内：`core.project.code`、`core.customer.code`、`integration.outbox_event.idempotency_key`、`delivery.policy_scope` 主键。
  - **I5 十五张表**补 `UNIQUE(tenant_id, id)`（其中 5 张今天没有：`requirement_revision` / `task_run` / `document_revision` / `change_proposal` / `approval_request`）——这是 ADR §3.5 示例未覆盖、实施期才发现的结构性缺口。
  - `core.principal_identity` 收敛：删 `claims_tenant_key`，改 `tenant_id UUID`，唯一键 `(provider, tenant_id, subject)`（ADR §7.1 的移交完成）。
- **γ+δ（`0020_agent_run_identity.sql`）**：`ai.agent_definition`（全局三行种子）+ `core.tenant_agent`（租户实例，`UNIQUE(principal_id)`）；`delivery.task_run` 加 `agent_id` / `triggered_by_user_id`（均有 FK）、`executor_id`（可空、无 FK，D6）。`dev_agent` 由 per-project 提为**租户级复用**（`bootstrap::ensure_tenant_agents`），`ai_pm` 仍 per-project。
- **auth 链**：`resolve_or_provision` 先按 `(provider, tid)` upsert `core.tenant`（Q1 镜像上游、自动建即生效），再按三元组查 identity；`is_admin` 每请求查 `core.tenant_member.role`（D5），token `roles` 不再参与授权；env 对命中时首管理员直接 `role='owner'` + 激活（Q2 冷启动唯一入口）。`dev-off` 挂系统租户 owner。
- **Tenant Member Role Management API**（用户补充要求）：`GET/POST /v1/pm/tenants/{tid}/members`、`GET/DELETE .../members/{pid}`、`PATCH .../members/{pid}/role`、`POST .../owner-transfer`。全部走 `apply_member_change`：单事务 → 锁 `core.tenant` 行串行化本租户成员变更 → 校验调用者权限 → 锁目标行 → 校验角色转换 + 最后 owner 保护 → 写 DML → 写 `tenant_audit_log`（同事务，失败即回滚）。
- **写/读路径**：生产 53 处 INSERT 补 `tenant_id`（B 类用父行子查询、A/D 类显式）；测试 200 处 INSERT 同步；对外入口（`pm_lists` / `customers` / `pm_runs` / `pm_writes` / `pm_repositories` / `pm_imports` / `documents` / `pm_artifacts` / `principals`）加租户谓词，内部流转靠组合外键蕴含（Q5 分层）。
- **不做**：Executor Registry（T-030）；费用核算聚合（R4）；Git 凭据层级（R3）；`tenant_agent.is_enabled=false` 的 MCP 领取门（待 D3 执行端接入）。
- **实测**：`cargo test --workspace` 退出码 0（详见「T-021b 交付」的最终数字）；新增 `tenant_members.rs` 11 例（RV-1..RV-10 + TN-4/TN-5）、`tenant_schema.rs` 5 例（XT-3 ×3 + TN-7/TN-8）、`tenant_isolation.rs` 1 例（XT-1/XT-2）。
- **关键结论**：
  - `tenant_member.role` 成为唯一事实源后，撤权在**下一次请求**即生效，不等 token 过期（RV-2/RV-3 用同一张 JWT 验证）。
  - 组合外键使「两份 tenant_id 不一致」在 DB 层不可能出现；`information_schema` 断言把 I1 变成机器可验证的不变量。
  - `admin` 只能**往下降**（可把另一 admin 降为 member，不能提权、不能碰 owner），同时满足用户补充要求第 1 条与第 5 条。
- **遗留**：`tenant_agent.is_enabled=false` 的门控未接（D3 执行端尚无）；`governance.approval_request.project_id` 与 `tenant_id` 之间没有直接组合外键（靠 `(tenant_id, proposal_id)` 传递保证一致），属可接受缺口。

### 文档一致性巡检（todo.md 完成度审计）—— 2026-11-01 完成

- **提交**：`d3b144e` docs(adr)（ADR-0001 事实表 + 前向列表述校正）→ `f7b1ecc` fix(bootstrap)（模块头注释）→ `chore(todo)` → `chore(memory)`。
- **ADR-0001 修正**：F1「17 迁移 / 42 表」→ **18 迁移 / 43 张表**；F2 写清 7 张不带 `project_id` 的表归属（业务 3 + 全局 3 + 租户键表 1），全库 **36 带 / 7 不带**；F3「4 个 principal」→ **5 个**（`bootstrap.rs:172-189`：owner human + `ai_pm` + architect / developer / tester）；§3.4 计数自检「3 + 39 = 42」→ **3 + 39 + 1(`principal_identity`) = 43**；§8 方案 B「42 张表」→ 43；§9 与 §12 R4 的 `task_run.tenant_id` 标为 **T-021b 前向列（当前 `delivery.task_run` 无该列）**。
- **todo.md 修正**：§12 表格「`task_run.tenant_id` 已留载体」→「载体由 T-021b 的 α 阶段 `0019_tenant.sql` 预留，**当前未建**」；T-061 依据 `0010:122-144` → `0010:123-144`（122 是注释行，表体 123-142）。
- **代码注释修正**：`crates/pm-application/src/bootstrap.rs` 模块头写「three `core.principal` / three `core.project_member`」，实际 **5 / 5**（`bootstrap.rs:218-222`）；注释改正。纯注释，零行为变化。
- **实测**：`cargo test --workspace` 退出码 0 —— **64 个 `test result: ok`、401 passed、0 failed、0 ignored、0 panicked**。

### T-041 实施 D4（多仓库交付与结果回写）—— 2026-11-02 完成

- **提交**：`6bbc906` feat(delivery)（16 文件：迁移 0022 + `pm-application::delivery` 三模块 + `git_access` 通道 + `pm_submit_result` 定稿 + 只读接口 + 27 条用例）→ `8b43eca` docs(d4)（设计稿 §9 实施记录 + 迁移台账 + todo 勾选）→ `chore(todo)`（归档）→ `chore(memory)`，四者分开提交（§0.2 第 5 条：一任务一 commit）。
- **目标**：D4 证明「AI 实际交付了什么」——逐仓落 commit + 证据的事实层，任务最终裁决留给 D5。
- **依据**：`docs/ai-pm-slice-d4-delivery-v1.md` V1.1（已过稿，6 项裁定 + 评审收编 4×P0 / 4×P1）。
- **做**：迁移 `0022_delivery_manifest.sql`（`task_run` 补 `UNIQUE (project_id, id)` / `integration_status` / `request_fingerprint`；建 `delivery.task_repository_commit`；建视图 `v_task_delivery_manifest`）；`pm-application::delivery` 单事务管线 ⓪~⑦；`delivery/git_verify.rs` 四项 git 校验；`delivery/manifest.rs` 清单读模型；`pm_submit_result` 定稿（去 `Unsupported`、只加 `integration_status`）；`GET /v1/pm/tasks/{task_id}/delivery` 只读接口；tenant_schema 收编新表（40 → 41）。
- **不做**：不做自动 merge / 推远端；不做审批流改造；不引入新的交付状态机；不做 Run 物理删除。
- **实施期裁定（9 项，详见设计稿 §9.2）**：① `task_repository_commit → task_repository` 组合外键**不成立**（后者主键含 `task_id`，本表无此列），仓归属改由两条 `(project_id, …)` 组合外键保证；② 事务所有权按值传给收尾函数（`finish_rejected` 自行 COMMIT），杜绝半提交路径；③ **可写仓漏报不驳回**——D4 只记事实，视图显示 `partial`，裁决归 D5；④ 落库 `commit_sha` 取**工作区实际 HEAD**，执行端声明只做一致性比对；⑤ `changed_files` 以 Git 现算为准；⑥ 审计 actor 回退到平台 service principal；⑦ 指纹落 Run 级列（覆盖整单）；⑧ 业务驳回不是异常（工具层回 200 + `outcome: "rejected"` + 机器可读 `code`）；⑨ git 失败与超时统一走 `commit_not_descendant`，不扩张错误码表。
- **验收**：`docs/ai-pm-slice-d4-delivery-v1.md` §6 的 C71–C97（27 条）；实测 `cargo test --workspace` 退出码 0 —— **74 个 `test result: ok`、0 failed**；`cargo check --workspace --all-targets` 0 error / 0 warning；D1/D2/D3 既有用例不回归。
- **关键结论**：① 幂等识别**必须在管线最前部**（⓪，先于 fencing）——成功提交后 Run 已非活跃，幂等若放在 UNIQUE 冲突分支就永远不可达；② 校验范围 = **契约仓全集**而非上报集合（否则漏报只读仓即绕过）；③ 三层边界不可越：D3 证明执行过什么、D4 证明交付了什么、D5 才判断是否满足要求；④ `submitted` ≠ 交付完整 ≠ 任务完成。
- **遗留**：`integration_status` 的判读（何时算失败/完成）、`partial` 的裁决、review run 的 `run_kind` 划分 → 全部归 T-043（D5）；交付验收前端页 → T-070；`DeliverySubmitted` 事件的消费端 → T-061。

### T-022 可见性收口（按 `project_member` 过滤）—— 2026-11-01 完成
- **提交**：feat(api) + docs(todo) + chore(memory)，三者分开提交。
- **用户裁定**：① 建项时把调用者写为 `project_member(owner)`（否则新项目对创建者不可见）；② 零项目客户对同租户活跃成员可见（待归类池，不阻塞「先建客户→再建项目」）；③ 收口范围 = 全部 `project_id` 维度读端点。
- **做**：
  - `pm-application::bootstrap` 新增 `bootstrap_project_owned` / `bootstrap_project_for_customer_owned`：真实调用者占 owner 位，不造 `owner-<code>` 合成 principal；既有包装仍走合成路径（测试/导入不变）。
  - `/v1/pm/projects`、`/v1/pm/customers`（列表/详情/PATCH）、`requirements` / `tasks` / `milestones` / `approvals` / `decisions` / `audit-logs` / `task-runs` / `artifacts` / `revisions` 一律按 `core.project_member` 过滤；显式 `project_id` 不可见 → 404。
  - `repositories` / `imports` / `documents` 的项目边界从「租户」升级为「租户 + 成员」；客户可见性谓词提为 `CUSTOMER_VISIBLE`（成员任一项目或零项目）。
- **不做**：不改 MCP 工具面；`pm_writes` 的写端点仍只校验租户（T-022 定义为读端点收口）。
- **验收**：新增 `apps/pm-api/tests/visibility.rs` 3 例（双向成员视角、零项目客户、显式 project_id 404、建项自动 owner）；`cargo test --workspace` 退出码 0。
### T-023 Task 依赖与解锁设计稿 —— 2026-11-01 完成

- **提交**：`9fbb92a` docs(task-dependency)。
- **目标**：前置/后置任务关系可建、可查、可裁决；交付 `docs/ai-pm-task-dependency-v1.md`。
- **依据**：`target.md` §P1「Task 依赖」；实测依赖载体零命中、`task_waiting` 语义不同、`outbox` 仅 `TaskReady` 一个 handler。
- **用户已裁定四项**：D1 独立边表 `delivery.task_dependency`；D2 写边事务内递归 CTE 检环（409）；D3 禁止跨项目（同项目由双复合外键保证）；D4 上游终态同事务解锁下游并投递 `TaskReady`。本稿另定 D5–D8（满足集合 = completed/cancelled/superseded、只给 draft/blocked 建边、建边不改状态、删边不级联改状态）。
- **验收**：状态图、DDL 草案、C43–C58 验收矩阵齐备；**用户已过稿**（指示「请继续」进入 T-024）。

### T-024 实施 Task 依赖 —— 2026-11-01 完成

- **提交**：`583e4f8` feat(task-dependency)（12 文件：迁移 0021 + 模块 + 三处接入 + 端点 + 19 例用例）→ `17f6bd9` docs(task-dependency)（实施记录回填 §10）→ `chore(todo)`（纪律补充 + 归档）→ `chore(memory)`，四者分开提交（§0.2 第 5 条：一任务一 commit）。
- **做**：迁移 `0021_task_dependency.sql`；`pm-application::task_dependency`（`dependencies_satisfied` 唯一判定 / `add` / `remove` / `list` / `unlock_downstream`）；`apply_task_transition` 终态接入级联解锁（`blocked→ready` 走受守卫入口 + 同事务 `TaskReady`，幂等键沿用命令通道约定）；`claim_core` 领取判定（未满足 → `release_reason='dependency_unmet'` 回退避，不判失败）；pm-api 三端点 + T-022 可见性 + `task.dependency.add/remove` 审计；tenant_schema 收编新表（39 → 40）。
- **不做**：不改 13 态状态机取值；不做自动排期/甘特；不做跨项目依赖；不做子任务树。
- **实施期偏差**（详见设计稿 §10.2）：① UI 直写终态路径（`pm_write::update_task`）同样接入解锁，否则「人在 UI 完成任务」不解锁下游；② Model A 依赖未满足沿用 D1「回滚不退避」约定（`TaskNotReady("dependency_unmet")`）；③ `superseded` 无端到端写入方（应用层无取代命令），以 `cancelled` 端到端 + D5 集合单测兜底。
- **验收**：C43–C58 全覆盖（映射见设计稿 §10.3）；`cargo test --workspace` 退出码 0 —— **70 个 `test result: ok`、441 passed、0 failed**；`cargo check --workspace --all-targets` 0 error 0 warning。
- **关键结论**：依赖满足只有一套谓词（调度领取 / 建删边解锁 / 终态级联共用）；解锁产生的状态变更仍走 `apply_task_transition`（审计 action=`update_task_status`、actor=`ai-pm-dispatch`）；`TaskReady` 与迁移同事务写入，重复解锁不产生第二个语义键。
- **追补立项**：**T-025**（并发正确性加固，任务书 `docs/ai-pm-t025-task-dependency-hardening-v1.md`）——441 passed 只覆盖串行语义，并发场景（C59/C60）未验证；依据专家评审 `docs/ai-pm-task-dependency-review.md`（2026-11-01，基础架构 8.5/10）。
### T-030 D3 执行引擎适配器设计稿 —— 2026-11-01 完成

- **提交**：`edf230f` docs(executor)。
- **目标**：定义「能启动、监控、终止执行实例」的端口与适配器契约，交付 `docs/ai-pm-slice-d3-executor-adapter-v1.md`。
- **依据**：`target.md` §P4 十模块表；slice-d §5.9 预留的 `RunExecutor` 端口与 R4 接线；D3 待裁问题表第 4/5 条；`0011` 已预留 `executor_kind`/`executor_session`。
- **六项裁定（用户逐项确认，Q-D3-1…6）**：① 本地子进程 spawn Codex CLI（容器归 P11）；② 工作区不建新表——`{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}`，复用 D1 已写的 `task_run_repository.workspace_path`；③ 日志文件事实源（`logs/stdout.log`/`stderr.log`，路径确定性派生）；④ 双通道续租（执行端 MCP 心跳 + 平台 poll 兜底，防长编译误杀）；⑤ 统一终止管线分终态（SIGTERM→宽限→强杀；取消→`cancelled`、超时→`timed_out`）；⑥ `executor_session = {kind}:{uuid-v4}` + 三层 fail-injection（FakeExecutor / `AI_PM_CODEX_BIN` 假脚本 / `AI_PM_RUN_EXECUTOR` 注册表，缺省 noop）。
- **验收**：状态图、端口类型补全清单、C59–C70 测试矩阵、实施顺序齐备；**四节逐节过稿**（架构组件 / 数据流 / 错误恢复 / 测试计划）。
- **不做**：不写 `task_workspace`/`task_repository_commit`（D4）；不做失败判定与任务状态裁决（D5）；Run 的 `failed`/`timed_out` 只记录事实不动任务状态机。
### T-031 实施 D3 执行引擎 —— 2026-11-01 完成

- **提交**：`6f3ded4` feat(run-executor)（13 文件：端口补全 + 编排器 + CodexExecutor + worker 接线 + 15 例用例）→ `a690e4c` docs(executor)（实施记录回填 §8）→ `chore(todo)` → `chore(memory)`，分开提交。
- **做**：端口补全（`timeout_seconds` Option 化 / `ExecutorSession{run_id, workspace_root}` / `ExecutorProgress::Done{exit_code}` / `workspace_root_from_env`）；编排器 `claim_and_start`/`poll_active_runs`/`cancel_run`/`reconcile_orphans`（Run 终态条件 UPDATE 先到先得）；start 成功后编排器经受守卫入口推进 `claimed→running`（D1 §九问题 4）；start 失败按 B3-1 把任务 `claimed→ready`（经 `apply_task_transition`）+ 基础设施退避（耗尽落 `failed`）；租约续不上 → `terminate(LeaseReclaimed)` + B3-2（`recover_lost_run` 提取为共享封装）；`CodexExecutor`（镜像仓 + worktree checkout `base_commit` + spawn + pid 文件 + 日志落盘 + Unix SIGTERM→宽限→强杀 / Windows 强杀）；worker 第四循环接线 `AI_PM_RUN_EXECUTOR=codex|mock|noop`（缺省 noop）+ 启动孤儿 reconcile。
- **不做**：不写 `task_workspace`/`task_repository_commit`（D4）；不改 D2 工具面；不做失败判定与任务状态裁决（D5）；不做容器隔离（P11）。
- **实施期裁定**（详见设计稿 §8.2）：① Model B spawn 场景无 token 可下发 → 执行端 MCP 心跳通道 V1 不可用，租约由编排器 poll 兜底维持，token 下发随 D4 裁；② running 失联回收由编排器补齐（claim 候选只覆盖 claimed）；③ 失败行受 0010「每 Run 至多一条」约束——首败留痕、重试只递增计数；④ CLI 参数 `AI_PM_CODEX_ARGS` 留白（接入期调优）。
- **验收**：C59–C70 全覆盖 + reconcile/回归 3 例；`cargo test --workspace` 退出码 0 —— **72 个 `test result: ok`、456 passed、0 failed**；`cargo check --workspace --all-targets` 0 error 0 warning；Windows 实机验证假脚本 spawn/收割/强杀与 `file://` 镜像 + worktree checkout。
- **关键结论**：执行实例从「永久 queued」变为可启动、可监控、可终止、可恢复；Run 终态写入全部条件 UPDATE，与执行端 `submit_result` / 失联回收竞态先到先得；任务状态只经 `apply_task_transition`（D3 自身不裁决任务级失败，归 D5）。
### T-040 D4 多仓库交付设计稿 —— 2026-11-01 完成

- **提交**：`6f66305` docs(delivery)。
- **目标**：每仓独立记录基线、Commit、证据，形成可对账的交付清单；交付 `docs/ai-pm-slice-d4-delivery-v1.md`。
- **依据**：target.md §P5 八模块；multi-repo §3.2/§3.3（结果契约与校验规则）/§4.4（DDL 草案）/§4.5（清单表还是视图）；slice-d §三 D4 行 + Q2/Q5 + §15.5 不变量；D2 冻结的 `pm_submit_result` draft（D4 定稿权）。
- **六项裁定（用户逐项确认，Q-D4-1…6）**：① 不建 `task_workspace`（D3 派生规则 + 既有列已承担，只建 `task_repository_commit`）；② 基线防篡改双保险（声明比对快照 + 镜像仓 `merge-base --is-ancestor` 祖先校验 + 回读 author/时间）；③ 文件清单 = 计数落库 + git 现算（`diff --name-status`）；④ 测试证据 draft 原样落 `test_evidence` JSONB；⑤ 交付历史 = per-run 表 + 按 task 聚合视图 `v_task_delivery_manifest`；⑥ `integration_status` 契约补字段 + `task_run` 新列 + 视图算整单三态（裁决归 D5）。
- **验收**：表/列/视图 DDL、七步提交管线、错误码表、C71–C84 验收矩阵齐备；**三节逐节过稿**（数据模型与校验管线 / 契约定稿与错误码 / 测试计划与实施顺序）。
- **不做**：不做 Review 裁决（D5）；不建 task_workspace；不做全局仓库实体（C3 维持）；不做自动 merge/推远端。

### T-043 实施 D5（Review / 失败恢复 / 知识回写）—— 2026-11-02 完成

- **提交**：`333b7e0` feat(review)（迁移 0026 + `pm-application::review` 八模块 + `DeliverySubmitted` 生产/消费端 + 审批 API + 14 条用例）→ `5c8496b` docs(d5)（设计稿 §13 实施记录 + 迁移台账回填）→ `chore(todo)`（本条归档）→ `chore(memory)`，四者分开提交（§0.2 第 5 条：一任务一 commit）。
- **目标**：D5 判断「D4 记录的事实是否满足任务完成条件」——独立 Reviewer Run、失败判定、审批裁决、知识回写，形成可审计的治理闭环。
- **依据**：`docs/ai-pm-slice-d5-review-v1.md` V1.3（已过稿，七题裁定 + 评审 P0×6/P1×4 + 补充 4/5；知识提案改与 Task completed 同事务强一致；迁移号 0024 归 T-060、D5 取 0026；验收矩阵 C98–C137 共 40 条四要素化）。
- **做**：迁移 `0026_delivery_review.sql`（`task_run` 六列 + 三 CHECK + `attempt_no` 按 kind 分轨 + `uq_task_run_active` 收窄 + `uq_task_review_active` + 三新表 + 三触发器 + **交付视图收窄**）；`pm-application::review`（`anchor` 双哈希锚定 / `pipeline` L1–L4 纯函数管线 / `collect` 只读事实采集 + 镜像仓产物探测 + 漂移复核 / `queue` 拍①轻拍 / `worker` 拍②领取循环 / `decide` 完成判定器 + §5.1 矩阵 + 审批裁决单事务 / `knowledge` 确定性模板 + completed 同事务 `draft` 提案 / `failure` B4-A~D）；`DeliverySubmitted` 事件生产端（交付提交管线同事务）+ 消费端（`DeliverySubmittedHandler` → 拍①）；worker 第五/六条循环（review 领取拍 + 失败判定作业）；`GET /v1/pm/review-approvals` + `POST /v1/pm/review-approvals/{id}/decision`；tenant_schema 收编（43 → 44 张）。
- **不做**：不自动修改被审对象；不改 D2 工具面；不注册无消费端的语义 Handler（`ReviewCompleted` / `KnowledgeBaseWriteRequested` 默认不产生，按 `AI_PM_OUTBOX_EVENTS` 开关）；不新建 review_run 表（复用 `task_run` + `run_kind`）。
- **实施期裁定（9 项，详见设计稿 §13.2）**：① `attempt_no` 表级唯一 → 按 kind 分轨（review 第 N 次会与 delivery 第 N 次撞号）；② **D4 视图漏 `run_kind`** → 0026 内重建视图收窄（否则 review run 落库后 D5 会把自己的审查对象判成「未交付」）；③ 三处活跃 Run 计数全部补 `run_kind='delivery'`（否则 review run 被 delivery 循环捡走或挡住 delivery run 创建）；④ §2.5「三新表全禁 UPDATE」与 §5.2「UPDATE 审批行」矛盾 → 审批表改锚定列 once-written + pending 单向落终态触发器；⑤ 拍①幂等判据必须**先于**状态检查（否则重复消费返回 `TaskNotSubmitted`，幂等不可达）；⑥ `manual`/`review` 条目终值行在审批裁决事务内 INSERT `passed`；⑦ `document_proposal.created_by` NOT NULL → service principal + `(project_id, doc_key)` 活跃状态集去重；⑧ 产物探测 `git cat-file -e`，镜像仓缺失即 fail-closed；⑨ `DeliverySubmitted` 因已有真实消费者改为默认开启。
- **验收**：`docs/ai-pm-slice-d5-review-v1.md` §10 的 C98–C137（40 条；本轮逐条建用例 16 条，覆盖见设计稿 §13.3，其余机制已落地、故障注入与并发压测归 T-061 / QA 批次）；实测 `cargo test --workspace` 退出码 0 —— **75 个 `test result: ok`、516 passed、0 failed**；`cargo check --workspace --all-targets` 0 error / 0 warning；D1–D4 既有用例不回归。
- **关键结论**：① **Review 失败 ≠ Run 失败 ≠ Task 失败**——review run 失败只排 `run_kind='review'` 的新 run，失败判定作业扫描谓词结构上排除 review run（C114/C136 已断言）；② 锚定（双哈希 + 策略版本）让「同一输入必得同一输出」可机械验证，漂移即本轮 verdict 不落；③ 知识提案与 completed 同事务——「完成但没有提案」在库层不可表示；④ `manual`/`review` 条目**不落管线裁决行**（只有自动条目有确定性 verdict），人工终值由审批事务补齐。
- **遗留**：`RunCompleted` 生产者、死信重放接口、事件监控 → T-061（`DeliverySubmitted` / `ReviewCompleted` / `KnowledgeBaseWriteRequested` 已搭车）；审批页 / 交付验收页 → T-070；知识索引 → T-060。
