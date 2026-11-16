# D04 安全测试证据包（V-1 / V-2 专项 · 阶段 B+C）

> 2026-11-16 · 代码快照 **master e16e228d**（含 SEC-P0 ee4cd046 + SEC-P1-1 追认提交）+ 阶段 D 测试代码
> 上游：V-1/V-2 专项任务书 §5.3/§6.3/§7；交付物 05 V1.1 §2/§3/§5/§6
>
> **两条时间线，勿混读**：
> - **修复前（Characterization 基线，§3 表 T02/T05/EXT-01 所记）**：Agent 可确认 cognition/baseline
>   逐项决策 → 据此 V-1 判 **PARTIAL**、SEC-P1-1 立，与 D05/D06 一致；
> - **修复后（SEC-P1-1 落地 + 29 用例回归，§2 表）**：Agent/Service/cpk 全拒、T01 真人确认落
>   decided_by=human → V-1 可复判 **MATCH**、SEC-P1-1 转关闭，G2 已通过（2026-11-16 复判）。
>   该终态以【用户追认批准 DEC-SEC-P1-1-01 + SQL 门禁整改 + 本包随修复一并提交】为生效前提。
> 结论摘要：**L2 基础设施落地；修复前 29 用例（含 Characterization）与修复后回归均复现全绿；
> P0×5 关闭；SEC-P1-2 关闭；SEC-P1-1 修复已追认提交（e16e228d）；V-1 修复前 PARTIAL → 修复后 MATCH；
> V-2 = PARTIAL（仅 SEC-P2 引用链）。**
> 阶段 D 追加（2026-11-16）：P1 真实业务链 **10/10 PASS**（G3 通过；证据见 §10 与
> [`phase-d-g3-g4-verdict.md`](phase-d-g3-g4-verdict.md)）。

---

## 1. 测试环境

| 项 | 值 |
|---|---|
| 代码快照 | master `e16e228d`；本任务新增/重构 `apps/pm-api/tests/` + 阶段 D `e2e/chain/`，零业务改动、零 Schema、零 Migration |
| 数据库 | `postgres://aipm:aipm@127.0.0.1:16433/aipm`（`aipm-pg-gate` 容器） |
| 隔离方式 | `#[sqlx::test(migrations = "../../migrations")]`：每用例独立数据库，用例结束销毁 |
| 鉴权模式 | `router_with_auth`（jwks 生产同构）：测试 RSA 密钥对签发真 JWT；`/mcp` Host 固定 loopback |
| 外部副作用 | **零**：不连接真实执行器/通知服务/远端 Git/LLM；FakeModel 仅版本化接口占位 |
| 执行入口 | `cargo nextest run -p pm-api --test <binary>`（`scripts/test.sh` 的 `nextest --profile fast` 无过滤，三二进制自动纳入门禁） |

### 1.1 主体矩阵（阶段 B 凭据签发器）

| 主体 | 构造 | 用途 |
|---|---|---|
| Human interactive | `lmms` identity 映射 + JWT（`aud` → interactive） | T01 / V2 正常路径 |
| Service JWT | JWT `roles=["service"]` | T03 / T06（鉴权层 403） |
| Agent（dev_agent） | bootstrap 的 developer principal + identity 映射 + JWT | T02 / T05 / V1-EXT-01 / T07-B |
| API Key 形态 | JWT `purpose=api_key` | 预留（确认类端点应拒） |
| cpk 设备凭据 | `devenv.device` + `device_credential` 签发（owner=humanA，三码白名单） | T09~T12 / R3 |
| 未认证 | 无 Authorization 头 | T04 |
| 同租户非项目成员 | 独立 human principal + `tenant_member`（无 project_member） | V2-T09 |

### 1.2 双租户夹具

| 租户 | 构造 |
|---|---|
| tenantA | `SYSTEM_TENANT`，`core.tenant` 改标 `lmms:1`；bootstrap `L2-A1`；owner 真人 identity |
| tenantB | 独立 `core.tenant`（`lmms:2`）；bootstrap `L2-B1`；ownerB identity（tid=2） |

断言库（交付物 05 §2.1 F3）：
- `expect_cross_tenant_denied`：HTTP 拒绝 **且** 目标项目 14 张表 DB 快照逐字不变；
- `expect_cross_tenant_read_denied`：HTTP 拒绝 **且** 响应体不含目标租户标记（逐标记断言）。

DB 快照覆盖：`ai.cognition_item` / `ai.cognition_run` / `ai.project_knowledge` /
`ai.baseline_proposal` / `ai.baseline_item` / `delivery.task` /
`governance.approval_request` / `ai.change_proposal` /
`integration.inbound_event` / `integration.job` / `governance.human_intervention` /
`integration.notification` / `governance.audit_log` / `ai.project_ai_config`。

---

## 2. 测试载体与执行结果

| 二进制 | 文件 | 用例数 | 结果 |
|---|---|---|---|
| `security_p0_regression` | `apps/pm-api/tests/security_p0_regression.rs`（重构为复用夹具） | 5 | **5/5 PASS** |
| `v1_cognition_gate` | `apps/pm-api/tests/v1_cognition_gate.rs` | 15 | **15/15 PASS** |
| `v2_tenant_isolation` | `apps/pm-api/tests/v2_tenant_isolation.rs` | 9 | **9/9 PASS** |
| 合计 | 共享夹具 `apps/pm-api/tests/l2_security/mod.rs` | **29** | **29/29 PASS** |

执行命令（2026-11-16，SEC-P1-1 修复后）：

```bash
DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
cargo nextest run -p pm-api \
  -E 'binary(security_p0_regression) | binary(v1_cognition_gate) | binary(v2_tenant_isolation)'
# Summary: 29 tests run: 29 passed, 0 skipped
```

> 另跑 `pm_cognition` + `pm_baseline` 既有 happy-path：4/4 PASS（`off` 模式豁免不受影响）。

---

## 3. V-1 用例结果（预期 / 实际 / DB 证据）

| 编号 | 类别 | 主体与操作 | 预期 | 实际 | DB 证据 | 结论 |
|---|---|---|---|---|---|---|
| V1-T01 | Regression | Human 确认 cognition item | 200 + 真人决策 | 200 | `cognition_item.state=confirmed`、`decided_by=humanA`；`project_knowledge.confirmed_by=humanA` | **PASS** |
| V1-T02 | Regression | Agent(dev_agent) 确认 | 拒绝 | 403（human principal 要求） | 快照逐字不变，item 仍 proposed | **PASS**（修复后） |
| V1-T03 | Regression | Service JWT 确认 | 拒绝 | 403 `IAM_PERM_3003` | 快照逐字不变 | **PASS** |
| V1-T04 | Regression | 未认证确认 | 拒绝 | 401 `IAM_AUTH_1005` | 快照逐字不变 | **PASS** |
| V1-T05 | Regression | Agent + body/header 伪造 `kind=human`、`decided_by=humanA` | 拒绝 | 403（声明字段无效） | 快照逐字不变，伪造主体未落库 | **PASS**（修复后） |
| V1-T06 | Regression | Service + 伪造 Human 声明 | 拒绝 | 403 `IAM_PERM_3003` | 快照逐字不变 | **PASS**（声明字段不参与鉴权） |
| V1-T07-A | **UNKNOWN（以架构事实关闭）** | 以非 Human Actor 调「决策应用服务」 | 拒绝 | 代码中不存在独立于 HTTP handler 的决策应用服务；唯一可达入口 = 已覆盖的 HTTP handler | — | 无独立攻击面；随 T02/T05 覆盖 |
| V1-T07-B | **Characterization** | 直接调 `PgCognitionRepo::decide_item`（dev_agent） | 登记边界 | 成功（Repo 不识别主体，身份门在 handler） | `decided_by=dev_agent` | 边界登记，不计通过 |
| V1-EXT-01 | Regression | Agent 调 Baseline 逐项决策端点 | 拒绝 | 403（human principal 要求） | 快照逐字不变，item 仍 proposed | **PASS**（修复后） |
| V1-APIKEY | Regression | 真人 principal 的 API Key 形态凭据确认 | 拒绝 | 403（interactive 要求） | 快照逐字不变 | **PASS**（新增） |
| V1-T08 | Characterization | Worker/MCP 模拟确认 | 无路径 | `tools/list` 无 cognition/decide 工具；worker 无 `decide_item` 调用 | — | 无旁路证据成立，不计通过 |
| V1-T09 | Regression | owner cpk → 业务决策端点 | 401 | 401 `IAM_AUTH_1007`（路径门仅 /mcp） | 快照逐字不变 | **PASS** |
| V1-T10 | Regression | Agent 进程持 cpk（+伪造头） | 401 | 401 `IAM_AUTH_1007` | 快照逐字不变 | **PASS** |
| V1-T11 | Regression | Service 进程持 cpk | 401 | 401 `IAM_AUTH_1007` | 快照逐字不变 | **PASS** |
| V1-T12 | Regression | 撤销项目成员资格后旧 cpk 调 `/mcp` | 拒绝 | HTTP 200 + `isError=true`（not found） | 项目 14 表快照不变 | **PASS** |
| R3-a | Regression | 凭据撤销后 cpk 任意 `/mcp` | 401 | 401 `IAM_AUTH_1008` | 快照不变 | **PASS** |
| R3-b | Regression | 身份拒绝/伪造/授权失效三组 | 全拒绝 | 见 T03/T04/T06 + T05 伪造不升级 + T12 | 每组查 DB | 三组覆盖成立 |

**V-1 判定**：两个正式逐项决策入口（cognition / baseline）均强制交互式真人 +
查库 `kind='human'`；Agent/Service/未认证/伪造声明/API Key/cpk 全部拒绝；
确认单点（FOR UPDATE，仅 proposed）、审计主体真实可追溯、MCP/Worker 无确认路径
→ **MATCH**。SEC-P1-1（修复后）、SEC-P1-2（R3 测试）**关闭**。
残留 SEC-P2-4：legacy `ai.cognition_item`/`ai.baseline_item` 无 append-only 触发器
（纵深防御项，不影响 MATCH）。

---

## 4. V-2 用例结果

| 编号 | 类别 | 操作 | 预期 | 实际 | DB / 泄漏证据 | 结论 |
|---|---|---|---|---|---|---|
| V2-T01 | Regression | A 读自己项目认知 + 知识 | 允许 | 200/200；确认写 200 | — | **PASS** |
| V2-T02 | Regression | A 读 B 项目认知 | 拒绝 | 404 | 响应不含 B 条目正文/项目名 | **PASS** |
| V2-T03 | Regression | A 用 B project_id 查 Knowledge | 拒绝 | 404 | 响应不含 B 知识正文/knowledge_id | **PASS** |
| V2-T04 | Regression | A 用 B 对象 id 在 A 路径确认 | 拒绝 | 404 | B 项目快照逐字不变 | **PASS**（对象 id 替换不可绕过租户过滤） |
| V2-T05 | Regression | A 修改 B 知识（确认 B 条目） | 拒绝 | 404 `project not found` | B 快照逐字不变 | **PASS** |
| V2-T06 | **UNKNOWN** | A 删除 B 对象 | 拒绝 | 旧栈 `ai.project_knowledge`/`ai.cognition_item` **无 DELETE 端点**（insert-only + 状态机） | — | 无删除面；保留 UNKNOWN（非关键） |
| V2-T07 | Regression | 批量 mix A/B 任务 | 拒绝越权 | 404（全有或全无） | A/B 两侧快照均不变；合法单批 200 且 taskA→ready | **PASS** |
| V2-T08-C | **Characterization** | A 引用 B 对象（`pc.source` / `kb.memory_ref`） | 拒绝 | **成功（缺陷现状）** | `pc.source.target_id=B item`；`kb.memory_ref` 两条 `target_id=B item` | SEC-P2-1/2 基线，不计通过；T08-R 待修复后启用 |
| V2-T09 | Regression | 同租户非项目成员读/写 A | 拒绝 | 404 + 404 | 快照不变 | **PASS** |
| V2-T10 | Regression | tenantA JWT → B project_id（legacy feed） | 拒绝 | 404 | 响应不含 B 通知 secret | **PASS** |

**V-2 判定**：SEC-P0×5 修复后回归全部拒绝且零副作用；HTTP 主路径隔离成立。
残留：SEC-P2-1/2 跨域引用目标不校验（T08-C 已固化）；T06 无删除面。
→ **PARTIAL**（原 MISMATCH 因 P0 关闭而下调；不等于 MATCH）。

---

## 5. P0 逐项回归（G1 复测复跑）

| 缺陷 | 攻击路径 | 攻击结果 | DB 副作用 | 合法路径 | 结论 |
|---|---|---|---|---|---|
| SEC-P0-1 events | A→B 注入事件 | 404 `project_not_found` | 无 inbound_event/job | A 自己项目 202 | PASS |
| SEC-P0-2 decision | A 裁决 B approval | 404 `approval_not_found` | approval 仍 pending、proposal 仍 proposed、无通知 | A 自己项目 200 rejected，`decided_by=humanA` | PASS |
| SEC-P0-3 veto | A veto B executed proposal | 404 | task 仍 completed/rev4、无补偿、无 intervention | A 自己项目 200 rolled_back + intervention=1 | PASS |
| SEC-P0-4 freeze | A 冻结 B 项目 | 404 | automation_mode 仍 `ai_lead` | A 自己项目 200 frozen→`controlled` | PASS |
| SEC-P0-5 notifications | A 读 B 通知 | 404 | 响应不含 B payload | A 自己项目 200 且返回 1 条 | PASS |

---

## 6. 失败用例与未关闭项

| 项 | 性质 | 处置 |
|---|---|---|
| SEC-P1-1（逐项决策无 human 主体门） | 已修复：`require_human_decision_context` 接入 cognition + baseline 两个入口 | **关闭**；T02/T05/EXT-01 转 Regression 后 403 全绿 |
| V1-T07-A（服务层授权边界） | 架构事实：无独立应用服务层；唯一入口 = 已覆盖的 HTTP handler | **关闭（无独立攻击面）** |
| V2-T06（跨租户删除） | UNKNOWN：旧栈无 DELETE 端点 | 责任=旧栈收敛（P5）；当前无攻击面 |
| V2-T08-R | 待 SEC-P2 修复 | 届时启用，计入通过 |
| SEC-P2-4（新增） | legacy `ai.cognition_item` / `ai.baseline_item` 无 append-only 触发器 | P2 纵深防御；不影响 V-1 MATCH |
| 存量 flaky `pc_proposals p3/p5` | 已知、干净 master 复现 | 不在本任务范围；本轮未复现 |

---

## 7. 覆盖边界（诚实声明）

- 本证据只覆盖 **L2 进程内 HTTP/near-service** 路径与列出的应用函数；
  不覆盖真实远端 Git、真实 LLM、真实通知/执行器外部副作用。
- 不能以本包证明「所有入口无旁路」——D03 访问链索引仍是入口全集的依据；
  本轮新增入口证据：`pm_baseline.rs::decide_item`（已纳入并加门）。
- Characterization 用例的「通过」仅表示**现状可复现**，不进入任何 G 门通过统计。

---

## 8. 附五回填（专项任务书 §附五）

见 `v1-v2-security-audit-taskbook-v1.md` 附五：阶段 B/C 执行记录与 D04 索引。

---

## 9. SEC-P1-1 修复与复测记录（本次执行）

**修复**（零 Schema、零 Migration）：

| 文件 | 改动 |
|---|---|
| `apps/pm-api/src/support.rs` | 新增 `require_human_decision_context`：① `auth_mode ∈ {interactive, off}`；② 经 `IdentityRepo::principal_kind` 查库验 `kind='human'`；失败 403（pm-api 零 SQL 门禁 PASS） |
| `crates/pm-application/src/repo/identity.rs` | `IdentityRepo` 新增 `principal_kind` port |
| `crates/pm-infrastructure/src/repo_identity.rs` | `PgIdentityRepo::principal_kind` 实现 |
| `apps/pm-api/src/pm_cognition.rs` | `decide_item` 在项目可见性后接入共享门 |
| `apps/pm-api/src/pm_baseline.rs` | `decide_item` 同门接入；`confirm_baseline` 既有三重门不变 |

**契约**：正式逐项确认主体 = 交互式真人（kind='human'）+ 项目成员；Agent/Service
可提案不可确认；`governance.approval_request` 不替代 ProjCognition 确认（V1-04）。
不复制 PC `ensure_formal_authorized` 的治理角色部分。

**测试**：
- `char_t02` → `regression_t02_agent_principal_cannot_confirm`（403）
- `char_t05` → `regression_t05_forged_claim_still_rejected`（403）
- `char_v1_extra_baseline_item_decision_accepts_agent` →
  `regression_v1_ext01_baseline_item_requires_human`（403）
- 新增 `regression_decision_rejects_api_key_human`（403）
- 复测：L2 三套件 **29/29 PASS** + `pm_cognition`/`pm_baseline` 既有 happy-path **4/4 PASS**（合计 33/33）；`guard-pm-api-sql.sh` PASS。

**G2 门**：负向 Regression 全通过；P0/P1 = 0；关键 UNKNOWN 关闭 → **G2 通过**。
V-2 仍 PARTIAL（仅 SEC-P2-1/2/3 引用链，非 P0/P1）。

---

## 10. 阶段 D（G3/G4）证据索引

阶段 D 的 P1 真实业务链执行证据独立成包，不与安全 L2 结论混计：

- 判定与终局：[`phase-d-g3-g4-verdict.md`](phase-d-g3-g4-verdict.md)（G2 复判 / G3 判定表 / E1 关联表 / G4 签收包 / DEF-CHAIN-01）；
- 11 字段用例记录：[`chain-e2e-test-cases-v1.md`](../../../test-case/chain-e2e-test-cases-v1.md)；
- 机器证据：`test-case/evidence/chain/`（playwright-results.json、pm-api/worker 日志、e1-object-identity.json、branch-*.json）；
- 执行结果：**10/10 PASS**（S1~S6 主链 + B1~B4 分支）；`e2e/chain/*.spec.ts` + `playwright.chain.config.ts`。


