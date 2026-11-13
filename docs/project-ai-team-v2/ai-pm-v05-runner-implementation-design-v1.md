# 检查点 2 实施设计 V1.0 —— Runner 真机执行链

| 项目 | 内容 |
|---|---|
| 文档性质 | 实施设计稿（检查点 2 立项交付物；**落盘后等用户裁决，裁决前不进代码**） |
| 检查点目标 | 在 `.59` 上以 **Runner 真二进制进程**完成 claim→执行→人工介入→回报→平台只读验证全链，随后 **ACP-09 正式关闭**（当前仅剩 ACP-09 报告 §4 三条真机边界） |
| 上位裁决 | V-05-1（device 升级 Client Registry）/ V-05-2（Client 自管 push）/ V-05-5（Adapter 优先级 ZCode→Codex→DSH + Runner 走 MCP 自领）/ ACP V1.3.1 冻结 / 联合基线三约束 A/B/C / 身份三件冻结 / R-ACP-16 方案 a（已实施）/ checkpoint-1 全链 PASS（`docs/acceptance/checkpoint1-verification-record-v1.md`） |
| 设计基线 | ai-pm master @ **6efbef7**（2026-11-08，fmt 清偿后）；迁移 **0049~0055 全部已应用**（台账 §一） |
| 迁移纪律 | **本检查点零迁移取号**——0049~0055 已覆盖全部数据面（§11.1 逐项核对）；实施中若发现确需迁移，按台账纪律 max+1 取号并回填，不得复用/重解释既有号 |
| 覆盖 | ① Runner 形态与配置面 · ② claim 循环客户端化定稿 · ③ Adapter 层 · ④ Git 域严格分域 · ⑤ 离线恢复真进程落位 · ⑥ `.10` 共享栈升级窗口 · ⑦ `.59` 部署形态与验收清单 |
| 不覆盖（另批） | KB 同步实施（S01-08/WP-7）· AI Team UI（Roles/Client 管理页）· 调度②能力过滤生产化 · Model A 退役（保留）· Runner 自动升级/多实例高可用（V0.5 基线 §八非目标） |

---

## 0. 定位与边界

### 0.1 检查点 2 在 V0.5 关键路径上的位置

检查点 1 已收官（注册→建团→建项目→关联仓库→KB/PC 生成全链 PASS，迁移至 0053）。检查点 2 = **V0.5 目标①（三方通信）的最后缺口**：ACP-09 本机全栈回归 18/20（协议面十项场景语义全按 V1.2 兑现），仅剩 ACP-09 报告 §4 三条真机边界：

| # | 边界（报告 §4 原文要旨） | 本稿闭合节 |
|---|---|---|
| 1 | Runner **真二进制进程**的 claim 循环与 `.59` 部署形态 | §2 §3 §4 §6 §8 |
| 2 | `.10` 共享栈升级至 0049~0054 后的回归（现共享栈为 23 工具旧镜像，升级需停机窗口） | §7（实况更正：迁移上限现为 **0055**） |
| 3 | cpk 凭据经 `.59` Runner 配置注入的真机启动链 | §2.3 §8.3 |

另有前置债一项随本检查点一并闭合：**DEF-E2E-1**（REST Task 取消路径未联动介入/continuation 终态钩子，ACP-09 十项之 9 唯一 FAIL，报告 §3——接线级，非状态机缺陷）。

### 0.2 真机链全景（检查点 2 验收的一口鏡）

```text
.59（Runner 真二进制，cpk 凭据）                .10（升级后平台，26 工具）
  │ pm_list_tasks / pm_claim_task ───────────▶ 领取：lease_owner=client:{device}
  │ pm_get_run_context（Agent 自行）            契约：仓库/基线/验收/知识引用
  │ Adapter spawn ZCode/Codex（用户 PC 本地）    —
  │ git 本地 clone + commit + push（用户凭据）──▶ .16 GitLab（代码仓库域）
  │ pm_report_progress（心跳=续租）──────────▶ 租约管理
  │ marker 文件 → pm_report_interaction ──────▶ Interaction + 待办 + terminate 命令
  │ pm_poll_commands → terminate ack ─────────▶ 旧 Session 终止确认
  │ （人类经 REST/UI 回复，fail-closed）        continue 命令投递
  │ pm_poll_commands → continue → 新 Session   acked 事务：Task→running + 链关闭
  │ pm_report_consumption ────────────────────▶ 消费回执（双证据）
  │ （离线场景：kill .59 进程超 TTL → 重启）
  │ pm_discover_runs → pm_reacquire_lease ────▶ 同 Run 续领 + 命令改路由（generation+1）
  │ Agent: pm_submit_result（申报 SHA）────────▶ 平台只读验证（远端，Q-RC-1）→ verdict
```

服务端协议面（26 工具 + 0049~0055 状态机 + fencing）**已全部落地并有本机 E2E 18/20 背书**；检查点 2 的工程主体在 **Runner 客户端补齐 + 平台部署形态**，服务端只有两笔小项（DEF-E2E-1 接线修复 + Q-RC-1 只读验证档位，见 §9）。

---

## 1. 代码事实基线（设计钉在事实上，@ 6efbef7）

| # | 事实 | 证据 |
|---|---|---|
| F1 | Runner 骨架已落地：`apps/ai-pm-runner`（main/runner_loop/adapter/mcp 四模块，1295 行），真实栈 E2E 两阶段 PASS（假 Agent + opencode 真 CLI，INDEX 8a） | `apps/ai-pm-runner/` |
| F2 | cpk 凭据通道**服务端已全落地**：`cpk_` 前缀分支（每请求查库、fail-closed 不回退 JWT）+ claim 写 `lease_owner=client:{device_id}`、`lease_client_id=device.id`、`execution_mode='spawn'` | `apps/pm-api/src/auth.rs:223`、`crates/pm-application/src/run_dispatch/claim.rs:177,830,863` |
| F3 | 凭据管理 REST 已落地：签发（明文仅一次、重复签发 409）/撤销（幂等）/granted_permissions 裁剪 | `apps/pm-api/src/pm_devices.rs:240,286,319`、`device_credential.rs` |
| F4 | 恢复基础设施**已落地**（Recovery Foundation 批，0052）：`pm_discover_runs`（`lease_client_id` 严格相等匹配 + legacy NULL 拒绝门）+ `pm_reacquire_lease`（start_wait 前置 + generation+1 + 同事务命令改路由） | `crates/pm-application/src/recovery.rs:72,160,170,199,209` |
| F5 | Runner 启动恢复已接：`recover_inflight_run`（discover→reacquire→`awaiting_continue` 态，进程句柄 None，continue 到达按既有路径 spawn）——**仅恢复第一个发现项**（单 Run V1 语义） | `apps/ai-pm-runner/src/runner_loop.rs:95-125` |
| F6 | Runner 全部 MCP 调用面齐备：list/claim/context 由 Agent 面 + progress/interaction/poll/ack/consumption/discover/reacquire 十工具薄客户端 | `apps/ai-pm-runner/src/mcp.rs` |
| F7 | 进程硬化部分继承：Windows Job Object（KILL_ON_JOB_CLOSE，assign 失败降级告警）+ Unix `process_group(0)` + pid 文件（`process.json`，非原子写）+ ExitOutcome **字符串四值**（Success/Killed/Lost/NonZeroExit） | `apps/ai-pm-runner/src/adapter.rs:73-101,269` |
| F8 | codex_executor 服务端先例（仅参考不照搬）：镜像+worktree checkout、原子 pid 文件+identity_hint 锚点、日志脱敏管道（token→[REDACTED]）+大小上限、孤儿 reconcile 按 pid 强杀、LaunchProfile 审计摘要（environment 不落盘） | `crates/pm-infrastructure/src/codex_executor.rs:180-509` |
| F9 | ExitOutcome 权威五值：`Success / NonZeroExit / Signaled / Killed / Lost` | `crates/pm-application/src/run_dispatch/port.rs:173-184` |
| F10 | 交付验证现状 = **Model B 工作区域**：`verify_repository_head` 校验 `{AI_PM_WORKSPACE_ROOT}/{run_id}/{workspace_path}` 内 git 事实（对象存在/基线后代/==工作区 HEAD/变更数现算），错误码 `readonly_repo_commit`/`baseline_mismatch`/`commit_not_descendant` 现成 | `crates/pm-application/src/delivery/git_verify.rs:74-140`、`delivery/mod.rs:495-560` |
| F11 | 平台只读 git 面 = 命令白名单（ls-remote/clone/log/show/ls-tree），显式无 push、临时密钥用完即删；checkpoint-1 已实证同仓只读 ls-remote（项目 SSH key 密文落库） | `crates/pm-application/src/git_access.rs:1-4`、checkpoint-1 记录 §二.9 |
| F12 | Run 契约 `RepositoryView` **不含 clone URL**（repository_id/name/base_ref/base_commit/write_access/workspace_path/path_scope） | `crates/pm-application/src/run_context.rs:272-280` |
| F13 | 共享栈现状：`.10` pm-api 镜像为 ACP-03 时代（23 工具）；master 已 26 工具（Δ = `pm_report_checkpoint`/`pm_discover_runs`/`pm_reacquire_lease`），迁移上限 0055（54 文件） | ACP-09 报告 §4、`crates/pm-mcp/src/server.rs`（grep 26 命中） |
| F14 | pm-api 启动即跑 `sqlx::migrate!`——升级 = 换镜像重启，无独立迁移步骤 | `apps/pm-api/src/main.rs:28` |
| F15 | Runner 的 `AI_PM_RUNNER_TOKEN` 现为通用 Bearer 槽——`.59` 真机注入 cpk 即落 V-05-1 通道，**客户端代码零改动**（`bearer_auth` 透传）；cpk 通道下 claim 仍要求 `client_name` 参数（落 run 行 client_name 列，非身份用途） | `apps/ai-pm-runner/src/main.rs:19`、`mcp.rs:130-143`、`tools.rs:664-705` |
| F16 | Runner Cargo.toml 的 `pm-application` 依赖为**残留**（src 零引用 `pm_application::`）——客户端不应链接服务端应用栈 | `apps/ai-pm-runner/Cargo.toml:7` |
| F17 | cross-machine MCP 已实证：`192.168.2.4 → 192.168.2.10:8090`，C166 七项门禁跨机全过（rmcp Host 白名单配置化）；`.env.prod` 须含 `AI_PM_MCP_ALLOWED_HOSTS`（runbook 坑 5） | T-081 记录、runbook §八.5 |
| F18 | DEF-E2E-1 待修：REST `update_task` 取消路径未接 `cancel_open_interaction_tx` 联动（continuation 停留 started）——服务层钩子已在（单测覆盖），缺的是 REST 路径接线 | ACP-09 报告 §3 |
| F19 | checkpoint-1 尾账 4 红 baseline 中 `devenv revoked_device…` 一例与 0049 `uq_device_tenant_name` 相关，0055 `device_name_active_unique` 已应用——升级后须复跑验证该红是否闭合 | checkpoint-1 记录 §五 |

---

## 2. ①形态与配置面

### 2.1 crate 落位：维持 `apps/ai-pm-runner`（bin + lib），不迁 crates/

| 备选 | 判定 | 理由 |
|---|---|---|
| **a) 维持 `apps/ai-pm-runner`（推荐）** | ✅ | ① workspace 惯例 `apps/*` = 可部署成品二进制（pm-api/pm-worker/ai-pm-worker 同列），Runner 是下发用户 PC 的成品，不是被引用的库；② 骨架已在该位且 INDEX 8a E2E 以其为基，搬迁是纯摩擦；③ `apps/*` 已在 workspace members 通配内，零构建面改动；④ lib 面（`runner_loop`/`adapter`/`mcp` pub）已支撑集成测试驱动（F-12 惯例） |
| b) 迁 `crates/ai-pm-runner-core` + 薄 bin | ❌ | 仅当 Runner 内核被其他 crate 复用时才有价值——V0.5 无此消费方（平台不 import 客户端代码）；提前分层是抽象税 |

**随裁两笔清理（实施批内，零行为变更）**：
1. 摘除 Cargo.toml 残留依赖 `pm-application`（F16）——Runner 是客户端，不链接 sqlx 服务端应用栈，缩小二进制与供应链面；
2. `AI_PM_RUNNER_CLIENT_NAME` 语义对齐 0049：cpk 凭据通道下**不再拼 `runner:` 前缀**（前缀已降级为兼容识别，`claim.rs` 注释口径），值 = 设备登记名（默认 hostname，须与 `devenv.device.name` 一致——登记制）；真人 JWT 通道（Model A 兼容路径）保留前缀拼接。

### 2.2 配置面（单一 .env 文件 + 环境变量覆盖）

**装载规则**：`AI_PM_RUNNER_CONFIG` 显式路径 > exe 同目录 `.env` > 进程环境变量；同名键 env 覆盖文件。理由：ADR-0002 L2 凭据链路统一（凭据类配置一律经环境变量槽，不进配置结构体落盘/不进日志）；Windows 计划任务/服务形态下 env 文件是可审计的最小载体。

| 键 | 必填 | 语义 | 现状 |
|---|---|---|---|
| `AI_PM_PLATFORM_URL` | ✅ | Platform `/mcp` 端点（检查点 2 = `http://192.168.2.10:8090/mcp`） | 已有（main.rs:31） |
| `AI_PM_RUNNER_TOKEN` | ✅ | Bearer 槽 = **cpk_ 凭据**（V-05-1）；明文纪律见 §2.3 | 已有（main.rs:19） |
| `AI_PM_RUNNER_PROJECT_ID` | ✅ | 单项目 V1 | 已有 |
| `AI_PM_RUNNER_CLIENT_NAME` | — | 设备登记名（默认 hostname；§2.1.2 对齐） | 已有（语义修订） |
| `AI_PM_RUNNER_WORKSPACE_ROOT` | ✅ | `{root}/{run_id}/` 工作区根（F11 布局不变） | 已有 |
| `AI_PM_RUNNER_AGENT_CMD` | ✅* | Adapter 启动模板（\*三家模板库落地后可为 `AI_PM_RUNNER_ADAPTER=zcode` 选择式，§4.2） | 已有 |
| `AI_PM_RUNNER_CLAIM_INTERVAL_SECS` | — | 自领间隔（默认 10） | 已有 |
| `AI_PM_RUNNER_HEARTBEAT_SECS` | — | 心跳续租间隔（默认 60；**不变式：< 平台 `AI_PM_RUN_LEASE_SECS`=300**，§3.3） | 已有 |
| `AI_PM_INTERACTION_DETECT_INTERVAL_SECS` | — | 标记侦测间隔（默认 5） | 已有 |
| `AI_PM_COMMAND_POLL_INTERVAL_SECS` | — | 命令轮询间隔（默认 3；平台命令 claim 租约 300） | 已有 |
| `AI_PM_RUNNER_TERMINATE_ATTEMPTS` / `AI_PM_PROCESS_TERMINATION_TIMEOUT_SECS` | — | 终止预算（3 × 10s，§3.5 升级规则对齐） | 已有 |
| `AI_PM_RUNNER_GIT_MAP` | — | 仓库映射文件路径（代码仓库域，§5.2） | **新增** |
| `AI_PM_RUNNER_LOG_DIR` | — | 日志滚动目录（§8.2 日志面） | **新增** |
| `AI_PM_RUNNER_BACKOFF_*` | — | 续领/重连退避参数（基数/上限/jitter，§6.3） | **新增** |

**device 绑定语义（零第二套 Client Identity 的操作化）**：
- Runner 配置**不填 device_id**——身份由 cpk 在平台侧解析（`client:{device_id}`），Runner 本地仅持凭据与登记名。防「配置身份 ≠ 凭据身份」的混淆面（约束 A 的客户端延伸）。
- 稳定 Client 身份的"本地持久化"（专家复审 2.2 WP-3 项）= **同一 cpk 重用**：凭据 ↔ device 唯一绑定（0049 单列 hash），重启后原身份自动恢复；Runner 不本地缓存任何平台下发身份。lease_token 维持**不本地持久化**（R-16 方案 a 裁决不变）。
- 代理面：**不设**。reqwest 已 `.no_proxy()`（环境代理残留不得劫持内网 Platform 流量，mcp.rs:35 先例）；V0.5 目标环境为内网直连，代理是既有黑洞坑源（本机 ::1 SYN_SENT 先例），不新增代理配置键。

### 2.3 凭据注入与轮换的用户操作路径

```text
签发：.10 平台 REST  POST /v1/pm/devenv/devices/{id}/credential
      → cpk 明文【仅本次响应出现一次】（ACP-460；UI 凭据区块随收敛批，V1 用 API/文档配方）
注入：用户在 .59 将明文写入 .env 的 AI_PM_RUNNER_TOKEN（文件 ACL 限当前用户读）
      → 启动自检：首个 tools/call 失败 401 时 fail-fast 并打印「凭据无效/已撤销」分类，
        不进入无限 401 循环（实现批补 §3.4-I1）
轮换（V1 两步，0049 §4 单列两步制）：
      ① 签发新 cpk（同 device）→ ② .env 换新值 → ③ 重启 Runner →
      ④ 观察 claim 正常 → ⑤ DELETE credential 撤旧（即时生效：旧进程/旧凭据下一请求 401，约束 C）
撤销（应急）：DELETE /v1/pm/devenv/devices/{id}/credential → .59 下一请求 401
      → Runner 停止自动恢复并告警（不重试、不降级、不回退——约束 A 客户端面，§3.4-I2）
      【V1.0a 实施勘误】运行中 401 语义从「有界重探」收敛为 **fail-fast 退出**：
      同一凭据下重探结果不变（撤销不可自愈），supervisor（计划任务）重启后保持
      响亮失败（退出码 2）；有界重探原表述作废。
```

**落盘纪律**：cpk 明文与 lease_token 明文不进 Runner 日志、不进工作区文件、不进 spawn 参数（Adapter 日志脱敏管道随 §4.3-I4 补齐后由管道兜底 `[REDACTED]`）。

---

## 3. ②claim 循环：客户端化抽取定稿

### 3.1 工具面清单（零新协议声明）

Runner 只消费既有工具，检查点 2 **不新增任何 MCP 工具、不改四工具契约**（工程纪律红线维持）：

| 工具 | 调用方 | 用途 | 权限码（cpk 通道数据源 = `granted_permissions` 列） |
|---|---|---|---|
| `pm_list_tasks` | Runner | 发现 ready 且有 queued Run 的任务 | `PM_MCP_READ` |
| `pm_claim_task` | Runner | 领取（cpk 通道 → `lease_owner=client:{id}` + `lease_client_id` + spawn） | `PM_MCP_READ` |
| `pm_get_run_context` | **Agent**（Runner 不代拉） | 读执行契约 | `PM_MCP_READ` |
| `pm_report_progress` | Runner | 心跳=续租 + stage 推进 | `PM_MCP_READ` |
| `pm_report_interaction` | Runner | 介入上报（spawn 准入，ACP-448） | `PM_MCP_AGENT_REPORT` |
| `pm_report_consumption` | Runner | 消费回执（双证据，不伪造） | `PM_MCP_AGENT_REPORT` |
| `pm_poll_commands` / `pm_ack_command` | Runner | 命令轮询/回执（terminate/continue） | `PM_MCP_COMMAND` |
| `pm_discover_runs` / `pm_reacquire_lease` | Runner | 离线恢复发现/续领（§6） | `PM_MCP_COMMAND` |
| `pm_submit_result`（+ `pm_submit_evidence`） | **Agent** | 交付申报（含每仓 commit_sha，§5.3） | `PM_MCP_READ` |

签发凭据时的 `granted_permissions` 最小集 = 上表实际用到的码（默认三码全给亦可；**ACP-464 裁剪语义平台侧已实现**，检查点 2 真机抽测覆盖）。

### 3.2 双持身份（不变式，沿用 V1.2 §3.3）

Runner 同时持两种身份、缺一不可，且**不得互相替代**：

| 通道 | 身份 | 校验要素 |
|---|---|---|
| `/mcp` 鉴权（全部调用） | Client 凭据（cpk） | 每请求查库：hash 命中 ∧ device active ∧ 同租户 owner active ∧ 未撤销（约束 A 四项链） |
| 执行面工具（context/progress/interaction/consumption） | Run Lease 三要素 | 一次性 lease_token + generation + run scope |
| 命令面工具（poll/ack/discover/reacquire） | Command Delivery 身份 | worker_id 绑定（cpk 通道 = `client:{device_id}` 快照，claim 响应下发**原样回带**）+ generation 集合比对 |

### 3.3 循环结构（现骨架定稿 + 运行时不变式）

```text
启动：load config → cpk fail-fast 自检 → discover（恢复在途，§6）→ 主循环
主循环（500ms tick）：
  ① 收割：ExitOutcome 判定（awaiting_continue 窗口内退出=预期，不丢 Run）
  ② 自领：无活跃 Run 且到 claim 间隔 → list → claim → Adapter spawn
  ③ 通信拍：心跳续租 / 标记侦测→上报 / 命令轮询→terminate·continue→ack→消费回执
交付：Agent 自行 pm_submit_result（Model A 既有模式，Runner 不代交）
```

**运行时不变式（实施批以此为准入检查）**：

| # | 不变式 | 依据 |
|---|---|---|
| INV-1 | `heartbeat_secs < lease TTL(300)` 且等待窗口内**无条件续心跳**（存活信号非 Agent 心跳，R-9 语义） | V1.2 §4.3 |
| INV-2 | claim/poll/ack 的 `worker_id` 一律用 claim 响应 `lease_owner` 快照原样回带（cpk = `client:{id}`；无该字段才回退本地重建） | runner_loop.rs:216-229 既有 + ACP-462 |
| INV-3 | 单 Runner 同时至多 1 个活跃 Run（V1 串行）；discover 命中多个时逐个处理并在日志告警余量 | F5 单 Run 语义显式化 |
| INV-4 | continue 前置 guard 校验（frozen / deadline 过期→拒绝 failed，不 spawn）；terminate ACKED 是新 Session 启动前置（ACP-C4） | `preflight_continue` 既有（runner_loop.rs:371-391） |
| INV-5 | 回执失败不丢弃命令：下轮 poll 重领（claim 过期或 claimed_by=自方）后幂等重 ack | §3.4 重投既有语义 |
| INV-6 | 收割竞态兜底：`process=None` 且非 awaiting_continue → 释放 Run 槽（awaiting_continue 期间 None 是 terminate 预期态，**不得丢 Run**） | runner_loop.rs:178-185（E2E 实测坑，钉住） |

### 3.4 客户端化抽取的缺口清单（实现批 R1 主体）

| # | 项 | 说明 | 对应验收 |
|---|---|---|---|
| I1 | 启动 cpk 自检 + 401 分类 | 启动后先做一次轻量调用（`pm_list_tasks`）；401 → fail-fast 打印凭据分类（无效/已撤销/设备停用），退出非零；运行中 401 → **停止自动恢复 + 告警后退出（退出码 2）**【V1.0a 勘误：原「有界重探」表述作废，同 §2.3】 | 约束 C 客户端面；ACP-436/465 真机 |
| I2 | 错误分类扩展 | `RunnerError::Tool` 细分：凭据类（401/凭据字样）→ 停止自动恢复；租约类（expired/generation）→ 触发 §6 续领路径；业务类 → 按既有 F-9 语义 | ACP-447 |
| I3 | 心跳节拍守卫 | 启动时校验 `heartbeat_secs × 2 < lease TTL`，不满足拒绝启动（防配置踩 INV-1） | INV-1 |
| I4 | 续领退避（§6.3 参数化） | 指数退避 + jitter | 专家 2.2 |
| I5 | 多发现项处理 | discover 结果 >1 时逐 Run 记录 + 只领第一个 + 告警（INV-3 显式化） | INV-3 |
| I6 | 标记文件名含 event_id（P2-1 收敛项） | Runner 侦测 glob `interaction.{event_id}.json`（现固定名 `interaction.json` + 归档 event_id 键控）；并发出事件并存不丢 | V1.2 §1.2；层2 残余 ⑤ |
| I7 | receipt Schema 版本化 | ack receipt 统一 `{schema_version:"aipm.receipt.v1", run_generation, terminate_requested_at?, terminate_confirmed_at?, ...原有字段}`；`session_id` 字段名不变（身份三件冻结之二 = `receipt.session_id`，值语义维持 Adapter 本地新进程标识 `runner:{pid}`） | 专家 2.1；层2 残余 ⑦ |
| I8 | instruction.md 正式扩展 | `.aipm/interaction.{event_id}.json` Schema + 只创建新事件 + 不得自称获得人工批准（A*） | §7.2 既有 |

> 明确**不做**：Runner 本地持久化 lease_token（方案 B 不采用，R-16 已裁）；多 Run 并发；Runner 代 Agent 提交交付（F1 循环注释既有裁决）。

---

## 4. ③Adapter 层：ZCode → Codex → DSH

### 4.1 优先级链（V-05-5 冻结，不重开）

```text
声明顺序：ZCode（首选）→ Codex → DeepSeek Harness（DSH）
承接规则（检查点 2 档位）：
  AI_PM_RUNNER_ADAPTER=zcode|codex|dsh   显式选择（推荐——检查点确定性优先）
  或 AI_PM_RUNNER_ADAPTER=auto           按上序探测可用性（executable 可执行 + 登录态自检），第一个可用者承接
```

能力对齐 ACP-08 §7.1 静态声明（不建动态 Discovery）：三家 `interaction_detection = marker_file`；注入方式与消费证据按 R-6 表：

| Adapter | 初始输入注入方式 | INPUT_INJECTED 证据（§1.4 R-6） | 消费回执 |
|---|---|---|---|
| ZCode | 启动参数/提示文件（启动即读 ⇒ 必然接收） | spawn 参数摘要 + 进程存活 | 双证据可提交 |
| Codex | 同上（`exec "<prompt>"` 形态） | 同上 | 双证据可提交 |
| DSH | stdin 流式（观测**首个输出轮次**）或无法满足 | 不伪造：无法满足 → **N-6 合法稳态**（RESPONDED 未消费 + `interaction_unconsumed_total`，不影响执行——R-4 后 Task 已在 acked 事务回 running） | ACP-450 语义 |

### 4.2 落地形态：模板库起步，trait 边界就位（裁决点 Q-RC-6）

- **推荐：模板库起步**。`adapter/` 下新增 `profiles.rs`：三家各一套 `{program, args 模板, env 约定, 注入通道, marker 引导文案}` 常量（ZCode/Codex 的 prompt 文件注入 + DSH 的 stdin 注入分支）；`AI_PM_RUNNER_AGENT_CMD` 保留为**自定义兜底**（现 E2E 已证模板链可行）。零抽象税、每家行为显式可审计。
- `trait AgentAdapter`（`spawn / inject / detect / terminate / evidence` 五方法）**只立接口位**不强制三家立即多态化——V2 出现进程内混跑/能力协商需求时再收敛（非目标纪律：OpenCode/Trae/Claude Code 仅接口位）。
- DSH 检查点 2 档位：**stdin 注入分支实现 + N-6 稳态路径可用**即可（真机十项之 4 允许以协议层断言 + Adapter 声明档关闭，见 §8.2-A4 说明）。

### 4.3 进程生命周期：codex_executor 先例继承对照（不全盘照搬——服务端 vs 客户端差异显式化）

| 能力点 | codex_executor 先例 | Runner 现状 | 检查点 2 处置 |
|---|---|---|---|
| Windows Job Object（KILL_ON_JOB_CLOSE 整树终止） | spawn 后立即 assign（:496-502） | ✅ 已移植（assign 失败降级直杀+告警） | 维持 |
| Unix 进程组 `process_group(0)` | :484 | ✅ 已设 | 维持 |
| 原子 pid 文件 + identity_hint 锚点 | tmp 写 + identity（/proc starttime / Win StartTicks），PID 重用防误杀（:504-509） | ⚠️ `process.json` 非原子、无 identity_hint | **I9 补**：tmp+rename 原子写 + identity_hint 落盘 |
| ExitOutcome 五值 | port.rs:173 权威枚举 | ⚠️ 字符串四值（缺 `Signaled` 区分） | **I10 补**：对齐五值（Windows `Signaled`=不适用显式注记） |
| 日志脱敏管道 + 大小上限 | stdout/stderr → [REDACTED] → 落盘，超限丢弃防磁盘耗尽（:459-470） | ❌ 无（子进程输出未捕获） | **I4'/补**：Runner 侧捕获管道（同款脱敏 + 每 Run 上限），落 `{run_dir}/logs/`——既是审计面也是 §8.2 日志面的 Run 级证据 |
| 孤儿 reconcile（按 pid 文件核对后强杀） | identity_hint 一致才杀 | ❌ 无 | **I11 补**：启动扫描 `{workspace_root}/*/.aipm/process.json`，处置档位见 Q-RC-5 |
| LaunchProfile 审计摘要（脱敏、env 不落盘） | :393-405 | ❌ 无 | **I12 补**：spawn 参数摘要落 `{run_dir}/.aipm/launch.json`（R-6 第一类注入证据的落盘形态） |
| 临时密钥用完即删 / 禁交互 / 显式超时 | git 域先例 | —（Runner git 域见 §5，用户凭据不经 Runner 进程） | §5.2 |
| 有界重试终止 | —（终止预算在 Runner 侧先落地） | ✅ `terminate_with_retry` | 维持（ACP-444/§3.5：耗尽 → failed → 平台冻结启动） |

**不照搬项声明**：镜像仓 + worktree add（服务端共享镜像优化）不适用用户 PC——Runner 代码仓库域走 §5 的用户本地 clone；`AI_PM_CODEX_ARGS` 降级覆盖等容器 env 假设不适用。

---

## 5. ④Git 域严格分域（V-05-2 ↔ S01-08 互不污染）

### 5.1 分域总表

| 域 | 凭据持有方 | 推送方向 | 检查点 2 落位 |
|---|---|---|---|
| **代码仓库域** | **用户**（.59 自备凭据：自己的 GitLab 账号 SSH key 或 write deploy key） | 用户 PC → .16 远端（Agent 正常 commit/push） | §5.2/§5.3 |
| 代码仓库验证面 | **平台**（项目只读 deploy key，checkpoint-1 已建那把） | 无 push（只读白名单） | §5.3（Q-RC-1） |
| **知识库域** | **平台**（S01-08 R-01/R-02 本义：平台持知识库仓库凭据、自动同步） | 平台 → 用户 Git 服务器 | **检查点 2 不实施**——S01-08/WP-7 另批；Runner 对知识库 Git **零接触** |
| KB/PC 正文权威 | —（KBGIT/PCGIT 后置，checkpoint-1 §四：正文暂存 PG） | — | 维持后置，不因检查点 2 提前 |

**互不混用守则**：①两把钥匙不共享（用户 push 凭据永不进平台库；平台只读 key 永不下发执行端——ADR-0002 口径「执行端不接触平台持有的凭据」）；②方向单向（Runner→代码远端；平台→知识库远端；平台对代码远端只读）；③Runner 代码域 clone/push 全部在用户会话下进行，凭据不进 Runner 进程（经 git 自身 credential 体系：ssh-agent / credential helper——Runner 只准备目录，不碰凭据）。

### 5.2 Runner 侧：本地 clone 与用户凭据（用户操作路径）

- Run 契约只有 `repository_id/name/workspace_path/base_commit`（F12，无 URL）——**不改契约**（四工具冻结）。Runner 侧以映射文件补足：`AI_PM_RUNNER_GIT_MAP` 指向用户维护的 JSON/INI：`{ "<repository_id 或 name>": { "local_path": "D:\\work\\ai-pm", "url": "git@192.168.2.16:tooling/ai-pm.git"（可选，local_path 缺失时 clone 用） } }`。
- Run 启动序列（Adapter spawn 前）：映射命中 `local_path` → 校验 `.git` 存在 + `git fetch` + 按 `base_commit` 建立执行起点（detach checkout 或工作分支，Q-RC-2 细化）；未命中 → 按 `url` clone 到 `local_path`（clone 失败=Run 失败，fail-fast 提示用户配置凭据）。
- **凭据全程用户侧**：clone/fetch/push 由 git 进程在用户会话凭据体系下完成；Runner 不读取、不转存、不注入任何 git 凭据（`GIT_SSH_COMMAND`/密钥文件不经 Runner 之手——与 codex_executor 的平台密钥机制**刻意不同**，该先例属平台持有凭据域）。
- Agent push：Agent 在工作区内正常 commit/push（其自身继承用户会话凭据）；push 失败（无写权限/远端冲突）由 Agent 决定重试或上报 blocked——平台不管代码远端的写路径。

### 5.3 平台侧：申报 SHA 的只读验证（本稿唯一服务端设计扩展点，Q-RC-1）

**问题闭合**：F10 现状 `verify_repository_head` 是 **Model B 工作区域**（校验服务器本地工作区 HEAD）——Runner 形态工作区在用户 PC，平台无该工作区，现有校验对真机链必然 `workspace_unavailable`。V-05-2 已裁「Runner 申报 SHA + 平台只读验证（git_access.rs 现有只读面，错误码现成）」——即**远端验证路径**。

**设计（零迁移、零 MCP 契约变更、错误码语义不变）**：

```text
触发：pm_submit_result 每仓申报 {repository_id, base_commit, commit_sha, changed_files, tests}
模式判定（以 Run 载体事实为准，读 task_run 既有列，零新列）
  【V1.0a 实施勘误：远端模式收窄为仅 `client:` 前缀（cpk 通道）——`mcp:` 形态
  （Model A 真人 / runner: 过渡）的存量交付链走工作区模式不动，避免存量语义漂移；
  `mcp:` 远端化属后续独立裁决项】：
  lease_owner 为 `client:{id}`（cpk 通道）→ 平台侧无该工作区 → 远端模式
  `mcp:{name}:{token8}`（含 runner: 过渡 / human）→ 工作区模式维持现状不动
  服务器 Worker 载体（workspace 由服务端管理）→ 工作区模式维持现状不动
  ① 可达性：git_access 只读面 ls-remote——声明 commit_sha 存在于远端 refs（分支 head / 任意 ref 可达）
  ② 后代性：clone/fetch 至临时镜像（复用 repo verify 的临时目录 + 临时密钥用完即删惯例）
     → 白名单内命令组合判 base_commit → commit_sha 可达性
     （如 git log/--not 组合；白名单语义不扩、不引入 merge-base 等新命令面）
  ③ changed_files：远端模式无工作区可 diff → 以申报值登记 + verdict 注记「客户端申报，平台未现算」
     （不改 submit 驳回语义；ACP 验收以 ①② 为准，③ 为诚实注记——不伪造现算）
  失败映射：沿用 commit_not_descendant / baseline_mismatch / repo_not_in_contract 错误码与
  「业务驳回=SubmitOutcome::Rejected 非 5xx」既有语义（tools.rs:769-778）
```

**为什么不裁掉 Q-RC-1 也能过检查点**：不允许。§8 验收清单之「平台只读验证」是检查点目标链的最后一段——无远端验证则「申报 SHA」退化为无证据申报，ACP-09 真机十项之 1/7 无法诚实关闭。Q-RC-1 问的不是做不做，而是**本检查点做**（推荐）还是降级为「平台挂账 + 检查点以远端可达性 ① 手工验证替代」。

---

## 6. ⑤离线恢复在真进程的落位

### 6.1 重启恢复时序（真进程版，F4/F5 骨架已通，本节钉验收语义）

```text
.59 Runner 重启（崩溃/休眠恢复/手动）
 1. load config + cpk 自检（§3.4-I1；401 → 停止自动恢复，不进入恢复链）
 2. pm_discover_runs（只读、无副作用；按 lease_client_id 严格相等——legacy NULL 拒绝门在服务端）
 3. 命中 → pm_reacquire_lease（前置全在服务端单事务：start_wait ∧ 同 Client ∧ deadline 未过
    ∧ 存在 responded Interaction）
    效果：generation+1 + 新 token + 命令同事务改路由（reacquired_generation 写回）
 4. 本地无旧进程句柄 → ActiveRun{process:None, awaiting_continue:true}
    terminate 重路由命令到达 → 进程已不存在 = 幂等终止确认（ExitOutcome::Lost，ACP-C4 时序 4）
 5. continue 到达 → guard 校验（新 generation 通过）→ spawn 新 Session（携带回复）
    → acked（INPUT_INJECTED）→ Task 回 running + 链关闭 + guard→none（平台侧同事务）
    → 消费回执（双证据：reply 文件 + 进程存活）
 6. 同一 Run 全程：run_id / attempt_no / deadline_at 不变（ACP-452）；回复不丢（ACP-453）
```

**多 Run 发现**：discover 命中多个 → 按 INV-3 逐个告警、只领第一个；余下待当前 Run 终局后下轮 discover 再领（幂等、无副作用）。

### 6.2 失败纪律（专家 2.2 收编，实现批 I4）

| 规则 | 落位 |
|---|---|
| 续领失败 → 指数退避 + jitter（基数 2s、上限 60s、jitter ±20%，`AI_PM_RUNNER_BACKOFF_*` 可调） | runner 侧 |
| 持续失败（如 10 次连续）→ 告警日志升级 + 平台可见（`interaction_shielded_runs` 指标已有平台侧承载）+ **人工处置入口 = 用户重启 Runner / 平台 REST 查询面**——禁止 panic 丢恢复上下文 | 两侧 |
| **停止自动恢复条件**（不重试、转人工/ACP-07）：身份失效（401）· deadline 过期（续领前置拒绝且 guard 死锁语义）· device revoked | I1/I2 |
| Model A 适用：真人 MCP 客户端重连同走发现+续领（协议面已通用）；`human` Run 不可造新介入（ACP-448 不变） | 平台侧既有 |

### 6.3 本地进程账本（ACP-02 §6.9 指针的检查点 2 落位）

- 权威分工不变：Process 事实权威 = `worker_command.receipt` + 审计 + `executor-events.jsonl`（R-7 选 b）；Runner 本地账本只服务**孤儿发现**与诊断，不是平台权威（`executor-events.jsonl` 定位=诊断证据，§6.6-④）。
- 启动扫描：遍历 `{workspace_root}/*/.aipm/process.json` → 核对 pid 存活 + identity_hint → 命中孤儿按 Q-RC-5 档位处置（推荐：**只报告不强杀**——用户 PC 上强杀无 Runner 上下文的进程越权风险大于收益；codex_executor 的强杀先例属服务器容器独占域）。
- 标记文件重启扫描：未归档 `interaction.{event_id}.json` 幂等重报（event_id 幂等键，平台 UNIQUE 已有）；历史 `.reported/.rejected/.invalid` 只读不重报（I6 收敛后天然成立）。

---

## 7. ⑥`.10` 共享栈升级窗口方案

### 7.1 前置条件（全部满足才开窗）

| # | 项 | 判据 |
|---|---|---|
| P1 | master pin SHA 含：0049~0055 全部迁移 + **DEF-E2E-1 修复**（F18，S9 依赖）+ Q-RC-1 若裁实施 + 层2 残余服务端小项（⑤⑦） | git log + 台账核对 |
| P2 | 三镜像于**干净 worktree** 构建并经本机 E2E 复跑（acp09 20 项 ≥19 绿含 S9） | 构建配方 checkpoint-1 §一（rsproxy `--add-host` 钉 IP；runtime 镜像补 `openssh-client`；desktop-linux builder） |
| P3 | `.10` 实况核实：`tools/list` 当前工具数（预期 23）+ `SELECT max(version) FROM _sqlx_migrations`（预期 0047 时代）——**以实测为准**，本表 Δ = 0048~0055 八项 + 3 工具 | 窗口前 10 分钟 |
| P4 | `.env.prod` 核实：`AI_PM_MCP_ALLOWED_HOSTS` 含 .10 IP（F17）；`AI_PM_SECRET_KEY` 在位（项目 SSH key 密文解密依赖，§5.3 前置） | runbook §八.5 |
| P5 | 用户知会停机窗口（共享栈，预计 ≤20 分钟；ai-frontend 会话将短暂中断） | 用户裁决窗口时刻 |

### 7.2 窗口步骤（推荐最小重建集，裁决点 Q-RC-3）

| 步 | 操作 | 说明 |
|---|---|---|
| 1 | 开发机：`docker save` 三镜像（pm-api / pm-worker / ai-pm-worker）→ gzip → scp → `.10` `docker load` | 停机**前**完成（`.10` 不通 Docker Hub，runbook §一） |
| 2 | `.10` 备份：`pg_dump` aipm 库 → `backup_pre_upgrade.sql` | 窗口内第一步；回滚保证 |
| 3 | 最小重建：`docker compose ... up -d --force-recreate pm-api pm-worker ai-pm-worker` | **不重建 token-service**（JWT 密钥存活 → 用户登录态不掉线）；不触 lmms 八服务；卷不动 |
| 4 | pm-api 启动自动迁移 0048~0055（F14：`sqlx::migrate!`） | 观察 `docker logs pm-api` 迁移段；失败 → 停在第 N 项，`pg_restore` 回滚后处置（诚实失败不硬闯） |
| 5 | 健康验证：`/health` 200 + `tools/list`=26 + 迁移版本=0055 | §7.3 回归清单通过即收窗 |

**备选（整栈重建，runbook 标准路径）**：仅当最小集 recreate 出现 compose 依赖失败时降级使用——代价 = token-service 密钥重置（全员重登）+ lmms 服务重启窗口拉长；不作为首选。

### 7.3 升级后回归清单（全绿才收窗）

| # | 项 | 判据 |
|---|---|---|
| R1 | 基础链 | admin 登录 + `/v1/pm/me` + 菜单（最小集下旧 JWT 应存活） |
| R2 | 工具面 | `tools/list` = 26；新增三工具 description 在位 |
| R3 | **ACP-09 E2E 复跑**：`acp09_e2e.py` 指向 `.10` 全量 20 项 | **S1~S10 全绿含 S9**（DEF-E2E-1 修复验证）；18/20 不可接受（本次升级的验收核心） |
| R4 | cpk 全链冒烟（真栈） | 签发→cpk claim（`client:{id}` + spawn）→ 撤销→全工具 401（ACP-436/465 真栈抽测） |
| R5 | checkpoint-1 链路抽测 | repo verify 同 HEAD 幂等短路（不新建 intake） |
| R6 | 基线红复跑 | `devenv revoked_device…` 用例（0055 应闭合，F19）；未闭合 → 登记挂账不阻塞收窗 |
| R7 | lmms 零回归 | lmms 八服务 healthy + `:3000/v1/iam/auth/login` 可用（最小集未触碰，冒烟即可） |
| R8 | UI 冒烟 | ai-frontend 登录 + 执行环境设备列表可见（凭据区块 UI 属收敛批，API 面已验即可） |

---

## 8. ⑦`.59` 部署形态与验收清单

### 8.1 部署形态（V0.5 检查点档位，裁决点 Q-RC-4）

| 形态 | 判定 | 理由 |
|---|---|---|
| **a) 前台进程 + 计划任务自启（推荐）** | ✅ | ① 调试优先：检查点阶段问题定位需要直读 stdout（tracing 面板）；② **用户会话约束**：Agent CLI 登录态（ZCode）与 git 用户凭据（ssh-agent/credential helper）都活在交互用户会话——Windows 服务 Session 0 隔离下这些凭据体系不可用，是硬伤而非偏好；③ 计划任务「用户登录时启动 + 失败重启（30s）」即覆盖开机自启与崩溃拉起 |
| b) Windows Service（NSSM/sc） | ⚠️ 备选 | 仅当后续 Runner 改造为「凭据全部自带（不含用户会话依赖）」后再升格；列为 V2 候选，检查点 2 不采用 |
| c) 纯手动前台 | — | 允许用于联调初期（§8.4 步骤 1~3），验收链须以 a) 形态复跑一轮 |

**形态定稿配置**：计划任务（最高权限=否、仅登录时运行、失败重启）；`.env` 与 exe 同目录；`AI_PM_RUNNER_LOG_DIR` 指向 `%LOCALAPPDATA%\aipm-runner\logs`。

### 8.2 日志面（三层，与权威分工一致）

| 层 | 载体 | 内容 | 权威性 |
|---|---|---|---|
| Runner 运行日志 | tracing stdout → `tracing-appender` 日滚动（保留 14 天）+ `logs/runner-YYYYMMDD.log` | 生命周期事件（claim/spawn/terminate/ack/续领/退避） | 诊断 |
| Run 级证据 | `{run_dir}/logs/{stdout,stderr}.log`（I4' 脱敏管道）+ `{run_dir}/.aipm/{process.json, launch.json, executor-events.jsonl}` | 进程输出（token→[REDACTED]）+ spawn 摘要 + 事件流 | 诊断证据（**非平台权威**，§6.6-④） |
| 平台侧可见性 | `.10` REST/审计/指标 | Run 状态、Interaction、命令 receipt、`interaction_shielded_runs` | **权威** |

脱敏纪律全局生效：cpk / lease_token 明文在任何层不得出现（I4' 管道兜底 + `[REDACTED]`）。

### 8.3 验收清单 A：ACP-09 真机版十项对照

以 ACP-09 报告 §1 十项为基，全部换真机断言（`.59` 真二进制 + `.10` 升级后真栈）：

| # | 必测场景 | 真机断言（差异点加粗） | 证据 |
|---|---|---|---|
| A1 | cpk 全链 claim→稳定身份 | .59 进程以 cpk claim → `lease_owner=client:{device}`、`lease_client_id`、`execution_mode=spawn`；**Runner 二进制经 env 注入 cpk 启动（边界 3）** | 审计行 + claim 响应 |
| A2 | 介入/回复全链 | **真实 Agent CLI 写 marker**（ZCode/Codex）→ 待办 → 人类 REST 回复 → continue → **.59 真进程 ack + 新 Session spawn** | 命令 receipt + 链关闭 |
| A3 | 离线超 TTL 恢复 | **`.59` 进程真实 kill（或主机休眠）超 `AI_PM_RUN_LEASE_SECS`** → 重启 → discover→续领→continue **同 Run**；deadline/attempt_no/Run 行不变（ACP-452/453） | §6.1 时序日志 + 平台审计 |
| A4 | DSH 无证据 | 真机档：DSH Adapter 装配后**不伪造回执**（RESPONDED 稳态 + 指标）；若检查点不装配 DSH 真机，以协议层 S5 复跑 + Adapter 声明档关闭并显式登记 | 指标 + 声明档（Q-RC-6 附带确认） |
| A5 | 迟到 ACK | 续领后旧 generation/旧 worker_id 的 ACK 被拒（ACP-454/C3 fencing）；真机模拟 = 重启前旧实例最后一次 ack 或协议层回放 | 拒绝审计 |
| A6 | 多次介入 | 同 Task 两次真实介入 = 两链独立 + `previous_chain_id` 串联（0052 列真机验证） | Interaction 行 |
| A7 | 06 场景（Checkpoint） | marker checkpoint 字段 → `pm_report_checkpoint` 登记（0054 元数据）；幂等 + 无正文列 | 登记行 |
| A8 | 07 场景（Deadline 授权） | deadline 过期：续领/continue 均拒（T3 不顺延）→ **ACP-07 REST 重授权** → 新有效期独立审计 → continue 放行（0054 语义真机走通） | 授权审计 + guard |
| A9 | 终态钩子 | REST 取消 Task → Interaction cancelled + continuation closed（**DEF-E2E-1 修复后**；升级回归 R3 同源） | S9 绿 |
| A10 | payload 0054 扩展 | checkpoint / execution_authorization_id 字段在投递与回执链路可见 | payload 断言 |

附带：ACP-446（claim 过期重投）、ACP-449（非本 Run poll 空集）、ACP-450/451（新链）、**ACP-462（同凭据跨重启命令路由稳定——真机天然覆盖）**；专家 E2E 必测 ①~⑩ 中真机可测项（②⑤⑥⑧⑨）并入 A2/A3/A5/A8/A6。

### 8.4 验收清单 B：E2E §4 三边界逐条关闭标准

| 边界 | 关闭标准（全部满足才算关） |
|---|---|
| 1. Runner 真二进制 claim 循环 + `.59` 部署形态 | ① `.59` 以 §8.1-a 形态运行 release 构建 Runner；② 清单 A 的 A1/A2/A3 以**真进程**（非 acp09_e2e.py 协议驱动）跑通完整一轮链（claim→执行→介入→回复→续领→交付）；③ Runner 连续运行 ≥ 一个完整链周期无 panic/无人工干预恢复 |
| 2. `.10` 共享栈升级回归 | §7.2 步骤执行完成 + §7.3 回归 R1~R8 全绿（**R3 必须 20/20**）；升级窗口记录落档（含 pg_dump 备份路径与迁移日志段） |
| 3. cpk 注入启动链 | 明文仅签发响应一次 → `.59` `.env` 注入 → 真机启动 claim 成功（正向）；错 key / 撤销后 → 401 不回退、Runner 停止自动恢复（负向，ACP-465/467 真机）；`.env` 与日志无明文残留（审计抽测） |

### 8.5 ACP-09 正式关闭条件（汇总）

```text
DEF-E2E-1 修复 + 回归 R3（S9 绿）
+ 三边界 B1/B2/B3 逐条关闭
+ 清单 A 十项真机全绿（A4 允许声明档关闭但须显式登记）
+ 验收记录落档 docs/（检查点 2 验证记录，对齐 checkpoint-1 体例：环境/步骤/证据/幂等负向/门禁遗留）
```

---

## 9. 实施批次与证据门槛（架构通过 ≠ 代码交付完成）

> 联合基线 §7 纪律：每批以「迁移（本批零迁移）+ 事务/行为实现 + 自动化测试 + E2E 证据」四件齐为完成。**各批独立提交独立验收，裁决前不进代码。**

| 批次 | 内容 | 证据门槛 |
|---|---|---|
| **R1 Runner 客户端补齐** | §2.1 清理（摘 pm-application 依赖、client_name 语义）+ §3.4 I1~I8 + §4.3 I9~I12 + 模板库（ZCode/Codex 先行，DSH 注入分支）+ §6.2 退避 + §6.3 孤儿扫描 | Runner 集成测试（wiremock 面）覆盖 I1/I2/I4/I6/I7；本机栈假 Agent 回归绿；cargo fmt/clippy 干净 |
| **R2 服务端小项** | DEF-E2E-1 接线修复（REST update_task → cancel_open_interaction_tx 联动）+ 层2 残余服务端侧（⑤ marker 文件名服务端口径 / ⑦ receipt schema 服务端从严校验）+ Q-RC-1 若裁：git_access 白名单内远端验证路径（delivery 层接入点 + 单测） | acp09 E2E 本机复跑 **20/20**（S9 绿）；Q-RC-1 单测（后代性正/负例）；守卫 C174/C180 若新增审计点同步登记 |
| **R3 `.10` 升级窗口** | §7 全套 | §7.3 R1~R8 全绿；窗口记录落档 |
| **R4 `.59` 真机联调与验收** | §8 部署 + 清单 A/B 全项 + 检查点 2 验证记录落档 | §8.5 关闭条件全部满足；ACP-09 正式关闭 |

依赖序：R1∥R2（并行）→ R3（依赖 R1+R2 pin）→ R4。

---

## 10. 待用户裁决项（裁决前不进代码）

| # | 事项 | 推荐 | 备选 |
|---|---|---|---|
| **Q-RC-1** | 平台只读验证申报 SHA 的**远端验证路径**（§5.3）是否随检查点 2 实施 | ✅ 随本检查点实施（R2 批；唯一服务端功能项） | 降级：平台挂账，检查点以「远端可达性手工验证 + 申报注记」关闭（诚实降级，须在验证记录显式登记） |
| **Q-RC-2** | Runner 侧仓库映射与执行起点形态 | `local_path` 预 clone（用户常驻工作目录）+ fetch + **detach 于 base_commit 的临时工作分支**（Run 间互不踩；push 由 Agent 显式推分支） | 每_Run_独立 clone（磁盘开销大但隔离最强） |
| **Q-RC-3** | `.10` 升级窗口形态 | 最小重建集（三服务 recreate；token-service 不动，登录态存活） | 整栈重建（runbook 标准路径，全员重登） |
| **Q-RC-4** | `.59` 进程形态 | 前台 + 计划任务自启（用户会话凭据体系可用性优先） | Windows Service（NSSM；Session 0 凭据硬伤，留 V2） |
| **Q-RC-5** | 孤儿进程处置档位 | 启动扫描**只报告不强杀**（用户 PC 越权风险 > 收益） | 按 pid 文件+identity_hint 强杀（codex_executor 服务器先例，容器独占域才适用） |
| **Q-RC-6** | Adapter 落地形态 | 模板库起步（profiles 常量）+ trait 接口位不强制多态 | 立即 trait 多态化三家 |

---

## 11. 工程纪律（不变项汇总）

### 11.1 零迁移核对单（本稿逐项核过，检查点 2 全程零取号）

| 检查点 2 疑似需求 | 结论 |
|---|---|
| cpk 凭据/lease_client_id/granted_permissions | 0049 已覆盖 ✅ |
| discover/续领/命令改路由（reacquired_generation） | 0052 已覆盖 ✅ |
| previous_chain_id（多次介入弱关联） | 0052 已覆盖 ✅ |
| Delivery/Consumption/Continuation（receipt/消费回执链） | 0050/0051 已覆盖 ✅ |
| Checkpoint 登记 / Deadline 重授权 | 0054 已覆盖 ✅ |
| 设备名唯一性修订 | 0055 已覆盖 ✅ |
| §5.3 远端验证 | **无表结构变更**（delivery 既有列承载 verdict）✅ |

### 11.2 冻结项清单（本稿全程未触碰、实施亦不得触碰）

1. **V-05-1/2/5** 三裁决（含 ADR-0002 口径修订三件事）不重开；`runner:` 前缀维持降级兼容、不新签。
2. **ACP V1.3.1 冻结** + ACP-C1~C4 横向约束（C3 generation fencing / C4 续领与 Session 重启分离在本稿 §3.3-INV4、§6.1 直接消费）。
3. **约束 A/B/C**（凭据四项验证链不回退 / 复合 FK 租户一致 / 撤销覆盖完整命令生命周期含既有连接不缓存绕过）——检查点 2 真机验收即其最终实证。
4. **身份三件冻结**：`task_run.executor_session`（Platform Session ID）/ `worker_command.receipt.session_id`（新 Session 标识）/ `delivery.continuation.id`（Continuation 身份）——Runner 不引入第四种 Session 身份；Runner 形态 `executor_session` 允许 NULL（terminate 按 pid 文件定位，R-8 放宽已登记）。
5. **四工具契约不动**；discover/poll 双工具并存契约不动（R-ACP-16 §2.5）；`lease_token` 不本地持久化；T3 deadline 不顺延（过期走 ACP-07）。
6. 禁重建区（核验报告 §四）继续有效；全部写入经 `pm-application`（V-6），Runner 仅适配层。
7. 主 checkout 有并行会话在途物：**禁 sweep、禁 `git add -A`**，只提交本批文件；设计稿落盘 = 本批唯一文件，**裁决前不进任何代码**。
8. 落盘后等待用户裁决（§10 六项 + 总体裁决）；裁决通过后按 §9 批次实施，每批独立验收。

---

**文档结束**（设计稿 V1.0，2026-11-08；裁决入口 = §10）
