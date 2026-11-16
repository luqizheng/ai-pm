# ProjCognition 人工主体门修复任务书 V1.0（已执行）

> 2026-11-16 · 基于 master `8b25177f` · 上游：V-1/V-2 专项任务书 §5.2/§附三、D05 **SEC-P1-1**
> **执行状态：已完成**（2026-11-16）：§3 共享门接入两入口；§4 测试转换/新增完成；
> L2 三套件 29/29 + `pm_cognition`/`pm_baseline` 4/4 绿；零 Schema。G2 已通过。
> 性质：单独报批；审计任务本身保持只读证据职责。

## 1. 事实与缺陷

运行时证据（D04，28/28 用例）：

| 入口 | 代码 | 现状 |
|---|---|---|
| cognition item 决策 | `apps/pm-api/src/pm_cognition.rs:279-318` | 仅 `require_project_visible`（tenant+成员）；Agent(dev_agent) 可 confirm（V1-T02/T05） |
| baseline item 决策 | `apps/pm-api/src/pm_baseline.rs:135-172` | 同上；Agent 可 confirm（V1-EXT-01） |
| baseline 版本确认 | `apps/pm-api/src/pm_baseline.rs:190-242` | **正确样板**：auth_mode（interactive/off）→ `require_human`（DB kind）→ 可见性 |
| MCP | `crates/pm-mcp/src/tools.rs` | 无 cognition/baseline 决策工具（V1-T08） |

`AuthedPrincipal`（`apps/pm-api/src/auth.rs:165-182`）**无 principal kind 字段**；
`HumanActor`（`crates/pm-application/src/caller.rs:32-43`）同样无 kind，且 cpk 通道也会生产。
`dev_agent` 是 `core.project_member` 一行（bootstrap 即写入），因此成员校验恒真。

## 2. 身份契约比较（不照抄 PC）

| 维度 | PC（正确参考） | ProjCognition（本域） |
|---|---|---|
| 主体要求 | `ensure_formal_authorized`：DB `core.principal.kind='human'` **且** `ensure_governance_authorized`（Lead / approver_grant） | 决策是项目级认知落库（`ai.project_knowledge`），无治理角色概念；要求 = 真人交互主体 + 项目成员 |
| 身份来源 | `AuditActor`（principal_id） | `AuthedPrincipal`（auth 层生产，唯一来源） |
| 状态写入 | 发布事务（版本/快照/CAS），PC 域专属 | `PgCognitionRepo::decide_item`（FOR UPDATE，仅 proposed） |
| 审批语义 | `pc.approval` 为本域审批 | `governance.approval_request` 属导入/监督链，**不得替代** ProjCognition 确认（V1-04） |
| 复用结论 | 只复用「查库验 kind='human'」这一事实口径 | **不复用** `ensure_governance_authorized`（PC 治理模型，不适用于项目认知） |

**ProjCognition 自有 Human Actor 契约（本任务定版）**：

> 正式逐项确认的主体 = 经可信认证链建立的 **真人交互主体**（`core.principal.kind='human'`
> 且 `auth_mode ∈ {interactive, off}`），同时是该租户内、该项目的成员；
> Agent（dev_agent / ai_pm）与服务主体可以提案，**不得确认**；
> 通用审批记录不能作为确认凭证。

## 3. 修复方案

### 3.1 新增共享门（pm-api 层，唯一实现）

在 `apps/pm-api/src/support.rs` 增加：

```
pub(crate) async fn require_human_decision_context(
    pool: &PgPool,
    principal: &crate::auth::AuthedPrincipal,
) -> ApiResult<()>
```

判定顺序（fail-closed）：
1. `auth_mode` 必须为 `interactive` 或 `off`；`api_key` / `unknown` / `client_credential` → 403；
2. `SELECT kind FROM core.principal WHERE id = $1` 必须为 `'human'`（查库，不信任 token 声明）；
3. principal 未激活由既有 `require_active` 层拦截（不重复）。
4. 项目可见性仍由各 handler 现有 `require_project_visible` 承担（404 不泄露存在性）。

语义：403（有身份、无该动作权限）与 404（不可见）保持现有区分。

### 3.2 接入点（仅两处）

| 文件 | 位置 | 改动 |
|---|---|---|
| `apps/pm-api/src/pm_cognition.rs` | `decide_item` handler（:279） | 在 `require_project_visible` 后调用共享门 |
| `apps/pm-api/src/pm_baseline.rs` | `decide_item` handler（:135） | 同上 |

不改 Repository（`repo_cognition.rs` / `repo_baseline.rs` 不识别 JWT——身份授权职责在
Application/Service 层，维持 V1-T07-B 登记口径）。

### 3.3 明确不做

- 不把 PC `ensure_formal_authorized` 整体搬入本域（治理角色不适用）；
- 不改 Schema / 不加列 / 不做 Migration；
- 不改前端（后端强制门，前端按钮禁用只是体验）；
- 不让 `governance.approval_request` 参与认知确认（V1-04 语义隔离维持）。

### 3.4 风险与误伤

| 风险 | 处置 |
|---|---|
| API Key 登录的真人被拒 | 契约即「交互式真人」；与 baseline 版本确认（既有）一致 |
| `off` 模式回归 | 显式放行（`DEV_OFF_PRINCIPAL` 构造为 human） |
| cpk 通道 | 业务端点已被 `IAM_AUTH_1007` 路径门拦截（V1-T09~T11），本门为纵深防御 |
| Agent 自动化提案链 | 提案 / start_run / 基线创建不改；只有 confirm/reject 等**决策动作**受限 |

## 4. 测试与验收（修复落地后执行）

1. **用例转换（必须）**：
   - `char_t02_agent_principal_can_confirm_until_sec_p1_1` → `regression_t02_agent_cannot_confirm`：403 + DB 快照不变；
   - `char_t05_forged_human_claim_does_not_change_actor` → `regression_t05_forged_claim_still_rejected`：403；
   - `char_v1_extra_baseline_item_decision_accepts_agent` → `regression_v1_ext01_baseline_item_requires_human`：403；
   - `char_t07b` 保持 Characterization（Repository 边界不变）。
2. **新增**：
   - 真人但 `auth_mode=api_key` 调决策 → 403（新用例 `regression_decision_rejects_api_key_human`）；
   - V1-T07-A：若未来引入决策应用服务，以非 Human Actor 直调 → 拒绝。
3. **回归**：`v1_cognition_gate` 全绿；`security_p0_regression` 5/5 不回归；
   `pm_cognition.rs` / `pm_baseline.rs` 既有 happy-path 全绿。
4. **D05/D06 更新**：SEC-P1-1 → Closed；V-1 复判（预期 MATCH 或仅余 P2）；
   G2 复判 → 通过后允许进入 G3。

## 5. 改动清单（预估）

| 文件 | 类型 | 规模 |
|---|---|---|
| `apps/pm-api/src/support.rs` | 新增 1 个共享函数 | ~25 行 |
| `apps/pm-api/src/pm_cognition.rs` | +1 调用 + 注释 | ~5 行 |
| `apps/pm-api/src/pm_baseline.rs` | +1 调用 + 注释 | ~5 行 |
| `apps/pm-api/tests/v1_cognition_gate.rs` | 3 个 char→regression + 1 新用例 | ~80 行 |
| `docs/audit/.../v1-v2-d05-d06.md` | 状态流转 | 文档 |

零 Schema、零 Migration、零前端、零新依赖。

## 6. 报批请求

请批准：
1. 按 §3 新增 `require_human_decision_context` 并接入两个逐项决策入口；
2. 按 §4 转换/新增测试并复判 G2；
3. 不并入其他业务重构，不启动 Schema Migration。

批准后由安全修复任务执行；本审计任务书（阶段 B/C）保持只读证据职责。
