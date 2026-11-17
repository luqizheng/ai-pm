-- 0032_write_operation_request_fingerprint.sql
--
-- P3-D04-02 Step5 Batch4a GAP-01/27：write_operation 账本扩展（Q-01 方案 A）。
-- ① request_fingerprint 列=请求意图指纹（D6.6 §1：账本行=单一权威指纹；九要素
--    规范化投影，认领即锚定、完成前不可变；发布段=发布确认时的内容锚）。
-- ② command CHECK 收敛增列起草段命令（分析链起草+提交一体账本行）。
-- 两域镜像；纯增量无数据迁移；AGENTS.md 规则 1。

ALTER TABLE pc.write_operation
    ADD COLUMN request_fingerprint text;
ALTER TABLE projc.write_operation
    ADD COLUMN request_fingerprint text;

ALTER TABLE pc.write_operation
    DROP CONSTRAINT write_operation_command_check;
ALTER TABLE pc.write_operation
    ADD CONSTRAINT write_operation_command_check
    CHECK ((command = ANY (ARRAY['pc_commit_object_revision'::text, 'pc_commit_relationship_revision'::text, 'pc_publish_version'::text, 'pc_propose_from_analysis'::text])));

ALTER TABLE projc.write_operation
    DROP CONSTRAINT write_operation_command_check;
ALTER TABLE projc.write_operation
    ADD CONSTRAINT write_operation_command_check
    CHECK ((command = ANY (ARRAY['projc_commit_object_revision'::text, 'projc_publish_version'::text, 'projc_propose_from_analysis'::text])));
