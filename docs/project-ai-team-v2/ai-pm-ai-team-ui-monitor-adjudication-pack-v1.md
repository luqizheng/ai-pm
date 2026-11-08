# AI Team UI 批前置裁决包 V1.0（U-1 Monitor 数据面 + U-2 管理界面范围）

> 状态：**已裁决 · 2026-11-08（U-1=C，U-2=A，见 §〇 裁决记录）**
> 日期：2026-11-08
> 目的：AI Team 管理界面（Execution Monitor / Overview）动工前的最后一组产品决策。代码侧其余工作（Batch H 清偿批）不依赖本包，已并行推进。
> 依据：开工批实施记录（`ai-pm-ai-team-kickoff-batch-record-v1.md`）挂账 #4、比对报告 §五.3、对齐核验 §七.6。
> 裁决方式：回复如「U-1=C，U-2=A」或逐项修改。

## 〇、裁决记录（2026-11-08）

| 编号 | 裁决项 | 用户裁决 |
| --- | --- | --- |
| U-1 | Execution Monitor 数据面 | **C** 先聚合现有 audit/outbox 落 UI，B（run 事件表）挂 Phase 4 |
| U-2 | 管理界面范围（=稿 Q4） | **A** 完整管理页面，分步：Roles → Overview → Model Policies → Execution Monitor → Execution Policies |

附带：V1.0 稿 Q4 就此关闭（=A）；稿 Q2 维持挂账（代码事实近 A，随时可顺手裁）。自本记录起 UI 批按 §三 A 落地序排期。

---

## 一、裁决总览

| 编号 | 裁决项 | 选项 | 推荐 |
| --- | --- | --- | --- |
| U-1 | Execution Monitor / 运行观测的数据面 | A / B / C | **C（先 A 后按需 B）** |
| U-2 | AI Team 管理界面范围（= V1.0 稿 Q4，编号≠V2.0 基线 Q1-Q3） | A / B | **A**（随 U-1=C 分步落地） |

---

## 二、U-1：Monitor 数据面

### 现状（代码事实）

- 只读面已有：`GET /v1/pm/task-runs`（Run 列表）+ `GET /v1/pm/task-runs/{id}`；前端 `api/execution.ts` 已写好但**零消费**；Command Center 数据为 fixtures。
- 执行事件持久化现状：**库内无 run 事件表**；事件数据散在 outbox（15 类）、`governance.audit_log`（claim/skip/reclaim + 命令审计）、执行端磁盘 executor-events.jsonl（库外，`port.rs:141-151`）。
- 已落地的观测资产：`policy_snapshot`/`release_reason`/`scheduling_shield`（Run 行上）、`llm_call_record`（含 conversation_reply）、审计 SSE 流（`/v1/pm/events/stream`）。

### 选项

**A. 聚合现有审计/outbox，不建事件表**
- Monitor 端点按 run 聚合 `audit_log`（source_kind=run_dispatch + 命令审计）+ `task_run` 行状态 + `llm_call_record`，前端消费。
- 零迁移、立即可用；缺点：无阶段级事件粒度，深查靠审计翻译。

**B. 新建 run 事件表**（0012 名字被占，需新名如 `delivery.task_run_event`）
- claim/progress/submit/interaction/checkpoint 等节点写入，阶段级可观测。
- 干净但引入新写入面 + 保留策略治理（容量/清理），成本一整个迁移+双端改造。

**C. 折中（推荐）**：先 A 落地 UI 与聚合端点；B 作为 Phase 4 挂账，等真实观测需求（如阶段耗时分析）出现再建。与 D-4=A「V0.5 口径先行」同一哲学。

### 影响面 / 解锁

- A：新增 1 个聚合读端点（零迁移）+ 前端消费 `api/execution.ts`（首次真消费）。
- **解锁 UI 批**：U-2 的 Execution Monitor 页可直接动工。

---

## 三、U-2：管理界面范围（V1.0 稿 Q4）

### 现状

- `/project/settings/ai` 占位页；五子页（Overview/Roles/Agent Clients/Model Policies/Execution Policies）全缺；Execution Monitor 无页面。
- **后端已就绪可支撑的部分**：Roles（D-1 三端点齐）、Model Policies（0043 GET/PUT 项目级+租户 Provider 页已有）、Devices（settings/runners 页已在）、会话/回复（Batch G）。

### 选项

**A. 完整管理页面（推荐，分步）**
- 入口照稿：Project Settings → AI Team，五子页 + Execution Monitor。
- 落地序随 U-1=C：Roles 子页（后端全齐，纯前端）→ Overview（聚合只读）→ Model Policies 项目级页 → Execution Monitor（audit 聚合）→ Execution Policies（四层策略 UI，最重，最后）。

**B. 只做基础配置**
- 仅 Roles + Model Policies，执行观测维持 task-runs 列表页现状。

### 推荐：A（分步）

理由：后端四子页数据面已就绪大半；稿 Q4 推荐 A 且理由成立（AI Team 是基础设施须可观察）；分步序把最重的 Execution Policies 放最后，不阻塞早期价值。

### 影响面 / 解锁

- 纯前端为主（+U-1A 一个聚合端点）；解锁后按上面落地序排 1~2 个前端批次。
- 附带：`/project/settings/ai` 占位页由 AI Team 子页替换。

---

## 四、回复模板

```text
U-1=C（或 A / B）
U-2=A（或 B）
```

两项目前均无代码在途依赖；裁决后 UI 批即可排期。
