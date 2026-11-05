-- 0039_s02_requirement_core: S02-I1 需求核心（S02-04 V1.2 §4/§5/§7/§15.5 落库）。
--
-- 迁移号段更正（§17.2 顺移补丁）：文档写「S02 自 0036 起」，但 0036/0037 已被
-- S01-04/05（cognition/baseline）占用、0038 被 S01 仓库基线确认占用——S02 实际
-- 自 0039 起（RSK-4 实施前复核 2026-11-06：migrations 止于 0038）。
--
-- 内容（I1 切片边界；task_requirement 改造归 S02-I4，本迁移不动）：
--   1) delivery.requirement：status→lifecycle_status 三态 + CHECK（S02D-04 前半；
--      旧值 'draft'/'approved' 废弃——「是否有正式版本」由 current_revision_id 表达，
--      无存量数据，直改）；+requirement_type/priority（S02D-13/S02D-05）；
--      +created_by/updated_at/updated_by（归因列不挂 FK——0038 先例，auth-off/
--      种子 principal 可能不入库）。
--   2) delivery.requirement_revision：state CHECK 三值→五值（S02D-04 后半，I1-D03
--      rejected/superseded 终态）；+base_current_revision_id/base_row_revision（CAS
--      双令牌的基版本快照，S02D-01/I1 §7）；+markdown_version_ref（I2 定结构，
--      I1 先留 NULL）+submitted_at（pending 冻结时刻）；DROP source_artifact_id
--      （S02D-17/Q-20：指向相反语义的裸列，0002 以来零读写）。
--      approved_by 过渡保留（I1-D09：读取消费者迁 Confirmation 读模型后另行删列）。
--      content_hash 语义切换 = sha256(markdown)（S02D-16，单点 helper
--      pm-application::requirement）；无存量不回填（AGENTS 规则 1），旧口径哈希
--      仅存在于本地开发库，声明 legacy 不做迁移。
--   3) delivery.requirement_confirmation 新表（S02D-21，DEF-02 修复载体）：一次
--      确认/拒绝行为的不可变事实。一条 Revision 最多一条 confirm、最多一条 reject
--      （拒绝终态不可复活，I1-D03——UNIQUE(revision_id, action) 承载）。

-- ═══════════════════════════════════════════════════════════════════
-- 1) delivery.requirement
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE delivery.requirement RENAME COLUMN status TO lifecycle_status;

-- 防御性归一（无存量；若本地库有手工行，一律按 active 处理）。
UPDATE delivery.requirement SET lifecycle_status = 'active'
WHERE lifecycle_status NOT IN ('active', 'withdrawn', 'archived');

ALTER TABLE delivery.requirement
    ALTER COLUMN lifecycle_status SET DEFAULT 'active',
    ADD CONSTRAINT chk_requirement_lifecycle
        CHECK (lifecycle_status IN ('active', 'withdrawn', 'archived')),
    ADD COLUMN requirement_type TEXT NOT NULL DEFAULT 'functional',
    ADD CONSTRAINT chk_requirement_type
        CHECK (requirement_type IN ('functional', 'non_functional')),
    ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal',
    ADD CONSTRAINT chk_requirement_priority
        CHECK (priority IN ('low', 'normal', 'high')),
    ADD COLUMN created_by UUID,
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN updated_by UUID;

-- ═══════════════════════════════════════════════════════════════════
-- 2) delivery.requirement_revision
-- ═══════════════════════════════════════════════════════════════════

ALTER TABLE delivery.requirement_revision
    DROP CONSTRAINT requirement_revision_state_check;

ALTER TABLE delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_state_check CHECK (
        state IN ('draft', 'pending_confirmation', 'approved', 'rejected', 'superseded')
    ),
    ADD COLUMN base_current_revision_id UUID,
    ADD COLUMN base_row_revision BIGINT,
    ADD COLUMN markdown_version_ref TEXT,
    ADD COLUMN submitted_at TIMESTAMPTZ;

ALTER TABLE delivery.requirement_revision DROP COLUMN source_artifact_id;

-- ═══════════════════════════════════════════════════════════════════
-- 3) delivery.requirement_confirmation（S02D-21）
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE delivery.requirement_confirmation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    requirement_id UUID NOT NULL,
    revision_id UUID NOT NULL,
    markdown_version_ref TEXT,
    content_hash TEXT,
    human_principal_id UUID NOT NULL,
    identity_id UUID,
    action TEXT NOT NULL CHECK (action IN ('confirm', 'reject')),
    reason TEXT,
    source TEXT NOT NULL DEFAULT 'rest',
    audit_event_id UUID,
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    tenant_id UUID NOT NULL,
    FOREIGN KEY (project_id, requirement_id)
        REFERENCES delivery.requirement(project_id, id),
    FOREIGN KEY (requirement_id, revision_id)
        REFERENCES delivery.requirement_revision(requirement_id, id),
    -- 一条 Revision 最多一条 confirm、最多一条 reject（拒绝终态不可复活）。
    CONSTRAINT uq_requirement_confirmation_revision_action UNIQUE (revision_id, action)
);

CREATE INDEX requirement_confirmation_tenant_idx
    ON delivery.requirement_confirmation (tenant_id, requirement_id, confirmed_at);
