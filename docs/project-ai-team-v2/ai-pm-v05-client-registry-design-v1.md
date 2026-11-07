# V-05-1 Client Registry 实施设计 V1.0 —— Client Identity Foundation 最小生产可用集

| 项目 | 内容 |
|---|---|
| 文档性质 | 实施设计稿（WP-B 交付物；待评审 + 用户裁决 Q-A~Q-D） |
| 上位裁决 | V0.5 WP-0①（device 升级 Client Registry）· 专家复审第 2/3 块（最小集必须含撤销 + V-05-1 = R-16 生产启用硬前置 + 撤销立即影响全部工具面）· V2 架构基线 Q1=A（Agent Client 个人级注册授权给 Project） |
| 排期位 | roadmap §3 第二阶段（身份基座）；迁移号 **0049**（roadmap §7，0048 已被 ACP 收敛占用——0046 撞号修复后顺延） |
| 覆盖 | Client 登记与稳定 client_id · client_name 唯一归属 · 认证主体绑定 · 凭据签发/验证/**撤销** · lease_client_id 绑定与历史回填 · 权限码校验面 |
| 不覆盖（另稿） | 能力三分动态管理（ACP-08 已有静态声明）· 多凭据轮换 · Client 生命周期 UI · 设备健康管理 · Client 独立 principal 化（V2 调度批）· 调度入口（roadmap §9C） |
| 代码基线 | ai-pm master @ ee1b441（2026-11-07） |

---

## 1. 代码事实基线（设计钉在事实上）

| # | 事实 | 证据 |
|---|---|---|
| F1 | `devenv.device` 已具身份与撤销骨架：客户端生成 UUID、tenant、`owner_principal_id`、`status ∈ {active, revoked}` + `revoked_at`、readiness 快照 | `migrations/0027_devenv.sql:23-44` |
| F2 | 设备声明已是**归因链**但非**认证链**：`X-AI-PM-Device-Id` 经 `attach_device_claim` 校验（租户/owner/status=revoked 拒绝）后注入 `DeviceClaim` extension；**缺席不阻断授权**；`device_project_binding` 为治理子表 | `apps/pm-api/src/device_claim.rs:39-95`、`routes.rs:153-160` |
| F3 | `/mcp` 鉴权三层（auth on）：外层 Bearer JWT（lmms 签发，**权限码内嵌 JWT permissions**）→ `require_mcp_permission`（PM_MCP_READ + 工具写码表）→ `attach_device_claim`（最内层） | `routes.rs:145-165`、`auth.rs:586-655` |
| F4 | MCP 工具层授权 = **真人 principal 成员校验**（`ensure_member(caller.human)`）+ 执行主体解析（dev_agent/ai_pm 行）；`CallerContext{human, device}` 自 extensions 组装、无 IO | `crates/pm-mcp/src/caller.rs:1-60`、`pm-application/src/caller.rs:113-127` |
| F5 | 执行端凭据**寄生真人**：Runner 用真人 JWT 过 `/mcp`（E2E 用 admin JWT）；无独立可吊销凭据——V2 核验六缺口之一 | V2 核验报告 §六 |
| F6 | 新工具权限双态门（F-7）：`PM_MCP_AGENT_REPORT`/`PM_MCP_COMMAND` 默认 fail-closed 403（`IAM_PERM_3001`），env `AI_PM_MCP_LEGACY_PERMISSION_GRANT=1` 显式降级为 PM_MCP_READ；**签发面在 lmms IAM**（权限码内嵌 JWT），ai-pm 侧无自治签发 | `auth.rs:661-700` |
| F7 | `lease_owner` 过渡形态 = `mcp:{client_name}:{token8}` 自报拼接（注释自认非凭证）；`execution_mode` 三分已落（`runner:` 前缀 → spawn） | `claim.rs:32,701-758` |
| F8 | 命令面/续领按 `worker_id` 绑定路由；R-16 生产启用 = 本设计完成后才解锁（ACP V1.2 §3.6 生产启用门） | V1.2 §3.3/§3.6 |
| F9 | `task_run` 无 client 身份列；历史 Run 的 Client 身份只有 lease_owner 自报文本可溯 | `0011`/`0023` DDL |

## 2. 目标架构：身份、凭据、授权三分离

**层级模型（专家裁决保留）**：`Team → Project → Client{Identity, Credential, Revocation}`——Client 是**可被多 Project 绑定的身份载体**（`device_project_binding` 治理面既有），授权范围由 Team/Project 关系控制。

**三分离语义（本设计的语义基座，V1 口径）**：

| 概念 | V1 承载 | 语义 |
|---|---|---|
| **Client 身份** | `devenv.device.id`（稳定 UUID，即 lease_client_id） | 命令路由/fencing/审计归因的主体；创建后不变 |
| **Client 凭据** | device 上的**自发 opaque key**（cpk_ 前缀，哈希落库） | 认证通道钥匙；可签发/撤销/（后补）轮换；**凭据 ≠ 身份**——换凭据不换身份 |
| **授权锚点** | `owner_principal_id`（真人） | 成员校验仍经 owner（F4 语义不变）；**凭据撤销 = 通道关闭，即使 owner 仍是成员** |

> **Q-B 裁决点**：owner 锚点是 V1 的显式过渡——Client 不独立 principal 化（tenant_agent/principal kind 扩展挂 V2 调度批）。此语义写进 API 文档：Client 凭据的有效性 = owner 成员资格 ∩ 凭据未撤销 ∩ device active，三者交集。

**撤销即时性（专家硬约束）**：凭据校验**每请求查库**（不缓存；MCP 调用频率低，无性能压力）；撤销影响面见 §8 矩阵——五执行器/命令面工具 + 未来续领/发现，**不能只禁续领放行轮询**。

## 3. 数据模型（迁移 0049）

```sql
-- 1) device 升级为 Client Registry 最小集（0027 骨架上扩展）
ALTER TABLE devenv.device
  ADD COLUMN credential_key_hash   TEXT,        -- sha256(lower(hex)) of "cpk_<32bytes>"
  ADD COLUMN credential_issued_at  TIMESTAMPTZ,
  ADD COLUMN credential_revoked_at TIMESTAMPTZ, -- 撤销/轮换旧凭据时落值（安全底线单列）
  ADD COLUMN granted_permissions   TEXT[] NOT NULL
       DEFAULT ARRAY['PM_MCP_READ','PM_MCP_AGENT_REPORT','PM_MCP_COMMAND'];
-- status 复用 0027 既有 {active, revoked}；client_name 唯一性 = name 在租户内唯一：
ALTER TABLE devenv.device
  ADD CONSTRAINT uq_device_tenant_name UNIQUE (tenant_id, name);
CREATE INDEX idx_device_credential_hash
  ON devenv.device (credential_key_hash) WHERE credential_key_hash IS NOT NULL;

-- 2) Run 的稳定 Client 身份（R-16/R-3 的链路终点）
ALTER TABLE delivery.task_run
  ADD COLUMN lease_client_id UUID REFERENCES devenv.device(id);
CREATE INDEX idx_task_run_client ON delivery.task_run (lease_client_id) WHERE lease_client_id IS NOT NULL;
```

- **凭据单列**（不做凭据表）：专家裁决「多凭据轮换与并存」属后补；轮换 V1 = issue-new → revoke-old 两步。
- **`granted_permissions` 列**（Q-C 裁决点）：V1 权限字典的 **ai-pm 自治面**——签发时可裁剪（最小 {PM_MCP_READ}）；lmms IAM 正式字典落地后切双轨（§4.4）。
- **`lease_client_id` 不回填自报文本**：仅 §7 三分类中「可靠对应」类回填 device.id。

## 4. 凭据形式与鉴权链改造

### 4.1 凭据形式（Q-A 裁决点，推荐方案 B）

- **方案 B（推荐）：ai-pm 自发 opaque key**，格式 `cpk_<43 chars base62>`（≥256bit 熵）；落库 sha256；仅签发响应返回一次明文。
- 方案 A（否决备案）：等 lmms client-credentials JWT——撤销即时性与签发自治都受制于跨仓批次，且 JWT 权限码内嵌使细粒度裁剪僵化；留作 V2 与 lmms Gateway 统一后的演进位。

### 4.2 鉴权链（三层不动结构，外层加一条通道）

```text
外层（改造点）：Bearer <JWT>                    → 现行链不动
              Bearer cpk_…（非 JWT 形态分支）    → device_credential 查表：
                  key_hash 命中 ∧ status='active' ∧ credential_revoked_at IS NULL
                  ∧ owner_principal 存在且 active
                  → 构造 AuthedPrincipal{ principal=owner, permissions=granted_permissions 列 }
                  （失败 → 401 IAM_AUTH_XXXX，不泄露命中与否）
中层：require_mcp_permission                    → has_permission 判定不变（数据源变了）
最内层：attach_device_claim                      → 凭据通道自动注入 DeviceClaim{device_id}
                  （凭据即声明，免 X-header；真人 JWT 通道维持 X-header 现状）
```

- **解析位置**：`auth.rs` 的 bearer 解析分支（`bearer_token` 处）——`cpk_` 前缀路由到新解析器；**每请求查库**（撤销即时性）。
- **A\* 不受影响**：Client Key 通道仅开 `/mcp` 执行器/命令面；人类确认仍走 REST/UI + interactive JWT（A* 红线零触碰）。
- **off 模式**：鉴权 off 时无凭据通道（现状口径不变）。

### 4.3 claim 面写入（过渡形态退役路径）

- 凭据通道领取：`lease_client_id = device.id`，`lease_owner` 改为 `client:{device_id}`（**稳定值**——不再含 token8，命令路由跨重启稳定，直接解除 R-11 的 token8 解耦债）；`execution_mode='spawn'` 判定改读凭据通道（`runner:` 前缀降级为兼容识别）。
- 真人 JWT 通道（Model A）：`lease_client_id = NULL`，owner 格式不变。
- Worker 通道：`lease_client_id = NULL`（进程内直连无凭据），worker_id 不变。

### 4.4 权限码字典双轨（Q-C 裁决点）

| 通道 | 权限判定数据源 | 状态 |
|---|---|---|
| 真人 JWT | lmms IAM 内嵌码（现状） | 不动；lmms 注册两新码后自然生效 |
| Client Key | `granted_permissions` 列（签发时裁剪） | V1 生效；**撤除 F-7 降级开关的前置** |
| 字典正式化 | lmms IAM 注册 `PM_MCP_AGENT_REPORT`/`PM_MCP_COMMAND` | 跨仓批；落地后 Client 通道可切换/合并，开关撤除 |

## 5. 签发 / 撤销 / 轮换管理面（REST）

| 端点 | 语义 | 权限 |
|---|---|---|
| `POST /v1/pm/devenv/devices/{id}/credential` | 签发（或轮换第一步）：生成 cpk_ 明文（**仅本次响应返回**）+ hash 落库 + `credential_issued_at`；已有未撤销凭据 → 409（先 revoke） | 租户 admin 或 device owner |
| `DELETE /v1/pm/devenv/devices/{id}/credential` | 撤销：`credential_revoked_at = now()`（幂等）；**即时生效**（下一请求 401） | 同上 |
| `PATCH /v1/pm/devenv/devices/{id}` | status active/revoked（0027 既有语义）+ granted_permissions 裁剪 | 租户 admin |

- 审计：`device.credential.issued` / `device.credential.revoked` / `device.permissions.changed`（governance.audit_log，四身份列全写）。
- UI：执行环境设备三页（c0ba39e）挂「凭据」区块——随收敛批，V1 先 API。

## 6. 历史回填（0049 迁移内，三分类）

| 分类 | 判据 | 处置 |
|---|---|---|
| 可靠对应（自动回填） | `lease_owner LIKE 'client:%'`（新格式，回填期尚不存在）∪ **`runner:` 前缀 claim 且 device 存在绑定与 owner 交叉可证**（`device_project_binding` ∩ audit claim 归因一致） | `lease_client_id = device.id` + 审计 `run.lease_client_backfilled` |
| 仅自报（暂不认定） | `mcp:{client_name}:{token8}` 且无交叉证据 | `lease_client_id = NULL`，保留 `lease_owner` 原值，审计 `run.lease_client_legacy` |
| 无法确定 | owner 为 NULL/Worker | NULL，不审计（无 Client 语义） |

**迁移原则（专家原文采纳）：历史兼容 ≠ 历史身份可信**——`lease_client_id IS NULL` 的 Run 在 Recovery Foundation 批的发现/续领路径中**一律拒绝**（§3.6 前置加一条），人工处置入口兜底。

## 7. 撤销生效矩阵（每请求查库，无缓存窗口）

| 面 | 撤销后行为 |
|---|---|
| `/mcp` 全部工具（含四 ACP 工具 + 未来 discover/reacquire） | 401（外层解析失败，先于权限判定） |
| `attach_device_claim` 真人通道（X-header） | 现状已拒（Revoked）✅ 不变 |
| 在途 Run 的租约 | **不主动吊销租约**（租约是 Run 状态非凭据）；该 Client 无法再续租/上报，Run 走既有离线恢复路径 |
| 管理面 REST（devenv devices） | JWT 通道，与凭据无关 |
| UI 设备页 | 显示凭据状态（issued/revoked 时间） |

## 8. 验收标准

| 编号 | 场景 | 必须保证 |
| --- | --- | --- |
| ACP-435 操作化 | Client Key 调用四 ACP 工具 | granted_permissions 裁剪生效：未授予码 → 403（`IAM_PERM_3001`）；F-7 开关与凭据通道**正交**（开关只影响真人 JWT 通道） |
| ACP-436 操作化 | 撤销凭据后 | 下一请求 401；discover/续领/轮询/上报全部关闭（撤销矩阵逐面验证）；owner 仍成员不影响 |
| ACP-460 | cpk_ 凭据签发与明文纪律 | 明文仅签发响应出现一次；落库仅 sha256；重复签发 409 |
| ACP-461 | claim 写入稳定身份 | 凭据通道 claim → `lease_client_id=device.id` ∪ `lease_owner='client:{id}'`；真人通道 → NULL |
| ACP-462 | 跨重启命令路由稳定 | 同凭据重启后 poll（`client:{id}` 路由）命中在途命令（token8 死锁形态不可再现） |
| ACP-463 | 回填三分类 | 三类样例 Run 的 lease_client_id/审计行各如其分；NULL 行不得进入 Recovery Foundation 续领 |
| ACP-464 | granted_permissions 最小化 | 仅 PM_MCP_READ 的凭据调 report/poll → 403 |

E2E 必测（roadmap 第二阶段收口）：真实栈 Client Key 全链（签发→claim→介入→回复→撤销→全工具 401）+ 未授权 Client 无法发现/抢占他人 Run（roadmap §8 验证⑥）。

## 9. 待用户/专家裁决项

| # | 事项 | 推荐 |
|---|---|---|
| Q-A | 凭据形式：ai-pm 自发 cpk_（B）vs 等 lmms client-credentials JWT（A） | **B**（撤销即时 + 无跨仓阻塞 + A* 零触碰；A 留 V2 Gateway 统一后演进） |
| Q-B | owner 锚点语义（成员校验仍经 owner；凭据撤销=通道关闭） | **接受**；Client 独立 principal 化挂 V2 调度批 |
| Q-C | 权限双轨（granted_permissions 列先行 vs 等 lmms 字典） | **列先行**；lmms 字典落地后合并并撤 F-7 开关 |
| Q-D | 0049 列集（四列 + uq_device_tenant_name + lease_client_id） | **按 §3 落**；实施前重核台账（当前 max=0048） |

## 10. 工程纪律

1. 实施批次 = 0049 迁移 + 服务层（device credential 签发/校验）+ auth.rs cpk_ 分支 + claim 写入 lease_client_id + REST 管理面——**单批纵切**，Recovery Foundation（discover/reacquire/命令改路由）依赖本批落地后实施。
2. `runner:` 前缀过渡形态在凭据通道落地后**只读兼容识别**（不新签）；历史 `mcp:{name}:{token8}` 依 §6 分类。
3. 禁重建区：不建 principal 新 kind、不动 tenant_agent、不建凭据表（单列）、不改 13 态。
4. 全部写入经 pm-application（V-6）；审计四身份列全写（C174/C180 守卫同步登记）。
5. 迁移号 0049 落地前重核台账；本批不消耗 0048（ACP 收敛批专用）。

---

**文档结束**
