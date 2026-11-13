# Q2 执行端身份与凭据 · 专家裁决问题清单

> 用途：拿去找业务专家裁决。评审意见 Q2（执行端身份与 MCP 认证）的方案空间受**三条已裁决约束**限制，本清单列出全部交叉点，供专家逐项裁决。
> 关联：评审材料包 `docs/executor/ai-pm-t031-executor-expert-review-pack.md` §8；ADR-0001（Accepted）；D2 设计稿 v1.1；执行端接入 runbook v1（草稿）。
> 日期：2026-11-01

---

## 〇、背景：三条已裁决结论 + 评审新要求 = 冲突

**既有裁决（不可默认推翻，推翻需专家明示）：**

| # | 裁决 | 内容 | 出处 |
|---|---|---|---|
| ① | ADR-0001 D4 | Agent 是 attribution-only principal，**不发独立凭据**；真人是授权/认证主体；拒绝方案「Agent 领自己的 token」（§8 方案 D） | ADR-0001 §2/§8 |
| ② | ADR-0001 §7 | 与 D2 关系「**不改**」：MCP actor 仍是 `dev_agent`/`ai_pm`；**`lease_token` 仍是 run 级唯一凭据** | ADR-0001 §7 |
| ③ | D2 R-G4 | run 级 MCP 准入要求 `lease_token_hash IS NOT NULL`；**Model B（worker 领取）与存量 Run 一律拒绝**——Model B 下 worker 不持有 token 明文，hash 为 NULL | D2 §6.1（评审 R-G4） |

**评审 Q2 新要求（外部评审者，2026-11-01）：**

- Codex 经 MCP 调 D2 工具必须有执行身份：平台 spawn 的进程不能「因为是平台启动的就自动获得写入权限」。
- 建议「短期、限定 Run、可撤销的执行凭据」；凭据不一定非得是原始 lease_token。
- MCP Server 必须验证 session + run_id + 权限 + 有效期。

**冲突现状一句话：T-031 平台 spawn 的 Codex 走 Model B → 按裁决 ③ 调任何 run 级 MCP 工具（`pm_get_run_context` / `pm_report_progress` / `pm_submit_result`）都会被拒绝 → D3 的执行端心跳通道 V1 不可用（T-031 实施裁定 1 已自认），D4 的 `pm_submit_result` 落库后同样无法被 Model B 执行端调用。这不是优化项，是 Model B 闭环的**必经裁决点**。**

```
Model A（人工，当前可用）          Model B（平台 spawn，T-031 新增）
─────────────────────            ─────────────────────────────
真人持 API Key → Bearer           worker 领取（不签发 token 明文）
pm_claim_task → lease_token       Run: lease_token_hash = NULL
执行端持 token 调 MCP ✅          Codex 无任何凭据 → 调 MCP 被拒 ❌
                                  （AI_PM_LEASE_TOKEN 留空待 D4）
```

---

## 专家裁决问题（8 问）

### Q-A · 凭据主体：执行凭据发给谁？

**冲突点**：评审建议凭据发给「Executor Session（进程实例）」；但 ADR 裁决 ①② 锁定「凭据只有一种 = run 级 lease_token」，且 principal 体系内没有也不允许有「进程实例」这个认证主体。

**问题**：执行凭据的主体与形态，三选一（或给出新方案）：
- **A1**：复用 lease_token——worker claim 后把明文一次性下发执行端（改动最小：解除裁决 ③ 对 Model B 的排除即可，无新凭据体系）；
- **A2**：新增独立的 executor credential（session 绑定、独立 TTL、独立撤销），与 lease_token 并存——需新 ADR（触碰裁决 ②「lease_token 是唯一凭据」）；
- **A3**：凭据不进 MCP 认证层，维持 Model B 执行端不走 MCP（纯环境变量交互）——那 D4 交付上报与执行端心跳需要另行设计，评审 Q2 的心跳分离诉求落空。

### Q-B · D2 R-G4 的修订方式

**冲突点**：裁决 ③ 把 Model B 的 Run 排除在 MCP run 级准入之外，这条是 D2 评审（R-G4）明文钉死的，当初理由是「Model B 无凭据可校验」。T-031 之后 Model B 有平台托管进程，条件已变。

**问题**：R-G4 如何修订？
- **B1**：Model B claim 时也签发 token 明文（与 Model A 统一为同一条凭据链路：claim → token → 下发），`lease_token_hash` 非空，R-G4 改为「hash 为 NULL 一律拒绝」不再提 Model B；
- **B2**：Model B 引入新校验（凭 executor session），与 Model A 的 token 校验并存两套；
- **B3**：不改 R-G4，Model B 执行端继续不走 MCP（同 A3）。

### Q-C · 凭据安全属性是否达标

**冲突点**：评审要求「短期、可撤销、限定 Run」。lease_token 现状：32B 随机、sha256 入库、明文只返回一次（D2 裁决 D）、限定单 Run、回收路径同步 `SET lease_token_hash = NULL`（四处：requeue ×2 / cancel / isolate）——「限定 Run + 可撤销」已满足；「短期」= 跟随租约有效期（默认 300s，续租延长），**不是独立 TTL**。

**问题**：
- **C1**：lease_token 的「有效性 = 租约有效期」是否视为满足「短期」？（执行端心跳即续租，token 随租约一直活着）
- **C2**：若要求独立 TTL（如 15 分钟轮换），是在 token 体系内加，还是凭 A2 的新凭据承担？
- **C3**：「可撤销」除租约回收外，是否需要独立的管理端撤销入口（管理员杀某次执行）？落点是清 hash 还是杀进程 + 清 hash？

### Q-D · 无人值守执行的审计归因

**冲突点**：ADR §5 口径「工具代表代理，actor = `dev_agent` principal；真人信息进 `actor_identity_id`/`triggered_by_user_id`」。Model B 无人值守时**真人不在场**。

**问题**：Model B 执行端调 MCP 时，审计三列怎么填？
- **D1**：actor = `task_run.agent_id` 对应的 dev_agent principal（既有机制），真人字段 = `task_run.triggered_by_user_id`（派发任务的触发人）——无需新机制；
- **D2**：增加「无人值守」标记（actor_auth_mode 或新列），明确区分 Model A（真人在场监督）与 Model B（无人）——审计可解释性更好，动审计列（关联 T-059 待落地）。

### Q-E · 横向越权面是否提前收口

**冲突点**：D2 v1.1 威胁模型明示（R-F13/G-10）：V1 无 per-project 调用方绑定，**持有效 bearer 的任意调用方可领取任意项目任务**，已知且 V1 接受（待裁 #12 / T-051 挂起）。评审要求「禁止无身份认证的 MCP 请求修改 Run 状态」——run 级凭据解决的是「改错 Run」（纵向），不解决「任意 bearer 领任意任务」（横向）。

**问题**：执行端从「用户 PC 人工模式」扩展到「平台 spawn 无人值守 + 多 PC 多租户」后：
- **E1**：横向越权面维持「V1 接受」到何时？触发收口的条件是什么（第二个租户上线？跨机接入 T-081？）；
- **E2**：若提前收口，per-project 绑定落在 API Key 签发侧还是 run 级凭据侧？T-051 是否从「观察项」升级为 D4 前置项？

### Q-F · 凭据下发通道与泄露面

**冲突点**：评审另一项意见明确指出「CLI 环境变量、stderr 可能包含密钥」。而凭据下发候选通道全部经过执行端进程环境。

**问题**：
- **F1**：明文 token 下发通道选哪种：环境变量（`AI_PM_LEASE_TOKEN`，D3 已留位）/ instruction 文件（权限受限）/ MCP 客户端配置注入？
- **F2**：配套的泄露面约束是否随之立项：日志脱敏（token 不进 stdout/stderr/审计明文）、`/proc` 环境可见性（Linux 同机用户可读他进程 environ）在你们的威胁模型里是否接受？

### Q-G · 两套接入形态的凭据边界

**冲突点**：runbook（M8）规划的是「真人 API Key、Model A、用户 PC」；T-031/D3 是「平台 spawn、Model B、服务端 worker」。codex-bootstrap（未过稿，M10）的自动初始化同时面向两者。凭据体系若分叉（真人 key / executor credential），边界必须现在钉死。

**问题**：
- **G1**：确认两条红线——真人 API Key 永不进入平台 spawn 的进程环境；executor credential 永不发给用户 PC 上人工模式的执行端。是否同意成文？
- **G2**：codex-bootstrap 的「Worker 安装 + MCP 配置 + 身份认证」协议，是否需要按 Model A / Model B 分叉出两套安装产物？（评审 Q6 建议的环境初始化与 Launch Profile 分层也在此一并裁）

### Q-H · executor_id / Registry 的联动

**冲突点**：ADR D6 把 Executor Registry 推迟到「T-030/D3」，`executor_id` 是无 FK 的前向列（T-031 未填值，仍为 NULL）；T-031 只落了 `executor_kind`。评审的 Executor Session 凭据体系若建立，session 与 executor 实例的绑定关系需要 Registry 承载。

**问题**：
- **H1**：T-032 若引入 session 级凭据，是否要求同步落地最小 Registry（executor 实例行 + `executor_id` 补 FK），还是凭据只绑 Run/Session、Registry 继续推后？
- **H2**：`executor_id` 的填值时机——随 T-032 还是随 P11 规模化（容器/远程执行时才有真实多 executor）？

---

## 建议裁决顺序

1. **Q-A → Q-B**（凭据主体与 R-G4 修订）：决定 Model B 闭环走「统一 lease_token 链路」还是「新凭据体系」——这是 D4 `pm_submit_result` 能否被 Model B 执行端调用的前置。
2. **Q-C → Q-F**（安全属性与下发通道）：在 Q-A 定了形态后裁参数。
3. **Q-D**（审计归因）：随 T-059 一并落。
4. **Q-E → Q-G → Q-H**（越权收口时机、两套形态边界、Registry 联动）：可带出评审现场，登记后续任务。

**底线提醒**：若专家选择 A3/B3（Model B 不走 MCP），则评审 Q2 的「心跳分离」诉求在 Model B 上无法实现（执行端无通道上报心跳），T-032 的「Agent Heartbeat」项需相应改为「平台侧代理心跳」（以进程 CPU/子进程活动等旁证代替 MCP 心跳）或明确推迟到 Model A 场景。
---

## 七、专家复审勘误（2026-11-01，§六「对清单本身的评价」）

专家复审确认本清单「抓到了一个真正的架构冲突」，并给出三处修订，处置如下：

| # | 修订意见 | 处置 |
|---|---|---|
| 1 | **Q-A 不应把 Executor Session 当凭据主体必选方向**——Executor Session 是执行环境概念，不是认证主体；应首先讨论 Run 级授权凭据是否足够 | **部分成立，已修正**：本清单 Q-A 选项排序本以 A1（复用 lease_token）居首，但问题框架确实预设了「Session 凭据」方向。专家裁决实际采纳 A1，框架偏差未影响结论；此勘误留档防止后续清单重犯 |
| 2 | **Q-C「短期凭据」定义不严谨**——300s 租约 ≠ Token 最长存活 300s；只要允许续租 Token 就可能长期有效；应明确**最大执行期限**与**租约所有权变化时的撤销机制** | **完全正确，实质纠错**：原清单把「有效性=租约有效期」直接等同于「短期」是错的。正确定义已由 ADR-0002 D3 采纳（绝对执行截止时间强制化 + 取消/隔离/重领/回收即撤销） |
| 3 | **Q-E 应提升优先级**——横向越权面不宜放在后续讨论，多项目/多仓/多租户/多 PC 执行节点前必须收口 | **已被采纳**：ADR-0002 D5 将 T-051 升级为「多租户/跨机上线前置」（P0 安全前置），todo.md 已标注门控 |

**后续复审补充（同日）**：专家 §四给出最终统一架构（七项特点，已定稿进 ADR-0002 §6，原图转 mermaid）；§五给出任务分层拆分（T-032 收窄为认证闭环 P0 五项、execution_mode 移交 T-059、生命周期管理能力另立任务、Registry 维持 P11）；§六即本节勘误。
---

## 八、T1–T4 终裁（2026-11-01，设计稿过稿裁决）

针对 T-032 设计稿过稿前最后三个技术裁定点（`docs/executor/ai-pm-t032-finalization-expert-questions.md`），专家终裁：**T1: ii / T2: a / T3: i / T4: 确认**，并附加强化约束（PID 文件不存在 ≠ 子进程不存在，reconcile 三重证据；旧 session 覆盖前留痕；generation 首次领取=1、心跳/失败不递增；deadline 首次 claim 固化、重领/续租不得重置、绝对截止优先于续期；心跳 ≠ 进展）与迁移约束（0011/0015 已应用不得回改，forward-only `0023`）。全文已并入设计稿 §2.2/§3.3/§3.4/§4/§8.1——**设计稿已过稿，进入实施**。