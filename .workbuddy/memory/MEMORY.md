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
  - G3 按 `project_member` 过滤未实现（auth 层只验签，principal 未映射到 `core.principal`）
  - G4 客户级 AI 上下文（术语表/验收偏好/红线）→ v2
- **重启条件**：需要「按客户聚合跨项目视图」或「客户维度权限隔离」的真实需求时再启动；
  重启前回到 `docs/ai-pm-customer-master-data-v1.md` §2 重新确认 5 条裁定是否仍成立。

## 项目 tag（标签）归纳轴 — 放弃

- **裁定（2026-11-01）**：放弃。不实施，无代码/迁移/文档残留。
- **理由留档**（若日后重启需重新面对）：AI 已有分类产出 `DraftProfile.domain`/`tech_stack` 半成品；
  project 属性当前直写 SQL 不经命令通道；全局自由文本词表有跨客户泄露面；
  跨项目按 tag 聚合与「项目隔离」原则冲突（与 G3 同源）。
- **术语约定**：项目级叫 tag，工作项级叫 category（`estimation.estimate_item.category`，PRD 未落库）。
- 与客户维度正交：客户答「活是谁的」，tag 答「活是什么性质」。

---

## 当前主路径（2026-11-01）

客户后续工作让位后，主路径回到：

1. **G3 主体身份**（principal 映射到 `core.principal`）——客户视图可见性与 tag 聚合都卡在这里，
   但它本身是 auth 层的独立切片，不必绑客户功能一起做。
2. 其余按既有 plans 目录推进：`docs/superpowers/plans/`（2026-10-31 系列）。

---

## 待办队列：`todo.md`（仓库根目录）

- **建立（2026-11-01）**：用户要求「增加 todo.md，把要做的事情写入，我会安排 AI 去读，然后用目标来执行」。
- **定位**：项目**唯一**待办队列，取代此前「无集中 backlog、靠文档状态与记忆约束」的隐式状态。
- **结构**：§0 读取协议（一次领一条、状态机、交付三条定义、docs 与 feat 分开提交）→ §1 状态快照 →
  §2 P0 清场（T-001 客户 0017 提交）→ §3 主线主体身份（T-020 设计稿 / T-021 实施 / T-022 可见性）→
  §4 D3（T-030 / T-031）→ §5 D4/D5（T-040 / T-041）→ §6 技术债与开放待裁（T-050~T-053）→
  §7 明确延后/放弃（防重新提议）→ §8 归档。
- **任务编号约定**：`T-0X0` 段位即优先级（001 清场 / 02x 身份 / 03x D3 / 04x D4D5 / 05x 技术债）。
- **方案 C（主体身份）已裁决（2026-11-01）**：未知 JWT subject → 自动 provision 为
  `core.principal(kind='human', is_active=false)`，不自动授予成员资格；映射键为
  `(provider, tenant_id, subject)` 三元组（lmms 多租户，`tid` 必带，只用 `sub` 会跨租户串号）。
- **维护规则**：状态原地改、条目只追加不删、完成后移 §8 归档并回填 commit。

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
