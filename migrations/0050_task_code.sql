-- 0050 · Task 业务编号 code（PA-07 任务身份锚，2026-11-21）
--
-- id = 内部主键（关联/外键一律走 id），code = 项目内业务编号（UI 展示、文档
-- 引用、审计锚，与 product.key / requirement.code 同位）。创建后不可变。
--
-- 规则：NOT NULL；项目内 lower(code) 唯一索引（大小写不敏感）。两种合法形态
-- （ck_task_code_format）：
--   PM-NNN      项目内连续号，由 BEFORE INSERT 触发器统一发号（单源）——人工
--               REST、AI 命令通道、intake、测试/演示种子所有 INSERT 路径一致，
--               服务层不再各自发号；
--   P-XXXXXXXX  并发同刻冲突的兜底（id 前 8 位十六进制，与 id 一一对应无竞态）。
-- P- 前缀刻意不匹配 '^PM-[0-9]+$'，不干扰连续号统计口径；关联一律走 id。

ALTER TABLE delivery.task ADD COLUMN code text;

-- 存量回填：项目内按 created_at 序连续发号（与触发器同语义）。
WITH ranked AS (
    SELECT id, row_number() OVER (PARTITION BY project_id ORDER BY created_at, id) AS rn
    FROM delivery.task
)
UPDATE delivery.task t
SET code = 'PM-' || lpad(r.rn::text, 3, '0')
FROM ranked r
WHERE r.id = t.id;

ALTER TABLE delivery.task ALTER COLUMN code SET NOT NULL;

ALTER TABLE delivery.task ADD CONSTRAINT ck_task_code_format
    CHECK (code ~ '^(PM|P)-[A-Z0-9]{3,32}$');

CREATE UNIQUE INDEX uq_task_code_project ON delivery.task (project_id, lower(code));

CREATE OR REPLACE FUNCTION delivery.fill_task_code() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.code IS NOT NULL THEN
        RETURN NEW;
    END IF;
    BEGIN
        -- 项目内连续号：同事务先插的行对本事务可见，批量分解一次建 N 条自然连号。
        NEW.code := 'PM-' || lpad((
            SELECT COALESCE(MAX(NULLIF(regexp_replace(code, '\D', '', 'g'), '')::bigint), 0) + 1
            FROM delivery.task
            WHERE tenant_id = NEW.tenant_id AND project_id = NEW.project_id
              AND code ~ '^PM-[0-9]+$')::text, 3, '0');
    EXCEPTION WHEN unique_violation THEN
        -- 同项目并发创建撞号（罕见）：回退 id 短码，不阻塞创建。
        NEW.code := 'P-' || upper(substring(NEW.id::text from 1 for 8));
    END;
    RETURN NEW;
END;
$$;

CREATE TRIGGER tg_task_fill_code
    BEFORE INSERT ON delivery.task
    FOR EACH ROW
    EXECUTE FUNCTION delivery.fill_task_code();
