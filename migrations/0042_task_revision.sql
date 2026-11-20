-- 0042: PA07-IMP-002 Task Revision 不可变快照（PA-07 V1.0 §3.1；实施规范 §4.2）
-- 1) delivery.task_revision：目标/范围/验收条件/执行契约的不可变版本行。
--    delivery.task.revision（CAS 整数）只是并发控制版本号，不是 Revision 历史；
--    本表补齐「每次 Run 可追溯精确执行契约」的权威快照，创建后不得原位更新。
-- 2) delivery.task.current_task_revision_id：当前生效指针（与 revision CAS 同事务推进）。
-- 3) delivery.task_run.task_revision_id：Run 固定引用启动时生效的 Revision。
-- 快照列映射：goal=task.goal；scope=task.constraints；acceptance_criteria=
-- task_acceptance_criterion 行集冻结；execution_contract=deliverables/task_type/
-- git_branch/milestone_id/ai_role_version_id 等执行契约冻结。
-- 无存量数据：直接建表加列，不回填（AGENTS.md 规则 1）。

CREATE TABLE delivery.task_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    revision_no bigint NOT NULL,
    goal text,
    scope jsonb DEFAULT '{}'::jsonb NOT NULL,
    acceptance_criteria jsonb DEFAULT '[]'::jsonb NOT NULL,
    execution_contract jsonb DEFAULT '{}'::jsonb NOT NULL,
    change_reason text,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT fk_task_revision_task FOREIGN KEY (task_id) REFERENCES delivery.task(id) ON DELETE CASCADE,
    CONSTRAINT fk_task_revision_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE delivery.task_revision ADD CONSTRAINT task_revision_pkey PRIMARY KEY (id);

-- 同一 Task 的 Revision 号唯一；revision_no 与 delivery.task.revision 同源
-- （创建任务=1，每次实质变更 +1，指针与 CAS 同事务推进）。
ALTER TABLE delivery.task_revision
    ADD CONSTRAINT uq_task_revision_task_no UNIQUE (task_id, revision_no);

CREATE INDEX idx_task_revision_task ON delivery.task_revision (task_id);

ALTER TABLE delivery.task ADD COLUMN current_task_revision_id uuid;

ALTER TABLE delivery.task
    ADD CONSTRAINT fk_task_current_revision
    FOREIGN KEY (current_task_revision_id) REFERENCES delivery.task_revision(id);

ALTER TABLE delivery.task_run ADD COLUMN task_revision_id uuid;

ALTER TABLE delivery.task_run
    ADD CONSTRAINT fk_task_run_task_revision
    FOREIGN KEY (task_revision_id) REFERENCES delivery.task_revision(id);

CREATE INDEX idx_task_run_task_revision ON delivery.task_run (task_revision_id);

COMMENT ON COLUMN delivery.task_revision.revision_no IS
    '快照创建时的 delivery.task.revision 纪元：实质变更（goal/scope/验收条件/执行契约）产生快照；状态迁移也推进 task.revision 但不产生快照行，故快照号允许有间隙。';
COMMENT ON COLUMN delivery.task.current_task_revision_id IS
    '当前生效的不可变 Revision 指针；与 revision CAS 在同一事务内推进（PA07-IMP-002）。';
COMMENT ON COLUMN delivery.task_run.task_revision_id IS
    'Run 启动时固定的 Task Revision 引用；解释历史执行依据，不随任务后续变更改变（PA-07 审查81）。';
