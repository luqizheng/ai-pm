# S01 · I6-B V1.0 代码核验反馈

> 文档性质：对《I6-B Discovery 代码分析承接能力增强设计稿 V1.0》的代码事实核验反馈
> 核验对象：I6-B V1.0 全部代码事实引用与设计-代码一致性
> 核验基线：`D:\kudi-project\templates\ai-pm`（master `aaee6e5`），I6-A 报告 V1.0（含勘误）
> 版本：V1.0
> 状态：待反馈专家

---

## §0. 总评

1. **三项裁决（Q1-A 独占 / Q2-A 串行+失败隔离 / Q3-A Evidence 独立表）与代码事实无冲突，可实施。**
2. I6-B §0.4 引用的代码事实中，**一条需要修正**（源自 I6-A 的错误，已在我方报告勘误）：`bootstrap_project` **会**创建 `ai.project_ai_config`（`bootstrap.rs:288-297`，`ai_lead`）。V-07 的疑虑收窄为"冻结发生在飞 Import 无终态化"。
3. **四处设计-代码冲突需要专家裁决**（§2，C-1～C-4），其中 C-1/C-2 直接影响 I6-B-01 的验收行，建议先裁再动。
4. 五条实施注记（§3）不改变设计结论，供落地时直接采用。

---

## §1. I6-B §0.4 代码事实逐条核对

| I6-B 断言 | 核对结果 |
|---|---|
| `discovery_session` / `discovery_finding_repository` / `repo_import.discovery_session_id` 已存在 | ✅ `0025:12-26/67-76/139-143` |
| `repo_import` Project 级、`repo_import_source` 关联 | ✅ |
| Source 无独立状态；串行但首个失败终止整体 | ✅ `lifecycle.rs:350-419`，失败即 `fail_import` + `return` |
| Commit 基准采集后才落库、重跑覆盖 | ✅ `lifecycle.rs:396-414` `ON CONFLICT DO UPDATE` |
| Finding 只能关联 Repository、无 Source/Commit 追溯 | ✅ |
| Knowledge 写入协调机制已存在 | ✅ `knowledge_write_coordinator.rs` |
| REST 已有、MCP 缺 Repository/Import 工具 | ✅ 18 个 MCP 工具零命中 |
| 取消覆盖 / initializing 泄漏 / 审计幂等缺失 | ✅（见 I6-A §8/§12/§14） |
| "Bootstrap 不一定创建该配置行"（§0.4/§8.1） | ❌ **修正**：`bootstrap.rs:288-297` 必建（`ai_lead`、enabled）。D-03 降级为 P1，详见勘误 |

---

## §2. 设计-代码冲突（需裁决）

### C-1 · I6-006 验收行与 `ProjectNotEmpty` 门互斥 【P0，阻塞 I6-B-01 验收】

- **代码事实**：`start_import` 要求 `delivery.requirement` 计数为 0（`lifecycle.rs:50-57`）。apply 执行成功后 requirements 落库 → 计数 > 0 → **applied 之后永远无法再创建 Import**（409 `project_not_empty`，与名额无关）。
- **冲突**：验收行 `I6-006「第一个 Import applied 后创建 → 成功」`在当前代码下必失败。冷启动导入是**一次性 bootstrap**，不是可重复流程。
- **需裁决**：二选一——
  - A. 保持一次性语义：I6-006 改为「applied 后创建 → 409 `project_not_empty`」；"重新分析"明确为未来能力（需先裁决 requirement 清理/新项目流程）。
  - B. 支持重复分析：放宽 `ProjectNotEmpty` 门，但必须同时定义旧 requirements/proposals 与新 Import 的关系（影响面大，不建议进 V1.0）。

### C-2 · Q1-A 扩大名额 + 取消不支持 `proposed` = 新的永久锁死 【P0，I6-B-01 范围必须纳入】

- **代码事实**：`cancel_import` 只允许 `IN_PROGRESS ∪ {draft_ready}`（`lifecycle.rs:241-243`）。而 `advance.rs:98-107` 的部分失败路径会把 Import **打回 `proposed`**（`error_code='partial'`）。
- **冲突**：Q1-A 把 `proposed` 纳入占用名额后，卡在 `proposed` 的 Import（partial 是现实可达路径）**没有任何终止手段** → 名额永久占用，项目再也无法导入。I6-B §13.1 把 `proposed → cancelled` 标为"仅符合业务规则时"，太弱。
- **建议**：I6-B-01 范围强制加入"`proposed` 的显式终止/放弃操作"（含审批请求撤销语义联动），作为扩大唯一索引的**前置同批**改动，否则新索引上线即引入回归。

### C-3 · 索引变更必须走新迁移，不能原地改 0009 【P1】

- **代码事实**：0001～0028 已在 192.168.2.10 测试服务器与本地开发库执行（栈已部署，见 `docs/runbook-test-server-deploy.md`）。
- **含义**：0009 属"已共享 Migration"。按 I6-B §12.1 自身原则，应新增迁移：`DROP INDEX repo_import_one_in_progress` + 建 `repo_import_one_active_per_project`。请在 I6-B-09 明确"已共享"的环境清单，避免执行机原地改 0009 后与服务器库漂移。

### C-4 · §11"有历史引用即拒绝 DELETE"需要 FK 层配合 【P1，I6-B-08 范围】

- **代码事实**：`repo_import_source.repository_id`（`0019:385`）与 `discovery_finding_repository.repository_id`（`0025:74-75`）均为 `ON DELETE CASCADE`——当前删除**静默抹历史**，不报错。
- **需裁决**：实现"拒绝"的两条路——
  - A. FK 改 `ON DELETE RESTRICT`（新迁移重建两约束）+ 错误映射为领域错误（数据库兜底，推荐）；
  - B. 仅应用层预检（依赖每个入口都记得检查，弱）。
  注意 `delivery.task_repository` 等已是 NO ACTION（删除被拒但报 500，`pm_repositories.rs:95-98`），无论选哪条都应一并把 500 修为明确错误。

---

## §3. 实施注记（不改设计结论）

### N-1 · V-10 的答案：`head_commit_sha` 可以承担基准，改法是机械替换

单次运行内一致性**已经成立**：`repo_facts::collect` 全部 git 读（`rev-parse` / `log` / `ls-tree` / `show branch:path`，`repo_facts.rs:154-275`）都在**同一份新鲜 bare clone** 上按 branch 相对执行，clone 时点即快照。Pin 的落地 = ①先解析 SHA（clone 前 `ls_remote` 或 clone 后 `rev-parse`）→ ②持久化 → ③`collect` 的读基准从 `branch` 换成 `<sha>`（参数级替换）→ ④`head_commit_sha` 加 write-once 语义（I6-A 已证 `ON CONFLICT DO UPDATE` 会覆盖，`lifecycle.rs:400-403`）。另：bare 仓库没有 checkout，§4.6 的"checkout 固定 SHA"措辞应改"按固定 SHA 读取"。

### N-2 · V-07 / D-03 勘误：冻结语义是有意的，缺陷在"冻结时的在飞处理"

`job_queue.rs:11-13` 注释明确 claim 过滤就是冻结语义（frozen / controlled 不派发）。`bootstrap.rs:288-297` 必建 `ai_lead` 配置。剩余缺陷：项目**事后**切 `controlled` / `enabled=false` 时，已 `queued` 的 Import 无人认领也永不终态。修法建议（与 I6-B §8.2 的 A/B 之外补第三条）：**start_import 创建时 fail-fast 校验配置** + **冻结/归档时终态化在飞 Import**（与 I1 §9.2 对齐），不要删 claim 条件。

### N-3 · `repository_ids` 是新 API 面

现 `POST /imports` **无请求体**（`pm_imports.rs:136-149`）。§4.3 落地时需定义：空数组=拒绝；不带该字段=报错还是过渡期沿用"全量 verified"语义（建议直接报错，开发期无兼容包袱，与 §12.1 原则一致）。

### N-4 · Import 幂等可直接复用 inbound_event 约束

`integration.inbound_event` 已有 `UNIQUE(project_id, source, idempotency_key)`（`0003:33`）。现状是自生成 UUID 当键（`lifecycle.rs:88,102`）。改为**透传客户端幂等键**即得 §10.3 的"相同键返回原 Import"（冲突时反查原 Import 返回），不必新建台账。

### N-5 · Worker 恢复靠 lease 回收，不靠 attempts

`run_import_job` 的失败路径一律 `Ok(())` → `job_queue::complete`（`ai-pm-worker/src/main.rs:230-235`），即 **git/模型失败不触发 job 级重试**；真正的崩溃恢复是 claim 即回收（`status='running' AND lease_until < now()`，`job_queue.rs:52-57`）。I6-B-04 的"Worker 崩溃后可恢复"应明确建立在 **lease 过期重认领** 上；Source Attempt 恢复判定也以 lease/Attempt 过期为准，不要依赖 `attempts` 退避。

---

## §4. 其余核对确认（无冲突）

- §2.2 状态全集与 0009 CHECK 完全一致 ✅
- §2.3 部分唯一索引语法可行（PG partial index + IN 谓词）✅
- §4.7/§13.2 Source 状态机为纯新增（现无该字段），无既有语义冲突 ✅
- §6.4 "Source 用 (import_id, repository_id) 组合主键、Evidence 不引入伪 UUID"与 0009 PK 一致 ✅
- §7.1/§7.2 Session 枚举与 FK 方向与 0025 一致 ✅
- §9.1 REST 能力清单与实际路由一致 ✅
- §11 归档优先与现有 `archive_repository`（`repo.rs:179-215`）衔接顺畅（新 Import 只选 verified 已是现状）✅
- §15 缺陷映射表与 I6-A D-01～D-22 一一对应（D-03 优先级随勘误降 P1，处理方式见 N-2）✅
- 任务顺序 I6-B-01→09 认可；唯一要求：**C-2 并入 I6-B-01、C-4 并入 I6-B-08** ✅（待确认）

---

## §5. 请专家回复的决策点

| 编号 | 决策点 | 建议 |
|---|---|---|
| C-1 | applied 后能否再创建 Import（I6-006 去留） | 选 A：保持一次性，改验收行 |
| C-2 | `proposed` 终止操作进 I6-B-01 是否强制 | 强制 |
| C-3 | 0009 修改走新迁移 + 明确"已共享"环境清单 | 是 |
| C-4 | 删除拒绝用 FK RESTRICT 还是应用层预检 | RESTRICT + 错误映射 |
| N-3 | `repository_ids` 缺省行为 | 缺字段报错 |

---

**文档状态：V1.0，待反馈专家。**
