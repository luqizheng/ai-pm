# AI-PM Visualization V0-2 技术设计（Artifact 数据模型 · 状态机 · 容量治理）V1.0

> 状态：设计稿（证据齐备，待 Migration Owner 签收取号） · 日期：2026-11-12
> 上游：[ADR V1.0.1](ai-pm-viz-09-architecture-decision-record-v1.md)（ADR-VIZ-006、闭合 R-04 / R-05）· [12 号 V0-1 设计](ai-pm-viz-12-v0-1-spec-validation-design-v1.md)· [D4 系统架构](ai-pm-viz-04-system-architecture-v1.md)
> 落地约定：本文所有"仓库事实"均来自 2026-11-12 对 `D:\kudi-project\templates\ai-pm` 的实证（路径:行号随附）。

## 1. 范围与闭合目标

**交付物**：`viz` schema + 四表（artifact / approval / render_output / slot_config）+ 服务层 + API 端点骨架 + 容量清理规则。

**闭合**：
- **R-04** Artifact 与 Approval 状态机分离（ADR-VIZ-006 要求"审批状态不应直接混入 Artifact 状态字段"）
- **R-05** 容量与清理规则（单产物上限、历史保留、失败产物、清理、备份恢复、读取成本）
- **R-06（部分）** nature 落到 Artifact 与 Edge 两级（节点级粒度属 V0-3，见 14 号）

**非目标**：Live 查询投影层（V0-3）· 渲染实现（V0-9）· 前端组件（V0-8）· 跨环境迁移回填（AGENTS.md 规则 1：新项目不写迁移脚本）。

## 2. 取号结论（关闭 R-01 的事实基础）

### 2.1 实证结论

| 证据 | 结果 |
|---|---|
| `git ls-tree -r --name-only master -- migrations/` | 树内仅 **0001–0019**（19 个文件）；`0015_task_schedule` ~ `0019_conversation_context_item` 均在 |
| 台账 `docs/reference/ai-pm-migration-ledger.md` §一「已应用（历史审计——文件已并入 0001_baseline.sql）」 | 登记 0022–0026、0044–0054 等**基线前**编号 |
| 台账 §〇·二「基线后在途迁移（权威 = migrations/ 目录，编号自 0002 起）」 | **只登记到 0014** → 0015~0019 五条迁移**未登记**（台账缺口，需补登） |
| 历史文件存在性（`git log --all --diff-filter=A`） | `0020_agent_run_identity.sql`、`0021_task_dependency.sql`、0055~0060（`0055_device_name_active_unique.sql` 等）**均曾以文件形式存在**，基线 squash（2026-11-09）后从 master 树移除，内容并入 `0001_baseline.sql` |
| master 树中是否存在其他迁移目录 | **不存在**（`git ls-tree` 全仓只有一个 `migrations/`） |

### 2.2 取号论证

sqlx 的 `_sqlx_migrations` 记录**已应用版本号**，要求文件集合与库内已应用集合一致；复用或跨过历史版本号会导致 `VersionMissing` / `VersionTooOld`（本仓已有此坑记录：test-performance-toolchain 记忆中"锁表/VersionMissing 两个坑"）。

~~因此新号必须 > 任何环境可能已应用的最大版本号 = 0060。~~
**该推理已于 2026-11-12 实测推翻**：真实环境 `_sqlx_migrations` = max **18** / 18 条，无旧链残留；
且台账 §〇 明文「新链自 0002 起，不再受旧链历史与空号约束」——旧链号段不参与取号计算。

| 候选 | 论证 | 结论 |
|---|---|---|
| ~~0020/0021 曾以文件存在~~ | 那属**旧链**（已并入 0001_baseline）；实测无环境残留该版本记录 | ✅ 0020 可用 |
| 0028（台账 §三.1 公式 max(0019+1, 0027+1)） | 0022–0027 亦在基线前取用区间内 | ❌ 否决 |
| ~~0061~~ | ~~严格大于历史最大已用号 0060~~ → **实测证伪**：真实环境无旧链残留（max 18），无需跨号 | ❌ 已弃用 |
| **0020** | 台账 §〇 明文「新链自 0002 起、不受旧链历史与空号约束」；当前目录最大 0019 + 1；实测真实环境 max 18 无残留 | ✅ **采用** |

**结论（2026-11-12 实测复核后更正）**：取号 **0020**（`0020_visualization_artifact.sql`）——此前"必须 > 0060"的论证基于"某些环境可能应用过 0055~0060"的**假设**，实测真实环境 `_sqlx_migrations` = max 18 / 18 条、**无旧链残留**，假设不成立；按台账 §〇「新链自 0002 起、不受旧链约束」与「当前目录最大 0019 + 1」取 0020。**须 Migration Owner 在真实 master 上签收**；若签收前发现某环境已应用 > 0060 的版本，则顺延。签收前不得创建迁移文件。

**台账补登**（同一提交内完成）：补 0015~0019 五条到 §〇·二，并登记 0020 行。

## 3. schema 与表设计

### 3.1 schema

```sql
CREATE SCHEMA IF NOT EXISTS viz;
```
先例：域级迁移一律先建 schema（`migrations/0010_design_domain.sql:36`、`0011:23`、`0016:24`、`0012:16`）。

### 3.2 `viz.artifact`

```sql
CREATE TABLE viz.artifact (
    id                uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id         uuid NOT NULL,
    project_id        uuid NOT NULL,
    slot_key          text NOT NULL,
    version_no        integer NOT NULL,
    state             text DEFAULT 'draft' NOT NULL,
    nature            text DEFAULT 'business' NOT NULL,
    spec              jsonb NOT NULL,
    spec_hash         text NOT NULL,
    source_hash       text NOT NULL,
    provenance        jsonb NOT NULL,
    renderer_id       text,
    renderer_version  text,
    created_by        uuid NOT NULL,
    published_by      uuid,
    published_at      timestamp with time zone,
    row_revision      bigint DEFAULT 1 NOT NULL,
    created_at        timestamp with time zone DEFAULT now() NOT NULL,
    updated_at        timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_artifact_pk PRIMARY KEY (id),
    CONSTRAINT viz_artifact_slot_key_check CHECK (length(btrim(slot_key)) > 0),
    CONSTRAINT viz_artifact_version_check CHECK (version_no >= 1),
    CONSTRAINT viz_artifact_state_check CHECK (state = ANY (ARRAY['draft'::text,'published'::text,'stale'::text,'rejected'::text])),
    CONSTRAINT viz_artifact_nature_check CHECK (nature = ANY (ARRAY['business'::text,'explanatory'::text])),
    -- 解释图不得进入业务槽位（DV-13）：explanatory 不可 published
    CONSTRAINT viz_artifact_nature_publish_check CHECK (state <> 'published' OR nature = 'business'),
    CONSTRAINT viz_artifact_spec_hash_check CHECK (length(btrim(spec_hash)) > 0),
    CONSTRAINT viz_artifact_source_hash_check CHECK (length(btrim(source_hash)) > 0),
    CONSTRAINT viz_artifact_provenance_check CHECK (jsonb_typeof(provenance) = 'object')
);
```

| 列 | 设计要点 | 依据 |
|---|---|---|
| `id uuid DEFAULT gen_random_uuid()` | 统一主键，零 identity/serial | `0019:30`、`0016:34`、`0010:41` |
| `tenant_id` / `project_id` NOT NULL | 两列都必填 | `0019:31-32`、`0013:26-27` |
| `state text` + CHECK | 禁 PG ENUM，一律 text + `= ANY(ARRAY[...])` | `0019:60-70`、`0010:52`、`0013:38-47` |
| `nature` + publish CHECK | **把 DV-13 的"解释图不得发布为业务事实"落到数据库层** | ADR-VIZ-009 |
| `spec jsonb` | Spec 快照原文（**必须保留**，Git 化推迟的条件三） | ADR-VIZ-006 |
| `spec_hash` / `source_hash` text | 裸小写 hex sha256，无前缀 | `pm_write/support.rs:3-11`、`knowledge.rs:700-702` |
| `provenance jsonb` | `{source_domain, source_object_ids, source_revision_refs, source_snapshot_hash}` | ADR-VIZ-006 / D3 §3 |
| `row_revision bigint DEFAULT 1` | 乐观锁，UPDATE 时 +1 | `0010:48`、`0011:36`、`0016:38` |
| `created_at/updated_at` | `timestamp with time zone DEFAULT now()`；updated_at 由应用层显式 SET | `0019:53-54`、`design.rs:1951` |
| **无 `rejected_reason`** | 审批理由归 `viz.approval.decision_reason`（R-04 状态机分离） | ADR-VIZ-006 |

**索引**：
```sql
CREATE UNIQUE INDEX uq_viz_artifact_slot_version ON viz.artifact (project_id, slot_key, version_no);
CREATE INDEX idx_viz_artifact_slot_state ON viz.artifact (tenant_id, project_id, slot_key, state, created_at DESC);
CREATE INDEX idx_viz_artifact_created ON viz.artifact (tenant_id, project_id, created_at DESC);
```

**不可变守卫（发布版不可改）**——沿用仓库既有的触发器守卫范式（`pc.forbid_approval_mutation` `0001:474-496`、`design.guard_design_revision`）：
```sql
CREATE FUNCTION viz.guard_artifact_immutability() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.state = 'published' AND NEW.spec IS DISTINCT FROM OLD.spec THEN
        RAISE EXCEPTION 'published artifact spec is immutable';
    END IF;
    IF OLD.state = 'published' AND NEW.state NOT IN ('published','stale') THEN
        RAISE EXCEPTION 'published artifact cannot leave published/stale';
    END IF;
    NEW.row_revision := OLD.row_revision + 1;
    RETURN NEW;
END $$;
CREATE TRIGGER trg_viz_artifact_guard BEFORE UPDATE ON viz.artifact
    FOR EACH ROW EXECUTE FUNCTION viz.guard_artifact_immutability();
```
语义：**发布版只能向 stale 迁移**（ADR-VIZ-006"published 转为 stale 时只改变生命周期状态，不修改历史发布内容"），且任何 spec 变更被触发器拒绝。

### 3.3 `viz.approval`

```sql
CREATE TABLE viz.approval (
    id                uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id         uuid NOT NULL,
    project_id        uuid NOT NULL,
    artifact_id       uuid NOT NULL,
    slot_key          text NOT NULL,
    target_hash       text NOT NULL,
    expected_version_no integer NOT NULL,
    status            text DEFAULT 'pending' NOT NULL,
    requested_by      uuid NOT NULL,
    decided_by        uuid,
    decision_reason   text,
    decided_at        timestamp with time zone,
    created_at        timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_approval_pk PRIMARY KEY (id),
    CONSTRAINT viz_approval_status_check CHECK (status = ANY (ARRAY['pending'::text,'approved'::text,'rejected'::text,'cancelled'::text])),
    CONSTRAINT viz_approval_target_hash_check CHECK (length(btrim(target_hash)) > 0),
    -- 拒绝必须带理由（对齐 ai.conversation_interaction:52-54）
    CONSTRAINT viz_approval_reject_reason_check CHECK (status <> 'rejected' OR (decision_reason IS NOT NULL AND length(btrim(decision_reason)) > 0)),
    CONSTRAINT viz_approval_decided_check CHECK (status = 'pending' OR decided_by IS NOT NULL),
    CONSTRAINT viz_approval_fk_artifact FOREIGN KEY (artifact_id) REFERENCES viz.artifact(id)
);
CREATE UNIQUE INDEX uq_viz_approval_pending ON viz.approval (project_id, slot_key) WHERE status = 'pending';
```

| 设计点 | 依据 |
|---|---|
| `target_hash` 乐观并发锚点（= 被审 artifact 的 spec_hash） | `design.design_approval.target_hash`（`0010:268`）、裁决时比对漂移→409（`design.rs:1897,2007`） |
| `expected_version_no` CAS 期望版本 | `pc.approval.expected_version_no`（`0001:3031`） |
| `slot_key` 冗余 | 使"每槽位至多一条 pending"可由**数据库唯一约束**保障，而非应用层先查后写（ADR-VIZ-006 明确要求） |
| 部分唯一索引 | `design_approval` 同款 `uq_design_approval_pending`（`0010:286`） |
| 状态含 `cancelled` | ADR-VIZ-006"cancelled（如果支持撤回）" |
| reject 必带 reason | `ai.conversation_interaction` 同款 CHECK（`0005:52-54`） |
| FK 只在同 schema 内 | 跨聚合无 FK 的仓库惯例（`0013:21-22`），同 schema 有 FK 先例（`0016:56`、`0018:49`、`0019:83-84`） |

**裁决触发器**（照 `pc.forbid_approval_mutation` 范式 `0001:474-496`）：`target_hash / expected_version_no / created_at` 不可变；`pending` 只能转 `approved/rejected/cancelled` 且终态不可再改。

### 3.4 `viz.render_output`

```sql
CREATE TABLE viz.render_output (
    id                uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id         uuid NOT NULL,
    project_id        uuid NOT NULL,
    artifact_id       uuid NOT NULL,
    format            text NOT NULL,
    content           text,
    content_bytes     bytea,
    content_hash      text NOT NULL,
    byte_size         integer NOT NULL,
    renderer_id       text NOT NULL,
    renderer_version  text NOT NULL,
    created_at        timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_render_output_pk PRIMARY KEY (id),
    CONSTRAINT viz_render_output_format_check CHECK (format = ANY (ARRAY['html'::text,'svg'::text,'png'::text])),
    CONSTRAINT viz_render_output_content_check CHECK (
        (format IN ('html','svg') AND content IS NOT NULL AND content_bytes IS NULL)
        OR (format = 'png' AND content IS NULL AND content_bytes IS NOT NULL)
    ),
    CONSTRAINT viz_render_output_hash_check CHECK (length(btrim(content_hash)) > 0),
    CONSTRAINT viz_render_output_fk_artifact FOREIGN KEY (artifact_id) REFERENCES viz.artifact(id)
);
CREATE UNIQUE INDEX uq_viz_render_output_artifact_format ON viz.render_output (artifact_id, format);
```

**失败产物策略（R-05）**：**失败的渲染不落本表**——只写 `governance.audit_log` + 既有 `integration.job` 记录（job 表已存在，见 `crates/pm-infrastructure/src/job_queue`）。理由：失败产物无消费价值，且会放大容量与清理负担。

### 3.5 `viz.slot_config`

```sql
CREATE TABLE viz.slot_config (
    id                uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id         uuid NOT NULL,
    project_id        uuid NOT NULL,
    slot_key          text NOT NULL,
    view_kind         text NOT NULL,
    require_approval  boolean DEFAULT true NOT NULL,
    auto_stale        boolean DEFAULT true NOT NULL,
    allowed_nature    text DEFAULT 'business' NOT NULL,
    max_versions      integer DEFAULT 20 NOT NULL,
    retention_days    integer DEFAULT 180 NOT NULL,
    row_revision      bigint DEFAULT 1 NOT NULL,
    created_at        timestamp with time zone DEFAULT now() NOT NULL,
    updated_at        timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT viz_slot_config_pk PRIMARY KEY (id),
    CONSTRAINT viz_slot_config_slot_key_check CHECK (length(btrim(slot_key)) > 0),
    CONSTRAINT viz_slot_config_nature_check CHECK (allowed_nature = ANY (ARRAY['business'::text,'explanatory'::text,'both'::text])),
    CONSTRAINT viz_slot_config_max_versions_check CHECK (max_versions >= 1),
    CONSTRAINT viz_slot_config_retention_check CHECK (retention_days >= 0)
);
CREATE UNIQUE INDEX uq_viz_slot_config_slot ON viz.slot_config (project_id, slot_key);
```

**无行兜底**：slot 未配置时使用默认策略（`require_approval=true`、`allowed_nature=business`、`max_versions=20`、`retention_days=180`），避免"未配置=默认放行"的静默风险。

## 4. 状态机（R-04 闭合：两台分离）

### 4.1 Artifact 状态机

```
        ┌──────────────► rejected（终态；理由在 viz.approval）
        │  审批拒绝
  draft ──────────► published ──────────► stale
        │  审批通过         源对象变更
        │ （或免审批槽位直接发布）
```

| 迁移 | 触发 | 守卫 |
|---|---|---|
| draft → published | 审批通过（`require_approval=true`）或免审批槽位直接发布 | `viz_artifact_nature_publish_check` + slot 的 `allowed_nature` + 触发器 |
| published → stale | stale 检测命中 | 触发器允许；**不改 spec** |
| draft → rejected | 审批拒绝 | 触发器禁止再变更 |

**published 不可回退、不可再编辑**（触发器强制）。

### 4.2 Approval 状态机

```
  pending ──► approved   （decided_by + decided_at 必填）
     │
     ├──────► rejected   （decision_reason 必填，CHECK 强制）
     └──────► cancelled  （撤回）
```
终态不可再改（触发器）。**Approval 状态不写回 artifact.state 之外的任何字段**；artifact.state 由审批结果驱动，但两表字段物理分离（artifact 无 decided_by/reason 列）。

### 4.3 两表交互（应用层事务）

```
BEGIN
  1. SELECT ... FROM viz.artifact WHERE id=$1 FOR UPDATE        -- 取 artifact + row_revision
  2. 校验 target_hash == artifact.spec_hash                      -- 否则 409（漂移）
  3. 校验 expected_version_no == artifact.version_no
  4. UPDATE viz.approval SET status='approved', decided_by=$2, decided_at=now() WHERE id=$3 AND status='pending'
  5. UPDATE viz.artifact SET state='published', published_by=$2, published_at=now() WHERE id=$1
  6. INSERT INTO governance.audit_log (...)                       -- 同事务
COMMIT
```
并发保障：pending 唯一索引冲突 → `is_unique_violation` → 409（沿用 `design.rs:1791-1797` 的 `is_unique_violation` → Conflict 写法）。

## 5. 容量与清理规则（R-05 闭合）

| 项 | 规则 | 依据/理由 |
|---|---|---|
| 单 Artifact spec 大小 | ≤ 256 KB | 校验器上限（12 号 §7），DB 侧再兜 `jsonb` 大小检查（应用层） |
| 单 render_output（html） | ≤ 2 MB | 产物为自包含 HTML，含内联脚本与样式 |
| 单 render_output（svg） | ≤ 8 MB | 矢量 + 内嵌字体可偏大 |
| 单 render_output（png） | ≤ 4 MB | **条件能力**，R-03 未闭合前不承诺 |
| 每槽位保留版本数 | `slot_config.max_versions`（默认 20） | 超出部分由清理 sweep 删除 |
| 保留期 | `slot_config.retention_days`（默认 180 天） | 派生物过期即删；**已发布且被引用的版本不删**（引用方为 artifact 自身，无外键引用 → 以 `published_at` 超过保留期且非当前版本为删除条件） |
| 失败产物 | **不落表**，仅审计 + job 记录 | §3.4 |
| 清理任务挂载 | `apps/ai-pm-worker` 的 60s 轮询循环内新增一个 sweep（先例：`interaction_timeout_sweep` `main.rs:189`、`run_cleanup_round` `main.rs:196`） | 仓内无现成 TTL 框架，必须自建 |
| 清理实现范式 | 幂等 + 分批 + `FOR UPDATE SKIP LOCKED`（先例 `run_cleanup.rs:57-77`，`CLEANUP_BATCH_LIMIT = 64`） | 避免长事务 |
| 备份与恢复 | 与业务表同库同实例 → 随主库备份；**产物可由 spec + renderer 重渲染重建**（renderer_id/version 必留） | ADR-VIZ-006 |
| 大文件读取成本 | 列表端点**不返回 content**；产物只经 `GET .../outputs/{format}` 单条取；单次读取上限 2 MB（HTML） | 避免列表接口拖垮响应 |
| 清理与 stale 判定的关系 | stale 标记不触发删除；**stale 产物是"过期但仍可解释"的证据**，保留期从 published_at 起算 | ADR-VIZ-006 |

**仓内事实提醒**：仓内**不存在**任何"按 created_at 保留 N 天后删除"的先例（`delivery.task_run_cleanup` 是状态机账本而非 TTL）。本节规则是 V0-2 新建的先例，需在实施报告中标明。

## 6. 服务层与 API

### 6.1 服务层落点

`crates/pm-application/src/visualization/`（新子目录，遵循 `crates/pm-application/src/{review,pc,discovery,delivery,kb_service}` 范式）：`mod.rs`（模块真源 + `pub use`）、`artifact.rs`、`approval.rs`、`slot.rs`、`source_hash.rs`、`cleanup.rs`；错误类型 `VisualizationError`（thiserror，`NotFound/Conflict/Validation/Forbidden/Db`，参照 `ContextItemError` `ai_context_items.rs:122-144`）。

**SQL 直写在 service 模块内**（仓内无 repository 层惯例：`crates/pm-application/src/design.rs`、`ai_context_items.rs` 同款）。

**审计**：所有写操作同事务写 `governance.audit_log`，用既有 `append_audit(tx, project_id, actor, action, target_type, target_id)` / `append_audit_with_after(...)`（`crates/pm-application/src/pm_write/audit.rs:39-100`），action 命名点分：`viz.artifact.created` / `viz.artifact.published` / `viz.approval.requested` / `viz.approval.decided` / `viz.artifact.stale`。

### 6.2 API 端点

新模块 `apps/pm-api/src/viz_artifacts.rs` 提供 `pub(crate) fn router() -> Router<AppState>`，在 `routes.rs` 的 `build_router()` 内 `.merge(crate::viz_artifacts::router())`，合并处**必须带中文注释说明归属**（先例 `routes.rs:283-285`）。

| 方法 | 路径 | 说明 | 门 |
|---|---|---|---|
| GET | `/v1/pm/visualizations/projects/{project_id}/slots` | 槽位列表 + 当前版本摘要 | `require_project_visible` |
| GET | `/v1/pm/visualizations/artifacts/{artifact_id}` | 详情（含 spec 与 provenance） | 可见性（artifact→project→成员） |
| GET | `/v1/pm/visualizations/projects/{project_id}/artifacts?slot_key=&state=&page=&size=` | 历史版本列表（分页） | `require_project_visible` |
| POST | `/v1/pm/visualizations/projects/{project_id}/artifacts` | 提交 spec → 创建 draft | `require_project_visible` + 真人门 |
| POST | `/v1/pm/visualizations/artifacts/{artifact_id}/approval-requests` | 请求审批 | `ensure_trusted_context` + 真人门 |
| POST | `/v1/pm/visualizations/approvals/{approval_id}/decision` | 批准/拒绝/撤回 | `ensure_trusted_context` + 真人门 + `require_human` |
| GET | `/v1/pm/visualizations/artifacts/{artifact_id}/outputs/{format}` | 取渲染产物 | 可见性；单次上限 2 MB |
| GET | `/v1/pm/visualizations/projects/{project_id}/live/{view_kind}` | Live 图查询 | 可见性；**实现属 V0-3**（14 号） |
| POST | `/v1/pm/visualizations/projects/{project_id}/artifacts/{artifact_id}/refresh` | 触发 stale 重算（同步） | 可见性 |

**路由前缀偏差（需 ADR 修订）**：ADR-VIZ-001 写 `/v1/visualizations/*`，但仓库惯例是 **pm-api 资源全量 `/v1/pm/` 前缀、无 `/api` 前缀**（`routes.rs:231-232` 注释）。建议改为 **`/v1/pm/visualizations/*`**，保持与 ADR 描述等价（分区独立）且符合仓内路径惯例。→ 记为 ADR-001 修订候选（待 Architecture Owner 确认，见 §9）。

**DTO 约定**：请求体在 handler 模块内 `pub(crate) struct *Body`（`ai_context_items.rs:102`）；service 侧请求结构 `*Request` 放 pm-application（`design.rs:345-356`）。分页用既有 `Paging::to_response` → `{items,total,page,size}`（`support.rs:130-139`，MAX_SIZE=200）。

**错误映射**：`ApiError` 错误体只有 `{message}`（`support.rs:1-6`），故校验问题列表不进错误体：
- 提交非法 spec → 400 `{"message": "spec 校验未通过：E_SPEC_… @ …；…"}`（前 N 条 code + 定位）
- 目标哈希漂移 → 409 `{"message": "artifact changed since approval request"}`
- pending 冲突 → 409 `{"message": "an approval request is already pending for this slot"}`
- 跨项目/非成员 → 404 同形（`"artifact not found"`），**不出现"无权"字样**（`pm_requirement_read.rs:77-93` 范式）

**无 OpenAPI**：仓内无 utoipa/openapi，契约以本文 + `docs/visualization/` 散文契约为准；前端 DTO 落 `ai-frontend/src/api/visualizations.ts`（每域一文件惯例）。

## 7. 测试与门禁

| 项 | 方案 |
|---|---|
| 集成测试位置 | `crates/pm-application/tests/visualization.rs`（域测试）+ `apps/pm-api/tests/viz_artifacts.rs`（API 层） |
| 迁移重放 | `#[sqlx::test(migrations = "../../migrations")]`（全仓 1120 处同款），每个用例建库重放全量迁移 |
| **新增迁移后必做** | `bash scripts/seed-test-template.sh` 重播种 template1（否则 `sqlx::test` 走慢路径；schema 钉在迁移集上，`seed-test-template.sh:18-27`）；`DATABASE_URL` 写 `127.0.0.1`（AGENTS.md 规则 3） |
| 状态机用例 | draft→published→stale；draft→rejected（reason 必填）；published 不可改 spec（触发器报错）；pending 唯一（并发第二条 → 409） |
| 容量用例 | spec 超 256 KB → 400；html 产物超 2 MB → 拒绝入库；版本数超 max_versions → 清理 sweep 保留最近 N |
| 隔离用例 | 跨租户/非成员读取 → 404 同形；api_key 发布 → 403（`ensure_trusted_context`） |
| 迁移号纪律 | **0020**（按台账「当前目录最大 + 1」；旧链号段不参与）；台账补登 0015~0019 与 0020 |

## 8. 实施任务清单

| # | 任务 | 完成定义 |
|---|---|---|
| V0-2.1 | 取号签收与台账补登 | Migration Owner 签收 0061；台账 §〇·二 补 0015~0019 + 0061 行；`migrations/0020_visualization_artifact.sql` 建档 |
| V0-2.2 | 迁移文件 | 四表 + 索引 + 触发器 + CHECK 全部落文件；头注含用途/口径/依据文档/取号史（0019 样板 `0019:6-27`） |
| V0-2.3 | 服务层骨架 | `crates/pm-application/src/visualization/` + `VisualizationError` + 审计写入 |
| V0-2.4 | Artifact CRUD + 状态机 | 创建 draft / 发布 / 标 stale；触发器守卫生效 |
| V0-2.5 | Approval 流程 | 请求/裁决/撤回；哈希漂移 409；pending 唯一 409 |
| V0-2.6 | 产物存取 | 写入/读取 render_output；大小上限；失败不落表 |
| V0-2.7 | 槽位配置 | slot_config CRUD + 默认兜底策略 |
| V0-2.8 | 清理 sweep | worker 60s 循环挂载；幂等 + 分批 + SKIP LOCKED |
| V0-2.9 | API 端点 | §6.2 端点 + 分页 + 错误映射 + 404 同形 |
| V0-2.10 | 测试 | §7 全部用例绿；template1 重播种后 `scripts/test.sh` 全绿 |

## 9. 待签收与偏差

| # | 项 | 决策人 | 说明 |
|---|---|---|---|
| D-1 | **迁移号 0061** | Migration Owner | §2 论证；旧环境 VersionMissing 风险 |
| D-2 | **路由前缀** `/v1/pm/visualizations/*` | Architecture Owner | ADR-VIZ-001 原文为 `/v1/visualizations/*`；建议按仓内 `/v1/pm/` 惯例修订（§6.2） |
| D-3 | 解释图存储位置 | Architecture Owner | 现设计：explanatory artifact 可入库但不可 published；是否需独立表/独立命名空间？ |
| D-4 | 发布权限角色 | Architecture Owner | 仓内实际授权只认 `core.project_member.role` 的 owner/member（`approver.rs:36-44`、`pm_runs.rs:74-95`），agent 职能角色（ai_pm/architect/developer/tester）不是授权角色 —— 发布到共享槽位是否限定 owner？（V0-3 一并落定） |
| D-5 | 保留期默认值 | Product/Backend | 180 天 / 20 版本 / 2 MB 是否符合预期 |
| D-6 | PNG 列预留 | Security/Backend | `format='png'` 列已预留但能力未闭合（R-03），是否保留预留 |
