# S03-ACP 实施设计 · ACP-06 / ACP-07 联合 V1.0
# —— Checkpoint 登记 / Workspace Lock × Deadline 重新授权

| 项目 | 内容 |
|---|---|
| 文档性质 | 联合实施设计稿（批 5/6 前置设计；待评审） |
| 上位裁决 | ACP V1.3.1 §6（Checkpoint）/ §9.4（Deadline 校验）/ §12.7（Workspace Lock）/ §12.9（Execution Authorization）；联合裁决：T3 deadline 不顺延 · 到期后禁自动恢复、回复保留、进 ACP-07（专家第 1/2 块批准）· **ACP-05/06/07 经 `get_open_by_run` 明确接口接入，不吸收其职责**（ACP-05 边界）· Deadline 到期 UI 快捷入口直达重授权（专家 2.3 五约束） |
| 排期位 | 联合实施基线 §3 批 5（0054 起 tentative；0051 已落）；实施顺序 = 本设计 → 0054 → ACP-09 完整回归 |
| 代码基线 | ai-pm master @ 6bbabf0（批 4 已落：0049/0050/0052/0051 四迁移 + Recovery + Continuation） |

---

## 0. 接入点声明（ACP-05 边界兑现）

两个能力都经 `continuation::get_open_by_run(run_id)` 接入，不吸收对方职责：

- **ACP-06**（Checkpoint 登记 / Workspace Lock）：Checkpoint 是辅助上下文（基线 §6：Workspace 才是工程状态权威）——登记面挂 continuation 生命周期（frozen/started 期均可登记）；Workspace Lock = 同一 Workspace 的**新 Session 启动互斥**（ACP-C4 的物理面）。
- **ACP-07**（Deadline 重新授权）：`execution_authorization` 独立账（基线 §12.9）——授权 = 「过期 Deadline 下启动新 Session 的唯一合法凭证」，消费点 = §9.4 检查 6 + §3.6 续领前置（deadline 门）。

## 1. ACP-06：Checkpoint 登记（最小面）

### 1.1 事实与原则

- 基线 §6.1：Checkpoint 保存在 Workspace（`.aipm/checkpoint.json`），**Platform 不保存正文**，只存登记元数据（基线 §6.1 清单）。
- 基线 §6.3：Agent 可在 `interaction.required` 前写入；**Checkpoint 不存在不阻止介入**；Worker 不等 Checkpoint、不判真实性；无 Checkpoint → 新 Session 从 Task+Workspace 重建（既有 F-1 guard 快照已承载）。
- V0.5 现实（E2E 报告）：opencode/codex Adapter 未实现 checkpoint 写入——本设计按「登记面先行、写入面随 Adapter」分级。

### 1.2 数据模型（迁移 0054）

```sql
CREATE TABLE delivery.task_checkpoint (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    task_id       UUID NOT NULL REFERENCES delivery.task(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    interaction_id UUID REFERENCES delivery.task_interaction(id),
    -- 基线 §6.1 登记元数据
    file_path     TEXT NOT NULL,          -- Workspace 相对路径（.aipm/checkpoint.json）
    content_hash  TEXT NOT NULL,          -- sha256 小写 hex（执行端上报）
    file_size     BIGINT NOT NULL,
    session_id    TEXT,                   -- 关联 Session（R-8 口径 executor_session）
    registered_via TEXT NOT NULL DEFAULT 'executor_report'
                  CHECK (registered_via IN ('executor_report', 'worker_receipt')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);
CREATE INDEX idx_checkpoint_run ON delivery.task_checkpoint (run_id, created_at DESC);
```

### 1.3 登记通道（两条，都走统一应用服务）

| 通道 | 触发 | 语义 |
| --- | --- | --- |
| executor_report | 执行端经 `pm_report_checkpoint`（新工具，PM_MCP_AGENT_REPORT，租约三要素）上报 | Agent 侧事实上报（永无确认语义）；重复 checkpoint_id 幂等返回原 id |
| worker_receipt | Worker/Runner terminate 收割时发现 `.aipm/checkpoint.json` → 读取 hash/size 附入 terminate ack receipt | Worker 不判真实性（基线 §6.3.5）；Platform 收 ack 后登记 |

### 1.4 消费

- 新 Session 启动（continue 命令 payload）追加 `checkpoint` 引用（§11.3 初始上下文 = 原始任务 + 执行上下文 + Checkpoint + 回复；**不含正文**——执行端按 file_path 从 Workspace 自读）。
- 无 Checkpoint → payload `checkpoint: null`，Agent 从 Task+Workspace 重建（既有行为，零改动）。

## 2. ACP-06：Workspace Lock（新 Session 启动互斥）

- **锁对象**：Workspace（`{workspace_root}/{run_id}` 已按 Run 隔离——C11 无删除 Run 工作区代码）。
- **V1 结论**：Run 隔离本身已提供单写者语义（一个 Run 一个目录；B3-2 新 attempt Run 经 `supersedes_run_id` 继承同一 Workspace）——**唯一真实竞争面 = 情形 5/6 下旧进程未确认退出 + 新进程已启动**，该互斥已由 **决策表情形 6 + ACP-C4**（terminate 确认前置）承载。
- **裁决建议（Q-W）**：V1 **不引入独立 Workspace Lock 表**——以「情形 6 + terminate 确认 + run 隔离目录」为等效实现，登记为对基线 §12.7 的操作化修订（R-ACP-15 同规格）；`task_workspace_lock` 表留待多 Runner 同 Workspace 场景（V2）再评估。
- 保底硬约束：`AI_PM_WORKSPACE_ROOT` 同目录并发写 = 运维配置错误（两 Runner 绑定同 Workspace 属登记校验拒绝项——device binding 面已覆盖）。

## 3. ACP-07：Deadline 重新授权

### 3.1 数据模型（0054 同批）

```sql
CREATE TABLE delivery.task_run_execution_authorization (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    interaction_id UUID REFERENCES delivery.task_interaction(id),
    -- 独立审计记录 + 独立有效期（专家 2.3 五约束）
    authorized_by UUID NOT NULL REFERENCES core.principal(id),
    reason        TEXT NOT NULL,           -- expired_deadline / frozen_disposition / manual_extension
    new_deadline_at TIMESTAMPTZ NOT NULL,  -- **新独立有效期**（不改 run.deadline_at）
    granted_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL,    -- = new_deadline_at（授权即有效期边界）
    revoked_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);
```

### 3.2 语义（全部为已裁原则的落表）

| 原则 | 落表 |
| --- | --- |
| 不直接修改原 deadline | `run.deadline_at` 零改动；授权行自带 `new_deadline_at` |
| 不自动延长人工决策有效期 | 交互 deadline（task_waiting.deadline）不动 |
| 已持久化回复不丢弃 | 授权不影响 interaction_response / delivery |
| 独立审计 + 独立有效期 | 本表即审计（+ governance 审计行）；有效期 = expires_at |
| 未授权前禁自动启动 | 消费点见 §3.3 |

### 3.3 消费点（唯一合法过期启动凭证）

1. **§3.6 续领前置**：`reacquire_lease` 的 deadline 门改为——`run.deadline_at > now()` **或** `存在未撤销且未过期（expires_at > now()）的授权行`（授权有效期内续领 = 合法；授权过期后恢复拒）。
2. **continue §9.4 检查 6**（F-1 guard 快照）：payload `agent_deadline_at` 追加 `execution_authorization_id`；过期场景无有效授权 → 执行端 preflight 拒绝（既有 F-1 机制）。
3. **UI 快捷入口**（专家 2.3）：`POST /v1/pm/runs/{id}/execution-authorization`（Lead/Approver 守卫，body={reason, new_deadline_at}）——直达 ACP-07 重授权；审计 + 通知面。

### 3.4 撤销

`DELETE /v1/pm/runs/{id}/execution-authorization/{auth_id}` → `revoked_at = now()`（软撤；已消费的启动不追溯——启动事实在 worker_command ack）。

## 4. 验收标准（ACP-478~483）

| 编号 | 场景 | 必须保证 |
| --- | --- | --- |
| ACP-478 | 执行端上报 Checkpoint | 登记元数据落表；重复 checkpoint 幂等；Platform 不存正文 |
| ACP-479 | 无 Checkpoint 介入/继续 | 不阻止介入；continue payload checkpoint=null；Agent 重建上下文 |
| ACP-480 | terminate 收割发现 checkpoint 文件 | worker_receipt 通道登记；不判真实性 |
| ACP-481 | Deadline 过期 + 有效授权 | 续领通过（授权有效期门）；continue preflight 携 execution_authorization_id |
| ACP-482 | Deadline 过期无授权 | 续领/启动拒绝；REST 授权后放行；撤销后下次拒绝 |
| ACP-483 | Workspace 互斥 | 情形 6 未确认禁启（既有）+ 两 Runner 同 Workspace 登记拒绝 |

## 5. 范围与纪律

- 迁移 0054（两表一批，tentative——落地前重核台账；0053 已被 KB-PC checkpoint 占）。
- 不建 Workspace Lock 表（Q-W 裁决建议）；`pm_report_checkpoint` 不挂 READ（§1.6 同规格）。
- Checkpoint 正文永远在 Workspace（四不清理）；Platform 仅元数据。
- C174/C180：新审计点登记（task_checkpoint 无 project 审计则登记tenant面或说明）。
- ACP-09（贯穿验证）：定向回归已随批 1/2/3/4 单测层完成（ACP-448/450/451/452~455/460~467/470~477）；**完整回归 = 真实栈 E2E 联调批**（Runner Key 全链 + 离线恢复 + DSH 无证据 + 迟到 ACK + 多次介入链 + 06/07 场景），挂账至联调批执行。

---

**文档结束**
