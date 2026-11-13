# 检查点 2 · R4 `.59` Runner 真机联调验收记录 V1.0

> **R4 收官 + ACP-09 关闭判定**（2026-11-08）。
> 范围：`.59`（gitlab-ci-01，Debian 13）部署 ai-pm-runner 真二进制，以 cpk 凭据完成
> **claim→工作区准备→spawn→介入→REST 回复→Continuation→消费→交付申报→平台远端验证**
> 全链真进程实证 + **A3 离线超 TTL 恢复（ACP-452/453）真机闭环**。
> 结论：**ACP-09 §4 三条边界全部关闭，ACP-09 正式关闭**（一项声明档遗留见 §五）。

## 一、环境与部署

| 项 | 值 |
|---|---|
| Runner | `apps/ai-pm-runner` master@969f767 Linux release 构建（rust:1-slim 容器内，5.5MB）→ `/home/kudi/aipm-runner/ai-pm-runner` |
| 设备身份 | `e2e-runner-59`（`7f679292-…`，platform=linux）→ 绑定项目 e2e-acp09 → **cpk 签发**（`POST /v1/pm/devices/{id}/credential`，明文仅签发响应一次） |
| 配置面 | `.env`（PLATFORM_URL/`:8090/mcp`·cpk Token·PROJECT_ID·CLIENT_NAME=e2e-runner-59·WORKSPACE_ROOT·AGENT_CMD·GIT_MAP·LOG_DIR） |
| Agent 驱动 | `fake_agent.py`（确定性协议驱动器；event_id 跨轮唯一 + Session-2 交付分支 commit/push/submit） |
| Git 域 | `git-map.json`：ai-pm → `git@192.168.2.16:tooling/ai-pm.git`，用户自备 deploy key（**可写**，挂于 GitLab .16，平台不接触） |
| 演练靶 | task `74acb744` + run `6f4ec3d4`（ready/queued 播种→Runner 自领） |
| Worker 演练隔离 | pm-worker/ai-pm-worker 演练期 stop（派发竞争隔离，见 §四-1），验收后已恢复 |

## 二、边界 1：Runner 真进程全链（ACP-452 前置 + 交付段）

**真进程时序**（`.59` stdout.log + `.10` DB 双侧证据，完整链 <10s）：

```text
claim（cpk 通道）→ lease_owner=client:{device} + execution_mode=spawn    [A1 真机]
→ workspace prep：git clone via 用户 deploy key → detach 于 base 239b539
→ spawn Session-1 → marker interaction.{event_id}.json → pm_report_interaction
→ Platform：pending + awaiting_decision + terminate 命令同事务
→ Runner poll → terminate → Killed → awaiting_continue
→ 人类 REST 回复（A* fail-closed 入口）→ continue 下发
→ Runner 消费 → Session-2 携回复 spawn → done 标记 → exit Success
→ 消费回执：interaction=consumed + delivery=consumed + shield=none + task=running
→ 交付：git commit/push（每轮唯一分支）→ pm_submit_result 申报 SHA
→ 平台远端只读验证（Q-RC-1 首次真栈）→ **outcome=accepted**
→ 交付证据：task_repository_commit=d398b89c… + audit delivery.submitted
```

**发现并修复**（随批提交）：0049 签发面 `query_scalar` 元组误标注（真栈 500 → `52e28c5`，pm-api 0.2.1 上栈）。

## 三、边界 3：cpk 注入启动链 + 负向

| # | 断言 | 结果 |
|---|---|---|
| 1 | 签发明文仅响应一次 → `.59` `.env` 注入（不经日志/仓库） | ✓ |
| 2 | Runner 启动 cpk 自检（轻量调用）通过 → 进入 claim 循环 | ✓ |
| 3 | 伪造 cpk → **401**（fail-closed 不回退，ACP-465 方向） | ✓ |
| 4 | claim 写稳定身份 `client:{id}` + spawn + 命令路由原样回带 | ✓（A1） |

## 四、A3：离线超 TTL 恢复（ACP-452/453 真机，R-16 方案 a 真进程闭环）

时序：Runner 领取+报介入（pending/human_wait）→ **Runner kill（离线）** → 人类 REST 回复
（**responded 持久化** + continue 投递至已死路由）→ **TTL 340s**（>300s 租约）→ 重启 Runner →

```text
启动 → pm_discover_runs 命中 → pm_reacquire_lease：
  generation 1→2 + 命令同事务改路由（rerouted=1）
→ 消费重路由 continue → Session-2 spawn → 消费回执
→ 终态：interaction=consumed | shield=none | task=running | generation=2
```

`recovered in-flight run after offline window (same-owner re-lease) generation=2 rerouted=1` —— ACP-452/453 真机关闭；deadline/attempt_no/Run 行不变（T3）核验同批。

## 五、十项对照与三边界总账

| 十项（ACP-09 报告 §1） | 真机覆盖 | 状态 |
|---|---|---|
| 1 cpk 全链 claim→稳定身份 | A1 真机 | ✓ |
| 2 介入/回复全链 | 真机全链（Agent=协议驱动器） | ✓（CLI 声明档，§五-1） |
| 3 离线超 TTL 恢复 | A3 真机（ACP-452/453） | ✓ |
| 4 DSH 无证据 | 协议层（S5 + 单测）+ N-6 稳态声明 | ✓（声明档） |
| 5 迟到 ACK | 协议层（S7 真栈 E2E + 单测 T2 fencing） | ✓ |
| 6 多次介入 | 协议层（S6-pre 双链 + 0052 previous_chain_id） | ✓ |
| 7 06 Checkpoint | 真机 E2E S10（登记幂等/无正文列） | ✓ |
| 8 07 Deadline 授权 | 真栈 E2E S6a~S6d + REST 授权 | ✓ |
| 9 终态钩子 | 真栈 S9（DEF-E2E-1 修复后）+ R3 回归 | ✓ |
| 10 payload 0054 扩展 | 真机 E2E S4b | ✓ |

**三边界**：① Runner 真二进制 claim 循环 + `.59` 部署形态=✓（本记录 §二）；② `.10` 升级回归=✓（R3 记录 20/20）；③ cpk 注入启动链=✓（§三）。**→ ACP-09 正式关闭。**

## 六、发现与遗留

1. **派发器 ↔ 领取型 Runner 竞争**（架构登记，待独立裁决）：简化派发器（pm-worker，
   `AI_PM_RUN_DISPATCH` 默认 On 且 compose 未透传该键）对「cpk 领取型设备」的 ③ 可用性
   过涝始终拒绝（readiness 快照无 check-in 概念），重试后把任务打入
   `denied: no_client_available`，与 Runner 自领竞争同一 queued run。演练期以停 worker
   隔离；正式解法候选：(a) 派发器把 claim 型设备视为可用；(b) 接线 `AI_PM_RUN_DISPATCH`
   透传并按租户/项目关闭无人值守派发；(c) Runner 心跳注册为派发可用性来源。
2. **CRLF/LF 迁移指纹**（承 R3 记录）：Dockerfile 构建段 `migrations/` LF 规范化小批待做。
3. **CLI Adapter 真机档**（声明档遗留）：ZCode/Codex 在 `.59` 未安装——十项之 2 的
   「真实 LLM CLI 写 marker」由协议驱动器替代关闭，模板库（zcode/codex/dsh）已就位；
   CLI 真机适配随 CLI 可用环境另行验证。
4. `.59` 形态为前台 `start.sh`（setsid nohup）；Linux systemd 单元为 V0.6 候选（Q-RC-4 的
   Linux 对应档位）。
5. 迁移台账/部署 pin：`.10` = 0.2.1（pm-api，含 52e28c5）+ 0.2.0（workers@239b539）；
   master 未 push 池自 d2b60b0 起 8 commits。
