# TODO — 执行清单

> 本文件是**唯一待办队列**。每条任务自包含，供 AI 直接领取执行。
> 维护规则：状态原地改（`[ ]` → `[~]` → `[x]`），条目只追加不删除；做完的移到 §8 归档。
> 最后核对：2026-11-01 10:56（对照 `migrations/`、`crates/`、`apps/`、`git log` 实测）

---

## 0. 开工前置流程（AI 必须先读这一节）

### 0.1 强制：任何任务开工前先走 superpowers `brainstorming`

> **用户裁定（2026-11-01）**：「任务开始前都要按照 superpower 的 /brainstorming 来执行。」
> 触发命令：`/brainstorm`（等价于调用 `superpowers:brainstorming` 技能）。

**适用范围（必须走）**：新增功能、新建组件、改动行为、新增迁移、新增接口、改变既有契约、以及**任何需要做设计决策的任务**——包括本清单里标了「出设计稿」的 T-020 / T-030 / T-040。

**豁免范围（可跳过，但不得跳过「读依据」）**：纯机械动作，不产生任何设计决策的任务——T-001（按既定内容提交）、T-050（修一行注释）。判据：**如果做这件事不需要在两个以上方案之间选择，就不必 brainstorm**。

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
# 后端（需先启动 dev 库）
docker compose -f docker-compose.dev.yml up -d
DATABASE_URL=postgres://aipm:aipm@127.0.0.1:5433/aipm bash scripts/cargo-test.sh
cargo check --workspace --all-targets

# 前端
cd frontend && npx vue-tsc --noEmit --skipLibCheck && npx vitest run
```

> 沙箱注意：子进程写 `%TEMP%` 可能 `os error 5`，跑测试前把 `TEMP`/`TMP` 指到工作区内目录（如 `target/testtmp`）。

---

## 1. 状态快照（2026-11-01）

| 切片 | 内容 | 状态 |
|---|---|---|
| D1 | Run 调度与领取（`0011`/`0012`/`0013`） | ✅ 完成，22+4 用例绿 |
| D2 | MCP 任务上下文契约（`0014`→实为 `0015`） | ✅ S1–S3 全部完成，G5/G6 已收口 |
| D2.1 | 四层策略合成（内容寻址 `composed-v1`，`0016`） | ✅ 完成 |
| — | 客户主数据（`0017`） | ✅ 已提交（`1fae9e3`/`8afc24a`）；后续 G1–G4 延后 |
| D3 | Codex 执行适配器 | ⬜ 未开始（本文档 T-030/T-031） |
| D4 | 多仓库交付与结果回写 | ⬜ 未开始 |
| D5 | Review / 失败恢复 / 知识回写 | ⬜ 未开始 |
| — | 主体身份（G3 principal 映射） | ⬜ 未开始，方案 C 已裁定（T-020~T-022） |

---

## 2. P0 · 清场（15 分钟，必须先做）

- [x] **T-001 提交客户主数据（0017）与延后裁定**
  - **目标**：工作区干净，`git status` 无残留；客户的已落地部分入库留痕，延后裁定有据可查。
  - **依据**：`docs/ai-pm-customer-master-data-v1.md`（头部状态已改 🟡）、`.workbuddy/memory/MEMORY.md`「客户主数据 v1 — 后续工作延后」
  - **做**：按三层拆成 3 个 commit —— ① `feat(api/web): 客户主数据 0017`（`migrations/0017_*.sql`、`apps/pm-api/src/customers.rs`、`crates/pm-application/src/bootstrap.rs`、前端 `api/customers.ts` + `views/pm/customers/` + 项目列表/导入向导改动、`scripts/seed-demo.sql`）；② `docs(customer): 设计说明与延后裁定`；③ `chore(memory): 长期记忆与当日流水`
  - **不做**：不趁机改代码、不补 G1–G4、不重跑无关模块的重构。
  - **验收**：`cargo check --workspace --all-targets` 0 error；`cargo test -p pm-api --test customers --test pm_lists` 全绿；`cd frontend && npx vue-tsc --noEmit --skipLibCheck` 0 error；`git status` 干净。
  - **交付（2026-11-01）**：`1fae9e3` feat(api/web): 客户主数据 0017 → `8afc24a` docs(customer) → `cf454bc` chore(memory)；D2.1 遗留用例已由 `d7519d8` 先行入库。实测：cargo check 0 error、vue-tsc 0 error、customers 8/8、pm_lists 9/9、run_dispatch 24/24、`git status` 干净。

---

## 3. P1 · 主线：主体身份（G3 principal 映射）

> **为什么这条排在 D3 之前**：当前 `core.principal` 只有 `kind`/`display_name`，**没有任何外部身份列**。所有 PM 写操作的审计 actor 由 `pm_write::owner_actor`（`crates/pm-application/src/pm_write.rs:92`）三级回退猜出来（项目 owner → 任意 human → 全库第一个 human），JWT 的 `sub`/`username`/`tid` 在 `require_bearer` 校验后只塞进 extensions 从不落库。
> 结论：**`governance.audit_log` 里的 actor 不是真实调用者**。D3/D4/D5 每一个都要往审计里写执行证据（进程生命周期、逐仓 commit、验收裁决），越晚修，脏数据越多、回填越贵。

- [ ] **T-020 出设计稿 `docs/ai-pm-principal-identity-v1.md`**
  - **本条按 §0.1 执行**：必须走完整 brainstorming 流程（先看现状 → 一次一问 → 2–3 方案 → 分节确认 → 落稿提交），不得直接产出文档。
  - **目标**：一份可实施的 v1 设计依据，覆盖：身份映射模型、自动 provision 流程、鉴权矩阵、迁移 DDL 草案、验收矩阵、遗留取舍。
  - **依据**：`apps/pm-api/src/auth.rs`（现行 JWT 验签 + `PM_MCP_READ` 门）、`migrations/0001_core.sql:32-40`（`core.principal` 现状）、`crates/pm-application/src/pm_write.rs:92`（`owner_actor`）、`docs/runbook-login-chain.md`（lmms 签发的 claims 形状：`sub`/`tid`/`roles`/`permissions`）
  - **已裁定的上位约束（不得推翻）**：
    - **方案 C —— 自动 provision 但默认未激活**：首次见到未知 identity 时创建 `core.principal(kind='human', is_active=false)` + 身份映射行，**不自动授予任何项目成员资格**；待管理端显式激活后方可使用。这样把「建身份」与「给权限」解耦，`is_active` 是现成开关，零新增权限表。
    - 映射键必须是三元组 `(provider, tenant_id, subject)`：lmms 是**多租户**签发方（`tid` 为强制 claim），只用 `sub` 会在跨租户同名 subject 时串号。
    - 未激活身份一律 fail-closed，错误码沿用 lmms 约定风格（如 `IAM_PERM_3002`），响应体不泄露存在性。
  - **要在对话中裁清的点**（按 §0.1 逐条一问一答，裁完再写结论，不要由 AI 自行拟定）：
    1. 映射载体：独立表 `core.principal_identity` vs `core.principal` 加列 —— 给出选择与理由（多 provider 演进）。
    2. 未激活身份的读权限：全拒（仅留 `/v1/pm/me` 自查）还是允许只读 —— 给出结论。
    3. `owner_actor` 的去留：保留给 bootstrap/系统内部路径，业务写路径一律改真实 principal —— 明确边界。
    4. 存量数据：现有 `core.principal` 无 identity，需给出「首次登录认领」还是「管理端手工绑定」的迁移路径。
  - **已完成的前置裁定（2026-11-01，A/B/C 三方案对比后选定，可直接引用）**：未知 JWT subject 的处理走**方案 C**——自动 provision + `is_active=false`，见下方「已裁定的上位约束」。
  - **不做**：不改代码；不动 D3/D4/D5；不顺带做 RBAC 角色体系（V1 只做身份归因 + 成员资格）。
  - **验收**：文档含「字段 → 库内来源」映射表与验收矩阵（建议 C 系列编号续 D2 的 C28 之后）；用户过稿确认后才可进 T-021。

- [ ] **T-021 实施身份映射与鉴权（待 T-020 过稿）**
  - **目标**：真实调用者身份贯穿到所有 PM 写路径与审计；未激活身份 fail-closed。
  - **依据**：T-020 的稿。
  - **做**：迁移 `0018_principal_identity.sql`；auth 中间件解析 claims → 解析/自动 provision principal → 注入 typed extension（不再只塞裸 `Value`）；新增 `GET /v1/pm/me`；把 `pm_writes`/`pm_lists`/`pm_repositories`/MCP 工具里的 `owner_actor` 调用点替换为真实 principal；审计 `actor` 写真实身份。
  - **不做**：不改 `core.principal.kind` 取值集合；不做 per-project 调用方绑定（D2 待裁 #12 仍开放）。
  - **验收**：新增用例覆盖「首次登录自动 provision 为未激活 + 业务请求 403」「激活后可写」「审计 actor == JWT sub 对应 principal」；D2 的 C1–C28 不回归；`cargo test` 全绿。

- [ ] **T-022 可见性收口（待 T-021）**
  - **目标**：把「按 `project_member` 过滤」从「全局可见」改为真实按成员过滤。
  - **依据**：`docs/ai-pm-customer-master-data-v1.md` §2 裁定 5 与 §6 G3。
  - **做**：客户列表 / 项目列表按当前 principal 的 membership 过滤；无成员关系的资源返回「不存在」而非「无权限」（不泄露存在性）。
  - **不做**：不重开 tag 轴（已放弃）；不做客户级 AI 上下文（G4，v2）。
  - **验收**：非成员访问他人客户/项目返回 404；成员访问正常；`pm_lists` / `customers` 测试补齐多身份场景。

---

## 4. P2 · 主线：D3 Codex 执行适配器

- [ ] **T-030 出设计稿 `docs/ai-pm-slice-d3-executor-adapter-v1.md`**
  - **本条按 §0.1 执行**：完整 brainstorming 流程，不得直接产出文档。
  - **目标**：定义「能启动、监控、终止执行实例」的端口与适配器契约，以及 run 状态在 `claimed → running → 终止` 全程的合法迁移。
  - **依据**：`docs/ai-pm-slice-d-execution-pipeline-v1.md` §三/§九（D3 范围锚定与「不做」列表）、`crates/pm-application/src/run_context.rs`（D2 已落的租约与契约）、`migrations/0011_run_dispatch.sql:39-40`（`executor_kind`/`executor_session` **列已存在**，D3 只填值，不需要新迁移）
  - **要在稿里裁清的点**：进程启动方式（本地子进程 vs 容器 vs 远程 MCP 客户端）；心跳与租约续期的责任方（D2 已有 `renew_lease_by_token`，D3 是复用还是另开）；终止语义（SIGTERM 宽限期、超时强杀、终止后 run 落何状态）；`executor_session` 的标识格式；失败注入点（测试用假执行端）。
  - **不做**：不写 `task_workspace`/`task_repository_commit`（属 D4）；不做失败判定与任务状态裁决（属 D5）；不实现仓库级策略（D4 及后续）。
  - **验收**：含状态机图 + 端口 trait 签名 + 测试计划（含假执行端）；用户过稿后才可进 T-031。

- [ ] **T-031 实施 D3**（前置：T-030 设计稿已经用户确认）
  - **目标**：`pm-claim` 之后能真正拉起执行端并全程可观测、可终止。
  - **依据**：T-030 的稿。
  - **做**：`pm-application` 执行器端口 + `CodexExecutor` 适配器；run 状态迁移走 `apply_task_transition`（不得旁路）；填 `executor_kind`/`executor_session`；启动/终止/超时均写审计。
  - **不做**：不越界到 D4/D5；不改 D2 的工具面契约字段。
  - **验收**：假执行端可完成「启动→心跳→正常结束」与「启动→超时→强制终止」两条路径；`cargo test` 全绿；D1/D2 用例不回归。

---

## 5. P3 · 主线：D4 / D5（顺序固定，不得跳步）

> D 系列顺序是硬约束（`docs/ai-pm-slice-d-execution-pipeline-v1.md` §3.2）：D1→D2→D3→D4→D5，跳步会导致「契约字段无来源」或「基线可被事后篡改」。

- [ ] **T-040 D4 多仓库交付与结果回写**（前置：T-031 完成）
  - 目标：每个仓库独立记录基线、Commit、验证结果；形成交付清单视图。
  - 落点：`task_run_repository`、`task_repository_commit`；对账以 D1 锁定的 `base_commit` 为唯一基准。
  - 先出设计稿再实施，同 T-030 的流程。

- [ ] **T-041 D5 Review、失败恢复与知识回写**（前置：T-040 完成）
  - 目标：形成可审计的执行闭环 —— 失败判定服务（B4）、评审编排、知识索引写入。
  - 依赖 D2 的 `delivery.task_acceptance_criterion`（逐条验收留痕已建表）。

---

## 6. P4 · 技术债与已开放待裁（可搭车，勿单独开切片）

- [ ] **T-050 迁移文件头注释残留**：`migrations/0010_task_state_machine.sql` 头注释仍写「0009_task_state_machine」（迁移号顺移残留）。纯注释修复，随下一条涉迁移的任务搭车。
- [ ] **T-051 待裁 #12 执行端预注册档位**：V1 已明确接受横向越权面（持有效 bearer 可领取任意项目任务），档位机制待首批执行端接入后再裁。**保持开放，不主动实施**。
- [ ] **T-052 待裁 #13 验收条目版本化**：`task_acceptance_criterion` 已有 `updated_at`；若 D5 证据锚定需要更强不变性，再开 forward-only 追加迁移。**保持开放，等 D5 需求明确**。
- [ ] **T-053 estimation PRD 未落库**：`docs/ai-pm-estimation-prd-v1.md` 有完整 PRD，但**零迁移、零代码**（`grep estimation migrations/` 无命中）。属独立大块，需先裁 P0 范围再排期。

---

## 7. 明确延后 / 已放弃（不要重新提议）

| 项 | 裁定 | 重启条件 |
|---|---|---|
| 客户 G1–G4（建项客户必选、客户详情页、可见性、客户级 AI 上下文） | 🟡 延后，**非必须完成项**，不排主路径 | 出现「按客户聚合跨项目视图」或「客户维度权限隔离」的真实需求；重启前回 `docs/ai-pm-customer-master-data-v1.md` §2 复核 5 条裁定 |
| 项目 tag（标签）归纳轴 | ❌ 放弃，不实施 | 需先解决 4 条阻塞（AI 分类半成品收编、project 属性直写 SQL 无命令通道、词表跨客户泄露面、与项目隔离原则冲突） |
| 平台内置 AI 编程助手 / 自研多 Agent 运行时 / 为单一编码工具深度定制 | ❌ 已推翻，不再进入设计讨论 | 无（见 positioning §「明确被推翻的路线」） |

> 注意：客户 G3（可见性）与本文 T-022 是**同一件事**。T-022 作为身份切片的自然延伸执行；客户功能本身（G1/G2/G4）仍延后。

---

## 8. 归档

（完成的条目移到这里，保留 commit 与结论，不删除）
