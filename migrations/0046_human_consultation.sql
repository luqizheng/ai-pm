-- 0046: PA07-IMP-006 Human Consultation 最小表（PA-07 V1.0 §10.2 / 审查85；
-- 实施规范 §4.7 IMP-006）。
-- 现状核对结论：governance.human_intervention 是执行干预动作（veto/rollback/
-- freeze，无决策内容/范围语义）；delivery.task_waiting 是执行等待。均不是
-- 「需要人类正式决定的工作项」——按实施规范 §4.7 新建最小表，不建通用审批流。
--
-- 状态机：pending / in_progress / decided / cancelled（审查85 最小四态）。
-- decided 的四元组（决定内容/决定人/决定时间）由 DB CHECK 强制完整；
-- cancelled 必须带原因（仅表示不再需要处理，≠默认同意原方案）。
-- 既有决定被改变时追加新行（supersedes_consultation_id 关联），不覆盖历史。
-- 「未回应不得视为同意」由结构保证：不存在任何自动转 decided 的路径。
-- 调度联动：挂 task_id 的咨询同事务写 delivery.task_waiting('needs_decision')，
-- 复用 claim 七门与解锁判定的现成执法点；decide/cancel 时结束等待。
-- 多 Product 决策分别建行（product_scope 记录适用范围；一行不得代替另一
-- Product 的决定，由行为约定 + product_scope 可见性保证）。
-- 无存量数据：直接建表（AGENTS.md 规则 1）。

CREATE TABLE governance.human_consultation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid,
    title text NOT NULL,
    question text NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    decision_content text,
    decided_by uuid,
    decided_at timestamp with time zone,
    product_scope jsonb DEFAULT '[]'::jsonb NOT NULL,
    scope_note text,
    cancelled_reason text,
    supersedes_consultation_id uuid,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_hc_status CHECK (
        status = ANY (ARRAY['pending'::text, 'in_progress'::text, 'decided'::text, 'cancelled'::text])
    ),
    CONSTRAINT ck_hc_decided_complete CHECK (
        status <> 'decided'
        OR (decision_content IS NOT NULL AND decided_by IS NOT NULL AND decided_at IS NOT NULL)
    ),
    CONSTRAINT ck_hc_cancelled_reason CHECK (
        status <> 'cancelled' OR cancelled_reason IS NOT NULL
    ),
    CONSTRAINT fk_hc_task FOREIGN KEY (task_id) REFERENCES delivery.task(id) ON DELETE SET NULL,
    CONSTRAINT fk_hc_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE governance.human_consultation
    ADD CONSTRAINT human_consultation_pkey PRIMARY KEY (id);

-- 自引用 FK 必须在 PK 之后声明（supersede 追加链）。
ALTER TABLE governance.human_consultation
    ADD CONSTRAINT fk_hc_supersedes FOREIGN KEY (supersedes_consultation_id)
    REFERENCES governance.human_consultation(id);
CREATE INDEX idx_hc_project ON governance.human_consultation (project_id, status);
CREATE INDEX idx_hc_task ON governance.human_consultation (task_id);

COMMENT ON COLUMN governance.human_consultation.product_scope IS
    '适用 Product 范围（product_id 数组）。多 Product 事项各 Product 分别建行分别决定；一行不得代替另一个 Product 的正式决定（PA-07 §10.2）。';
COMMENT ON CONSTRAINT ck_hc_decided_complete ON governance.human_consultation IS
    '审查85：decided 必须包含决定内容、决定人、时间——缺一不可为 decided。';
