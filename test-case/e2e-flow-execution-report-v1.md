# E2E 全流程 · 执行报告 V1.1

> **用例文件**：L2=`apps/pm-api/tests/e2e_flow.rs`（真 router + 真 PG + 真 Git 远端）；L3=`ai-frontend/e2e/flow.spec.ts`（Playwright + 真 vite + 真 pm-api，2026-11-10 第二批落地）；**P0 发布验收**=`ai-frontend/e2e/p0-0{1..7}-*.spec.ts`（Playwright + 真 vite + 真 pm-api + **真 ai-pm-worker**，2026-11-11 第三批落地，§1.2/§1.4）
> **计划**：`test-case/e2e-test-plan-v1.md`（FLOW-01 ~ FLOW-16 + P0 阶段 A~D）
> **执行日期**：2026-11-10（L2/L3）· 2026-11-11（P0 阶段 B~D）
> **基线**：`feat/e2e-flow-suite` @ `684bc418` + 本批修复（见 §3）
> **执行入口**：`bash test-case/evidence/e2e-flow/run-e2e-flow.sh`（L2/L3）· `bash test-case/evidence/p0/run-p0.sh`（P0 全量）
> **判定口径**：HTTP 码 + 响应字段 + **DB 终态** 三者一致才算 PASS；未执行不算 PASS；NOT_IMPLEMENTED 单列（不计 PASS）。

---

## 1. 执行结果总览

### 1.0 L3 UI E2E · UIFLOW-01 ~ 08（`ai-frontend/e2e/flow.spec.ts` + `playwright.flow.config.ts`）

**8/8 PASS**（两次复跑全绿，34.7s / 35.7s；Playwright + MS Edge headless 1440×900 zh-CN + 真 vite `:8860`（`VITE_USE_MOCK=0`）+ 真 pm-api `:18495`（jwks）+ 一次性库）

| 用例 | 场景 | 结果 | 关键断言（实测值） |
|---|---|---|---|
| **UIFLOW-01** | 未建团 → 登录中转页分流 | **PASS** | `/auth/signing-in` → `/onboarding`；`/me.onboarding_required=true`、`team=null`；DB：租户内 ACTIVE 成员数 = 0（不产生虚假默认团队）。深链 `/projects` **不被拦**（现状记录，见 §3.1 OBS-03） |
| **UIFLOW-02** | UI 建团 → 邀请页横幅 | **PASS** | `onboarding-choice-create` → 填表 → `/onboarding/invite`；横幅含团队名；API：`onboarding_required=false`、`tenant_role=owner`、`team.name` 一致 |
| **UIFLOW-03** | UI 建项目 → 总控台；列表可见 | **PASS** | 表单提交 → `/project/command-center?project_id=<uuid>`；`/projects` 列表显示项目名；DB：`core.project_member` 恰 5 行（owner/ai_pm/architect/developer/tester） |
| **UIFLOW-04** | 需求页读真 | **PASS** | API 造需求（201）→ `/project/requirements?project_id=` 渲染标题；API 列表确认 `latest_state=draft` |
| **UIFLOW-05** | 任务页读真 | **PASS** | API 造 2 条 draft 任务（201）→ 总览 `ov-stat-draft` 徽标 = 2；backlog 两标题真实渲染 |
| **UIFLOW-06** | 仓库设置页读真（**真实远端**） | **PASS** | 平台密钥库注入本机私钥（走平台 `seal_text` 路径）→ GitLab + GitHub 两仓库 API verify 均 `verified`（HEAD sha 40 位）→ UI 两行各带「已验证」徽标 |
| **UIFLOW-07** | 路由守卫 | **PASS** | 裸 context 无 token 访问 `/project/requirements` → `/auth/login`（修复环境泄漏后稳定落点，见 DEF-E2E-04） |
| **UIFLOW-08** | 零 mock 校验（网络面板） | **PASS** | 页面 `/api/**` 请求全部 2xx、零 `fixtures/demo/mock` 路径；DB：展示的需求行确在 `delivery.requirement`（假绿排除） |

> 用例按真实用户旅程**串行**（同一 owner 租户走完全链）；串行的原因：UIFLOW-02 建的团是 03~08 的前置，03 建的项目是 04~06、08 的前置——前置链路断了，跑后面只会产出误导性失败。

### 1.1 L2 服务级 E2E · FLOW-00 ~ 09（`apps/pm-api/tests/e2e_flow.rs`）

**7/7 PASS**（nextest；单次全量 19.7 s）

| 用例 | 场景 | 结果 | 关键断言（实测值） |
|---|---|---|---|
| `flow_00_url_hardening_rejects_non_ssh` | URL 硬化门 | **PASS** | `https://` / `file://` / `ext::` 三种非 SSH 地址均 400 `invalid repository url` |
| `flow_01_precondition_real_remotes_reachable` | 真实远端前置 | **PASS** | 平台 `git_access::ls_remote` 对 GitLab / GitHub 各返回 ≥1 分支，sha 长度 40 |
| `flow_01_verify_real_remotes` | 接真实仓库 + verify | **PASS** | GitLab 与 GitHub 均 `status=verified`、`error_code=null`、HEAD sha 40 位；DB `status='verified'`、`head_commit_sha` 与响应一致；**无密钥时** `error_code='ssh_key_missing'` |
| `flow_04_cold_start_import_from_real_repo` | 真实仓库冷启动导入 | **PASS** | 真 `clone --bare` + 真事实采集 → `draft_ready`；提案前 `delivery.{requirement,milestone,task}` **零写入**；整批审批后**草案里的 300 条任务全部落地**（与提案 `item_count` 一致）、承接人全部解析、导入 `applied`、审计 `project_import_applied` × 1 |
| `flow_06_requirement_design_plan_task_flow` | 需求→设计→计划→任务状态机 | **PASS** | 需求 `draft→pending_confirmation→approved`；哈希不符 409 且终态不变；设计 `draft→approved`；计划 `draft→in_review→approved` → 物化 2 任务（`draft`）→ `mark-ready` → `ready`；非法流转 409 |
| `flow_14_cross_tenant_isolation` | 跨租户隔离 | **PASS** | 非成员主体对本项目需求列表可见数为 0 |
| `flow_15_archive_and_no_key_leak` | 归档 + 私钥零泄露 | **PASS** | 归档 → `status=archived`；重复归档 409 `ALREADY_ARCHIVED`；`POST /ssh-keys` 响应**无** `private_key` 字段 |

### 1.2 P0 发布验收 · 阶段 A（P0-001a/002/003/004/005/006/007）

**7/7 PASS**（两次复跑全绿，35.0s / 29.3s；Playwright + 真 vite `:8850`（`VITE_USE_MOCK=0`）+ 真 pm-api `:18490`（jwks）+ 一次性库；载体 `ai-frontend/e2e/p0-*.spec.ts` + `playwright.p0.config.ts`，规格 11/12 号文档的落地）

| 用例 | 场景 | 结果 | 关键断言（实测值） |
|---|---|---|---|
| **P0-001a** | 会话建立与身份正确 | **PASS** | 中转页分流 `/onboarding`；localStorage 四件套；`/me` `onboarding_required=true`；DB：provision 主体行 `is_active=f` |
| **P0-002** | 首登无 Team | **PASS** | 双入口 testid 可见；无 Skip/Later；深链 `/projects` 现状=不被拦（R4）；API `team=null`；DB **ACTIVE 成员=0 + `tenant.slug IS NULL`**（R5 口径） |
| **P0-003** | 建团→邀请→接受全链 | **PASS** | UI 建团→横幅；邀请 **201**；受邀者 UI `accept-confirm` 接受；`/me` 入团；DB 恰 `member,owner` 两个 ACTIVE 成员。**R65 修正**：受邀者与 owner 同 tid（跨 tid 被服务端硬拒） |
| **P0-004** | UI 建项目归属 Team | **PASS** | 列表页入口→表单→CC `?project_id=`；列表按 code 命中（R58：无单体 GET 路由）；DB `tenant_id` 归属 + 恰 5 人编制 |
| **P0-005** | 零外部依赖进 CC | **PASS** | `项目初始化` 文本可见；无阻断提示；CC 200 + items 非空（**R68：`blocks.initialization_progress.data.items`**）；DB `ai.project_ai_config` = `true/ai_lead`、仓库 0 行 |
| **P0-006** | 初始化 7 事项 7 态派生一致 | **PASS** | items ≥7 且 key/status 全在已知集合；`project_basics` = `completed` + `source_state='planning'`；刷新不丢 |
| **P0-007** | 同/跨 Team 隔离 | **PASS** | A1 需求对 A2/B1 列表不可见；**带 `project_id` 过滤的集合读非成员=404**（R68：比规格 R19 更强）；不带过滤全列表不含他项目数据；单体读 404；跨项目写 404 且修订行数不变、标题未动 |

**落地过程中修正的规格断言（R65~R68，编号续接全局序列避让 18 号文档 R57~R64；已回写规格 11/12 §0）**：跨 tid 接受邀请被硬拒；无单项目 GET 路由；需求创建响应 `{requirement}` 包裹 + DraftRevision 必填 `markdown`；CC 响应 `Block<T>` 包装、过滤集合读非成员=404（隔离比规格声称更强）。

一键复跑：`bash test-case/evidence/p0/run-p0.sh`（证据落 `test-case/evidence/p0/`）。

### 1.4 P0 发布验收 · 阶段 B~D（P0-008a~014 / 015~021+040~048 / 049a~056）

**41 collected / 38 PASS / 3 NOT_IMPLEMENTED（045b、051、052）**（全量 1.9 min；Playwright + 真 vite `:8850`（`VITE_USE_MOCK=0`）+ 真 pm-api `:18490`（jwks）+ **真 ai-pm-worker**（D11 进栈：`AI_PM_RUN_DISPATCH=on` / `AI_PM_RUN_EXECUTOR=mock`）+ mock LLM（OpenAI 兼容，确定性响应）+ 一次性库 `aipm_p0_e2e`；载体 `ai-frontend/e2e/p0-0{3,4,5,6,7}-*.spec.ts` + `ai-frontend/e2e/p0/{fixtures,helpers,global-setup}.ts`）

**阶段 B · Project AI-PM 核心 + Requirement/Knowledge 三层权威（8/8 PASS）**

| 用例 | 场景 | 结果 | 关键断言（实测值） |
|---|---|---|---|
| **P0-008a** | 无外部工具的会话存储 | **PASS** | 会话与项目关联；user 消息落库；刷新不丢 |
| **P0-008b** | 真实回复链（worker + provider） | **PASS** | 注册 mock provider（enable 乐观锁 409 重试）→ worker 回复链 → assistant 消息出现 |
| **P0-009** | 分析链 | **PASS** | `POST /api/v1/events` → `ai.analysis_run.status='completed'`（**R93**：不是 `succeeded`）；结构化结果含 proposals；`ai.change_proposal` + `governance.approval_request` 各 ≥1；**零自动落地**（`delivery.requirement` = 0） |
| **P0-010** | 人工确认门 | **PASS** | AI 主体直调 confirm → 403（`human principal`）；api_key 通道 → 403；两次拒绝后 `current_revision_id` 不变；人工确认 → v1 `superseded` / v2 `approved`；审计含 confirm |
| **P0-011** | 三存储关联 | **PASS** | create 只落 PG draft（**R27**：文件与 commit 诞生于 submit）；submit+confirm 后：文件含正文、Git commit 含正文、`rv.markdown_version_ref` = 该 commit、content_hash 命中、投影行 ≥1 |
| **P0-012** | 修订 + 陈旧哈希门 | **PASS** | Rev3 诞生 → 旧版 `superseded`、新版权威生效；持旧哈希 confirm → 409 且 `current_revision_id` 不变 |
| **P0-013** | 知识域红线 | **PASS** | 红线提案 → `redline_pending`；**Git HEAD 不动**、本文档 published 修订 = 0；AI 身份确认 → 403、api_key 确认 → 403；人工确认 → published 修订 = 1 且 HEAD 推进 |
| **P0-014** | PageIndex 可删可重建 | **PASS** | 删 `ai.document_section_index`（按 `document_revision_id` 关联）→ 权威三面不动、索引行 = 0；`POST /v1/pm/documents/revisions/{id}/reindex`（body 必带 `project_id`）→ 索引行 > 0、修订行数不变、检索命中 |

**阶段 C · Task 执行与交付 + 外部 Agent/MCP（18 例：17 PASS + 1 NOT_IMPLEMENTED）**

| 用例 | 场景 | 结果 | 关键断言（实测值） |
|---|---|---|---|
| **P0-015** | 计划物化 + 跨项目隔离 | **PASS** | task(draft) 归属正确；跨项目主体读写均 404，标题未动 |
| **P0-016** | Run 生命周期 | **PASS** | 承接人必须是 `dev_agent`（**R97**：真人承接 → `no_executor_assignee` 退避）；mark-ready → queued→claimed→running→succeeded（attempt_no=1）；重复 mark-ready → `skipped`、活跃 Run = 0；lease 明文只在执行端环境变量 |
| **P0-016B** | 执行失败链 | **PASS** | 不可 clone 的仓库 → `executor_start_failed` failure 行 + 退避重排队（基线不可解析时判 `baseline_unavailable`，故夹具必须给 `head_commit_sha`） |
| **P0-017** | 事件/命令面 | **PASS** | `/v1/pm/task-runs/{id}/events` 含 `run_finished`；commands 读面 200；审计 `run_finished` 落账 |
| **P0-018** | 交付三态 | **PASS** | 未交付任务 → `never_submitted`；**租约只在 claimed/running 有效**（R99：Run 收尾后提交 → `lease_invalid`）→ 交付必须在 Run 活跃期提交：accepted → `delivered` + manifest commit 行 = 1 + 任务 `submitted` |
| **P0-019** | Review 审核 | **PASS** | 交付 → review 审批面（列表端点**必须带 `project_id`**）；人工 approved → 任务 `completed`；**重复裁决 → 409**（DEF-P0-04）；api_key 裁决 → 403 |
| **P0-020** | 多仓交付 | **PASS** | 双仓提交 → manifest 2 行 + `runs/{id}/commits` 双向可读 + git 直验 sha 存在；申报 `changed_files` 与 git 实测不符 → `commit_not_descendant` 拒绝（平台自带一致性校验） |
| **P0-021** | 取消 + 迟到提交 | **PASS** | running 中 cancel → `cancelled`；终态 Run 的迟到交付 → `lease_invalid` 拒绝、commit 行 = 0；`/waiting`、`/agents` 读面 200 |
| **P0-040** | MCP 连接认证 | **PASS** | `PM_MCP_READ` 令牌可用、5 工具固定集可见；无 token / 无权限 → 401/403 |
| **P0-041** | 设备绑定 | **PASS** | **实现口径（R77）**：MCP 读面授权 = **项目成员资格**（`ensure_member`），`X-AI-PM-Device-Id` 只做归因 → 跨项目（同一真人成员）放行、非成员主体拒绝；设备绑定的强制点在**派发侧**（D-2=c）：绑定 runner 设备的项目 → Run 恒 `queued`（10 s 观察窗内 `dispatch_attempts=0`） |
| **P0-042** | /install + Skills 包 | **PASS** | `/install?format=json` 目录含 `codex`；`/install/codex?format=json` 的 bootstrap 段含包地址 + manifest + 4 端点；manifest `sha256` == 实包字节 sha256（**R101**：manifest 是整包 sha256 + `contents` 清单，不是 per-file 列表）；4 个 `SKILL.md`（四件套） |
| **P0-043** | ready 任务可见 | **PASS** | mark-ready 后任务经 MCP 可见可读 |
| **P0-044** | 知识读工具缺口 | **PASS** | 固定 5 工具集不含知识读工具（通道缺口如实呈现）；正式知识经 documents REST 可得 |
| **P0-045a** | 内部编排链 | **PASS** | 本项目自证：绑仓 → mark-ready → 执行 → 交付提交 → 收尾；Run `succeeded` + `run_finished` 事件与终态同源 |
| **P0-045b** | MCP 回传通道 | **NOT_IMPLEMENTED** | 执行端无 MCP 凭据（D4 未开通）——回传走内部编排链（045a 已证）。不计 PASS |
| **P0-046** | commit 归属 | **PASS** | manifest 行数与 `runs/{id}/commits` 读面一致 |
| **P0-047** | 越权与注入 | **PASS** | 伪造 task_ref → 工具错误；`PM_MCP_READ` 调写工具 → 权限错误；注入文本经**提案面**（`pm_propose_document` + 红线）→ `redline_pending`、**零 published 修订**、注入文本原样落库（当数据不当指令） |
| **P0-048** | 幂等与 fencing | **PASS** | TaskReady 重放不重复建 Run；终态 Run 凭据已清（`lease_owner`/`lease_token_hash` NULL）不复活 |

**阶段 D · 知识学习与治理（8 例：7 PASS + 2 NOT_IMPLEMENTED）**

| 用例 | 场景 | 结果 | 关键断言（实测值） |
|---|---|---|---|
| **P0-049a** | 分析链可追溯 | **PASS** | events API → analysis `completed`；`change_proposal.analysis_run_id` 非空（可追溯）；同 `idempotency_key` 重放 → 同 `event_id`、事件表行数不变；AI 主体零 published 修订 |
| **P0-050** | 来源与可信度 | **PASS** | `kb.memory_ref`（`ref_role`/`target_domain`/`target_type`/`revision_missing_reason`）+ `kb.source`（`source_kind`/`content_hash`）+ 引用缺失必声明原因的 CHECK 约束 |
| **P0-051** | 外部 Commit 自动发现 | **NOT_IMPLEMENTED** | 无发现管道（表名反证）——归属验证语义由 P0-020 覆盖。不计 PASS |
| **P0-052** | 澄清与采纳闭环 | **NOT_IMPLEMENTED** | 无专用管道；澄清语义载体 `delivery.task_interaction` 的链式列（`event_id`/`previous_chain_id`/`continuation_generation`/`chain_open`）可观测。不计 PASS |
| **P0-053** | 统一协调 | **PASS** | 同文档两提案**串行**落账（p1→rev1、p2→rev2）；p3 声明过期 `base_revision_id` → **409**；正式修订恰 2 条且 head = p2 的哈希（无静默覆盖）；幂等账本表存在 |
| **P0-054** | 质量门禁 | **PASS** | 无来源锚的低质提案 → `rejected`（`rejection_reason` 含规则码）；被拒提案确认 → 409 |
| **P0-055** | Revision ↔ Git 一致性 | **PASS** | published 修订恒有 `git_commit_sha`（0 违例） |
| **P0-056** | 恢复面 | **PASS** | 幂等账本表存在；`reindex_required` 的真实载体是**质量门结果 JSON**（`ai.document_proposal.quality_gate_result`，R102：不是独立列） |

> **P0 栈的三个"确定性替身"**（均为真链路内的受控端点，非 mock 后端）：① mock LLM（OpenAI 兼容 `/chat/completions`）——provider 指向它，回复链与分析链都走真 HTTP；② fake-codex 执行脚本（`AI_PM_CODEX_BIN`）——真 spawn、真 worktree、真 commit，**只把"执行内容"换成确定性脚本**；③ 本地 `file://` 仓库夹具——真 clone / 真镜像 / 真 worktree，差别只在远端地址。

### 1.5 全量回归（同一基线）

| 层 | 命令 | 结果 |
|---|---|---|
| Rust 全仓 | `cargo nextest run --workspace --no-fail-fast` | **1464 passed / 0 failed / 4 skipped**（791.6 s，末次全绿） |
| Rust L2 E2E（含真远端） | `cargo nextest run -p pm-api --test e2e_flow` | **7/7 PASS**（真 GitLab 192.168.2.16 + GitHub 两个远端） |
| P0 全量 | `bash test-case/evidence/p0/run-p0.sh` | **41 collected / 38 PASS / 3 NOT_IMPLEMENTED**，连跑两次 ALL GREEN（1.7 min/次） |
| Rust doctest | `cargo test --workspace --doc` | 全绿 |
| 前端单测 | `pnpm exec vitest run` | **184 files / 1052 tests passed**（58.7 s） |
| clippy | `bash scripts/clippy.sh` | 6 处**存量** `type_complexity`（design.rs / task_plan.rs / task_read.rs），与本批无关（stash 基线对比确认） |
| rustfmt | `cargo fmt --all -- --check` | 仅 `run_context.rs` 一处**存量**差异；本批文件（含新 example）全部合规 |

> **环境事实（新登记）**：① QA PG 容器默认 `max_locks_per_transaction=64`，全量 nextest 并行建库时锁表耗尽 → 批量 `53200 out of shared memory`（**看着像代码坏了，其实是环境**）；修法：`ALTER SYSTEM SET max_locks_per_transaction = 512;` + **重启容器**（该参数 postmaster 级，`pg_reload_conf()` 无效）。② 内网 GitLab `192.168.2.16` 随 VPN 隧道时通时不通——不通时 L2 真远端 3 例报「git authentication failed」（实为 22 端口 timeout，GitHub 同刻仍通可区分）。两者均非本批代码问题。

---

## 2. 真实仓库与私钥接入（本次新增能力）

### 2.1 私钥通路（三层）

| 层 | 机制 | 落点 |
|---|---|---|
| ① Git 客户端 | `~/.ssh/config`：`Host github.com → ssh.github.com:443`（GitHub 禁 22 端口）+ `Host 192.168.2.16`；均 `IdentityFile ~/.ssh/id_rsa`、`IdentitiesOnly yes` | 开发者手工 `git clone` / `ls-remote` |
| ② 平台密钥库 | `integration.project_ssh_key.private_key_ciphertext`（AES-256-GCM 信封，AAD 绑定 project_id） | `crates/pm-application/src/secret.rs:47` |
| ③ 运行时注入 | `open_ssh_private_key` → `write_temp_key`（临时目录，用后即删）→ `GIT_SSH_COMMAND` 带 `IdentitiesOnly=yes` / `BatchMode=yes` / `StrictHostKeyChecking=accept-new` 与独立 `UserKnownHostsFile` | `crates/pm-application/src/git_access.rs:83` |

### 2.2 E2E 如何用上真实私钥

平台**不**读家目录密钥。E2E 把本机私钥按平台同口径加密后**装进平台自己的密钥库**：

```
读 ~/.ssh/id_rsa（明文，仅进程内存）
  → seal_text(project_id, 明文)         ← 平台同一条加密路径
  → UPDATE integration.project_ssh_key SET private_key_ciphertext = <密文>
  → POST /v1/pm/projects/{pid}/repositories { url: "git@192.168.2.16:tooling/ai-pm.git" }
  → POST /v1/pm/repositories/{rid}/verify  ← 平台用注入的私钥跑真 git ls-remote
```

因此 FLOW-01/04 验证的是**平台自己的解密 + 临时密钥 + Git 协议栈**，不是测试代码另起的捷径。

> **L3（Node 侧）同路**：UIFLOW-06 不能在 TS 里重实现 AES-256-GCM 信封（两套实现漂移 = 假绿/假红，见 `docs/e2e-spec/00` §7.2 红线）。`e2e/flow/stack.ts` 的 `sealPrivateKey()` 调 `cargo run -p pm-application --example seal_ssh_key`（薄封装平台 `seal_text`，输出 hex），再经 psql `decode(hex)` 落库——加密永远只有平台一份实现。

### 2.3 实测前置事实

| 检查 | 结果 |
|---|---|
| 私钥指纹 | `SHA256:AcVtnh+QINUUOenISahwol/UYDqEHz+Y5RZS3KmilqM`（ED25519，`luqizheng@126.com`） |
| `git ls-remote` GitLab | ✅ 5 个分支（`aipm/6f4ec3d4` 等） |
| `git ls-remote` GitHub | ✅ 2 个分支（`main`、`wip/ui-theme-overlays`） |
| GitLab **完整 bare clone** | ✅ **9.3 s / 15 MB**（844 commits / 2173 文件） |
| GitHub 完整 bare clone | ⚠️ 实测 `early EOF`（`fetch-pack: unexpected disconnect`）——`ls-remote` 正常，clone 需重试 |
| 平台同款 `GIT_SSH_COMMAND`（原生 Windows 临时路径） | ✅ 两个仓库均通过（OpenSSH 不校验 Windows 上的 POSIX 权限位） |

> **已知事实（非缺陷）**：`git clone --depth 1` 对 GitLab 侧报 `early EOF`（服务端浅克隆协议问题），但**完整 `--bare` 克隆成功**——而平台走的正是 `clone --bare`，故不阻塞。

---

## 3. 本批修复的缺陷

### DEF-E2E-01（P1，已修复）：冷启动导入在 >200 条 commit 的真实仓库上必然失败

| 项 | 内容 |
|---|---|
| **现象** | `POST /v1/pm/approvals/{id}/decide` 返回 409 `{"message":"command execution failed"}`；导入永久停在 `proposed`，需求/里程碑/任务一条都落不下去 |
| **复现** | 对 `git@192.168.2.16:tooling/ai-pm.git`（844 commits / 402 篇 doc 候选）走冷启动导入 → 草案生成 300 条任务 + 24 条需求 → 审批第一条即 409 |
| **根因** | 执行器对单条批量命令有 `MAX_BATCH_ITEMS = 200` 守卫（`crates/pm-application/src/command_executor/guards.rs:35`）；而导入层 `propose_import` 把**整份选中集**塞进**一条** `CreateTasks` 命令（`project_import/proposal.rs`），从不拆批。真实仓库的 commit/doc 数量轻易超过 200 → 整条命令被守卫拒绝 → 审批链 409 |
| **影响面** | **任何** commit 数 > 200 的仓库都无法完成冷启动导入。存量 `imports_e2e.rs` 用本地 2-commit 夹具，永远触不到这条线 |
| **修复** | `propose_import` 按 `MAX_BATCH_ITEMS` 把需求 / 任务 / 文档三类**按批拆分**成多条命令（spec §16「批量命令的 payload 体积上限（建议单批 ≤200 项，**超出拆批并提示**），实现时按提示预算定值」）；`MAX_BATCH_ITEMS` 由 `pub(crate)` 提升为 `pub` 并在 crate root 导出，避免调用方各写一份魔数 |
| **验证** | FLOW-04 转绿：草案 300 条任务被拆成 `200 + 100` 两条提案，全部审批通过后需求/里程碑/任务落地、导入 `applied` |
| **回归** | 全仓 1463 例全绿（含 `imports.rs` / `imports_e2e.rs` / `import_contract.rs`） |

### DEF-E2E-02（P2，已修复）：草案标题重复 → 任务落地数少于提案宣称数

| 项 | 内容 |
|---|---|
| **现象** | 导入 `draft_ready` 后草案含 300 条任务，审批全部通过后 `delivery.task` 只有 **293** 行；提案 `item_count` 说 300，实际落 293，**无任何提示** |
| **复现** | 对 `git@192.168.2.16:tooling/ai-pm.git` 冷启动导入 → 断言「每条草案任务都落地」失败（293 ≠ 300） |
| **根因** | 平台以「**项目 + 标题**」为任务身份——执行器 `create_tasks` 遇同标题即视为同一任务而**合并**（`created=false`，`crates/pm-application/src/command_executor/executor.rs:620`）。而 Fake 分析模型用 **commit subject 直接当任务标题**（`crates/pm-ai/src/repo_model.rs:123`）。真实仓库的 commit subject 天然重复——ai-pm 自身 300 条 commit 只有 **293** 个不同 subject（3 个 subject 各出现 2 次），重复项被静默合并 |
| **影响面** | 任何存在重复 commit message 的仓库，导入后任务数与提案数不符且无提示。`OpenAiRepoAnalysisModel` 同样用自然语言标题，**同样会命中** |
| **修复** | 在接地层（`ground_draft`，模型无关的公共收口）按**证据引用**（commit sha 前 7 位 / doc 末段文件名）消歧重复标题，追加后缀保证「一条 commit 一条任务」 |
| **验证** | FLOW-04 转绿（300 条任务全部落地）；新增回归用例 `grounding_disambiguates_duplicate_titles`（`apps/pm-api/tests/imports.rs`）钉住「标题必须唯一」这一不变量 |

### DEF-E2E-03（P3，已修复）：vitest 误收 Playwright e2e 套件导致假红

| 项 | 内容 |
|---|---|
| **现象** | `pnpm exec vitest run` 报 `3 failed`（`e2e/s02-ui.spec.ts` / `e2e/s02-ui-live.spec.ts` / `e2e/team-onboarding.spec.ts`），但 1052 条单元测试全过 |
| **根因** | vitest 默认 glob 把 `e2e/**` 收进来，在 collect 阶段因缺 Playwright 运行时而报错——掩盖真正的单元测试失败 |
| **修复** | `ai-frontend/vitest.config.ts` 显式 `exclude: [...configDefaults.exclude, "e2e/**"]` |
| **验证** | `pnpm exec vitest run` → **184 files / 1052 tests passed，0 failed** |

### DEF-E2E-04（P2·套件环境泄漏，已修复）：UIFLOW-07 登录页被本机 full-dev 网关劫持成注册页

| 项 | 内容 |
|---|---|
| **现象** | UIFLOW-07 期望「无 token 访问受保护页 → `/auth/login`」，实测落在 **`/auth/setup`**（注册页）——同一套件两次运行结果取决于本机碰巧在跑什么 |
| **根因** | vite 的 `/gw` 代理默认打 `LMMS_GATEWAY_URL ?? 127.0.0.1:3000`（`vite.config.ts:47`）。本栈没有 iam 网关，但本机恰好在跑 full-dev 栈（`superpowers-full-api-gateway-1` 占 3000）→ `iam.auth.setupStatus()` 打到了**外部真实网关**，返回 `setup_required=true` → LoginView 重定向 `/auth/setup`（`LoginView.vue:65-67`）。测试结果依赖宿主机上碰巧运行的服务 = 环境泄漏 |
| **修复** | `playwright.flow.config.ts` 的 webServer env 显式 `LMMS_GATEWAY_URL: "http://127.0.0.1:59999"`（死端口）→ `fetchSetupStatus` 走 **fail-open=null**（`api/setup.ts:44-50`「只有确证 setup_required=true 才引导到注册页」）→ 登录页稳定停留 |
| **验证** | UIFLOW-07 转绿；连续两次全量复跑全绿（34.7s / 35.7s） |
| **教训** | E2E 栈里**每一个**对外依赖都必须显式钉死（连「死端口」也是钉法之一）——默认值会静默命中宿主机上的其它服务 |

### 3.1 阶段 B~D 新发现并修复的缺陷

#### DEF-P0-03（P2，已修复）：知识域人工门把「授权拒绝」报成 409 Conflict

| 项 | 内容 |
|---|---|
| **现象** | `POST /v1/pm/documents/proposals/{id}/confirm` 以 `kind='ai_pm'` 主体（或 api_key 通道）调用时返回 **409 KNOWLEDGE_CONFLICT**，而规格与需求域的同款人工门都是 **403** |
| **根因** | 知识域有自己的 `require_human`（`crates/pm-application/src/knowledge_proposal/support.rs:8`），非真人主体被映射成 `KnowledgeError::Conflict` → HTTP 409；需求域的同款判定（`requirement::require_human`）走 `PmWriteError::Forbidden` → HTTP 403。**同一平台内两套人工门语义不一致** |
| **影响面** | 授权拒绝被客户端当成「可重试的状态冲突」；审计与监控按 409 归类会掩盖越权尝试；规格 13/17 号的 403 断言无法成立 |
| **修复** | `KnowledgeError` 新增 `Forbidden(String)`（与 `Conflict` 严格分开）；知识域人工门改抛 `Forbidden`；`pm-api` 映射为 **403 KNOWLEDGE_FORBIDDEN**；`pm-mcp` 的 `map_knowledge_error` 同步映射 `ToolError::Forbidden`；回归用例 `knowledge_v2.rs::non_human_cannot_confirm_a_redline_proposal` 改断言 `Forbidden` |
| **验证** | P0-013 转绿（AI 身份 / api_key 双 403）；P0-054 的被拒提案仍 409（真冲突，语义未混） |

#### DEF-P0-04（P2，已修复）：重复裁决返回 200（裁决其实没生效）

| 项 | 内容 |
|---|---|
| **现象** | 同一条 review 审批二次裁决 → HTTP **200**（响应体 `outcome=already_decided`），而 API 契约（`pm_reviews.rs` 模块头「AlreadyDecided \| 409 \| already_decided」）与错误码枚举都按 409 定义 |
| **根因** | 服务层在「审批行非 pending」时返回 `Ok(ApprovalOutcome::AlreadyDecided{..})`（`review/decide.rs:534`），**没走** `Err(ApprovalError::AlreadyDecided)`——于是 API 层的 409 映射分支是死代码，重复裁决被报成成功 |
| **影响面** | 客户端重试（如裁决请求超时后重发）会读到 200，误以为裁决已生效；实际终态行未被改动 |
| **修复** | 改抛 `Err(ApprovalError::AlreadyDecided(status))` → HTTP 409；删除随之失效的 `ApprovalOutcome::AlreadyDecided` 变体（不留兼容分支） |
| **验证** | P0-019 转绿（人工 approved → 重复 rejected → 409，且终态不变） |

#### DEF-P0-05（P2·flaky，已修复）：依赖图 advisory lock 把 Uuid 绑到 `$1::text` 参数 → 随机 UTF-8 报错

| 项 | 内容 |
|---|---|
| **现象** | Review 人工裁决偶发 **500**：`state transition refused: database error: invalid byte sequence for encoding "UTF8": 0xa9`；任务完成/裁决链路随机失败（12 小时里 1 次，P0 全量 5 次里 1 次） |
| **定位** | Postgres 侧日志给出确切语句：`STATEMENT: SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))`，`CONTEXT: unnamed portal parameter $1` |
| **根因** | 两处共用**同一条 SQL 文本**的 advisory lock：依赖图锁（`task_dependency.rs:97`，绑 `Uuid`）与知识索引锁（`knowledge_indexer.rs:349`，绑 `String`）。`$1::text` 让服务端把参数类型解析为 **text**；sqlx 的**语句缓存按 SQL 文本复用**，同一连接若先按 text 绑定准备过该语句，后续按 uuid 绑定复用缓存语句时，服务端把 uuid 的 **16 字节二进制**当 text 解析 → 随机命中非法 UTF-8 字节 |
| **修复** | 依赖图锁改为**绑字符串**（`project_id.to_string()`）并给 SQL 加 `task-graph:` 前缀（两条语句的缓存键不再相撞）；并发回归用例 `task_dependency_concurrency.rs` 同步对齐（锁键必须与实现同口径） |
| **验证** | `task_dependency_concurrency` 9/9 PASS；P0 全量 41/41 ALL GREEN（复跑两次）；Postgres 日志不再出现该错误 |
| **教训** | **同一 SQL 文本 + 不同参数类型 = sqlx 语句缓存的隐性雷**：参数类型必须与 SQL 的 `$n::type` 显式一致（text 参数就绑字符串），且不同锁域用不同前缀避免缓存键相撞 |

### 3.2 阶段 B~D 的实现事实源偏差（规格与实现不符，按实现为准）

> 裁决口径：**以我们的实现为事实源**。下列偏差是 E2E 实测出来的「规格写法 ≠ 实现行为」，规格文档需按此回写（已在 `docs/e2e-spec/` 就地标注）。

| 编号 | 规格写法 | 实现事实（实测） | 影响 |
|---|---|---|---|
| **R92** | intake 的 `analysis_request` 产生 `ai.analysis_run` | intake 的 analysis_request 路由到 **kb_analyze**（kb stub），**不产生** analysis_run；`ai_analysis` 的真实入口是 `POST /api/v1/events`（`ingestion.rs:103`） | 规格 13 号 R23 需改写 |
| **R93** | 分析终态 = `succeeded` | `ai.analysis_run.status` 终态字面量是 **`completed`**（`succeeded` 是 Run 的词汇） | 断言词统一 |
| **R94** | `model_name` 反映命中的 provider | `ai.analysis_run.model_name` 来自 `ai.project_ai_config`（bootstrap 默认 `'fake'`），provider 命中由 worker 日志 + mock 调用记录证明 | 断言放宽为「如实记录」 |
| **R95** | 缺设备声明 → 拒绝 | `DeviceClaim::absent()` **仍执行**（C176：无归属但放行） | P0-041 断言方向翻转 |
| **R96** | `pm_get_task_context` 参数 `task_id` | 参数名是 **`task_ref`**（`pm-mcp/src/tools.rs:78`） | 用错参数名 → 工具报缺参错误（假红） |
| **R97** | 任务有承接人即可被领取 | 可领取性判定第 4 条要求承接人 `kind='dev_agent'`（`run_dispatch/policy.rs:179`）；真人承接 → Run 被判 `no_executor_assignee` 退避回队 | 物化链必须派给 dev_agent（bootstrap 播种的 developer 主体） |
| **R98** | 基线不可解析 → 仍会执行 | 可领取性判定第 6 条：基线不可解析 → `baseline_unavailable` 退避（永远到不了执行器） | 失败注入夹具必须给 `head_commit_sha` |
| **R99** | 交付提交可在 Run 结束后做 | 租约只在 `status ∈ {claimed, running}` 且 `lease_token_hash` 未清时有效；Run 收尾后提交 → `lease_invalid` | 交付必须在 Run 活跃期提交（E2E 用「释放文件」协议制造窗口） |
| **R100** | 设备未绑定项目 → 工具层拒绝 | MCP 读面授权 = **项目成员资格**（`ensure_member`）；设备头只做归因。设备绑定的强制点在**派发侧**（D-2=c：绑定 runner 设备的项目，worker 候选发现直接排除） | 规格 16 号场景 B 需改写为「派发侧谓词」 |
| **R101** | manifest 含 per-file sha256 列表 | manifest 是**整包 sha256** + `contents` 清单（`skills_package.rs:34`） | 断言按整包哈希 |
| **R102** | `reindex_required` 是索引表列 | 它是**质量门结果 JSON** 的字段（`quality_gate_result->>'reindex_required'`）；`ai.document_index_state` 无此列 | 断言落点改到提案行 |

### 3.3 待裁决项引用（不计缺陷）

| # | 观察 | 说明 |
|---|---|---|
| OBS-03 | onboarding 门只在登录中转页，无路由级守卫 | UIFLOW-01 实测：`/auth/signing-in` 被分流到 `/onboarding`，但深链 `/projects` **不被拦**。与 team-onboarding.spec **B09-18 的 fixme** 同源（「待裁决：onboarding 门只在登录中转页」），按 team-qa 计划 §5 **PENDING-DECISION** 口径记录现状，不计缺陷。若产品裁决补路由级守卫，UIFLOW-01 的深链断言应同步改为「被拦」 |

### 3.4 本批修复未涉及（登记但不擅自改）

| # | 观察 | 说明 |
|---|---|---|
| OBS-01 | GitHub 完整 bare clone 偶发 `early EOF` | 平台 `clone_bare` 对 GitHub 远端存在**重试缺失**；`ls-remote` 正常，故 FLOW-01 不受影响。是否在 `git_access::clone_bare` 加重试，留待裁决 |
| OBS-02 | 提案 `item_count` 在 `GET /imports/{id}/proposals` 恒为 0 | `existing_proposals` 不回填 `item_count`（`advance.rs:130`）；`apply` 响应里有值。纯展示瑕疵，未改 |

---

## 4. 未覆盖项（诚实登记）

| # | 计划中的用例 | 状态 | 原因 |
|---|---|---|---|
| 1 | ~~L3 UI E2E~~ | ✅ **已落地（2026-11-10 第二批）** | §1.0：UIFLOW-01~08 全部 PASS，接入真实仓库链路（UIFLOW-06），一键 `run-e2e-flow-ui.sh` |
| 5 | ~~P0 阶段 B~D~~ | ✅ **已落地（2026-11-11 第三批）** | §1.4：P0-008a~014 / 015~021+040~048 / 049a~056 全部落地，41 collected / 38 PASS / 3 NOT_IMPLEMENTED（045b、051、052）；一键 `run-p0.sh` |
| 2 | FLOW-02 的**导入**路径（GitHub 侧） | BLOCKED | GitHub 完整 bare clone 偶发 `early EOF`（OBS-01）；FLOW-02 当前只覆盖 GitHub 的 **verify**（`ls-remote`）路径，导入走 GitLab |
| 3 | L4 两机实测（T-090 / T-091） | 未执行 | 需 192.168.2.10 / .43 两台内网机器，本批未涉及 |
| 4 | 执行器真实执行（Run → 交付 commit） | 未覆盖 | `AI_PM_RUN_EXECUTOR` 默认 noop；需真实 Codex / Z-Code 执行端，属 L4 范畴 |

---

## 5. 复跑方式

```bash
# ── L2 服务级（FLOW-*）────────────────────────────────────────────
# 全量（含 SSH 前置检查）
bash test-case/evidence/e2e-flow/run-e2e-flow.sh

# 仅跳过 SSH 前置（仅本地排障）
bash test-case/evidence/e2e-flow/run-e2e-flow.sh --skip-ssh-check

# 直接跑（需 DATABASE_URL 指向可 CREATE DATABASE 的 PG）
DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
  cargo nextest run -p pm-api --test e2e_flow --no-fail-fast

# ── L3 UI（UIFLOW-*）──────────────────────────────────────────────
# 一键（自动构建 pm-api + seal_ssh_key → Playwright + 真 vite/pwm-api）
bash test-case/evidence/e2e-flow/run-e2e-flow-ui.sh

# 保留一次性库（调试页面状态）；跳过真实仓库用例（UIFLOW-06 标 skip）
FLOW_E2E_KEEP_DB=1 bash test-case/evidence/e2e-flow/run-e2e-flow-ui.sh
FLOW_E2E_SKIP_SSH=1 bash test-case/evidence/e2e-flow/run-e2e-flow-ui.sh

# 直接跑（需先 export FLOW_E2E_PM_API_BIN=<pm-api 可执行文件>）
cd ai-frontend && pnpm exec playwright test -c playwright.flow.config.ts

# ── P0 发布验收（阶段 A~D，41 例）────────────────────────────────
# 一键（自动构建 pm-api + ai-pm-worker + submit_run_result → Playwright +
#      真 vite / 真 pm-api / 真 worker / mock LLM）
bash test-case/evidence/p0/run-p0.sh

# 保留一次性库（排障用）
TEAM_QA_KEEP_DB=1 bash test-case/evidence/p0/run-p0.sh

# 单套件（需先 export TEAM_QA_PM_API_BIN / P0_WORKER_BIN）
cd ai-frontend && pnpm exec playwright test -c playwright.p0.config.ts e2e/p0-05-task-delivery.spec.ts
```

**证据落盘**（`test-case/evidence/e2e-flow/`）：
- L2：`e2e-nextest-<stamp>.log`（不入库，已加 `.gitignore`）
- L3：`e2e-ui-<stamp>.log`（运行日志，不入库）、`playwright-ui-report/`（HTML 报告）、`playwright-ui-results.json`、`playwright-ui-artifacts/`（失败现场截图 + trace，仅失败时产生）

**证据落盘**（`test-case/evidence/p0/`）：
- `run-<stamp>.log`（编排日志）、`e2e-<stamp>.log`（Playwright 输出）、`pm-api-<stamp>.log`（后端日志）
- `playwright-report/`（HTML 报告）、`playwright-results.json`、`playwright-artifacts/`（失败现场截图 + trace）
