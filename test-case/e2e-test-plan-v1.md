# AI-PM · E2E 测试计划 V1.0

> **上位文件**：`test-case/full-lifecycle-test-plan-v1.md`（全流程测试计划）。本文件只管 **L2 服务级 E2E + L3 UI E2E** 两层。
> **性质**：真进程、真数据库、真 Git 远端、真凭据的端到端验证。**禁止**用 mock / fixture 冒充后端。
> **基线**：master @ `2b3f509e`；迁移基线 `migrations/0001_baseline.sql`。

---

## 1. 为什么需要独立 E2E 层

L1（`#[sqlx::test]` + axum `oneshot`）覆盖不到四类真实故障，这四类恰好是生产事故高发区：

| # | 覆盖不到的 | 真实故障样例 |
|---|---|---|
| **E1** | **进程边界** | `router()` 能过但 `pm-api` 二进制起不来（env 缺失、端口占用、迁移撞号） |
| **E2** | **Worker 异步链** | `integration.job` 入了队但 worker 没领取；租约过期不回收；outbox 死信 |
| **E3** | **真实 Git 协议与凭据** | SSH 私钥 / host key / 仓库权限 / clone 超时 / URL 硬化 —— 本次接入私钥即为此层专有 |
| **E4** | **真实浏览器渲染** | 路由守卫跳转、data-testid 契约、渲染后异步态 |

因此 E2E 层的准入条件是：**后端进程 + DB + worker 全部真身**。

---

## 2. 载体与拓扑

### 2.1 L2 服务级 E2E（**已落地**：`apps/pm-api/tests/e2e_flow.rs`）

```
┌──────────── 一键脚本 test-case/evidence/e2e-flow/run-e2e-flow.sh ─────────────┐
│                                                                                │
│  ┌──────────────────────┐   DATABASE_URL    ┌─────────────┐  真实 Git over SSH │
│  │ pm-api 真路由        │ ─────────────────► │  QA PG      │  ┌───────────────┐ │
│  │ (axum oneshot)       │                    │  容器        │─►│ 192.168.2.16  │ │
│  │ + run_import_job     │                    │  :16433      │  │ github.com   │ │
│  │ (真 clone --bare)    │                    │  一次性库     │  │ ~/.ssh 私钥   │ │
│  └──────────────────────┘                    └─────────────┘  └───────────────┘ │
└────────────────────────────────────────────────────────────────────────────────┘
```

| 组件 | 选择 | 理由 |
|---|---|---|
| PG | 容器 `aipm-pg-gate`（宿主 `127.0.0.1:16433`）；`#[sqlx::test]` 每用例独立临时库，自毁 | 不碰正式库；`127.0.0.1` 规避 IPv6 回环黑洞 |
| 路由 | `pm_api::routes::router(pool)` 真路由 + `oneshot` | 走全部中间件与 handler，不经 socket 也仍是真契约 |
| 导入 | `pm_application::project_import::run_import_job`（**真** `git clone --bare` + 真事实采集 + `FakeRepoAnalysisModel`） | 验 E3（真实 Git 协议与凭据）；Fake 模型保证断言可复现 |
| 身份 | `off` 模式的 `dev-off` 真人主体（`auth_mode="off"`，非 api_key） | 满足确认类命令的真人门（`ensure_trusted_context`） |
| Git 远端 | `git@192.168.2.16:tooling/ai-pm.git`（GitLab）+ `git@github.com:luqizheng/world-view-rs.git`（GitHub） | 真协议 + 真凭据（E3） |
| 私钥 | `~/.ssh/id_rsa`（ED25519），经平台自己的 `integration.project_ssh_key` 通道使用 | 平台**不**直读家目录密钥；密钥经 `seal_text` 密文入库，`write_temp_key` 落临时目录即用即删 |

> **为何不用 Node/Playwright 起真进程**：L2 要验的核心是「平台自己的解密 + 临时密钥 + Git 协议栈」。放在 Rust 集成测试里可以直接调 `seal_text` / `write_temp_key` / `run_import_job`，避免在 TS 侧**重新实现**一遍 AES-256-GCM 信封（两套实现漂移 = 假绿或假红）。真进程启动（E1）与 worker 异步链（E2）由 L3 与 L4 覆盖。 |

### 2.2 L3 UI E2E

| 组件 | 选择 |
|---|---|
| 浏览器 | 系统 MS Edge（`channel: "msedge"`，免 Chromium 下载），1440×900，`locale: zh-CN` |
| 前端 | vite 独立端口 `:8860`，`PM_API_URL` 指向本套 pm-api，`VITE_USE_MOCK=0` |
| 登录态 | 注入 `localStorage`：`prism_access_token` / `prism_refresh_token` / `prism_expires_at` / `prism_user`（同 `e2e/team/stack.ts` 既有约定） |
| 身份 | 每个用例独立 `tid` → 独立租户，互不污染 |

---

## 3. SSH 私有key 接入（本次新增能力）

### 3.1 家目录密钥的三层通路

| 层 | 机制 | 落点 |
|---|---|---|
| **① Git 客户端** | `~/.ssh/config` 声明 `Host github.com → ssh.github.com:443`（GitHub 禁 22 端口）+ `Host 192.168.2.16`；`IdentityFile ~/.ssh/id_rsa`，`IdentitiesOnly yes` | 开发者手工 `git clone` / `git ls-remote` |
| **② 平台密钥库** | `integration.project_ssh_key` 表存 `private_key_ciphertext`（AES-256-GCM，`nonce(12)‖ciphertext‖tag(16)`，AAD 绑定 project_id）；平台生成公钥 + 指纹，只回公钥 | `crates/pm-application/src/secret.rs:47`、`repository/ssh_key.rs:56` |
| **③ 运行时注入** | `open_ssh_private_key` 解密 → `write_temp_key` 落 `0700` 临时目录 → `GIT_SSH_COMMAND` 带 `-o IdentitiesOnly=yes -o BatchMode=yes -o StrictHostKeyChecking=accept-new` 与独立 `UserKnownHostsFile` | `crates/pm-application/src/git_access.rs:83` |

### 3.2 E2E 注入方式（关键）

平台**不**读家目录密钥。E2E 要用 `~/.ssh/id_rsa` 访问真实仓库，必须把它**装进平台自己的密钥库**：

```
读 ~/.ssh/id_rsa（明文）
  → 用平台同口径 AES-256-GCM 加密（key = sha256(AI_PM_SECRET_KEY)，AAD = project_id）
  → UPDATE integration.project_ssh_key SET private_key_ciphertext = <密文> WHERE project_id = <pid>
  → POST /v1/pm/projects/{pid}/repositories { url: "git@192.168.2.16:tooling/ai-pm.git" }
  → POST /v1/pm/repositories/{rid}/verify   ← 平台用注入的私钥跑真 git ls-remote
```

> **安全边界**：密文只在一次性库里、跑完即删；私钥明文不落盘到仓库、不进日志、不进证据目录。脚本结束时打印**指纹**而非密钥内容。

### 3.3 SSH 前置检查（脚本第 0 步，失败即中止）

| 检查 | 命令 | 期望 |
|---|---|---|
| 私钥存在且可读 | `ssh-keygen -l -f ~/.ssh/id_rsa` | 指纹行 + `(ED25519)` |
| GitLab 可达 | `git ls-remote --heads git@192.168.2.16:tooling/ai-pm.git` | 至少 1 个 `refs/heads/` |
| GitHub 可达 | `git ls-remote --heads git@github.com:luqizheng/world-view-rs.git` | 至少 1 个 `refs/heads/` |
| 平台同款命令可达 | 用 `GIT_SSH_COMMAND`（与 `git_access.rs:93` 字面一致）重跑上两条 | 与上面一致 |

> **已知事实**：`git clone --depth 1` 对 GitLab 侧报 `early EOF`（服务器浅克隆协议问题），但**完整 `--bare` 克隆成功**（实测 15MB）——而平台走的正是 `clone --bare`，故不阻塞。E2E 只用完整 bare 克隆路径。

---

## 4. 用例矩阵

### 4.1 L2 · 流程主链（`FLOW-*`）——**已落地**（7 例全绿，见执行报告）

| ID | 场景 | 断言要点 | 状态 |
|---|---|---|---|
| **FLOW-00** | URL 硬化门 | `https://` / `file://` / `ext::` 三种非 SSH 地址 → 400 `invalid repository url` | ✅ PASS |
| **FLOW-01** | 接**真实远端**仓库并 verify | GitLab + GitHub 均 `status=verified`、`error_code=null`、HEAD sha 40 位；DB `status='verified'`、`head_commit_sha` 与响应一致 | ✅ PASS |
| **FLOW-02** | 冷启动导入（真仓库、真 clone） | `POST /imports` → 202 `queued` → `draft_ready`；`draft` 非空；`progress>0` | ✅ PASS（并入 FLOW-04 用例） |
| **FLOW-03** | SSH 私钥缺失时的负向 | 无 `project_ssh_key` → `verify.status=unreachable`、`error_code='ssh_key_missing'` | ✅ PASS（并入 FLOW-01 用例） |
| **FLOW-04** | 导入 apply → 提案 → 整批审批 | 每种类型至少一条提案；**每条提案条目数 ≤ 200**（执行器单批上限）；提案前 `delivery.{requirement,milestone,task}` 零写入；审批后落地 + 导入 `applied` + 审计 ×1 | ✅ PASS |
| **FLOW-05** | 需求全生命周期 + 哈希门禁 | create → revision(draft) → submit → confirm；`latest_state='approved'`；hash 不符 → 409 且终态不变 | ✅ PASS |
| **FLOW-06** | 设计 / 计划 / 任务状态机 | 设计 `draft→approved`；计划 `draft→in_review→approved` → 物化任务（`draft`）→ `mark-ready` → `ready`；非法流转 409；计划校验门禁 400 | ✅ PASS |
| **FLOW-07** | 交付读面 | 任务级 `items`/`total` 自洽；项目级每个已物化任务都出现在卷起里，`delivery_state` ∈ `delivered/partial/missing/no_contract` | ✅ PASS |
| **FLOW-08** | 跨租户隔离 | 非成员主体对本项目需求列表可见数为 0 | ✅ PASS |
| **FLOW-09** | 项目归档 + 私钥零泄露 | 归档 → `status='archived'`；重复归档 409 `ALREADY_ARCHIVED`；`POST /ssh-keys` 响应**无** `private_key` | ✅ PASS |

### 4.2 L3 · UI（`UIFLOW-*`）——**已落地**（`ai-frontend/e2e/flow.spec.ts`，8/8 PASS）

| ID | 场景 | 断言要点 |
|---|---|---|
| **UIFLOW-01** | 未建团 → 登录中转页分流 | `/auth/signing-in` → `/onboarding`；`/v1/pm/me` 的 `onboarding_required=true` |
| **UIFLOW-02** | 建团 → 邀请页横幅 | URL 落到 `/onboarding/invite`；横幅含团队名；后端创建者 = owner |
| **UIFLOW-03** | 项目列表 → 建项目 | `/project/create` 提交 → 列表页出现新项目；后端 `core.project` 落 5 人编制 |
| **UIFLOW-04** | 需求页读真 | `/project/requirements` 显示 FLOW-05 造出的需求；标题与后端一致 |
| **UIFLOW-05** | 任务页读真 | `/project/tasks` 显示 FLOW-06 物化出的任务；状态徽标与 DB 一致 |
| **UIFLOW-06** | 仓库设置页 | `/project/settings/repositories` 显示 FLOW-01 接入的两个仓库与其 `verified` 状态 |
| **UIFLOW-07** | 路由守卫 | 无 token 访问受保护页 → `/auth/login`；登出后返回守卫生效 |
| **UIFLOW-08** | 零 mock 校验 | 抓取网络面板：所有 `/api` 请求命中真 pm-api，无本地 fixture 响应 |

### 4.3 P0 发布验收 · 阶段 A~D（`P0-*`）——**已落地**（41 collected / 38 PASS / 3 NOT_IMPLEMENTED，见执行报告 §1.2 / §1.4）

载体：`ai-frontend/e2e/p0-0{1..7}-*.spec.ts` + `ai-frontend/e2e/p0/{fixtures,helpers,global-setup}.ts` + `ai-frontend/playwright.p0.config.ts`；一键 `bash test-case/evidence/p0/run-p0.sh`。
栈：真 vite `:8850`（`VITE_USE_MOCK=0`）+ 真 pm-api `:18490`（jwks）+ **真 ai-pm-worker**（`AI_PM_RUN_DISPATCH=on` / `AI_PM_RUN_EXECUTOR=mock`）+ mock LLM（OpenAI 兼容）+ 一次性库。

| 阶段 | 用例 | 状态 | 断言要点（落地口径） |
|---|---|---|---|
| **A** | P0-001a~007 | ✅ 7 PASS | 会话/建团/建项目/CC 初始化/7 事项 7 态/跨团队隔离 |
| **B** | P0-008a~014 | ✅ 8 PASS | 会话持久化 · worker 回复链 · 分析链（`completed`）· 人工门 403 · 需求/知识三存储权威 · 修订哈希门 · 知识红线（Git HEAD 不动）· 索引可删可重建 |
| **C** | P0-015~021 + 040~048 | ✅ 17 PASS + 1 NI（045b） | 物化/隔离 · Run 生命周期（承接人须 dev_agent）· 失败退避链 · 事件面 · 交付三态（租约活跃期提交）· Review 审核（重复裁决 409）· 多仓 manifest · 取消与迟到拒绝 · MCP 认证/设备绑定/install/工具集/注入/幂等 |
| **D** | P0-049a~056 | ✅ 7 PASS + 2 NI（051/052） | 分析链可追溯与幂等 · 来源可信度契约 · 统一协调（过期基线 409）· 质量门禁 · Revision↔Git 一致 · 恢复面（reindex_required 载体） |

> **三个"确定性替身"**（真链路内的受控端点，非 mock 后端）：mock LLM（provider 指向它，回复链/分析链走真 HTTP）、fake-codex 执行脚本（真 spawn + 真 worktree + 真 commit，只换执行内容）、本地 `file://` 仓库夹具（真 clone/镜像/worktree，差别只在远端地址）。
> **NOT_IMPLEMENTED 三项**（不计 PASS）：045b（MCP 回传通道 D4 未开通）、051（外部 Commit 发现管道未实现）、052（澄清采纳闭环专用管道未实现）。

---

## 5. 执行方式

### 5.1 L2 一键（**已落地**）

```bash
bash test-case/evidence/e2e-flow/run-e2e-flow.sh
```

| 参数 | 作用 |
|---|---|
| `--skip-ssh-check` | 跳过 §3.3 前置检查（仅本地排障） |

**脚本行为**：① 校验 QA PG 容器在跑并清理 `sqlx::test` 残留库 → ② SSH 前置检查（私钥可读 + 两个真实远端 `ls-remote` 通）→ ③ `cargo nextest run -p pm-api --test e2e_flow` → ④ 证据落 `test-case/evidence/e2e-flow/e2e-nextest-<stamp>.log`（已 gitignore）。

**实测**：7/7 PASS，单次 19.7 s。

### 5.2 L3 一键（**已落地**）

```bash
bash test-case/evidence/e2e-flow/run-e2e-flow-ui.sh
```

载体：`ai-frontend/playwright.flow.config.ts`（vite `:8860`，`VITE_USE_MOCK=0`，`reuseExistingServer: false`）+ `e2e/flow/global-setup.ts`（一次性库 `aipm_e2e_flow_ui` + 本地 JWKS `:18496` + 真 pm-api `:18495`）。与 team-qa（:8850/:18490 系）、s02（:8848）端口互斥。

实测：UIFLOW-01~08 全部 PASS，34.7s。UIFLOW-06 接入**真实远端**——私钥经 `cargo run -p pm-application --example seal_ssh_key`（薄封装平台 `seal_text`）装进平台密钥库，不在 TS 侧重实现加密。

### 5.3 P0 发布验收一键（**已落地**）

```bash
bash test-case/evidence/p0/run-p0.sh
```

载体：`ai-frontend/playwright.p0.config.ts`（vite `:8850`，`VITE_USE_MOCK=0`）+ `ai-frontend/e2e/p0/global-setup.ts`（一次性库 `aipm_p0_e2e` + 本地 JWKS + 真 pm-api `:18490` + **真 ai-pm-worker** + mock LLM + fake-codex 执行脚本）。与 team-qa（:8850/:18490 系）端口互斥，串行复用。

**脚本行为**：① 启 QA PG 容器（127.0.0.1，禁 localhost）→ ② 构建 `pm-api` + `ai-pm-worker` + `submit_run_result` example（rust-lld 需显式 `LIB`/`INCLUDE`）→ ③ `playwright test -c playwright.p0.config.ts` → ④ 证据落 `test-case/evidence/p0/`（run/e2e/pm-api 日志 + HTML 报告 + results.json）。

**实测**：41 collected / 38 PASS / 3 NOT_IMPLEMENTED，全量 1.9 min。

### 5.4 与既有 E2E 的关系

| 套件 | 范围 | 载体 | 入口 |
|---|---|---|---|
| `apps/pm-api/tests/e2e_flow.rs`（新增） | **全流程主链 + 真实 Git 远端** | Rust 集成（真 router + 真 PG + 真 SSH） | `bash test-case/evidence/e2e-flow/run-e2e-flow.sh` |
| `e2e/p0-0{1..7}-*.spec.ts`（新增） | **P0 发布验收 阶段 A~D（41 例）** | Playwright（真 pm-api + 真 worker + 真 vite） | `bash test-case/evidence/p0/run-p0.sh` |
| `e2e/team-onboarding.spec.ts` | Team Onboarding 20 例 | Playwright（真 pm-api + 真 vite） | `bash test-case/evidence/team-qa/run-team-qa.sh` |
| `e2e/s02-ui.spec.ts` | 已整体 skip（fixtures 演示态退役） | — | — |
| `e2e/s02-ui-live.spec.ts` | S02 真栈 UI | Playwright | `pnpm exec playwright test -c playwright.config.ts` |

> **vitest 不得收 e2e/**：`ai-frontend/vitest.config.ts` 已显式 `exclude: ["e2e/**"]`——否则 vitest 会把 Playwright 套件当单元测试收进来，collect 阶段假红（DEF-E2E-03）。

---

## 6. 判定口径

| 判定 | 含义 |
|---|---|
| **PASS** | HTTP 码 + 响应字段 + DB 终态 + （L3）UI 状态，四者一致符合设计 |
| **FAIL** | 与设计口径或产品裁决不符 → 登记缺陷（现象 / 复现 / 怀疑点） |
| **PENDING-DECISION** | 设计稿与实现不一致但属产品口径未定 → 单列待裁，不计缺陷 |
| **NOT IMPLEMENTED** | V1 裁决明确不做（如 §6 缺口 G1–G4） |
| **BLOCKED** | 环境原因无法执行 → 写明原因（如 GitLab 侧不可达、私钥缺失） |

**禁则**
1. 未执行不得标 PASS。
2. HTTP 200 不作唯一证据（团队 QA 已踩过：登录 200 但 DB 无对应行）。
3. 私有key 明文不得进日志 / 证据 / 提交。

### 6.1 已知覆盖缺口（登记，不静默）

| # | 缺口 | 状态 |
|---|---|---|
| ~~G-E2E-1~~ | ~~L3 UI E2E 未落地~~ | ✅ **已关闭（2026-11-10）**：`e2e/flow.spec.ts` 8/8 PASS，见执行报告 §1.0 |
| **G-E2E-2** | **GitHub 侧导入未覆盖**——GitHub 完整 bare clone 偶发 `early EOF`（`fetch-pack: unexpected disconnect`），平台 `clone_bare` 无重试；FLOW-02 当前只覆盖 GitHub 的 verify（`ls-remote`）路径 | 已登记（OBS-01） |
| **G-E2E-3** | **执行器真实执行未覆盖**——`AI_PM_RUN_EXECUTOR` 默认 noop；Run → 真实 commit → 交付态属 L4 范畴 | 待 L4 |
| **G-E2E-4** | 提案 `item_count` 在 `GET /imports/{id}/proposals` 恒为 0（`existing_proposals` 不回填） | 已登记（OBS-02），纯展示瑕疵 |

---

## 7. 缺陷分级与发布门

沿用 `full-lifecycle-test-plan-v1.md` §5：**P0 禁止发布；P1 禁止发布（除非风险裁决）；P2/P3 登记跟进**。

E2E 专项附加门：
- **FLOW-01/02/04/05 任一 FAIL** → 仓库接入与冷启动导入链视为不可用 → **禁止发布**（S01 是全部后续阶段的前置）。
- **FLOW-16 FAIL** → **P0 安全**，立即禁止发布。

---

## 8. 证据落盘

```
test-case/evidence/e2e-flow/
├── run-e2e-flow.sh              一键入口
├── run-<stamp>.log              总日志
├── pm-api-<stamp>.log           pm-api stderr/stdout
├── ai-pm-worker-<stamp>.log     worker 日志
├── flow-results.json            每条用例的断言摘要
├── db-state-<stamp>.json        关键表终态快照
└── playwright/                  L3 报告 + 截图/trace
```

每条 PASS 必须能指到：**用例 ID → 证据文件 → 关键断言值**。
