# S03-ACP · ACP-09 完整回归 E2E 报告 V1.0（本机全栈）

| 项目 | 内容 |
|---|---|
| 文档性质 | ACP-09 贯穿式验证 · 阶段 B 完整回归 E2E 报告（本机全栈形态） |
| 环境 | 本机真实进程：pm-api debug 二进制（jwks 模式，JWKS=192.168.2.10:3000 lmms 真签发）+ 真实 MCP HTTP（Streamable）+ 真实 PG（infrastructure-aipm-pg-1:16433 独立库 e2e_acp09，迁移 0001~0054 全应用）+ F-7 过渡开关开启（AI_PM_MCP_LEGACY_PERMISSION_GRANT=1，admin JWT 无 ACP 权限码） |
| 驱动 | scripts/e2e/acp09_e2e.py（REST + MCP JSON-RPC 混合驱动）+ scripts/e2e/acp09_reset.py（确定性重置：acp09_reset.sql @@标记哈希替换） |
| 范围声明 | 本报告为**本机全栈形态**的 ACP-09 阶段 B 完整回归；`.59 Runner 真进程 + .10 共享栈部署` 的 cpk 真机链（Runner 二进制实机 claim 循环）仍为独立联调批——本报告已覆盖协议面全部十项场景语义 |
| 结果 | **18/20 PASS**；2 FAIL = 同一发现（DEF-E2E-1，见 §3） |

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
| 9 | 终态钩子 | S9：**FAIL → DEF-E2E-1**（§3） | ❌ |
| 10 | payload 0054 扩展 | S4b：checkpoint/execution_authorization_id 字段在位 | ✅ |

附加覆盖：S4a（情形 6 过滤）、S4c（R-4 恢复原子性）、S6d（T3 断言）、ACP-466（复合外键拒跨租户，单测层）、S1 前 ACP-460 等价（cpk 签发哈希一致性，单测层）。

## 2. 执行记录

- 驱动全程 18 项 check（S1~S10 含子断言），汇总 `scripts/e2e/acp09_results.json`；原始输出 `/tmp/e2e_acp09/e2e_run.log`。
- 十项中 9 项 PASS；唯一 FAIL 见 §3。
- 过程缺陷（驱动侧，均已修复）：SSE 多事件解析（取含 result 的 data 行）；payload 为 dict 非字符串；acp09_reset.sql 占位符与 psql 变量语法冲突（改 @@标记）；迁移后租户重绑定（task↔project↔repo 循环 FK 以依赖序重建 + 新 customer 中转）。

## 3. DEF-E2E-1（真实发现，待修复）

**REST Task 取消路径未联动介入/continuation 终态钩子。**

- 现象：`PATCH /v1/pm/tasks/{id}` status=cancelled 后，介入链取消 + continuation close 钩子未触发（continuation 停留 started）。
- 预期：§14/§5.1 CANCELLED 触发行——Task 取消联动取消未决 Interaction（cancel_open_interaction_tx，其内已接 continuation close 钩子，单测 acp477/chain 用例覆盖服务层）。
- 根因（初判）：REST 任务更新路径（update_task）未路由至带钩子的 update_task_status/cancel_open_interaction_tx 联动——**接线缺失，非状态机缺陷**。
- 影响面：REST 取消的任务其 continuation 停留 started（无自动关闭）；不影响 R-4 恢复/安全（closed 仅为记账终态）。
- 处置建议：将 REST 取消路径接线至 update_task_status 联动（或复用既有 S01-I4-F 钩子），修复后 S9 复跑；修前以「已持久化回复/继续语义不受影响」缓解（ACP-C2）。

## 4. 与 .10/.59 联调批的边界

本报告未覆盖（留待真实联调批）：
1. Runner **真二进制进程**的 claim 循环与 .59 部署形态（本报告以协议驱动替代——同一 MCP 面）；
2. .10 共享栈升级至 0049~0054 后的回归（共享栈当前为 23 工具旧镜像，升级需停机窗口）；
3. cpk 凭据经 `.59` Runner 配置注入的真机启动链。

## 5. 结论

ACP-09 十项必测在本机全栈形态下 **9 项 PASS、1 项 FAIL（DEF-E2E-1，接线级）**；协议面/状态机/安全 fencing 语义全部按 V1.2 + 联合裁决兑现。DEF-E2E-1 修复 + `.10/.59` 真机部署联调完成后，ACP-09 方可关闭。

---

**文档结束**
