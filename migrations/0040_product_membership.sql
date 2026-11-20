-- 0040: P4-ORG Phase 3 Product Membership 与生命周期（PA 基线 §2/§3/§4/§5；DR-PA-01/02/03，2026-11-20 签发）
-- 1) core.product_member：Product 内部成员名单（Owner/Member 双身份；Team Member
--    不自动成为 Product 成员——DR-ORG-03①②已废止）。
-- 2) core.product.status：{active, abandoned} 两态（PA §5；restart 是状态转换，
--    不设持久化 reactivated）。
-- 无存量数据：直接建表改契约（AGENTS.md 规则 1）。

CREATE TABLE core.product_member (
    product_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    role text NOT NULL,
    granted_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_product_member_role CHECK ((role = ANY (ARRAY['owner'::text, 'member'::text]))),
    CONSTRAINT fk_product_member_product FOREIGN KEY (product_id) REFERENCES core.product(id) ON DELETE CASCADE,
    CONSTRAINT fk_product_member_principal FOREIGN KEY (principal_id) REFERENCES core.principal(id),
    CONSTRAINT fk_product_member_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.product_member ADD CONSTRAINT product_member_pkey PRIMARY KEY (product_id, principal_id);

ALTER TABLE core.product ADD COLUMN status text DEFAULT 'active'::text NOT NULL;
ALTER TABLE core.product
    ADD CONSTRAINT ck_product_status CHECK ((status = ANY (ARRAY['active'::text, 'abandoned'::text])));

CREATE INDEX idx_product_member_principal ON core.product_member (principal_id);
CREATE INDEX idx_product_member_product ON core.product_member (product_id);
