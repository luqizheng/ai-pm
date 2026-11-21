-- 0049 · Product 业务标识 Key（PA-07 spec §4.8，2026-11-21）
--
-- id = 内部主键（实体关联/外键引用），key = 全局唯一稳定业务标识（UI 展示、
-- 业务文档、AI-PM/MCP/API 引用、审计、导入导出）。两者不互相替代：一切
-- 关联仍走 id，key 不作外键。
--
-- 规则：NOT NULL；全局唯一以 lower(key) 唯一索引保证（大小写不敏感）；
-- 格式 ^[A-Z0-9][A-Z0-9-]{0,62}[A-Z0-9]$；创建时由服务层从 name 派生自动
-- 生成（派生/兜底/冲突后缀规则见 pm-application product.rs），创建后不可变。
-- 本迁移对存量行做一次性回填：同服务层派生规则 + 同基名行号消解冲突 +
-- 格式/冲突兜底（id 前 8 位十六进制）。

ALTER TABLE core.product ADD COLUMN key text;

-- 回填第一遍：按服务层派生规则生成基名；同基名行按 created_at 序追加行号。
WITH raw AS (
    SELECT id, created_at,
           regexp_replace(
               regexp_replace(
                   regexp_replace(upper(name), '[^A-Z0-9]+', '-', 'g'),
                   '^-+|-+$', '', 'g'),
               '-{2,}', '-', 'g') AS base
    FROM core.product
),
derived AS (
    SELECT id, base, row_number() OVER (PARTITION BY base ORDER BY created_at, id) AS rn
    FROM raw
)
UPDATE core.product p
SET key = CASE
    WHEN d.base IS NULL OR length(d.base) < 2
        THEN 'P-' || upper(substring(p.id::text from 1 for 8))
    WHEN d.rn = 1
        THEN substring(d.base from 1 for 64)
    ELSE substring(d.base from 1 for 55) || '-' || d.rn::text
END
FROM derived d
WHERE d.id = p.id;

-- 回填第二遍兜底：对不合格式或仍冲突的行，用 id 前 8 位十六进制重建
-- （全局唯一由 id 保证；开发期存量极小，正常不触发）。
UPDATE core.product p
SET key = substring(regexp_replace(p.key, '-+[A-Z0-9-]*$', ''), 1, 52)
         || '-' || upper(substring(p.id::text from 1 for 8))
WHERE p.key IS NULL
   OR p.key !~ '^[A-Z0-9][A-Z0-9-]{0,62}[A-Z0-9]$'
   OR EXISTS (
       SELECT 1 FROM core.product q
        WHERE q.id <> p.id AND lower(q.key) = lower(p.key)
   );

ALTER TABLE core.product ALTER COLUMN key SET NOT NULL;

ALTER TABLE core.product ADD CONSTRAINT ck_product_key_format
    CHECK (key ~ '^[A-Z0-9][A-Z0-9-]{0,62}[A-Z0-9]$');

CREATE UNIQUE INDEX uq_product_key ON core.product USING btree (lower(key));
