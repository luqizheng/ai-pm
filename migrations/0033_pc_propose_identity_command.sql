-- 0033_pc_propose_identity_command.sql
--
-- P3-D04-02 Step5 Batch4b GAP-02：pc.write_operation command CHECK 增列身份链
-- 起草段命令（D6.6 §4 身份链 op_id=`{pc}-identity:{round_id}:{slot}`；
-- 1 round→≤2 PC 提案，slot∈{profile,vision}；ProjC 无身份综合链=域差异第 7 项）。
-- request_fingerprint 列已随 0032；纯增量；AGENTS.md 规则 1。

ALTER TABLE pc.write_operation
    DROP CONSTRAINT write_operation_command_check;
ALTER TABLE pc.write_operation
    ADD CONSTRAINT write_operation_command_check
    CHECK ((command = ANY (ARRAY['pc_commit_object_revision'::text, 'pc_commit_relationship_revision'::text, 'pc_publish_version'::text, 'pc_propose_from_analysis'::text, 'pc_propose_identity'::text])));
