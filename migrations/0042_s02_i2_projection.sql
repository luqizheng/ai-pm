-- 0042_s02_i2_projection: S02-I2 需求投影协议的登记面。
--
-- knowledge_write_operation.command 扩第五类 'bind_requirement_projection'
-- （requirement::submit_revision 的投影绑定点，requirement_projection.rs）：
-- 幂等账本沿用既有 (project_id, operation_id) 唯一键，operation_id 约定
-- `req-submit:{revision_id}`。doc_type 'requirement' 已在 0006 四值内，
-- markdown_version_ref/hash 列已随 0039 就位——本迁移是 I2 的唯一登记面。

ALTER TABLE ai.knowledge_write_operation
    DROP CONSTRAINT knowledge_write_operation_command_check;

ALTER TABLE ai.knowledge_write_operation
    ADD CONSTRAINT knowledge_write_operation_command_check CHECK (
        command IN (
            'create_document',
            'submit_proposal',
            'confirm_redline',
            'veto_revision',
            'bind_requirement_projection'
        )
    );
