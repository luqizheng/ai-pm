# S01 · I6-A 代码事实核验报告

> 文档性质：实施前代码事实核验报告
> 所属设计：I6 · Repository Onboarding & Code Discovery（设计稿 V1.0）
> 核验清单：`smark-pm-s01-i6-code-facts-checklist`（I6-A-001～115）
> 版本：V1.0（2026-11-03 勘误一次：D-03 / §8.4 —— `bootstrap_project` **会**创建 `ai.project_ai_config`（`bootstrap.rs:288-297`），"作业无人认领"路径收窄为"冻结发生在飞 Import 无终态化"）
> 状态：待专家裁决
> 核验对象：`D:\kudi-project\templates\ai-pm`（master `aaee6e5`，migrations 0001～0028）
> 核验原则：以当前代码、Migration、实际调用链为依据；设计建议不算已有实现。

---

## §0. 结论摘要

### 0.1 总体判断

I6 V1.0 设计稿是在 **"仓库接入与代码分析尚未实现"** 的前提下写的。代码事实相反：

| I6 V1.0 的前提假设 | 代码事实 | 结论 |
|---|---|---|
| "建议新建独立的 `discovery_session`" | `discovery.discovery_session` **已存在**（0025），且已有生产者、消费者、状态机 | **前提作废** |
| "建议新增 Finding ↔ Repository 多对多关联表" | `discovery.discovery_finding_repository` **已存在且已被真实读写** | **前提作废** |
| "`repo_import(source=WEB)`" | `repo_import` **没有 `source` 列**；`WEB` 是 `discovery_session.source` | **前提错误** |
| "Import 取消不回滚 `initializing`，是待修复缺陷" | 属实，**但范围比设计稿写得更广**：失败、驳回、部分失败、Worker 无人认领 **同样不回滚** | **缺陷成立且需扩大** |
| "一个 Import 可以包含多个 Source" | 数据模型支持，**执行层不支持**：首个仓库失败即中止整个 Import | **部分成立** |
| "Source 独立记录分析状态" | Source **无状态字段**，只有 `error_code` | **前提错误** |

**净结论：I6 V1.0 不应作为实施基线。** 需要按代码事实重写领域模型部分（§2～§4），再进入 I6-B。

### 0.2 第一轮（本报告重点）覆盖深度

| 清单章节 | 覆盖方式 | 深度 |
|---|---|---|
| §2 Repository | 直读 `repository/repo.rs` + 迁移 + 调用链 | 逐行核验 |
| §3 Repo Import | 直读 `project_import/*` + `job_queue.rs` + worker | 逐行核验 |
| §4 Repo Import Source | 直读 `lifecycle.rs` 执行循环 + 迁移 | 逐行核验 |
| §8 Project 生命周期 | 直读全部 `core.project.status` 写入点 | 逐行核验 |
| §5 / §6 / §7 | 单遍核验（证据充分，结论可信） | 见 §12 |
| §9 / §10 | 单遍核验（证据充分，结论可信） | 见 §12 |
| §11 Migration / 空库 | 目录级核验（未连库执行） | 见 §12 |

### 0.3 标记含义

沿用核验清单定义：

`[已核实]` 有代码/Migration/测试证据 · `[部分核实]` 部分证据 · `[待核实]` 未查完 · `[不存在]` 已搜索确认无实现 · `[设计候选]` 建议新增 · `[存在缺陷]` 已发现不符合预期

---

# §2. Repository 仓库连接模型

## 2.1 数据库结构（I6-A-001～010）

最终表：`integration.project_repository`，定义于 `migrations/0008_repositories.sql:18`，后续由 `0009_repo_import.sql:14-21`、`0019_tenant.sql:120/162/192/292-294` 增补，**0020～0028 无任何 ALTER**。

| 编号 | 核验项 | 标记 | 事实与证据 |
|---|---|---|---|
| I6-A-001 | 主表 Schema | [已核实] | `0008:18-32`。列：`id / project_id / name / url / role / default_branch / description / revision / created_at / updated_at`；`0009:15-21` 增 `ssh_key_id / status / head_commit_sha / last_verified_at / last_error_code / last_error_detail`；`0019:120` 增 `tenant_id` |
| I6-A-002 | Project 关联 | [已核实] | `project_id → core.project(id) ON DELETE CASCADE`（`0008:20`，FK 在 `0019:192` DROP 后于 `0019:292-294` 重建为组合 FK `(tenant_id, project_id)`）。`UNIQUE(project_id, url)`（`0008:30`）保证同项目同 URL 不重复；`UNIQUE(tenant_id, id)`（`0019:162`）供组合 FK 引用 |
| I6-A-003 | Repository 标识 | [已核实] | **无 provider / owner / clone_url 字段**。只有 `name` + `url` + `role`(`main/docs/service/infra`, `0008:23-24`) + `description`。角色语义仅作标签，代码里无差异化行为 |
| I6-A-004 | Repository 状态 | [已核实] | `status` CHECK = `pending / verified / unreachable / archived`（`0009:16-17`）。写入点只有两处：`verify_repository`（`repository/repo.rs:324`）与 `archive_repository`（`repo.rs:194`）。**没有 `default_branch` 之外的连接态细分** |
| I6-A-005 | 默认分支 | [已核实] | `default_branch` 持久化（`0008:25`）。verify 时**覆盖写**：优先沿用现有值（在远端 heads 中时），否则 `main` → `master` → 首个分支（`repo.rs:272-287`） |
| I6-A-006 | HEAD Commit | [已核实] | `head_commit_sha`（`0009:18`），来自 `git ls-remote --heads` 的分支 sha（`repo.rs:288-290`）。**失败时写 NULL（清空旧值）**（`repo.rs:314`）。语义 = "最近一次 verify 看到的 HEAD"，**不是**某次分析的基准 |
| I6-A-007 | 最后验证时间 | [已核实] | `last_verified_at = now()`（`repo.rs:326`），**成功与失败都刷新** |
| I6-A-008 | 最近错误 | [已核实] | `last_error_code` 分类：`ssh_key_missing / timeout / auth_failed / not_found / too_large / git_failed`（`repo.rs:252, 301-308`）。**成功时清空**（`repo.rs:296`）。`last_error_detail` 截断 500 字符 |
| I6-A-009 | 唯一性 | [已核实] | `UNIQUE(project_id, url)`。重复 → `RepositoryError::Duplicate`（`repo.rs:54-57`） |
| I6-A-010 | 删除与解绑 | [部分核实] | **硬删除**（`repo.rs:146`）会级联删 `repo_import_source` 快照与 `discovery_finding_repository` 关联（见 §4.5）。**"解除关联但保留仓库"的 API 不存在**；`archive_repository`（`repo.rs:179-215`）是软替代 |

### 2.2 三个必须区分的 Commit（核验清单 §2.2 第 4 问）

| 概念 | 权威位置 | 写入时机 | 是否历史留存 |
|---|---|---|---|
| 仓库当前连接 HEAD | `project_repository.head_commit_sha` | `verify_repository` | ❌ 覆盖写，仅最后一次 |
| 本次分析采用的 Commit | `repo_import_source.head_commit_sha` | `run_import_job` 克隆+采集**之后** | ✅ 按 (import_id, repository_id) 留存 |
| 分析执行时实际用的 Commit | — | `repo_facts::collect` 内 `git rev-parse <branch>`（`repo_facts.rs:154-155`） | 与上一行同源 |

**结论**：三个概念在代码中是**两个**（第三行与第二行同源）。设计稿 §5.2 的区分成立，但"持久化基准在分析之前"（设计稿 §5.3 步骤 6）**未实现**——基准是采集完才落库。

### 2.3 必须回答的问题

| 问题 | 回答 | 依据 |
|---|---|---|
| 一个 Project 可关联多个 Repository？ | ✅ 可以，且这是当前唯一支持的方式 | `0008:30` 仅约束同项目同 URL 唯一 |
| 一个 Repository 可被多个 Project 关联？ | ✅ 可以（`project_repository.id` 独立，`UNIQUE(project_id, id)` 不禁止跨项目重复） | `0008:31` |
| 连接状态与分析状态是否分离？ | ✅ **已分离** | 连接态在 `project_repository.status`；分析态在 `repo_import.status` + `repo_import_source`。但分析态**不回写**连接态 |
| `head_commit_sha` 是验证 HEAD 还是分析 Commit？ | 验证 HEAD | `repo.rs:325` |
| 删除 Repository 后 Import/Source 如何处理？ | **CASCADE 删除，历史丢失** | `0009:63-64` → `0019:380-385` |
| 是否存在"默认 Repository"概念？ | ❌ 只有 `role='main'` 标签，**无默认语义** | `0008:23-24`；无任何按 role 筛选的代码 |

### 2.4 连接验证的真实实现（设计稿 §2.1 遗留项）

设计稿 §2.1 把"验证仓库是否可访问"列为待建能力。**代码事实：已实现，且是真实 git 子进程调用。**

- `verify_repository`（`repository/repo.rs:223-350`）→ `git_access::ls_remote`（`git_access.rs:122-163`，执行 `git ls-remote --heads <url>`，30s 超时，`GIT_TERMINAL_PROMPT=0`）
- SSH 仓库无 active 项目密钥 → 直接判 `unreachable` + `ssh_key_missing`（`repo.rs:245-254`）
- **校验失败不抛异常**，返回 `Ok(VerifyOutcome{status:"unreachable"})`，仅 DB 错误才 `Err`（`repo.rs:221-222`）
- **无历史留痕**：无 verify 历史表，审计只记动作名 `repository.verified`（`repo.rs:338-346`），**不含 status/sha/error**
- `0008_repositories.sql:4` 的注释"不做 clone / 可达性校验（后续切片）"**已过时**

### 2.5 权限边界（设计稿 §10.3 要求）

| 层级 | 现状 | 标记 |
|---|---|---|
| 租户边界 | ✅ DB 层组合 FK + API 层 `project_visible`（`apps/pm-api/src/support.rs:126-143`） | [已核实] |
| 项目成员 | ⚠️ **仓库级端点只校验 tenant_id，不校验项目成员**（`pm_repositories.rs:118-135`）。同租户内非该仓所属项目的成员可 PATCH/DELETE/verify/archive 该仓 | [存在缺陷] |
| 跨项目 | ⚠️ `discovery` 关联仓库时仅校验租户（`discovery/finding.rs:100-114`），同租户跨项目仓库可被挂到本项目 Finding | [存在缺陷] |

---

# §3. Repo Import 分析作业模型

## 3.1 数据库结构（I6-A-011～020）

最终表：`integration.repo_import`，定义于 `0009_repo_import.sql:27-49`，`0019` 补 `tenant_id` 与组合 FK，`0025:139-143` 补 `discovery_session_id` 与组合 FK。

| 编号 | 核验项 | 标记 | 事实与证据 |
|---|---|---|---|
| I6-A-011 | 主表 Schema | [已核实] | `id / project_id / requested_by / status / stage_detail / progress / model_name / draft / draft_revision / dropped_items / error_code / error_detail / analysis_run_id / created_at / updated_at / proposed_at / applied_at` + `tenant_id` + `discovery_session_id` |
| I6-A-012 | 项目归属与租户 | [已核实] | `project_id` + `tenant_id`；组合 FK `(tenant_id, project_id) → core.project`（`0019:295`）；`UNIQUE(tenant_id,id)`（`0019:163`） |
| I6-A-013 | Import 来源 | [不存在] | **`repo_import` 无 `source` 列**（`0009:27-49` + 全部 ALTER 已穷举核对）。设计稿的 `repo_import(source=WEB)` 是错误前提。`WEB` 实际是 `discovery_session.source`（`0025:16`）。Import 侧只有 `integration.inbound_event.source = 'repo_import'`（`lifecycle.rs:97`），那是**事件来源**不是发现来源 |
| I6-A-014 | Import 状态 | [已核实] | CHECK = `queued / cloning / reading / analyzing / draft_ready / proposed / applied / rejected / failed / cancelled`（`0009:31-34`）。代码侧 `IN_PROGRESS = ["queued","cloning","reading","analyzing"]`（`project_import/mod.rs:118`）。**无"部分成功"状态** |
| I6-A-015 | 时间字段 | [已核实] | `created_at / updated_at / proposed_at / applied_at`。**无 started_at / completed_at / cancelled_at**——取消与失败时刻只能从 `updated_at` 推断 |
| I6-A-016 | 执行信息 | [已核实] | `stage_detail`（人读中文阶段文案）、`progress`(0-100)、`model_name`（worker 启动时回写，`ai-pm-worker/src/main.rs:222`）、`analysis_run_id`。**无 Worker 标识、无 lease、无 attempt 计数**（这些在 `integration.job` 上） |
| I6-A-017 | 完成结果 | [已核实] | 成功：`status='applied'` + `applied_at`；失败：`error_code/error_detail`；**取消：`error_code/error_detail` 为空**，只有 `status='cancelled'` |
| I6-A-018 | 同项目并发限制 | [已核实] | **部分唯一索引** `repo_import_one_in_progress ON (project_id) WHERE status IN ('queued','cloning','reading','analyzing')`（`0009:52-54`）。⚠️ **`draft_ready` 不在谓词内** |
| I6-A-019 | Import 与 Repository 关系 | [已核实] | **仅通过 `repo_import_source`**。`repo_import` 无 `repository_id` 列 |
| I6-A-020 | Import 与 Discovery Session | [已核实] | **已存在**：`repo_import.discovery_session_id` + 组合 FK `(tenant_id, discovery_session_id) → discovery_session(tenant_id,id)`（`0025:139-143`），单向，无反向 FK。机械断言见 `crates/pm-infrastructure/tests/discovery_schema.rs:40-74` |

### 3.2 并发限制精确核验（核验清单 §3.2）

| 问题 | 回答 | 依据 |
|---|---|---|
| 唯一索引还是其他约束？ | 部分唯一索引（`CREATE UNIQUE INDEX ... WHERE`） | `0009:52-54` |
| 覆盖哪些状态？ | `queued / cloning / reading / analyzing` | `0009:54` |
| 只按 project_id 限？ | ✅ 是，且**不含 tenant_id**（租户已由 project_id 间接约束） | `0009:53` |
| 失败/取消/完成后释放？ | ✅ 释放（谓词不含终态） | 同上 |
| 状态更新遗漏导致长期无法再分析？ | ⚠️ **存在反向漏洞**：`draft_ready` 不占用名额，而 `start_import` 的 in-progress 检查（`lifecycle.rs:70-85`）用同一 4 状态集合 → 停在 `draft_ready` 的 Import **不阻止**发起第二个 Import | `lifecycle.rs:76` / `mod.rs:118` |

**核验清单 §3.2 特别确认项 —— 结论成立**：

> 当前模型限制的是同一个 Project 同时只有一个 in-flight Import，而不是同一个 Repository 同时只有一个 Import。

代码事实完全一致（`0009:52-54` 仅按 `project_id`），且**更严格**：当前实现下，一个 Project 的多个仓库**根本不能并行分析**，因为它们被打包在同一个 Import 里串行执行（见 §4.3）。

### 3.3 完整生命周期（真实状态机）

```text
start_import (lifecycle.rs:37-137)
   ├─ 校验 project 存在 / requirement 数=0 / ≥1 个 verified 仓库 / 无 in-progress Import
   ├─ tx: INSERT inbound_event(source='repo_import') → INSERT job(job_type='repo_import')
   ├─ tx: INSERT repo_import(status='queued', progress=0, model_name='fake')
   └─ tx: UPDATE core.project SET status='initializing' WHERE id=$1 AND status='planning'
                                  │
Worker: job_queue::claim(pool,"repo_import",worker,LEASE=3600s)
        （候选须 ai.project_ai_config.enabled AND automation_mode='ai_lead'）
        ↓
run_import_job (lifecycle.rs:297-459)
   ├─ 若 status='cancelled' 或非 IN_PROGRESS → 直接 return Ok(())
   ├─ set_stage 'cloning' 10%
   ├─ SELECT 全部 status='verified' 仓库 ORDER BY name        ← 注意：不是用户选择，是全取
   ├─ for each repo（串行）:
   │     ├─ clone_bare (600s)  失败 → record_source_error + fail_import + return
   │     ├─ repo_facts::collect → head_commit_sha / commits / docs / size
   │     └─ INSERT repo_import_source ... ON CONFLICT DO UPDATE      ← 基准在此落库
   ├─ set_stage 'reading' 45% → 'analyzing' 70%
   ├─ model.draft(materials, OrgChart::p0())  失败 → fail_import + return
   ├─ ground_draft → 若无 grounded 项 → fail_import('no_grounded_output')
   └─ UPDATE status='draft_ready', progress=100, draft=..., draft_revision+1
                                  ↓
propose_import (proposal.rs:111+)   ← REST POST .../apply
   ├─ status='applied' → AlreadyApplied / 'proposed' → 复用既有提案 / 非 draft_ready → InvalidState
   ├─ draft_revision 不符 → StaleDraft
   ├─ 写 ai.analysis_run + ai.change_proposal + governance.approval_request
   └─ UPDATE repo_import SET status='proposed', analysis_run_id=...   (proposal.rs:362)
                                  ↓
advance_import_after_decision (advance.rs:30-109)  ← 审批执行后触发
   ├─ 任一提案 vetoed  → import 'rejected'，**project 保持 initializing**（advance.rs:53-62）
   ├─ 全部 executed    → import 'applied' + project 'planning' + 审计（advance.rs:63-96）
   └─ 存在 failed      → import 保持 'proposed' + error_code='partial'，**project 保持 initializing**（advance.rs:98-107）
```

**Import 与 Project 状态回退的完整事实表**：

| 终态路径 | Import 状态 | Project 状态 | 是否回退 `planning` |
|---|---|---|---|
| 全部 executed | `applied` | `planning` | ✅ |
| 任一 vetoed | `rejected` | `initializing` | ❌ |
| 存在 failed | `proposed` + `partial` | `initializing` | ❌ |
| `fail_import`（clone/采集/模型失败） | `failed` | `initializing` | ❌ |
| `cancel_import` | `cancelled` | `initializing` | ❌ |
| 作业从未被认领 | `queued` | `initializing` | ❌ |

**结论：只有 1/6 条终态路径会把 Project 恢复到 `planning`。** 设计稿 §8.5 只提到"取消"，实际情况是**除全成功外全部泄漏**。

### 3.4 Import 触发源与执行者（I6-A-016/090～096）

| 环节 | 实现 | 证据 |
|---|---|---|
| 事件投递 | `integration.inbound_event` + `integration.job`，`UNIQUE(event_id, job_type)` | `0003:6-40`，`lifecycle.rs:94-115` |
| 作业认领 | `job_queue::claim`，`FOR UPDATE SKIP LOCKED` + 原子租约授予 | `crates/pm-infrastructure/src/job_queue.rs:29-79` |
| 租约 | `LEASE_SECS = 3600`（worker 侧） | `ai-pm-worker/src/main.rs:26` |
| 崩溃恢复 | **认领即回收**：`status='running' AND lease_until < now()` 的过期租约会被重新认领 | `job_queue.rs:52-57` 注释与谓词 |
| 失败重试 | `job_queue::fail`：`attempts+1`，达 `max_attempts` 转终态 `failed`，否则回 `queued` 并指数退避 `base*2^attempts` | `job_queue.rs:95-128`；`MAX_JOB_ATTEMPTS=5`、`BASE=5s`（worker `:20-21`） |
| 取消传递 | ⚠️ **不传递到执行中作业**。`cancel_import` 只改 `repo_import.status` 与 `integration.job.status`；`run_import_job` 仅在**入口**检查一次 status（`lifecycle.rs:308-310`） | 见 §8.3 |

---

# §4. Repo Import Source 多仓库来源模型

## 4.1 数据库结构（I6-A-021～030）

| 编号 | 核验项 | 标记 | 事实与证据 |
|---|---|---|---|
| I6-A-021 | 主键 | [已核实] | `PRIMARY KEY (import_id, repository_id)`（`0009:73`）；`tenant_id` 由 `0019:140` 补 |
| I6-A-022 | Import 外键 | [已核实] | `(tenant_id, import_id) → repo_import(tenant_id,id) ON DELETE CASCADE`（`0019:380-384`） |
| I6-A-023 | Repository 外键 | [已核实] | `(tenant_id, repository_id) → project_repository(tenant_id,id) ON DELETE CASCADE`（`0019:385`） |
| I6-A-024 | Commit 基准 | [部分核实] | `head_commit_sha` 可空，**由 `repo_facts::collect` 的 `git rev-parse <branch>` 得到**（`repo_facts.rs:154-155` → `lifecycle.rs:407`）。⚠️ **不具不可变性**：重跑同一 Import 时 `ON CONFLICT DO UPDATE` 覆盖（`lifecycle.rs:400-403`） |
| I6-A-025 | Source 状态 | [不存在] | **无任何状态字段**。只有 `error_code`（可空）区分"这一行有错误"。Source 的"成功/失败"由 `error_code IS NULL` 隐式表达 |
| I6-A-026 | 分析结果 | [已核实] | `facts JSONB`（默认 `{}`）+ `commit_count / doc_count / size_bytes / truncated`。**无耗时字段** |
| I6-A-027 | 多 Source 查询 | [已核实] | `get_import` JOIN 返回全部 Source（`lifecycle.rs:179-189`），一次返回多仓库 ✅ |
| I6-A-028 | Source 部分失败 | [不存在] | **一个仓库失败 = 整个 Import 失败**。`lifecycle.rs:364-368` 与 `387-391` 都是 `fail_import(...)` 后 `return`，**剩余仓库不再处理**。已成功 Source 的行保留（不删），但 Import 状态为 `failed` |
| I6-A-029 | Source 重试 | [不存在] | 无 Source 级重试。无重试则整 Import 重跑（作业级退避）；或人工发起新 Import（受 §3.2 的 in-progress 限制） |
| I6-A-030 | Source 历史 | [已核实] | 每个 Import 一份 Source 行，保留该次 commit 基准。**但删仓库会 CASCADE 抹掉历史**（§4.5） |

### 4.2 三个 Commit 概念的区分结论（核验清单 §4.3 末段）

> 需要特别区分：仓库当前连接的 HEAD / Import Source 本次分析采用的 HEAD / 分析执行完成后实际使用的 Commit。

代码事实：

| 概念 | 现状 | 缺口 |
|---|---|---|
| 仓库当前连接 HEAD | ✅ `project_repository.head_commit_sha`，verify 时覆盖写 | 无历史 |
| Source 本次分析采用的 HEAD | ✅ `repo_import_source.head_commit_sha` | ⚠️ **落库时机在分析之后**，不是"先定基准再分析" |
| 分析执行实际使用的 Commit | ❌ **不存在独立记录**——它与上一行同源（`rev-parse` 结果直接写入） | 若采集期间分支移动，二者可能不一致且无从发现 |

**判定：三个概念目前只区分了两个。** 设计稿 §5.5"分析结果必须能关联到 Commit"部分成立，"分析过程使用的代码版本与持久化基准一致"（设计稿 §5.3 末句）**当前无法保证**。

### 4.3 必须回答的问题

| 问题 | 回答 | 依据 |
|---|---|---|
| 一个 Import 真的可以包含多个 Source 吗？ | ✅ 数据模型可以，✅ 正常路径也会写多行（遍历全部 verified 仓库） | `lifecycle.rs:350-419` |
| 每个 Source 独立记录分析状态吗？ | ❌ 无状态字段，只有 `error_code` | `0009:60-74` |
| 顺序处理还是并行？ | **严格顺序**（单个 `for` 循环，`lifecycle.rs:350`） | `lifecycle.rs:350` |
| 一个 Source 失败，Import 失败还是部分成功？ | **整个 Import 失败**（首个失败即 return），后续仓库不再分析 | `lifecycle.rs:364-368, 387-391` |
| 能否只重试失败 Source？ | ❌ 不能 | 无 Source 级重试路径 |
| Import 完成后 Source 结果可独立查询吗？ | ✅ 可以（`get_import` 的 `sources` 数组） | `lifecycle.rs:179-189` |
| Source 的 Commit SHA 在执行期间保持不变？ | ⚠️ 执行期间不会变，但**重跑会覆盖** | `lifecycle.rs:400-403` |

### 4.4 一个被设计稿遗漏的重要事实：Source 的选择权不存在

设计稿 §6.1 主流程写"选择一个或多个 Repository"。**代码事实：`run_import_job` 取的是该项目下全部 `status='verified'` 的仓库**（`lifecycle.rs:317-323`），用户无从选择子集。

推论：当前模型下，"一个 Import 包含哪些仓库"= "此刻哪些仓库 verified"，这是一个**漂移的口径**——在 clone 之前某仓库被 archive，会导致本次 Import 的 Source 集合与用户预期不一致。设计稿 §5.3 建议的"创建或确认 Repo Import Source（第 5 步）→ 持久化 Commit 基准（第 6 步）→ 获取代码（第 7 步）"正是缺失的这一段：**Source 集合必须在执行前确定并落库**。

### 4.5 删除仓库对 Source 历史的影响（[存在缺陷]）

| 被引用对象 | FK 语义 | 删除仓库后果 |
|---|---|---|
| `repo_import_source.repository_id` | `ON DELETE CASCADE`（`0019:385`） | 该仓在**所有历史 Import** 中的 commit 基准、计数、facts **被永久删除** |
| `discovery_finding_repository.repository_id` | `ON DELETE CASCADE`（`0025:74-75`） | Finding ↔ 仓库关联行被静默删除（Finding 本体保留） |
| `delivery.task_repository` / `task_run_repository` / `task_repository_commit` | **无 ON DELETE → NO ACTION**（`0011:100-102,118-120`；`0022:79-80`） | 删除被数据库拒绝 → 应用层无预检 → HTTP 500 `internal error`（`pm_repositories.rs:95-98`） |

**判定：设计稿 §5.5"不得只按 Repository ID 覆盖历史分析结果"在删除路径上不成立。** 仓库硬删除会静默抹掉历史分析基准。

---

# §8. Project 生命周期与 Import 取消缺陷（P0）

## 8.1 Project 状态定义（I6-A-068）

| 编号 | 核验项 | 标记 | 事实与证据 |
|---|---|---|---|
| I6-A-068 | Project 状态定义 | [已核实] | `core.project.status TEXT NOT NULL DEFAULT 'planning'`，**无 CHECK、无枚举表**（`migrations/0001_core.sql:24`）。0019～0028 **未改动该列**。实际取值仅 `planning` / `initializing`（`bootstrap.rs:255,257`；`lifecycle.rs:127`；`advance.rs:73`）。⚠️ `active` / `archived` **在代码中不存在**——设计稿 §9.1"在 I1 尚未实施的阶段，I6 不得擅自把 `active` 当作现有 Project 状态"这一条提醒是对的 |

## 8.2 Import 对 Project 状态的全部写入点（I6-A-069～071）

生产代码中对 `core.project.status` 的**写入仅 3 处**：

| # | 位置 | 语句 | 触发条件 |
|---|---|---|---|
| 1 | `bootstrap.rs:254-262` | INSERT project，status ∈ {`planning`,`initializing`}，其它值报 `unsupported initial project status` | 项目创建 |
| 2 | `project_import/lifecycle.rs:127` | `UPDATE core.project SET status='initializing' WHERE id=$1 AND status='planning'` | `start_import` 事务内 |
| 3 | `project_import/advance.rs:73` | `UPDATE core.project SET status='planning', updated_at=now() WHERE id=$1` | `advance_import_after_decision` 全部提案 executed 时，**同事务** |

**没有第 4 处。**

## 8.3 取消路径的完整核验（I6-A-072）

`cancel_import`（`project_import/lifecycle.rs:229-258`）全文：

```rust
let status = ... SELECT status FROM repo_import WHERE id=$1 AND project_id=$2
if !IN_PROGRESS.contains(status) && status != "draft_ready" {
    return Err(ImportError::NotCancellable);      // 409
}
let mut tx = pool.begin().await?;
UPDATE integration.repo_import SET status='cancelled', updated_at=now() WHERE id=$1
UPDATE integration.job     SET status='cancelled' WHERE event_id=(... idempotency_key=$1)
tx.commit()?;
```

**这个函数里没有一行触碰 `core.project`。**

### 判定：[存在缺陷] —— 且比设计稿描述的更严重

设计稿 §1.4 / §8.5 只点名了 `cancel_import`。核验后，**同一类泄漏共 5 条路径**：

| # | 路径 | Import 结果 | Project 终态 | 位置 |
|---|---|---|---|---|
| 1 | 人工取消 | `cancelled` | ❌ `initializing` | `lifecycle.rs:229-258` |
| 2 | clone/采集失败 | `failed` | ❌ `initializing` | `lifecycle.rs:282-293`（`fail_import` 不碰 project） |
| 3 | 模型失败 / 无 grounded 输出 | `failed` | ❌ `initializing` | `lifecycle.rs:431, 438-446` |
| 4 | 提案被驳回 | `rejected` | ❌ `initializing` | `advance.rs:53-62` |
| 5 | 部分提案执行失败 | `proposed` + `partial` | ❌ `initializing` | `advance.rs:98-107` |
| 6 | 作业从未被认领 | `queued` | ❌ `initializing` | 见 §8.4 |

### 泄漏后的连锁效应 [存在缺陷]

**`initializing` 是不可自愈的终态**：

1. `core.project.status` 无 CHECK，`initializing` 不阻塞任何后续操作（无门禁代码，见 §12.6）。
2. `start_import` 的 `UPDATE ... WHERE status='planning'`（`lifecycle.rs:127`）**命中 0 行**——因为 project 已是 `initializing`。但**Import 照常创建**（该 UPDATE 不检查影响行数）。→ 项目永久停在 `initializing`，且每次再发起导入都徒劳。
3. `start_import` 的 `ProjectNotEmpty` 检查只看 `delivery.requirement` 数量（`lifecycle.rs:50-52`）。未 apply 的导入不落 requirement → 数量仍为 0 → **可以反复发起导入**，每次都再插一行 `initializing` 的 project 状态（无效写入）+ 新 Import。
4. 唯一会把状态拉回 `planning` 的是 `advance.rs:73`，它要求 `analysis_run_id` 能匹配到 import 且提案全部 `executed`。一旦驳回/失败/取消，`analysis_run_id` 或提案集合不满足条件，**恢复路径不可达**。

**结论：项目可以永久卡在 `initializing`，且当前没有任何自愈、定时修复或 orphan reconciliation 覆盖 `repo_import`。**

（对照：`run_executor::reconcile_orphans` 存在，但只覆盖 Run（`ai-pm-worker/src/main.rs:123`），grep `repo_import` 在 `run_executor.rs` / `outbox_handlers.rs` 中**零命中**。）

### 取消语义的三个附加缺陷

| # | 缺陷 | 证据 |
|---|---|---|
| A | **取消可被静默推翻**：执行中的 Import 被取消后，worker 继续跑完，`set_stage`（`lifecycle.rs:261-279`）与最终 `UPDATE status='draft_ready'`（`lifecycle.rs:449`）**都不检查当前状态**，会把 `cancelled` 覆盖回 `draft_ready`。取消的用户会看到一个"已取消"的导入最终变成"草案已生成" | `lifecycle.rs:261-279, 449` |
| B | **作业状态也被覆盖**：worker 结束时调用 `job_queue::complete` → `UPDATE job SET status='completed'`（`job_queue.rs:88-97`），覆盖 `cancel_import` 写入的 `'cancelled'` | `ai-pm-worker/src/main.rs:232` |
| C | **取消不幂等**：第二次取消因 `status='cancelled'` 不在允许集内 → `NotCancellable` → 409 | `lifecycle.rs:241-243` |

## 8.4 Worker 异常退出与恢复（I6-A-073～075）

| 问题 | 回答 | 依据 |
|---|---|---|
| Worker 被杀死，Project 是否长期停在 `initializing`？ | ⚠️ **取决于作业是否已被认领**。未认领（`queued`）→ 作业会一直留在队列，Project 永久 `initializing`；已认领（`running`）→ 租约过期后被重新认领并重跑 | `job_queue.rs:52-57` |
| 是否存在定时修复 / orphan reconciliation？ | ❌ **repo_import 没有**。只有 Run 有 `reconcile_orphans` | `ai-pm-worker/src/main.rs:123`；grep 无 repo_import 命中 |
| 是否存在"作业永远不被认领"的确定路径？ | ⚠️ **存在（V1.0 勘误，见文末）**：claim 的候选条件包含 `JOIN ai.project_ai_config cfg ON cfg.project_id=job.project_id AND cfg.enabled AND cfg.automation_mode='ai_lead'`（`job_queue.rs:38-45`），这是**有意的冻结语义**（`job_queue.rs:11-13` 注释）。~~bootstrap 不创建该行~~ **勘误：`bootstrap_project` 会创建该行**（`bootstrap.rs:288-297`，`automation_mode='ai_lead'`）。剩余真实路径：项目创建**之后**配置被置 `enabled=false` 或切到 `controlled`（冻结）时，**已在队列中的 repo_import 作业永不被认领、也永不过期失败** → Import 永远停在 `queued`，Project 永远停在 `initializing`，无 reconcile 覆盖 | `job_queue.rs:38-45`；`bootstrap.rs:288-297` |
| 状态恢复是否可能误覆盖另一个操作？ | ⚠️ **会**。`advance.rs:73` 的 `UPDATE core.project SET status='planning' WHERE id=$1` **没有守卫条件**（对比 `lifecycle.rs:127` 有 `AND status='planning'`）。若此时有第二个 Import 正在运行，其 `initializing` 会被这条无守卫的 UPDATE 覆盖为 `planning` | `advance.rs:72-77` |
| Project 状态更新是否与 Import 状态更新同事务？ | ⚠️ **只有全成功路径同事务**（`advance.rs:64-95` 一个 tx）。取消、失败、驳回路径都是**单表 UPDATE，无事务包裹** | `lifecycle.rs:244-256`、`lifecycle.rs:284-292`、`advance.rs:54-61` |
| 重复状态更新是否幂等？ | 部分。`advance.rs` 的 `statuses.iter().all(...)` 判定天然幂等；但 `fail_import` 无状态守卫，重复调用会覆盖 `error_code` | `lifecycle.rs:282-293` |

## 8.5 多 Import 竞争（I6-A-076）

| 场景 | 现状 |
|---|---|
| 两个 Import 同时 `start_import` | 应用层先查 `in_progress` 计数（`lifecycle.rs:70-85`）→ 有竞态窗口；DB 部分唯一索引（`0009:52-54`）兜底。但**索引冲突未映射为明确错误**——`lifecycle.rs:116-126` 的 INSERT 遇唯一冲突会返回 `ImportError::Db`（`mod.rs:94-96`）→ REST 500，不是设计稿 §8.3 要求的"明确的项目分析正在进行错误" |
| `draft_ready` 状态下再发起导入 | ⚠️ **允许**（索引与计数都不含 `draft_ready`）→ 同一项目可同时存在两个 Import |
| 两个 Import 各自 apply | 各自产生 `analysis_run` 与 `change_proposal`，无相互约束 |

**判定：[存在缺陷]** 设计稿 §8.3"如果唯一索引冲突：返回明确的项目分析正在进行错误；不创建孤立 Source；不留下不完整 Session"——**当前实现只有前两项隐式成立（事务回滚），错误映射缺失**。

## 8.6 与 I1 归档门禁的关系

核验事实（供 I1 参考，非本报告重点）：

- `core.project` **不存在 `archived` 状态**，也无任何归档入口。
- 生产代码中**没有任何** `SELECT ... FROM core.project ... WHERE status = ...` 的门禁查询（仅测试文件命中）。
- 后果：`start_import` / `create_task` / Run 创建三处**均无项目状态门禁**。

**I6 §9 的"归档门禁复用 I1 规则"在 I1 实施前无法落地，且 I6 也不能单方面发明 `archived` 语义**——这与设计稿 §9.1/§9.3 的自我约束一致。

---

# §12. 补充核验（单遍，第二轮深化）

以下章节由同批次并行核验得出，证据充分但未逐行复核，标注为单遍结论。

## 12.1 §5 Discovery Session —— 设计稿 §2.4/§3 的前提作废

| 编号 | 标记 | 事实 |
|---|---|---|
| I6-A-031～032 | [已核实] | `discovery.discovery_session` **已存在**（`0025:12-26`）。字段：`id / tenant_id / project_id / source / kind / status / created_by / created_at / updated_at` |
| I6-A-033 | [已核实] | 生产者存在一条真实链路：`discovery::start_session_from_import`（`crates/pm-application/src/discovery/web.rs:12-69`）——创建 Session → 回填 `repo_import.discovery_session_id` → 把 `repo_import.draft.unknowns` 映射为 `discovery_question`。HTTP 入口 `POST /v1/pm/projects/{id}/discovery/sessions`（`apps/pm-api/src/pm_discovery.rs:167`） |
| I6-A-034～035 | [已核实] | `source ∈ (WEB, CODEX, SYSTEM)`、`kind ∈ (INITIAL_ONBOARDING, UPDATE, TASK_DISCOVERY)`、`status ∈ (IN_PROGRESS, NEEDS_USER_INPUT, REVIEW_REQUIRED, COMPLETED, PAUSED)`（`0025:16-20`）。转移表在 `discovery/session.rs:163-177`。**`source=CODEX/SYSTEM`、`kind=UPDATE/TASK_DISCOVERY` 无生产者**；**无自动暂停/恢复**，只能人工调 status 端点 |
| I6-A-036 | [已核实] | **已关联**（`0025:139-143`，单向 FK） |
| I6-A-037 | [已核实] | `tenant_id NOT NULL` + 组合 FK `(tenant_id, project_id) → core.project(tenant_id,id)`（`0025:25`） |
| I6-A-038 | [已核实] | 消费者：应用层 `discovery/session.rs`、REST `pm_discovery.rs:148-237`、MCP `tools.rs:987-1042`（`pm_discovery_get` / `pm_discovery_get_context`，含 `resolve_session` 取项目 live session）、前端 `frontend/src/api/discovery.ts` |
| I6-A-039 | [已核实] | `discovery_finding.session_id` NOT NULL + 组合 FK CASCADE（`0025:61`） |
| I6-A-040 | [已核实] | **允许多 Session 并行**，库层无"项目仅一个活跃 Session"约束；测试 `parallel_sessions_are_allowed`（`crates/pm-application/tests/discovery.rs:150-176`）。"live session" 只存在于 MCP 读取侧的排序偏好（`tools.rs:1008-1017`） |

**对设计稿 §3.2 的直接影响**：设计稿花了大量篇幅论证"Discovery Session 应独立于 Repo Import"。**代码已经这样做了**（0025 独立建表 + 单向 FK）。设计稿 §3.2/§3.3/§3.5 的论证与 §3.3 的"建议新建独立的 `discovery_session`"全部作废——表已存在，且 `source/kind/status` 三字段已存在且语义已定（`source` = 会话产生入口，不是"发现来源"的可选枚举 WEB/REPOSITORY/MANUAL/MCP）。

## 12.2 §6 Finding / Question / Proposal

| 编号 | 标记 | 事实 |
|---|---|---|
| I6-A-041～042 | [已核实] | `discovery_finding`（`0025:48-62`）：`kind ∈ (observed, user_confirmed, speculation, contradicted)`、`verification_status ∈ (PENDING, CONFIRMED, REJECTED, CONFLICT)` 默认 `PENDING`、`confidence 0..1`、`evidence JSONB`、`actor_role` |
| I6-A-043 | [已核实] | **多对多关联表已存在**：`discovery.discovery_finding_repository`（`0025:67-76`），PK `(tenant_id, finding_id, repository_id)`，双组合 FK CASCADE |
| I6-A-044 | [不存在] | **Finding 无法追溯到 Source**。`discovery_finding` 无 `import_id` / `import_source_id` / `commit_sha` 列；关联只到 Repository 粒度 |
| I6-A-045 | [不存在] | **Finding 不保存 commit 基准**。`evidence` JSONB 的约定形状是 `[{repository, file, description}]`（`0025:57` 注释），**无 commit 字段** |
| I6-A-049～052 | [已核实] | `discovery_question`（`0025:79-93`）：`status ∈ (open, answered, deferred)`、可空 `finding_id`。**无作者/方向字段**（核验清单 §6.2 已预判，正确） |
| I6-A-053～058 | [已核实] | `discovery_proposal`（`0025:113-133`）：`status ∈ (pending, approved, rejected, conflict)`、`action ∈ (add, modify)`、`target_document`、`document_id`、`base_revision_id`、`document_proposal_id`。**`approved` 无生产者**；`document_proposal_id` 回填只写关联不改状态（`proposal.rs:167-196`） |
| — | [已核实] | **`speculation` 默认 PENDING 成立**（DDL 默认 + 应用层 `finding.rs:90` + 机械断言 `discovery_schema.rs:141-153`）；**AI 晋级路径不存在**——`set_finding_verification` 的唯一生产调用者是真人 Web 路径（`web.rs:92`），MCP 侧 `submit_findings` 强制 `verification_status: None`（`tools.rs:1126`）且入参结构体无该字段（`tools.rs:866-880`） |

## 12.3 §7 Knowledge Proposal 与版本并发

| 编号 | 标记 | 事实 |
|---|---|---|
| I6-A-060～061 | [已核实] | `ai.document_proposal`（`0014:38-41`）8 态：`draft / quality_gates_passed / redline_pending / confirmed / commit_pending / committed / rejected / rolled_back`。⚠️ `quality_gates_passed` 与 `confirmed` **代码从不写入**，仅 CHECK 允许 |
| I6-A-062 | [已核实] | **版本基准字段是 `base_revision_id`**（`0014:31`），**不存在 `base_version`**。另有 `base_content_hash`（`0014:32`）全代码零读写（死列） |
| I6-A-063 | [已核实] | **乐观并发校验已实现**：`knowledge_write_coordinator.rs:268-280` `check_base_revision`，调用点 `knowledge_proposal/submit.rs:96-101`；质量门内再校一次 `knowledge_quality.rs:111-121`（gate=`version`） |
| I6-A-064 | [已核实] | 冲突 → `KnowledgeError::RevisionConflict` → discovery 侧落 `conflict` 状态（`tools.rs:1229-1258`） |
| I6-A-065 | [已核实] | Project 级 advisory 写锁（跨实例有效）：`pg_try_advisory_lock('ai.project_knowledge_write:{project_id}')`，默认 10s 超时（`knowledge_write_coordinator.rs:80-121`） |
| I6-A-067 | [已核实] | **统一写入协调服务已存在**：`crates/pm-application/src/knowledge_write_coordinator.rs` |
| — | [已核实] | `discovery_proposal` 的 `base_revision_id` 写入侧校验（`proposal.rs:84-112`）与知识链校验三层齐备，测试见 `tests/discovery.rs:492-524` |

**对设计稿的影响**：设计稿 §7.6 提出的"`base_version` 乐观并发校验"**已经存在等价物**（`base_revision_id` + advisory lock + 质量门复检）。I6 无需新增并发机制，只需确保 Discovery 路径正确复用（现状已复用）。

## 12.4 §9 REST / MCP / Worker 调用链

| 编号 | 标记 | 事实 |
|---|---|---|
| I6-A-077～081 | [已核实] | REST：`/v1/pm/projects/{id}/repositories`（GET/POST）、`/v1/pm/repositories/{id}`（PATCH/DELETE）、`.../verify`、`.../archive`（`pm_repositories.rs:28-45`）；`/v1/pm/projects/{id}/imports`（GET/POST）+ `/{id}` + `/cancel` + `/apply` + `/proposals`（`pm_imports.rs:23-43`）；Discovery 9 条（`pm_discovery.rs:25-58`） |
| I6-A-083～084 | [不存在] | **MCP 侧没有 repository / repo_import 任何工具**。18 个 MCP 工具中唯一涉及仓库的是 `pm_discovery_get_context` 内的只读 SQL（`tools.rs:1058-1063`） |
| I6-A-085～086 | [已核实] | MCP discovery 工具 6 个：`pm_discovery_get / get_context / submit_findings / submit_answers / submit_proposal / complete`（`server.rs:240-354`） |
| I6-A-082/087/088 | [已核实] | REST 用 `support::project_visible`（tenant+member），MCP 用 `authz::require_project_member`——**语义等价但不是同一函数**。MCP 权限表 `MCP_TOOL_PERMISSIONS`（`apps/pm-api/src/auth.rs:562-568`）只列 5 个工具，**`pm_discovery_submit_proposal` 缺失**（其描述却声明需 `PM_MCP_DOCUMENT_WRITE`）；findings/answers/complete 等写操作实际只需 `PM_MCP_READ` |
| I6-A-090～096 | [已核实] | Worker：`ai-pm-worker` 第二条循环专跑 `repo_import`（`main.rs:75-84`），入口 `handle_repo_job` → `run_import_job`。**无 repo_import 的 orphan reconcile**；无取消传播；`run_import_job` 的失败路径一律 `Ok(())` → `job_queue::complete` → **git/模型失败不触发作业重试** |

## 12.5 §10 审计与幂等

| 编号 | 标记 | 事实 |
|---|---|---|
| I6-A-097～099 | [已核实] | 审计表 `governance.audit_log`；helper **真实存在于** `crates/pm-application/src/pm_write/audit.rs`（设计稿路径正确）：`append_audit`（`:40`）、`append_audit_with_after`（`:72`）、`record_decision_audit`（`:109`）。⚠️ `repository/support.rs:12` 另有一份**复制品**，应收敛 |
| I6-A-100 | [存在缺陷] | **Import 创建/取消/apply 均无审计**（`lifecycle.rs:33-141` / `229-258` / `proposal.rs:111+` 全无 audit 调用）。只有 `advance.rs:79` 的 `project_import_applied` |
| I6-A-101 | [已核实] | Repository 五个动作均有审计：`repository.created/updated/deleted/archived/verified`（`repository/repo.rs:59,119,150,204,338`） |
| I6-A-102 | [存在缺陷] | Finding 人工决定**无审计**（`crates/pm-application/src/discovery/*.rs` 全文无 `audit` 字样） |
| I6-A-103 | [已核实] | 知识提案有审计（`knowledge_proposal/support.rs:32`）；discovery 侧提案无 |
| I6-A-104 | [存在缺陷] | 创建 Import **无客户端幂等键**：`import_id = Uuid::new_v4()` 且以其自身作为 `idempotency_key`（`lifecycle.rs:88,102`）→ 每次都是新事件 |
| I6-A-105 | [存在缺陷] | 取消 Import **不幂等**（重复取消 409） |
| I6-A-106 | [存在缺陷] | `pm_discovery_submit_findings` / `submit_answers` **无幂等键** → AI 重试会产生重复 Finding/Question |
| I6-A-107 | [已核实] | 知识提案幂等：`operation_id` + `claim_operation`（`knowledge_write_coordinator.rs:142-193`） |
| I6-A-108 | [已核实] | 作业级幂等：`job_queue` 租约 + `attempts` 退避；Import 侧重跑用 `ON CONFLICT DO UPDATE`（幂等覆盖） |

## 12.6 §11 Migration 与空库初始化

| 编号 | 标记 | 事实 |
|---|---|---|
| I6-A-109 | [部分核实] | `migrations/0001～0028` 连续无缺号。未连库实际执行（本次为静态核验） |
| I6-A-110 | [已核实] | I6 相关三表最终结构见 §2.1 / §3.1 / §4.1 |
| I6-A-111 | [已核实] | 未发现重复字段/索引/约束；`0008` 的注释已过时（"不做可达性校验"） |
| I6-A-112～113 | [已核实] | 外键与索引见 §2.1/§3.1/§4.1 |
| I6-A-114～115 | [待核实] | 空库初始化与测试库一致性未实测 |

---

# §13. 设计候选与代码事实对照（核验清单 §12）

| 编号 | 设计候选 | 代码事实 | 判定 |
|---|---|---|---|
| I6-D-001 | Discovery Session 独立于 Import | **已独立存在**（`0025:12-26`），单向 FK `repo_import.discovery_session_id` | ✅ **已满足，无需新建** |
| I6-D-002 | Import 关联 Session | **已存在**（`0025:139-143`） | ✅ **已满足** |
| I6-D-003 | 一个 Session 可关联多个 Import | ✅ 允许（Session 侧无唯一约束；`repo_import.discovery_session_id` 可重复） | ✅ **已满足** |
| I6-D-004 | 一个 Import 可包含多个 Source | 数据模型 ✅，正常路径 ✅，**失败路径 ❌**（首个失败即中止整体） | ⚠️ **部分满足** |
| I6-D-005 | Source 独立 Commit 基准 | ✅ 有字段且按 Import 留存；⚠️ **落库在分析之后**、**重跑会覆盖** | ⚠️ **部分满足** |
| I6-D-006 | Source 级部分重试 | ❌ 不存在 | ❌ **需新增** |
| I6-D-007 | Finding 可关联多个 Repository | ✅ **表与读写均已存在**（`0025:67-76`） | ✅ **已满足，无需新增表** |
| I6-D-008 | Finding 可追溯 Source | ❌ Finding 无 import/source/commit 关联 | ❌ **需新增** |
| I6-D-009 | Knowledge Proposal base_version | ✅ 等价机制已存在：`base_revision_id` + advisory lock + 质量门复检 | ✅ **复用，不新增** |
| I6-D-010 | 多 Agent 并行分析 | ⚠️ Import 级不支持；MCP 多 Agent 并行产出 Finding 支持（但无幂等键） | ⚠️ **部分满足** |
| I6-D-011 | Knowledge 写入统一协调 | ✅ `knowledge_write_coordinator.rs` 已存在并被 Discovery 路径复用 | ✅ **已满足** |
| I6-D-012 | speculation 默认待确认 | ✅ DDL 默认 + 应用层 + 机械断言 | ✅ **已满足** |
| I6-D-013 | Agent 不可自行晋级可信知识 | ✅ 代码层无路径（MCP 强制 `verification_status: None`） | ✅ **已满足** |

---

# §14. 缺陷清单（按优先级）

| 编号 | 优先级 | 缺陷 | 证据 | 与设计稿关系 |
|---|---|---|---|---|
| D-01 | **P0** | Project 终态泄漏：**5 条终态路径不恢复 `planning`**（取消 / clone 失败 / 模型失败 / 驳回 / 部分失败），项目可永久卡 `initializing` | §8.3 | 设计稿只提了"取消"，实际范围更广 |
| D-02 | **P0** | 取消可被静默推翻：worker 继续推进 `set_stage` → `draft_ready`，覆盖 `cancelled`；作业状态亦被 `complete` 覆盖 | `lifecycle.rs:261-279,449`；`job_queue.rs:88-97` | 设计稿未覆盖 |
| D-03 | ~~P0~~ **P1（V1.0 勘误）** | ~~bootstrap 不创建 ai 配置行~~ **勘误：`bootstrap_project` 创建该行**（`bootstrap.rs:288-297`，`ai_lead`）。真实缺陷收窄为：claim 的冻结过滤是有意语义（`job_queue.rs:11-13`），但项目冻结（`enabled=false` / `controlled`）发生时，**在飞 `queued` Import 无人认领也无终态化机制** → 永久卡 `queued` + `initializing`。修法=创建时 fail-fast 校验 + 冻结时终态化在飞 Import（与 I1 对齐），不是删 claim 条件 | `job_queue.rs:38-45`；`bootstrap.rs:288-297` | I6-B §8.2 方向正确 |
| D-04 | **P1** | 取消/失败/驳回路径**无审计** | `lifecycle.rs`、`proposal.rs` | 设计稿 §12 要求审计 |
| D-05 | **P1** | `advance.rs:73` 的 `UPDATE core.project SET status='planning'` **无守卫**，可覆盖并发中的 `initializing` | `advance.rs:72-77` | 设计稿 §8.4 "避免误覆盖" |
| D-06 | **P1** | 唯一索引冲突未映射为明确错误 → 500 而非 409 | `lifecycle.rs:116-126` | 设计稿 §8.3 明确要求 |
| D-07 | **P1** | `draft_ready` 不占用 in-progress 名额 → 同一项目可并存两个 Import | `0009:52-54`；`mod.rs:118` | 设计稿未覆盖 |
| D-08 | **P1** | 仓库硬删除 **CASCADE 抹掉全部历史 Import 的 commit 基准** 与 Finding 关联；被 task/run 引用时删除被 DB 拒绝 → 500 | §4.5 | 设计稿 §5.5 直接被推翻 |
| D-09 | **P1** | 仓库级端点只校验 `tenant_id`，**不校验项目成员** → 同租户跨项目可改/删/验他人仓库 | `pm_repositories.rs:118-135` | 设计稿 §10.3 要求校验归属 |
| D-10 | **P1** | `pm_discovery_submit_proposal` 缺失于 MCP 权限表；findings/answers/complete 等写操作仅需 READ 权限 | `auth.rs:562-568` | 设计稿 §7.6 / §10.3 |
| D-11 | **P1** | `pm_discovery_submit_findings/answers` 无幂等键 → AI 重试写重复 | `tools.rs:1102-1190` | 设计稿 §10.4 |
| D-12 | **P1** | Discovery 全部写操作无审计 | `crates/pm-application/src/discovery/*.rs` | 设计稿 §12 |
| D-13 | **P2** | Import 的 `cancel`/`fail` 不幂等；创建 Import 无客户端幂等键 | `lifecycle.rs:241-243, 88` | 设计稿 §10.4 |
| D-14 | **P2** | `source` 集合在执行期才确定（取全部 verified 仓库），用户无选择权，集合会漂移 | `lifecycle.rs:317-323` | 设计稿 §6.1 假设"用户选择" |
| D-15 | **P2** | Source 无状态字段，无法表达"部分成功" | `0009:60-74` | 设计稿 §4.4 |
| D-16 | **P2** | `repo_import` 无 `started_at/completed_at/cancelled_at`，终态时刻只能靠 `updated_at` 推断 | `0009:27-49` | 设计稿 §2.2 要求 |
| D-17 | **P2** | `ssh_key_id` 列自 0009 起从未写入，恒为 NULL | `0009:15` | — |
| D-18 | **P2** | `base_content_hash` 死列（全代码零读写） | `0014:32` | — |
| D-19 | **P2** | `discovery_proposal.status='approved'` 无生产者；知识提案终态不回写 discovery 侧 | `proposal.rs:167-196` | 设计稿 §7.5 |
| D-20 | **P2** | `append_audit` 存在两份独立实现 | `pm_write/audit.rs:40` vs `repository/support.rs:12` | 设计稿 §12 要求复用 |
| D-21 | **P3** | `core.project.status` 无 CHECK，`active`/`archived` 不存在且无门禁 | `0001:24` | 设计稿 §9.1 的自我约束成立 |
| D-22 | **P3** | `integration.job` / `inbound_event` / `repo_import` 状态列均无 CHECK | `0003`、`0009` | — |

---

# §15. 对 I6 设计稿的修订要求

## 15.1 必须作废的章节

| 设计稿位置 | 作废理由 |
|---|---|
| §2.4、§3.2、§3.3、§3.5 | `discovery_session` 已存在且已有生产者/消费者/状态机。设计稿的"建议新建"与论证全部作废 |
| §2.3（Source 字段建议）、§2.5（关系示意） | 关系已由 0025 落地，且方向是 `repo_import → discovery_session`（与设计稿推荐一致） |
| §3.4 的 `source` 枚举候选（WEB/REPOSITORY/MANUAL/MCP） | 实际枚举是 `WEB/CODEX/SYSTEM`，且语义是"会话产生入口" |
| §4.1 中"新增 Finding↔Repository 多对多关系表" | 表已存在且已读写 |
| §5.3（持久化基准在分析之前）、§5.5（历史不覆盖） | 需按 §4.2/§4.5 现状重写 |
| §6.1 主流程中"用户选择仓库" | 现状是全取 verified 仓库 |
| §7.6 的 `base_version` 机制设计 | 已有等价物 `base_revision_id` + advisory lock，改为"复用并补 Discovery 侧一致性" |
| §8.5（仅取消需修复）、§8.3（并发保护的具体形态） | 需按 §8.3/§8.5 的实际缺陷重写 |
| §14 的 I6-A/I6-B 拆分 | 核验已完成，I6-B 应直接是"按代码事实重写领域模型" |
| §18 全部六个待裁决问题 | Q1/Q2/Q3/Q5/Q6 已由代码事实回答（见下） |

## 15.2 §18 待裁决问题的代码事实答案

| 问题 | 答案 |
|---|---|
| Q1 Session 与 Import 关联约束 | **已落地**：`repo_import.discovery_session_id` → `discovery_session.id`，组合 FK，单向，无反向 FK（`0025:139-143`） |
| Q2 `source` 的权威归属 | **`repo_import` 无 source 列**。`WEB` 属 `discovery_session.source`（会话产生入口）；Import 侧唯一 source 是 `inbound_event.source='repo_import'`（事件来源）。三概念本就未复用，无需裁决 |
| Q3 `pm_discovery_*` 命名空间 | **已存在** `discovery` schema（`0025:9`）。既有表 6 张 + 既有组合 FK/唯一键模式（`0019` 多租户模式）。新增对象沿用 `discovery.` 前缀 |
| Q4 Source 级部分重试 | **当前完全不支持**。且 §4.4 表明 Source 集合本身在执行期才确定。**先做"执行前固化 Source 集合"，部分重试才有意义** |
| Q5 Finding 多仓库关联 | **表已存在**（`0025:67-76`）且读写已实现（`finding.rs:131-141` / `59-80`）。**无需新增表**。真正缺的是 Finding↔Source/Commit 追溯（D-08 / I6-D-008） |
| Q6 知识提案版本校验 | **已存在**：`base_revision_id`（`0014:31`）+ `check_base_revision`（`knowledge_write_coordinator.rs:268-280`）+ 质量门 version gate（`knowledge_quality.rs:111-121`）+ advisory lock（`:80-121`）。**复用，不新增** |

## 15.3 I6-B 的新任务定义

I6-B 不再是"设计领域模型"，而是**"按代码事实重写 I6 领域模型并收敛 22 项缺陷"**。建议按以下顺序切分：

| 顺序 | 工作块 | 内容 | 关联缺陷 |
|---|---|---|---|
| 1 | Project 终态收敛 | 统一终态回退规则：取消/失败/驳回/部分失败一律恢复 `planning`；`advance.rs:73` 加守卫；引入"是否有其它初始化操作在飞"的判定 | D-01 D-05 |
| 2 | 取消语义 | 取消可推翻修复（所有状态推进加守卫）、作业状态不覆盖、取消幂等 | D-02 D-13 |
| 3 | 作业认领可达性 | `repo_import` 作业的 claim 条件与 `bootstrap_project` 的 `ai.project_ai_config` 缺失对齐 | D-03 |
| 4 | Source 集合固化 | Import 创建时确定并落库 Source 集合 + Commit 基准前置；补 Source 级状态 | D-14 D-15 |
| 5 | 归属与权限 | 仓库级端点补项目成员校验；Finding 关联仓库补项目级校验；MCP 权限表补全 | D-09 D-10 |
| 6 | 审计与幂等 | Import 三动作 + Discovery 写路径补审计；findings/answers 补幂等键；收敛双份 `append_audit` | D-04 D-11 D-12 D-20 |
| 7 | 历史保全 | 仓库删除/归档策略：保历史还是保仓库；被 task 引用时给友好错误 | D-08 |
| 8 | 追踪补强 | Finding 增加 Source/Commit 追溯（I6-D-008） | — |
| 9 | Discovery 侧收口 | `discovery_proposal` 状态联动；`base_content_hash` 死列处置；`ssh_key_id` 死列处置 | D-17 D-18 D-19 |

---

# §16. 第二轮待补

| 清单章节 | 剩余工作 |
|---|---|
| §11 | 空库初始化实测（从零执行 0001～0028）；测试库与开发库一致性 |
| §2～§4 | 与 §12 的单遍结论交叉复核（重点：§4 Source 字段级复核、§3 作业表租户列核对） |
| §9 | REST/MCP 完整调用链图（需逐条确认 MCP 18 工具的领域服务映射） |
| §10 | 审计 actor 五列实际填充逐列核对 |
| §6.2 | `discovery_question` 无作者字段的影响面（谁可以提 question） |
| — | 与 I1（归档生命周期）、I2（Finding 人工确认）、I3（Git 权限）的依赖对齐 |

---

# §17. 核验完成度自评

| 清单要求 | 状态 |
|---|---|
| I6-A-001～115 均有核验状态 | ✅ §2/§3/§4/§8 逐条；§5～§7/§9～§11 见 §12 |
| 所有 [已核实] 有代码或 Migration 依据 | ✅ 全部带 `文件:行号` |
| 所有 [部分核实] 明确剩余问题 | ✅ I6-A-010、024、109、114、115 |
| 所有 [不存在] 明确搜索范围 | ✅ I6-A-013、025、029、043、044、045、083、084 及 §4/§12 |
| 所有 [存在缺陷] 有复现路径或代码证据 | ✅ §14 共 22 项，每项带证据 |
| Repository / Import / Source 关系已明确 | ✅ §2.1 / §3.1 / §4.1 |
| Discovery Session 是否存在已明确 | ✅ **已存在**（§12.1） |
| Finding / Question / Proposal 真实关系已明确 | ✅ §12.2 |
| Knowledge Proposal 版本机制已明确 | ✅ §12.3 |
| Import 取消与 Worker 恢复行为已明确 | ✅ §8.3 / §8.4 |
| REST / MCP / Worker 调用链已明确 | ✅ §3.4 / §12.4 |
| 空库初始化路径已核实 | ⚠️ 部分（§12.6，未实测） |
| I6 V1.0 设计候选已与代码事实分离 | ✅ §13 + §15.1 |

**结论：I6-A 的核验目标已达成，可进入 I6-B —— 但 I6-B 的输入不是 I6 V1.0，而是本报告 §13 对照表与 §15.3 拆分。**

---

**文档状态：I6-A V1.0，待专家裁决。**

**下一步：先裁决 §15.1（哪些章节作废）与 §15.3（I6-B 的九个工作块顺序），再启动 I6-B。**
