-- 0044: PA07-IMP-004 Run 启动门禁快照（PA-07 V1.0 §4/§5.2；实施规范 §4.3/§4.4/§5）
-- 1) core.product.status 四值化（IMP-001 §5-1 裁决落地；PA-07 V1.0 §4.1 基线）：
--    {active,abandoned} → {normal,suspended,resuming,archived}。
--    abandoned 语义并入 archived（§12：归档不提供常规恢复，restart 语义收敛为
--    suspended/resuming → normal 的再激活）。无存量数据：直接改契约。
-- 2) delivery.task_product：Task→Product 业务服务归属（§4.3，与 Product 门禁
--    检查的技术依赖分开建模）；withdrawn_* 表达有效性历史，撤回可审计。
-- 3) delivery.task_run_dependency_snapshot：Run 领取成功的依赖门禁逐边快照
--    （审查81：前置 Task/Revision/条件/交付物引用/检查结果/检查时间）。
--    快照是历史执行证据，不是第二套依赖权威数据。
-- 4) delivery.task_run.product_gate_snapshot：领取时关联 Product 的执行状态
--    与放行结论（jsonb 证据；当前权威在 core.product.status）。

ALTER TABLE core.product DROP CONSTRAINT ck_product_status;
ALTER TABLE core.product ALTER COLUMN status SET DEFAULT 'normal';
ALTER TABLE core.product
    ADD CONSTRAINT ck_product_status CHECK ((status = ANY (ARRAY['normal'::text, 'suspended'::text, 'resuming'::text, 'archived'::text])));

CREATE TABLE delivery.task_product (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    product_id uuid NOT NULL,
    scope text,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    withdrawn_by uuid,
    withdrawn_at timestamp with time zone,
    CONSTRAINT fk_task_product_task FOREIGN KEY (task_id) REFERENCES delivery.task(id) ON DELETE CASCADE,
    CONSTRAINT fk_task_product_product FOREIGN KEY (product_id) REFERENCES core.product(id),
    CONSTRAINT fk_task_product_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

-- 仅对「有效关联」唯一：撤回（withdrawn_at 非空）后允许重新建立归属（新行或
-- 复活由应用层选择；V1 直接允许并存历史行，读面按 withdrawn_at 区分）。
CREATE UNIQUE INDEX uq_task_product_active
    ON delivery.task_product (task_id, product_id) WHERE withdrawn_at IS NULL;

ALTER TABLE delivery.task_product ADD CONSTRAINT task_product_pkey PRIMARY KEY (id);
CREATE INDEX idx_task_product_task ON delivery.task_product (task_id);
CREATE INDEX idx_task_product_product ON delivery.task_product (product_id);

CREATE TABLE delivery.task_run_dependency_snapshot (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    run_id uuid NOT NULL,
    task_id uuid NOT NULL,
    predecessor_task_id uuid NOT NULL,
    predecessor_revision_id uuid,
    required_condition text NOT NULL,
    required_deliverable_ref jsonb,
    satisfied boolean NOT NULL,
    checked_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT fk_run_dep_snapshot_run FOREIGN KEY (run_id) REFERENCES delivery.task_run(id) ON DELETE CASCADE,
    CONSTRAINT fk_run_dep_snapshot_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE delivery.task_run_dependency_snapshot
    ADD CONSTRAINT task_run_dependency_snapshot_pkey PRIMARY KEY (id);
CREATE INDEX idx_run_dep_snapshot_run ON delivery.task_run_dependency_snapshot (run_id);

ALTER TABLE delivery.task_run ADD COLUMN product_gate_snapshot jsonb;
ALTER TABLE delivery.task_run ADD COLUMN product_gate_snapshot_at timestamp with time zone;

COMMENT ON COLUMN core.product.status IS
    'PA-07 §4.1 执行/业务状态四值：normal/suspended/resuming/archived（0044 四值化，abandoned 并入 archived）。suspended/archived 阻塞受影响开发工作；resuming 按 Portfolio Review 门禁逐步开放。';
COMMENT ON COLUMN delivery.task_product.scope IS
    '服务范围说明（可空）；withdrawn_at 非空=该关联已撤回退出有效范围（历史保留，§6.4 需求撤回）。';
COMMENT ON COLUMN delivery.task_run.product_gate_snapshot IS
    'Run 领取时各关联 Product 的执行状态与放行结论快照（历史证据；权威在 core.product.status）。';
