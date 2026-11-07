# S03-ACP 真实栈 E2E 报告 V1.0 —— 通信面故事线（192.168.2.10 平台 + 192.168.2.59 Runner）

| 项目 | 内容 |
|---|---|
| 报告性质 | 真实栈端到端实测报告（目标 ②：部署 + F-7 开关 + Runner 连真库全故事线） |
| 代码基线 | feat/s03-acp-comm-face @ 1f95506（平台镜像 aipm/pm-api + aipm/ai-pm-worker @ 0.1.0；Runner Linux 二进制同源构建） |
| 拓扑 | Platform = 192.168.2.10（24 服务 compose：lmms 1.7.0-pg 鉴权链 + pm-api/ai-pm-worker/observability）；Runner = 192.168.2.59（Debian，Linux 二进制 + 假 Agent python3） |
| 结论 | **[E2E-STORYLINE-PASS]**：介入→终止→REST 回复→Continuation→消费回执全链真实栈打通；F-7 过渡开关实测生效 |

---

## 1. 部署实录

| 步骤 | 结果 |
| --- | --- |
| 镜像构建（pm-api + ai-pm-worker，desktop-linux builder） | ✅（v2rayN DNS 劫持导致两次构建失败；**断开 v2rayN 后成功**——与 runbook"代理劫持"档案同因） |
| 镜像传输（docker save/gzip → paramiko SFTP → docker load） | ✅（77MB；SFTP 需绝对路径 + MSYS_NO_PATHCONV=1，见 §4 坑位） |
| compose 更新 | ✅ `AI_PM_MCP_LEGACY_PERMISSION_GRANT` 加入 pm-api environment（`${VAR:-}` 形式，默认空 = fail-closed） |
| 服务器 .env.prod | ✅ `AI_PM_MCP_ALLOWED_HOSTS=192.168.2.10,192.168.2.59,192.168.2.4` + `AI_PM_MCP_LEGACY_PERMISSION_GRANT=1` |
| 全栈启动（scripts/start-prod.sh，24 服务） | ✅（需先清三代旧代容器名冲突；aipm-infra-*/observability-* 旧容器移除，卷保留） |
| 迁移 | ✅ pm-api 启动自动应用 0036→0047（`_sqlx_migrations` 46 行 max=47；卷内旧 aipm 库为 0039 时代数据 + **CRLF checksum 污染**，已 DROP 重建） |

## 2. F-7 过渡开关实测

- pm-api env `AI_PM_MCP_LEGACY_PERMISSION_GRANT=1` 下：Runner（仅 `PM_MCP_READ` 令牌）成功调用四新工具（pm_claim_task/pm_report_interaction/pm_poll_commands/pm_ack_command/pm_report_consumption）——**开关生效**。
- 默认（开关未设）= fail-closed：新工具 403（`IAM_PERM_3001`）——双态行为符合设计（单测钉住）。
- **裁决执行记录**：E2E 期间开关=1 属运维显式决定；V-05-1 权限字典落地后撤除。

## 3. 故事线证据（`[EVIDENCE]` 逐条，完整记录 target/e2e/storyline_evidence.txt）

```
S0-login      admin JWT（lmms :3000 登录链，purpose=user_login）
S1-runner     Runner 于 .59 启动（3s 认领间隔）
S2-interaction Runner 自领 queued Run → spawn Agent(S1) → 2s 内侦测 .aipm/interaction.json
              → pm_report_interaction → interaction PENDING（D-03 同事务 terminate 命令）
S2-guard      run=claimed, task=awaiting_decision, shield=human_wait（六情形决策表情形 1）
S3-terminate  Runner 领命令 → Job Object 整树终止 → ack（ExitOutcome=Killed）
S4-reply      人类经 REST POST /v1/pm/interactions/{id}/responses → 200
              （A* 可信入口；同事务 continue 命令下发，guard→start_wait）
S5-consumed   Runner 领 continue 命令 → preflight（guard 快照）→ spawn Agent(S2)
              回复注入（reply.json + env）→ ack(session_created+input_injected)
              → pm_report_consumption → CONSUMED；shield=none（释放点正确）
S6-agent-done Agent S2 done 标记含人类回复原文（INPUT_INJECTED 证据闭环）
S7-final      interaction=consumed | shield=none | task=running | ledger=completed
              | terminate=acked | continue=acked
```

## 4. E2E 实测修复（已提交 1f95506）+ 新发现

### 修复（Runner 三处生命周期/传输缺陷）
1. **reap 误判**：terminate 杀掉的 Agent 被 reap 当意外退出 → 丢弃 Run → continue 无人消费。awaiting_continue 期间预期退出保持 Run。
2. **收割竞态兜底同病**：循环尾部 `child.id().is_none()` 在 terminate 当拍成立即丢弃——补 `&& !awaiting_continue`（双保险）。
3. **reqwest 无超时 + 连接复用黑洞**：Platform rmcp 每 session 响应后即关连接，复用陈旧连接 → TCP 重传挂死监督循环。加 30s/10s 超时 + 禁用空闲复用。

### 新发现（记入后续切片）
- **F-14 [P1→V-05-1] worker_id 身份不稳定**：命令路由 `worker_id = mcp:{client}:{token8}`，token8 随每次登录变化 → Runner 重启后旧 continue 命令无人认领（E2E 实测死锁态，靠重置故事线解）。**V-05-1 Client Registry（稳定 Client 身份）前，生产语义下 Runner 不得在介入窗口重启**——临时缓解已文档化，正式修复 = Client 身份稳定化。
- **F-15 [P2] template1 污染面扩大**：192.168.2.10 服务器两处 PG（宿主 5432 旧实例 + 栈卷）均发现历史污染（template1 整套 ai-pm schema / aipm 库 CRLF checksum）——所有环境初始化脚本须防误指 template1；已修复宿主 5432 实例，栈卷 aipm 库已重建。

### 坑位档案（运维）
- v2rayN 开启 → Docker 构建容器 DNS 劫持（rsproxy 解析超时）→ 构建必失败；断开即愈。
- MSYS 路径转换：`/tmp/...` argv 会被转成 `C:/Program Files/Git/tmp/...` → SFTP ENOENT；全程 `MSYS_NO_PATHCONV=1`。
- pkill 自杀：`pkill -f ai-pm-runner` 匹配执行 shell 自身 cmdline → 会话自杀（rc 127）；用 `pkill -f '[a]i-pm-runner'`。
- Docker Desktop 挂载 mtime/内容缓存 → cargo 增量编译漏检源码变更 → 重建前必须 `touch` 源文件 + `cargo clean -p <crate> --release`，并以 grep 验证二进制含新字符串。
- paramiko 5.x SFTP put 相对路径在 Windows 失败 → 一律 os.path.abspath。

## 5. E2E 范围声明

- **已覆盖**：通信面全故事线（claim/interaction/terminate/reply/continuation/consume/guard 生命周期/账本）+ F-7 开关双态 + 鉴权链（lmms 登录 → JWT → JWKS）。
- **未覆盖（后续）**：真实 DSH/OpenCode CLI Adapter（假 Agent 已验证协议面；OpenCode 待其模型 key 配置后作第二阶段）· pm_submit_result 交付验证链（需真实仓库）· 多 Runner 竞争 · Worker（服务器 Model B）载体命令消费（V-05-2③ 维持现状）。

---

**执行**：ZCode（E2E 驱动脚本 scripts/e2e/{ssh_run,run_storyline,fake_agent}.py 已入库）
**日期**：2026-11-07
