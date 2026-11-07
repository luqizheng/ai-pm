# PC-01B — 治理与追溯：Source / Evidence / Proposal / Approval / Conflict（V1.2 拆分件 04，原文档 §9~§10）

> **拆分声明**：本文件为《PC-01B 数据模型与数据库设计》拆分件之一（内容基线 V1.1 拆分，**V1.2 差异复核落版 2026-11-07**：§9.2/§9.3 C-4 撤 ai FK + 撤 document\* 定位列、evidence 增四元组列、判别器 CHECK 改写、§9.4 指称更新），内容为原文档 §9~§10；章节编号保持原文档编号。总纲/裁决/验收见拆分件 00；§11 PC-D 裁决详录（含 §11.2 PC-D-02、§11.3 PC-D-06）见拆分件 00。

---

# 9. Source / Evidence / KB Memory 模型（PC-D-05 关闭）

## 9.1 设计裁决

1. **Source 与 Evidence 实体化分离**（〔裁决〕基线 §6/01A §6.4）：Source = 信息来自哪里（四元组指针 + 来源定位）；Evidence = 支持或反驳某条 Revision 的具体依据（结构化定位 + 内容哈希 + 判定状态）。
2. **禁止单 JSONB 混用语义**（〔裁决〕01A §6.4）：Source 的四元组、Evidence 的定位字段全部结构化列；JSONB 仅保留给 `properties` 类真正灵活属性（本节两表无 JSONB 列）。
3. **证据钉 Revision 级**：evidence 只能挂在 object_revision 或 relationship_revision 上（二选一 CHECK），不挂身份层——追溯链按「版本成员引用的 Revision」闭合。
4. **Confidence 与 Confirmation 分离**（〔裁决〕基线 §6.4）：`confidence`（high/medium/low，AI 或来源可信度判断）与正式确认状态（由 proposal/approval/version 事实承载）分属不同表、不同字段；任何高 Confidence 不产生正式确认语义。

## 9.2 pc.source — 来源

```sql
CREATE TABLE pc.source (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    source_kind           TEXT NOT NULL CHECK (source_kind IN
                            ('kb_document','kb_memory','manual_input',
                             'requirement','requirement_revision',
                             'discovery_finding','project_cognition','external_reference')),
    -- 四元组协议（01A §6.6）：三元定位必填，Revision 级引用按通道状态表（§13.1）
    target_domain         TEXT NOT NULL,
    target_type           TEXT NOT NULL,
    target_id             TEXT NOT NULL,           -- TEXT：容纳跨域非 UUID 身份
    target_revision_id    TEXT,
    revision_missing_reason TEXT,                  -- 允许空缺的通道必须记录原因（01A §6.6）
    -- 来源定位（结构化；V1.2/C-4：撤 document_* 定位列与 ai FK——KB 文档引用全部走上方四元组列，
    -- section_anchor/start_line 对 target_revision_id 指向的文档做章节定位，不依赖 FK；跨域完整性=服务层组合键校验+周期对账（§13.3，R-13））
    section_anchor        TEXT,
    start_line            INT,
    locator               TEXT,                    -- 其余域的自由定位（路径/URL/记忆 ID 等单一值）
    description           TEXT,
    source_content_hash   TEXT,                    -- 来源内容指纹（对账/去重参考）
    collected_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- Revision 空缺语义：无 Revision 必须有原因（不允许「忘了填」）
    CHECK ( target_revision_id IS NOT NULL OR revision_missing_reason IS NOT NULL ),
    -- kb_document 通道收窄（C-4/V1.2）：必须四元组钉 KB 文档 Revision，堵 revision_missing_reason 逃逸口（评审 C-4-③）
    CHECK ( source_kind <> 'kb_document'
            OR (target_domain = 'knowledge_base' AND target_revision_id IS NOT NULL) )
);

CREATE INDEX idx_pc_source_target  ON pc.source (tenant_id, project_id, target_domain, target_type);
CREATE INDEX idx_pc_source_kind    ON pc.source (tenant_id, project_id, source_kind);
```

## 9.3 pc.evidence — 证据（A7 判别器模式）

```sql
CREATE TABLE pc.evidence (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                 UUID NOT NULL REFERENCES core.tenant(id),
    project_id                UUID NOT NULL,
    source_id                 UUID NOT NULL,
    -- 证据主体：Revision 级二选一（对象 Revision XOR 关系 Revision）
    object_revision_id        UUID,
    relationship_revision_id  UUID,
    evidence_type             TEXT NOT NULL CHECK (evidence_type IN
                                ('code','test_result','commit','document_section',
                                 'meeting','user_statement','system_observation')),
    -- 结构化定位（A7：0031:128-171；V1.2/C-4：撤 document_* 定位列与 ai FK——
    -- document_section 类证据经 target_* 四元组列引用 KB 文档 Revision + section_anchor/end_line 定位）
    target_domain             TEXT,               -- 证据来源四元组（document_section 类必填 knowledge_base，C-4）
    target_type               TEXT,
    target_id                 TEXT,
    target_revision_id        TEXT,
    file_path                 TEXT,
    symbol                    TEXT,
    code_range                TEXT,
    commit_sha                TEXT,
    section_anchor            TEXT,
    start_line                INT,
    end_line                  INT,
    excerpt                   TEXT,               -- 短摘录（受控例外，基线 §12.2）
    description               TEXT,
    evidence_hash             TEXT NOT NULL,      -- 被引内容的哈希（防证据漂移；C-4 后亦为跨域引用内容锚）
    confidence                TEXT NOT NULL DEFAULT 'medium'
                                CHECK (confidence IN ('high','medium','low')),
    verification_status       TEXT NOT NULL DEFAULT 'unverified'
                                CHECK (verification_status IN
                                ('unverified','verified','refuted','conflicted')),
    created_by                UUID NOT NULL REFERENCES core.principal(id),
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES pc.source (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_revision_id)
        REFERENCES pc.relationship_revision (tenant_id, project_id, id),
    -- 主体二选一
    CHECK (
        (object_revision_id IS NOT NULL) <> (relationship_revision_id IS NOT NULL)
    ),
    -- 类型判别器（A7 模式：定位列按证据类型互斥约束，禁止单 JSONB 混装；V1.2/C-4：document_section 改四元组列）
    CHECK (
        ( evidence_type IN ('code','test_result','commit')
          AND file_path IS NOT NULL AND commit_sha IS NOT NULL
          AND target_domain IS NULL )
        OR
        ( evidence_type = 'document_section'
          AND target_domain = 'knowledge_base'
          AND target_revision_id IS NOT NULL
          AND file_path IS NULL AND commit_sha IS NULL )
        OR
        ( evidence_type IN ('meeting','user_statement','system_observation')
          AND (excerpt IS NOT NULL OR description IS NOT NULL)
          AND target_domain IS NULL )
    )
);

CREATE UNIQUE INDEX uq_pc_evidence_identity
    ON pc.evidence (tenant_id, project_id, source_id,
        coalesce(object_revision_id::text, relationship_revision_id::text),
        evidence_type, coalesce(file_path,''), coalesce(section_anchor,''), coalesce(code_range,''));
CREATE INDEX idx_pc_evidence_subject ON pc.evidence (tenant_id, project_id, object_revision_id);
CREATE INDEX idx_pc_evidence_subject_rel ON pc.evidence (tenant_id, project_id, relationship_revision_id);
```

注记：

1. 代码/提交/测试类证据要求 `file_path + commit_sha` 齐备（A7 对应 commit_sha/attempt 语义，PC 以 commit_sha + locator 承载，不 FK discovery/import 域——那些是各自领域数据，PC 只记事实快照）。
2. `verification_status` 是证据核验状态，**不是**正式确认状态（Confirmation 走 proposal/approval/version 链）。
3. `uq_pc_evidence_identity` 防同源同定位证据重复登记；不同 subject 可共享同一 source。
4. **〔V1.2/C-4〕document_\* 定位列已撤除**：document_section 类证据经 target_\* 四元组列（钉 KB 文档 Revision）+ section_anchor/end_line 定位；跨域引用完整性 = 服务层组合键校验 + 周期对账 + evidence_hash 内容锚（R-13；对账数据源=K3/PK1 只读 API，禁止直查 KB schema）。

## 9.4 KB Memory 输入通道（Q-PC-12）

```text
knowledge_base 域文档/记忆（当前 ai.document\*，目标 kb.document\*（ADR-06b）；审核通过）
    → pc.source (source_kind='kb_memory' | 'kb_document', 四元组指向 KB 对象)
    → pc.proposal (origin_kind='kb_memory', proposal_member 拟变更集)
    → pc.approval (人工确认)
    → pc.cognition_version (发布)
```

- KB 审核通过 ≠ PC 正式确认（〔裁决〕Q-PC-12）：`pc.source` 行只登记「输入存在」，不产生任何对象/版本状态变化。
- 直接人工输入（manual_input）：同样必须先建 `pc.source`（source_kind='manual_input'）再建 proposal——追溯链完整（任务书 §6.3）。
- 本通道是**运行时输入**，无任何历史数据迁移语义（零迁移硬边界，§2.1）。

## 9.5 追溯链完整性要求

全链可查：`pc.evidence → pc.source → proposal → cognition_revision/relationship_revision → cognition_version`。§16 Q13 给出链式查询与索引支撑；反向（对象 → 其全部来源/证据）由 `idx_pc_evidence_subject` 支撑（修复 M-4「来源反向索引完全缺失」）。

---

# 10. Proposal / Approval / Conflict 模型（PC-D-01 关闭）

## 10.1 pc.proposal + pc.proposal_member — 提案与拟变更成员集

```sql
CREATE TABLE pc.proposal (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           UUID NOT NULL REFERENCES core.tenant(id),
    project_id          UUID NOT NULL,
    title               TEXT NOT NULL,
    rationale           TEXT NOT NULL,
    origin_kind         TEXT NOT NULL CHECK (origin_kind IN
                          ('ai_agent','kb_memory','manual_input','requirement_change',
                           'conflict_resolution')),
    source_id           UUID,                        -- 追溯链入口（可空：多来源时经 proposal 引用 evidence）
    governance_level    TEXT NOT NULL CHECK (governance_level IN ('L1','L2')),
                        -- 〔V1.1（R7/F-07）〕等级一致性契约：提案 governance_level 必须 ≥
                        -- 全部目标类型/关系类型的固定等级（object_type/relationship_type
                        -- 注册表 governance_level 列；business_rule/product_vision/
                        -- business_process 固定 L2 不可下调）。服务层 submit 前置校验。
    risk_level          TEXT NOT NULL DEFAULT 'medium'
                          CHECK (risk_level IN ('low','medium','high','critical')),
    confidence          TEXT NOT NULL DEFAULT 'medium'
                          CHECK (confidence IN ('high','medium','low')),
    expected_version_no BIGINT NOT NULL CHECK (expected_version_no >= 0),
                        -- 提案基于的当前正式版本号（0 = 项目尚无版本）；发布事务 CAS 核验（§17.2 场景 3/5）
    proposal_content_hash TEXT NOT NULL,
                        -- 规范哈希 = hash(title, rationale, 有序成员集, 变更摘要)；审批锚定（八原则 1/6）
    status              TEXT NOT NULL DEFAULT 'draft' CHECK (status IN
                          ('draft','submitted','approved','rejected','withdrawn',
                           'expired','applied','failed')),
    submitted_at        TIMESTAMPTZ,
    created_by          UUID NOT NULL REFERENCES core.principal(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES pc.source (tenant_id, project_id, id)
);

CREATE INDEX idx_pc_proposal_status ON pc.proposal (tenant_id, project_id, status, created_at DESC);

CREATE TABLE pc.proposal_member (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                 UUID NOT NULL REFERENCES core.tenant(id),
    project_id                UUID NOT NULL,
    proposal_id               UUID NOT NULL,
    change_kind               TEXT NOT NULL CHECK (change_kind IN
                                ('create_object','update_object','create_relationship',
                                 'update_relationship','deprecate_object','deprecate_relationship')),
    object_id                 UUID,
    object_revision_id        UUID,
    relationship_id           UUID,
    relationship_revision_id  UUID,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES pc.proposal (tenant_id, project_id, id),
    -- 目标引用完整性（修复 m-3 型缺陷：提案目标必须存在于同项目）
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id)
        REFERENCES pc.cognition_revision (tenant_id, project_id, object_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id)
        REFERENCES pc.relationship_revision (tenant_id, project_id, relationship_id, id),
    -- 双维度互斥（同 §7.3）
    CHECK (
        (    object_id IS NOT NULL AND object_revision_id IS NOT NULL
         AND relationship_id IS NULL AND relationship_revision_id IS NULL )
        OR
        (    relationship_id IS NOT NULL AND relationship_revision_id IS NOT NULL
         AND object_id IS NULL AND object_revision_id IS NULL )
    ),
    -- change_kind 与维度一致性：create_* 的 Revision 可空（待 Git 提交后回填提案成员 Revision，
    -- 提交前由 proposal_content_hash 锚定内容）；update_*/deprecate_* 必须指认目标
    CHECK ( change_kind NOT IN ('update_object','deprecate_object')
            OR object_id IS NOT NULL ),
    CHECK ( change_kind NOT IN ('update_relationship','deprecate_relationship')
            OR relationship_id IS NOT NULL )
);

CREATE UNIQUE INDEX uq_pc_pm_target
    ON pc.proposal_member (tenant_id, project_id, proposal_id,
        coalesce(object_id::text,''), coalesce(relationship_id::text,''))
    WHERE change_kind IN ('update_object','deprecate_object',
                          'update_relationship','deprecate_relationship');
-- 同提案对同一目标至多一条「更新/废弃」类变更（create 类天然不重复，目标为新身份）

-- 提案提交后成员锚定：draft 之外禁止改动成员集（审批锚定 hash 的前提）
CREATE FUNCTION pc.enforce_proposal_member_anchored() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p_status TEXT;
BEGIN
    SELECT status INTO p_status FROM pc.proposal
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.proposal_id, OLD.proposal_id);
    IF p_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'proposal % is %; members are anchored by proposal_content_hash',
            COALESCE(NEW.proposal_id, OLD.proposal_id), p_status;
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored AFTER UPDATE OR DELETE ON pc.proposal_member
    DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION pc.enforce_proposal_member_anchored();
CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored_ins AFTER INSERT ON pc.proposal_member
    DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION pc.enforce_proposal_member_anchored();

-- 〔V1.1（R4/F-04）〕Proposal 内容锚定：submitted 之后内容锚定列 once-written
-- （八原则 1 的 proposal 侧闭环；审批锚定 hash 的比对基础不可变）。
CREATE FUNCTION pc.forbid_proposal_content_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status <> 'draft' THEN
        IF (OLD.title IS DISTINCT FROM NEW.title)
           OR (OLD.rationale IS DISTINCT FROM NEW.rationale)
           OR (OLD.origin_kind IS DISTINCT FROM NEW.origin_kind)
           OR (OLD.governance_level IS DISTINCT FROM NEW.governance_level)
           OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
           OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
           OR (OLD.source_id IS DISTINCT FROM NEW.source_id) THEN
            RAISE EXCEPTION 'pc.proposal % is %; content anchor columns are immutable', OLD.id, OLD.status;
        END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER trg_pc_proposal_anchors BEFORE UPDATE ON pc.proposal
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_proposal_content_mutation();
```

**成员时序契约〔V1.1（R5/F-05）裁决，发布输入的硬前提〕**：

1. **draft 阶段即完成全部落库**：提案起草（draft）内完成 Git 提交与 Revision 登记并可回填 proposal_member——`change_kind/object_id/relationship_id` 身份列与 `object_revision_id/relationship_revision_id` 全部齐备（互斥 CHECK 自然满足）。上表 §10.1 CHECK 注记中「create_* Revision 可空待回填」仅指 draft 阶段的瞬态，**submitted 时刻必须齐备**。
2. **submit 前置校验**（服务层）：成员集非空、每个成员 Revision 非空、create_relationship 成员的 PC 内端点对象成员已在集合中（见第 3 条）、proposal_content_hash 按成员集终态计算。
3. **端点成员自动并入〔F-05 方案 A，2026-11-07 授权裁决〕**：提案服务在构建成员集时，对每个关系成员自动检查其 RR 端点（PC 内端点的 `(object_id, object_revision_id)`），缺失则自动并入对象成员（若端点 Revision 与既有成员 Revision 不同，以 RR 端点快照为准并入），并纳入 proposal_content_hash 计算。提案人无需手工凑齐端点；发布事务步骤②后仍有完整性断言兜底（§18.1）。
4. 由此 `trg_pc_pm_anchored`（submitted 后禁改成员）与发布输入完整性不再矛盾——回填窗口只存在于 draft 阶段。

## 10.2 pc.approval — 不可变审批事实（A2 范式 + 八项技术原则）

```sql
CREATE TABLE pc.approval (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id             UUID NOT NULL REFERENCES core.tenant(id),
    project_id            UUID NOT NULL,
    proposal_id           UUID NOT NULL,
    proposal_content_hash TEXT NOT NULL,             -- 锚定：审批对象不可事后替换（八原则 1）
    expected_version_no   BIGINT NOT NULL,           -- 锚定：审批时预期版本（八原则 6，CAS 配套）
    status                TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','approved','rejected')),
    decided_by            UUID REFERENCES core.principal(id),   -- 决策人（权限闸 A3，§10.2 注记 2）
    decision_reason       TEXT,
    decided_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES pc.proposal (tenant_id, project_id, id)
);

-- 同提案至多一个待决审批（八原则 4：不允许重复有效审批；0026:250-251 范式）
CREATE UNIQUE INDEX uq_pc_approval_pending
    ON pc.approval (proposal_id) WHERE status = 'pending';

-- 锚定列 once-written + 状态单向（0026:273-296 同构，八原则 2/3/5）
CREATE FUNCTION pc.forbid_approval_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF (OLD.proposal_id IS DISTINCT FROM NEW.proposal_id)
       OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
       OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'pc.approval % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'pending' THEN
        IF NEW.status NOT IN ('approved','rejected') THEN
            RAISE EXCEPTION 'pc.approval % may only move from pending to a terminal status', OLD.id;
        END IF;
        IF NEW.decided_by IS NULL OR NEW.decided_at IS NULL THEN
            RAISE EXCEPTION 'pc.approval % terminal status requires decider and time', OLD.id;
        END IF;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'pc.approval % is already decided and cannot change status', OLD.id;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER trg_pc_approval_immutable BEFORE UPDATE ON pc.approval
    FOR EACH ROW EXECUTE FUNCTION pc.forbid_approval_mutation();

CREATE INDEX idx_pc_approval_proposal ON pc.approval (tenant_id, project_id, proposal_id, created_at DESC);
```

注记（八项技术原则逐条落实，〔裁决〕基线 §9.3）：

| 八项原则 | 库层/服务层落实 |
|---|---|
| 1 审批对象不可事后替换 | `proposal_content_hash` once-written（触发器）；proposal 成员集 submitted 后锚定（§10.1） |
| 2 状态受控 | `status CHECK 3 值`（对比 approval_request 无 CHECK，0003:162） |
| 3 状态单向转换 | 触发器 pending→terminal 单向（0026:285-291 同构） |
| 4 不允许重复有效审批 | `uq_pc_approval_pending` 部分唯一索引 |
| 5 审批事实不可覆盖 | 锚定列 immutable + 终态行不可改状态；无 UPDATE 场景绕道 |
| 6 版本发布与审批一致 | 发布事务核验 `proposal_content_hash` 与 `expected_version_no` 双锚点（§18.1 步骤③④⑤） |
| 7 并发事务保护 | 待决审批行 `SELECT ... FOR UPDATE`（§17.2 场景 4）+ 部分唯一索引兜底 |
| 8 全程可审计 | 每次审批动作写 `governance.audit_log`（source_kind='pc_proposal.approve/reject'，§19） |

权限判定复用 A3：`ensure_governance_authorized`（approver.rs:56-79，Lead OR `core.approver_grant` 生效行）——不重复建设权限体系（Q-PC-07/08）。已知限制（H-11）：该闸为无锁只读判定，授权撤销与审批提交的竞态窗口由 §21.2 的锁序设计收窄。

## 10.3 pc.conflict + pc.conflict_object — 冲突与冻结范围

```sql
CREATE TABLE pc.conflict (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id         UUID NOT NULL REFERENCES core.tenant(id),
    project_id        UUID NOT NULL,
    conflict_type     TEXT NOT NULL CHECK (conflict_type IN
                        ('content_contradiction','rule_contradiction','scope_conflict',
                         'relationship_conflict','source_conflict','revision_conflict',
                         'reality_mismatch','cross_domain_authority')),
    severity          TEXT NOT NULL CHECK (severity IN ('critical','major','minor')),
    description       TEXT NOT NULL,
    detection_source  TEXT,                        -- 检测来源摘要（AI/人工/对账任务）
    status            TEXT NOT NULL DEFAULT 'detected' CHECK (status IN
                        ('detected','under_review','awaiting_human','resolved',
                         'dismissed','reopened')),
    resolution_note   TEXT,
    resolved_by       UUID REFERENCES core.principal(id),
    resolved_at       TIMESTAMPTZ,
    created_by        UUID NOT NULL REFERENCES core.principal(id),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- 终态必须留痕；resolved/dismissed 必须有裁决人与时间
    CHECK ( status NOT IN ('resolved','dismissed')
            OR (resolved_by IS NOT NULL AND resolved_at IS NOT NULL) )
);

CREATE TABLE pc.conflict_object (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conflict_id     UUID NOT NULL,
    tenant_id       UUID NOT NULL REFERENCES core.tenant(id),
    project_id      UUID NOT NULL,
    object_id       UUID,
    relationship_id UUID,
    FOREIGN KEY (tenant_id, project_id, conflict_id) REFERENCES pc.conflict (tenant_id, project_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, project_id, object_id)
        REFERENCES pc.cognition_object (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, relationship_id)
        REFERENCES pc.relationship (tenant_id, project_id, id),
    CHECK ( (object_id IS NOT NULL) <> (relationship_id IS NOT NULL) )
);
-- 〔V1.1（R1/F-01）〕原内联 PRIMARY KEY 含 coalesce 表达式（PostgreSQL 语法错误），
-- 改为 id 主键 + 独立表达式唯一索引：
CREATE UNIQUE INDEX uq_pc_conflict_object
    ON pc.conflict_object (tenant_id, project_id, conflict_id,
        coalesce(object_id::text,''), coalesce(relationship_id::text,''));

CREATE INDEX idx_pc_conflict_status ON pc.conflict (tenant_id, project_id, status);
CREATE INDEX idx_pc_conflict_object ON pc.conflict_object (tenant_id, project_id, object_id);
CREATE INDEX idx_pc_conflict_rel    ON pc.conflict_object (tenant_id, project_id, relationship_id);
```

冻结语义（〔裁决〕基线 §9.4 L3）：`status IN ('detected','under_review','awaiting_human','reopened')` 的 conflict 经 conflict_object 圈定受影响对象/关系；发布事务步骤⑥与提案提交服务校验目标不在冻结集内，否则拒绝（§18.1）。`resolved/dismissed` 解冻。状态可复位（reopened 重新冻结）；冲突行本身不承载 Revision 改写（冲突解决经 proposal 走正常变更链，〔裁决〕基线 §10.4-6）。