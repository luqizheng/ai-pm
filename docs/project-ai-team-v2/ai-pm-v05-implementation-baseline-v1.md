# V0.5 联合实施基线 V1.0 —— WP-B/WP-D 架构裁决收编 + 分批验收清单

| 项目 | 内容 |
|---|---|
| 文档性质 | 联合实施基线（专家终审裁决整理产物；**架构裁决=通过，工程完成=以迁移/事务/测试/E2E 验收证据为准**——两态不得混标） |
| 来源 | 专家「WP-B / WP-D 联合架构裁决」（2026-11-07，用户转发；原文逐字见附录） |
| 上位文档 | Client Registry 实施设计 V1.0（`ai-pm-v05-client-registry-design-v1.md`，裁决收编）· ACP-04/05 联合数据契约 V1.0（`docs/scenarios/s03/ui/acp/smark-pm-s03-acp-slice-design-acp04-05-joint-v1.md`，裁决收编）· roadmap · ACP V1.2 |
| 代码基线 | ai-pm master @ b71b5c1（2026-11-07） |

---

## 1. 裁决总览

**通过项**：Q-A（cpk_ 凭据）/ Q-B（owner 锚点）/ Q-C（权限双轨）/ Q-D（0049 列集）/ 五问①~⑤ / 0050·0051 数据模型 / 实施顺序。

**三项工程约束（实施验收条件，不改变架构选择）**：

| 约束 | 内容 | 收编落位 |
|---|---|---|
| **A** | 凭据与 Device 唯一不可混淆绑定：只存 SHA-256；明文只返回一次；校验**不得退化为仅查 Hash 存在**——须同时验证 Device 状态、Tenant、Owner 成员资格、撤销状态；**cpk 认证失败不回退真人 JWT 路径** | 设计稿 §4.2 已收编（fail-closed 不回退 + 四项验证链）；验收 ACP-465 |
| **B** | Run 与 Device 租户一致性由**数据库约束**保障：`lease_client_id` 复合外键（或等效约束），杜绝跨租户关联 | 设计稿 §3 DDL 已改为复合 FK（需 device 上 `UNIQUE(tenant_id,id)` 陪伴约束）；验收 ACP-466 |
| **C** | 撤销检查覆盖**完整命令生命周期**：五工具 + 未来 discover/续领；已撤销 Client 不得轮询/续领/执行命令/**经既有连接或缓存绕过** | 设计稿 §7 撤销矩阵收编扩充（"每请求查库、不缓存"已是实现口径，矩阵补"命令执行中连接"行）；验收 ACP-436 扩面 |

**冻结身份表（不允许出现第四种 Session 身份）**：

| 身份 | 权威字段 | 归属 |
|---|---|---|
| Platform Session ID | `task_run.executor_session` | 执行平台 |
| 新 Session 标识 | `worker_command.receipt.session_id` | Worker Receipt |
| Continuation 身份 | `delivery.continuation.id` | ACP-05 |

**冻结原则（七条）**：身份/凭据/授权三分离 · Client 撤销每请求生效 · Delivery 不决定 Task 完成 · ACP-04→05 单向依赖 · Run/Session/Continuation 身份分离 · Consumption 事实持久化（审计不得经 Delivery 状态间接推断消费）· **Recovery Foundation 先于 ACP-05**。

## 2. 迁移台账实况勘定（诚实勘误）

专家裁决注记「0048 已承担 ACP 收敛」——**台账实况与此有出入，须以台账为准**：

| 号 | 台账实况 | 说明 |
|---|---|---|
| 0046 | `0046_acp_communication_face.sql`（已应用） | ACP 通信面 |
| 0047 | `0047_interaction_response.sql`（已应用） | ACP-03 |
| **0048** | **`0048_kb_pc_intake.sql`（已应用，KB-PC 批）** | **ACP 收敛列并未占用 0048**——原 roadmap 的「0048 = ACP 收敛」因双 0046 撞号顺移被 KB-PC 批取走（9f58473，DEF-01 同款纪律） |
| 0049 | **预留 = Client Registry（本裁决确认）** | 设计已出（WP-B），未落地 |
| 0050 | **预留 = ACP-04 Delivery（确认）** | 未落地 |
| 0051 | **暂定预留 = ACP-05 Continuation（暂定确认）** | 落地前重核 |
| ACP 收敛列 | **未落地，随 Recovery Foundation 批取号（tentative 0052）** | `worker_command.reacquired_generation` + `task_interaction.previous_chain_id` |

「不覆盖、不重新解释已有迁移」原则不变——本节仅勘定事实，防止按误记取号造成第三次撞号。

## 3. 实施顺序（裁决认可，Recovery Foundation 提前）

```text
0049 Client Registry → 0050 ACP-04 Delivery → Recovery Foundation → 0051 ACP-05 → ACP-06/07 → ACP-09
```

**Recovery 先于 0051 的理由（专家原文要旨）**：0051 的 ack 推进要求与 R-4 恢复同事务——恢复基础设施（续领/命令改路由/generation fencing）未建立时，会出现"数据模型先完成、恢复语义无法兑现"的倒挂。

**延续有效约束（不因顺序调整改变）**：R-11 续领与 Session 重启分离（ACP-C4）· 续领必须 generation fencing · deadline 不顺延（过期进 ACP-07）· `pm_discover_runs` 独立工具、poll 契约不动 · Chain 增 `previous_chain_id`。

## 4. 分批验收清单（工程完成的证据门槛）

### 批 1 —— 0049 Client Registry

| 类别 | 项 |
|---|---|
| 迁移 | §3 DDL（含**约束 B 复合 FK**：`device` 加 `UNIQUE(tenant_id,id)` 陪伴约束 + `task_run.lease_client_id` 复合外键）+ 回填三分类 SQL + 审计行 |
| 服务 | 凭据签发/撤销/轮换（单列两步）+ **约束 A 四项验证链**（hash/tenant/device status/revocation + owner active；cpk 失败 **401 不回退 JWT**）+ granted_permissions 裁剪 |
| 鉴权 | auth.rs cpk_ 分支（每请求查库）+ claim 写 `lease_client_id`/`lease_owner='client:{id}'` |
| 测试 | ACP-460~464 + **ACP-465（cpk 失败不回退）** + **ACP-466（跨租户绑定被复合 FK 拒绝）** + ACP-436 扩面（撤销后 poll/上报/执行/既有连接全断） |
| 证据门槛 | 迁移在干净库应用成功；守卫 C174/C180 登记新审计点；真实栈 Client Key 全链 E2E |

### 批 2 —— 0050 ACP-04 Delivery

| 类别 | 项 |
|---|---|
| 迁移 | `interaction_response_delivery` 全 DDL（UNIQUE(response_id) 等） |
| 服务 | I-1 六接口 + I-3 六点接线（0047 三写 / ack 三分支 / delivery_id 必填化 / resolution_note 补引 / unconsumed 指标切表） |
| 测试 | ACP-470~473 + 重投复用行断言 + acp_comm_face 既有 20 例回归 |
| 证据门槛 | 三写事务原子性（缺一回滚）；重投不新建行 |

### 批 3 —— Recovery Foundation（续领恢复基础设施）

| 类别 | 项 |
|---|---|
| 迁移 | ACP 收敛列（tentative 0052：`reacquired_generation` + `previous_chain_id`） |
| 服务 | `pm_discover_runs` + `pm_reacquire_lease`（**前置加 `lease_client_id IS NOT NULL` 门**）+ 同事务命令改路由 + F-1 guard 快照对齐 |
| 测试 | ACP-452/453/454/455 + E2E 必测②③⑥（离线超租约/迟到 ACK/未授权抢占） |
| 证据门槛 | 真实栈离线恢复全链（ Runner 重启→发现→续领→continue 同 Run 成功）；deadline 不顺延验证 |

### 批 4 —— 0051 ACP-05 Continuation

| 类别 | 项 |
|---|---|
| 迁移 | `delivery.continuation` 全 DDL（UNIQUE(run_id,generation)）+ 0050 `continuation_id` 回填 |
| 服务 | I-2 六接口 + acked 同事务推进 + 终态钩子 + frozen 镜像 |
| 测试 | ACP-474~477 + C2 回归钉（consumed 迁移不得触碰 Task/guard/执行面） |
| 证据门槛 | 消费事实持久化：审计不再依赖 Delivery 状态间接推断消费 |

### 批 5/6 —— ACP-06/07、ACP-09

按既定依赖推进；ACP-09 完整回归含两轮 E2E 必测合并清单（V1.2 §8 十项）。

## 7. 工程纪律（不变项）

1. 架构通过 ≠ 代码交付完成——每批以"迁移干净库应用 + 事务原子性 + 自动化测试 + E2E 证据"四件齐为完成。
2. 并行会话纪律：迁移号单一负责人分配；进共享分支后不得自建同号。
3. F-7 降级开关为**过渡架构不得永久保留**：lmms IAM 正式字典落地后必须统一清理（裁决原文）。
4. 全部审计新点四身份列全写（C174/C180 同步登记）。

---

## 附录：专家裁决原文（2026-11-07，逐字保留）

WP-B / WP-D 联合架构裁决

原则上全部通过，进入实施基线。

我的裁决是：接受你汇总的全部推荐方案，实施顺序也认可，但增加三项必须落实的工程约束。

这两块的架构方向是正确的，而且与此前 S03-ACP V1.3.1、第一至第三块已经形成的裁决能够衔接，没有必要重新讨论已经确定的架构方向。

本次不是重新开放方案比较，而是将尚未明确的工程约束补齐，形成可以执行的基线。

一、Q-A～Q-D：全部通过

| 事项 | 最终裁决 | 状态 |
|---|---|---|
| Q-A 凭据形式 | B：ai-pm 自发 cpk_ | 通过 |
| Q-B Owner 锚点 | 接受，Client 独立 Principal 留 V2 | 通过 |
| Q-C 权限双轨 | granted_permissions 列先行 | 通过 |
| Q-D 0049 列集 | 按提案四列、唯一约束、Run 外键 | 通过 |

特别确认三个原则：
1. cpk 是 Client 通道凭据，不取代真人 JWT。
2. Client 的授权有效性依赖 Owner 的项目成员资格，凭据撤销立即关闭 Client 通道。
3. 权限双轨是过渡架构，不允许永久保留 F-7 降级开关。lmms IAM 完成正式权限字典注册后，必须完成统一和清理。

增加的工程约束

约束 A：凭据与 Device 必须形成唯一、不可混淆的绑定。
- 凭据只保存 SHA-256 Hash。
- 签发明文只返回一次。
- 凭据校验不能退化为仅检查 Hash 是否存在。
- 必须同时验证 Device 状态、Tenant、Owner 成员资格和撤销状态。
- cpk 认证失败不能回退到真人 JWT 认证路径。

约束 B：Run 与 Device 的租户一致性必须由数据库约束保障。
- task_run.lease_client_id 不应只保证引用了一个存在的 Device，还必须保证 Run 与 Device 属于同一个 Tenant。
- 建议采用复合外键，或等效的数据库级一致性约束，避免跨 Tenant 关联。

约束 C：撤销检查覆盖完整命令生命周期。
- 不仅包括五个现有 MCP 工具，还包括未来的 discover、续领等入口。
- 特别是：已撤销 Client 不得继续轮询。不得继续续领。不得继续执行命令。不得通过已建立的连接或缓存绕过撤销检查。

这三项属于实施验收条件，不改变 Q-A～Q-D 的架构选择。

二、ACP-04/05 五问：全部通过

| 问题 | 最终裁决 |
|---|---|
| ① Delivery 关联 Run + 四个命令锚点，不关联 Session | 通过 |
| ② Consumption 表增加 generation，工具签名不变 | 通过 |
| ③ Delivery 不参与 Task 完成判定 | 通过 |
| ④ ACP-05 只读消费 ACP-04，仅回填 continuation_id | 通过 |
| ⑤ Session 三种身份分别定义，不临时另建 | 通过 |

这里特别冻结以下身份定义：

| 身份 | 权威字段 | 归属 |
|---|---|---|
| Platform Session ID | executor_session | 执行平台 |
| 新 Session 标识 | receipt.session_id | Worker Receipt |
| Continuation 身份 | continuation.id | ACP-05 |

不允许后续实施为了方便关联而再增加第四种 Session 身份。

0050 / 0051 数据模型裁决

0050：interaction_response_delivery，接受：
- UNIQUE(response_id)。
- 一条 Response 对应一条 Delivery 账。
- 失败重投复用原行。
- attempt_count + 1。
- 不因重投创建新 Delivery。

0051：delivery.continuation，接受：
- UNIQUE(run_id, generation)。
- continuation_generation 与 lease_generation、attempt_no 完全分离。
- 状态机 open → started → consumed/closed，并支持 frozen。
- Ack 推进与 R-4 恢复同事务。
- Run 终态、Task 取消驱动关闭。

另外，正式保留 ACP-C2 的收口方向：consumed 从推导态转变为持久化事实。这意味着后续审计不能再依赖 Delivery 状态间接推断是否消费。

三、迁移号：0050 / 0051 暂定认可

| 迁移号 | 内容 | 裁决 |
|---|---|---|
| 0049 | Client Registry | 确认 |
| 0050 | ACP-04 Delivery | 确认 |
| 0051 | ACP-05 Continuation | 暂定确认 |

保留落地前重核迁移台账的要求。

这里需要注意：0048 已经承担 ACP 收敛，0049 承担 Client Registry。不能因为新设计调整而覆盖或重新解释已有迁移。

四、实施顺序：认可，明确依赖关系

最终实施路线：
1. 0049 — Client Registry：V-05-1 身份、凭据、验证、撤销、绑定、权限字典。
2. 0050 — ACP-04 Delivery：Response、Command、Delivery 三写事务，投递幂等与消费挂账。
3. Recovery Foundation — 续领恢复基础设施：建立可靠的恢复与 fencing 能力，确保 Ack 与 R-4 恢复原子性。
4. 0051 — ACP-05 Continuation：Continuation 身份注册、代次隔离、状态机及消费回执。
5. ACP-06 / ACP-07：按既定依赖推进后续调度、续领及过期处理。
6. ACP-09：最后完成后续收口。

我认可这个顺序，尤其不建议把 Recovery Foundation 放到 0051 之后。原因是 0051 的 Ack 推进和 R-4 恢复要求同事务。如果恢复基础设施尚未建立，就容易出现数据模型先完成、恢复语义却无法兑现的问题。

另外，延续此前已经确定的约束：
- R-11 续领与 Session 重启分离。
- 续领必须具备 generation fencing。
- deadline 不顺延，过期进入 ACP-07。
- pm_discover_runs 新增，但保留原 poll 契约。
- Chain 增加 previous_chain_id。
这些不因本次迁移顺序调整而改变。

五、最终交付范围

| 工作范围 | 处理结论 |
|---|---|
| WP-B：V-05-1 Client Registry | 本次通过 |
| WP-D：ACP-04/05 联合数据契约 | 本次通过 |
| 撞号修复 | 已交付，保持 |
| Audit 守卫收口 | 已交付，保持 |
| team_llm 存量红修复 | 已交付，保持 |

此前已确认的 ACP-448、ACP-450/451、ACP-452/453 等设计裁决继续有效，不重新开放。

六、最终冻结意见

WP-B / WP-D 架构决策：通过。

Q-A～Q-D + 五问①～⑤：全部接受推荐方案，附加三项工程约束。

冻结的关键原则：
- 身份、凭据、授权三分离。
- Client 撤销每请求生效。
- Delivery 不决定 Task 完成。
- ACP-04 → ACP-05 单向依赖。
- Run、Session、Continuation 身份分离。
- Consumption 事实持久化。
- Recovery Foundation 先于 ACP-05。

实施顺序：0049 → 0050 → Recovery Foundation → 0051 → ACP-06/07 → ACP-09。

我的最终判断：这两块可以结束架构方案讨论，转入实施设计和代码验收。

但要区分两个状态：
- 架构裁决：本次通过。
- 工程完成：仍须以迁移、事务实现、自动化测试和 E2E 验收结果为准。
不要把架构通过直接标记为代码交付完成。

下一步最有价值的是将本次裁决整理成一份联合实施基线，并按迁移顺序逐项核对代码、数据库约束和 ACP-470～477 验收证据。

**文档结束**
