# AI-PM 平台：Slice D2 MCP 任务上下文契约（V1 · 设计稿）

> 面向：技术评审方 / 实施方（含 Codex 执行端）
> 日期：2026-11-01
> 状态：**设计依据 v1（已裁决 A–D，可实施）**（本文只定义 D2 的实施范围；D3–D5 仅作范围锚定）
> 前置基线：`f644696`（D1 run dispatch core）、`5921eee`（D1 验收补齐）、`92c1f26`（R5 多级审批）、`7c1bbf4`（0012 事件级幂等）
> 上游依据：`docs/ai-pm-slice-d-execution-pipeline-v1.md`（§八 D2 范围锚定、§九 D2 留口）、`docs/ai-pm-mcp-executor-positioning-v1.md`（§四 §五）、`docs/ai-pm-multi-repo-model-v1.md`（§6 契约扩展）

---

## 〇、一句话结论

**D1 让「Run 被领取、基线被锁定」；D2 要让「执行端只凭一次领取，就能拿到足以独立完成任务的完整契约」。**

D2 不是新增一个读接口，而是把 `delivery.task` 从「标题 + 描述」升级为**可执行契约**（目标 / 约束 / 逐条验收 / 产物路径 / 知识引用 / 每仓基线 / 租约）。当前库中这七类字段**一项都不存在**，因此 D2 的主体是**一次迁移 + 一个契约读模型 + 一条续租通道**，而不是工具层包装。

D2 落地前的三个硬缺口：

| # | 缺口 | 严重度 | 影响 |
| --- | --- | --- | --- |
| G1 | 契约七类字段零存储（`grep acceptance\|criteria\|deliverable migrations/` 仅命中注释） | **P0** | `get_context` 无法返回契约；D5 无逐条验收载体 |
| G2 | 无 run 级凭据：`/mcp` 只有粗粒度 `PM_MCP_READ`，`pm_get_task_context` 仅凭 `project_id + task_ref` 可读任意任务 | **P0** | 执行端越权读取；无「持有本次租约」的证明物，fencing 缺输入 |
| G3 | 领取语义未定：`claim_once` 是「领任意一条候选」，执行端需要「领指定 task」 | **P0** | 契约的 `lease` 字段无来源；D3 无法确定进程归属 |

---

## 一、阶段目标与判定标准

### 1.1 D1 → D2 的能力跃迁

| 维度 | D1 交付后 | D2 目标 |
| --- | --- | --- |
| 任务 | 13 态状态机 + `task_repository` 声明 | 任务携带**可执行契约**（目标/约束/验收/产物/知识） |
| Run | 被原子领取、基线快照落 `task_run_repository` | 执行端可凭租约**读取本 Run 的契约快照** |
| 租约 | 平台侧签发、内部 `renew_lease()` | 对外可续租通道（续租即心跳） |
| 执行端 | 无接入点 | `claim` → `get_run_context` → `report_progress` 三工具可闭合 |
| 权限 | 粗粒度 `PM_MCP_READ` | **run 级能力**：凭 `lease_token` 只能读到自己持有的那一次 Run |

### 1.2 阶段判定（三条都要成立）

1. **契约完整**：`pm_get_run_context` 返回的 JSON 通过 schema 校验，且**每个字段都有库内来源**（文档给出字段 → 列/表 的映射表，无「拼装字段」）。
2. **租约可信**：无有效 `lease_token` 一律拒绝；租约过期后旧 token 立即失效（T6/T7）；`pm_report_progress` 续租后旧到期时间被前移（T8）。
3. **不越界**：D2 **不写** commit / 交付清单（D4），**不启动**进程（D3），**不裁决**任务状态（D5）。`pm_submit_result` 只冻结 schema，不落库。

### 1.3 本阶段明确不做

| 不做 | 理由 |
| --- | --- |
| `pm_submit_result` 的实际落库 | 归 D4（`task_repository_commit` / 交付清单） |
| `task.request_decision` / `knowledge.propose_change` | 依赖 C 级审批编排，归 D5 |
| 完整策略合成（能力类 AND / 约束类累加） | D1 §九 #1 留口，需策略引擎类型化，另立阶段 |
| 进程启动与终止 | 归 D3 |
| 前端执行中心改造 | 数据源已具备（`GET /v1/pm/task-runs`） |

---

## 二、现状盘点（已逐项核实）

### 2.1 可复用（不重建）

| 能力 | 位置 | 复用方式 |
| --- | --- | --- |
| MCP 服务壳（rmcp streamable http，`/mcp` 路由） | `crates/pm-mcp/src/server.rs`、`apps/pm-api/src/routes.rs:94-150` | 追加 `#[tool]` 方法，不换 transport |
| 工具函数层与错误契约（`ToolError` → `isError` 结果） | `crates/pm-mcp/src/tools.rs:87-97` | 沿用；新增 `Forbidden` / `LeaseExpired` 两个变体 |
| 权限门（`PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE`） | `apps/pm-api/src/auth.rs:75` | 保留为**第一层**，D2 在其内叠加 run 级校验 |
| 领取事务模板（六步 + 锁序 run→task） | `crates/pm-application/src/run_dispatch.rs:307-550` | 抽出「按 task_id 筛选候选」参数，**不复制事务体** |
| 多仓基线快照（`task_run_repository`） | 同上 `snapshot_repositories:744` | 契约 `repositories[]` 的唯一来源 |
| 续租（`renew_lease`） | 同上 `:892` | 由 `pm_report_progress` 调用 |
| fencing 校验 | 同上 `accept_executor_result:913` | D2 扩展为凭 `lease_token` 校验 |
| 知识文档身份（`ai.project_document` + `document_revision`） | `migrations/0006_knowledge_documents.sql:7-40` | `knowledge_refs[]` 的来源；正文在 Git，不在库 |
| `core.principal.kind` 已含 `service` | `migrations/0001_core.sql:33-35` | 直接用于补齐 D1 §九 #6（审计无 human 成员时降级） |

### 2.2 缺口清单

| # | 缺口 | 证据 | 严重度 | D2 处置 |
| --- | --- | --- | --- | --- |
| G1 | `goal` / `constraints` / `acceptance_criteria` / `deliverables` 无任何列或表 | 全量 `grep` 13 个迁移零命中 | **P0** | `0014` 扩列 + 1 张验收项表 + 1 张知识引引用表 |
| G2 | 无 run 级凭据；`lease_owner` 是 worker 标识字符串，不可作凭证 | `run_dispatch.rs:492`、`auth.rs:75` | **P0** | `task_run` 增 `lease_token_hash` / `client_name` / `client_version` |
| G3 | `claim_once` 只能「领任意候选」，无「领指定 task」入口 | `run_dispatch.rs:316-331` | **P0** | 新增 `claim_task_run(project, task, client)`，复用同一事务模板 |
| G4 | 现有 `pm_get_task_context` 仅返回 `task_id/title/status/revision/requirement/milestone` | `tools.rs:241-258`、`read.rs:36-71` | P1 | **保留为摘要工具**，新增 `pm_get_run_context` 作执行契约（见 §3-B 边界） |
| G5 | `policy_snapshot.execution_enabled` 在代码中为字面量 `true` | `run_dispatch.rs:510` | P1 | D2 收口：策略读取失败 → `execution_enabled=false` 并落快照（S3 完整化） |
| G6 | 审计 `actor` 三级回退只认 `human`，项目无 human 成员时 `RowNotFound` 导致领取失败 | `run_dispatch.rs:937-958` | P1 | 回退链追加 `kind='service'`（`0014` 不建表，仅代码） |
| G7 | 无执行端身份字段（`client_name` / `client_version`） | `ask` 契约 §4.4 方案 C 预留 | P2 | 随 G2 一并补列，V1 只记录不据此分档 |

### 2.3 口径冲突（先对齐，否则 D4 返工）

| # | 冲突 | 现状（权威） | 文档草案 | 处置 |
| --- | --- | --- | --- | --- |
| C1 | 契约里 `git.branch` 由谁定 | 无字段 | positioning §五示例 `task/TASK-001` | **平台生成**，格式 `task/<task_id 前 8 位>`，写入 `delivery.task.git_branch`，执行端不得自定（与 R6 `workspace_path` 同一原则） |
| C2 | `knowledge_refs` 是路径还是 id | `ai.project_document.canonical_path` 为权威路径 | positioning §五示例为路径字符串 | 契约同时给 `document_id` + `canonical_path`，**以 id 为准**（路径可因重命名变化） |
| C3 | 契约 `type` 取值 | `delivery.task.task_type` 为 TEXT 无约束（0013） | positioning 示例 `architecture` | D2 **不引入枚举约束**，原样透传 `task_type`；取值规范化留策略阶段 |

---

## 三、裁决记录（本轮 4 项，已确认）

| # | 议题 | 裁决 | 影响面 |
| --- | --- | --- | --- |
| **A** | 领取归属 | **A 为主（执行端 `pm_claim_task` 领指定任务）+ 保留 B（worker `claim_once` 供无人值守）**。两者**复用同一事务模板**，仅候选筛选条件不同 | `run_dispatch.rs` 抽参、新增 MCP 工具 |
| **B** | 工具命名 | **沿用 `pm_*` 下划线**。`task.*` 仅作文档语义名，另附映射表；现有 4 工具零破坏 | 契约文档 |
| **C** | 验收标准载体 | **独立表逐条留痕**（`delivery.task_acceptance_criterion`），D5 可逐条判定并存证据 | `0014` 建表 |
| **D** | 上下文鉴权 | **run 级 `lease_token`（sha256 入库，明文只返回一次）**；`get_run_context` 只读不续租，续租仅发生在 `report_progress` | `0014` 补列 + 工具实现 |

> A 的关键推论：**`lease_owner` 的语义从「worker 标识」变为「租约持有者标识」**。Model B 下是 `ai-pm-worker-<uuid>`；Model A 下是 `mcp:<client_name>:<lease_token 前 8 位>`。两者共用同一列，但**不可作为凭证**——凭证是 `lease_token`。这条必须在代码注释中固化，否则 D3 会误用 `lease_owner` 做 fencing。

---

## 四、契约定义

### 4.1 `pm_get_run_context` 返回体（V1）

```json
{
  "contract_version": 1,
  "run": {
    "run_id": "…",
    "attempt_no": 2,
    "stage": "dispatch",
    "timeout_seconds": 1800,
    "lease": {
      "expires_at": "2026-11-01T12:05:00Z",
      "heartbeat_interval_secs": 60
    }
  },
  "task": {
    "task_id": "…",
    "title": "确定地形渲染架构",
    "task_type": "architecture",
    "status": "claimed",
    "revision": 7,
    "priority": "normal",
    "goal": "确定 Bevy 2D 地形高度晕渲方案",
    "description": "…",
    "constraints": { "language": "Rust", "engine": "Bevy 0.19", "map_scale": "regional" },
    "deliverables": ["docs/adr/001-terrain-rendering.md"],
    "git_branch": "task/4f2a9c1b"
  },
  "acceptance_criteria": [
    { "id": "…", "ordinal": 1, "text": "比较 GPU Shader 和 CPU 预计算", "verification_kind": "review" },
    { "id": "…", "ordinal": 2, "text": "cargo test 全绿", "verification_kind": "automated_test" }
  ],
  "repositories": [
    {
      "repository_id": "…",
      "name": "world-view",
      "base_ref": "main",
      "base_commit": "…",
      "write_access": true,
      "workspace_path": "repos/world-view-<repository_id>",
      "path_scope": ["crates/terrain/**"]
    }
  ],
  "knowledge_refs": [
    { "document_id": "…", "doc_key": "architecture.terrain", "doc_type": "architecture",
      "canonical_path": "architecture/terrain.md", "revision_no": 3, "is_required": true }
  ],
  "policy": {
    "approval_required": true,
    "approval_status": "approved",
    "execution_enabled": true,
    "snapshot_scope": "d1-minimal"
  }
}
```

### 4.2 字段 → 来源映射（每个字段必须可追溯，禁止拼装）

| 契约字段 | 库内来源 | 可空 |
| --- | --- | --- |
| `run.*` | `delivery.task_run`（`attempt_no` / `stage` / `timeout_seconds` / `lease_expires_at`） | 否 |
| `task.title/task_type/status/revision/priority/description/goal/git_branch` | `delivery.task` | `task_type` / `goal` 可空 |
| `task.constraints` / `task.deliverables` | `delivery.task.constraints` / `deliverables`（`0014` 新增） | 否（默认 `{}` / `{}`） |
| `acceptance_criteria[]` | `delivery.task_acceptance_criterion` | 否（0 条合法，D5 按策略判定） |
| `repositories[]` | `delivery.task_run_repository` **JOIN** `integration.project_repository` | 否（领取时已锁定） |
| `knowledge_refs[]` | `delivery.task_knowledge_ref` **JOIN** `ai.project_document` + 最新 `document_revision` | 否 |
| `policy.*` | `delivery.task_run.policy_snapshot`（D1 已写 `d1-minimal`） | 否 |
| `run.lease.heartbeat_interval_secs` | 配置 `AI_PM_RUN_HEARTBEAT_SECS`，默认 60 | 否 |

> **禁止**：契约中出现任何工具相关字段（tool 名、prompt 模板、model 名）。角色指令属平台配置，不进契约（positioning §五）。

### 4.3 `pm_report_progress` 入参与语义

```json
{
  "project_id": "…", "run_id": "…", "lease_token": "…",
  "stage": "started",
  "note": "已定位到 crates/terrain/src/render.rs",
  "blocked": false
}
```

| 动作 | 副作用 | 约束 |
| --- | --- | --- |
| 续租 | `lease_expires_at = now() + AI_PM_RUN_LEASE_SECS`，`heartbeat_at = now()` | 仅 `status IN (claimed, running)` 且 token 匹配 |
| 首报 `stage='started'` | `claim_once` 已写 `stage='dispatch'`；此处推进为 `started`，写 `executor_session` 与 `executor_kind` | **`claimed→running` 由平台写**（D1 §九 #4），工具不得直写 `running` |
| `blocked=true` | 写 `governance.audit_log(action='run_blocked')` | 不新建 `task_waiting`（D5 才裁决是否落 `AwaitingDecision`） |

### 4.4 `pm_submit_result` 契约冻结（D2 不实现落库）

```json
{
  "project_id": "…", "run_id": "…", "lease_token": "…",
  "repositories": [
    { "repository_id": "…", "base_commit": "…", "commit_sha": "…",
      "changed_files": 12, "tests": [{ "name": "cargo test", "passed": true, "evidence_ref": "…" }] }
  ],
  "summary": "…",
  "knowledge_proposals": [{ "document_id": "…", "change_summary": "…" }]
}
```

D2 只冻结形状并写 schema 测试；**不接受调用**（返回 `Unsupported` 且附 `"implemented_in": "D4"`），避免执行端误以为已可提交。

---

## 五、数据模型（迁移 `0014`）

> 原则：forward-only 扩列 + 两张关联表；不改既有列语义；全部可空或有默认值。

```sql
-- 0014_task_contract: 执行契约字段（D2）
-- 依据：docs/ai-pm-slice-d2-mcp-context-v1.md §四 §五

-- 1) 任务契约（目标 / 约束 / 产物 / 平台生成的分支名）
ALTER TABLE delivery.task
    ADD COLUMN IF NOT EXISTS goal         TEXT,
    ADD COLUMN IF NOT EXISTS constraints  JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS deliverables TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS git_branch   TEXT;

-- 2) 逐条验收（C 裁决：独立表，D5 逐条裁决并存证据）
CREATE TABLE IF NOT EXISTS delivery.task_acceptance_criterion (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id       UUID NOT NULL,
    task_id          UUID NOT NULL,
    ordinal          INT  NOT NULL CHECK (ordinal >= 1),
    text             TEXT NOT NULL CHECK (length(btrim(text)) > 0),
    verification_kind TEXT NOT NULL DEFAULT 'manual'
        CHECK (verification_kind IN ('manual', 'automated_test', 'artifact_exists', 'review')),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_id, ordinal),
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE
);

-- 3) 知识引用（C2：以 document_id 为准，路径仅作展示）
CREATE TABLE IF NOT EXISTS delivery.task_knowledge_ref (
    project_id   UUID NOT NULL,
    task_id      UUID NOT NULL,
    document_id  UUID NOT NULL,
    is_required  BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, document_id),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, document_id) REFERENCES ai.project_document(project_id, id)
);

-- 4) run 级凭据（D 裁决）：只存哈希，明文在 claim 时一次性返回
ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS lease_token_hash TEXT,
    ADD COLUMN IF NOT EXISTS client_name      TEXT,
    ADD COLUMN IF NOT EXISTS client_version   TEXT;

CREATE INDEX IF NOT EXISTS idx_task_run_lease_token
    ON delivery.task_run(lease_token_hash)
    WHERE lease_token_hash IS NOT NULL;
```

### 5.1 兼容性

| 项 | 影响 | 回退 |
| --- | --- | --- |
| 四个新列 | 全部可空或有默认值；存量 `task` 行 `constraints={}`、`deliverables={}` | `DROP COLUMN` |
| 两张新表 | 新表，无存量依赖 | `DROP TABLE` |
| `lease_token_hash` | 存量 Run 为 `NULL`，无法通过 token 校验（预期：D1 期间无 MCP 领取） | 无需回退 |
| `git_branch` | 存量任务为空；首次领取时若为空则按 `task/<id 前 8 位>` 回填 | 无 |

---

## 六、接口与鉴权

### 6.1 工具清单增量（B 裁决：`pm_*`）

| MCP 工具名 | 文档语义名 | 权限 | D2 状态 |
| --- | --- | --- | --- |
| `pm_claim_task` | `task.claim` | `PM_MCP_READ` + 项目内 `dev_agent` | **实现** |
| `pm_get_run_context` | `task.get_context` | 第一层 `PM_MCP_READ`；第二层 `lease_token` | **实现** |
| `pm_report_progress` | `task.report_progress` | 同上 | **实现** |
| `pm_get_task_context` | （摘要，非执行契约） | `PM_MCP_READ` | **保留不改**（G4） |
| `pm_submit_result` | `task.submit_result` | 同上 | **冻结 schema，返回 Unsupported** |
| `pm_submit_technical_design` / `pm_get_submission_status` / `pm_submit_document` | — | 现状 | 不动 |

### 6.2 鉴权两层（D 裁决）

```text
第一层（既有）：bearer + PM_MCP_READ  →  服务级准入
第二层（D2 新增）：run_id + lease_token  →  run 级准入
        sha256(token) 与 task_run.lease_token_hash 相等
        且 status IN ('claimed','running')
        且 lease_expires_at > now()
```

失败一律返回 `isError` 文本（`LeaseExpired` / `Forbidden`），**不回显 hash 或正确 owner**。

### 6.3 与 D1 领取路径的关系（A 裁决）

```mermaid
flowchart LR
    subgraph A["Model A：执行端主动领取（D2 主路径）"]
      E1["Codex / Claude Code"] -->|pm_claim_task(project, task)| C1["claim_task_run()"]
      C1 -->|lease_token 明文| E1
      E1 -->|pm_get_run_context| C1
    end
    subgraph B["Model B：worker 领取（D1 现状，无人值守）"]
      W1["ai-pm-worker 第四条循环"] --> C1
    end
    C1 --> T[("同一事务模板<br/>run→task 锁序 · 基线快照 · 审计")]
```

**实现要求**：`claim_task_run(project, task, client)` 与 `claim_once(worker)` 必须调用**同一个**内部函数 `claim_core(tx, candidate_filter, lease_owner)`；新增的只是候选筛选（`WHERE task_id = $1`）与 `lease_owner` 取值，**不得复制事务体**（D1 B3-6 同款纪律）。

### 6.4 留口补齐（D1 §九）

| D1 留口 | D2 处置 |
| --- | --- |
| #2 `policy_version` / `policy_hash` 口径 | 仍写 `NULL`；`0014` 不引入策略版本表，留策略阶段 |
| #3 续租对外通道与鉴权 | `pm_report_progress` + `lease_token`（本文 §6.2） |
| #4 `claimed→running` 触发者 | `pm_report_progress(stage='started')` 由**平台**写 `running` 与 `executor_session` |
| #6 系统 actor 缺失降级 | `audit_run` 回退链追加 `kind='service'`（`0014` 无需建表） |
| `execution_enabled` 字面量 | 策略读取失败 → `execution_enabled=false` 并落快照（G5） |

---

## 七、验收与测试矩阵

| ID | 场景 | 断言 |
| --- | --- | --- |
| C1 | `pm_claim_task` 领指定任务 | `task_run` 恰 1 行 `claimed`；返回 `lease_token` 明文 32B；库内只有 hash |
| C2 | 同任务第二个 `pm_claim_task` | 拒绝（`uq_task_run_active` + 行锁），不产生第 2 条活跃 Run |
| C3 | `pm_get_run_context` 全字段 | 七类字段均非空且有来源；`repositories[]` 数量 = `task_run_repository` 行数 |
| C4 | schema 校验 | 返回体通过 `contract_v1` JSON Schema；未知字段为 0 |
| C5 | 跨项目读取 | 另一项目的 `run_id` → `NotFound`（不泄露存在性） |
| C6 | 无 token / 错 token | `Forbidden`；不泄露 hash |
| C7 | 租约过期后读 | `LeaseExpired`（证明有效期参与校验，非仅 token 匹配） |
| C8 | `report_progress` 续租 | `lease_expires_at` 前移；`heartbeat_at` 更新；`stage` 推进到 `started` |
| C9 | 首报 `started` | Run 落 `running` 且写 `executor_session`/`executor_kind`；`task.status='running'` 同事务 |
| C10 | 非持有者续租 | 拒绝；`lease_owner` 与 token 均不变 |
| C11 | `pm_submit_result` | 返回 `Unsupported` + `implemented_in: D4`；**不产生任何写入** |
| C12 | 策略读取失败 | `policy_snapshot.execution_enabled=false`，领取被拒（G5） |
| C13 | 无 human 成员项目的审计 | 回退到 `service` principal，领取成功（G6） |
| C14 | 0014 可重入 | 重复执行不报错；存量行 `constraints={}`、`deliverables={}` |

测试文件规划：

| 文件 | 内容 |
| --- | --- |
| `crates/pm-infrastructure/tests/migration_0014.rs` | C14、约束校验（`ordinal` 唯一、`verification_kind` 枚举）、外键级联 |
| `crates/pm-application/tests/run_context.rs` | C1/C2/C6/C7/C8/C9/C10/C12/C13（真实 PG，复用 `run_dispatch.rs` 的 `seed()` 范式） |
| `crates/pm-mcp/tests/contract.rs` | C3/C4/C5/C11（契约形状与 schema） |

门禁：`cargo test` 全绿（含 D1 的 26 例不回归）+ `cargo build` 无新 warning + 依赖方向（`pm-mcp` 依赖 `pm-application`，不依赖 `pm-infrastructure`）。

---

## 八、实施顺序与提交纪律

沿用 D1 的 Q10 纪律，**两次提交，不得合并**：

| 步骤 | 内容 | 提交 |
| --- | --- | --- |
| S0 | 本文档（D2 设计依据 v1） | `docs(executor): ...` |
| S1 | `0014` 迁移 + 迁移测试 | `feat(executor): ...` |
| S2 | `claim_task_run` + `pm_claim_task` + `pm_get_run_context` + 契约测试 | 同上 |
| S3 | `pm_report_progress`（续租/阶段推进）+ G5/G6 收口 | 同上 |

**提交前必须清场**：工作区存在未提交的 `README.md` 与 `docker-compose.dev.yml`（测试库 `fsync=off` 提速改动），属独立优化，应先单独提交，否则 `0014` 的 diff 会混入无关变更。

---

## 九、D3–D5 边界（D2 留口，不越界）

| 项 | 归属 | D2 只做到 |
| --- | --- | --- |
| 启动/终止执行进程 | D3 | 记录 `executor_kind`/`executor_session`，不拉起进程 |
| 逐仓 commit 与交付清单 | D4 | 冻结 `submit_result` schema，不落库 |
| 失败判定与任务状态裁决 | D5 | 只写 `run_blocked` 审计，不改任务状态 |
| 完整策略合成 | 策略阶段 | 只透传 `d1-minimal` 快照 |
| 执行端能力分档（无人值守/自动批准） | 后续 | 只记录 `client_name`/`client_version`，不据此调整租约 |

---

## 附录：工具命名映射（B 裁决）

| 文档语义名 | MCP 工具名 | 状态 |
| --- | --- | --- |
| `task.claim` | `pm_claim_task` | D2 新增 |
| `task.get_context` | `pm_get_run_context` | D2 新增 |
| `task.report_progress` | `pm_report_progress` | D2 新增 |
| `task.submit_result` | `pm_submit_result` | D2 冻结，D4 实现 |
| `task.get_review` | — | D5 |
| `task.request_decision` | — | D5 |
| `knowledge.search` | — | 知识空间线 |
| `knowledge.propose_change` | — | D5 |
| （摘要） | `pm_get_task_context` | 既有，保留 |
