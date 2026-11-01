# TODO — 执行清单

> 本文件是**唯一待办队列**。每条任务自包含，供 AI 直接领取执行。
> 维护规则：状态原地改（`[ ]` → `[~]` → `[x]`），条目只追加不删除；做完的移到 §13 归档。
> 编号只是稳定 ID，**排序以里程碑（§3–§10）为准**，不再按编号段位推断优先级。

**本版（v2，2026-11-01）的合并口径**：

- 依据 A：`target.md` —— 技术顾问**只按产品描述**制定的 12 阶段路线（P0–P11），他看不到本仓库实况。
- 依据 B：仓库实测 —— `migrations/`（0001–0018，18 个迁移 / 43 张表）、`crates/`（5 个包：pm-application / pm-domain / pm-ai / pm-infrastructure / pm-mcp）、`apps/`（3 个：pm-api / ai-pm-worker / pm-worker，合计 8 个 workspace 成员）、`frontend/src/shell/views/` + `frontend/src/workspace/`、`git log`。
- 冲突时**以实测为准**：顾问的路线用来「补漏」，不用来「重排已完成的活」。已实现的部分不再重复开工（§1 逐项给了证据）。
- **2026-11-01 增量**：多租户 / Agent / Executor 身份边界由 `docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`（Accepted）裁定；M1 拆为 **T-021a（β，身份归因）** / **T-021b（α+γ+δ，Tenant + Agent + Run 身份列）**；迁移号顺移（T-024 → `0021`、T-041 → `0022`）；ADR 未决项登记为 T-055~T-058，T-021a 遗留登记为 T-059。**T-020 已过稿、T-021a 已实施并验收**（交付见 §13 归档）。

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
5. **提交纪律**（沿用 D1 的 Q10）：`docs(...)` 与 `feat(...)` **必须分开提交**，不得合并成一个 commit。提交前清场，工作区不得混入无关改动。
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
| P0 项目与仓库基础 | 项目、仓库、多对多关联、成员角色、环境/系统配置 | `0001_core` + `0017`（客户）、`0008`（`project_ssh_key`/`project_repository`/verify）、`bootstrap` 落 5 人编制、`0007`/`0003`（LLM 与技能配置）、`0018`（`core.principal_identity` + 审计三列） | ✅ 基本完成 | 身份归因已落地（T-021a：token → `core.principal` 映射 + 未激活 fail-closed + `GET /v1/pm/me`）；**仍缺租户维度**（全库零 `tenant` 列）与**按成员过滤的可见性** → T-021b / T-022 |
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
> **仍未做**：可见性未按成员过滤（T-022）、MCP 侧审计三列未落地（T-059）、`tenant_agent.is_enabled=false` 的 MCP 领取门（D3 执行端接入时补，见 T-059）。
>
> 架构边界已裁定：`docs/ai-pm-adr-0001-tenant-agent-executor-identity.md`（Accepted，已按实施结果修订）。设计依据：`docs/ai-pm-tenant-infrastructure-v1.md`。

- [ ] **T-022 可见性收口（T-021a / T-021b 已完成，待开工）**
  - **目标**：把「按 `project_member` 过滤」从「全局可见」改为真实按成员过滤（客户 G3 与本文同源）。
  - **依据**：`docs/ai-pm-customer-master-data-v1.md` §2 裁定 5 与 §6 G3。
  - **做**：客户列表 / 项目列表按当前 principal 的 membership 过滤；无成员关系的资源返回「不存在」而非「无权限」（不泄露存在性）。
  - **不做**：不重开 tag 轴（已放弃）；不做客户级 AI 上下文（G4，v2）。
  - **验收**：非成员访问他人客户/项目返回 404；成员访问正常；`pm_lists` / `customers` 测试补齐多身份场景。

---

## 5. M2 · Task 依赖与解锁（补 P1 的唯一硬缺口）

- [ ] **T-023 出设计稿 `docs/ai-pm-task-dependency-v1.md`**
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **目标**：前置/后置任务关系可建、可查、可裁决。
  - **依据**：`target.md` §P1「Task 依赖：前置任务、后置任务、依赖关系」；实测 `grep task_dependency|depends_on|blocked_by` **零命中**，`delivery.task_waiting`（`0010:90`）只存等待原因与解锁条件，没有依赖边；`outbox` 只有 `TaskReady` 一个处理器。
  - **要在稿里裁清的点**：
    1. 载体：独立边表 `delivery.task_dependency(task_id, depends_on_task_id)` vs 复用 `task_waiting`（后者是「等什么」的状态，语义不同，倾向独立表）。
    2. 环检测时机：写边时校验 vs 调度时探测（写时校验可给出明确 409）。
    3. 跨项目依赖是否允许（V1 建议禁止，与项目隔离原则一致）。
    4. 解锁触发：终态时发 `TaskReady` 事件 vs 调度器轮询——选前者可复用现成 outbox 幂等。
  - **不做**：不做子任务树/工作项分解（P9 范畴）；不做甘特排期与关键路径。
  - **验收**：含状态与失败路径图、边表 DDL 草案、验收矩阵；用户过稿后才可进 T-024。

- [ ] **T-024 实施 Task 依赖（待 T-023 过稿）**
  - **做**：迁移 `0021_task_dependency.sql`；建/删边的接口或命令通道；领取前判定「有未完成前置 → Skip 并写 `release_reason`」；上游完成时投递下游 `TaskReady`（走 outbox，不得旁路插入 run）。
  - **不做**：不改 13 态状态机取值；不做自动排期。
  - **验收**：用例覆盖「建边」「环拒绝」「跨项目拒绝」「前置未完成时领取 Skip」「前置完成解锁下游」「重复事件不产生第二个活跃 run」。

---

## 6. M3 · Codex 执行引擎（D3）

- [ ] **T-030 出设计稿 `docs/ai-pm-slice-d3-executor-adapter-v1.md`**
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **目标**：定义「能启动、监控、终止执行实例」的端口与适配器契约，以及 run 在 `claimed → running → 终止` 的合法迁移。
  - **依据**：`target.md` §P4 模块表（RunExecutor / Codex CLI Adapter / Workspace / Git Worktree / Checkout / Process / Log / Timeout / Cancel / Recovery）；`docs/ai-pm-slice-d-execution-pipeline-v1.md` §三/§九；`crates/pm-application/src/run_context.rs`；`0011_run_dispatch.sql:39-40`（`executor_kind`/`executor_session` 列已存在，D3 只填值）。
  - **要在稿里裁清的点**：
    1. 进程启动方式：本地子进程 vs 容器 vs 远程 MCP 客户端（现网是「执行端自己拉 MCP」，三者成本差异大）。
    2. 工作区：D3 只造运行期目录，还是直接落 `task_workspace`（后者属 D4，写清边界与命名约定）。
    3. 日志落点：文件 / 表 / SSE 流式。
    4. 心跳与租约续期责任方：复用 `renew_lease_by_token` 还是执行器自管。
    5. 终止语义：SIGTERM 宽限期 → 超时强杀 → run 落哪个终态；用户取消与超时是否同一路径。
    6. `executor_session` 标识格式；假执行端（fail-injection）注入点。
  - **不做**：不写 `task_workspace`/`task_repository_commit`（D4）；不做失败判定与任务状态裁决（D5）；不实现仓库级策略。
  - **验收**：含状态机图 + 端口 trait 签名 + 测试计划（含假执行端）；用户过稿后才可进 T-031。

- [ ] **T-031 实施 D3（前置：T-030 已确认）**
  - **做**：`pm-application` 执行器端口 + `CodexExecutor` 适配器；run 状态迁移走 `apply_task_transition`（不得旁路）；填 `executor_kind`/`executor_session`；启动/终止/超时均写审计（真实 actor 依赖 T-021a；`executor_id` 列已由 T-021b 预留，Registry 本阶段建）。
  - **不做**：不越界到 D4/D5；不改 D2 工具面契约字段。
  - **验收**：假执行端跑通「启动→心跳→正常结束」与「启动→超时→强制终止」两条路径；`cargo test` 全绿；D1/D2 用例不回归。

---

## 7. M4 · 交付与验收（D4）

- [ ] **T-040 出设计稿 `docs/ai-pm-slice-d4-delivery-v1.md`**
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **目标**：每个仓库独立记录基线、Commit、变更清单、验证证据，形成可对账的交付清单（target.md §P5 Delivery Manifest 字段表）。
  - **依据**：`docs/ai-pm-multi-repo-model-v1.md`（ER + P0-2/P2-1 里程碑）、`docs/ai-pm-slice-d-execution-pipeline-v1.md` §D4、「只读仓出现 commit 即整单驳回」的裁定；现状 `0011` 只有 `task_repository` / `task_run_repository`。
  - **要在稿里裁清的点**：`task_workspace` 与 `task_repository_commit` 的列集；基线唯一基准（D1 锁定的 `base_commit`）如何防事后篡改；文件清单存全量还是 diff 摘要；测试证据的形态（命令+结果+时间 vs 产物路径）；「交付历史」视图按 run 还是按仓库聚合。
  - **不做**：不做 Review 裁决（D5）；不追求跨仓原子事务（多仓各自提交，只做「整单一致性」判定）。
  - **验收**：含 DDL 草案、对账规则、验收矩阵（建议 C 系列续编号）；用户过稿后才可进 T-041。

- [ ] **T-041 实施 D4（前置：T-040 已确认）**
  - **做**：迁移 `0022_delivery_manifest.sql`；`pm_submit_result` 的交付段落两表；Baseline Validation（base_commit 与仓库实际 HEAD 对账）；交付清单只读接口（前端页在 T-070）。
  - **不做**：不做自动 merge/推远端；不做审批流改造。
  - **验收**：三仓任务可插入完整链路并逐仓对账；基线漂移被拒；只读仓出现 commit 整单驳回；`cargo test` 全绿。

---

## 8. M5 · Review、失败恢复与知识回写（D5）

- [ ] **T-042 出设计稿 `docs/ai-pm-slice-d5-review-v1.md`**
  - **本条按 §0.1 执行**：完整 brainstorming。
  - **目标**：形成可审计的闭环——失败判定（B4）、独立 Review Run、知识回写。
  - **依据**：`target.md` §P7 五条边界（Reviewer 是独立 Run、默认只读、Review 失败 ≠ Run 失败、必须区分「需要审批」与「已经审批」、高层强制审批不可被任务级放宽——最后一条 D2.1 已实现）；`delivery.task_acceptance_criterion`（`0015`，逐条留痕已建表）；`0014` 知识提案/质量门（回写只能走提案，不得直写）。
  - **要在稿里裁清的点**：失败判定的输入（退出码/测试证据/超时 vs 人工驳回）与终态映射；Reviewer Run 是否复用 `task_run` 还是独立表；四值结论（Approved/ChangesRequested/Inconclusive/Failed）如何落到 task 状态；逐条验收裁决的存储与「结论≠证据」的区分；知识回写的触发点与去重。
  - **不做**：不做多 Agent 协作；不做自动修复（ChangesRequested 之后由人/新 run 处理）。
  - **验收**：含状态图、裁决表、验收矩阵；用户过稿后才可进 T-043。

- [ ] **T-043 实施 D5（前置：T-042 已确认）**
  - **做**：失败判定服务；Reviewer Run 编排（只读，写 findings）；逐条 `task_acceptance_criterion` 裁决留痕；完成/失败后的知识回写（走 `knowledge_proposal`）；投递 `ReviewCompleted`/`KnowledgeBaseWriteRequested` 事件。
  - **不做**：不自动修改被审对象；不改 D2 工具面。
  - **验收**：两条闭环用例（交付通过→completed + 知识提案；交付不通过→changes_requested 且原 run 不被判失败）；`cargo test` 全绿。

---

## 9. M6 · 知识索引与 Outbox 补齐（可与 M4/M5 搭车）

- [ ] **T-060 索引层可替换化 + 完整检索（P6 后半）**
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
  - **验收**：四条事件均有生产端与消费端；重复投递不产生副作用；死信重放幂等；监控页或指标可读。

---

## 10. M7 · 工作台补齐（P10 的两个真空）

- [ ] **T-070 执行中心接真数据 + 交付验收页（前置：T-031 / T-041）**
  - **目标**：把 `workspace/:projectId/execution` 从骨架换成真实 run 数据；新增交付验收视图（Commit / 测试证据 / 审批结论）。
  - **依据**：`GET /v1/pm/task-runs`（`apps/pm-api/src/pm_runs.rs`，现只有读端点）、`target.md` §P10「AI 执行中心 / 交付验收」两行、`docs/ai-pm-task-detail-page-v1.md`（页面契约）。
  - **做**：run 列表与详情（状态、租约、executor_session、日志入口）；交付清单页（多仓 commit + 证据）；空态与错误态。
  - **不做**：不做假数据页（`VITE_USE_MOCK=0` 必须能跑）；不做 Agent 管理页（P9 延后）。
  - **验收**：真库下能看到 T-031 产生的 run 与 T-041 产生的交付；`vue-tsc` 0 error；vitest 全绿。

---

## 11. 技术债与已开放待裁（可搭车，勿单独开切片）

- [ ] **T-051 待裁 #12 执行端预注册档位**：V1 已明确接受横向越权面（持有效 bearer 可领取任意项目任务），档位机制待首批执行端接入后再裁。**保持开放，不主动实施**。
- [ ] **T-052 待裁 #13 验收条目版本化**：`task_acceptance_criterion` 已有 `updated_at`；若 D5 证据锚定需要更强不变性，再开 forward-only 追加迁移。**保持开放，等 D5 需求明确**。
- [ ] **T-053 estimation PRD 未落库**：`docs/ai-pm-estimation-prd-v1.md` 有完整 PRD，但零迁移、零代码（`grep estimation migrations/` 无命中）。属独立大块，需先裁 P0 范围再排期。
- [ ] **T-054 逐字段变更历史缺失**（顾问 P1「Task 变更记录」的剩余部分）：现只有 `task_supersession` 与审计行，没有字段级 diff。先观察 D4/D5 是否需要「回放某次交付时的任务原貌」，需要再开表。
- [ ] **T-055 私有部署认证 provider（ADR-0001 R1）**：客户可能没有 lmms。`core.principal_identity.provider` 维度已预留，加 `provider='local'/'oidc'` 的解析器**不改表结构**；属独立任务，不是 T-021a/b 的一部分。**保持开放，等私有部署立项**。
- [ ] **T-056 私有部署首管理员 bootstrap（ADR-0001 R2）**：现只有 env 指定 `AI_PM_FIRST_ADMIN_TENANT/SUBJECT`，私有部署装完无人可激活。候选：`AI_PM_BOOTSTRAP_FIRST_ADMIN=false`（默认关，私有部署建议开，首个登录者 `is_active=true` + `tenant_admin`）。**与 T-055 同批裁**。
- [ ] **T-057 Git 凭据层级（ADR-0001 R3）**：现为项目级 `integration.project_ssh_key`，与 ADR §6「凭据归属平台或 Tenant」不一致。V1 不动，待首批私有部署需求明确后裁。
- [ ] **T-058 `tenant_admin` 跨项目可见性（ADR-0001 R6）**：与 T-022 一起裁。默认**不可见非成员项目**（仍按 `project_member` 过滤）；若运维需要全租户只读视图，另开审计专用入口，不放宽业务列表。
- [ ] **T-059 MCP 审计三列（T-021a 遗留）**：MCP 工具写 `governance.audit_log` 时补 `actor_identity_id` / `actor_client` / `actor_auth_mode`。阻塞：rmcp 3.8 的 `StreamableHttpService::new(factory)` 工厂闭包不接收 `http::Request`（拿不到 axum extensions），且会话在 `tokio::spawn` 子任务运行（task-local 不继承）。候选方案：① 换/包 transport，把请求扩展带进会话；② 自研极薄 `/mcp` 转发层（解析 JSON-RPC 后直接调 `pm_mcp::tools`，绕开 rmcp 会话）；③ 接受 NULL 并只保留 HTTP 路径归因。**保持开放，等 D3 执行端接入时一并裁**（执行端接入后 MCP 归因才有真实使用方）。

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
