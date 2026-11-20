-- 0045: PA07-IMP-005 Product 维度验收表达（PA-07 V1.0 §13；实施规范 §4.6）。
-- V1 不建独立 Product Delivery 子系统：一张最小验收记录表，多 Product Task 的
-- 共同/专属验收条件独立保存（A 通过不代表 B 通过）；历史保留不覆盖（追加式，
-- 判定取各 Product 最新一条）。撤回/归档 Product 的专属范围经正式处理退出
-- 有效范围后不再阻塞 Completed（task_product.withdrawn_at IS NULL 过滤），
-- 历史验收保留。
-- Task Completed 与最后一条必要验收记录的原子一致由 apply_task_transition
-- 内的验收门禁执法（单点：review complete_task / D2 命令通道 / MCP 全部经过）。
-- 无存量数据：直接建表（AGENTS.md 规则 1）。

CREATE TABLE delivery.task_product_acceptance (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    task_revision_id uuid,
    product_id uuid NOT NULL,
    result text NOT NULL,
    condition_scope text NOT NULL DEFAULT 'product_specific',
    evidence_ref text,
    accepted_by uuid,
    source text NOT NULL DEFAULT 'human',
    note text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_task_product_acceptance_result CHECK (
        result = ANY (ARRAY['approved'::text, 'rejected'::text])
    ),
    CONSTRAINT ck_task_product_acceptance_scope CHECK (
        condition_scope = ANY (ARRAY['common'::text, 'product_specific'::text])
    ),
    CONSTRAINT ck_task_product_acceptance_source CHECK (
        source = ANY (ARRAY['human'::text, 'platform_flow'::text])
    ),
    CONSTRAINT fk_task_product_acceptance_task FOREIGN KEY (task_id) REFERENCES delivery.task(id) ON DELETE CASCADE,
    CONSTRAINT fk_task_product_acceptance_product FOREIGN KEY (product_id) REFERENCES core.product(id),
    CONSTRAINT fk_task_product_acceptance_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE delivery.task_product_acceptance
    ADD CONSTRAINT task_product_acceptance_pkey PRIMARY KEY (id);
CREATE INDEX idx_task_product_acceptance_task ON delivery.task_product_acceptance (task_id);
CREATE INDEX idx_task_product_acceptance_product ON delivery.task_product_acceptance (product_id);

COMMENT ON TABLE delivery.task_product_acceptance IS
    'Product 级验收记录（PA-07 §13，审查89-90）：追加式历史，不覆盖已通过记录；整体 Completed 门禁取每个有效关联 Product 的最新一条。';
