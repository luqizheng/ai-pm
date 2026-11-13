# AI-PM 项目长期记忆

> 维护规则：append / 原地修订，不整行覆盖；每条决策写明「结论 + 依据 + 何时可重启」。
> 当日流水见 `2026-*.md`。

---

## 客户主数据 v1（迁移 0017）— 后续工作延后

- **裁定（2026-11-01）**：客户功能**已实施部分保留、不回滚**；**后续工作（G1–G4）延后，属非必须完成项**，
  不排入当前主路径。用户原话：「客户这个文档，暂时延后，他不是必须完成的工作」。
- **已落地（保留）**：迁移 `0017_customer_master_data.sql`、`pm-api` `/v1/pm/customers`、
  项目列表客户列与筛选、建项向导客户下拉。验证已通过（cargo check 0 error、customers 8/8、
  pm_lists 9/9、vue-tsc 0 error、vitest 518 全绿）。
- **延后项（非阻塞、非必做）**：
  - G1 建项客户可留空 → 落 `placeholder`，UI 显示「未归类 · <code>」
  - G2 客户详情页未做（项目列表客户筛选等价替代）
  - G3 按 `project_member` 过滤未实现 —— **原因已变**：principal 映射（T-021a）已落地，
    剩下的是「按成员过滤列表」本身，属 `todo.md` 的 **T-022**（与客户 G3 同一件事）。
  - G4 客户级 AI 上下文（术语表/验收偏好/红线）→ v2
- **重启条件**：需要「按客户聚合跨项目视图」或「客户维度权限隔离」的真实需求时再启动；
  重启前回到 `docs/governance/ai-pm-customer-master-data-v1.md` §2 重新确认 5 条裁定是否仍成立。

## 项目 tag（标签）归纳轴 — 放弃

- **裁定（2026-11-01）**：放弃。不实施，无代码/迁移/文档残留。
- **理由留档**（若日后重启需重新面对）：AI 已有分类产出 `DraftProfile.domain`/`tech_stack` 半成品；
  project 属性当前直写 SQL 不经命令通道；全局自由文本词表有跨客户泄露面；
  跨项目按 tag 聚合与「项目隔离」原则冲突（与 G3 同源）。
- **术语约定**：项目级叫 tag，工作项级叫 category（`estimation.estimate_item.category`，PRD 未落库）。
- 与客户维度正交：客户答「活是谁的」，tag 答「活是什么性质」。

## Worker 容量感知调度 — 放弃（1.0 不做）

- **裁定（2026-11-01）**：放弃。worker 上报 CPU/内存供服务做负载感知并行调度的提案，1.0 不做。
- **提案内容（已 brainstorming 收敛但未落盘）**：Pull + 容量限流模型；独立平台级 `delivery.worker_node`
  表（容量 cpu/mem + 实时 cpu%/已用mem + 心跳，不带 tenant_id）；双通道上报（Model B 直接 upsert、
  Model A 新增 `pm_report_worker_capacity` MCP 工具）；`claim_once` 顶部插容量闸（静态上限 env
  `AI_PM_WORKER_MAX_RUNS` 默认 4 + 动态软闸 cpu>85%/mem>0.9 + 新鲜度窗）；仅限 Model B 自主领取。
- **5 问全走推荐项**，用户在确认前 NACK。无任何代码/迁移（`0022` 未建）/文档残留。
- **重启条件**：未来某版本真正需要「服务按 worker 负载决定并行派几个」时再提；重启前须重新走
  `todo.md` §0.1 brainstorming，且需先确认 D3 执行引擎（T-030）与 Executor Registry 状态（ADR-0001 ④）。

---

## 多租户 / Agent / Executor 身份边界 — ADR-0001（Accepted 2026-11-01）

- **裁定**：V1 建 `core.tenant` + `core.tenant_member`，租户业务数据加 `tenant_id`（用户选方案①）；
  架构边界见 `docs/adr/ai-pm-adr-0001-tenant-agent-executor-identity.md`（D1–D6）。
- **六条实施边界（不得推翻）**：①不机械扩散 `tenant_id`（全局定义/字典/系统配置不加）；
  ②`agent_definition` 全局可复用、`tenant_agent` 租户实例、项目授权复用 `project_member`；
  ③Agent 用 attribution-only principal，**不发独立凭据**，不改真人 API Key 与 MCP 认证协议；
  ④Executor Registry 移出 V1（归 T-030/D3），Run 只预留可空 `executor_id`；
  ⑤`tenant_member.role` 是租户成员角色**唯一事实源**，token `roles` 只作提示，管理员操作必须查库；
  ⑥Run 记 `agent_id` / `triggered_by_user_id` / 可空 `executor_id`，`created_by`/`approved_by` 不重复写 Run。
- **「这批 codex 属于谁」**：归属 = **使用那把 key 的真人**（上游保证 API Key 必绑 user）；
  Agent 不构成独立认证主体，代理是执行主体、真人是授权/操作主体，两者都留在审计里。
- **「有 key 就被系统调度」在代码上不成立**：Model A 是显式拉取式（`pm_claim_task` 带 task_id），
  Model B 才受 `AI_PM_RUN_DISPATCH` 门控，D3 零实现。
- **T-021 拆分**：T-021a（β 身份归因，**已完成**）→ T-021b（α+γ+δ，**待开工**）。
  α 阶段**必须先出逐表迁移方案供评审**（39 张表逐条 DDL + 迁移序号 + 组合外键 + 隔离/撤权测试），不许一次性机械改造。
- **私有部署**：用户问过「有没有必要继续多租户，私有部署怎么办」——ADR 采用「部署 = 租户」的折中：
  单租户私有部署时只有一行 `core.tenant`；非 lmms 的认证 provider 留 T-055，首管理员 bootstrap 留 T-056。

---

## 当前主路径（2026-11-01 更新）

1. **T-021b**（Tenant 实体 + `tenant_id` + Agent 三层 + Run 身份列，α+γ+δ）——T-021a 已落地，
   α 阶段**先出逐表迁移方案供评审**再改表；是 T-030/T-031 执行引擎落库的前提。
2. **T-022** 可见性收口（按 `project_member` 过滤）——客户 G3 同源，依赖 T-021a 已完成。
3. **T-023/T-024** Task 依赖（零表零列，是 P1 唯一硬缺口）。
4. 其余按 `todo.md` 的里程碑顺序推进：D3（T-030/031）→ D4（T-040/041）→ D5（T-042/043）→
   T-060/T-061（**均已完成 2026-11-02**）→ T-070（执行中心接真数据 + 交付验收页）；
   P9/P11 维持不做。
5. **M8 执行端接入（§10.5，2026-11-01 用户新增）**：T-080 URL 引导式接入（粘贴 URL +「帮我安装」，
   须走 §0.1 brainstorming）→ T-081 `/mcp` 跨机暴露（G1 loopback 现状）/ T-082 `pm_list_tasks`；
   依据 `docs/executor/ai-pm-executor-onboarding-runbook-v1.md`；候选执行端 trae/codex/workbuddy/z-code/dsh 均支持 MCP。
   与 D3 的关系待排：T-081 跨机是 T-080 跨机场景前置，档位机制仍归 T-051。

---

## 待办队列：`todo.md`（仓库根目录）

- **建立（2026-11-01）**：用户要求「增加 todo.md，把要做的事情写入，我会安排 AI 去读，然后用目标来执行」。
- **定位**：项目**唯一**待办队列，取代此前「无集中 backlog、靠文档状态与记忆约束」的隐式状态。
- **结构（v2，2026-11-01）**：§0 开工前置（§0.1 强制 brainstorming + §0.2 读取执行协议）→ §1 顾问路线 ↔ 实测状态 →
  §2 排序理由 → §3 M0 清场 → §4 M1 主体身份与多租户边界（T-020/T-021a/T-021b/T-022）→
  §5 M2 Task 依赖 → §6 M3 D3 → §7 M4 D4 → §8 M5 D5 → §9 M6 索引与 Outbox → §10 M7 工作台 →
  §11 技术债与开放待裁（T-050~T-059）→ §12 明确延后/放弃 → §13 归档。
- **任务编号约定（v2 改）**：编号只是**稳定 ID**，**排序以里程碑（§3–§10）为准**，不再按段位推断优先级。
- **方案 C（主体身份）已裁决并实施（2026-11-01，T-021a）**：未知 JWT subject → 自动 provision 为
  `core.principal(kind='human', is_active=false)`（未激活 fail-closed），不自动授予成员资格；
  映射键为 `(provider, claims_tenant_key, subject)` 三元组——落库列名是 `claims_tenant_key`
  （上游 `tid` 原文），**不是**内部 `tenant_id`（内部租户主键留给 T-021b）。
- **维护规则**：状态原地改、条目只追加不删、完成后移 §13 归档并回填 commit。

---

## 工作流程惯例：开工前先 brainstorming（2026-11-01）

- **用户裁定**：「任务开始前都要按照 superpower 的 /brainstorming 来执行。」已写入 `todo.md` §0.1 作为**硬门禁**。
- **技能实体**：`superpowers:brainstorming` v4.0.3，位于
  `~/.workbuddy/plugins/marketplaces/codebuddy-plugins-official/external_plugins/superpowers/skills/brainstorming/SKILL.md`；
  入口命令 `/brainstorm`（`commands/brainstorm.md`，`disable-model-invocation: true`，需显式调用）。
- **六步流程**：先看现状 → 一次一问（优先多选）→ 2–3 方案带权衡（先推荐）→ 分节 200–300 字逐节确认 →
  YAGNI 砍非必需 → 定稿落 `docs/ai-pm-<topic>-v1.md` 并 `docs(...)` 单独提交 → 确认后才进实施。
- **豁免判据**：不需要在两个以上方案间选择的任务（T-001 提交、T-050 注释修复）可跳过流程，但不得跳过读依据。
- **路径适配**：superpowers 默认落 `docs/plans/YYYY-MM-DD-<topic>-design.md`；本项目设计依据统一走 `docs/` 根，
  实施计划才走 `docs/superpowers/plans/`。已在该节注明，避免后续执行者按默认路径另建目录。

---

## S01 前端四切片（`docs/ui/s01/`）— 前端已落地，后端待同事实现（2026-11-04）

- **分支 / worktree**：`feat/s01-frontend-design` @ `D:/kudi-project/templates/ai-pm-s01-frontend`（从 master `c645031` 起）。
  ⚠️ 仓库有**并行会话**在动 master 与 worktree 注册，提交前须 `git worktree list` 复核。
- **依据**：`docs/ui/s01/` 四份**已冻结** UX 规范（S01-01 基线确认 / S01-02 项目创建 /
  S01-03 仓库管理 / S01-04 AI-PM 认知）。这四份是**上位设计依据**，实现不再重新设计。
- **分工裁定（用户）**：初选「全覆盖 + 同步实现后端」，后改「**后端由同事加，我出清单**」→
  转为契约分工，前端按契约并行。
- **已落地（4 提交）**：
  - `ff68bf2` `docs/s01-backend-api-requirements-v1.md` —— 后端需求清单（已有端点清单 + 代码事实 + 6 项待裁决）
  - `1a3e281` 前端契约层 `api/types/initialization.ts` + `api/initialization.ts`
  - `b748ac1` 四切片页面 + `s01` 命名空间 i18n（zh-CN/en）+ 19 条 vitest
  - `4dffa41` `docs/s01-frontend-integration-guide-v1.md` —— 联调说明（端点映射/错误码/批次顺序）
- **验收基线**：vue-tsc 0 error · eslint 0 warning · vitest 96 文件/561 例全绿 · PRISM token 门禁过 ·
  i18n 双语 1431 key 对齐。
- **后端待办（清单 §7 六项裁决）**：Q1 确认权判定（**推荐 A：复用 `project_member.role='owner'`**，
  S02 Approver 表落地后替换）· Q2 Repository role 改造（**推荐新建 v2 表**，现 CHECK 是
  `main/docs/service/infra` 与 UX 的 `Primary/Supporting/Infrastructure/Test` 不符）· Q3 迁移号
  （**0036** 起，0029 空号）· Q4 `core.project.customer_id` NOT NULL 是否放开 · Q5 批量认知确认
  **不提供**后端端点（前端逐条提交）· Q6 Cognition 启动**不走 MCP**（仅 REST）。
- **前端刻意不做的**：初始化工作空间页、项目 profile 页、基线历史页、知识变更提案页（API 层已封装，
  契约已定，后端就绪后补视图即可）。
