# AI 研发管理平台：执行端接入 Runbook（V1 · 草稿）

> **标注图例**：〔已核实〕= 2026-11-01 对照 `crates/pm-mcp`（`server.rs` / `tools.rs`）与 `docs/runbook-login-chain.md` 源码逐条核实；〔设计〕= 既有设计文档的既定约束；〔待确认〕= 本稿新增推断或待拍板项。

| 项 | 内容 |
| --- | --- |
| 文档性质 | 执行端（AI Coding Tool）接入操作手册草稿 |
| 版本 | V1（草稿，未定稿） |
| 日期 | 2026-11-01 |
| 记录人 | 司南（产品经理 Agent） |
| 状态 | **草稿待评审** → 过稿后以 `docs(...)` 单独提交 |
| 上位约束 | `ai-pm-mcp-executor-positioning-v1.md`、`ai-pm-multi-repo-model-v1.md`、`ai-pm-adr-0001-tenant-agent-executor-identity.md`、`ai-pm-principal-identity-v1.md`、`ai-pm-slice-d-execution-pipeline-v1.md` |
| 适用读者 | 执行端使用者（开发者 + 其 AI 工具）、平台管理员 |

---

## 〇、一句话结论

> 用户 PC 上**不安装任何 ai-pm 组件**：执行端工具（Trae / Codex / WorkBuddy / z-code / dsh 等）凭「平台地址 + 一把绑定真人的 API Key」通过 MCP 接入；任务在 Web 控制台上创建与挑选，执行端用 `pm_claim_task` 显式领取（Model A），在本地自备的工作树中干活，经 MCP 回写结果。〔已核实 + 设计〕

---

## 一、部署拓扑：什么装在哪里

| 组件 | 位置 | 职责 | 当前状态 |
| --- | --- | --- | --- |
| pm-api（含 `/mcp`、dispatch worker） | **服务端**（docker-compose / 本机 8090） | 任务/状态机/审批/验收权威源 + MCP Server | 已实现，`127.0.0.1:8090`〔已核实〕 |
| Web 控制台（frontend） | 服务端 | 建任务、看任务列表、审批、审计 | 已实现 |
| lmms token-service | 服务端 | 签发绑定真人的 API Key | 外部依赖 |
| 执行端工具（trae/codex/…） | **用户 PC** | 读代码、改文件、跑验证、commit | 各自安装，与 ai-pm 无安装耦合 |
| Runner / Worker（无人值守拉起进程） | 用户 PC | 无头启动执行端、超时重试 | **二阶段（D3/T-030）**，当前 `AI_PM_RUN_DISPATCH` 默认 off、执行器仅 NoopExecutor〔已核实〕 |

**常见误解纠正**：不存在「安装 ai-pm 的 worker」这一步。平台调度 worker 在服务端；用户侧无人值守 Runner 是二阶段才有的组件。

---

## 二、接入前置（管理员侧，一次性）

按顺序完成，缺一会被 401/403 拦住：

| # | 动作 | 校验方式 | 失败表现 |
| --- | --- | --- | --- |
| 1 | 部署并启动 pm-api（含 lmms 栈、数据库） | `GET http://127.0.0.1:8090/health` | 连接失败 |
| 2 | lmms 为每个使用者签发 **API Key**（client_credentials，绑真人） | token 含 `sub`=真人、无 `aud`、`permissions` ∋ `PM_MCP_READ` | 401 `IAM_AUTH_1004/1005`；403 `IAM_PERM_3001` |
| 3 | 首次持 key 调 `GET /v1/pm/me` → principal 自动 provision（`is_active=false`） | 返回 200 + `is_active:false` | — |
| 4 | 管理员激活该 principal：`PATCH /v1/pm/principals/{id}`（body `{"is_active":true}`） | 再调业务端点 200 | 403 `IAM_PERM_3002` |

〔已核实：`principal-identity-v1.md` §6/§9；`runbook-login-chain.md`〕

**⚠ 跨机接入限制（当前实现）**：`/mcp` 的 rmcp `allowed_hosts` 默认 loopback —— **M1 按本地单机设计**，当前仅「与 pm-api 同机的工具」可直连 `http://127.0.0.1:8090/mcp`。多 PC / 远程接入需先解决：放宽 host 白名单 + TLS + 反向代理与鉴权加固（见 §十 G1，待裁）。〔已核实 `server.rs:10-14`〕

---

## 三、接入配置（每个工具一份）

MCP 端点信息（同机阶段）：

| 项 | 值 |
| --- | --- |
| URL | `http://127.0.0.1:8090/mcp` |
| Transport | Streamable HTTP（无状态 JSON 模式，无 SSE 通知）〔已核实〕 |
| 认证 | `Authorization: Bearer <API Key>` |
| 必带头 | `Host`（rmcp host 校验，缺失 → 400 `missing Host header`）〔已核实〕 |
| 附加权限 | 文档类工具（`pm_submit_document` 等 4 个）另需 `PM_MCP_DOCUMENT_WRITE`〔已核实〕 |

通用 MCP 客户端配置（各工具配置入口与文件格式不同——Trae 的 MCP 添加界面、Codex 的 `config.toml`、WorkBuddy 的 `mcp.json`、z-code / dsh 各自设置；字段语义一致）：

```json
{
  "mcpServers": {
    "ai-pm": {
      "url": "http://127.0.0.1:8090/mcp",
      "headers": {
        "Authorization": "Bearer <你的-api-key>",
        "Host": "127.0.0.1"
      }
    }
  }
}
```

〔待确认〕**接入向导**：建议平台提供「执行端接入」页，按工具生成对应格式的配置片段 + 登记档位（transport / 自治档 / 租约档），把 Q9/Q10/Q11 一次收口。是否进 V1 待评审。

---

## 四、操作流程

### 4.1 人工 / 半自动模式（Model A，当前唯一可用）

**前提**：任务已在 Web 控制台创建为 `ready` 并有可领取 Run。当前**没有** `task.list` / `pm_list` MCP 工具——任务挑选发生在 Web 控制台，执行端按 `task_id` 显式领取。〔已核实：`server.rs` 工具清单无 list 工具〕

| 步 | 动作 | 谁做 | MCP 调用 / 结果 |
| --- | --- | --- | --- |
| 1 | 在 Web 控制台挑选 `ready` 状态任务，记下 `task_id`、`project_id` | 人 | — |
| 2 | 打开工具，对 agent 说「领取任务 <task_id>」 | 人 → agent | `pm_claim_task{project_id, task_id, client_name:"trae", client_version?}` |
| 3 | 原子领取 + 签发**一次性租约 token**（明文仅返回一次，库内只存 sha256）+ 锁定各仓 base_commit | 平台 | 返回 `{run_id, lease_token, …}`；无 Run→not-found；非 ready / 已有活跃 run→conflict〔已核实〕 |
| 4 | 读取执行契约 | agent | `pm_get_run_context{project_id, run_id, lease_token}` → 目标 / 约束 / 验收标准 / 交付物 / **每仓 base_commit** / 知识引用 / **git 分支** / 策略快照〔已核实〕 |
| 5 | 报告开工 | agent | `pm_report_progress{stage:"started", …}` → 任务经状态机守卫推进到 `running`〔已核实〕 |
| 6 | 按 §六 准备本地工作树 → 干活 → 验证 → commit | agent + 人监督 | 工具内的确认按钮即人工介入点 |
| 7 | （长任务）定期心跳续租 | agent | `pm_report_progress` —— **续租是唯一心跳**，不上报就等租约过期被回收〔已核实〕 |
| 8 | 遇架构分歧 | agent | `request_decision` 类（定位文档 §4.1 提议，**未实现**，见 §十 G4）→ 任务停在 `awaiting_decision` 走平台审批 |
| 9 | 提交结果（commits[] + 测试证据 + 摘要 + 知识提案） | agent | `pm_submit_result` —— **当前为冻结 draft 形状，调用返回 Unsupported `implemented_in:D4`**，落库在 D4（见 §十 G2）〔已核实〕 |

**中断语义**（A4，一律按会话可能彻底丢失设计）：人下班 / 关工具 / 崩溃 → 不续租 → 租约到期 → 下一次领取语句直接命中回收，任务回 `ready`，重领取 + 读上下文即可恢复，无需断点续跑。〔已核实：回收与领取同路径〕

### 4.2 无人值守模式（二阶段，暂不可用）

Runner 领取 → 本地建 worktree → 以该 cwd 无头拉起执行端进程 → 监控/超时/重试。门控 `AI_PM_RUN_DISPATCH` 当前默认 off；`pm_claim_task` 本身不受该开关影响（它只门控无人值守 worker）。〔已核实〕

---

## 五、租约与心跳

| 项 | 当前值 / 规则 | 来源 |
| --- | --- | --- |
| 默认租约时长 | `AI_PM_RUN_LEASE_SECS`，默认 **300 秒** | slice-d §5.4〔设计〕 |
| 续租方式 | 仅 `pm_report_progress`（严格向前，不可回拨） | 已实现〔已核实〕 |
| 人工模式风险 | 5 分钟对「人点确认节奏」明显偏短，任务会被误回收 | 定位文档 §五〔设计〕 |
| 建议 | 人工档调 env 加长 + agent 定时心跳；最终按预注册档位分档（Q10 方案 C） | 〔待确认〕分档数值 |

---

## 六、执行目录与工作空间：MCP 不碰文件系统

MCP Server 在远端，**不解决本地目录**。分工如下〔设计：`multi-repo-model-v1.md` §2.1/§3.3/Q7〕：

| 职责 | 归属 |
| --- | --- |
| 工作空间**布局**（`repos/<name>`、每仓分支 `task/<TASK>/<repo>`、base_commit） | **平台下发**（契约字段），执行端不得自行决定 |
| 工作树**物理创建**（`git fetch` + `git worktree add`） | **本地**：人工模式由 agent 执行（执行端能力底线 = 调 MCP + 读写文件 + 跑 git）；无人值守模式由 Runner 准备并把 cwd 设好 |
| 隔离保证 | 一任务一工作空间目录、每仓一棵 worktree、独立分支、**禁止跨任务共用目录**；原子领取保证同任务同时只有一个活跃 run |

多仓任务示例（三仓）：

```bash
git -C repos/map-core   fetch origin && git worktree add ../ws/TASK-025/repos/map-core   -b task/TASK-025/repo-core   <core-base-sha>
git -C repos/map-editor fetch origin && git worktree add ../ws/TASK-025/repos/map-editor -b task/TASK-025/repo-editor <editor-base-sha>
git -C repos/map-assets fetch origin && git worktree add ../ws/TASK-025/repos/map-assets -b task/TASK-025/repo-assets <assets-base-sha>
# 打开 ws/TASK-025 作为该会话的工作目录，按 path_scope 改动，每仓分别 commit
```

base_commit 与分支从 `pm_get_run_context` 契约中取，不得使用本地已有的随意分支。

---

## 七、一台 PC 装 N 套开发工具的管理规范

**原则：按任务组织资源，不按工具组织；按人分发凭据，不按机器分发。**

| 维度 | 规范 | 机制保障 |
| --- | --- | --- |
| 身份 / 凭据 | **每人一把 key**。同一人的 N 个工具配同一把 key（归因同一真人）；多人共用 PC → 各自在各工具 profile 配自己的 key。**禁止一把 key 多人共用**（归因塌缩：分不出谁在用） | principal-identity §10.6〔设计〕 |
| 区分哪个工具干的 | `pm_claim_task` 的 `client_name` / `client_version` 记入 `task_run`（V1 不用于分档，仅记录） | 已实现〔已核实〕 |
| 任务互斥 | N 个工具并行 = 各领**不同**任务；两个会话抢同一任务，第二个 claim 直接 conflict | 原子领取 + 活跃 run 唯一索引〔已核实〕 |
| 目录隔离 | 工作空间按 `ws/<TASK-id>/` 组织，**不按工具 clone**（避免 N 份全量拷贝各自漂移）；工具只是进程，cwd 指向任务工作空间 | L1 物理隔离〔设计〕 |
| 配置管理 | endpoint + key 每工具一份；建议用接入向导生成片段（§三〔待确认〕） | — |
| key 治理 | 离职 / 泄露 → 停用 principal 或换 key；一处停用、各工具在 token 过期（≤15 分钟）后 fail-closed | 已实现〔已核实〕 |
| 审计追溯 | 「哪个 commit 是谁、用哪个工具、哪次 run」→ `task_run` 链路已齐；MCP 审计三列（`actor_client` / `actor_auth_mode`）受 rmcp transport 限制，登记 T-059 | 〔已核实〕 |

**WorkBuddy 特别提醒**：workbuddy 可同时充当 PM 界面与执行端（同一 key 归因同一真人，审计成立）。须防「AI 派活给自己、无人审」：高风险动作（写正式知识库、改主分支）无论执行端自动化档位多高都走平台人工确认（定位文档 Q11）。〔设计〕

---

## 八、MCP 工具清单与实现状态（11 个，2026-11-01 核实）

| 工具 | 作用 | 状态 |
| --- | --- | --- |
| `pm_submit_technical_design` | 提交技术设计，幂等入队 AI 分析 | ✅ 已实现 |
| `pm_get_submission_status` | 查询提交事件处理状态 | ✅ 已实现 |
| `pm_get_task_context` | 读取任务上下文快照（字段/需求修订/里程碑） | ✅ 已实现 |
| `pm_submit_document` | 创建 Git 知识文档（需 `PM_MCP_DOCUMENT_WRITE`） | ✅ 已实现 |
| `pm_propose_document` | 知识提案过质量门（红线挂起人工确认） | ✅ 已实现 |
| `pm_confirm_document_proposal` | 人工确认红线提案 | ✅ 已实现 |
| `pm_veto_document_revision` | 撤回已发布知识修订（veto 窗口内） | ✅ 已实现 |
| `pm_claim_task` | Model A 显式领取 + 一次性租约 token + 锁多仓基线 | ✅ 已实现 |
| `pm_get_run_context` | 凭租约读执行契约 | ✅ 已实现 |
| `pm_report_progress` | 心跳续租 + 首次 `started` 推进 running + blocked 审计 | ✅ 已实现 |
| `pm_submit_result` | 提交 commits[] + 证据（形状已冻结） | ⚠️ **draft 形状，落库在 D4** |

命名说明：定位文档 §4.3 提议 `task.*` / `knowledge.*` 点分命名（Q1 待收敛）；**当前实现与本文一律用 `pm_*`**，以代码为准。

---

## 九、安全底线

1. **每人一把 key**，不共用（归因原则）。
2. **知识库默认提议优先于直写**：`pm_propose_document` 红线（外部承诺 / 删历史 / 改已交付）必须人工确认后才落 Git。〔已核实〕
3. **状态机唯一裁决**：执行端只能经 MCP 触发状态迁移，无通用写接口；非法迁移返回明确错误码。
4. **高风险人工确认**：写正式文档、改主分支等动作（Q11 自动批准矩阵，待落）。
5. key 泄露处置：停用 principal，≤15 分钟 fail-closed（依赖 token 过期，即时失效是已知取舍）。〔设计〕

---

## 十、当前缺口（接入前必须知道）

| # | 缺口 | 影响 | 处置 |
| --- | --- | --- | --- |
| G1 | `/mcp` loopback-only（M1 本地单机设计） | **跨机 / 多人接入不可用** | **已登记 T-081**（host 白名单 + TLS + 反代鉴权，安全评审后定） |
| G2 | `pm_submit_result` 仅 draft 形状（D4） | 人工流程**最后一环提交结果未落库**，闭环差一块 | 走 T-040/T-041（D4 排期）；接入验收先做到「提交即返回 Unsupported 且可解释」 |
| G3 | 无任务列表 MCP 工具 | 挑任务必须切 Web 控制台 | **已登记 T-082**（`pm_list_tasks`） |
| G4 | `request_decision` 未实现 | 执行期分歧无通道，只能人盯 | 走 T-042/T-043（D5 排期） |
| G5 | 租约默认 300s，人工档未分档 | 人工模式易被误回收 | env 先调长；人工档默认值随 T-080 一并裁定 |
| G6 | 审计三列 T-059 未落地 | 「哪个工具」暂只能看 `task_run.client_name` | 走 T-059（随 D3 执行端接入一并裁） |

---

## 十一、待确认问题

| # | 问题 | 结论 / 建议 |
| --- | --- | --- |
| R1 | 接入方式：平台向导 vs URL 引导式接入 | **用户已定向（2026-11-01）：URL 引导式接入** —— 用户在执行端粘贴平台 URL 并说「帮我安装」，agent 按该 URL 返回的安装方法自动完成配置；已登记 **T-080** |
| R2 | 跨机接入的暴露方案（反代 / TLS / host 白名单） | **已登记 T-081**；同机阶段先跑通闭环 |
| R3 | 人工档租约时长与心跳间隔默认值 | 建议人工档默认 ≥2h、心跳 ≤30min；随 T-080 裁定 |
| R4 | `pm_list_tasks` 是否 V1 补齐 | **已登记 T-082**：消除「切控制台拿 task_id」的断点 |
| R5 | `pm_*` vs `task.*` 命名收敛（Q1） | 契约冻结前必须定死，否则改名成本高 |

---

本方案为产品设计参考，具体实施请结合公司合规与法务要求确认。
