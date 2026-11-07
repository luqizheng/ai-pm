# S03-ACP 实施设计 · ACP-03（Human Response：REST/UI 回复入口 + 权限模型）V1.0
# —— §23.3"人工回复权限与项目隔离"闭合

| 项目 | 内容 |
|---|---|
| 文档性质 | ACP 切片实施设计稿（待评审、待用户裁决） |
| 设计基线 | S03-ACP V1.3.1（冻结：D-01~07）· 通信面切片实施设计 V1.1（已冻结为 ACP-01/02/08 基线，复审通过 @85fde61） |
| 前置裁决 | R-ACP-1~9（2026-11-07 全落）；R-ACP-7 = terminate 失败仅冻结启动、**回复提交开放** |
| 覆盖 | ACP-03：REST 回复入口、Human Response 持久化（§12.2 interaction_response）、审计、幂等、§8.3 六项权限检查操作化、与 §8.5 三层的"持久化→投递"衔接 |
| 不覆盖（另稿） | ACP-04（interaction_response_delivery 表 + Delivery/Consumption 状态机——V1 以 continue 命令承载投递事实，见 §6）· UI 页面（Personal Workspace Inbox 投影，随 WP-5 前端）· step-up 确认票据（I6-A 候选 c，另立） |
| 代码基线 | feat/s03-acp-comm-face @ 1767d36；关键事实见 §2 |
| 关联裁决 | A*（正式确认仅可信 REST/UI，MCP 无确认入口）· I6-A（Lead + 显式 Approver；approver_grant 0040）· D-04（人类 REST/UI 提交，Worker/Adapter 消费）· R-ACP-7 |

---

## 1. 范围一句话

人类经**可信 REST**（UI 调用同一端点）对一条 `pending` 的 `delivery.task_interaction` 提交回复：平台在**同一事务**内完成 §8.3 六项检查 → `interaction_response` 落行（§12.2 全字段）→ Interaction `pending→responded`（guard `human_wait→start_wait`，复用 V1.1 §4.1）→ **同事务下发 `agent.task.continue` 命令**（携带回复 + guard 快照，V1.1 §1.4 三阶段的平台侧起点）→ 审计。此后链路（执行端消费/消费回执）已由 84c67e9/1234162 实现。

---

## 2. 代码事实基线（引用）

| # | 事实 | 证据 |
| --- | --- | --- |
| C1 | REST = 薄适配：handler 持 `Extension(AuthedPrincipal)` → 租户/成员门 → 直调 `pm-application` 服务（`pool + actor`）；**无 REST 权限码中间件**（`has_permission` 仅 MCP 内层） | `pm_requirements.rs:168-190,389-392`、`auth.rs:601` |
| C2 | `ensure_trusted_context`：`auth_mode == "api_key"` → 403（confirm/reject/withdraw/restore/archive 在用）——auth_mode 准入消费先例（I6-A 核验文档"零消费"结论已被此先例超越） | `pm_requirements.rs:47-54`、`auth.rs:133-148` |
| C3 | I6-A 治理授权：`ensure_governance_authorized(pool, project_id, principal)` = `is_project_lead`（project_member.role='owner'）OR `core.approver_grant` 生效行（0040，UNIQUE(project_id,principal_id)+revoked_at）；前置 `require_human`（principal.kind='human'） | `approver.rs:38-75`、`0040:30-46` |
| C4 | 成员门：`require_project_visible`（不可见→404 不泄露）/ `require_task_project`（task→project→成员复核）现成 | `support.rs:135-199` |
| C5 | 错误封套：`ApiError{BadRequest,Conflict,NotFound,Forbidden,Internal}` → `{message}`；`PmWriteError→ApiError` 映射在 `support.rs:46-53`；**`From<InteractionError>` 缺**（需补） | `support.rs:14-53` |
| C6 | 挂钩点：`interaction::record_response_persisted(pool,…)`（pool 版）= pending→responded + guard start_wait + 审计——ACP-03 需**事务核化**（同 §6） | `interaction.rs:355-401` |
| C7 | `issue_continue_command(tx,…)` 已带 guard 快照（frozen/deadline，F-1）；需 `run_id/worker_id/lease_generation` | `worker_command.rs:97-110` |
| C8 | 迁移台账：现最大 0046（ACP 通信面）→ 本切片取 **0047**（落地前重核） | `docs/ai-pm-migration-ledger.md` |

---

## 3. REST 契约

```text
POST /v1/pm/interactions/{interaction_id}/responses
Authorization: Bearer <interactive 令牌>
{ "response_payload": {…}, "idempotency_key": "<client 生成，UUID>" }
→ 200 { "response_id", "interaction_id", "status": "responded",
        "replayed": false, "continue_command_id": <uuid|null> }
  409 { "message": "…" }        ← 并发竞争失败（§9.3）/状态不允许
  403 / 404 / 400                ← 检查②③ / ① / ⑥
```

- UI（Personal Workspace Inbox 投影）调用同一端点——前端页随 WP-5，本稿只锁契约。
- 无独立权限码（C1：REST 面本就无 V-perm 基建）；准许 = §4 六项检查。若 V-05-1 落 REST 权限字典，可在迁移批补 `PM_INTERACTION_RESPOND` 行——挂待裁 R-ACP-12。

---

## 4. §8.3 六项权限检查操作化（全部 fail-closed）

| # | 基线检查（§8.3 原文） | 操作化 | 实现锚点 |
| --- | --- | --- | --- |
| ① | 当前用户是否属于目标 Project | interaction → task → project；`require_project_visible`（不可见 404 不泄露）；令牌 tenant 绑定复核 | `support.rs:153-165` + handler |
| ② | 当前用户是否具有相应操作权限 | **auth_mode ≠ api_key**（`ensure_trusted_context` 同款）+ `require_human`（principal.kind='human'，C3 前置） | C2/C3 先例复用 |
| ③ | 当前用户是否为 Project Lead 或被明确授权的 Approver | **按 reason 分级（R-ACP-10 待裁）**：`needs_approval` → `ensure_governance_authorized`（Lead OR approver_grant）；`needs_info / needs_decision / external_dependency` → 项目成员（检查①已过） | `approver.rs:51-75` |
| ④ | Interaction 是否仍处于允许回复的状态 | `status='pending'`（FOR UPDATE 行锁内判定；responded/consumed/timed_out/cancelled 一律 409）；`continuation_start_frozen_at` **不阻**提交（R-ACP-7：仅冻结启动） | §6 步骤 3 |
| ⑤ | 是否已经超过 deadline_at | `now() <= deadline_at`（belt-and-braces：§8.2 清扫已把过期行翻 `timed_out`，此处防清扫间隙竞态） | §6 步骤 3 |
| ⑥ | 当前回复是否符合要求的响应结构 | `response_payload` 非空；`required_response=true` 且 `options` 非空时，payload 须含 `decision` 且取值 ∈ options[].value（自由文本走 `content` 字段，两者至少其一） | 服务层校验 |

任何一项不满足 → fail-closed（403/404/409/400），无部分成功。

---

## 5. 数据模型：`interaction_response`（迁移 0047，落地前重核台账）

按 V1.3.1 §12.2 全字段：

```sql
CREATE TABLE delivery.interaction_response (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),   -- response_id
    tenant_id     UUID NOT NULL REFERENCES core.tenant(id),
    project_id    UUID NOT NULL REFERENCES core.project(id),
    interaction_id UUID NOT NULL REFERENCES delivery.task_interaction(id),
    task_id       UUID NOT NULL REFERENCES delivery.task(id),
    run_id        UUID NOT NULL REFERENCES delivery.task_run(id),
    responded_by  UUID NOT NULL REFERENCES core.principal(id),
    response_payload JSONB NOT NULL,
    idempotency_key  TEXT NOT NULL,
    authorization_context JSONB NOT NULL,     -- 六项检查结果快照（谁/何通道/何角色/检查①~⑥结论）
    continue_command_id UUID,                -- 同事务下发的 continue 命令（§6 步骤 6；V1 投递事实，ACP-04 补 delivery 表）
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, id)
);
-- 幂等：同 (interaction_id, idempotency_key) 重放返回原行
CREATE UNIQUE INDEX uq_interaction_response_idem
    ON delivery.interaction_response (interaction_id, idempotency_key);
-- §9.3 并发裁决：同 Interaction 至多一条有效回复（乐观并发，败者 409）
CREATE UNIQUE INDEX uq_interaction_response_single
    ON delivery.interaction_response (interaction_id);
```

不可覆盖（§8.4）：无 UPDATE 路径；后续修改 = 新审计记录，不改行。

---

## 6. 回复服务事务（`pm-application`，V-6 单事务纪律）

新模块 `interaction_response.rs`（或并入 `interaction.rs`，落位随实施）：

```text
1. interaction FOR UPDATE（project 校验，不可见 → NotFound）
2. 检查②③（handler 层已做 auth_mode/require_human/分级授权——服务层二次断言，
   双层 fail-closed）
3. 检查④⑤：status='pending' 且 now() <= deadline_at（违规 → Conflict）
4. 检查⑥：response_payload 结构校验（违规 → Validation）
5. INSERT interaction_response（幂等键命中 → 直接返回原行，replayed=true）
6. record_response_persisted 事务核化调用：pending → responded +
   guard human_wait → start_wait（V1.1 §4.1 释放点不变）
7. worker_command::issue_continue_command（同事务；worker_id=run.lease_owner
   快照、lease_generation 快照、response=§5.4 报文 + guard 快照自动生成）
   —— 回写 interaction_response.continue_command_id
8. 审计 interaction.responded（沿用 record_response_persisted 现有审计，
   authorization_context 一并落 after）
```

**与 §8.5 三层的衔接**：持久化（本事务）→ 投递（continue 命令 = V1 投递事实；`interaction_response_delivery` 表随 ACP-04 补登 DELIVERED/CONSUMED 细账，`delivery_id` 关系前向兼容）→ 消费回执（已实现：`pm_report_consumption` 双证据）。

---

## 7. 幂等与并发

| 场景 | 行为 |
| --- | --- |
| 同 idempotency_key 重放 | 返回原 response_id + `replayed=true`，零副作用（§8.4） |
| 两个有权限者同时回复（§9.3） | `uq_interaction_response_single` 败者 409 Conflict；先到者生效；无静默覆盖 |
| 回复到达早于 terminate 回执（§4.4/ACP-421） | 持久化成功、continue 照常下发；执行端由 guard/preflight 拒绝启动（84c67e9/1234162 已闭合）——回复不丢失 |
| 回复到达晚于 deadline（§8.2） | 检查⑤拒绝；Interaction 已被清扫翻 `timed_out` → 检查④拒绝；重试须走 `retry_interaction` 挂链（V1.1 §5.2） |

---

## 8. 验收标准（顺延编号）

| 编号 | 场景 | 预期 |
| --- | --- | --- |
| ACP-446 | 合法回复全链 | response 落行 + interaction=responded + guard=start_wait + continue 命令同事务下发 + 审计含 authorization_context |
| ACP-447 | api_key 令牌回复 | 403（检查②；interactive 先例 C2） |
| ACP-448 | needs_approval 且非 Lead/Approver | 403（检查③治理级）；approver_grant 授予后 200 |
| ACP-449 | 非 pending 状态回复 | 409（检查④；consumed/timed_out/cancelled 各一） |
| ACP-450 | deadline 过后回复 | 409（检查⑤；清扫间隙竞态亦被拒） |
| ACP-451 | options 不符（decision 越界/缺失） | 400（检查⑥） |
| ACP-452 | 同 idempotency_key 重放 | 原行返回 + replayed=true + 零副作用 |
| ACP-453 | 双人并发回复 | 一成功一 409；无静默覆盖（§9.3） |
| ACP-454 | 回复先于 terminate 回执（ACP-421 对齐） | 持久化成功 + continue 下发；执行端 preflight 拒绝启动 |
| ACP-455 | continuation_start_frozen 存在时回复 | **提交成功**（R-ACP-7：仅冻结启动） |
| ACP-456 | 项目不可见成员回复 | 404（不泄露存在性） |

---

## 9. 待裁决清单

| # | 事项 | 选项 | 倾向 |
| --- | --- | --- | --- |
| R-ACP-10 | 检查③分级 | a) reason 分级：needs_approval→治理级（Lead/Approver），其余→项目成员（本稿） b) 全量治理级（§8.3 check3 字面） | **a**。needs_info/needs_decision 的问题本就面向执行协作角色；全量治理级会把普通澄清也压到 Lead，与 task_waiting 语义错位。若裁 b，检查③统一走 `ensure_governance_authorized` |
| R-ACP-11 | 回复是否同事务下发 continue | a) 是（本稿，V1 投递事实 = 命令；delivery 细账随 ACP-04 补） b) 等 ACP-04 delivery 表再接 | **a**。否则回复后无人驱动投递（Runner 轮询等 continue）；ACP-04 落表时前向兼容（continue_command_id 已留） |
| R-ACP-12 | REST 准许载体 | a) 六项检查（无权限码，本稿） b) 增 `PM_INTERACTION_RESPOND` 权限码（随 V-05-1 字典） | **a 先行 + b 预留**（V-05-1 落字典时补码不破契约） |
| R-ACP-13 | 迁移号 | 0047（interaction_response 一表） | 落地前重核台账；与 F-7 撤除开关同批评估 |

---

## 10. 工程纪律引用

1. 全部写入经 `pm-application`（V-6）；REST/MCP 仅适配层。
2. `From<InteractionError> for ApiError` 随实施补（C5 缺口）。
3. 迁移号落地前重核台账；禁 sweep、禁 `git add -A`（主仓并行会话在途）。
4. commit 时机由用户定；本稿落 `docs/ui/s03-acp/`。

---

**文档结束**
