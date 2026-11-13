# AI-PM · P1 测试规格 V1.1（七组 · 已按代码事实修正）

> **性质**：P1 执行批的总规格（体系总纲 §5 的 P1 批判据）。定义 7 组测试、案例框架、执行矩阵与验收清单。
> **上位文件**：`docs/e2e-spec/00-e2e-testing-framework-v1.md`（分级词汇：案例优先级 Must/Should/Could，本批构成 P1 执行批）、`docs/e2e-spec/10-p0-test-specification-v1.md`（P0 门禁 38/38；P1 不得替代门禁）。
> **需求真源**：S01 QA 追溯矩阵 `docs/scenarios/s01/qa/01-requirement-traceability-matrix-v2.1.md`（DATA-020 :70 / UX-052 :117 / SEC-054 :119 / FR-085 :165 / BR-011 :54 / ADR-041 :101 / ADR-023 :73）。
> **V1.1 修订**（2026-11-11）：按代码事实全面复核，修正 R59/R63/D37/D38/D40 四处定性问题，新增 **R69/R70** 两条实现缺口（R65–R68 已被 12 号 P0-02 落地实测发现占用，本规格避让）。基线 master @ `2b3f509e`。

---

## 0. 首轮评审结论（修正后）

| # | 级别 | 规格假设 | 代码事实 | 处置 |
|---|---|---|---|---|
| **R57** | ✅ | P1-02 登录默认落地 `/personal` | `AuthLoadingView.vue:62` 默认 `push("/personal")`；`?redirect=` 优先（`:58-59`）；onboarding 门更优先（`:51`）；`/personal` 经导航树注册（`navigation.ts:91` → `index.ts` 路由映射）；**守卫已实证**（`router/index.ts:609-630`：无 token → `/auth/login?redirect=<fullPath>`；已登录访问登录页 → `/dashboard`） | 可直接落断言 |
| **R58** | ✅ | P1-03 列表权限 | 后端强制（`personal-home.ts:33`「后端已按 `core.project_member` 做可见性过滤，本模块不再放宽」+ `pm_lists.rs:111-127` JOIN）；撤销成员行即时生效；**聚合端点 `GET /v1/pm/personal-home` 尚不存在**（目标形态，`personal-home.ts:37-39`）——当前 = 5 端点组合；`delivery.task` 无 due 列 → `DueInfo.dueAt` 恒 null（`:34-35`，不得虚构期限） | 断言用 `GET /v1/pm/projects` |
| **R59** | 🔴→✅改判 | P1-04「S27 临时代理」= 运行时代理机制 | **误读**：矩阵 :165 原文「Phase 2/3 以 **S27 验收矩阵**临时代理」= FR-085 正式需求文档**缺失**（状态 `DOCUMENT_MISSING`），用 S27 验收矩阵**临时替代需求文档**——需求治理术语，**非运行时组件**；仓内无独立 S27 文档与任何代理实现 | 删除原「临时代理生命周期 6 项」；P1-04 重写为可测子集（§4），复用矩阵已登记自动化证据（`tests/pm_devices.rs` 设备层用例族） |
| **R60** | ✅ | 「六项裁决无正式名称」 | **已实名**：`01-requirement-traceability-matrix-v2.1.md:73` S01-ADR-023 = **R-BL-01~06**（S19 §2） | 按 §6 原文映射 |
| **R61** | ✅+🟡 | Baseline 不可变 + 人工确认 | `ai.project_baseline`（`version NOT NULL` + `confirmed_by NOT NULL`，`0001_baseline.sql:1068-1076`）；提案 `pending/confirmed/superseded`；**条目状态 5 值** `proposed/confirmed/rejected/investigation/deferred`（`:715`，D40 解决）；确认门 = **存在 `proposed` 态条目即 409**（`pm_baseline.rs:329-331` `"baseline proposal still has undecided items"`），deferred/investigation 作为 documented uncertainties 不阻断（R-BL-02）。**🔴 但 `confirm_baseline` 无真人守卫** → R70 | 拆可测/缺陷复现两部分 |
| **R62** | ✅改判 | P1-07 DEF-03/DEF-04 旧入口 | **旧定稿入口不可达**：`pm_write/revision.rs` 的 REST 载体 `pm_writes.rs` 无自有 router，`pm_lists.rs` 仅挂载 task create/update、mark-ready、decide 四个 handler —— **revision 起草/定稿无 pm_write 路由**（需求修订域单入口 = `pm_requirements.rs` → `requirement::*` 三重防护）。**DEF-04 活跃分叉在任务域**：REST `create_task` 带 `require_human`（`pm_write/task.rs:180`），而执行器路径非红线命令 AI 可直写（R8）→ 正是 **DEF-P0-01** | P1-07 重写（§7） |
| **R63** | ⚠️ | DATA-020 快照含 `expanded` | **矩阵与代码分歧**：DATA-020 官方六步⑤明确列 `expanded`（矩阵 :70），但 `InitItem` struct = `key/title/status/required/source_state/detail`（`command_center.rs:325-333`）无 `expanded` | 测试按真实字段断言；分歧登记（D42：建议修订矩阵——expanded 是 UI 展开态，不应入 Read Model） |
| **R64** | ✅ | P1-05 与 P0-008 关系 | 主体重叠（`13-p0-03` P0-008a）；矩阵注记 BR-011/ADR-041 为 **P0 级**需求（:54,:101）→ 主体归 P0 批；P1-05 增量 = 真 LLM Provider 能力面 + 模型不可达语义 | 收窄为增量项 |
| **R69** | 🔴修正 | （波及 15 号）「无直接建 Task REST」 | **`POST /v1/pm/tasks` 存在**（`pm_lists.rs:46` → `pm_writes::create_task_handler` → `pm_write::create_task`，`require_human_conn` 于 `pm_write/task.rs:180`）——Task 有**两条人类创建路径**：①直接 REST（human 闸）②计划物化。P0-05 R33 定性错误，随本修订回改 | ✅ 已同步修正 15 号/10 号 |
| **R70** | 🔴 | P1-06「AI 不能确认 Baseline」 | **当前必 FAIL**：`confirm_baseline` 仅 `require_project_visible`，**无 `require_human` / 治理授权**（`pm_baseline.rs:318-323`）；注释自称「显式人类动作」但代码无守卫 —— 而 ai_pm 主体正是项目成员（bootstrap 自动加入）。条目级决策端点同族 | **登记 DEF-P0-02**（S1 审批边界缺口，与 `pm_cognition.rs:300` 同族）；负向用例 = 缺陷复现，修复前不得标 PASS |

**七组总览（优先级按矩阵定级换算 MoSCoW）**

| 组 | 关联需求 | 矩阵定级 | 案例优先级 | 状态 |
|---|---|---|---|---|
| P1-01 初始化持久一致性 | DATA-020 | P1 | Should | IMPLEMENTATION_GAP（Phase 3 实测） |
| P1-02 Personal Home 登录落地 | UX-052 | P1 | Should | 可直接执行 |
| P1-03 Personal Home 列表权限 | SEC-054 | **P0 级 SEC** | **Must**（排 P1 批执行） | 可直接执行（补列表面直接断言） |
| P1-04 Codex/MCP Bootstrap | FR-085 | P1 | Should | **DOCUMENT_MISSING**；可测子集见 §4 |
| P1-05 Native AI-PM 零外部依赖 | BR-011/ADR-041 | **P0 级** | 主体 Must（已在 P0 批 = P0-008）；增量 Should | 增量收窄 |
| P1-06 Project Baseline 确认 | FR-021/ADR-023 | P1 | Should | R-BL-01~05 可测；**R-BL-06 权限差 = DEF-P0-02** |
| P1-07 并发定稿与语义一致 | DEF-03/DEF-04 | —（缺陷回归） | Should | D38 已解决；重写见 §7 |

---

## 1. P1-01 · 初始化持久一致性（DATA-020 · Should）

**测试目标**：初始化状态可由真实领域数据跨会话稳定恢复（矩阵 :70 六步验收，Phase 3 独立实测）。

> 口径复用 P0-02 R16/R17：InitItem = `init_judgement` 派生（7 态），权威 7 张领域表，读取路径每请求实时查询（矩阵 :70 已认 E2 业务代码证据）。本组补**一致性实证**。

**Fixture**：Project-A 部分初始化（基础信息 ✓ / Goal ✓ / Repository ✗ / Import ✗ / Discovery ✗）。

**快照结构（R63：按真实字段；expanded 分歧见 D42）**

```ts
type InitItemSnapshot = {
  key: string;          // 7 固定 key
  status: string;       // 7 态之一
  required: boolean;
  source_state: string | null;
  detail: string | null;
};
```

**六步持久化验收（矩阵原文）**：① 创建并部分初始化 ② 刷新重读 ③ 退出登录 ④ 重登后打开同一项目 ⑤ 逐项对比（按 R63 字段） ⑥ 确认非会话/缓存所致 —— 全程同一项目同一领域数据，不得只验页面可打开。

**负向**：删来源实体 → Read Model 重算；解绑 Repository → `repository_access` 回退；他会话完成步骤 → 本会话刷新后反映最新领域态；清缓存重登 → 无残留。

**红线**：不得为通过测试把 InitItem 实现为独立业务实体（本仓无 InitItem 表，回归防退化）。

---

## 2. P1-02 · Personal Home 登录落地（UX-052 · Should）

| 场景 | 预期 | 锚点 |
|---|---|---|
| 正常登录 | 默认 `/personal` | `AuthLoadingView.vue:62` |
| 带 `?redirect=` | 回跳原目标 | `:58-59` |
| 首次登录（onboarding_required） | **先** `/onboarding`（门优先于落地） | `:51` |
| 多项目 / 无项目 | 均落地（无项目空态） | `PersonalHomeView.vue:172-173` |
| 未登录直访 `/personal` | → `/auth/login?redirect=/personal` | `router/index.ts:625-629` |
| 已登录访问登录页 | → `/dashboard`（防重登录） | `:613-616` |
| 刷新 / 重开 | 正常恢复 | 5 端点全量拉取 |

**通过标准**：所有登录场景符合落地规则；初始化未完成不阻断落地页。

---

## 3. P1-03 · Personal Home 列表权限（SEC-054 · **Must**，排 P1 批）

> **缺口定位（矩阵 :119 正确）**：单元/集成证据 = 间接用例（`project_members.rs::outsider_cannot_see_project_members` 等）；**Personal Home 列表过滤本身无直接测试** → 本组补。

**Fixture**：User-A（A 可见 / B 不可见）、User-B（B 可见）、User-C（曾可见已撤销）。

| 核心测试 | 断言 | 锚点 |
|---|---|---|
| 列表接口 | A 含 A 不含 B/C | `pm_lists.rs:111-127` |
| 列表 UI | 渲染与接口一致（testid 补齐后收紧） | `PersonalHomeView.vue:82,132` |
| 搜索/筛选/分页 | 结果与 total 均不含未授权 | 同一过滤谓词 |
| **撤销时效** | 移除成员后**下一次请求**即不可见（不等 token 过期） | 凭证不带 membership，查库判定 |
| 详情直访 | 404 | `support.rs:178-208` |
| 元数据不泄漏 | 搜索无命中不泄漏名称/计数 | SQL 层过滤 |
| 期限字段 | 无伪造期限（后端无 due 列，`dueAt` 恒 null） | `personal-home.ts:34-35` |

---

## 4. P1-04 · Codex / MCP Bootstrap 验收（FR-085 · Should）

> **R59 改判**：「S27 临时代理」= 用 S27 验收矩阵**临时替代缺失的正式需求文档**（矩阵 :165，状态 `DOCUMENT_MISSING`）—— 不是运行时组件。原规格「临时代理生命周期 6 项验证」建立于误读，**整节撤销**；若未来出现真正的代理机制，另立用例。
> **矩阵已登记的既有自动化证据**（复用，不重建）：`tests/onboarding.rs::health_is_public_and_me_is_not`；`tests/pm_devices.rs`（`register_reuse_and_server_generated_id` / `detail_visibility_and_404` / `bind_checks_revoke_flow` / `c184_only_tools_call_triggers_device_layer`）。缺口 = **`/install` 端点无自动化用例**（矩阵明示）。

| 子域 | 断言 | 锚点 |
|---|---|---|
| /install 目录 | `GET /install` 返回工具清单；未知 slug **不 404** 落通用条目 | `onboarding.rs:27,133` |
| /install 指引 | `GET /install/{tool}` 指引含稳定入口；不泄漏他人/他项目信息 | `:134-157` |
| Skills ZIP | 四 SKILL.md + verify-install 脚本；manifest SHA-256 全覆盖；`integrity_only`；无凭证泄漏 | `skills_package.rs:40-53`（16 号 R44） |
| MCP 认证 | 有效 Key 连接 → 授权项目操作放行；撤销 Key 后续调用拒（每请求独立鉴权） | 16 号 P0-040 |
| 设备绑定 | 复用 `tests/pm_devices.rs` 已有用例面 + 跨项目篡改拒绝 | 16 号 P0-041 |
| Runner 边界 | 无 Runner 二进制不算失败 | 规格边界保留 |

**新增：/install 端到端一条**（补矩阵登记缺口）：

```ts
test("P1-04 install guides + skills package e2e", async () => {
  const catalog = await api("GET", "/install", userAToken);
  expect(catalog.status).toBe(200);

  const guide = await api("GET", "/install/codex", userAToken);
  expect(guide.status).toBe(200);

  const manifest = await api("GET", "/install/packages/release-manifest.json", userAToken);
  expect(manifest.body.contents).toContain("skills/pm-task-execution/SKILL.md");
  expect(manifest.body.sha256).toMatch(/^[0-9a-f]{64}$/);
});
```

**通过标准**：安装包 / 指引 / Skills / MCP 认证 / 撤销闭环全过；需求文档缺失（DOCUMENT_MISSING）不阻塞以代码事实为锚的验收，但**不得判定 S01-06 需求完整**（矩阵原文纪律）。

---

## 5. P1-05 · Native AI-PM 零外部依赖（BR-011 / ADR-041 · 主体 Must 已在 P0 批；增量 Should）

> **R64**：主体能力面 = P0-008a（`13-p0-03`），矩阵定级 P0 —— 已在 P0 批门禁。本组只做**真 LLM Provider 增量**。

| 步 | 断言 | 锚点 |
|---|---|---|
| 真 Provider 配置下分析 | `ai.analysis_run.model_name` 记录**真实模型名**（非 fake） | `ai.team_llm_provider` 配置 + `RoutedAnalysisModel` |
| 会话回复链 | provider 可达时回复落库（`AI_PM_CONVERSATION_REPLY=on`） | `conversation_reply.rs` |
| 需求建议人工确认 | `pending_confirmation`，不自动转正 | P0-03 三道闸 |
| **Provider 不可达** | 明确失败（结构化 WARN/错误），**不自动切换模型、不用 fake 冒充** | `conversation_reply.rs:16-17`；分析链语义 D39 确认 |

**通过标准**：真 LLM 下完整能力可用且 `model_name` 如实；模型故障不静默、不伪装。

---

## 6. P1-06 · Project Baseline 确认（FR-021 / ADR-023 · Should）

> **R60 六项裁决实名**（矩阵 :73，S19 §2）+ **R61/R70 新事实**。

| # | 裁决 | 断言 | 可测性 |
|---|---|---|---|
| R-BL-01 | Requirement 只引用 Revision (B) | 条目引用 revision id，不复制正文 | ✅ |
| R-BL-02 | 未决允许且标记 (A) | `investigation/deferred` 条目不阻断确认且标记保留；**`proposed` 态条目阻断**（409 `"baseline proposal still has undecided items"`） | ✅（枚举实证 `:715`） |
| R-BL-03 | 无 Repository 可确认 (A) | 空仓库下确认成功 | ✅ |
| R-BL-04 | Cognition 不完整可确认 (A) | 部分认知下确认成功 | ✅ |
| R-BL-05 | 完整逻辑快照 (A) | `version` 递增；提案 `pending→confirmed`，旧提案 `superseded`；条目集合完整 | ✅ |
| R-BL-06 | 确认权限与创建者分离 (B) | `confirmed_by` 与 proposal 创建者分列 ✅（schema）；**但「AI/未授权者不能确认」当前无守卫** | 🔴 **DEF-P0-02** |

**正向链路**（`pm_baseline.rs` 4 端点）：提案 → 逐条目决策（state ∈ 5 值）→ 确认（无 proposed → 版本头落库 + 提案 confirmed → 条目不可变）。

**不可变性**：AI 后台改已确认 Baseline → 拒（红线族）；已确认内容变更 → 新提案新版本，原快照与差异可追溯；`version` 只增不改。

**🔴 负向 = 缺陷复现（DEF-P0-02，修复前不得标 PASS）**：
1. `kind='ai_pm'` 主体令牌调 `POST .../baseline/confirm` → **当前会成功**（应 403）；
2. 普通成员 vs 授权确认人 → **当前无权限差**（都只有成员可见性闸）；
3. 伪造 confirmed_by → 服务端取 caller（此半边安全：`confirmed_by` 由服务端写入，非客户端参数）。

> **修复口径建议**：`confirm_baseline`（及条目决策端点）加 `require_human_conn` + 治理授权（对齐 `requirement::confirm_revision`）；与 `pm_cognition.rs:300` 同族一并修。修复后负向转正为回归用例。

---

## 7. P1-07 · 并发定稿与执行语义一致性（DEF-03 / DEF-04 · Should）

> **R62/D38 已解决**：旧 revision 定稿入口（`pm_write/revision.rs`）**无可达路由**（`pm_writes.rs` 无 revision handler 被挂载）—— 需求修订域单入口 = `pm_requirements.rs` → `requirement::*`（行锁 + `row_revision` CAS + `expected_content_hash`，P0-04 已证）。DEF-04 活跃分叉在**任务域**。

### 7.1 DEF-03 · 并发定稿丢失更新（统一入口回归）

| 步 | 操作 | 预期 |
|---|---|---|
| 1 | User-A/B 同读 Rev-1（各取 `content_hash`） | 同基线 |
| 2 | A：draft→submit→confirm（真哈希） | 成功 → Rev-2 |
| 3 | **真并发**（`Promise.all`）：B 持旧哈希 confirm | **409**（陈旧哈希），终态不变 |
| 4 | 检查 | `current_revision_id` = A 的 Rev-2；Rev-1/2 保留；审计含冲突 |

**防复活守卫（新增）**：断言旧入口保持不可达 —— `POST/PATCH` 到 pm_write revision 面（若未来被挂载）不得绕过统一入口；回归里以「路由清单快照」守护（D38 关闭记录）。

### 7.2 DEF-04 · 语义一致（任务域活跃分叉 → DEF-P0-01 缺陷复现）

| 对照面 | REST | 执行器 | 当前实况 |
|---|---|---|---|
| 创建任务 | `POST /v1/pm/tasks` → **require_human**（`pm_write/task.rs:180`，AI 403） | `create_tasks`（红线）→ 拒 / `update_task_status` 等非红线 → **AI 可直写**（R8） | **身份闸分叉** = DEF-P0-01 实锤 |
| 断言 | REST 面：ai_pm 令牌建任务 → 403 ✅ 可测 | 执行器面：AI 直写任务 → 当前成功（应拦） | 登记于 **DEF-P0-01**（15 号 §7 已挂） |

**通过标准**：DEF-03 统一入口回归全绿；DEF-04 在 DEF-P0-01 修复后（策略层收紧或执行器闸对齐 REST）双入口逐字段一致；在此之前任务域对照只作缺陷证据。

---

## 8. 执行矩阵

| 组 | 对象 | 层 | 证据重点 | 前置 |
|---|---|---|---|---|
| P1-01 | DATA-020 | E2E + DB | 跨会话持久一致 | Phase 3 批次（D41） |
| P1-02 | UX-052 | Playwright | 落地路由与恢复 | 无 |
| P1-03 | SEC-054 | API + Playwright | 列表过滤 + 撤销时效 | 无 |
| P1-04 | FR-085 | E2E + 复用设备测试族 | /install 缺口补齐 | 无 |
| P1-05 | BR-011/ADR-041 | E2E + LLM | 真 Provider 增量 + 故障语义 | D39 |
| P1-06 | FR-021/ADR-023 | E2E + API + DB | R-BL-01~05 + DEF-P0-02 复现 | 修复后负向转正 |
| P1-07 | DEF-03/04 | Integration + 真并发 | 统一入口回归 + 任务域分叉证据 | 无（D38 已闭） |

---

## 9. 验收清单（33 项 → 修正标注）

- **P1-01**（5 项）：全部可执行；快照字段按 R63（无 expanded，D42 登记分歧）
- **P1-02**（4 项）：全部可执行（R57）
- **P1-03**（4 项）：全部可执行（R58）；**Must** 级
- **P1-04**（5 项）：安装包 ✓ / `/install` 自动化 ✓（本组新增，补矩阵登记缺口）/ ~~临时代理生命周期~~（**撤销**，R59 误读）/ MCP 身份授权 ✓ / 撤销失效 ✓
- **P1-05**（5 项）：前四复用 P0-008a；「模型故障明确失败」增量（D39）
- **P1-06**（5 项）：六裁决完整 ✅ / 确认身份 🔴（DEF-P0-02）/ 不可覆盖 ✅ / AI 无法自动确认 🔴（同 DEF-P0-02）/ 未决不误阻断 ✅（R-BL-02 + 409 语义）
- **P1-07**（5 项）：并发不丢更新 ✅ / 过期版本冲突 ✅ / REST↔Executor 一致 🔴（DEF-P0-01 修复后转正）

> P1 门禁：P1 批为 **G-release 辅助判据**（Should），不替代 P0 38/38；**DEF-P0-02 为 S1 级审批边界缺口** —— 与 DEF-P0-01 一并进 P0 门禁缺陷清单（S0/S1 未清零禁止发布）。

---

## 10. 待裁决项（V1.1 更新）

| # | 议题 | 状态/建议 |
|---|---|---|
| **D37** | ~~S27 临时代理设计~~ | **已解决**：S27 = 需求层术语（验收矩阵临时替代缺失的 FR-085 正式文档），非运行时组件；P1-04 无代理可测，原 6 项撤销 |
| **D38** | 旧写入口清单 | **已解决**：revision 旧面不可达（pm_writes 无 revision 挂载）；活跃分叉 = 任务域身份闸（DEF-P0-01）；以路由清单快照防复活 |
| **D39** | Provider 不可达语义 | 分析链失败形态实现时确认后写死断言 |
| **D40** | Baseline 条目状态枚举 | **已解决**：`proposed/confirmed/rejected/investigation/deferred`（`:715`）+ confidence 三值 + category 七类 |
| **D41** | P1-01 批次归属 | 纳入 Phase 3 实测批次（矩阵原文），先于其他 P1 组 |
| **D42** | DATA-020 `expanded` 分歧 | 建议修订矩阵（expanded 为 UI 展开态，不入 Read Model）；若产品要求保留字段则改 Read Model —— 二选一后更新验收记录 |

---

## 11. 缺陷登记（本规格新增）

| ID | 级别 | 描述 | 位置 | 状态 |
|---|---|---|---|---|
| **DEF-P0-02** | S1（审批边界） | Baseline 确认与条目决策 REST **无真人守卫**：注释声称「显式人类动作」，实际仅 `require_project_visible`；ai_pm 成员令牌可确认基线。违反 R-BL-06「确认权限与创建者分离」的权限半边 | `apps/pm-api/src/pm_baseline.rs:310-323`（同族：`pm_cognition.rs:300`） | 登记待修；P1-06 负向 = 缺陷复现 |
| （关联） | — | 任务域 REST（require_human）与执行器（非红线 AI 直写）身份闸分叉 | `pm_write/task.rs:180` vs `policy_engine.rs:32` | = **DEF-P0-01**（15 号 §7 已挂），此处为另一实证面 |

---

## 12. 关联文档

| 文件 | 内容 |
|---|---|
| `docs/e2e-spec/13-p0-03-playwright-spec-v1.md` | P1-05 复用（P0-008a 零依赖能力面） |
| `docs/e2e-spec/14-p0-04-playwright-spec-v1.md` | P1-07 复用（哈希门/Revision 断言口径） |
| `docs/e2e-spec/15-p0-05-playwright-spec-v1.md` | DEF-P0-01 挂靠点；R69 修订波及（已同步回改） |
| `docs/e2e-spec/16-p0-06-playwright-spec-v1.md` | P1-04 复用（Bootstrap/MCP/设备锚点） |
| `docs/scenarios/s01/qa/01-requirement-traceability-matrix-v2.1.md` | 七组需求真源（DATA-020 :70 / UX-052 :117 / SEC-054 :119 / FR-085 :165 / ADR-023 :73） |
| `docs/scenarios/s03/s03-00-technical-design-adr-domain-asset-inventory-v1.md` | DEF-03/04 档案（:383 静态重验；本规格以路由面实证闭环 D38） |
| `apps/pm-api/src/pm_writes.rs` + `pm_lists.rs:30-64` | 任务域 REST 入口（require_human）与挂载面 |
| `apps/pm-api/src/pm_baseline.rs` + `migrations/0001_baseline.sql:699-734,1068-1076` | Baseline 4 端点 + 5 态枚举 + 版本化快照 |
