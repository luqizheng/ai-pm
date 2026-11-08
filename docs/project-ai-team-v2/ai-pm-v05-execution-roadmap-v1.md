# AI-PM V0.5 执行路线图 V1.0 —— 双主线 · 共享基础设施 · 关键路径

| 项目 | 内容 |
|---|---|
| 文档性质 | 实施排期与依赖裁决记录（专家复审第 3 块收编产物，待用户确认启动） |
| 来源 | 专家复审 `docs/ui/s03-acp/smark-pm-s03-acp-slice-design-acp01-02-08-v1-2-expert-review-3.md`（2026-11-07，第 3 块=排期依赖） |
| 关联 | V0.5 范围基线（`ai-pm-v05-mvp-scope-baseline-v1.md`）· V2 架构基线 + 代码核验（本目录）· ACP 通信面 V1.2（`docs/ui/s03-acp/`，三轮专家复审已收编） |
| 代码基线 | ai-pm master @ 220173f |

---

## 1. 项目划分（防止错误的整体命名）

**不采用**「ACP V1.2 收敛完成后，再启动 AI Team V2」的串行表述。正式划分为**四个独立交付物**，共享底层能力：

1. **ACP V1.2 收敛**：现存通信面正确性（Correctness Fix）与恢复可靠性（Recovery Foundation）。
2. **Client Identity Foundation（V-05-1）**：平台统一执行端身份（独立平台基础切片，非 ACP 子任务）。
3. **AI Team V2**：团队、成员、角色、模型与调度领域建设。
4. **ACP-04~09**：交付持久化、Continuation、Checkpoint、授权与回归。

## 2. 双主线 + 共享基础设施

```text
        共享基础设施
  V-05-1 Client Registry（含撤销）
  Team/Project 权限边界 · 凭据与授权机制
  执行事件基础（最小持久化）
        │              │
   ACP 主线        AI Team V2 主线
   通信面收敛        Team Domain / Membership
   R-16 离线恢复     Project Role / Team LLM
   ACP-04/05        Agent 调度
   ACP-06/07
   ACP-09
```

**解堵判断**：V2 的 Team 创建、成员管理、角色权限、LLM 配置等基础领域**不等** ACP-05/06/07；涉及真实 Agent Client 注册、执行调度、跨 Project 执行授权时**必须**用 V-05-1 正式身份与权限模型——**V2 可以提前开始，但不能提前创造第二套 Client Identity**。

## 3. 五阶段关键路径

| 阶段 | 内容 | 优先级 |
|---|---|---|
| 1. 活风险正确性修复 | ACP-448 + ACP-450/451 + **Model A 定向回归**（同步） | P0 最高 |
| 2. 身份基座与恢复链路 | V-05-1 最小安全子集 + ACP-452/453 + ACP-C1~C4 + `pm_discover_runs` + `previous_chain_id` | 生产恢复能力前置 |
| 3. ACP-04 / ACP-05 | 联合设计闭数据契约 → Delivery/Consumption 持久化 + Continuation 正式状态机 | P1 |
| 4. ACP-06 / ACP-07 | Checkpoint、Workspace Lock、Deadline 与人工授权 | P1 |
| 5. 完整 ACP-09 回归 | Model A、Runner 离线/重启/超时/迟到 ACK/多次介入真实栈验证 | 发布验收 |

## 4. 立即启动的四个工作包（WP-A~D）

| 工作包 | 内容 | 依赖 | 启动 |
|---|---|---|---|
| **WP-A · ACP Correctness** | ACP-448、ACP-450/451、Model A 定向回归 | 已有 ACP 通信面 | 立即 |
| **WP-B · Client Identity** | V-05-1 实施设计、0049 迁移设计、身份安全 | 身份架构裁决已完成（WP-0①） | 立即（**实施设计 V1.0 已出 2026-11-07**：`ai-pm-v05-client-registry-design-v1.md`——方案 B 自发 cpk_ 凭据 + 0049 列集 + 回填三分类 + 撤销矩阵 + Q-A~Q-D 待裁） |
| **WP-C · Repository Integrity** | task_repository 生产写入口 P0 | 既有 Project Repository + 授权机制 | 立即（独立并行） |
| **WP-D · ACP-04/05 Architecture** | Delivery 与 Continuation 正式数据契约联合设计 | — | 并行设计 |

## 5. 执行依赖表（专家原表）

| 工作包 | 优先级 | 依赖 | 可并行性 |
|---|---|---|---|
| ACP-448 | P0 | 已有 ACP 通信面 | 立即 |
| ACP-450/451 | P0 | 已有 Interaction/Command | 立即 |
| Model A 定向回归 | P0 | 上述修复 | 与修复同步 |
| Task Repository INSERT | P0 | 现有 Project Repository、授权机制 | 独立并行 |
| 0048 ACP 收敛迁移 | P1 | 收敛设计冻结 | 与 V-05-1 设计并行 |
| V-05-1 | P0 | 身份架构裁决已完成 | 立即启动 |
| 0049 Registry 迁移 | P1 | V-05-1 实施设计 | 紧随 V-05-1 |
| ACP-452/453 | P1 | V-05-1 身份安全 | 依赖 V-05-1 |
| ACP-05 | P1 | Continuation 数据契约 | 与 ACP-04 联合设计 |
| ACP-04 | P1 | Delivery 数据契约 | 可与 ACP-05 设计并行 |
| ACP-06 | P1 | ACP-05 | 串行 |
| ACP-07 | P1 | ACP-05、Deadline 契约 | 与 ACP-06 设计并行 |
| ACP-09 完整回归 | P1 | ACP-04~07 及恢复链路 | 最终验收 |
| AI Team V2 | 独立主线 | Client、Project Role、调度设计 | 基础工作可提前 |

## 6. V-05-1 最小生产可用集（含撤销，专家裁决）

**必须包含**：Client 登记与稳定 `client_id`；`client_name` 唯一性及归属约束；Client 与认证主体绑定；凭据签发、验证；**凭据撤销或 Client 禁用**（V1 只需简单 `revoked_at`/`disabled_at`——撤销能力是安全底线，复杂撤销管理不是）；`lease_client_id` 与历史租约绑定；旧身份兼容与回填策略；**正式权限码字典及校验**。

**可后续增强**：能力三分完整动态管理；多凭据轮换与并存；生命周期管理 UI；设备状态与健康管理。

**撤销生效面**：撤销立即影响 `pm_discover_runs` / `pm_reacquire_lease` / `pm_poll_commands` / `pm_report_interaction`（+ ack/report_consumption 执行面）——**不能只禁续领而放行轮询**。

**层级模型**：`Team → Project → Client{Identity, Credential, Capabilities, Revocation}`——Client 是可被多 Project 绑定的身份载体，授权范围由 Team/Project 关系控制；**不得将 Client 简化为某 Project 内的 Runner 名称**。

## 7. 迁移号规划（0048 / 0049）

| 迁移 | 内容 | 所属 |
|---|---|---|
| 0048 | `worker_command.reacquired_generation`（R-16）+ `task_interaction.previous_chain_id`（R-4 延伸） | ACP 收敛 |
| 0049 | Client Registry 登记表 + `task_run.lease_client_id` + 历史 Client 身份回填 + 权限码字典及关联约束 | V-05-1 |

- 两类内容**独立审查/测试/回滚/排期**；V-05-1 设计调整不影响 0048。
- **lease_client_id 回填三分类**：有可靠认证主体对应 → 自动回填；仅自报 client_name → 暂不认定真实身份；来源不明 → 标 `legacy/unverified`。历史记录保留原 `lease_owner` 但**不得据此获得新的续领权限**。**迁移原则：历史兼容 ≠ 历史身份可信**。
- 迁移号由**单一实施负责人统一分配**；进入共享分支后并行会话不得自建同号。

## 8. task_repository 生产写入口 P0（WP-C，独立切片）

链路：Task Repository Creation → Repository Authorization → Task Claim → Execution Allows → Repository Scope → Delivery/Commit。

六约束：①关联只来自当前 Project 的有效 Repository；②创建关联时校验既有授权；③禁跨 Project 写入；④重复写入幂等；⑤Claim 与 execution_allows 消费**同一关联事实**；⑥禁经 MCP 参数伪造 Repository 归属。**沿用既有 Project Repository 领域对象，不另建 Task Repository 权限体系**。

`task_waiting` 零写入方已由 ACP 切片补上 → **移出 P0 空洞清单**（保留真实栈回归证据）。

## 9. AI Team V2 七项待裁的分类（专家版）

| 待裁项 | V2 启动前必须？ | 判断 |
|---|---|---|
| Git 归属表述 | 否 | 文本修订，可并行 |
| agent_client 载体 | **是** | = V-05-1，身份基础 |
| 调度入口 | **是（设计闭合）** | 不必全部实施，但须确定：谁创建执行请求/选 Role/选模型/决定 Project/分发/失败重试终结——AI Team、ACP Runner、Project Agent 不得各自发展调度入口 |
| 模型策略 UNIQUE(tenant,provider) | **是（Schema 冻结前）** | 产品决策可并行；落库前按 `Provider Connection → Team Model Authorization → Project AI Role Model Selection` 关系核对，明确前不冻结唯一约束 |
| project_role vs member.role | **是** | 权限模型硬前置：Team Role（Team Membership）≠ Project Role（Project Membership/Grant）——Team Admin 管团队 LLM 不天然拥有全部 Project 业务管理权；Project Admin 管 Git/Runner/AI 配置但不绕过 Team 模型授权。与既定 Team LLM Provider 授权原则一致 |
| 执行事件持久化 | V2 **正式运行前**必须 | 先做最小方案（event_id/team/project/task/run/client/event_type/generation/occurred_at/received_at，幂等写入+审计查询）——不做大型事件溯源 |
| Phase 5 双轨收敛 | 否 | 独立规划不阻塞启动；须定最终收敛方向，避免双轨长期并存成为事实架构 |

> **注记（2026-11-08）**：本表「调度入口」设计闭合的既定输入新增一条——派发器 ↔
> Runner 竞争缓解裁决 D-2=c（Runner 心跳注册为派发可用性来源，可用性真源统一）。
> 裁决见 `ai-pm-v05-dispatch-competition-adjudication-record-v1.md` §四；
> 设计包（代码事实 + 缺口 + 方案 + 待裁 Q-D2C-1~6 + 实施切片）见
> `ai-pm-v05-d2c-runner-availability-design-v1.md`。

## 10. ACP-04 / 05 联合设计检查单（WP-D 入题）

① Delivery 引用 Run 还是 Session？② Consumption 是否需要 continuation_generation？③ Delivery 状态是否参与 Task 完成判定？④ ACP-05 是否消费 ACP-04 持久化结果？⑤ ACP-04 是否依赖 05 未建的正式 Session 身份？（若依赖 → 先定字段契约再分阶段落地，**不得临时另建 Session 身份**。）

实施顺序默认：联合设计 → ACP-04 核心持久化 → ACP-05 Continuation 状态机 → ACP-06 → ACP-07。ACP-05 边界：先定义正式 Continuation 身份/状态/代次/生命周期，06/07 经明确接口接入，不顺带吸收其职责。

---

**文档结束**
