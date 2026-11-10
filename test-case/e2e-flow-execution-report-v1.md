# E2E 全流程 · 执行报告 V1.0

> **用例文件**：`apps/pm-api/tests/e2e_flow.rs`（真 router + 真 PG + 真 Git 远端）
> **计划**：`test-case/e2e-test-plan-v1.md`（FLOW-01 ~ FLOW-16）
> **执行日期**：2026-11-10
> **基线**：master @ `2b3f509e` + 本批修复（见 §3）
> **执行入口**：`bash test-case/evidence/e2e-flow/run-e2e-flow.sh`
> **判定口径**：HTTP 码 + 响应字段 + **DB 终态** 三者一致才算 PASS；未执行不算 PASS。

---

## 1. 执行结果总览

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

### 1.1 全量回归（同一基线）

| 层 | 命令 | 结果 |
|---|---|---|
| Rust 全仓 | `cargo nextest run --workspace --no-fail-fast` | **1464 passed / 0 failed / 4 skipped**（207 s） |
| Rust doctest | `cargo test --workspace --doc` | 全绿 |
| 前端单测 | `pnpm exec vitest run` | **184 files / 1052 tests passed**（50 s） |
| clippy | `bash scripts/clippy.sh` | 6 处**存量** `type_complexity`（design.rs / task_plan.rs / task_read.rs），与本批无关（stash 基线对比确认） |
| rustfmt | `cargo fmt --all -- --check` | 仅 `run_context.rs` 一处**存量**差异，本批文件全部合规 |

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

### 3.1 本批修复未涉及（登记但不擅自改）

| # | 观察 | 说明 |
|---|---|---|
| OBS-01 | GitHub 完整 bare clone 偶发 `early EOF` | 平台 `clone_bare` 对 GitHub 远端存在**重试缺失**；`ls-remote` 正常，故 FLOW-01 不受影响。是否在 `git_access::clone_bare` 加重试，留待裁决 |
| OBS-02 | 提案 `item_count` 在 `GET /imports/{id}/proposals` 恒为 0 | `existing_proposals` 不回填 `item_count`（`advance.rs:130`）；`apply` 响应里有值。纯展示瑕疵，未改 |

---

## 4. 未覆盖项（诚实登记）

| # | 计划中的用例 | 状态 | 原因 |
|---|---|---|---|
| 1 | **L3 UI E2E**（UIFLOW-01 ~ 08，Playwright + 真浏览器） | **NOT IMPLEMENTED** | 本批先落 L2（服务级，真 router + 真 PG + 真 Git）。UI 层复用既有 `e2e/team-onboarding.spec.ts` 基建，尚未接入本批的真实仓库链路 |
| 2 | FLOW-02 的**导入**路径（GitHub 侧） | BLOCKED | GitHub 完整 bare clone 偶发 `early EOF`（OBS-01）；FLOW-02 当前只覆盖 GitHub 的 **verify**（`ls-remote`）路径，导入走 GitLab |
| 3 | L4 两机实测（T-090 / T-091） | 未执行 | 需 192.168.2.10 / .43 两台内网机器，本批未涉及 |
| 4 | 执行器真实执行（Run → 交付 commit） | 未覆盖 | `AI_PM_RUN_EXECUTOR` 默认 noop；需真实 Codex / Z-Code 执行端，属 L4 范畴 |

---

## 5. 复跑方式

```bash
# 全量（含 SSH 前置检查）
bash test-case/evidence/e2e-flow/run-e2e-flow.sh

# 仅跳过 SSH 前置（仅本地排障）
bash test-case/evidence/e2e-flow/run-e2e-flow.sh --skip-ssh-check

# 直接跑（需 DATABASE_URL 指向可 CREATE DATABASE 的 PG）
DATABASE_URL='postgres://aipm:aipm@127.0.0.1:16433/aipm' \
  cargo nextest run -p pm-api --test e2e_flow --no-fail-fast
```

**证据落盘**：`test-case/evidence/e2e-flow/e2e-nextest-<stamp>.log`（不入库，已加 `.gitignore`）。
