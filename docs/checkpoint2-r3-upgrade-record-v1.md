# 检查点 2 · R3 `.10` 共享栈升级窗口记录 V1.0

> **R3 收官记录**（2026-11-08，窗口完成于 11:28 前后）。
> 范围：`.10` 共享栈从「23 工具 / 迁移 47」升级到 **0.2.0 三服务一致 pin = master@239b539**
> （含检查点 2 R1/R2 + ACP-09 DEF-E2E-1 修复 + 并行批 s02-ui/setup），迁移 **0048~0055 全量应用**，
> ACP-09 E2E **20/20 PASS**。结论：**R3 关闭；R4（`.59` 真机联调）就绪待执行**。

## 一、窗口前侦察（§7.1 前置核实）

| 项 | 实况 | 判定 |
|---|---|---|
| `.10` 栈现状 | pm-api 旧镜像（**23 工具**）、迁移 max=**47**、ai-pm-worker Exited(137)（OOMKilled=false，系先前窗口期被显式 stop） | 与设计预期一致（Δ = 0048~0055 + 3 工具） |
| `.env.prod` | `AI_PM_MCP_ALLOWED_HOSTS` / `AI_PM_SECRET_KEY` / `AI_PM_MCP_LEGACY_PERMISSION_GRANT=1`（F-7 过渡开关）/ `AIPM_VERSION=0.2.0`（并行批参数化） | P4 通过 |
| SSH 通道 | runbook 测试凭据 + `scripts/e2e/ssh_run.py`（paramiko） | 可用 |

## 二、事件与处置（诚实记录）

1. **并发部署冲突（11:04~11:07）**：本窗口 pg_dump 备份之际，**并行会话对 `.10` 执行了全栈重建**
   （compose 项目 superpowers，全新空卷 + 54 迁移 + 其自有 pm-api:0.2.0 构建）。
   本窗口立即**冻结全部 `.10` 写操作**并上报仲裁；用户裁定并行会话已收场（其成果已提交
   master：28b5900/f67a6be/239b539，其中 239b539 =「空库强制注册首个账号」特性，
   证实清库重建为有意行为）。
2. **迁移指纹 VersionMismatch(25) 根因 = CRLF/LF 分叉**：并行会话的镜像自**主 checkout**构建
   （工作文件 CRLF，sqlx 指纹 = CRLF sha384 入库）；本窗口的干净 worktree（`.gitattributes`
   `* text=auto eol=lf` 规范化 **LF**）构建的镜像必然 VersionMismatch。库为全新空库
   （AGENTS 规则 1：零存量数据），处置 = **清库重迁移**（DROP 全 schema + `_sqlx_migrations`
   → 我方 0.2.0 镜像从零迁移，LF 指纹定版）。**遗留险源**：未来任何从主 checkout（CRLF）
   的 `docker build` 部署都会再次 VersionMismatch——建议独立小批在 Dockerfile 构建段加
   `migrations/` LF 规范化（指纹确定性），见 §五。

## 三、窗口执行序列（最终定版）

| 步 | 操作 | 结果 |
|---|---|---|
| 1 | 干净 worktree 构建（a978e27 三镜像，作诊断基线） | ✓ |
| 2 | 冲突冻结与仲裁 | ✓（见 §二.1） |
| 3 | 重建于 master@**239b539** → 三镜像 tag 0.2.0（pm-api `c79349f001fd` / pm-worker `eeda1158420c` / ai-pm-worker `6ef6fb10dccb`） | ✓ exit 0 |
| 4 | save/gzip/SFTP 上传（各 66~74MB，13~26s）+ `docker load` | ✓ |
| 5 | 清库（空库，零数据）→ 最小重建 `pm-api pm-worker ai-pm-worker`（`--no-deps --force-recreate`，token-service 未动） | ✓ |
| 6 | 迁移 0048~0055：`_sqlx_migrations` = **54 行 / max=55**，pm-api listening | ✓ |
| 7 | 引导：首用户注册 admin（lmms setup_required=false）→ 建团 checkpoint-team（激活+owner）→ 项目 e2e-acp09 → 设备 e2e-runner-10（active）→ SQL 播种 E2E 任务行 | ✓ |
| 8 | **ACP-09 E2E 复跑**（驱动上传 `.10` 本地执行，常量替换为实际 id，PSQL 指向主库 aipm） | **20/20 PASS** |

## 四、回归清单对账（§7.3 R1~R8）

| # | 项 | 结果 |
|---|---|---|
| R1 | 基础链：注册/登录/JIT principal/建团激活/`/v1/pm/me` | ✓（fresh boot 全链） |
| R2 | 工具面 `tools/list` = **26**（含 discover/reacquire/checkpoint） | ✓ |
| R3 | ACP-09 E2E 20 项（S2~S10 含 **S9 终态钩子**） | **20/20 PASS** |
| R4 | cpk 负向冒烟：伪造 cpk → **401**（fail-closed 不回退，ACP-465 方向） | ✓ |
| R5 | checkpoint-1 链路抽测 | **N/A**（fresh 栈无关联仓库 intake；以本记录 SQL 引导链替代） |
| R6 | 0055 基线红（devenv revoked_device） | ✓（本地全量套件绿 + .10 应用 0055） |
| R7 | lmms 零回归 | ✓（fresh boot：注册/登录/JWT 签发链实证） |
| R8 | UI 冒烟 | ai-frontend-web 容器 Up；深度 UI 验收属并行批 s02-ui E2E（11ab905，5/5 PASS） |

**ACP-09 三边界对账**：边界 2（`.10` 升级回归）= **关闭**（本记录）；边界 1/3 = R4（`.59` 真机）执行项。

## 五、遗留与移交

1. **迁移指纹确定性**（P1 建议独立小批）：Dockerfile 构建段对 `migrations/` 做 LF 规范化，
   杜绝 CRLF/LF 双指纹陷阱（§二.2）；本记录前先以「干净 worktree 构建」纪律兜底。
2. **`.10` 现为全新空生产栈**：admin/Admin@123456（首用户）、团队 checkpoint-team、
   项目 e2e-acp09、设备 e2e-runner-10（未签发正式 cpk——R4 时按 §2.3 流程签发）。
   原 47 迁移时代数据已被并行批有意清空（用户裁定）。
3. **R4 就绪清单**：`.59` 部署 Runner 二进制（本仓 `apps/ai-pm-runner`，Windows 优先）→
   签发 cpk 注入 `.env` → `AI_PM_RUNNER_GIT_MAP` 配置用户自备凭据 clone → 按 §8 清单跑
   十项对照 + 三边界 1/3 关闭。
4. 本仓 master HEAD = 11ab905（239b539 + s02-ui 测试文件，不影响已部署镜像）；
   **origin 未 push** 的提交池（自 d2b60b0 起 5 个）由用户定推时机。
