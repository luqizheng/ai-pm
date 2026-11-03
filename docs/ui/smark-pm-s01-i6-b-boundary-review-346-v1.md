# S01 · I6-B §3 / §4+§5 / §6 边界评审（冻结 Migration 前）

> 文档性质：数据模型与事务边界评审
> 评审对象：I6-B V1.0 §3（初始化操作归属）、§4～§5（Source/Attempt 分层）、§6（Evidence 外键）
> 输入基线：I6-A 报告 V1.0（含勘误）+ 专家裁决（C-1～C-4、N-1～N-5，2026-11-03）
> 代码基线：`master aaee6e5`，migrations 0001～0028
> 版本：V1.0
> 状态：待专家确认后并入 I6-B V1.1，随后冻结 Migration

---

## §0. 总评

| 边界 | 判定 | 一句话结论 |
|---|---|---|
| §3 初始化操作归属 | ✅ 可行，附 4 条强制细节 | 生产写入点已穷举，`initializing` 的生产入口**只有一个**（`start_import`），操作归属表首期单一 Producer，成立 |
| §4～§5 Source/Attempt 分层 | ⚠️ 方向正确，**有一处分层漏洞** | Attempt 表不含 facts/材料快照，而 Source 层的 `facts JSONB`/计数列现被重跑覆盖——不补归属，"重试覆盖首次执行信息"原样保留 |
| §6 Evidence 外键 | ⚠️ **发现一个硬阻塞** | `repo_import_source` 没有 `(tenant_id, import_id, repository_id)` 唯一键（PK 仍是 0009 两列式），Evidence/Attempt 的组合 FK **在现状 Schema 上建不出来**；另有跨项目不变量 FK 无法表达，需拍板 |

以下逐节展开。R- 编号为并入 V1.1 的强制修订项；D- 编号待专家裁决。

---

## §1. §3 评审：Project 初始化操作归属

### 1.1 `core.project.status` 写入点全景（穷举，生产代码）

| # | 位置 | 行为 | 生产/测试 |
|---|---|---|---|
| W-1 | `bootstrap.rs:264` | INSERT，status ∈ {`planning`,`initializing`} | 生产入口 `POST /v1/pm/projects`（`pm_lists.rs:182-191`）**恒传 None → `planning`** |
| W-2 | `bootstrap.rs:103` `bootstrap_project_with_status(Some("initializing"))` | 同上但可出生即 `initializing` | **唯一调用方是测试** `apps/pm-api/tests/imports.rs:67`；生产零调用（已 grep 穷举） |
| W-3 | `lifecycle.rs:127` | `UPDATE ... SET status='initializing' WHERE id=$1 AND status='planning'`（**不检查影响行数**） | 生产：`start_import` 事务内 |
| W-4 | `advance.rs:73` | `UPDATE ... SET status='planning'`（**无守卫**） | 生产：`advance_import_after_decision` 全 executed 分支 |

**结论：生产环境中把 Project 置入/置出 `initializing` 的入口各只有一个（W-3 / W-4）。** §3 的操作归属表首期只有 `repo_import` 一个 operation_type、一个 Producer。设计成立，且比设计稿预想的简单——不存在"其他初始化操作所有者"需要同时建模（裁决 §10 I6-B-02/V-08 的答案：当前没有第二个）。

### 1.2 强制修订项

**R-3-1 · 占有与释放都必须检查影响行数。**
现状 `lifecycle.rs:127` 带 `AND status='planning'` 守卫但不看 `rows_affected`——命中 0 行时 Import 照常创建、Project 却不被占有，这正是"无主 initializing"的成因之一。V1.1 必须写明：占有 UPDATE（或 operation INSERT）成功是 Import 创建事务的**提交前置条件**，0 行即整体回滚。

**R-3-2 · 测试专用入口不得绕过归属。**
`bootstrap_project_with_status(Some("initializing"))`（W-2）出生即 `initializing` 却不产生任何操作记录。V1.1 二选一：①该变体同事务补插 operation 行（`operation_type='bootstrap'`）；②删除该变体，测试 seeding 改走 `start_import` 真路径。推荐 ②（少一条特殊路径，测试更接近生产）。

**R-3-3 · "同 Project 至多一个 active 初始化操作"必须 DB 化。**
与裁决 §2.3 对 Import 名额的口径一致：操作表加部分唯一索引 `ON core.project_initialization_operation(project_id) WHERE status='active'`。应用层检查只做友好报错。这也让 §3.3 中"是否允许多 active 由上层决定"的悬念在 V1 直接落死为"不允许"，与 Q1-A 单活动 Import 自洽。

**R-3-4 · generation 落点：推荐操作表权威 + Project 行锁，不加 `core.project` 列。**
理由：①`core.project` 现无 generation 列，加列波及面大；②释放路径已在单事务内 `FOR UPDATE` 锁 Project 行（裁决 §3.5 第 1 步），行锁 + `AND status='initializing'` 条件 UPDATE 已足以防"迟到释放覆盖新操作"——旧操作的事务拿不到行锁或条件不满足即 0 行放弃；③generation 语义由"active 操作行唯一"（R-3-3）+ 行锁共同表达。若未来出现第二种初始化操作再评估升列。V1.1 应把 §3.4 的"是否放入 core.project 待核验"直接裁掉。

### 1.3 事务边界判定

裁决 §3.5 的释放流程（锁 Project → 锁操作 → 终态化 → 查余量 → 条件释放 → 审计 → 提交）为**单事务**，与现状 `advance.rs:64-95`（applied 分支已是单事务）同构，可行，无跨服务协调需求。✅

---

## §2. §4～§5 评审：Source 固定 Commit 与 Attempt 历史的分层

### 2.1 分层漏洞（专家点名的边界，现状会原样穿透）

I6-B §4.9 的 Attempt 表字段是 `attempt_no / status / commit_sha / started_at / finished_at / error_code / error_detail / job_id`——**不含 facts 与采集结果**。

而现状的**每轮执行结果**挂在 Source 行上：`facts JSONB`、`commit_count`、`doc_count`、`size_bytes`、`truncated`、`error_code`，且重跑时被 `ON CONFLICT (import_id, repository_id) DO UPDATE` **整行覆盖**（`lifecycle.rs:400-403`，成功还清空 `error_code`）。

若 V1.1 只给 Source 加状态、建 Attempt 表，但不挪结果字段：首轮的 facts 在重试成功瞬间被覆盖，`repo_import_source` 上看不到"第一次失败时采集到什么"——**"重试覆盖首次执行信息"的缺陷换个位置继续存在**。

### 2.2 字段归属表（V1.1 直接采用）

| 层 | 字段 | 语义 |
|---|---|---|
| **Source（身份 + 基准 + 当前态）** | `import_id, repository_id, tenant_id` | 身份（见 §3 R-6-1 键约束） |
| | `sequence_no` | 串行序 |
| | `requested_branch` | 创建时确定 |
| | `head_commit_sha` + `commit_pinned_at` | **write-once 基准**（裁决 N-1） |
| | `status`, `attempt_count`, `last_error_code/detail`, `started_at/completed_at`（最近轮） | 当前态指针 |
| **Attempt（每轮事实，只增不改）** | `attempt_no, status, started_at, finished_at` | 执行历史 |
| | `commit_sha`（=Source 基准，执行事实核验值） | |
| | `error_code, error_detail, job_id` | |
| | **`facts JSONB, commit_count, doc_count, size_bytes, truncated`** | **从 Source 层迁入**；成功材料与失败现场都按轮留档 |

草稿生成读"最新 succeeded Attempt 的 facts"；`source_succeeded` 等汇总走聚合查询（设计稿 §4.11 已允许不落盘）。旧 Source 列在迁移中删除或停写（开发期无存量包袱，直接删列）。

### 2.3 规则空缺：重试成功后草稿怎么办（D-1）

场景链（全部合法可达）：partial 成功 → Import `draft_ready`（用成功 Source 材料生成草案）→ 用户按裁决 §1.4 重试失败 Source → 重试**成功** → 材料集变化。

此时旧草案缺新仓库内容。V1.0/裁决均未定义。两个选项：

- **D-1a（推荐）**：`draft_ready` 仍允许重试；重试成功后**重生成草案**（`draft_revision + 1`，旧草案不覆盖、可追溯）；`proposed` 起**禁止**重试（只能走 C-2 终止路径后重来，避免审批与材料漂移）。
- D-1b：`draft_ready` 即禁止重试，partial 场景必须先 cancel 整个 Import。

推荐 a：与"一次性 bootstrap + Source 级恢复"（裁决 §1.4）语义连贯，且不产生第二个 Import。需 V1.1 写入 Import 状态机（§13.1 `draft_ready` 行）与验收。

### 2.4 N-1 落地确认

pin 协议无需额外 git 往返：clone 后 `rev-parse <branch>` 得 SHA → 持久化（write-once）→ `collect` 全部读改用 `<sha>` 基准（`repo_facts.rs:154-275` 四类读均为参数级替换）→ Attempt 侧另以 `rev-parse <sha>` 验证存在性作为重试前置校验（裁决 §9.2 第 5 步）。✅ 与裁决 §5 完全一致。

---

## §3. §6 评审：Evidence 外键与归属约束

### 3.1 硬阻塞：source 缺租户组合唯一键（R-6-1）

**代码事实**：`repo_import_source` 的主键仍是 0009:73 的 `PRIMARY KEY (import_id, repository_id)`；0019 只加了 `tenant_id` **列**和两个组合 FK（`fk_ris_tenant_import` / `fk_ris_tenant_repository`，0019:380-386），**从未加过 `UNIQUE (tenant_id, import_id, repository_id)`**（全迁移 grep 零命中）。

**后果**：PG 组合外键要求被引用列集上存在唯一约束/索引。因此 I6-B §6.4 的 Evidence→Source 组合 FK、§4.9 的 Attempt→Source 组合 FK，**在现状 Schema 上都建不出来**。

**修订（并入 I6-B-08 Migration）**：新迁移为 source 增补
```sql
ALTER TABLE integration.repo_import_source
    ADD CONSTRAINT uq_repo_import_source_tenant_full
    UNIQUE (tenant_id, import_id, repository_id);
```
（或直接把 PK 升为三列；推荐前者——PK 语义不变、改动面小，Attempt/Evidence 引用新唯一键。）此项是 §6 设计成立的**前置条件**，缺它整个 Evidence FK 方案落空。

### 3.2 跨项目不变量：FK 表达不了，需拍板（D-2）

Evidence 的两条 FK 各自只锚 `tenant_id`：`(tenant_id, finding_id)` 锚 Finding、`(tenant_id, import_id, repository_id)` 锚 Source。"**Finding 的项目 == Source 所属 Import 的项目**"是跨两个父表的不变量，**任何 FK 组合都无法表达**（同租户跨项目引用在纯 FK 下合法）。专家要求"不能只靠应用层校验"，则只有数据库级一档：

- **D-2a（DB 级，代价=冗余链条）**：`discovery_finding` 加 `project_id` 列（自 session 冗余）+ `UNIQUE (tenant_id, project_id, id)`；`repo_import` 补 `UNIQUE (tenant_id, project_id, id)`（project_id 已有列）；Evidence 加 `project_id NOT NULL`，两条 FK 全部带上 project_id → 同项目由 DB 保证。改动：3 列 + 3 唯一键 + 回填（0025 已在测试库执行，开发期回填自 session 即可）。
- **D-2b（事务内应用层）**：FK 只保 tenant + 存在性；同项目校验放 Finding+Evidence 同事务（复用 `finding.rs:100-114` 现有归属校验模式）+ 跨项目用例锁进验收。

**推荐 D-2a**：专家口径明确要 DB 级；且 I6-A D-09 已证实同租户跨项目引用是本仓**现实存在的漏洞模式**（仓库端点、finding 关联两处同病），借 Evidence 建表一次性把 discovery 侧的 project 归属锚死，顺带修 D-09 的 finding 侧。

### 3.3 判别约束（R-6-2）

裁决/设计 §6.7 允许非代码证据（无 Source），§6.4 又把 `import_id / repository_id` 列为必填——两者矛盾未消。V1.1 迁移必须加 CHECK 判别：

```sql
CHECK (
  (evidence_type IN ('file','symbol','code_range')
     AND import_id IS NOT NULL AND repository_id IS NOT NULL AND commit_sha IS NOT NULL
     AND source_attempt_id IS NOT NULL)
  OR
  (evidence_type NOT IN ('file','symbol','code_range')
     AND import_id IS NULL AND repository_id IS NULL AND commit_sha IS NULL
     AND source_attempt_id IS NULL)
)
```

非代码来源用 `evidence_type`（如 `user_statement / system_observation`）+ 现有业务来源关系表达，不伪造 Commit（与设计 §6.5 末段一致）。

### 3.4 commit_sha 一致性边界

`Evidence.commit_sha == Source.head_commit_sha` 是非键值相等，FK/CHECK 均无法表达。裁决 §6.5 已定性为"快照字段，创建时从 Source 复制并校验"——落地为：Evidence 创建事务内校验（§6.6 第 8 步），`source_attempt_id` 必填（code 类）使证据可锚定到执行轮，attempt 的 `commit_sha` 提供第二个核验源。接受此边界，不需触发器。✅

### 3.5 事务边界判定

Finding + Finding↔Repository 关联 + Evidence + 审计同事务创建（§6.6）——与现状 `add_finding` 单事务模式（`finding.rs:131-141`）同构，可行。✅ 旧 `discovery_finding.evidence` JSONB 按裁决 §6.8 只读保留、不回填不伪造，✅。

---

## §4. 并入 I6-B V1.1 的合并清单

**来自裁决（12 条，全部收编）**：C-1（一次性语义 + I6-006 改 409）、C-2（proposed 终止进 I6-B-01 同批）、C-3（新迁移 + 环境清单）、C-4（RESTRICT + 错误映射）、N-1（SHA 固定 + 措辞改"按固定 SHA 读取"）、N-2（fail-fast + 冻结终态化 + D-03 定级 P1）、N-3（repository_ids 必填、空数组拒绝、REST/MCP 同契约）、N-4（幂等透传客户端键）、N-5（lease 恢复 + Attempt 六态）、任务范围调整（§10）、验收补充 I6-B-036～051（§11）。

**来自本评审（新增）**：

| 编号 | 内容 | 归入任务（按裁决 §10 新编号） |
|---|---|---|
| R-3-1 | 占有/释放必须检查影响行数，0 行即回滚 | I6-B-01 |
| R-3-2 | `bootstrap_project_with_status(initializing)` 删除或纳入归属（推荐删，测试走真路径） | I6-B-01 |
| R-3-3 | 操作表 `active` 部分唯一索引（单 active DB 化） | I6-B-01 / I6-B-08 |
| R-3-4 | generation 落点=操作表权威 + Project 行锁，不加 project 列 | I6-B-01 |
| R-4-1 | facts/计数/错误字段从 Source 层迁 Attempt 层（§2.2 归属表） | I6-B-02 |
| R-6-1 | source 补 `UNIQUE (tenant_id, import_id, repository_id)`——Evidence/Attempt 组合 FK 的前置条件 | I6-B-08（**阻塞项**） |
| R-6-2 | Evidence 代码/非代码 CHECK 判别器 | I6-B-04 / I6-B-08 |

**待专家裁决（2 项）**：

| 编号 | 问题 | 推荐 |
|---|---|---|
| D-1 | 重试成功后草稿处理 | D-1a：draft_ready 可重试、成功后重生成（draft_revision+1）；proposed 起禁止重试 |
| D-2 | Evidence 同项目约束实现档位 | D-2a：DB 级（finding/import/evidence 三处 project_id + 组合唯一键），顺带修 I6-A D-09 |

---

## §5. 冻结 Migration 的门槛

满足以下三条即可冻结：

1. 裁决 12 条 + 本评审 R-3-1～R-6-2 并入 V1.1 正文（裁决 §12 要求，不做附录）；
2. D-1、D-2 拍板（影响状态机一行 + 三张表列集）；
3. R-6-1 进入迁移清单首项（它是 §4/§6 全部组合 FK 的前置）。

---

**文档状态：V1.0，待专家确认 D-1/D-2 后并入 I6-B V1.1。**
