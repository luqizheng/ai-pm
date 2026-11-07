# PC-01B — 核心表：Cognition Object / Relationship（V1.2 拆分件 02，原文档 §5~§6）

> **拆分声明**：本文件为《PC-01B 数据模型与数据库设计》拆分件之一（内容基线 V1.1 拆分，**V1.2 差异复核落版 2026-11-07**：§5.3 C-1 承载域内化 / C-2 项目级组合 FK / 注记 2 编号勘误），内容为原文档 §5~§6；章节编号保持原文档编号。总纲/裁决/验收见拆分件 00；版本层与端点闭合见拆分件 03。

---

# 5. Cognition Object / Object Revision 模型

## 5.1 pc.object_type — 对象类型注册表（PC-D-04 载体之一）

```sql
CREATE TABLE pc.object_type (
    type_key          TEXT PRIMARY KEY,              -- 受控键，如 'product_vision'
    label             TEXT NOT NULL,
    layer             TEXT NOT NULL CHECK (layer IN
                        ('L1_identity','L2_structure','L3_usage','L4_semantic','L5_decision')),
    governance_level  TEXT NOT NULL CHECK (governance_level IN ('L1','L2')),
    singleton         BOOLEAN NOT NULL DEFAULT false,
    doc_template_key  TEXT,                          -- Markdown 模板标识（§10.4）
    status            TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','deprecated')),
    description       TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- 种子数据：§3.3 的 14 行（迁移内 INSERT，属 DDL 交付的一部分）
```

- 字典为**平台级**（无 tenant_id）：类型体系是 PC 产品模型的一部分，不由租户扩展。新增类型 = 种子迁移追加（受控），运行期不开放租户自定义。
- `governance_level IN ('L1','L2')`：L0 是无需提案的草稿动作、L3 是冻结态，都不是对象类型缺省等级（§3.3 注记）。
- 服务层校验：代码内常量注册表（14 类 + 字段模板）与字典表启动比对，不一致则拒绝启动（防止字典被绕过——PC-D-04「共同约束」）。

## 5.2 pc.cognition_object — 稳定身份

```sql
CREATE TABLE pc.cognition_object (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    project_id          UUID NOT NULL,
    object_type         TEXT NOT NULL REFERENCES pc.object_type(type_key),
    object_key          TEXT,                        -- 受控键（如 Profile slug）；可空，非空时项目内唯一
    lifecycle_status    TEXT NOT NULL DEFAULT 'identity' CHECK (lifecycle_status IN
                          ('identity','draft','active','deprecated','superseded')),
    display_name        TEXT,                        -- 受控检索副本（正文权威仍在 Git，基线 §12.2）
    current_revision_id UUID,                        -- 当前 Revision（可空：identity 态无 Revision）
    superseded_by_id    UUID,                        -- 承接对象（lifecycle='superseded' 时必填，CHECK 承载）
    created_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by          UUID REFERENCES core.principal(id),
    deprecated_at       TIMESTAMPTZ,
    UNIQUE (tenant_id, id),                            -- 0019 租户级身份键（A4）
    UNIQUE (tenant_id, project_id, id),                -- PC 项目级身份键（复合 FK 引用锚，§14.2）
    UNIQUE (tenant_id, project_id, object_key),        -- object_key 项目内唯一
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- 当前 Revision 指针：自引用循环 → DEFERRABLE，创建事务内先插 Object（NULL）再回填
    FOREIGN KEY (tenant_id, project_id, id, current_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id)
        DEFERRABLE INITIALLY IMMEDIATE,
    FOREIGN KEY (tenant_id, project_id, superseded_by_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id)
);

-- 单例约束（Profile / Vision）：每项目每单例类型至多一个「未废弃」对象
-- 〔V1.1（R12/F-12）〕谓词函数为 IMMUTABLE 硬编码实现（部分索引谓词不可查表），
-- 与字典 singleton 列的一致性由 §5.1 启动校验承担。
CREATE OR REPLACE FUNCTION pc.object_type_singleton(key TEXT) RETURNS boolean
    LANGUAGE sql IMMUTABLE AS
    $$ SELECT key IN ('product_profile','product_vision') $$;

CREATE UNIQUE INDEX uq_pc_object_singleton
    ON pc.cognition_object (tenant_id, project_id, object_type)
    WHERE pc.object_type_singleton(object_type) = true
      AND lifecycle_status IN ('identity','draft','active');
```

要点：

1. 身份不含可变正文（〔裁决〕基线 §5.2）；`display_name` 为受控投影，与正文一致性由服务层维护（写入 Revision 时同步），不构成第二权威。
2. `current_revision_id` 语义 = 当前生效 Revision（服务层保证其 revision_no 为该对象最大值；写入走 CAS，§17.2）。
3. 并发令牌：`updated_at` + `updated_by`；乐观锁以 `(current_revision_id, updated_at)` 快照比较实现（CAS 双令牌 A6 同构，§17.2）。
4. 生命周期约束触发器（终态不可复活 + superseded 必填承接对象）：

```sql
CREATE FUNCTION pc.enforce_object_lifecycle() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'pc.cognition_object % is terminal (%)', OLD.id, OLD.lifecycle_status;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded object % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER trg_pc_object_lifecycle BEFORE UPDATE ON pc.cognition_object
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_object_lifecycle();
```

## 5.3 pc.cognition_revision — 内容修订（append-only）

```sql
CREATE TABLE pc.cognition_revision (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    project_id          UUID NOT NULL,
    object_id           UUID NOT NULL,
    revision_no         INT  NOT NULL CHECK (revision_no >= 1),
    -- Git 正文引用（PC-D-02 V1.2/C-1：一对象一文档，正文权威 = pc.document_revision（PCGIT）；承载域内化）
    document_id         UUID NOT NULL,               -- → pc.document（域内，C-1）
    document_revision_id UUID NOT NULL,              -- → pc.document_revision（C-1）
    content_hash        TEXT NOT NULL,               -- = pc.document_revision.content_hash（发布核验相等）
    change_summary      TEXT,
    source_context      TEXT,                        -- 生成语境摘要（人工/AI/KB，追溯链起点之一）
    created_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, object_id, revision_no),   -- 修订号唯一递增
    UNIQUE (tenant_id, project_id, object_id, id),            -- 复合 FK 引用锚（Member/端点）
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    -- 文档承载引用（V1.2/C-1 域内化；C-2 项目级组合 FK：组合一致性 document_revision_id ∈ document_id
    -- 由库层保证，较原双租户级 FK 增强）
    FOREIGN KEY (tenant_id, project_id, document_id)
        REFERENCES pc.document (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id)
        REFERENCES pc.document_revision (tenant_id, project_id, document_id, id)
);

CREATE INDEX idx_pc_revision_object     ON pc.cognition_revision (tenant_id, project_id, object_id, revision_no DESC);
CREATE INDEX idx_pc_revision_document   ON pc.cognition_revision (tenant_id, document_revision_id);

-- append-only：禁止 UPDATE 与 DELETE（A2 范式，0026:257-262 同构）
CREATE TRIGGER trg_pc_revision_immutable BEFORE UPDATE OR DELETE ON pc.cognition_revision
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();
```

设计裁决：

1. **不冗余 `revision_status`**（基线 §5.3 建议属性，本设计显式不采纳）：是否进入正式版本 = `EXISTS(pc.version_member ...)`，派生可查（视图 `pc.v_revision_versioned`，§15.3）。冗余状态会引入「成员表与状态列」双写漂移，违背完整性目标。
2. `content_hash` 必须等于所引 `pc.document_revision.content_hash`（0039:19-21 sha256(markdown) 口径）——发布事务核验（§18.1 步骤③ 审批锚点主校验含成员集重算；V1.2 勘误：原引「步骤⑤」为旧编号占位，评审 N-7），不入库时校验（服务层，Git-PG 协议 §12）。
3. Revision 创建顺序：`INSERT revision（revision_no = max+1，服务层事务内 SELECT ... FOR UPDATE object 行推号）→ CAS 回填 current_revision_id`。并发推号由对象行锁 + `UNIQUE(tenant_id, project_id, object_id, revision_no)` 双保险（§17.2 场景 1）。
4. 修订正文不止一个文档的情况：**不允许**（PC-D-02 方案 A，§10.1）；对象内容需要拆分时走「新对象 + Relationship」。

---

# 6. Relationship / Relationship Revision 模型

## 6.1 端点身份规则（任务书 §3.3 五问，技术结论）

| # | 问题 | 结论 | 理由 |
|---|---|---|---|
| 1 | 端点是否属于 Relationship 稳定身份？ | **端点 Object 身份（object_id）属于 Relationship 身份；端点 Revision 属于 Relationship Revision** | 端点对象更换（A realized_by M → A realized_by N）是不同事实，应为新关系；端点对象内容更新（M 出新 Revision）不改变关系事实本身，只更新钉住的 Revision |
| 2 | 端点变化时新建还是修订？ | **端点对象身份变化 → 新 Relationship（旧关系 `superseded`，`superseded_by` 指向新关系）；端点 Revision 变化 → 新 Relationship Revision** | 身份稳定 + 语义版本化的折中：既保证关系身份可长期引用，又保证历史版本还原后端点语义不漂移 |
| 3 | Relationship Revision 是否保留端点语义快照？ | **是**。每条 Revision 冗余存储完整端点（对象身份 + Revision 身份，或跨域四元组），append-only 不可变 | 版本成员只需引用 relationship_revision_id 即可完整还原端点，无需 join 当前 relationship 行；冗余换取历史自包含 |
| 4 | 如何防止历史版本还原后关系指向变化？ | **Relationship Revision 行不可变触发器（禁 UPDATE/DELETE）** + 端点快照内嵌 Revision | 已发布版本成员指向的 Revision 永远持有创建时的端点；「还原 = 读历史」，无任何行会被改写 |
| 5 | 两端 Object Revision 如何在正式版本中完整闭合？ | §8 三层防线（复合 FK + 发布事务断言 + DEFERRED 约束触发器 + 服务层二次校验） | 集合级闭合无法用静态 FK 表达，必须由发布事务断言 + 延迟约束触发器共同承担 |

## 6.2 pc.relationship_type — 关系类型注册表（PC-D-04 载体之二）

```sql
CREATE TABLE pc.relationship_type (
    type_key              TEXT PRIMARY KEY,            -- 如 'realized_by'
    label                 TEXT NOT NULL,
    directionality        TEXT NOT NULL CHECK (directionality IN ('directed','undirected')),
    source_object_types   TEXT[] NOT NULL,             -- 允许的源端 object_type 集合（PC 内端点）
    target_object_types   TEXT[] NOT NULL,
    allow_self_reference  BOOLEAN NOT NULL DEFAULT false,  -- source 与 target 为同一对象
    allow_duplicate       BOOLEAN NOT NULL DEFAULT false,  -- V1 全 false（§6.3 注记 4）
    governance_level      TEXT NOT NULL CHECK (governance_level IN ('L1','L2')),
    status                TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','deprecated')),
    description           TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- 种子数据：基线 §7.2 核心关系矩阵落地——完整种子清单见 §15.5（V1.1/R8 补齐：
-- 17 条注册 + 2 条由 Source/Evidence 追溯结构承载，01C 直接执行，不再重新决策）
```

治理：新增/修改类型 = 种子迁移 + L2 人工确认流程（PC-D-04 裁决：类型变更不改运行期字典，改种子）；`governed_by` 类（业务规则约束）固定 L2。

## 6.3 pc.relationship — 稳定关系身份

```sql
CREATE TABLE pc.relationship (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    relationship_type_key TEXT NOT NULL REFERENCES pc.relationship_type(type_key),
    lifecycle_status      TEXT NOT NULL DEFAULT 'identity' CHECK (lifecycle_status IN
                            ('identity','draft','active','deprecated','superseded')),
    -- 稳定身份端点（PC 内）：对象身份属于关系身份（§6.1 #1）
    source_object_id      UUID,
    target_object_id      UUID,
    -- 跨域端点（四元组协议，01A §6.6）：与 PC 内端点严格互斥
    source_ext_domain     TEXT,  source_ext_type  TEXT,
    source_ext_id         TEXT,  source_ext_revision_id TEXT,
    target_ext_domain     TEXT,  target_ext_type  TEXT,
    target_ext_id         TEXT,  target_ext_revision_id TEXT,
    superseded_by_id      UUID,
    current_revision_id   UUID,
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, target_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, superseded_by_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, id, current_revision_id)
        REFERENCES pc.relationship_revision (tenant_id, project_id, relationship_id, id)
        DEFERRABLE INITIALLY IMMEDIATE,
    -- 端点形态互斥：PC 内端点 XOR 跨域端点（source / target 各自独立判定）
    CHECK ( (source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL) ),
    CHECK ( (target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL) ),
    -- 跨域端点必须带 domain/type（四元组三元定位，01A §6.6 要求 1）
    CHECK ( source_ext_id IS NULL OR (source_ext_domain IS NOT NULL AND source_ext_type IS NOT NULL) ),
    CHECK ( target_ext_id IS NULL OR (target_ext_domain IS NOT NULL AND target_ext_type IS NOT NULL) ),
    CHECK ( lifecycle_status <> 'superseded' OR superseded_by_id IS NOT NULL )
);

-- 活跃关系身份唯一：同类型 + 同端点组合不可重复（函数唯一索引容忍跨域 NULL）
CREATE UNIQUE INDEX uq_pc_relationship_identity
    ON pc.relationship (tenant_id, project_id, relationship_type_key,
        coalesce(source_object_id::text, source_ext_domain || ':' || source_ext_type || ':' || source_ext_id),
        coalesce(target_object_id::text, target_ext_domain || ':' || target_ext_type || ':' || target_ext_id))
    WHERE lifecycle_status IN ('identity','draft','active');

CREATE INDEX idx_pc_relationship_source ON pc.relationship (tenant_id, project_id, source_object_id, lifecycle_status);
CREATE INDEX idx_pc_relationship_target ON pc.relationship (tenant_id, project_id, target_object_id, lifecycle_status);
```

注记：

1. 方向语义由 `relationship_type.directionality` 定义（directed：source→target；undirected：端点对称），关系行不重复存方向列；Revision 快照冗余方向性（§6.4），保证历史自包含。
2. 端点对象身份变更 = 新关系：服务层禁止 UPDATE `source_object_id` / `target_object_id`（锚定触发器，§6.5），仅 `lifecycle_status` / `current_revision_id` / `superseded_by_id` / 审计列可变。
3. 跨域端点 Revision 为可空（按 01A §6.6 通道状态表：requirement 必填、S01-04 只读不钉、project_goal 允许空缺）；跨域完整性校验在服务层（§13.3），库层只保三元定位非空（CHECK 已含）。
4. `allow_duplicate`：V1 全部类型 false（活跃唯一索引生效）。若未来某类型需要重复关系，需将该类型的活跃唯一索引改为按 `relationship_id` 豁免的函数索引（01C 评估），字典列已预留语义。

## 6.4 pc.relationship_revision — 关系修订（append-only，端点语义快照）

```sql
CREATE TABLE pc.relationship_revision (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    relationship_id       UUID NOT NULL,
    revision_no           INT  NOT NULL CHECK (revision_no >= 1),
    relationship_type_key TEXT NOT NULL REFERENCES pc.relationship_type(type_key),
    directionality        TEXT NOT NULL CHECK (directionality IN ('directed','undirected')),
    -- ── 端点语义快照（§6.1 #3：Revision 自包含）────────────────────────
    source_object_id      UUID,  source_object_revision_id UUID,
    target_object_id      UUID,  target_object_revision_id UUID,
    source_ext_domain     TEXT,  source_ext_type  TEXT,
    source_ext_id         TEXT,  source_ext_revision_id TEXT,
    target_ext_domain     TEXT,  target_ext_type  TEXT,
    target_ext_id         TEXT,  target_ext_revision_id TEXT,
    -- ── 语义与属性 ─────────────────────────────────────────────────
    properties            JSONB NOT NULL DEFAULT '{}'::jsonb,  -- 受控属性（§6.4 注记 3）
    change_summary        TEXT,
    -- 关系正文（可选）：关系通常无需 Markdown；需要时同构承载
    document_id           UUID,
    document_revision_id  UUID,
    content_hash          TEXT NOT NULL,   -- properties（+可选正文）的规范哈希
    created_by            UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, relationship_id, revision_no),
    UNIQUE (tenant_id, project_id, relationship_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    -- 端点 Revision 复合 FK（PC 内端点）：身份与 Revision 归属一致性由库层保证
    -- （任务书 §4.2；MATCH SIMPLE：跨域行 NULL 自动跳过）
    FOREIGN KEY (tenant_id, project_id, source_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, source_object_id, source_object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, target_object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, target_object_id, target_object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    -- 端点形态与身份层一致性：Revision 端点对象必须与 Relationship 行端点一致
    -- （行级仅能对照本行两列，跨行一致性由约束触发器 §6.5 承担）
    CHECK ( (source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL) ),
    CHECK ( (target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL) ),
    CHECK ( source_object_id IS NULL OR source_object_revision_id IS NOT NULL ),  -- PC 内端点必钉 Revision
    CHECK ( target_object_id IS NULL OR target_object_revision_id IS NOT NULL ),
    CHECK ( source_ext_id IS NULL OR (source_ext_domain IS NOT NULL AND source_ext_type IS NOT NULL) ),
    CHECK ( target_ext_id IS NULL OR (target_ext_domain IS NOT NULL AND target_ext_type IS NOT NULL) )
);

CREATE INDEX idx_pc_rel_revision_rel ON pc.relationship_revision (tenant_id, project_id, relationship_id, revision_no DESC);
CREATE INDEX idx_pc_rel_revision_endpoint_src ON pc.relationship_revision (tenant_id, project_id, source_object_id, source_object_revision_id);
CREATE INDEX idx_pc_rel_revision_endpoint_tgt ON pc.relationship_revision (tenant_id, project_id, target_object_id, target_object_revision_id);

CREATE TRIGGER trg_pc_rel_revision_immutable BEFORE UPDATE OR DELETE ON pc.relationship_revision
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();
```

注记：

1. `directionality` 是类型快照（该 Revision 生效时的类型定义），保证类型定义后续演化不影响历史 Revision 语义。
2. PC 内端点**必钉 Revision**（CHECK 强制）：四元组协议「有 Revision 的领域对象必须支持 Revision 级引用」（01A §6.6 要求 2）在 PC 内端点上的库层落实。
3. `properties` JSONB 边界（任务书 §15 JSONB 使用边界）：仅承载 `relationship_type` 声明的属性 schema 内数据（服务层按类型校验），不得用于端点身份、审批事实、版本成员——后者一律结构化列。
4. `content_hash` = 规范化序列化（type + directionality + 端点 + properties + 可选正文 hash）后的 sha256；发布时 snapshot_hash 计算输入之一。

## 6.5 Relationship 行锚定触发器

```sql
CREATE FUNCTION pc.enforce_relationship_anchors() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- 端点身份与类型属于身份锚：变更 = 新关系（§6.1 #2）
    IF (OLD.relationship_type_key IS DISTINCT FROM NEW.relationship_type_key)
       OR (OLD.source_object_id IS DISTINCT FROM NEW.source_object_id)
       OR (OLD.target_object_id IS DISTINCT FROM NEW.target_object_id)
       OR (OLD.source_ext_domain IS DISTINCT FROM NEW.source_ext_domain)
       OR (OLD.source_ext_type IS DISTINCT FROM NEW.source_ext_type)
       OR (OLD.source_ext_id IS DISTINCT FROM NEW.source_ext_id)
       OR (OLD.target_ext_domain IS DISTINCT FROM NEW.target_ext_domain)
       OR (OLD.target_ext_type IS DISTINCT FROM NEW.target_ext_type)
       OR (OLD.target_ext_id IS DISTINCT FROM NEW.target_ext_id) THEN
        RAISE EXCEPTION 'pc.relationship % endpoint anchors are immutable', OLD.id;
    END IF;
    -- 终态不可复活（同 §5.2）
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'pc.relationship % is terminal', OLD.id;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded relationship % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER trg_pc_relationship_anchors BEFORE UPDATE ON pc.relationship
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_relationship_anchors();

-- 〔V1.1（R3/F-03）〕Relationship Revision 端点身份一致性：RR 的端点身份与类型键
-- 必须等于父 Relationship 行（§6.1 #1 裁决的库层强制——修复评审报告构造的
-- 「R: A→B 身份行 + RR: C→B 修订」绕过路径）。Revision 只变端点 Revision，不变身份。
CREATE FUNCTION pc.enforce_rel_revision_endpoint_match() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE r_row pc.relationship%ROWTYPE;
BEGIN
    SELECT * INTO r_row FROM pc.relationship
     WHERE tenant_id = NEW.tenant_id AND project_id = NEW.project_id
       AND id = NEW.relationship_id;
    IF (r_row.relationship_type_key IS DISTINCT FROM NEW.relationship_type_key)
       OR (r_row.source_object_id IS DISTINCT FROM NEW.source_object_id)
       OR (r_row.target_object_id IS DISTINCT FROM NEW.target_object_id)
       OR (r_row.source_ext_domain IS DISTINCT FROM NEW.source_ext_domain)
       OR (r_row.source_ext_type IS DISTINCT FROM NEW.source_ext_type)
       OR (r_row.source_ext_id IS DISTINCT FROM NEW.source_ext_id)
       OR (r_row.target_ext_domain IS DISTINCT FROM NEW.target_ext_domain)
       OR (r_row.target_ext_type IS DISTINCT FROM NEW.target_ext_type)
       OR (r_row.target_ext_id IS DISTINCT FROM NEW.target_ext_id) THEN
        RAISE EXCEPTION 'relationship_revision % endpoint identity does not match relationship %',
            NEW.id, NEW.relationship_id;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER trg_pc_rel_revision_endpoint_match BEFORE INSERT ON pc.relationship_revision
    FOR EACH ROW EXECUTE FUNCTION pc.enforce_rel_revision_endpoint_match();
```