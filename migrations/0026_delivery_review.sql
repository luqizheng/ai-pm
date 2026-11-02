-- 0026_delivery_review: Review / 失败判定 / 知识回写的数据模型（Slice D5 / T-043）
-- 依据：docs/ai-pm-slice-d5-review-v1.md §2（V1.3）、§3.2 §3.6、§5.2.2
-- 上位依据：docs/ai-pm-task-state-machine-v1.md §十八 Q8 / §十九 Q9；
--           docs/ai-pm-slice-d4-delivery-v1.md §9.4（D5 口径边界）。
-- 迁移号：0024 归 T-060、0025 留缓冲、T-083 顺移（见 docs/ai-pm-migration-ledger.md）。
--
-- 原则：新项目、无存量数据（AGENTS.md 规则 1）——直接改契约，不写回填脚本。

-- ═══════════════════════════════════════════════════════════════════
-- 1) delivery.task_run 追加六列（run 行即 Review 报告头）
-- ═══════════════════════════════════════════════════════════════════
-- run_kind：'delivery' = D1/D3 执行 Run；'review' = D5 平台内确定性 Reviewer Run。
-- 两条恢复链（执行重试 vs 审查重试）各自恒定，互不越界（§5.4 不变量 4）。

ALTER TABLE delivery.task_run
    ADD COLUMN run_kind               TEXT NOT NULL DEFAULT 'delivery'
        CHECK (run_kind IN ('delivery', 'review')),
    ADD COLUMN role_id                TEXT,
    ADD COLUMN input_manifest_hash    TEXT,
    ADD COLUMN reviewed_manifest_hash TEXT,
    ADD COLUMN review_policy_version  TEXT,
    ADD COLUMN review_verdict         TEXT
        CHECK (review_verdict IN ('approved', 'changes_requested', 'inconclusive'));

-- Q8.5 语义约束：review 行必须带角色与策略版本，否则「以什么口径审的」无从追溯。
ALTER TABLE delivery.task_run
    ADD CONSTRAINT chk_review_role_present
        CHECK (run_kind <> 'review'
               OR (role_id IS NOT NULL AND review_policy_version IS NOT NULL));

-- 〔评审收编 P0-1〕executor_kind='platform' 与 Codex Executor 的库层隔离：
-- review run 只做确定性校验，不碰工作区，凭据面列一律 NULL。
ALTER TABLE delivery.task_run
    ADD CONSTRAINT chk_review_platform_isolation CHECK (
        run_kind <> 'review' OR (
            executor_kind = 'platform'
            AND executor_session IS NULL
            AND lease_token_hash IS NULL
            AND lease_generation = 0
            AND deadline_at IS NULL
            AND last_agent_heartbeat_at IS NULL
            AND stage IS NULL          -- review run 不走 D3 启动协议，不落 stage
        )
    ),
    ADD CONSTRAINT chk_delivery_executor_kind CHECK (
        run_kind <> 'delivery' OR executor_kind IS NULL OR executor_kind <> 'platform'
    );

COMMENT ON COLUMN delivery.task_run.run_kind IS
    'Run 种类：delivery=D1/D3 执行 Run；review=D5 平台内确定性 Reviewer Run。两条恢复链各自恒定。';
COMMENT ON COLUMN delivery.task_run.review_verdict IS
    'Review 结论三值（approved/changes_requested/inconclusive）。四值中的「审查运行本身失败」由 run status=failed 表达，不重复存。';

-- ═══════════════════════════════════════════════════════════════════
-- 2) attempt_no 序号按 kind 分轨 + 活跃 Run 索引重建
-- ═══════════════════════════════════════════════════════════════════
-- attempt_no 原为表级 UNIQUE(task_id, attempt_no)，两种 kind 共用一条序号：
-- review run 的第 N 次重试会与 delivery run 的第 N 次尝试撞号。
-- 改为按 kind 分轨的部分唯一索引——两条恢复链的「第几次」互不干扰。
ALTER TABLE delivery.task_run
    DROP CONSTRAINT IF EXISTS task_run_task_id_attempt_no_key;
CREATE UNIQUE INDEX IF NOT EXISTS uq_task_run_attempt_delivery
    ON delivery.task_run (task_id, attempt_no)
    WHERE run_kind = 'delivery';
CREATE UNIQUE INDEX IF NOT EXISTS uq_task_run_attempt_review
    ON delivery.task_run (task_id, attempt_no)
    WHERE run_kind = 'review';

-- 旧约束收窄：活跃 Run 判定仅对 delivery 生效（否则 review run 活跃期间
-- 挡住任何 delivery run 创建——ChangesRequested 重试链被 409 卡死的反例）。
DROP INDEX IF EXISTS delivery.uq_task_run_active;
CREATE UNIQUE INDEX uq_task_run_active ON delivery.task_run (task_id)
    WHERE run_kind = 'delivery'
      AND status IN ('queued', 'claimed', 'running');

-- 对偶约束：同一 task 至多一个活跃 review run（§3.6 R1）。
CREATE UNIQUE INDEX uq_task_review_active ON delivery.task_run (task_id)
    WHERE run_kind = 'review'
      AND status IN ('queued', 'running');

-- ═══════════════════════════════════════════════════════════════════
-- 2b) 交付对账视图收窄：latest_run 只看 delivery run
-- ═══════════════════════════════════════════════════════════════════
-- 0022 的视图早于 `run_kind`：若最新行是 review run，"最新 Run" 会被审的行取代，
-- 视图随即报 `never_submitted`——交付对账（D4）被审查编排（D5）污染。视图重建，
-- 两类 Run 的语义边界落在查询结构上，不靠调用方过滤。

CREATE OR REPLACE VIEW delivery.v_task_delivery_manifest AS
WITH contract AS (
    SELECT tr.task_id,
           tr.project_id,
           tr.repository_id,
           tr.write_access,
           pr.name AS repository_name
      FROM delivery.task_repository tr
      JOIN integration.project_repository pr
        ON pr.id = tr.repository_id AND pr.project_id = tr.project_id
),
latest_run AS (
    SELECT DISTINCT ON (r.task_id)
           r.task_id, r.id AS run_id, r.attempt_no, r.integration_status
      FROM delivery.task_run r
     WHERE r.run_kind = 'delivery'
     ORDER BY r.task_id, r.attempt_no DESC, r.created_at DESC
),
latest_submitted_run AS (
    SELECT DISTINCT ON (r.task_id)
           r.task_id, c.task_run_id AS run_id
      FROM delivery.task_repository_commit c
      JOIN delivery.task_run r ON r.id = c.task_run_id
     ORDER BY r.task_id, r.attempt_no DESC, c.created_at DESC, c.id DESC
),
counts AS (
    SELECT c.task_id,
           count(*) FILTER (WHERE c.write_access)::int AS expected_delivery_count,
           count(*) FILTER (
               WHERE c.write_access
                 AND lr.run_id IS NOT NULL
                 AND EXISTS (SELECT 1 FROM delivery.task_repository_commit cr
                             WHERE cr.task_run_id = lr.run_id
                               AND cr.repository_id = c.repository_id)
           )::int AS actual_delivery_count
      FROM contract c
      LEFT JOIN latest_run lr ON lr.task_id = c.task_id
     GROUP BY c.task_id
)
SELECT c.task_id,
       c.project_id,
       c.repository_id,
       c.repository_name,
       c.write_access,
       c.write_access AS delivery_required,
       lr.run_id                     AS latest_run_id,
       lr.attempt_no                 AS latest_attempt_no,
       lr.integration_status,
       lsr.run_id                    AS latest_submitted_run_id,
       trr.base_commit,
       cr.commit_sha,
       cr.changed_files,
       cr.test_evidence,
       cr.branch,
       cr.commit_author,
       cr.commit_authored_at,
       cnt.expected_delivery_count,
       cnt.actual_delivery_count,
       CASE
           WHEN cnt.expected_delivery_count = 0 THEN 'delivered'
           WHEN cnt.actual_delivery_count = 0 THEN 'never_submitted'
           WHEN cnt.actual_delivery_count = cnt.expected_delivery_count THEN 'delivered'
           ELSE 'partial'
       END AS delivery_state
  FROM contract c
  LEFT JOIN latest_run lr ON lr.task_id = c.task_id
  LEFT JOIN latest_submitted_run lsr ON lsr.task_id = c.task_id
  LEFT JOIN delivery.task_run_repository trr
         ON trr.run_id = lr.run_id AND trr.repository_id = c.repository_id
  LEFT JOIN delivery.task_repository_commit cr
         ON cr.task_run_id = lr.run_id AND cr.repository_id = c.repository_id
  JOIN counts cnt ON cnt.task_id = c.task_id;

-- ═══════════════════════════════════════════════════════════════════
-- 3) delivery.review_finding：结构化发现（Q8.2）
-- ═══════════════════════════════════════════════════════════════════

CREATE TABLE delivery.review_finding (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    project_id      UUID NOT NULL,
    review_run_id   UUID NOT NULL,
    criterion_id    UUID,               -- 可空：非条目类 finding（如交付对账失败）
    finding_kind    TEXT NOT NULL,
    severity        TEXT NOT NULL CHECK (severity IN ('info', 'warning', 'blocker')),
    code_location   TEXT,               -- 「仓 + 路径 + 行段」自由文本（Q8.2 定位要素）
    message         TEXT NOT NULL,
    evidence_ref    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (project_id, review_run_id)
        REFERENCES delivery.task_run (project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, criterion_id)
        REFERENCES delivery.task_acceptance_criterion (project_id, id)
);
CREATE INDEX idx_review_finding_run ON delivery.review_finding (review_run_id);
CREATE INDEX idx_review_finding_criterion ON delivery.review_finding (criterion_id)
    WHERE criterion_id IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════
-- 4) delivery.acceptance_criterion_verdict：逐条验收裁决（兑现 0015 预留外键）
-- ═══════════════════════════════════════════════════════════════════
-- 「结论 ≠ 证据」的库层表达：verdict 列与 evidence JSONB 分列。

CREATE TABLE delivery.acceptance_criterion_verdict (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    project_id      UUID NOT NULL,
    criterion_id    UUID NOT NULL,
    review_run_id   UUID NOT NULL,
    verdict         TEXT NOT NULL
        CHECK (verdict IN ('passed', 'failed', 'not_evaluable', 'awaiting_manual')),
    evidence        JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(evidence) = 'object'),
    decided_by      UUID REFERENCES core.principal(id),  -- 人工裁决回填
    decided_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (criterion_id, review_run_id),   -- 重审 = 新 review run 新行，历史不可变
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- 0015 注释预留的组合外键位，就地兑现：
    FOREIGN KEY (project_id, criterion_id)
        REFERENCES delivery.task_acceptance_criterion (project_id, id),
    FOREIGN KEY (project_id, review_run_id)
        REFERENCES delivery.task_run (project_id, id) ON DELETE CASCADE
);
CREATE INDEX idx_criterion_verdict_criterion
    ON delivery.acceptance_criterion_verdict (criterion_id);

-- ═══════════════════════════════════════════════════════════════════
-- 5) governance.review_approval：审批独立记录（Q-D5-4 从属裁定 2）
-- ═══════════════════════════════════════════════════════════════════
-- 审批三重绑定：具体 Review + 具体 Delivery + 交付快照（防旧交付审批被继承）。

CREATE TABLE governance.review_approval (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       UUID NOT NULL,
    project_id      UUID NOT NULL,
    task_id         UUID NOT NULL,
    review_run_id   UUID NOT NULL,
    delivery_run_id         UUID NOT NULL,
    delivery_manifest_hash  TEXT NOT NULL,   -- = 被审 delivery run 的 reviewed_manifest_hash
    kind            TEXT NOT NULL
        CHECK (kind IN ('auto_accept_gate', 'manual_review', 'review_degraded')),
    status          TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected')),
    required_role   TEXT,
    decided_by      UUID REFERENCES core.principal(id),
    decision_reason TEXT,
    decided_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task (project_id, id),
    FOREIGN KEY (project_id, review_run_id)
        REFERENCES delivery.task_run (project_id, id) ON DELETE CASCADE,
    -- 不带 CASCADE：审批不得随 run 删除连带消失。
    FOREIGN KEY (project_id, delivery_run_id)
        REFERENCES delivery.task_run (project_id, id)
);
-- 同一 task 同时刻至多一个待决审批（§3.6 I5）。
CREATE UNIQUE INDEX uq_review_approval_pending ON governance.review_approval (task_id)
    WHERE status = 'pending';

-- ═══════════════════════════════════════════════════════════════════
-- 6) 历史不可变约束〔评审收编 P1-2〕
-- ═══════════════════════════════════════════════════════════════════

CREATE FUNCTION delivery.forbid_row_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'row % in %.% is immutable',
        OLD.id, TG_TABLE_SCHEMA, TG_TABLE_NAME;
END $$;

-- finding / verdict 两表全表禁 UPDATE（append-only：重审 = 新 run 新行）。
CREATE TRIGGER trg_review_finding_immutable BEFORE UPDATE ON delivery.review_finding
    FOR EACH ROW EXECUTE FUNCTION delivery.forbid_row_update();
CREATE TRIGGER trg_criterion_verdict_immutable BEFORE UPDATE ON delivery.acceptance_criterion_verdict
    FOR EACH ROW EXECUTE FUNCTION delivery.forbid_row_update();

-- 审批行不是 append-only——裁决本身必须能落终态（§5.2 步骤 4）。但**锚定三元组
-- once-written**：被审的 run / 交付快照一旦写入不可改（§5.2.2 I4），且状态只能
-- 从 pending 单向落到 approved / rejected。
CREATE FUNCTION delivery.forbid_review_approval_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (OLD.task_id IS DISTINCT FROM NEW.task_id)
       OR (OLD.review_run_id IS DISTINCT FROM NEW.review_run_id)
       OR (OLD.delivery_run_id IS DISTINCT FROM NEW.delivery_run_id)
       OR (OLD.delivery_manifest_hash IS DISTINCT FROM NEW.delivery_manifest_hash)
       OR (OLD.kind IS DISTINCT FROM NEW.kind)
       OR (OLD.required_role IS DISTINCT FROM NEW.required_role)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'review approval % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'pending' THEN
        IF NEW.status NOT IN ('approved', 'rejected') THEN
            RAISE EXCEPTION 'review approval % may only move from pending to a terminal status', OLD.id;
        END IF;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'review approval % is already decided and cannot change status', OLD.id;
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER trg_review_approval_immutable BEFORE UPDATE ON governance.review_approval
    FOR EACH ROW EXECUTE FUNCTION delivery.forbid_review_approval_mutation();

-- task_run 侧：review 报告列创建后一次成型（§2.5）。用 OLD.run_kind 判定并把
-- run_kind 一并纳入保护列——否则把 review 行改标成 delivery 即可绕过触发器。
CREATE FUNCTION delivery.forbid_review_report_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.run_kind = 'review' THEN
        IF (NEW.run_kind IS DISTINCT FROM OLD.run_kind)
           OR (OLD.role_id IS DISTINCT FROM NEW.role_id)
           OR (OLD.input_manifest_hash IS DISTINCT FROM NEW.input_manifest_hash)
           OR (OLD.reviewed_manifest_hash IS DISTINCT FROM NEW.reviewed_manifest_hash)
           OR (OLD.review_policy_version IS DISTINCT FROM NEW.review_policy_version)
           OR (OLD.review_verdict IS NOT NULL
               AND OLD.review_verdict IS DISTINCT FROM NEW.review_verdict) THEN
            RAISE EXCEPTION 'review report columns of run % are immutable', OLD.id;
        END IF;
    END IF;
    RETURN NEW;
END $$;

CREATE TRIGGER trg_review_report_immutable BEFORE UPDATE ON delivery.task_run
    FOR EACH ROW EXECUTE FUNCTION delivery.forbid_review_report_update();
