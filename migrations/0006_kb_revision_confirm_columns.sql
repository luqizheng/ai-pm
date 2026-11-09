-- 0006_kb_revision_confirm_columns.sql
--
-- WP-0 · 内容管线接缝（KB 确认/发布通道）——检索架构 V1.0 任务书 §2.1。
--
-- 背景：`kb.document_revision` 此前只有 draft 落库（0053 digest 半层），**全仓无
-- draft→published 转换代码**（评审 F1）。没有这扇门，文档进不了「现行」态，
-- 检索索引管线（WP-3）无事件可消费，整条检索链是空的。
--
-- 本迁移只做**列与约束**准备，不含任何状态机语义变更：
-- - 库层既有枚举仍是 `draft / published / rejected`（baseline CHECK 不动）；
-- - `approved` / `pending_confirmation` / `superseded` 等词表属于 **kb.memory_revision**
--   （KB 记忆域审核面）与旧 ai 域的文档修订表（该域已冻结），**不是本表**——本次不
--   引入新状态词，不做 approved↔published 映射（WP-0 裁决 §四核对结论）。
-- - `rejected` 枚举原样保留（本批只交付 draft→published 通道，reject 操作能力后置，
--   生命周期语义不删——WP-0 裁决红线六之六）。
--
-- 约束语义（双向等价式）：
--   要么完整 published 态（confirmed_by + published_at 均非空），
--   要么完全未确认（两列均空）——禁止「已发布但无确认人」与「草稿却已确认」。
-- 存量行全为 draft + 两列 NULL，满足后者，迁移安全。

ALTER TABLE kb.document_revision
    ADD COLUMN confirmed_by uuid REFERENCES core.principal (id),
    ADD COLUMN published_at timestamp with time zone;

ALTER TABLE kb.document_revision
    ADD CONSTRAINT document_revision_confirm_consistency_check
    CHECK (
        (status = 'published' AND confirmed_by IS NOT NULL AND published_at IS NOT NULL)
        OR (status <> 'published' AND confirmed_by IS NULL AND published_at IS NULL)
    );

-- ── 触发器置换：全拒 append-only → 精细守卫（实施中发现的事实修正）─────────
--
-- **发现**：baseline 里 `trg_kb_revision_append_only` 是 `BEFORE DELETE OR UPDATE`
-- 全拒（`kb.reject_revision_mutation()`），**连 status 列都不可改**。即：
-- 状态枚举虽含 `published`，库层却从未实现 draft→published 转换通道——draft 行
-- 永远是死数据（与评审 F1「无 published 转换代码」互为印证：不是缺代码，是缺通道）。
--
-- **同域先例**：KB 记忆域的 `kb.memory_revision` 用的是**精细守卫**
-- `kb.guard_memory_revision()`——允许白名单状态转换（PENDING_REVIEW→APPROVED、
-- CURRENT→SUPERSEDED）、拒绝非法转换。文档修订采用同款纪律：**内容不可变，
-- 状态受控推进**。
--
-- 触发器自述 "drop trigger to migrate"——它本就是「转换通道尚未实现时的冻结态」。
-- 本迁移按该提示正式置换。
--
-- 守卫四纪律：
--   ① DELETE 一律拒绝（修订不可物理删除）；
--   ② 内容列不可改（内容寻址语义：content / content_hash / document_id /
--      revision_no / tenant_id / analysis_mode）；
--   ③ 状态转换白名单：仅 draft→published 一次（reject 通道后置，本批不实现）；
--   ④ published 后确认人/确认时间不可改——**幂等重确认的库层保证**。

DROP TRIGGER trg_kb_revision_append_only ON kb.document_revision;

CREATE FUNCTION kb.guard_document_revision() RETURNS trigger
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
        IF NOT (OLD.status = 'draft' AND NEW.status = 'published') THEN
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

CREATE TRIGGER trg_kb_revision_guard
    BEFORE UPDATE OR DELETE ON kb.document_revision
    FOR EACH ROW EXECUTE FUNCTION kb.guard_document_revision();
