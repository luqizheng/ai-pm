-- 0009_pc_revision_fts.sql
--
-- WP-3 · PC 域检索索引设施（检索架构 V1.0 任务书 §四 Q-D）。
--
-- **PC 不建章节表、不建 state 表**（裁决 Q-D 确认）：PC 正文（
-- pc.document_revision.content）与修订行一一对应，没有 KB 那种解析后拆成多个
-- 章节行的一对多派生关系——KB/PC 结构差异是**有意设计**，不是实现不一致。
--
-- 有效性兜底（裁决 Q-D 补充约束）：检索通道只经由
-- `pc.cognition_object.current_revision_id → cognition_revision → document_revision`
-- 链到达正文行——**只有现行内容行可达**，陈旧 tsv 在结构上不可见于检索结果；
-- tsv 更新失败由消费重试与 `rebuild_kb_index` 全量对账发现。
--
-- 术语同 0008：这是 PostgreSQL FTS，不是 BM25。

ALTER TABLE pc.document_revision
    ADD COLUMN search_tsv tsvector;

CREATE INDEX idx_pc_revision_tsv
    ON pc.document_revision USING gin (search_tsv);

-- ── 触发器置换：全拒 → 精细守卫（实施发现 F-3，与 0006 同款纪律）────────────
--
-- **发现**：`trg_pc_document_revision_immutable` 是 `BEFORE DELETE OR UPDATE`
-- 全拒（`pc.forbid_row_update()`）——tsv 派生列无法落表。与 kb 侧 0006 同一
-- 冻结模式、同一自述（"drop trigger to migrate"）。
--
-- **处置**：置换为精细守卫——DELETE 禁止；**内容列不可变**（content /
-- content_hash / document_id / revision_no / tenant_id / project_id / created_by）；
-- **派生列 `search_tsv` 允许更新**（索引管线唯一的写入口；不触碰知识正文）。

DROP TRIGGER trg_pc_document_revision_immutable ON pc.document_revision;

CREATE FUNCTION pc.guard_document_revision() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'pc.document_revision is append-only (delete forbidden)';
    END IF;

    IF NEW.content IS DISTINCT FROM OLD.content
       OR NEW.content_hash IS DISTINCT FROM OLD.content_hash
       OR NEW.document_id IS DISTINCT FROM OLD.document_id
       OR NEW.revision_no IS DISTINCT FROM OLD.revision_no
       OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
       OR NEW.project_id IS DISTINCT FROM OLD.project_id
       OR NEW.created_by IS DISTINCT FROM OLD.created_by
    THEN
        RAISE EXCEPTION 'pc.document_revision content columns are immutable';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_pc_document_revision_guard
    BEFORE UPDATE OR DELETE ON pc.document_revision
    FOR EACH ROW EXECUTE FUNCTION pc.guard_document_revision();
