# S01 · I6-B V1.1 修订正文（Discovery / Import 收敛与跨 Project 约束）

> 文档性质：I6-B 唯一有效正文 + Migration `0031` 的实施依据
> 状态：**待 DDL 审查冻结**（审查清单见 §13）
> 版本：V1.1　日期：2026-11-03
> 上位：`docs/s01-全局主菜单与信息架构设计基线/ui/smark-pm-s01-overall-implementation-plan-v1.md`（总体实施计划 V1.0）、`docs/s01-全局主菜单与信息架构设计基线/docs/s01-implementation-plan-v1.md`（S01 冻结基线）

---

## 0. 文档状态与输入

### 0.1 为什么是「新写」而不是「改写」

I6-B V1.0 正文**不在本仓**。`docs/ui/` 现有的 I6-B 相关文档只有四份：

| 文档 | 性质 | 是否正文 |
|---|---|---|
| `smark-pm-s01-i6-code-facts-v1.md` | I6-A 代码事实核验报告 | 否（事实源） |
| `smark-pm-s01-i6-b-boundary-review-346-v1.md` | 边界评审记录（R-3～R-6） | 否（裁决来源） |
| `smark-pm-s01-i6-b-v1-verification-feedback.md` | 核验反馈（C-1～C-4 / N-1～N-5） | 否（裁决来源） |
| `smark-pm-s01-i6-b-v1-1-merge-spec.md` | 合并规格 V1.0 | 否（DDL 级输入） |

因此 V1.1 **依据上述四份 + 当前代码事实重新成文**，而不是对 V1.0 正文做增量修订。合并规格 §7 的「V1.0 章节修订清单」在本稿中已**逐条落地**（对照见 §13.3）。

> **可追溯性声明**：本稿自定了 `I6-B-036~I6-B-051` 十六条验收（原两稿均称「来自裁决 §11」，该裁决文档不在本仓）。若日后找回原裁决文档，须**逐条比对**，冲突以原裁决为准；本稿的编号**从 036 起已被占用，不得回填他义**。

### 0.2 输入清单

| # | 输入 | 用途 |
|---|---|---|
| 1 | 合并规格 V1.0（§1 D-1a / §2 D-2a 与补-1/2/3 / §3 FK 策略 / §4 负向测试映射） | DDL 主输入 |
| 2 | 边界评审 346（§1 R-3-1~R-3-4、§2 R-4-1、§3 R-6-1/R-6-2） | 状态机与归属裁决 |
| 3 | 核验反馈（§2 C-1~C-4、§3 N-1~N-5） | 业务语义裁决 |
| 4 | I6-A 代码事实报告（22 缺陷 D-01~D-22） | 缺陷台账 |
| 5 | 当前代码：`migrations/0009/0019/0025`、`crates/pm-application/src/project_import/`、`crates/pm-application/src/discovery/` | 现状核对 |
| 6 | 用户裁决 2026-11-03（B-2：按现有三稿新写正文并自定验收） | 本稿授权 |

### 0.3 裁决收编状态

| 组 | 条目 | 状态 |
|---|---|---|
| C | C-1（Import 一次性 bootstrap）、C-2（proposed 须显式终止）、C-3（索引变更走新增迁移）、C-4（删除受历史引用保护） | ✅ 全收，进正文 |
| N | N-1（基准 write-once SHA）、N-2（冻结时终态化在飞 Import）、N-3（`repository_ids` 必填）、N-4（幂等键透传）、N-5（lease 过期重认领） | ✅ 全收，进正文 |
| R-3 | R-3-1（占有/释放检查 `rows_affected`）、R-3-2（删测试专用初始化变体）、R-3-3（active 部分唯一索引）、R-3-4（权威落操作表 + 行锁） | ✅ 全收，进正文 |
| R-4 | R-4-1（facts/计数/错误迁 Attempt） | ✅ 进正文 |
| R-6 | R-6-1（Source 四列 Project 作用域键）、R-6-2（Evidence CHECK 判别器） | ✅ 进正文 |
| 补 | 补-1（session 三列唯一键）、补-2（Evidence 用 `attempt_no` 组合 FK）、补-3（finding_repository 升格） | ✅ 进正文 |
| D | D-1a（草案版本表）、D-2a（数据库级 Project 归属约束） | ✅ 进正文 |
| — | I6-B-036~051 的原始定义 | ⚠️ 缺失 → 本稿自定（§11.1） |

### 0.4 与 `AGENTS.md` 硬规则 1 的冲突处理

`AGENTS.md` 规则 1：**新项目，无存量数据，不写迁移脚本 / 双写 / 兼容分支**。

本稿据此调整：

| 合并规格原要求 | 本稿处置 |
|---|---|
| 历史数据回填 | **取消**（`AGENTS.md` 规则 1）；改为「空库初始化 + 结构断言」 |
| `repo_import.draft` 单列「停用（保留兼容读）」 | **直接删列**（无兼容读） |
| `repo_import_source` 加列 + 回填 `project_id` | **整表 DROP + CREATE**（该表无任何 FK 依赖方，见 §3.4 依赖分析） |
| `discovery_finding` 加 `project_id` + 回填 | **保留一次性 UPDATE 对齐**（例外，见 §0.5 C-14） |

### 0.5 本稿追加的裁决

| 编号 | 裁决 | 理由 |
|---|---|---|
| **C-14** | `discovery_finding` / `discovery_finding_repository` 的 `project_id` 采用 **ADD COLUMN + 一次性 UPDATE + SET NOT NULL**（而非整表重建） | 这两张表被 `discovery_question.finding_id` 与既有 Discovery 数据引用；整表重建会连带销毁 Discovery 域既有数据并重建三条 FK，代价远大于一次 UPDATE。AGENTS 规则 1 禁止的是「兼容分支 / 双写 / 过渡层」，一次性结构对齐不属于该类 |
| **C-15** | `repo_import.draft_revision` 保留为**指针**（= 最新 `succeeded` 草案版本号），不再自增计数 | D-1a 要求「指针语义」：生成失败时旧 `succeeded` 版本与指针均不动 |
| **C-16** | `repo_import_source` 保留 `head_commit_sha`（基准，write-once）与 `current_attempt_no`（当前轮指针），**删除** `facts` / `commit_count` / `doc_count` / `size_bytes` / `truncated` / `error_code` | R-4-1：这些是「某一轮的事实」，属 Attempt 层；Source 只留身份、基准与当前态指针 |
| **C-17** | `discovery_finding.evidence` JSONB 列**删除**，证据改由 `discovery.discovery_finding_evidence` 表承载 | 「证据」需要可追溯到 Attempt 与 Commit，JSONB 数组做不到；且与 I6-B-04 的 Evidence 实体模型一致 |

---

## 1. 目标、范围与不做

### 1.1 目标

把「代码发现 / 仓库接入」从**过程脚本**变成**可追溯、可恢复、可约束**的业务流程：

1. 每次仓库分析轮次（Attempt）**留档**，成功与失败都留；
2. 每份草案（Draft）**带版本**，不覆盖历史，并记录它消费了哪一轮 Attempt；
3. **同 Project 归属由数据库保证**——Evidence → Attempt → Source → Import → Project 与 Finding → Session → Project 两条链，跨 Project 关联在 DB 层不可构造；
4. 仓库被历史引用时**不可物理删除**（RESTRICT + 409）。

### 1.2 范围

| 覆盖 | 不覆盖 |
|---|---|
| Import / Source / Attempt / Draft 四层模型 | Discovery Session 的状态机语义（属 I6-B-05 接口收敛） |
| Evidence 实体与类型判别器 | Finding 的人工确认流程（属 I2） |
| 初始化操作与 active 唯一性 | Project 归档流程（属 I1-D） |
| 仓库删除保护 | Git 能力授权（属 I3） |
| 审计与幂等 | Knowledge 发布管线（属 S01-5，已建成） |

### 1.3 明确不做

- 不引入第二套任务调度、事件总线或完整事件溯源。
- 不合并 `discovery.discovery_proposal` 与 `ai.document_proposal`（两套独立生命周期）。
- 不在 `core.project` 增列承载 generation（R-3-4）。
- 不自动合并 / 自动发布知识。
- 不推翻既有 Task 状态机。

---

## 2. 领域边界

以下对象**名称相似但状态机独立**，禁止合并：

| 对象 | 职责 | 状态机 |
|---|---|---|
| `discovery.discovery_session` | 发现过程（一次认知会话） | 5 态（0025） |
| `integration.repo_import` | 仓库接入与分析工作流 | 10 态（0009） |
| `integration.repo_import_source` | 一次导入里某个仓库的**身份 + 基准 + 当前态指针** | 无状态 |
| `integration.repo_import_source_attempt` | 某仓库的一次分析轮次 | 4 态 |
| `integration.repo_import_draft_version` | 某版导入草案 | 3 态（generating/succeeded/failed） |
| `discovery.discovery_finding` | 一条发现事实 | kind × verification_status |
| `discovery.discovery_finding_evidence` | 一条证据（可追溯到 Attempt） | 无状态 |
| `discovery.discovery_question` / `_answer` | 人工补充信息 | open/answered/deferred |
| `discovery.discovery_proposal` | 待处理的发现提案 | 4 态 |
| `ai.document_proposal` | 知识稿件发布 | 8 态（0014） |

> **Attempt ≠ Run**。Run 是「任务的一次 Agent 执行」；Attempt 是「仓库的一次分析轮次」。二者生命周期、归属、恢复语义都不同，不得混用。

---

## 3. 数据模型（Migration `0031_s01_discovery_import_v2.sql`）

### 3.1 对象清单（14 项）

| # | 对象 | 动作 | 依据 |
|---|---|---|---|
| 1 | `integration.repo_import` | 补 `UNIQUE (tenant_id, project_id, id)`；**删 `draft` 列**；`draft_revision` 降为指针 | D-2a、C-15 |
| 2 | `integration.project_repository` | 补 `UNIQUE (tenant_id, project_id, id)` | 补-3 |
| 3 | `discovery.discovery_session` | 补 `UNIQUE (tenant_id, project_id, id)` | 补-1 |
| 4 | `core.project_initialization_operation` | **新建** | R-3-3、R-3-4 |
| 5 | `integration.repo_import_draft_version` | **新建** | D-1a |
| 6 | `integration.repo_import_source` | **DROP + CREATE**（新形状） | R-4-1、R-6-1、C-16 |
| 7 | `integration.repo_import_source_attempt` | **新建** | R-4-1、补-2 |
| 8 | `discovery.discovery_finding` | 加 `project_id` + 换轨 FK + 删 `evidence` 列 | C-17、D-2a |
| 9 | `discovery.discovery_finding_evidence` | **新建** | I6-B-04、R-6-2 |
| 10 | `discovery.discovery_finding_repository` | 加 `project_id` + 两条换轨 FK（RESTRICT） | 补-3 |
| 11 | `discovery.discovery_question` | `finding_id` FK 换轨携带 project | D-2a |
| 12 | `discovery.discovery_proposal` | session FK 换轨携带 project | D-2a |
| 13 | `delivery.task_repository` | 仓库 FK 由 NO ACTION 改 **RESTRICT** | C-4 |
| 14 | `integration.repo_import_one_active_per_project` | 部分唯一索引重建（租户化） | C-3、R-6-1 |

### 3.2 完整 DDL

见 `migrations/0031_s01_discovery_import_v2.sql`。结构如下（**与迁移文件逐字一致**）：

```sql
-- ── 0. 组合唯一键（复合 FK 的父键锚点）──────────────────────────────
ALTER TABLE integration.repo_import
    ADD CONSTRAINT uq_repo_import_tenant_project_id UNIQUE (tenant_id, project_id, id);

ALTER TABLE integration.project_repository
    ADD CONSTRAINT uq_project_repository_tenant_project_id UNIQUE (tenant_id, project_id, id);

ALTER TABLE discovery.discovery_session
    ADD CONSTRAINT uq_discovery_session_tenant_project_id UNIQUE (tenant_id, project_id, id);

-- ── 1. Project 初始化操作（R-3-3 / R-3-4）───────────────────────────
CREATE TABLE core.project_initialization_operation (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   UUID NOT NULL,
    project_id  UUID NOT NULL,
    import_id   UUID,
    generation  BIGINT NOT NULL,
    status      TEXT NOT NULL CHECK (status IN ('active','released','completed','cancelled')),
    created_by  UUID REFERENCES core.principal(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ,
    UNIQUE (tenant_id, project_id, generation),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX uq_project_initialization_operation_active
    ON core.project_initialization_operation (project_id) WHERE status = 'active';

-- ── 2. 草案版本表（D-1a）──────────────────────────────────────────
CREATE TABLE integration.repo_import_draft_version (
    tenant_id        UUID NOT NULL,
    import_id        UUID NOT NULL,
    revision         BIGINT NOT NULL,
    draft            JSONB NOT NULL,
    source_attempts  JSONB NOT NULL DEFAULT '[]'::jsonb,
    dropped_items    INT NOT NULL DEFAULT 0,
    generation_status TEXT NOT NULL CHECK (generation_status IN ('generating','succeeded','failed')),
    generated_at     TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, import_id, revision),
    FOREIGN KEY (tenant_id, import_id) REFERENCES integration.repo_import (tenant_id, id) ON DELETE CASCADE
);

-- ── 3. Source 整表重建（R-4-1 / R-6-1 / C-16）──────────────────────
DROP TABLE integration.repo_import_source;
CREATE TABLE integration.repo_import_source (
    tenant_id         UUID NOT NULL,
    project_id        UUID NOT NULL,
    import_id         UUID NOT NULL,
    repository_id     UUID NOT NULL,
    head_commit_sha   TEXT,                       -- 基准：write-once（N-1）
    current_attempt_no INT,                       -- 当前轮指针
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, import_id, repository_id),
    UNIQUE (tenant_id, project_id, import_id, repository_id),
    FOREIGN KEY (tenant_id, project_id, import_id)
        REFERENCES integration.repo_import (tenant_id, project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, repository_id)
        REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE RESTRICT
);

-- ── 4. Attempt 表（R-4-1 / 补-2）───────────────────────────────────
CREATE TABLE integration.repo_import_source_attempt (
    tenant_id     UUID NOT NULL,
    project_id    UUID NOT NULL,
    import_id     UUID NOT NULL,
    repository_id UUID NOT NULL,
    attempt_no    INT NOT NULL,
    status        TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','cancelled')),
    commit_sha    TEXT,
    facts         JSONB,
    commit_count  INT,
    doc_count     INT,
    size_bytes    BIGINT,
    truncated     BOOLEAN,
    error_code    TEXT,
    error_detail  TEXT,
    job_id        UUID,
    started_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at   TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, import_id, repository_id, attempt_no),
    UNIQUE (tenant_id, project_id, import_id, repository_id, attempt_no),
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id)
        REFERENCES integration.repo_import_source (tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE,
    CHECK (finished_at IS NULL OR finished_at >= started_at)
);

-- ── 5. Finding 升格（C-17 / D-2a / 补-1）────────────────────────────
ALTER TABLE discovery.discovery_finding ADD COLUMN project_id UUID;
UPDATE discovery.discovery_finding f
   SET project_id = s.project_id
  FROM discovery.discovery_session s
 WHERE s.tenant_id = f.tenant_id AND s.id = f.session_id;
ALTER TABLE discovery.discovery_finding ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_finding
    ADD CONSTRAINT uq_discovery_finding_tenant_project_id UNIQUE (tenant_id, project_id, id);
ALTER TABLE discovery.discovery_finding DROP COLUMN evidence;
ALTER TABLE discovery.discovery_finding DROP CONSTRAINT discovery_finding_tenant_id_session_id_fkey;
ALTER TABLE discovery.discovery_finding
    ADD CONSTRAINT fk_discovery_finding_tenant_project_session
    FOREIGN KEY (tenant_id, project_id, session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id) ON DELETE CASCADE;

-- ── 6. Evidence 实体（R-6-2 / I6-B-04）─────────────────────────────
CREATE TABLE discovery.discovery_finding_evidence (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL,
    project_id    UUID NOT NULL,
    finding_id    UUID NOT NULL,
    evidence_type TEXT NOT NULL CHECK (evidence_type IN ('code','test_result','commit','user_statement','system_observation')),
    description   TEXT,
    file_path     TEXT,
    symbol        TEXT,
    code_range    TEXT,
    import_id     UUID,
    repository_id UUID,
    commit_sha    TEXT,
    attempt_no    INT,
    created_by    UUID REFERENCES core.principal(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, project_id, finding_id)
        REFERENCES discovery.discovery_finding (tenant_id, project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id)
        REFERENCES integration.repo_import_source (tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id, attempt_no)
        REFERENCES integration.repo_import_source_attempt
               (tenant_id, project_id, import_id, repository_id, attempt_no) ON DELETE CASCADE,
    CHECK (
        (evidence_type IN ('code','test_result','commit')
          AND import_id IS NOT NULL AND repository_id IS NOT NULL
          AND commit_sha IS NOT NULL AND attempt_no IS NOT NULL)
        OR
        (evidence_type IN ('user_statement','system_observation')
          AND import_id IS NULL AND repository_id IS NULL
          AND commit_sha IS NULL AND attempt_no IS NULL)
    )
);

-- ── 7. finding_repository 升格（补-3）───────────────────────────────
ALTER TABLE discovery.discovery_finding_repository ADD COLUMN project_id UUID;
UPDATE discovery.discovery_finding_repository r
   SET project_id = f.project_id
  FROM discovery.discovery_finding f
 WHERE f.tenant_id = r.tenant_id AND f.id = r.finding_id;
ALTER TABLE discovery.discovery_finding_repository ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_finding_repository
    DROP CONSTRAINT discovery_finding_repository_tenant_id_repository_id_fkey;
ALTER TABLE discovery.discovery_finding_repository
    DROP CONSTRAINT discovery_finding_repository_tenant_id_finding_id_fkey;
ALTER TABLE discovery.discovery_finding_repository
    ADD CONSTRAINT fk_finding_repository_tenant_project_finding
    FOREIGN KEY (tenant_id, project_id, finding_id)
    REFERENCES discovery.discovery_finding (tenant_id, project_id, id) ON DELETE CASCADE;
ALTER TABLE discovery.discovery_finding_repository
    ADD CONSTRAINT fk_finding_repository_tenant_project_repository
    FOREIGN KEY (tenant_id, project_id, repository_id)
    REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE RESTRICT;

-- ── 8. 其余 Finding 引用链换轨 ─────────────────────────────────────
ALTER TABLE discovery.discovery_question DROP CONSTRAINT discovery_question_tenant_id_finding_id_fkey;
ALTER TABLE discovery.discovery_question
    ADD CONSTRAINT fk_discovery_question_tenant_project_finding
    FOREIGN KEY (tenant_id, project_id, finding_id)
    REFERENCES discovery.discovery_finding (tenant_id, project_id, id) ON DELETE SET NULL;
-- discovery_question 需补 project_id 列（从 finding 继承）
ALTER TABLE discovery.discovery_question ADD COLUMN project_id UUID;
UPDATE discovery.discovery_question q
   SET project_id = f.project_id
  FROM discovery.discovery_finding f
 WHERE f.tenant_id = q.tenant_id AND f.id = q.finding_id;
ALTER TABLE discovery.discovery_question ALTER COLUMN project_id SET NOT NULL;
ALTER TABLE discovery.discovery_proposal DROP CONSTRAINT discovery_proposal_tenant_id_session_id_fkey;
ALTER TABLE discovery.discovery_proposal
    ADD CONSTRAINT fk_discovery_proposal_tenant_project_session
    FOREIGN KEY (tenant_id, project_id, session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id) ON DELETE CASCADE;
ALTER TABLE discovery.discovery_proposal ADD COLUMN project_id UUID;
UPDATE discovery.discovery_proposal p
   SET project_id = s.project_id
  FROM discovery.discovery_session s
 WHERE s.tenant_id = p.tenant_id AND s.id = p.session_id;
ALTER TABLE discovery.discovery_proposal ALTER COLUMN project_id SET NOT NULL;

-- ── 9. 仓库删除保护（C-4）──────────────────────────────────────────
ALTER TABLE delivery.task_repository DROP CONSTRAINT task_repository_project_id_repository_id_fkey;
ALTER TABLE delivery.task_repository
    ADD CONSTRAINT fk_task_repository_tenant_project_repository
    FOREIGN KEY (tenant_id, project_id, repository_id)
    REFERENCES integration.project_repository (tenant_id, project_id, id) ON DELETE RESTRICT;

-- ── 10. Import 名额索引重建（C-3）──────────────────────────────────
DROP INDEX IF EXISTS integration.repo_import_one_in_progress;
CREATE UNIQUE INDEX repo_import_one_active_per_project
    ON integration.repo_import (tenant_id, project_id)
    WHERE status IN ('queued','cloning','reading','analyzing');
```

> 注：`repo_import.draft` 列删除、`draft_revision` 指针语义在 §4.1 表述，DDL 中为
> `ALTER TABLE integration.repo_import DROP COLUMN draft;`（见迁移文件）。

### 3.3 约束链（DB 级保证）

```text
core.project (tenant_id, id)
   ▲
   │ (tenant_id, project_id, id) 组合唯一键
   ├──▶ integration.repo_import ──(t,p,import)──▶ repo_import_source ──(t,p,import,repo)──▶ repo_import_source_attempt
   │            ▲                                          ▲                                          ▲
   │            │ UNIQUE (t,p,import,repo)                │                                          │
   │            └──────────────────────────────────────────┘                                          │
   │                                                                                                   │
   └──▶ discovery.discovery_session ──(t,p,session)──▶ discovery_finding ──(t,p,finding)──▶ discovery_finding_evidence
                                                      │                                              │
                                                      └──(t,p,repo)──▶ project_repository            └──(t,p,import,repo,attempt_no)──▶ Attempt
```

两条链：

- **Evidence → Attempt → Source → Import → Project**
- **Finding → Session → Project**

每条边都携带 `project_id`，因此「同 Project 归属」由**数据库保证**，不存在 UUID 存在性漂移洞。

### 3.4 依赖分析（为何 `repo_import_source` 可整表重建）

| 引用方 | 类型 | 结论 |
|---|---|---|
| SQL 外键 | 无任何表 `REFERENCES integration.repo_import_source` | 可 DROP |
| Rust 代码 | `project_import/lifecycle.rs:183,396,464`（SELECT / INSERT） | 需随迁移同步改写（I6-B-02） |
| Schema 测试 | `pm-infrastructure/tests/tenant_schema.rs:68`（表清单） | 表名不变，仅列集变化 |

### 3.5 FK 删除策略定案

| FK | ON DELETE | 理由 |
|---|---|---|
| source → import | CASCADE | 沿既有语义（0019:381）；Import 随 Project 消亡 |
| source → project_repository | **RESTRICT** | C-4：历史引用阻止仓库物理删除 |
| attempt → source | CASCADE | Attempt 随 Source 生命周期 |
| evidence → finding | CASCADE | Evidence 随 Finding 消亡 |
| evidence → source（四列） | CASCADE | Source 仅随 Import/Project 级联消亡 |
| evidence → attempt（五列） | CASCADE | 同上 |
| finding → session | CASCADE | 沿既有（0025:61） |
| finding_repository → finding | CASCADE | 沿既有 |
| finding_repository → repository | **RESTRICT** | C-4 同口径 |
| question → finding | SET NULL | 问题可独立存在（finding_id 可空） |
| proposal → session | CASCADE | 沿既有 |
| task_repository → project_repository | **RESTRICT** | C-4；应用层映射 409 |

---

## 4. 状态机

### 4.1 Import 状态机（`integration.repo_import.status`）

既有 10 值不变：`queued / cloning / reading / analyzing / draft_ready / proposed / applied / rejected / failed / cancelled`。

| from → to | 触发 | 约束 |
|---|---|---|
| （新建）→ `queued` | `start_import` | 同一 `(tenant_id, project_id)` 至多一个 active（部分唯一索引）；`applied` 之后再建返回 **409 `project_not_empty`**（C-1） |
| `queued` → `cloning` → `reading` → `analyzing` | Worker 推进 | 每一步写 Attempt |
| `analyzing` → `draft_ready` | 生成草案成功 | 先插 `repo_import_draft_version(generation_status='generating')` → 成功转 `succeeded` + 推进 `repo_import.draft_revision` 指针；失败转 `failed`，**旧 succeeded 版本与指针不动** |
| `draft_ready` → `analyzing`（重试） | 任一 Source 产生新的成功 Attempt | **允许**；重试成功后**重新生成草案**（新 revision + 1）；`source_attempts` 记录本版消费的每 Source 最新成功轮 |
| `draft_ready` → `proposed` | 提交提案 | 之后**禁止重试** |
| `proposed` → `applied` / `rejected` | 人工裁决 | — |
| `proposed` → `cancelled` | **显式终止 / 放弃**（C-2） | 必须提供显式操作；否则 active 名额永久锁死 |
| 任意 → `cancelled` | 人类取消 / 冻结终态化（N-2） | 冻结或归档时**终态化在飞 Import** |
| 任意 → `failed` | 不可恢复错误 | — |

**状态机与「active 名额」的关系**：`repo_import_one_active_per_project`（部分唯一索引）与 `uq_project_initialization_operation_active`（操作表部分唯一索引）**语义不同、并存**：

| 索引 | 回答的问题 | 生命周期 |
|---|---|---|
| `repo_import_one_active_per_project` | 这个 Project 的 **Import 工作流**是否已有一次在跑 | Import 创建 → 终态 |
| `uq_project_initialization_operation_active` | 这个 Project 的**初始化操作**归谁 | 操作表 active 行 |

两者必须在 **Import 创建 / 终止的同一事务内成对建立 / 释放**（R-3-3）。

### 4.2 Attempt 状态机

```text
[*] → running → succeeded
              → failed
              → cancelled
```

| 规则 | 内容 |
|---|---|
| 轮次编号 | `attempt_no` 按 `(import_id, repository_id)` 单调递增，从 1 开始 |
| 事实留档 | 成功与失败都写 `facts` / `error_code` / `error_detail`（R-4-1：成功材料与失败现场都按轮留档） |
| 时间约束 | `finished_at IS NULL OR finished_at >= started_at`（CHECK） |
| 恢复 | 以 **lease / Attempt 过期**为准（N-5），不依赖 attempts 退避 |
| 基准一致性 | 采集按 Source 的 **固定 `head_commit_sha`** 读取（N-1），不随 HEAD 漂移 |

### 4.3 初始化操作状态机

```text
[*] → active → released      （被更高 generation 取代）
            → completed       （初始化完成）
            → cancelled       （显式取消）
```

| 规则 | 内容 |
|---|---|
| 占有 | 插入 active 行是 Import 创建事务的**提交前置条件**；`rows_affected = 0` 即整体回滚（R-3-1） |
| 释放 | 终态化时 `UPDATE ... SET status='released' WHERE status='active'`；同样检查 `rows_affected` |
| generation | 每次新建递增；权威落**操作表 + Project 行锁**，**不在 `core.project` 加列**（R-3-4） |
| 迟到释放 | 行锁 + 条件 UPDATE 保证「迟到的释放不会覆盖新操作」 |

### 4.4 Evidence 类型判别器

| `evidence_type` | 含义 | Commit 链路列 |
|---|---|---|
| `code` | 代码位置证据（file / symbol / code_range） | `import_id` / `repository_id` / `commit_sha` / `attempt_no` **全部非空** |
| `test_result` | 测试结果证据 | 同上，全部非空 |
| `commit` | 提交证据 | 同上，全部非空 |
| `user_statement` | 人类口述 | **全部为空** |
| `system_observation` | 系统观测（如环境、配置） | **全部为空** |

- 由 **CHECK 约束**保证（R-6-2），不靠应用层自觉。
- **非代码来源不伪造 Commit**（`user_statement` 不许带 `commit_sha`）。
- Evidence **不设 `source_attempt_id` UUID 普通外键**：改用 `attempt_no` 参与组合 FK（补-2），使「Evidence 引用的 Attempt 属于其 Source」成为键结构事实。

### 4.5 Discovery Session 与 Task 状态机的边界

两套状态机**并行且不交叉**：

| 状态机 | 拥有者 | 谁能推进 |
|---|---|---|
| Task 状态机（13 态） | `pm-domain` | **仅**平台领域服务（`apply_task_transition`） |
| Discovery Session 状态机（5 态） | `discovery/session.rs` | Discovery 领域服务（`transition_session`） |

**边界规则（I6-B-05 收敛项）**：

1. Discovery **不得**推进 Task 的权威状态；
2. Task 的状态变化**不得**隐式推进 Discovery Session；
3. 两者之间**只允许通过事件解耦**（不新建事件总线，见 §1.3）。

---

## 5. 权限与归属

### 5.1 Project 作用域约束

| 关系 | 强制方式 |
|---|---|
| Finding ↔ Session 同 Project | 组合 FK `(tenant_id, project_id, session_id)` |
| Evidence ↔ Finding 同 Project | 组合 FK `(tenant_id, project_id, finding_id)` |
| Evidence ↔ Source 同 Project | 组合 FK `(tenant_id, project_id, import_id, repository_id)` |
| Evidence ↔ Attempt 同 Project 且同 Source | 组合 FK 五列 |
| finding_repository ↔ Repository 同 Project | 组合 FK `(tenant_id, project_id, repository_id)` + RESTRICT |

### 5.2 谁能做什么

| 操作 | 授权主体 |
|---|---|
| 创建 Import（`start_import`） | Project 成员（真人） |
| 重试 / 取消 Import | Project 成员（真人）；`proposed` 后的终止须人工确认 |
| 提交 Finding | 项目成员（真人）或经授权的 Agent |
| 确认 / 拒绝 Finding | **可信真人**（`core.principal.kind = 'human'`）——见 I2 R2-B 铁律 |
| 创建 Evidence | 与其 Finding 的创建者同级；`user_statement` 仅真人可建 |
| 删除仓库 | Project 成员；**有历史引用即 409 `repository_has_history`** |

### 5.3 fail-fast 校验（N-2）

`start_import` 创建时**必须先校验**：

1. `ai.project_ai_config` 存在且可用；
2. Project 未冻结、未归档；
3. 该 Project 未曾 `applied` 过（C-1：一次性 bootstrap）。

冻结 / 归档时，必须**终态化在飞 Import**（把 `queued/cloning/reading/analyzing/draft_ready` 转为 `cancelled` 并释放 active 名额）。

---

## 6. 事务协议与锁序

### 6.1 锁序（全局唯一）

```text
core.project (FOR UPDATE)  →  integration.repo_import (FOR UPDATE)  →  integration.repo_import_source (FOR UPDATE)  →  Attempt 行
```

与 I1 的 `project → run → task` **同向**，不构成环；S01-W13 的 `tenant → project → member` 也在其上游，合并后仍为 `tenant → project → import → source → attempt` 的单向链。

### 6.2 事务边界

| 操作 | 事务内容 |
|---|---|
| `start_import` | 锁 project → 占有初始化操作（检查 `rows_affected`）→ 插入 Import（active 名额由唯一索引保证）→ 插入 Source 行（每仓一行）→ 插入 Attempt 行（running）→ 审计 |
| Attempt 采集完成 | 锁 import → 更新 Attempt（succeeded + facts）→ 更新 Source（`head_commit_sha` write-once + `current_attempt_no`）→ 审计 |
| 生成草案 | 插 `repo_import_draft_version(generating)` → 生成 → 转 `succeeded` + 推进 `draft_revision` 指针 → Import 转 `draft_ready` → 审计（同事务） |
| 终态化 | 锁 import → 更新 Import 终态 → 释放初始化操作（检查 `rows_affected`）→ 审计 |

### 6.3 幂等

- `start_import` 透传客户端幂等键，复用 `integration.inbound_event` 的 `UNIQUE(project_id, source, idempotency_key)`（N-4），**不新建幂等台账**。
- 重复请求返回同一 Import，不新建。

---

## 7. 接口契约

### 7.1 REST

> **实施状态（2026-11-03）**：本节所列契约**已实施**，验收见
> `apps/pm-api/tests/import_contract.rs`（7 例全绿）。

| 方法 | 路径 | 变更 | 实施结果 |
|---|---|---|---|
| POST | `/v1/pm/projects/{project_id}/imports` | **请求体新增必填 `repository_ids: string[]`**（非空数组；N-3）；透传 `idempotency_key` | ✅ 空/缺省 → 422 `repository_ids_required`；跨项目仓库 → 422 `repository_not_in_project`；同幂等键 → 同一 `import_id` + `replayed=true` |
| GET | `/v1/pm/projects/{project_id}/imports/{import_id}` | 响应新增 `draft_revision`（指针语义）、`sources[].attempts[]`（每轮摘要） |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/cancel` | 增加 `reason`（可选，写入审计 `after_data`）；`proposed` 态也允许（显式终止，C-2） | ✅ 已实施 |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/apply` | 不变 | — |
| POST | `/v1/pm/projects/{project_id}/imports/{import_id}/terminate` | **新增**：`proposed` 态的显式终止 / 放弃（C-2） | ✅ 已实施；终止后 active 名额释放，可再次发起导入 |

> **导入集合固化**：导入创建时只为 `repository_ids` 里的仓库建 Source 行与首轮 Attempt，
> Worker 只处理这批仓库（I6-B-03）。`GET .../imports/{id}` 的 `sources[]` 追加
> `attempts[]`（每轮摘要：轮次号 / 状态 / Commit / 错误码 / 起止时间）。

> **契约直接改**：`repository_ids` 从「可选」改为「必填且非空」，REST 与 MCP **同契约**；开发期无兼容包袱（N-3）。

### 7.2 MCP

| 工具 | 变更 |
|---|---|
| `pm_discovery_submit_findings` | `findings[].evidence[]` 的内嵌 JSONB 数组改为**结构化证据项**（`evidence_type` + 可选 `file_path`/`symbol`/`code_range`/`commit_sha`/`attempt_no`），服务端落 Evidence 表 |
| 新增 `pm_submit_evidence` | 提交证据到既有 Finding（绑定 `finding_id` + `attempt_no`）——✅ 已实施（19 个 MCP 工具） |
| 权限码 | `pm_discovery_submit_findings` / `pm_submit_evidence` / `pm_claim_task` / `pm_submit_result` / `pm_report_progress` 补独立权限码（现状只受 `PM_MCP_READ` 约束——缺陷 D-11） |

> **权限码收紧是跨仓协同项，本轮未落地**（2026-11-03 裁决）：lmms 侧当前只签发
> `PM_MCP_READ` / `PM_MCP_DOCUMENT_WRITE` / `PM_MCP_TASK_UPDATE` 三个码
> （`libs/authz/src/perms.rs`），**没有**发现族专用码。单仓擅自收紧会让**现网执行端
> 的 key 立刻 403**，直接打断 S01-4 的执行闭环。正确顺序是：
> ① lmms 侧新增发现族权限码并签发 → ② ai-pm 侧按工具收紧权限表 → ③ 回归执行链路。
> 本轮新增的 `pm_submit_evidence` 为**新工具**（无现网流量），暂挂 `PM_MCP_READ`
> + 真人成员门，与现有发现族写工具同口径；待 ①② 完成后统一收紧。

---

## 8. 审计与幂等

| 动作 | 审计表 | action |
|---|---|---|
| 创建 Import | `governance.audit_log`（项目域） | `repo_import.started` |
| 占有 / 释放初始化操作 | `governance.audit_log` | `project_initialization.acquired` / `.released` |
| Attempt 成功 / 失败 | `governance.audit_log` | `repo_import.attempt_succeeded` / `.attempt_failed` |
| 草案生成成功 / 失败 | `governance.audit_log` | `repo_import.draft_succeeded` / `.draft_failed`（含 revision 与消费的 `source_attempts`） |
| Import 终态化 | `governance.audit_log` | `repo_import.terminated` |
| 仓库删除被拒 | `governance.audit_log` | `repository.delete_rejected`（reason=`repository_has_history`） |

> **实施状态（2026-11-03）**：`repo_import.started`、`project_initialization.acquired`、
> `repo_import.attempt_succeeded` / `.attempt_failed`、`repo_import.draft_succeeded` /
> `.draft_failed`、`repo_import.terminated`（`after_data` 带 `previous_status` /
> `reason` / `initialization_released`）**已实施**；`repository.delete_rejected`
> 随 409 映射一并落地。

审计四列（`actor_id` / `actor_identity_id` / `actor_client` / `actor_auth_mode`）+ `claimed_device_id` 与既有 13 处写入点同口径。

**幂等键规则**：Import 创建透传 `idempotency_key`（N-4）；Attempt 与草案版本由服务端生成的 `(import_id, repository_id, attempt_no)` 与 `(import_id, revision)` 天然幂等。

---

## 9. 失败、取消与恢复

| 场景 | 处理 |
|---|---|
| Worker 崩溃（job 级） | job 级失败路径一律 `Ok(())`，**不依赖 attempts 退避**；恢复以 **lease 过期重认领**为准（N-5） |
| Import 卡在 `cloning` / `analyzing` | 租约过期 → 重新认领；已存在的 Attempt 标记 `failed`（留档），新开一轮 `attempt_no` |
| 用户取消 | Import → `cancelled`；在飞 Attempt → `cancelled`；释放 active 名额与初始化操作 |
| Project 冻结 / 归档 | 终态化在飞 Import（N-2） |
| 草案生成失败 | `repo_import_draft_version` 转 `failed`；**旧 succeeded 版本与 `draft_revision` 指针不动** |
| 仓库删除被拒 | RESTRICT → 应用层映射 **409 `repository_has_history`** |

---

## 10. 缺陷映射（I6-A 报告 D-01 ~ D-22 → 本稿任务）

| 缺陷 | 摘要 | 处置 | 任务 |
|---|---|---|---|
| D-01 | Project 终态泄漏（5 路径） | 归档流程属 I1-D；本稿只保证「终态化在飞 Import」（N-2） | I6-B-01 |
| D-03 | 冻结时在飞 Import 无终态化 | fail-fast + 终态化 | I6-B-01 |
| D-05 | Import 名额索引未租户化 | 部分唯一索引重建 | I6-B-01 |
| D-06 | `proposed` 无显式终止 | 新增终止端点 + 状态迁移 | I6-B-01 |
| D-07 | 草案覆盖写 | 草案版本表 | I6-B-02 |
| D-08 | 仓库删除 CASCADE | RESTRICT + 409 | I6-B-08 |
| D-09 | finding 可挂跨项目仓库 | finding_repository 升格 | I6-B-08 |
| D-10 | Finding 无 project_id | 加列 + 换轨 FK | I6-B-08 |
| D-11 | Evidence 无实体 | 新表 | I6-B-04 |
| D-12 | Evidence 无法绑 Attempt | `attempt_no` 组合 FK | I6-B-04 |
| D-13 | 事实/计数被覆盖 | 迁 Attempt | I6-B-02 |
| D-14 | 基准随 HEAD 漂移 | write-once SHA | I6-B-03 |
| D-15 | Source 集合未固化 | 集合固化 + 唯一键 | I6-B-03 |
| D-16 | 幂等台账缺失 | 透传 inbound_event 幂等键 | I6-B-06 |
| D-17 | 审计缺 Attempt 层 | 补审计动作 | I6-B-06 |
| D-18 | 恢复依赖 attempts 退避 | lease 过期重认领 | I6-B-07 |
| D-19 | 并发初始化无互斥 | active 部分唯一索引 + 行锁 | I6-B-01 |
| D-20 | 迟到释放覆盖新操作 | 条件 UPDATE + `rows_affected` 检查 | I6-B-01 |
| D-21 | 跨 Project 关联无 DB 级约束 | 全链组合 FK | I6-B-08 |
| D-22 | 证据类型无判别 | CHECK 判别器 | I6-B-04 |

> D-02 / D-04 已在 I6-A 报告勘误为 P1 / 不适用；本稿不重复处置。

---

## 11. 验收

### 11.1 I6-B-036 ~ I6-B-051（本稿自定，16 条）

> 编号段已被本稿占用。内容依据合并规格 §4 的负向测试映射、边界评审 §4 的修订清单与本稿 §3~§9 的规则展开。

| 编号 | 验收项目 | 验收标准 | 类型 |
|---|---|---|---|
| I6-B-036 | 初始化操作占有 | `start_import` 事务内插入 active 操作行；`rows_affected=0` 即整体回滚 | 并发 / 数据 |
| I6-B-037 | 初始化操作释放 | 终态化时条件 UPDATE 释放 active 行；迟到释放不覆盖新操作 | 并发 |
| I6-B-038 | active 唯一性 | 同 Project 至多一个 active 初始化操作（DB 违例 `23505`） | 数据 |
| I6-B-039 | Import 名额 | 同 Project 至多一个 active Import（DB 违例 `23505`） | 数据 |
| I6-B-040 | Import 一次性 | `applied` 后再创建 → **409 `project_not_empty`** | 负向 |
| I6-B-041 | 显式终止 | `proposed` 态可终止，终态化并释放名额 | 正向 |
| I6-B-042 | 草案不覆盖 | 两次生成产生两个 revision；旧版本内容不变 | 数据 |
| I6-B-043 | 草案指针 | 生成失败时 `draft_revision` 指针不变，旧 succeeded 版本仍在 | 负向 |
| I6-B-044 | 草案溯源 | 草案行的 `source_attempts` 记录本版消费的每 Source 最新成功轮 | 数据 |
| I6-B-045 | Attempt 留档 | 成功轮与失败轮各有独立行，`facts` / `error_code` 各自留档 | 数据 |
| I6-B-046 | 基准 write-once | `head_commit_sha` 写后不可改（第二次写被拒或被忽略） | 数据 |
| I6-B-047 | Source 集合固化 | Source 行唯一键 `(tenant_id, project_id, import_id, repository_id)` 生效 | 数据 |
| I6-B-048 | Evidence 判别器 | 代码类证据缺 4 列 → CHECK 违例 `23514`；非代码类带 Commit → CHECK 违例 | 数据 |
| I6-B-049 | Evidence 绑 Attempt | Evidence 引用不属于自己的 Attempt → FK 违例 `23503` | 数据 |
| I6-B-050 | 仓库删除保护 | 被引用的仓库删除 → RESTRICT 违例；应用层映射 409 | 负向 / 数据 |
| I6-B-051 | 审计与幂等 | 每个动作有审计行（带四列）；重复 `start_import` 不产生第二个 Import | 审计 / 幂等 |

### 11.2 DB 级负向测试（7 条，直接打库）

| # | 场景 | 期望约束违例 |
|---|---|---|
| 1 | 同 Tenant 不同 Project 的 Finding 与 Source 建 Evidence | `fk_evidence_finding_scope` / `fk_evidence_source_scope` → `23503` |
| 2 | 同 Project 不同 Import 的 Source 被错误 Attempt 引用 | `fk_evidence_attempt_scope` → `23503` |
| 3 | Evidence 引用的 Attempt 不属于其 Source | 同上（含 `attempt_no` 与四列同键锚定）→ `23503` |
| 4 | Source.project_id 与 Import 不一致时创建/更新 | `fk_source_tenant_project_import` → `23503` |
| 5 | Finding.project_id 与 Session 不一致时创建/更新 | 补-1 的 session 三列唯一键 + 换轨 FK → `23503` |
| 6 | 同租户跨项目仓库挂 Finding（D-09 复发） | 补-3 的 finding_repository 换轨 FK → `23503` |
| 7 | 代码类证据缺 import / commit / attempt_no | R-6-2 CHECK → `23514` |

**执行方式**：直接对数据库执行 `INSERT` / `UPDATE` 断言约束违例，**不经 REST/MCP**。

### 11.3 冻结门槛

| # | 条件（合并规格 §6 + 总体计划 §6.5） | 状态 |
|---|---|---|
| 1 | C-1~C-4、N-1~N-5 进正文 | ✅ §0.3 |
| 2 | R-3-1~R-3-4、R-4-1、R-6-1、R-6-2 进正文 | ✅ §0.3 |
| 3 | D-1a / D-2a 落定 | ✅ §0.3 |
| 4 | Attempt / Evidence 复合外键进入 DDL | ✅ §3.2 第 4/6 项 |
| 5 | `repo_import_source` 补复合唯一键 | ✅ §3.2 第 3 项 |
| 6 | 跨 Project 关联约束采用 DB 级方案 | ✅ §3.2 / §5.1 |
| 7 | 代码 / 非代码证据 CHECK 明确 | ✅ §3.2 第 6 项、§4.4 |
| 8 | 历史数据兼容性检查通过 | ✅ **按 AGENTS.md 规则 1 改为「空库初始化 + 结构断言」**（§0.4） |
| 9 | 旧环境升级通过（0030 → 0031） | ⬜ I6-B-09 实测 |
| 10 | 新环境初始化通过（空库 → 0031） | ⬜ I6-B-09 实测 |
| 11 | 并发、幂等、恢复及负向约束测试通过 | ⬜ I6-B-09 实测 |
| 12 | 0029（D4）发布顺序依赖方向确认 | ⬜ 实施时确认（`repo_import_source` 的 Rust 读写点 3 处） |

---

## 12. 任务映射

| 任务 | 范围 | 本稿章节 | 冻结要求 |
|---|---|---|---|
| I6-B-01 | Project 初始化操作、占有与释放、generation、active 唯一性、取消及终态恢复 | §3.2-1、§4.1、§4.3、§5.3 | DDL 审查 |
| I6-B-02 | Source / Attempt 数据归属与重试语义 | §3.2-3/4、§4.2 | DDL 审查 |
| I6-B-03 | Source 集合固化和 Commit 基准 | §4.2（N-1）、§11.1-046/047 | DDL 审查 |
| I6-B-04 | Finding / Evidence 模型及证据类型判别 | §3.2-6、§4.4 | DDL 审查 |
| I6-B-05 | Discovery 权限、提案及接口收敛 | §4.5、§7 | 接口审查 |
| I6-B-06 | 审计和幂等 | §6.3、§8 | 测试审查 |
| I6-B-07 | 历史数据和兼容性 | §0.4、§9 | 数据审查 |
| I6-B-08 | 复合外键、跨 Project 约束和 Migration 集成 | §3、§5 | 最终 DDL 审查 |
| I6-B-09 | 空库初始化 + 0030 升级验证 + DB 级负向测试 | §11.2 | DDL 冻结后实测 |

---

## 13. 审查清单

### 13.1 合并规格 §7 修订清单的落地对照

| 合并规格 §7 要求 | 本稿落地 |
|---|---|
| §2 索引 SQL 更新为扩展版四列键 | §3.2-3（`UNIQUE (tenant_id, project_id, import_id, repository_id)`） |
| §3 补 R-3-2 / R-3-4 / R-3-3 | §4.1、§4.3 |
| §4 字段归属表替换（R-4-1） | §3.2-3/4、§4.2 |
| §4 新增小节 D-1a | §3.2-2、§4.1 |
| §5 补「proposed 起禁止重试」 | §4.1 |
| §6 Evidence 字段表（删 `source_attempt_id`，加 `attempt_no`）+ CHECK 判别器 | §3.2-6、§4.4 |
| §8 补 N-2 fail-fast / N-5 lease 恢复 | §5.3、§9 |
| §9 补 N-3 / N-4 | §7.1、§6.3 |
| §10 审计动作表补 Attempt 层 | §8 |
| §12 预计对象表更新 | §3.1（14 项） |
| §13 Import 状态机 draft_ready / proposed 行修订 | §4.1 |
| §15 缺陷映射表更新 | §10 |
| §16/§18 任务范围与验收 | §12、§11 |

### 13.2 待审问题

| # | 问题 | 建议 |
|---|---|---|
| Q1 | `discovery_question.project_id` / `discovery_proposal.project_id` 是否需要独立列 | 需要——否则其 FK 无法携带 project（§3.2-8） |
| Q2 | `repo_import.source_attempts` 用 JSONB 还是独立关联表 | JSONB（合并规格已定）：草案是**快照**，其 Attempt 集合不应随后续轮次变化 |
| Q3 | `repo_import.status` 是否需要新增 `terminated` 终态 | 否——沿用 `cancelled`，用 `reason` 区分（C-2） |
| Q4 | `core.project_initialization_operation.import_id` 是否必填 | 否——占有发生在 Import 创建**之前**，两者同事务建立 |
| Q5 | Evidence 是否允许引用**失败轮** Attempt | 允许（失败现场也是证据），但 CHECK 判别器要求 `commit_sha` 非空——失败轮若无 SHA 则只能用 `system_observation` |

### 13.3 本稿未覆盖、需另立任务的事项

- **I2-D 的 REST/MCP 双入口 P0 修复**（Finding 创建入口的 Project 归属校验）——属 I2。
- **Discovery 分析作业**（Worker 侧自动产生 Finding）——属 I6-B-02 的后续扩展。
- **Project 归档与在飞 Import 的联动**——属 I1-D。
- **仓库级 Git 能力授权**——属 I3-B。

---

**文档状态：V1.1 正文成稿，待 G-DDL 逐条审查后冻结 Migration `0031`。**
