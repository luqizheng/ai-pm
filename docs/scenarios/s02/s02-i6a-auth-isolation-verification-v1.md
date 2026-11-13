# S02-I6-A · A* 认证隔离代码核验报告 V1.0

> 核验对象：《S02-I6-A 权限基座与人类正式确认协议 V1.2》§20.1「A* 认证隔离核验」（P0 阻塞）七问 + §20.2~20.5 实施前检查。
> 核验基线：worktree `ai-pm-s01` @ `feat/s01-implementation`，2026-11-04。
> 核验性质：静态代码事实 + 现行部署事实（runbook）；不含运行时验证。
> 判定口径：V1.2 §6.6——无法区分 MCP Key 与人类交互会话时**必须拒绝确认（fail-closed）**。

---

## §1. §20.1 七问逐项结论

**问 1 · REST 确认端点实际使用的认证中间件**
与全站共用 `require_bearer`：JWT(JWKS) → `AuthenticatedIdentity` → `resolve_or_provision` → `AuthedPrincipal`（`auth.rs:200-250`）。MCP 的 `require_mcp_permission` 是 `/mcp` 路由**内层**叠加（`routes.rs:139-141`），REST 与 MCP **共用同一 bearer 验证层**。已有拒绝先例：service token 拒绝（`auth.rs:240`，`IAM_PERM_3003`）、跨租户 `ct=true` 拒绝（`:241-244`，`IAM_PERM_3004`）——**按 auth_mode 拒绝的模式在代码里有现成先例可循**。

**问 2 · MCP lmms Human Key 能否被 REST 端点接受**
**能**。任何有效 lmms JWT 对 REST 与 MCP 同等有效；`/mcp` 的额外要求（`PM_MCP_READ` + 工具权限码）只在 MCP 路由生效，REST 不复检。

**问 3 · 是否已有独立可信交互认证上下文**
**没有**。唯一的令牌类型信号是 claims 分类（见下），但**只用于审计展示，零准入消费**。

**问 4 · 服务端能否可靠区分 MCP Key 与人类交互会话**
**存在三分类机制，但覆盖不了最现实的一条路径**（重大发现，`auth.rs:121-131`）：

```rust
/// `aud` 存在 = 浏览器登录；无 `aud` 但有 `client_id` = API Key；
fn auth_mode(&self) -> &'static str {
    if self.aud.is_some() { "interactive" }
    else if self.client_id.is_some() { "api_key" }
    else { "unknown" }
}
```

- **api_key 类可区分可拒绝**：`auth_mode="api_key"`（无 `aud` 有 `client_id`）——确认端点要求 `interactive` 即可挡住（判定材料现成，只差一个 if）。
- **登录令牌复用不可区分**：Agent 拿真人**登录 JWT**（有 `aud`，auth_mode=interactive）调 REST，与浏览器会话**无任何服务端可检差异**。而**现行部署正是这种用法**——`docs/runbooks/runbook-test-server-deploy.md:220-239`：MCP 探活/工具调用全部使用 `/v1/iam/auth/login` 的 admin 登录令牌。即 **H-17 场景（Agent 持登录令牌打 REST 确认端点）在现网真实存在，且当前必败**（无法拒绝）。
- `auth_mode` 现仅流入 `AuthedPrincipal.auth_mode`（`auth.rs:417`，审计四列之一），**没有任何端点用它做准入**。

**问 5 · principal_id / identity_id 可信绑定**
✓ 成立。`resolve_or_provision`（`auth.rs:356-375`）经 `principal_identity(provider,tenant,subject)` 解析稳定双 ID；T-088 四列已入审计。

**问 6 · 确认上下文绑定精确 Revision/MarkdownVersion/Hash**
**尚无此能力**——Human Confirmation Service 本身待建（I6-A 实施件）；绑定字段设计已在 V1.2 §7 就绪。

**问 7 · 其他绕过路径**
C-09 三条调用链（REST 直写 / Executor / Orchestrator AutoApply）均可达状态变更——收敛目标已定（V1.2 §8.3 唯一入口），属实施项非新发现。

---

## §2. A* 总判定

**A* 当前为「条件性可落地」，fail-closed 生效：**

| 冒用路径 | 可否防御 | 依据 |
|---|---|---|
| Agent 持 **API Key** 调 REST 确认 | **可防**（确认端点要求 `auth_mode=="interactive"`，判定材料现成） | `auth.rs:121-131` |
| Agent 持**登录 JWT**（有 aud）调 REST 确认 | **不可防——与浏览器会话不可区分**；且为现行 MCP 部署的标准用法 | runbook:220-239 |
| service token / 跨租户 ct | 已有拒绝先例 | `auth.rs:240-244` |

**按 V1.2 §6.6/§16.3：正式确认入口在补齐隔离能力前保持关闭。** H-16 可防、**H-17 当前必败**、H-18 需新能力建成后成立。

**补齐候选（供裁决）**：
- **(c) pm-api 自建 step-up 确认票据（推荐 V1 主选）**：确认端点要求一张短时效、单对象（绑定 revision_id+hash）、经密码重认证或等价强交互换取的确认凭据——不依赖 lmms 改造，落 I6-A 实施范围；
- (b) lmms 上游 token-purpose claim（跨仓改造，联动 [[ai-pm-lmms-integration]]，周期长）；
- (a′) 仅要求 interactive + 短 exp 窗口——**不彻底**（Agent 可即时登录），只可作为叠加缓解，不可单独作为 A* 依据。

---

## §3. §20.2~20.5 实施前检查结果

| 项 | 结论 |
|---|---|
| §20.2 Approver 授权 | C-04 已答：四端点在（`project_members.rs:33-48`）、判定层实时查库**无权限缓存**✓、X-2 租户撤成员级联✓、两档制下普通成员无法自授（POST members 需 owner）——grant_handler 的具体角色断言留实施时验证 |
| §20.3 `approved_by` 消费者 | **读**：`pm_artifacts.rs:33,50`（产物列表回显）、`pm_lists.rs:511`（修订列表回显）、`rows.rs:144`（Row 类型）+ 前端 `api/pm.ts` 修订列表契约 + 测试断言（`pm_writes.rs:510,578`、`pm_lists.rs:357`）；**写**：`executor.rs:901`、`pm_write/revision.rs:98` 两处。→ 读侧全为展示回显（可迁 Confirmation 读取），写侧随双入口收敛唯一；**删除可行**，迁移序=先迁读→收敛写→删列 |
| §20.4 人类草稿路径 | 现状：REST 建修订存在但直写 DB content（Q-3 后须改走受控写入，属 I1/I2）；需求域**不存在任何直接写 Markdown 的旁路**（Markdown 写入路径本身待建）✓ |
| §20.5 Migration | ✓ 已核：0030-0033 被 S01 并行批占用，**S02 自 0034 起**；实施时按分支实况复检 |

---

## §4. 结论

1. §20.1 七问全答；**核心判定=A* 条件性可落地，登录令牌复用路径不可防 → 确认入口 fail-closed 关闭直至隔离能力补齐**。
2. 已有可复用件：auth_mode 三分类（判定材料）、service/ct 拒绝先例（模式）、T-088 身份四列（审计）。
3. **建议 V1.2 增补一处**：§6.5/§20.1 注明「登录令牌（auth_mode=interactive）被 Agent 复用与浏览器会话不可区分（runbook 现行用法即此）」，并把候选 (c) step-up 票据列为推荐补齐路径——否则实施者可能误读「要求 interactive」即满足 A*。
4. 本报告关闭 V1.2 §21 未勾项中的「A* 认证隔离代码核验通过」？**否——核验完成但结论为"未通过（需补隔离能力）"**，该勾选项在 step-up（或等价能力）落地并过 H-17/H-18 前保持未勾。
