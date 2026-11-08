# S03-ACP · ACP-09 完整回归 E2E 报告 V1.1（本机全栈）

| 项目 | 内容 |
|---|---|
| 文档性质 | ACP-09 贯穿式验证 · 阶段 B 完整回归 E2E 报告（本机全栈形态） |
| 环境 | 本机真实进程：pm-api debug 二进制（jwks 模式，JWKS=192.168.2.10:3000 lmms 真签发）+ 真实 MCP HTTP（Streamable）+ 真实 PG（infrastructure-aipm-pg-1:16433 独立库 e2e_acp09，迁移 0001~0055 全应用；V1.0 写 0054，0055 已顺延）+ F-7 过渡开关开启（AI_PM_MCP_LEGACY_PERMISSION_GRANT=1，admin JWT 无 ACP 权限码） |
| 驱动 | scripts/e2e/acp09_e2e.py（REST + MCP JSON-RPC 混合驱动）+ scripts/e2e/acp09_reset.py（确定性重置：acp09_reset.sql @@标记哈希替换） |
| 范围声明 | 本报告为**本机全栈形态**的 ACP-09 阶段 B 完整回归；`.59 Runner 真进程 + .10 共享栈部署` 的 cpk 真机链（Runner 二进制实机 claim 循环）仍为独立联调批——本报告已覆盖协议面全部十项场景语义 |
| 结果 | **20/20 PASS**（V1.1 修订：DEF-E2E-1 已修复并复跑收口；V1.0 首跑 17/18，唯一 FAIL=S9，见 §3） |

---

## 1. 十项必测清单对照（V1.2 §8 / 联合基线 §8）

| # | 必测场景 | E2E 断言 | 结果 |
|---|---|---|---|
| 1 | cpk 凭据全链：claim→稳定身份 | S1/S1b/S1c：lease_owner=client:{device}、lease_client_id=设备 id、execution_mode=spawn | ✅ |
| 2 | 介入/回复全链 | S2（介入四写）+ S3（REST 回复三写：response+continue+delivery） | ✅ |
| 3 | 离线超 TTL 恢复 | S6a/S6b/S6c/S6d：过期无授权拒绝 → REST/SQL 授予 → 续领放行（gen+1、deadline 不动=T3） | ✅ |
| 4 | DSH 无证据 | S5（消费回执通道可达性；N-6 稳态由单测层 acp 测试钉住） | ✅ |
| 5 | 迟到 ACK | S7：旧 token 心跳拒绝（T2 fencing；lease expired/credential rejected 双形态） | ✅ |
| 6 | 多次介入 | S6-pre（新链 gen 侧证）+ S8a-c（续领后 continue 改路由可达、回复不丢、R-4 恢复） | ✅ |
| 7 | 06 场景（Checkpoint） | S10：登记幂等 + 无正文列 | ✅ |
| 8 | 07 场景（Deadline 授权） | S6 系 + ACP-482 撤销面（单测层） | ✅ |
| 9 | 终态钩子 | S9：PASS（V1.1：DEF-E2E-1 已修，§3 修复证据） | ✅ |
| 10 | payload 0054 扩展 | S4b：checkpoint/execution_authorization_id 字段在位 | ✅ |

附加覆盖：S4a（情形 6 过滤）、S4c（R-4 恢复原子性）、S6d（T3 断言）、ACP-466（复合外键拒跨租户，单测层）、S1 前 ACP-460 等价（cpk 签发哈希一致性，单测层）。

## 2. 执行记录

- 驱动全程 18 项 check（S1~S10 含子断言），汇总 `scripts/e2e/acp09_results.json`；原始输出 `/tmp/e2e_acp09/e2e_run.log`。
- 十项中 9 项 PASS；唯一 FAIL 见 §3。
- 过程缺陷（驱动侧，均已修复）：SSE 多事件解析（取含 result 的 data 行）；payload 为 dict 非字符串；acp09_reset.sql 占位符与 psql 变量语法冲突（改 @@标记）；迁移后租户重绑定（task↔project↔repo 循环 FK 以依赖序重建 + 新 customer 中转）。
- **V1.1 复跑（2026-11-08，DEF-E2E-1 修复后）**：驱动扩至 20 项 check（S9 拆 S9-http / S9-c2 / S9 三断言），**20/20 PASS**；原始输出 `/tmp/e2e_acp09/e2e_run.log`（复跑覆盖）。S9 环节新增驱动侧过程缺陷并已修复：裸体 PATCH 被 update_task 全字段校验 400 拒绝（见 §3 根因①）。

## 3. DEF-E2E-1（V1.0 真实发现 → V1.1 已修复关闭）

**REST Task 取消路径未联动介入/continuation 终态钩子。**

- 现象：`PATCH /v1/pm/tasks/{id}` status=cancelled 后，介入链取消 + continuation close 钩子未触发（continuation 停留 started）。
- 预期：§14/§5.1 CANCELLED 触发行——Task 取消联动取消未决 Interaction（cancel_open_interaction_tx，其内已接 continuation close 钩子，单测 acp477/chain 用例覆盖服务层）。
- 根因（初判）：REST 任务更新路径（update_task）未路由至带钩子的 update_task_status/cancel_open_interaction_tx 联动——**接线缺失，非状态机缺陷**。
- 影响面：REST 取消的任务其 continuation 停留 started（无自动关闭）；不影响 R-4 恢复/安全（closed 仅为记账终态）。
- 处置建议：将 REST 取消路径接线至 update_task_status 联动（或复用既有 S01-I4-F 钩子），修复后 S9 复跑；修前以「已持久化回复/继续语义不受影响」缓解（ACP-C2）。

### 3.1 V1.1 根因复核（实测两段，初判只对一半）

1. **驱动侧（V1.0 未检出）**：S9 旧驱动发裸体 `{"status":"cancelled"}`，而 `update_task` 是全字段编辑契约（Slice A mock 契约）——`priority` 缺省 `""` 先炸 `400 invalid priority`（`title` 同理），**请求根本没到状态写入**；旧驱动吞掉非 2xx 响应，于是 FAIL 只表现在 continuation 停留 started。
2. **服务侧（初判成立，且更深一层）**：`update_task` 确无 Cancelled 联动；但即使补上 `cancel_open_interaction_tx` 接线，**E2E S9 形态仍关不掉**——S8a 已 ack continue，`resume_execution_after_continue_tx` 已把链置 `chain_open=FALSE`（responded），铩链首句 UPDATE `WHERE chain_open` 零命中即短路返回，`close_open_by_run_tx` 不触。命令通道 `update_task_status` 的既有钩子同此短路（acp477 单测只覆盖链 open 形态，故未暴露）——按 acp04-05-joint §状态机触发行（Task 取消 → open/started continuation → closed），Task 终态的 continuation 关闭不应依赖链是否仍开。

### 3.2 修复（commit 6bdb8ba）

- `interaction::task_cancelled_linkage_tx`：铩未决介入链（既有事务核，含 guard 归 none / 等待 resolve / awaiting_decision 回转）+ 任务级 continuation 终态关闭，**命令通道（update_task_status）与 REST 直写（pm_write::update_task）共用同一内核**（V-6 入口仅适配）。
- `continuation::close_open_by_task_tx`：Task 全部 Run 的 `open/started/frozen` continuation → `closed`（ACP-477 闭合集）；`consumed` 为持久化事实（ACP-C2）不回写。
- `pm_write::update_task` 在状态机合法边 Cancelled 落地后同事务接线。零迁移、零状态机分支；复用既有审计动作（interaction.cancelled / continuation.closed），四身份列随 AuditActor 传递，无新增审计写入点（C174/C180 守卫不触发）。
- 驱动侧 S9 重放为契约体（title/priority/revision 取库内现值），断言拆三项：S9-http（200 + Task 终态 cancelled）/ S9-c2（responded 不被取消回写 = ACP-C2 回复语义保持）/ S9（continuation → closed）。

### 3.3 复跑对照（本机全栈同配方）

| | V1.0 首跑 | V1.1 复跑 |
|---|---|---|
| check 总数 | 18 | 20（S9 拆三） |
| S9 环节 | FAIL（continuation=started；实际 400 未达状态写入） | S9-http / S9-c2 / S9 全 PASS |
| 汇总 | 17/18 PASS | **20/20 PASS**（`scripts/e2e/acp09_results.json`） |
| 单测 | — | continuation 4/4 绿（acp477 存量 + `def_e2e1_rest_update_task_cancel_closes_continuation` 新增：started 链 REST 取消 → closed、responded 保持、终态零出边拒绝） |

### 3.4 顺带复核：checkpoint-1 存量红 devenv 两例

`devenv revoked_device…` + 同文件 `visibility…` 两红（checkpoint-1 记录判 baseline、归 client-registry 批）：**已被 0055（0c05946，`uq_device_tenant_active_name` 活跃名部分唯一索引）修复**，本批实测 `revoked_device_is_not_auto_restored_on_reregister` / `visibility_member_sees_own_admin_sees_tenant` 双绿，无需随本批再动。

## 4. 与 .10/.59 联调批的边界

本报告未覆盖（留待真实联调批）：
1. Runner **真二进制进程**的 claim 循环与 .59 部署形态（本报告以协议驱动替代——同一 MCP 面）；
2. .10 共享栈升级至 0049~0054 后的回归（共享栈当前为 23 工具旧镜像，升级需停机窗口）；
3. cpk 凭据经 `.59` Runner 配置注入的真机启动链。

## 5. 结论

**DEF-E2E-1 已闭，20/20（V1.1 修订）；ACP-09 关闭仍待 §4 真机联调批（边界不变）。**

V1.0 首跑 17/18、唯一 FAIL（DEF-E2E-1，接线级）经 V1.1 修复（commit 6bdb8ba）后同配方复跑收口：十项必测在本机全栈形态下全部 PASS，协议面/状态机/安全 fencing 语义全部按 V1.2 + 联合裁决兑现。`.10/.59` 真机部署联调（§4）完成后，ACP-09 方可关闭。

---

**文档结束**
