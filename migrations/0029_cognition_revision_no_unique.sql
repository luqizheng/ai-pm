-- 0029_cognition_revision_no_unique.sql
--
-- P3-D04-02 Step5 Batch1 GAP-04：两域 cognition_revision 补 (object_id, revision_no)
-- 唯一约束（DB 层 revision_no 兜底；行锁推号在位不变）。
-- 同款先例：ai/kb document_revision、kb.memory_revision
--   （0001:3722/5210 UNIQUE(document_id, revision_no) 同构）。
-- NOT NULL 核实（V1.1）：两域 object_id/revision_no 均 NOT NULL
--   （0001/0025 建表文本＋活库 information_schema 双证）——无需增补列约束。
-- 迁移前检查：两库两域 (object_id,revision_no) 重复分组=0
--   （2026-11-17 两度只读预检；实施日须复跑，非 0 即停止并报告）。
-- 实施日复跑记录（2026-11-17）：dev 库 aipm（链 0001→0028 重建后）dup=0/0；
--   aipm_mig_full（0001→0027 全链验证库）dup=0/0（projc 2 行非空验证）。
-- AGENTS.md 规则 1：无数据迁移、无回填、无兼容分支。

ALTER TABLE pc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_id_revision_no_key
    UNIQUE (object_id, revision_no);

ALTER TABLE projc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_id_revision_no_key
    UNIQUE (object_id, revision_no);
