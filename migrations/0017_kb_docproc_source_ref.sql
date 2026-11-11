-- 0017_kb_docproc_source_ref.sql
--
-- TD-05 §3 KB 接入落地：kb.document_revision 记录 Document Processing 产物
-- 引用（引用不复制——D-05/§2.2），confirm 时按 checksum 复检溯源。
--
--   source_ref        候选来源引用（`docproc:artifact:{uuid}`；NULL = 人工/AI
--                     既有通道，不归属 docproc）
--   source_checksum   引用时的产物 sha256（确认前复检锚点：docproc.artifact
--                     为 append-only 不可变，此列与当前 checksum 不一致 =
--                     引用已失效 → 拒绝确认）

ALTER TABLE kb.document_revision ADD COLUMN source_ref text;
ALTER TABLE kb.document_revision ADD COLUMN source_checksum text;
