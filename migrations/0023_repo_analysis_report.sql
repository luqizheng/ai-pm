-- 0023_repo_analysis_report.sql
--
-- D-5 · 仓库分析报告实体（docs/scenarios/s01/docs/s01-reanalysis-and-product-cognition-closure-v1.md，
-- 2026-11-15 用户裁决）+ D-1 分析锚点列。
--
-- 一次分析运行 = 一份报告，聚合本次全部产物（KB 报告修订 + PC 修订集 + PC 提案，
-- 经 intake_id / kb_revision_id / pc_source_id 关联）。生命周期：
--   analyzing（分析中）→ analyzed（分析完毕，待审理）→ closed（已关闭，门释放）；
--   failed / cancelled 为终态，不占门。
-- 门语义：analyzing/analyzed 期间该仓库禁止再分析（应用层 409 report_open）；
-- 关闭时固化 D-1 锚点（project_repository.last_analyzed_commit_sha / last_analyzed_at）。
--
-- 迁移号：0023 = 当前 migrations/ 目录最大号 0022 + 1（台账 §三 纪律；旧链 0055/0064
-- 为 squash 前历史档案号，不参与取号）。

CREATE TABLE integration.repo_analysis_report (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid NOT NULL REFERENCES core.tenant (id),
    project_id uuid NOT NULL REFERENCES core.project (id),
    repository_id uuid NOT NULL REFERENCES integration.project_repository (id),
    commit_sha text NOT NULL,
    intake_id uuid NOT NULL,
    status text NOT NULL DEFAULT 'analyzing'
        CHECK (status IN ('analyzing', 'analyzed', 'closed', 'failed', 'cancelled')),
    stage_detail text,
    error_code text,
    error_detail text,
    kb_revision_id uuid,
    pc_source_id uuid,
    closed_at timestamp with time zone,
    closed_by uuid,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    updated_at timestamp with time zone NOT NULL DEFAULT now(),
    UNIQUE (intake_id)
);

CREATE INDEX idx_repo_analysis_report_open
    ON integration.repo_analysis_report (repository_id)
    WHERE status IN ('analyzing', 'analyzed');

CREATE INDEX idx_repo_analysis_report_project
    ON integration.repo_analysis_report (project_id, created_at DESC);

-- D-1 · 分析锚点（与 Observed HEAD / Confirmed Commit 正交的第三类 commit 概念）：
-- 报告关闭时固化「已分析到哪」，增量分析（D-2）以此为起点。
ALTER TABLE integration.project_repository
    ADD COLUMN last_analyzed_commit_sha text,
    ADD COLUMN last_analyzed_at timestamp with time zone;

-- 放宽 kb.document_revision 状态守卫：白名单加 draft → rejected（报告关闭时对
-- 未确认的 KB 草稿自动处置）。0006 头注「reject 操作能力后置」在本批兑现；
-- 内容不可变/DELETE 全拒/published 元数据不可变三条纪律原样保留。
CREATE OR REPLACE FUNCTION kb.guard_document_revision() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'kb.document_revision is append-only (delete forbidden)';
    END IF;

    IF NEW.content IS DISTINCT FROM OLD.content
       OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
       OR NEW.document_id IS DISTINCT FROM OLD.document_id
       OR NEW.revision_no IS DISTINCT FROM OLD.revision_no
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.analysis_mode IS DISTINCT FROM OLD.analysis_mode
    THEN
        RAISE EXCEPTION 'kb.document_revision content columns are immutable';
    END IF;

    IF NEW.status IS DISTINCT FROM OLD.status THEN
        IF NOT (   (OLD.status = 'draft' AND NEW.status = 'published')
                OR (OLD.status = 'draft' AND NEW.status = 'rejected')) THEN
            RAISE EXCEPTION 'kb.document_revision illegal status transition % -> %',
                OLD.status, NEW.status;
        END IF;
    END IF;

    IF OLD.status = 'published'
       AND (NEW.confirmed_by IS DISTINCT FROM OLD.confirmed_by
            OR NEW.published_at IS DISTINCT FROM OLD.published_at) THEN
        RAISE EXCEPTION 'kb.document_revision publication metadata is immutable';
    END IF;

    RETURN NEW;
END;
$$;
