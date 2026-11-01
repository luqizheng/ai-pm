# T-032 设计稿过稿前 · 最终技术裁决提示词

> 提交对象：架构专家（第三轮复审）
> 背景：T-032 设计稿（`docs/ai-pm-slice-d3-executor-hardening-v1.md`）已吸收全部三轮评审意见与你此前的最终裁决（Q1/Q2/Q3 P0 不拆分、生命周期一致性总验收 §8.1）。实施方在过稿前做了代码事实验证，发现 **1 个设计稿事实性修正点 + 2 个落点选择**，需要你最终拍板。三项均不影响已裁决的架构原则（ADR-0002 七项 + 统一模型），只影响迁移形状与实现细节。
> 裁决格式：直接回复「T1: ii / T2: a / T3: i」（或你的替代方案）。

---

## T1 · starting 启动意图的载体（设计稿事实修正）

**验证出的事实**：设计稿 §2.2 写「`stage='starting'`（新 stage 值，不新增 Run 状态枚举）」——但 **`task_run` 表没有 `stage` 列**。`stage` 列在 `delivery.task` 上（`0010:23`，任务级「Q9.1 未裁阶段清单，先留空」）。设计稿字面隐含「给 task_run 新增 stage 列」的迁移，此前未明示。

**选项**：

| 选项 | 设计 | 代价 | 风险 |
|---|---|---|---|
| i | `task_run` 新增 `stage` 列，starting 落列 | 一列迁移 + reconcile 查询扩展为「stage='starting' OR executor_session IS NOT NULL」 | 为一个布尔语义加列 |
| **ii（实施方倾向）** | **`executor_session` 前置即启动意图**：spawn **之前**先生成 `{kind}:{uuid-v4}` 并条件 UPDATE 写入 Run 行（零迁移）；「claimed + executor_session 非空 + pid 文件未确认」即 starting 态；spawn 成功后回填 pid/创建时间确认 | **零迁移**；且现有 `reconcile_orphans` 查询条件就是 `executor_session IS NOT NULL`（`run_executor.rs:267-271`）——**查询一行都不用改**，孤儿可见性自动闭合 | spawn 失败时 executor_session 残留（见下） |

**需要你确认的子问题**：选项 ii 下 spawn 失败时 `executor_session` 残留在 claimed 态 Run 上（倾向：**保留不清理**，作为启动失败痕迹；下次重试 start 生成新 session 覆盖；reconcile 扫到它时 pid 文件不存在 → 判定进程不存在 → 正常回收，语义自洽）。是否接受？

---

## T2 · lease_generation：补列（方案 a）vs hash 等价替代（方案 b）

**已验证的事实**（上一轮 X3 场景走查，你已确认采纳为方案 a 论证）：

- 数据正确性上，既有机制已闭环：每次成功 claim 生成**新随机 token** 并覆盖 `lease_token_hash`（`claim.rs:552-576`，含 B3-1 重领同一 Run）；回收路径清空 hash + 置终态（`lease.rs:66-76`）→ 双执行端并发写入不可达。
- ADR-0002 D2-⑥ 要求「当前租约 generation 有效」，现有 schema 无此列。

| 方案 | 设计 | 利 | 弊 |
|---|---|---|---|
| **a（实施方倾向）** | 回改 `0015` 补 `lease_generation INT NOT NULL DEFAULT 1`；每次成功 claim（含重领）+1；MCP run 级校验带代次一致性；心跳续租**不**递增 | fencing 成为**数据约束**而非实现约定；审计可观测（代次 = Run 被重领次数）；字面满足 D2-⑥ | 一列迁移 |
| b | 确认 hash 覆盖等价，⑥ 以「② hash 一致 + ⑤ 租约未过期」替代 | 零列 | **安全性依赖实现纪律**：若未来有人改成「重领复用 token」或跳过覆盖，fencing 静默失效——无数据层防线 |

**实施方倾向 a 的关键理由**：ADR-0002 原则「Credential 负责授权」应当用数据说话——b 把正确性押在「永远没人改坏覆盖逻辑」上，与生命周期一致性总验收的「一致且可追溯」标准不匹配。

---

## T3 · Run 级绝对执行截止的落点（ADR-0002 D3「防止无限续租」）

**验证出的事实**：`timeout_seconds` 在 `delivery.task` 上（`0010:28`），`task_run` 无此列；现有 poll 判定在 `timeout_seconds = NULL` 时完全不判定（`run_executor.rs:308-317`）。Run 级截止必须新建落点。

| 选项 | 设计 | 利 | 弊 |
|---|---|---|---|
| **i（实施方倾向）** | `task_run` 回改 `0011` 补 `deadline_at TIMESTAMPTZ`；claim 时 `= now() + COALESCE(task.timeout_seconds, env 默认上限)`；poll 单列比较 | 截止值**随 Run 固化**——审计可回答「这个 Run 当时的截止是多少」；判定 O(1)；task 参数事后被改不影响在跑 Run | 一列迁移 |
| ii | poll 时从 `task.timeout_seconds` 现算 + env 默认兜底 NULL | 零列 | NULL 语义每处现算；**task 参数事后修改会漂移在跑 Run 的截止**——与「一致且可追溯」验收冲突 |

---

## T4 · 确认项（无争议，一并过目）

`last_agent_heartbeat_at` / `last_progress_at` 两列落 `task_run`（回改 `0011`）——心跳/进展分离（你已裁决的四信号分立）的载体，无替代方案争议。

---

## 实施方建议的裁决组合

**T1: ii（executor_session 前置，spawn 失败残留可接受）+ T2: a（补列）+ T3: i（deadline_at）+ T4: 确认**

裁决后实施方将：① 按 T1 修正设计稿 §2.2（零迁移方案）；② T2/T3/T4 补进设计稿 §3.3/§3.4 数据设计；③ 全部落入 §8.1 生命周期一致性验收；④ 设计稿即为过稿版进入实施。