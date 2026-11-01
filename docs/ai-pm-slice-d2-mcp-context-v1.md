# AI-PM 平台：Slice D2 MCP 任务上下文契约（V1 · 设计稿）

> 面向：技术评审方 / 实施方（含 Codex 执行端）
> 日期：2026-11-01
> 状态：**设计依据 v1.1（已裁决 A–D；已并入架构 + QA 评审修订，可实施）**（本文只定义 D2 的实施范围；D3–D5 仅作范围锚定）
> 修订记录：v1.1（2026-11-01）并入评审结论 R-F1~R-F17 / R-G1~R-G17：关闭 4 个 P0（调度开关门控、过期续租、错误变体分层、双入口并发测试），补 lease_token 生命周期规则、`renew_lease_by_token` 新函数、report_progress 事务化与审计、0014 DDL 三处修订；矩阵 C1–C14 扩至 C1–C28。汇总见 §十。
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
2. **租约可信**：无有效 `lease_token` 一律拒绝（C6/C17）；租约过期后正确 token 也被拒（C15）、重领后旧 token 失效（C19）；`pm_report_progress` 续租后 `lease_expires_at` 严格前移（C8/C22/C23）。
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
| 续租（`renew_lease`） | 同上 `:892` | **仅作参考、不复用**（v1.1：D2 落地为新函数 `renew_lease_by_token`，见 §4.3 §6.2——现函数以 `lease_owner` 为凭证、仅 `claimed` 态可续、**无 `lease_expires_at > now()` 检查**、无事务无审计，四项均与 D 裁决冲突） |
| fencing 校验 | 同上 `accept_executor_result:913` | D2 扩展为凭 `lease_token` 校验；`accept_executor_result` 的会话校验保留给 D3 执行器实例 |
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
| G6 | 审计 `actor` 回退只认 `human`，项目无 human 成员时 `RowNotFound` 导致领取失败；回退后审计无法追责真实触发者 | `run_dispatch.rs:937-958`（两级回退：owner human → 任意 human；三级的是 `pm_write::owner_actor`，口径需统一） | P1 | 种子化平台专用 service principal（`ai-pm-dispatch`），回退链追加 `kind='service'`；审计 `after_data` 强制携带 `client_name`/`lease_owner` 保留归因（评审 R-F7） |
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

**v1.1 评审补充裁决（A 项条件关闭）**：

| # | 补充裁决 |
| --- | --- |
| A-1 | **Model A（`pm_claim_task`）不受 `AI_PM_RUN_DISPATCH` 开关约束**。该开关只门控 Model B（worker 无人值守领取）——执行端显式请求不属「无人值守」。实现：`claim_core` 增显式参数 `gated_by_dispatch_mode: bool`，Model A 恒传 `false`。否则生产默认 off 下 D2 主路径静默失效（评审 R-F1，P0），须有「off 模式下 `pm_claim_task` 仍可领」测试。 |
| A-2 | 「复用同一事务模板」≠「只换候选筛选」——token 签发（仅 Model A）、退避/取消分支、审计 payload 均为 A/B 分叉点，`claim_core` 以参数表达分叉，禁止复制事务体。**双入口并发互斥必须有机器验证**（验收 C16，评审 R-G2，P0）。 |

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
    "deliverables": [{ "id": "…", "path": "docs/adr/001-terrain-rendering.md" }],
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
>
> **v1.1 语义声明（评审 R-F11/F10）**：`repositories[]` 冻结于领取（`task_run_repository` 快照）；`knowledge_refs[]` 为 **live 解析**（JOIN 最新 `document_revision`），两次调用可能返回不同 revision——V1 显式接受（执行端只读路径），如需固化快照在 D5 前追加迁移。`deliverables` 为 JSONB 对象数组（`[{id, path}]`），D4 按条目 `id` 对账。

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
| 续租 | `lease_expires_at = now() + AI_PM_RUN_LEASE_SECS`，`heartbeat_at = now()` | 仅 `status IN (claimed, running)` **且 `lease_expires_at > now()`** 且 token 匹配——过期后正确 token 也拒绝，防僵尸 Run 复活（评审 R-G1/G-6；现 `renew_lease` 无过期检查，不可复用） |
| 首报 `stage='started'` | `claim_once` 已写 `stage='dispatch'`；此处推进为 `started`，写 `executor_session` 与 `executor_kind` | **`claimed→running` 由平台经 `apply_task_transition` 写**（合法边已核实：守卫表含 `(Claimed, Running)`），**严禁直写 UPDATE**（R1 红线：不允许第二条绕过守卫的写入路径，评审 R-F4） |
| `blocked=true` | 写 `governance.audit_log(action='run_blocked')` | 不新建 `task_waiting`（D5 才裁决是否落 `AwaitingDecision`） |

**v1.1 硬性要求（评审 R-F4/F5）**：`pm_report_progress` 的全部副作用（续租 + `stage` 推进 + 任务状态迁移 + 审计）必须在**单事务**内完成，并写 `renew_run_lease` / `run_stage_started` 审计行（D1 §5.8 同款纪律）。续租由新函数 `renew_lease_by_token` 承载：凭 hash、含过期检查、同事务、带审计——现 `renew_lease` 四项皆缺，退役或改造。另明示：**心跳 = 上报**，执行端省略 `report_progress` 的代价是租约到期被 B3-1 回收、工作白做（与 positioning A2「上报可省略」的张力属已知取舍，评审 R-F16）。

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

D2 只冻结形状并写 schema 测试；**不接受调用**——**shape-first 不触库**：不校验 token、不查询任何表，直接返回 `Unsupported` 且附 `{"implemented_in": "D4", "schema": "draft"}`（v1.1：措辞由「冻结」降级为 **draft**，D4 拥有定稿权；结构上保证零写入，评审 R-F14/G-11）。

---

## 五、数据模型（迁移 `0014`）

> 原则：forward-only 扩列 + 两张关联表；不改既有列语义；全部可空或有默认值。

```sql
-- 0014_task_contract: 执行契约字段（D2）
-- 依据：docs/ai-pm-slice-d2-mcp-context-v1.md §四 §五

-- 1) 任务契约（目标 / 约束 / 产物 / 平台生成的分支名）
--    v1.1：deliverables 由 TEXT[] 改为 JSONB 对象数组（[{id, path}]）——
--          D4 交付对账需按条目 id 锚定证据，TEXT[] 无法引用单个条目（评审 R-F10）。
ALTER TABLE delivery.task
    ADD COLUMN IF NOT EXISTS goal         TEXT,
    ADD COLUMN IF NOT EXISTS constraints  JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS deliverables JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS git_branch   TEXT;

-- 2) 逐条验收（C 裁决：独立表，D5 逐条裁决并存证据）
--    v1.1：ordinal 允许间隙（仅作排序，不要求连续；中间插入用 max+10，
--          禁止全量重排——评审 R-F8）。updated_at 记录条目修订时间；
--          若 D5 需要完整版本化留痕，forward-only 追加迁移补。
CREATE TABLE IF NOT EXISTS delivery.task_acceptance_criterion (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id       UUID NOT NULL,
    task_id          UUID NOT NULL,
    ordinal          INT  NOT NULL CHECK (ordinal >= 1),
    text             TEXT NOT NULL CHECK (length(btrim(text)) > 0),
    verification_kind TEXT NOT NULL DEFAULT 'manual'
        CHECK (verification_kind IN ('manual', 'automated_test', 'artifact_exists', 'review')),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_id, ordinal),
    -- 预留 D5 证据表按 (project_id, id) 组合外键锚定单条验收项（与 0006/0011 同款）
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
--    v1.1 编码钉死（评审 R-G7）：明文 token = 32 字节随机数的**小写 hex（64 字符）**；
--    lease_token_hash = sha256(明文 token 字符串) 的**小写 hex（64 字符）**，严格匹配（大写变体拒绝）。
--    校验一律按 run_id 主键取行后比对 hash（不存在 token 反查场景），故无需 token 索引（评审 R-F9）。
ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS lease_token_hash TEXT,
    ADD COLUMN IF NOT EXISTS client_name      TEXT,
    ADD COLUMN IF NOT EXISTS client_version   TEXT;
```

### 5.1 兼容性

| 项 | 影响 | 回退 |
| --- | --- | --- |
| 四个新列 | 全部可空或有默认值；存量 `task` 行 `constraints={}`、`deliverables=[]` | `DROP COLUMN` |
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
| `pm_submit_result` | `task.submit_result` | 同上 | **schema draft + shape-first 返回 Unsupported**（D4 定稿） |
| `pm_submit_technical_design` / `pm_get_submission_status` / `pm_submit_document` | — | 现状 | 不动 |

**v1.1 威胁模型明示（评审 R-F13/G-10）**：`PM_MCP_READ` + 「项目存在 dev_agent 成员」是**部署级**门，不是调用方身份认证——V1 无 per-project 调用方绑定，持有效 bearer 的任意调用方可领取任意项目任务（横向越权面已知且 V1 接受）。执行端预注册档位（positioning §4.4 方案 C）列入待裁 #12。`pm_claim_task` 前置条件钉死：任务不存在或跨项目 → `NotFound`；存在但无 `queued` Run（TaskReadyHandler 未跑）→ `NotFound`（附提示等待就绪事件）；任务非 `ready` → `Conflict`（附当前状态）；并发竞争失败 → `Conflict`。

### 6.2 鉴权两层（D 裁决）

```text
第一层（既有）：bearer + PM_MCP_READ  →  服务级准入
第二层（D2 新增）：run_id + lease_token  →  run 级准入
        sha256(token) 小写 hex 与 task_run.lease_token_hash 严格相等
        且 status IN ('claimed','running')
        且 lease_expires_at > now()
        且 lease_token_hash IS NOT NULL（Model B / 存量 Run 一律拒绝，评审 R-G4）
```

失败一律返回 `isError` 文本（`LeaseExpired` / `Forbidden`），**不回显 hash 或正确 owner**。

**v1.1 分层修正（评审 R-G3，P0）**：错误枚举定义在 **pm-application**（`LeaseError::{Forbidden, Expired, NotFound, Conflict}`），pm-mcp 的 `ToolError` 1:1 映射——否则 pm-application 层测试无法断言（依赖方向 pm-mcp → pm-application）。第二层校验实现在 pm-application 工具函数层而非 pm-api 中间件，`AI_PM_AUTH_MODE=off`（测试形态）下依然生效（评审 R-F17①）。

**v1.1 token 生命周期规则（评审 R-F2/G-5）**：
1. **每次成功 claim 必签发新 token 并覆盖 `lease_token_hash`**——含 B3-1 重领同一 Run（旧持有者 token 立即失效）与 Model A/B 交替。
2. **一切去租约路径同步 `SET lease_token_hash = NULL`**：`requeue_with_backoff` / `requeue_for_redispatch` / `cancel_run` / `isolate_started_run` 四处（`run_dispatch.rs:838/856/873/799`）。
3. token 泄漏后的唯一吊销路径 = run 被回收 / 取代 / 终态（status 校验兜底）；V1 不提供主动吊销接口。

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

**实现要求（v1.1 修订）**：`claim_task_run(project, task, client)` 与 `claim_once(worker)` 必须调用**同一个**内部函数 `claim_core(tx, opts)`；`opts` 携带全部 A/B 分叉点——候选筛选（`WHERE task_id = $1` vs 通用候选）、`lease_owner` 取值、`gated_by_dispatch_mode`（Model A 恒 `false`，见 §3 A-1）、token 签发（仅 Model A）、审计 payload，**不得复制事务体**（D1 B3-6 同款纪律）。`git_branch` 回填用 `UPDATE ... WHERE id = $1 AND git_branch IS NULL`（幂等；非状态字段，不经 `apply_task_transition`，此例外须在代码注释成文，评审 R-F12）。

### 6.4 留口补齐（D1 §九）

| D1 留口 | D2 处置 |
| --- | --- |
| #2 `policy_version` / `policy_hash` 口径 | 仍写 `NULL`；`0014` 不引入策略版本表，留策略阶段 |
| #3 续租对外通道与鉴权 | `pm_report_progress` + `renew_lease_by_token`（本文 §4.3 §6.2） |
| #4 `claimed→running` 触发者 | `pm_report_progress(stage='started')` 由**平台经 `apply_task_transition`** 写 `running` 与 `executor_session` |
| #6 系统 actor 缺失降级 | `audit_run` 回退链追加 `kind='service'`（`0014` 无需建表） |
| `execution_enabled` 字面量 | 策略读取失败 → `execution_enabled=false` 并落快照（G5） |

---

## 七、验收与测试矩阵

| ID | 场景 | 断言 |
| --- | --- | --- |
| C1 | `pm_claim_task` 领指定任务 | `task_run` 恰 1 行 `claimed`；返回 `lease_token` 明文 32B；库内只有 hash |
| C2 | 同任务第二个 `pm_claim_task` | 拒绝（`uq_task_run_active` + 行锁），不产生第 2 条活跃 Run |
| C3 | `pm_get_run_context` 全字段（种子含值） | 种子写入全部契约字段后逐字段断言；`repositories[]` 数量 = `task_run_repository` 行数（v1.1：原「七类字段均非空」与 §4.2「0 条验收合法、`goal` 可空」矛盾——改为**种子含值测全字段**，空值合法性由 C18 覆盖，评审 R-G8） |
| C4 | schema 校验 | 返回体通过 `contract_v1` JSON Schema；未知字段为 0 |
| C5 | 跨项目读取 | 另一项目的 `run_id` → `NotFound`（不泄露存在性） |
| C6 | 无 token / 错 token | `Forbidden`；不泄露 hash |
| C7 | 租约过期后读 | `LeaseExpired`（证明有效期参与校验，非仅 token 匹配） |
| C8 | `report_progress` 续租 | `lease_expires_at` 前移；`heartbeat_at` 更新；`stage` 推进到 `started` |
| C9 | 首报 `started` | Run 落 `running` 且写 `executor_session`/`executor_kind`；`task.status='running'` 同事务 |
| C10 | 非持有者续租 | 拒绝；`lease_owner` 与 token 均不变 |
| C11 | `pm_submit_result`（shape-first） | 不触库直接返回 `Unsupported` + `{implemented_in: "D4", schema: "draft"}`；断言五表行数（task_run / task_run_repository / audit_log / inbound_event / task_run_failure）与 task 状态/revision/lease 字段调用前后全等 |
| C12 | 策略读取失败（注入 seam） | 仿 `PolicyEngine::for_tests()` 为策略读取加可注入 seam，测试注入失败引擎 → 领取被拒且**拒绝路径审计 `after_data` 携带失败快照**（不污染 run 行的 policy_snapshot——退避 Run 的快照会被下次成功领取覆盖，评审 R-F15） |
| C13 | 无 human 成员项目的审计 | 回退到 `service` principal，领取成功（G6） |
| C14 | 0014 可重入 | 重复执行不报错；存量行 `constraints={}`、`deliverables=[]` |
| C15 | 持有者 + **已过期**租约续租 | 正确 token + `lease_expires_at = now()-1s` → `LeaseExpired` 且 `lease_expires_at`/`heartbeat_at` 未变（防僵尸复活；现 `renew_lease` 无过期检查，评审 R-G1，P0） |
| C16 | 双入口并发互斥 | `tokio::join!(claim_task_run, claim_once)` 抢同一 task → active run ≤ 1、恰一入口成功（A 裁决「同一事务模板」的机器验证，评审 R-G2，P0） |
| C17 | Model B Run 的 NULL hash | `claim_once` 领取后任意 token 调 get_run_context → `Forbidden`（不 panic 不泄露，评审 R-G4） |
| C18 | 契约空值合法性 | 0 条验收项 + NULL goal 的任务 → 契约返回 `acceptance_criteria: []`、`goal: null`，schema 校验通过（评审 R-G8） |
| C19 | 重领后旧 token 失效 | B3-1 过期重领同一 Run → 旧 token 被拒、新 token 可用（token 生命周期规则 1，评审 R-G5） |
| C20 | tools/list 冻结名单 | 8 个 `pm_*` 工具名精确断言（含 `pm_submit_result`）；**既有 tools.rs smoke 的 4 工具名单断言须同步更新否则必挂**（评审 R-G9） |
| C21 | `lease_owner` 格式 | Model A 断言 `mcp:<client_name>:<token前8位>`；Model B 仍为 worker id（防 D3 误用做 fencing，评审 R-G12） |
| C22 | 运行态续租 | 首报 `started`（Run→running）后二次 report_progress → 续租成功且 `lease_expires_at` 严格前移（现 `renew_lease` 仅 `claimed` 可续，评审 R-G6） |
| C23 | 并发续租竞态 | `tokio::join!` 两路同 token 续租 → `lease_expires_at` 单调不回退；一正一错 token → 错路无副作用（评审 R-G15） |
| C24 | knowledge_refs 取最新 revision | 同文档种子 2 条 revision → 返回 `revision_no = max`（live 语义，评审 R-G16） |
| C25 | 验收项按 ordinal 升序输出 | 乱序 insert → 返回按 ordinal 升序（评审 R-G17） |
| C26 | `pm_claim_task` 前置条件 | 无 queued Run / 跨项目 → `NotFound`；非 ready → `Conflict`（附当前状态）（评审 R-G10） |
| C27 | `client_name`/`client_version` 落库 | claim 后 SELECT 断言两列值（评审 R-G14） |
| C28 | hash 编码严格匹配 | 库内写入大写 hex 变体 → 校验拒绝（按 §5 钉死的小写 hex 严格匹配，评审 R-G7） |

测试文件规划：

| 文件 | 内容 |
| --- | --- |
| `crates/pm-infrastructure/tests/migration_0014.rs` | C14（**须在测试内 `sqlx::raw_sql(include_str!)` 手动二连执行 0014**——`#[sqlx::test]` 不会重复跑迁移；先 INSERT task 再二跑验证默认值，评审 QA#3）、约束校验（`ordinal` 唯一、`verification_kind` 枚举）、外键级联 |
| `crates/pm-application/tests/run_context.rs` | C1/C2/C6/C7/C8/C9/C10/C12/C13/C15/C16/C17/C19/C21/C22/C23/C26/C27（真实 PG；断言 **pm-application 层 `LeaseError` 变体**而非 pm-mcp 的 ToolError；C13 需「无 human 成员」项目——现 `seed()` 走 `bootstrap_project` 必造 human，须加裸 `seed_project` + service principal 变体） |
| `crates/pm-mcp/tests/contract.rs` | C3/C4/C5/C11/C18/C20/C24/C25/C28（契约形状；**同步更新既有 tools.rs smoke 的 4 工具名单断言为 8 工具**） |

**v1.1 测试落地注脚（评审 QA#2/#4/#5）**：
- C4「未知字段为 0」用 `#[serde(deny_unknown_fields)]` 镜像结构体对返回 `Value` 往返反序列化实现，零新依赖；schema 结构体继续 derive `schemars`。
- C8 续租断言用**严格不等**（新 `lease_expires_at` > 旧值），不用 `== 旧值+N`，避免应用侧 chrono 与 DB `now()` 时钟偏差 flaky。
- 造过期后**不要再调 `claim_once`**（会触发 reclaim/supersede 改状态污染断言）；C19 把「造过期」与「断言旧 token」拆两步。
- 「每个字段有库内来源」属文档属性，靠 §4.2 映射表人工 review，不伪装成自动验收；C5「不泄露存在性」并入 C6 断言（错误文本不含 run_id/hash/owner）。

门禁：`cargo test` 全绿（含 D1 的 26 例不回归）+ `cargo build` 无新 warning + 依赖方向（`pm-mcp` 依赖 `pm-application`，不依赖 `pm-infrastructure`）。

---

## 八、实施顺序与提交纪律

沿用 D1 的 Q10 纪律，**两次提交，不得合并**：

| 步骤 | 内容 | 提交 |
| --- | --- | --- |
| S0 | 本文档（D2 设计依据；v1.1 已并入评审修订） | `docs(executor): ...` |
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

## 十、V1.1 评审修订记录（2026-11-01）

评审方式：架构反向审视（R-F 系，17 项）+ QA 矩阵可测性（R-G 系，17 项）并行独立评审，均核实过源码与迁移。总判定：**有条件可实施 → 修订后可实施**。四项裁决判定：B/C 维持；A/D 有条件维持，条件已全部并入正文。

| 类别 | 修订 |
| --- | --- |
| P0 关闭（4） | ① 调度开关门控（§3 A-1：Model A 不受 `AI_PM_RUN_DISPATCH` 约束）；② 过期续租拒绝 + `renew_lease_by_token` 新函数（§4.3 §6.2）；③ 错误枚举下沉 pm-application（§6.2）；④ 双入口并发测试 C16（§七） |
| P1 并入 | token 生命周期三规则（§6.2）；`claimed→running` 钉死 `apply_task_transition` + report_progress 单事务审计（§4.3）；service principal 种子与审计归因（§2.2 G6）；C3/G8 矛盾修正；`pm_claim_task` 前置条件钉死（§6.1） |
| P2 并入 | deliverables 改 JSONB 对象数组（§5）；ordinal 允许间隙 + `updated_at`（§5）；删 token 索引、保留 `(project_id, id)` 唯一（D5 证据 FK 预留，§5）；knowledge_refs **live 语义**显式声明（§4.2）；git_branch 幂等回填（§6.3）；pm_submit_result 降级 draft（§4.4）；威胁模型明示（§6.1） |
| 矩阵扩容 | C1–C14 → C1–C28（新增覆盖：过期续租 / 双入口并发 / NULL hash / 空值合法性 / 旧 token 失效 / 冻结名单 / owner 格式 / 运行态续租 / 并发续租 / revision 取最新 / 排序 / 前置条件 / 客户端落库 / 编码严格匹配） |
| 遗留代码债（S1 顺带） | `migrations/0010` 文件头注释仍写「0009_task_state_machine」（迁移号顺移残留）；`audit_run` 回退链补 `kind='service'`（平台专用 principal `ai-pm-dispatch` 种子化，`after_data` 强制携带 client_name/lease_owner） |

### 待裁清单（V1.1 后仍开放）

| # | 议题 | 建议 |
| --- | --- | --- |
| 11 | 契约七类字段的**写路径归属**（评审 R-F6：`pm_write` 现不写 goal/constraints/deliverables，工具面亦无写入口——0014 建好后生产任务契约是空壳） | **建议并入 D2**：扩展 `pm_write::update_task`（扩 UpdateTaskRequest + 两张关联表增删命令），比另立 slice 便宜；若另立须写入 §1.3「不做」 |
| 12 | 执行端预注册档位（positioning §4.4 方案 C）与 per-project 调用方绑定 | V1 接受横向越权面（§6.1 威胁模型），档位机制待首批执行端接入后裁 |
| 13 | 验收条目完整修订留痕（版本化） | `updated_at` 已加；若 D5 证据锚定需要更强不变性，forward-only 追加迁移 |

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
