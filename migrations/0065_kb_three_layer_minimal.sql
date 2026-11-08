-- 0065_kb_three_layer_minimal：KP-3 KB 三层最小面（V0.5 范围基线 V1.3 §3.3/§五 目标 4）。
-- 设计依据：
--   * docs/product-cognition-knowledge-base/product-cognition-knowledge-base-definition-architecture-v1.md
--     §5（三层资料模型 + Discussion Memory L1/L2）/ §6（Team Source Pool，一源多项目）/
--     §9（浅层记忆任何内容变化须人工审核 + 版本不可变）；
--   * docs/architecture/knowledge-services/kb-pc-independent-services-architecture-v1.1.md
--     §7.1（kb.source / kb.memory / kb.memory_revision 目标表，KB 唯一写入方）/ §7.3（四元组
--     引用本体）/ §12.2 K4（CAS 审核转达）K5（来源只读查询）/ §13.3 kb.memory.approved。
--
-- 编号注：台账纪律①——0063 已由 D-2=c 批公开记账（91fe74e）、0064 已由 KP-1 PC-01B 正式
-- 模型批占用（5ecb0e3 合入 master，`0064_pc_formal_model.sql`）→ 本批再顺移取 0065。
--
-- 三层落点：
--   * Original Sources   = kb.source（团队级统一身份，内容不可改写）+ kb.source_project
--                          （项目关联：一份资料可关联多个项目，访问权继承自关联项目成员关系）；
--   * Extracted Content  = 既有 kb.document / kb.document_revision（0053 digest 半层，本批不动）；
--   * Structured Memory  = kb.memory + kb.memory_revision + kb.memory_ref（L1 Shallow Memory）。
--
-- Discussion Memory 与 0060 ai.conversation* 的分工（零重复建设）：讨论原文 = L2 Deep Memory，
-- 继续留在既有载体（ai.conversation_message / delivery.task_interaction / kb.source），本迁移
-- 不复制任何消息；只建 L1 沉淀/评审/注入的差量面——kb.memory_ref 以 DomainRef 四元组指向 L2
-- 原文（ref_role='evidence'）与讨论主题对象（ref_role='subject'，按 Task 检索的锚）。
--
-- L1 治理（fail-closed，库层兜底，应用层另做真人校验）：
--   1. 任何内容变化（CREATE/UPDATE/WITHDRAW）先以 PENDING_REVIEW 修订进入待审核区，**插入时
--      不得自带审核结论或版本状态**（触发器拒绝）；
--   2. 只有 review_status='APPROVED' 且审核人/时刻齐备的修订才拥有 version_status（CHECK）；
--   3. 修订内容列不可变、行不可物理删除（触发器）；审核结论单向一次；版本状态迁移白名单
--      NULL→CURRENT|WITHDRAWN（随批准）、CURRENT→SUPERSEDED（被新版本替代）；
--   4. 同一记忆至多一条待审修订、至多一条 CURRENT 版本（部分唯一索引）。
-- MERGED 为 §9.2 版本状态词表保留值（合并流程后置 V0.6，本批无写入方、迁移白名单不开放）。

-- ── 通用：不可变行守卫（kb.source / kb.memory_ref 共用）─────────────────────────
CREATE FUNCTION kb.reject_immutable_mutation() RETURNS trigger AS $func$
BEGIN
    RAISE EXCEPTION '%.% is immutable (append-only; drop trigger to migrate)',
        TG_TABLE_SCHEMA, TG_TABLE_NAME;
END;
$func$ LANGUAGE plpgsql;

-- ── ① Original Sources：团队级来源池 ─────────────────────────────────────────
CREATE TABLE kb.source (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID NOT NULL REFERENCES core.tenant(id),
    source_kind    TEXT NOT NULL CHECK (source_kind IN
                     ('markdown','docx','pdf','meeting_notes','email','im_log',
                      'design','recording','other')),
    title          TEXT NOT NULL CHECK (length(btrim(title)) BETWEEN 1 AND 200),
    -- §7.4 Source Reference：uri（对象存储预签名 / 外部定位）与 inline_content（文本类内联，
    -- 0045 source_input.content 先例）二选一；KB 不持有二进制原文。
    uri            TEXT CHECK (uri IS NULL OR length(btrim(uri)) > 0),
    inline_content TEXT CHECK (inline_content IS NULL OR length(inline_content) > 0),
    mime           TEXT,
    size_bytes     BIGINT CHECK (size_bytes IS NULL OR size_bytes >= 0),
    content_hash   TEXT NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
        -- 小写十六进制 sha256（knowledge::content_hash 同口径）；内联来源由服务端计算，
        -- uri 来源由提交方声明（拉取后重算比对归 Phase 4 解析链）。跨服务对账锚。
    registered_by  UUID NOT NULL REFERENCES core.principal(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((uri IS NULL) <> (inline_content IS NULL)),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, content_hash)
        -- 团队池统一身份（§6.2）：同内容同一来源，二次登记 = 追加项目关联，不建新行。
);

CREATE TRIGGER trg_kb_source_immutable
    BEFORE UPDATE OR DELETE ON kb.source
    FOR EACH ROW EXECUTE FUNCTION kb.reject_immutable_mutation();
    -- 原始资料必须保留，不允许改写或覆盖（定义稿 §5.1）。

-- 项目关联：行存在 = 经人工确认的归属（V0.5 由项目成员登记即确认；AI 归属建议后置）。
CREATE TABLE kb.source_project (
    tenant_id  UUID NOT NULL,
    source_id  UUID NOT NULL,
    project_id UUID NOT NULL,
    linked_by  UUID NOT NULL REFERENCES core.principal(id),
    linked_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (source_id, project_id),
    FOREIGN KEY (tenant_id, source_id)  REFERENCES kb.source (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_kb_source_project_project
    ON kb.source_project (project_id, linked_at DESC);

-- ── ② Structured Memory（L1 Shallow Memory）──────────────────────────────────
-- 记忆身份：稳定 id 供 K4 路径寻址；类型/标题/正文全部在不可变修订上（改变含义须审核）。
CREATE TABLE kb.memory (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id  UUID NOT NULL,
    project_id UUID NOT NULL,
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_kb_memory_project
    ON kb.memory (project_id, created_at DESC);

-- 修订 = 待审核区提案（PENDING_REVIEW）∪ 审核结论（APPROVED / REJECTED）；
-- 仅 APPROVED 修订是「版本」（version_no / version_status 非空，§9.2）。
CREATE TABLE kb.memory_revision (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id        UUID NOT NULL,
    memory_id        UUID NOT NULL,
    revision_no      INT  NOT NULL CHECK (revision_no >= 1),
        -- 提案序（含被驳回者）；版本序见 version_no。
    change_action    TEXT NOT NULL CHECK (change_action IN ('CREATE','UPDATE','WITHDRAW')),
    base_revision_id UUID,
        -- 提案时的 CURRENT 版本（CREATE 为 NULL）；批准时 CAS：仍须是该记忆的 CURRENT。
    memory_type      TEXT NOT NULL CHECK (memory_type IN
                       ('discussion','decision_background','rejected_option',
                        'open_question','business_constraint','controversy')),
        -- 定义稿 §5.3 L1 六类：深入讨论及方案 / 正式决策背景 / 被否决方案及原因 /
        -- 重要未解决问题 / 重要业务约束 / 历史争议。
    title            TEXT NOT NULL CHECK (length(btrim(title)) BETWEEN 1 AND 200),
    summary          TEXT NOT NULL CHECK (length(btrim(summary)) BETWEEN 1 AND 2000),
        -- 注入面摘要（Agent Context 只带 summary，不带 body）。
    body             TEXT,
    change_reason    TEXT NOT NULL CHECK (length(btrim(change_reason)) BETWEEN 1 AND 1000),
    content_hash     TEXT NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
        -- 规范化内容（type/title/summary/body）的 sha256：版本内容锚。
    proposed_by      UUID NOT NULL REFERENCES core.principal(id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 审核面（唯一可变面，单向一次）：K4 approve → APPROVED / reject → REJECTED /
    -- revise → REVISION_REQUESTED（退回修改：本提案关闭，提案人另提新修订）。
    review_status    TEXT NOT NULL DEFAULT 'PENDING_REVIEW' CHECK (review_status IN
                       ('PENDING_REVIEW','APPROVED','REJECTED','REVISION_REQUESTED')),
    reviewed_by      UUID REFERENCES core.principal(id),
    reviewed_at      TIMESTAMPTZ,
    review_note      TEXT CHECK (review_note IS NULL OR length(review_note) <= 1000),
    -- 版本面（§9.2 四态；MERGED 保留值，本批无写入方）。
    version_no       INT CHECK (version_no IS NULL OR version_no >= 1),
    version_status   TEXT CHECK (version_status IS NULL OR version_status IN
                       ('CURRENT','SUPERSEDED','WITHDRAWN','MERGED')),
    UNIQUE (memory_id, revision_no),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, memory_id) REFERENCES kb.memory (tenant_id, id),
    FOREIGN KEY (tenant_id, base_revision_id) REFERENCES kb.memory_revision (tenant_id, id),
    -- 审核结论与审核人/时刻同进同出。
    CHECK ((review_status = 'PENDING_REVIEW') = (reviewed_by IS NULL)),
    CHECK ((review_status = 'PENDING_REVIEW') = (reviewed_at IS NULL)),
    -- L1 审核门：未经人工审核通过的修订永不成为版本。
    CHECK ((review_status = 'APPROVED') = (version_status IS NOT NULL)),
    CHECK ((version_no IS NULL) = (version_status IS NULL)),
    -- CREATE 无基线；UPDATE / WITHDRAW 必须钉住提案时的 CURRENT。
    CHECK ((change_action = 'CREATE') = (base_revision_id IS NULL)),
    -- 撤回结论只能来自撤回提案；新增/修改结论只能是 CURRENT/SUPERSEDED 线。
    CHECK (version_status IS DISTINCT FROM 'WITHDRAWN' OR change_action = 'WITHDRAW'),
    CHECK (change_action <> 'WITHDRAW' OR version_status IS NULL OR version_status = 'WITHDRAWN')
);

CREATE UNIQUE INDEX uq_kb_memory_revision_pending
    ON kb.memory_revision (memory_id) WHERE review_status = 'PENDING_REVIEW';
    -- 至多一条待审修订：新提案须先裁决在途提案（S01-05 单 pending 先例）。
CREATE UNIQUE INDEX uq_kb_memory_revision_current
    ON kb.memory_revision (memory_id) WHERE version_status = 'CURRENT';
CREATE UNIQUE INDEX uq_kb_memory_revision_version_no
    ON kb.memory_revision (memory_id, version_no) WHERE version_no IS NOT NULL;
CREATE INDEX idx_kb_memory_revision_review_queue
    ON kb.memory_revision (tenant_id, created_at) WHERE review_status = 'PENDING_REVIEW';

CREATE FUNCTION kb.guard_memory_revision() RETURNS trigger AS $func$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'kb.memory_revision is append-only (L1 versions are never physically deleted)';
    END IF;
    IF TG_OP = 'INSERT' THEN
        -- 审核门入口：一切内容变化必须先进待审核区。
        IF NEW.review_status <> 'PENDING_REVIEW' OR NEW.version_status IS NOT NULL
           OR NEW.version_no IS NOT NULL THEN
            RAISE EXCEPTION 'kb.memory_revision must be inserted as PENDING_REVIEW without version state (L1 review gate)';
        END IF;
        RETURN NEW;
    END IF;
    -- UPDATE：内容列冻结。
    IF NEW.id <> OLD.id OR NEW.tenant_id <> OLD.tenant_id OR NEW.memory_id <> OLD.memory_id
       OR NEW.revision_no <> OLD.revision_no OR NEW.change_action <> OLD.change_action
       OR NEW.base_revision_id IS DISTINCT FROM OLD.base_revision_id
       OR NEW.memory_type <> OLD.memory_type OR NEW.title <> OLD.title
       OR NEW.summary <> OLD.summary OR NEW.body IS DISTINCT FROM OLD.body
       OR NEW.change_reason <> OLD.change_reason OR NEW.content_hash <> OLD.content_hash
       OR NEW.proposed_by <> OLD.proposed_by OR NEW.created_at <> OLD.created_at THEN
        RAISE EXCEPTION 'kb.memory_revision content is immutable';
    END IF;
    -- 审核结论单向一次。
    IF OLD.review_status <> 'PENDING_REVIEW' AND (
           NEW.review_status <> OLD.review_status
        OR NEW.reviewed_by IS DISTINCT FROM OLD.reviewed_by
        OR NEW.reviewed_at IS DISTINCT FROM OLD.reviewed_at
        OR NEW.review_note IS DISTINCT FROM OLD.review_note) THEN
        RAISE EXCEPTION 'kb.memory_revision review decision is final';
    END IF;
    -- 版本号一经分配不可改。
    IF OLD.version_no IS NOT NULL AND NEW.version_no IS DISTINCT FROM OLD.version_no THEN
        RAISE EXCEPTION 'kb.memory_revision version_no is immutable once assigned';
    END IF;
    -- 版本状态迁移白名单。
    IF NEW.version_status IS DISTINCT FROM OLD.version_status THEN
        IF NOT (
               (OLD.version_status IS NULL
                AND NEW.version_status IN ('CURRENT','WITHDRAWN')
                AND OLD.review_status = 'PENDING_REVIEW'
                AND NEW.review_status = 'APPROVED')
            OR (OLD.version_status = 'CURRENT' AND NEW.version_status = 'SUPERSEDED')
        ) THEN
            RAISE EXCEPTION 'kb.memory_revision illegal version_status transition % -> %',
                coalesce(OLD.version_status, 'NULL'), coalesce(NEW.version_status, 'NULL');
        END IF;
    END IF;
    RETURN NEW;
END;
$func$ LANGUAGE plpgsql;

CREATE TRIGGER trg_kb_memory_revision_guard
    BEFORE INSERT OR UPDATE OR DELETE ON kb.memory_revision
    FOR EACH ROW EXECUTE FUNCTION kb.guard_memory_revision();

-- 来源依据（evidence → L2 原文）与讨论主题（subject → 按 Task 检索锚），属修订内容：不可变。
-- 结构化四元组列（§7.3；禁单 JSONB 混用语义）；domain 值由应用层按注册表校验。
CREATE TABLE kb.memory_ref (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id               UUID NOT NULL,
    project_id              UUID NOT NULL,
        -- 冗余：检索过滤 + 跨项目隔离（与所属记忆同项目，服务层写入时保证）。
    revision_id             UUID NOT NULL,
    ref_role                TEXT NOT NULL CHECK (ref_role IN ('evidence','subject')),
    target_domain           TEXT NOT NULL,
    target_type             TEXT NOT NULL,
    target_id               TEXT NOT NULL,
    target_revision_id      TEXT,
    revision_missing_reason TEXT,
        -- 检查单 R-4：未钉 Revision 必须记录原因（如 no_revision_mechanism）。
    locator                 JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (revision_id, ref_role, target_domain, target_type, target_id),
    FOREIGN KEY (tenant_id, revision_id) REFERENCES kb.memory_revision (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id)  REFERENCES core.project (tenant_id, id),
    CHECK ((target_revision_id IS NULL) = (revision_missing_reason IS NOT NULL))
);

CREATE INDEX idx_kb_memory_ref_subject
    ON kb.memory_ref (project_id, target_domain, target_type, target_id)
    WHERE ref_role = 'subject';
CREATE INDEX idx_kb_memory_ref_revision
    ON kb.memory_ref (revision_id);

CREATE TRIGGER trg_kb_memory_ref_immutable
    BEFORE UPDATE OR DELETE ON kb.memory_ref
    FOR EACH ROW EXECUTE FUNCTION kb.reject_immutable_mutation();
