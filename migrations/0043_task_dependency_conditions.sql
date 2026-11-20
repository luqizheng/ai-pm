-- 0043: PA07-IMP-003 Task Dependency 条件列与变更审计（PA-07 V1.0 审查75/81/82；
-- 实施规范 §4.4）。
-- 1) required_condition：依赖边满足条件，三值封闭枚举（审查75，不支持表达式）：
--    completed（前置正式完成）/ accepted（前置交付已通过验收审批）/
--    deliverable_approved（指定交付物已随获批交付）。
-- 2) required_deliverable_ref：deliverable_approved 必须引用明确交付物
--    （jsonb，含 id 或 path；按前置当前 Revision 快照的 deliverables 匹配）。
-- 3) updated_at / updated_by：边修改审计（创建审计已有 created_*）。
-- 语义变更（PA-07 §9.5）：cancelled/superseded 前置不再视为满足——后继保持
-- 阻塞等待恢复或调整，判定逻辑由状态集合改为按边条件求值。
-- 无存量数据：直接加列加约束（AGENTS.md 规则 1）。

ALTER TABLE delivery.task_dependency
    ADD COLUMN required_condition text DEFAULT 'completed'::text NOT NULL,
    ADD COLUMN required_deliverable_ref jsonb,
    ADD COLUMN updated_at timestamp with time zone DEFAULT now() NOT NULL,
    ADD COLUMN updated_by uuid;

ALTER TABLE delivery.task_dependency
    ADD CONSTRAINT ck_task_dependency_condition CHECK (
        required_condition = ANY (ARRAY['completed'::text, 'accepted'::text, 'deliverable_approved'::text])
    );

ALTER TABLE delivery.task_dependency
    ADD CONSTRAINT ck_task_dependency_deliverable_ref CHECK (
        required_condition <> 'deliverable_approved'::text
        OR (required_deliverable_ref IS NOT NULL
            AND (required_deliverable_ref ? 'id'
                 OR required_deliverable_ref ? 'path'))
    );

COMMENT ON COLUMN delivery.task_dependency.required_condition IS
    '依赖边满足条件（PA-07 审查75）：completed=前置完成；accepted=前置交付获批（governance.review_approval approved）；deliverable_approved=指定交付物随获批交付（按前置当前 Revision 冻结的 deliverables 匹配）。';
COMMENT ON COLUMN delivery.task_dependency.required_deliverable_ref IS
    'deliverable_approved 必填：{id} 或 {path}，按前置 task_revision.execution_contract.deliverables 包含匹配；不得隐式跟随最新版本（审查81）。';
