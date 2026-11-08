# S01 · I6-B V1.1 修订合并规格（Migration 冻结前最终输入）

> 文档性质：裁决合并 + DDL 级核对规格
> 输入：专家最终裁决（D-1a / D-2a 修正版 / R-3-1～R-6-2 全收 / 冻结条件 5+2）
> 代码基线：`master aaee6e5`，migrations 0001～0028（已核对全部既有唯一键与 FK）
> 版本：V1.0
> 状态：供 I6-B V1.1 正文改写；改写完成后按 §6 冻结门槛做完整 DDL 审查

---

## §0. 裁决收编状态

| 项 | 状态 |
|---|---|
| C-1～C-4、N-1～N-5（首轮裁决 12 条） | ✅ 已定，进正文 |
| R-3-1～R-3-4、R-4-1、R-6-2 | ✅ 全部接受，进正文 |
| R-6-1 | ✅ 接受并**扩展**为 Project 作用域组合键（本规格 §2） |
| D-1 | ✅ D-1a（含草案-Attempt 双历史规则，本规格 §1 落地） |
| D-2 | ✅ D-2a 数据库级（专家已指出三列键不足，本规格补齐链路缺口） |
| 待裁决项 | **无**。剩余工作=正文改写 + DDL 审查 |

**本规格在专家示意 DDL 基础上核对出三处必须补的缺口与一处简化**（§2.2），均已对照 0008/0009/0019/0025 实际约束核实。

---

## §1. D-1a 落地：草案版本与 Attempt 集合记录

### 1.1 现状与裁决的冲突点

裁决规则 5「旧草案保留，不覆盖历史版本」与现状**直接冲突**：草案是 `repo_import` 上的单列 `draft JSONB`，每次重生成被覆盖写（`lifecycle.rs:448-457`，`draft_revision + 1` 但旧值不存）。单列模型也无法记录"本版草案使用了哪轮 Attempt"。

### 1.2 落地规格（新增表，替换单列权威）

```text
integration.repo_import_draft_version
```

| 字段 | 说明 |
|---|---|
| `tenant_id, import_id, revision` | 组合主键 `(tenant_id, import_id, revision)`；FK `(tenant_id, import_id)` → `repo_import(tenant_id, id)` CASCADE |
| `draft JSONB` | 该版草案全文 |
| `source_attempts JSONB` | **Attempt 集合映射**：`[{repository_id, attempt_no}]`，本版草案消费的每 Source 最新成功轮 |
| `dropped_items INT` | 随版本走 |
| `generation_status` | `generating / succeeded / failed` CHECK |
| `generated_at / created_at` | |

配套规则：

1. `repo_import.draft` 单列**停用**（保留列做兼容读或直接删列，开发期建议删），`draft_revision` 保留为**指针**=最新 `succeeded` 版本号。
2. 生成流程：先插 `generating` 行 → 成功转 `succeeded` + 推进 `repo_import.draft_revision` 指针；失败转 `failed`，**旧 succeeded 版本与指针不动**（裁决规则 6"不得先删除旧版本"由指针语义保证）。
3. 重生成触发=任一 Source 新成功 Attempt（裁决 §1 规则 3/4）；`proposed` 起禁止重试（规则 7），故 `proposed` 后不会再有新版本。
4. `generation_status='succeeded'` 上可加部分唯一索引 `(tenant_id, import_id)` WHERE 语义允许多 succeeded（每 revision 一行，无需额外索引）。

草案-Attempt 双历史示例（裁决 §1 表格）在该模型下每格可查：`revision 1 → source_attempts=[{A,1},{B,1}]`（B 的失败轮在 Attempt 表留档）。

---

## §2. D-2a 修订：完整 Project 作用域约束链

### 2.1 专家示意 DDL 的核对结论

| 示意内容 | 核对结果 |
|---|---|
| `repo_import` 加 `UNIQUE (tenant_id, project_id, id)` | ✅ 可行（project_id 列已有，0009:29） |
| `repo_import_source` 加 `project_id` + 回填 + FK | ✅ 可行；**注意**：与既有 `fk_ris_tenant_import`（0019:381，`(tenant_id, import_id)`）冗余，应 DROP 旧 FK 换轨，不要并存两条到同表的 FK |
| source 三列 + 四列唯一键并存 | ⚠️ **建议简化**：四列键 `(tenant_id, project_id, import_id, repository_id)` 已覆盖全部新引用方（Attempt/Evidence 都带 project 作用域）；三列键没有引用方，**可不建**（若专家坚持保留作未来无 project 引用锚点亦无害，二选一，默认省略） |
| `discovery_finding` 加 `project_id` + 三列唯一键 | ✅ 可行（回填自 session，dev 期数据可重放） |
| Evidence 两条作用域 FK | ✅ 方向正确，删除策略见 §3 |

### 2.2 三处必须补的缺口

**补-1 · `discovery_session` 缺三列唯一键 → 负向测试 5 无法 DB 级执行。**
裁决负向测试 5 要求"Finding 的 project_id 与 Session 不一致时，不能创建或更新"。但示意 DDL 只给了 finding 三列唯一键，finding→session 仍是既有 `(tenant_id, session_id)` FK（0025:61）——**该 FK 不携带 project_id，数据库无法发现 finding.project_id 与 session.project_id 不一致**。必须补：

```sql
ALTER TABLE discovery.discovery_session
    ADD CONSTRAINT uq_discovery_session_tenant_project_id
    UNIQUE (tenant_id, project_id, id);
-- finding 的 session FK 换轨为携带 project 的版本（DROP 旧 FK (tenant_id, session_id)）
ALTER TABLE discovery.discovery_finding
    ADD CONSTRAINT fk_finding_tenant_project_session
    FOREIGN KEY (tenant_id, project_id, session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id) ON DELETE CASCADE;
```

**补-2 · Evidence→Attempt 用组合键，不用 `source_attempt_id` UUID 普通外键。**
裁决 §2.3 明确"不能只验证 Attempt UUID 存在"。最干净的实现：**Evidence 不设 `source_attempt_id` 列**，改设 `attempt_no INT`，参与组合 FK——Attempt 的身份本就是 `(tenant_id, project_id, import_id, repository_id, attempt_no)`，Evidence 已携带前四列，加 `attempt_no` 即完成锚定：

```sql
-- Attempt 表（归属沿裁决 §2.3）
CREATE TABLE integration.repo_import_source_attempt (
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    import_id UUID NOT NULL,
    repository_id UUID NOT NULL,
    attempt_no INT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running','succeeded','failed','cancelled')),
    commit_sha TEXT,
    facts JSONB, commit_count INT, doc_count INT, size_bytes BIGINT, truncated BOOLEAN,
    error_code TEXT, error_detail TEXT, job_id UUID,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(), finished_at TIMESTAMPTZ,
    PRIMARY KEY (tenant_id, import_id, repository_id, attempt_no),
    UNIQUE (tenant_id, project_id, import_id, repository_id, attempt_no),
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id)
        REFERENCES integration.repo_import_source (tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE,
    CHECK (finished_at IS NULL OR finished_at >= started_at)
);

-- Evidence 侧：attempt_no 列 + 组合 FK（替代 UUID 引用）
ALTER TABLE discovery.discovery_finding_evidence
    ADD CONSTRAINT fk_evidence_attempt_scope
    FOREIGN KEY (tenant_id, project_id, import_id, repository_id, attempt_no)
    REFERENCES integration.repo_import_source_attempt
        (tenant_id, project_id, import_id, repository_id, attempt_no);
```

`attempt_no` 仅 code 类证据非空（并入 R-6-2 CHECK 判别器）。附带收益：Evidence→Source 与 Evidence→Attempt 两条 FK 的列集前缀一致，**Attempt 归属 Source 由键结构直接保证**，负向测试 2/3 全部转化为 FK 违例。

**补-3 · `discovery_finding_repository`（既有 M2M）同批升格，否则 D-09 在 Finding 直连路径继续存在。**
该表现有 repository FK 只锚 `(tenant_id, repository_id)`（0025:74-75）——同租户跨项目仓库仍可挂到 Finding（I6-A D-09）。既然 finding 已加 `project_id`，同批补：

```sql
ALTER TABLE integration.project_repository
    ADD CONSTRAINT uq_project_repository_tenant_project_id
    UNIQUE (tenant_id, project_id, id);   -- 已有 (project_id, id)（0008:31），补租户版作 FK 锚点

ALTER TABLE discovery.discovery_finding_repository
    ADD COLUMN project_id UUID NOT NULL;  -- 回填自 finding
-- repository FK 换轨（DROP 旧）：
--   (tenant_id, project_id, repository_id) → project_repository(tenant_id, project_id, id) ON DELETE RESTRICT
-- finding FK 换轨（DROP 旧）：
--   (tenant_id, project_id, finding_id) → discovery_finding(tenant_id, project_id, id) ON DELETE CASCADE
```

升格后 C-4 的 RESTRICT 同时覆盖两条引用路径（source 与 finding_repository），仓库历史保护闭环。

### 2.3 全链归属图（DB 级保证）

```text
core.project (tenant_id, id)
   ▲                          ▲
   │ (tenant,project,id) 唯一键锚定（新增）
   │                          │
repo_import ──(t,p,imp,repo)──▶ repo_import_source ──▶ repo_import_source_attempt
   ▲        四列唯一键（新增）        │                        │
   │                                 │ (t,p,imp,repo,attempt_no) 组合 FK
   │                                 ▼                        │
   │                     discovery.discovery_finding_evidence ◀┘
   │                                 ▲
   │ (t,p,session) 换轨 FK            │ (t,p,finding) 组合 FK
discovery_session ──────────▶ discovery_finding ──(t,p,repo)──▶ project_repository
```

**每条边都携带 project_id → "Evidence→Attempt→Source→Import→Project" 与 "Finding→Session→Project" 两链的同 Project 归属均由数据库保证**，无 UUID 存在性漂移洞。

---

## §3. FK 删除策略定案（专家示意留空项）

| FK | ON DELETE | 理由 |
|---|---|---|
| source → import（换轨后新 FK） | CASCADE | 沿既有语义（0019:381），Import 随项目消亡 |
| source → project_repository | **RESTRICT** | C-4 裁决：历史引用阻止仓库物理删除（现为 CASCADE，D-08） |
| attempt → source | CASCADE | Attempt 随 Source 生命周期 |
| evidence → finding | CASCADE | Evidence 随 Finding 消亡（沿 0025 模式） |
| evidence → source（四列 FK） | CASCADE | Source 仅随 Import/Project 级联消亡，该场景下同项目 Finding/Evidence 亦随之级联（session 链），无历史孤悬 |
| evidence → attempt（组合 FK） | CASCADE | 同上 |
| finding → session（换轨后） | CASCADE | 沿既有（0025:61） |
| finding_repository → repository（换轨后） | **RESTRICT** | C-4 同口径 |
| project_repository → project | CASCADE | 沿既有（0019:292-294） |

配套：仓库 DELETE 被任一 RESTRICT 拒绝时，应用层映射 409 `repository_has_history`（C-4/裁决 §4.3，含 `delivery.task_repository` 的 NO ACTION 500 修复——该 FK 保持 NO ACTION 即可，仅补错误映射）。

---

## §4. 负向测试 → 约束映射（进 I6-B-09）

| # | 裁决要求负向测试 | 由哪条约束保证 |
|---|---|---|
| 1 | 同 Tenant 不同 Project 的 Finding 与 Source 建 Evidence | `fk_evidence_finding_scope` + `fk_evidence_source_scope`（均含 project_id） |
| 2 | 同 Project 不同 Import 的 Source 被错误 Attempt 引用 | `fk_evidence_attempt_scope` 组合键含 import_id |
| 3 | Evidence 引用的 Attempt 不属于其 Source | 同上（attempt_no 与 source 四列同键锚定） |
| 4 | Source.project_id 与 Import 不一致时创建/更新 | `fk_source_tenant_project_import` |
| 5 | Finding.project_id 与 Session 不一致时创建/更新 | **补-1** 的 session 三列唯一键 + 换轨 FK |
| 追加 | 同租户跨项目仓库挂 Finding（D-09 复发） | **补-3** 的 finding_repository 换轨 FK |
| 追加 | code 类证据缺 import/commit/attempt_no | R-6-2 CHECK 判别器 |

执行方式按裁决：直接对数据库执行 INSERT/UPDATE 断言约束违例，不经 REST/MCP。

---

## §5. R-3-3 补充确认（裁决 §3 提醒项）

操作表 active 部分唯一索引（初始化归属）与 `repo_import_one_active_per_project`（Import 名额）**语义不同、并存**：前者答"谁拥有 initializing"，后者答"Import 工作流名额"。两者在 Import 创建/终止的**同一事务**内成对建立/释放（创建=INSERT operation(active) + INSERT import(active)；终态化=UPDATE operation 终态 + UPDATE import 终态 + 条件释放 project）。已写入 V1.1 修订清单，防实现时只动其一。

---

## §6. 冻结门槛核对表

| 条件（裁决 §4，5+2 项） | 当前状态 |
|---|---|
| C-1～C-4、N-1～N-5 进正文 | ⬜ 待 V1.1 改写 |
| R-3-1～R-6-2 进正文 | ⬜ 待 V1.1 改写 |
| D-1/D-2 落定 | ✅ 本规格 §1/§2 |
| R-6-1（扩展版）作为组合 FK 前置 | ✅ §2 已并入 |
| D-2a 全链 DB 级验证 | ✅ 链路闭合（含补-1/2/3），⬜ 待 DDL 审查复核 |
| 空库初始化 + 0028 升级验证 | ⬜ I6-B-09 实测 |
| 负向测试 1～5（DB 级） | ✅ 映射就绪（§4），⬜ 随 I6-B-09 落地 |

**下一步**：按 §7 清单把 V1.0 正文改写为 V1.1 → 完整 Migration DDL 审查（逐条对照本规格 §2/§3/§4）→ 冻结。

---

## §7. V1.1 正文修订清单（按 V1.0 章节）

| V1.0 章节 | 修订 |
|---|---|
| §2 | 索引 SQL 更新为扩展版四列键方案；补 R-3-1 行数检查进 §2.4 事务协议 |
| §3 | 补 R-3-2（删 bootstrap initializing 变体）、R-3-4（操作表权威+行锁，删"是否加 project 列"悬念）、R-3-3 双索引并存与同事务规则（本规格 §5） |
| §4 | 字段归属表替换 §4.8（R-4-1：facts/计数/错误迁 Attempt）；§4.9 Attempt 表定稿为本规格 §2.2 补-2 DDL（含 project_id）；§4.11 汇总计数走聚合 |
| §4（新增小节） | D-1a：草案版本表（本规格 §1）、重生成触发规则、双历史示例表 |
| §5 | §5.4 补"proposed 起禁止重试"；重试成功→重生成草案（draft_revision 指针语义） |
| §6 | §6.4 Evidence 字段表更新：删 `source_attempt_id`，加 `attempt_no`；补 CHECK 判别器终稿（R-6-2 + attempt_no）；§6.8 不变 |
| §6（新增小节） | D-2a 完整约束链（本规格 §2 全部 SQL，标注"示意→实施时核对"已由本规格完成）；finding_repository 升格（补-3） |
| §8 | N-2 fail-fast/冻结终态化（裁决 §6）；N-5 lease 恢复协议（裁决 §9） |
| §9 | N-3（repository_ids 必填/空数组拒绝/MCP 同契约）；N-4（幂等透传） |
| §10 | 审计动作表补 Source Attempt 层；幂等键规则按裁决 §8 |
| §12 | 预计对象表更新：+`repo_import_draft_version`、+`repo_import_source_attempt`、`repo_import_source` 加列+换轨 FK、`discovery_finding`/`discovery_finding_repository`/`discovery_session`/`project_repository` 加唯一键/换轨 FK、删除策略表（本规格 §3） |
| §13 | Import 状态机：draft_ready 行补"允许 Source 重试 + 重生成草案"；proposed 行补"禁止重试，仅显式终止" |
| §15 | 缺陷映射表更新（D-03 P1 勘误版、D-08/D-09 由 RESTRICT+补-3 闭环） |
| §16/§18 | 任务范围按裁决 §10 新编号；验收并入 I6-B-036～051 + 本规格 §4 负向测试 |

---

**文档状态：V1.0——I6-B V1.1 改写的唯一输入；改写后按 §6 门槛做 DDL 审查，随后冻结 Migration。**
