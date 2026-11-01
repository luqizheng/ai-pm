-- 0017_customer_master_data: 客户从「建项副作用」升级为一等实体
--
-- 背景：0001 已建 core.customer 与 core.project.customer_id，但 bootstrap_project
-- 以 name = project.code 隐式造客户，实际是 1 项目 = 1 客户。本迁移补齐主数据
-- 字段，并把历史隐式客户标记为 placeholder，供 UI 区分与人工合并。
-- 设计说明见 docs/ai-pm-customer-master-data-v1.md。

ALTER TABLE core.customer
    ADD COLUMN code TEXT,
    ADD COLUMN short_name TEXT,
    ADD COLUMN industry TEXT,
    ADD COLUMN notes TEXT,
    ADD COLUMN parent_id UUID REFERENCES core.customer(id),
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- code 是业务侧可读唯一键（幂等键）。历史占位客户 code 为空，故用部分唯一索引：
-- 多个 NULL 不冲突，非空值必须全局唯一。
CREATE UNIQUE INDEX customer_code_uniq ON core.customer (code)
    WHERE code IS NOT NULL;

CREATE INDEX customer_parent_idx ON core.customer (parent_id);

-- 客户详情页按客户列项目，项目列表按客户过滤，都需要这个索引。
CREATE INDEX project_customer_idx ON core.project (customer_id);

-- 存量：把「客户名 == 该项目 code」的隐式客户标为 placeholder（未归类）。
-- 不做自动归并——系统无从判断 AIDEMO 属于哪家客户，交由人工在 UI 合并。
UPDATE core.customer c
SET status = 'placeholder'
WHERE c.code IS NULL
  AND EXISTS (
      SELECT 1
      FROM core.project p
      WHERE p.customer_id = c.id
        AND p.code = c.name
  );
