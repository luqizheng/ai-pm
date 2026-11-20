-- 0047: PA07-IMP-007 追溯闭环——Product↔Project Link 软删化（与 task_product
-- 同款有效性历史模式；IMP-001 §5-3 裁决方向落地）。
-- 解除 Link 不再物理删除：withdrawn_by/at 记录撤回时间与操作者，历史可追溯；
-- partial unique 仅约束有效行——撤回后允许重新建立归属（新行）。
-- 无存量数据：直接改契约（AGENTS.md 规则 1）。

ALTER TABLE core.product_project_link
    ADD COLUMN withdrawn_by uuid,
    ADD COLUMN withdrawn_at timestamp with time zone;

ALTER TABLE core.product_project_link DROP CONSTRAINT ck_product_project_link_pair;
CREATE UNIQUE INDEX uq_product_project_link_active
    ON core.product_project_link (product_id, project_id) WHERE withdrawn_at IS NULL;

COMMENT ON COLUMN core.product_project_link.withdrawn_at IS
    '非空 = 关联已解除（软删）；历史保留供追溯（PA07-IMP-007，与 delivery.task_product 同模式）。';
