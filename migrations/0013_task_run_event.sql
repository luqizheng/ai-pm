-- 0013_task_run_event.sql
--
-- S05 · 执行过程监控最小事件面（Execution Monitoring · Minimal Run Event Log）。
-- 设计依据：docs/project-ai-team-v2/ai-pm-v05-execution-roadmap-v1.md §9（执行事件
-- 持久化「V2 正式运行前必须；先做最小方案：event_id/tenant/project/task/run/
-- client/event_type/generation/occurred_at/received_at，幂等写入+审计查询——
-- 不做大型事件溯源」）+ 现状对照 docs/s05/S05-00-code-facts-and-gap-analysis-v1.md
-- 缺口 S05-05。
--
-- 核心立场：
--   ① 这是**事件账**，不是执行记录重建——执行事实仍在 delivery.task_run /
--     task_repository_commit / manifest 视图（禁重建区：不建 task_execution 新表）。
--   ② 只记「无专表承载」的生命周期时刻：Run 领取/启动/阶段/终态与 Worker 命令
--     生命周期。interaction / continuation / response_delivery 已有专表
--     （delivery.task_interaction / continuation / interaction_response_delivery），
--     监控面联表取数，本表不重复落账。
--   ③ 幂等写入：UNIQUE (run_id, event_type, lease_generation, dedupe_tag) +
--     INSERT ON CONFLICT DO NOTHING（应用层 run_events::record_run_event）。
--   ④ 追加式：UPDATE 禁止（事件不可改写）；DELETE 保留给未来保留期清理，
--     不做溯源回放，允许按保留策略删除。
--   ⑤ 沿用仓库模式：无数据库级 FK（跨聚合引用由应用层保证）、text + CHECK
--     枚举、tenant/project 冗余列。

CREATE TABLE delivery.task_run_event (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    client_id uuid,
    event_type text DEFAULT '' NOT NULL,
    lease_generation integer DEFAULT 0 NOT NULL,
    dedupe_tag text DEFAULT '' NOT NULL,
    occurred_at timestamp with time zone DEFAULT now() NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    detail jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_run_event_type_check CHECK ((event_type = ANY (ARRAY[
        'run_claimed'::text,
        'run_started'::text,
        'run_stage_changed'::text,
        'run_finished'::text,
        'command_issued'::text,
        'command_delivered'::text,
        'command_acked'::text,
        'command_failed'::text
    ]))),
    CONSTRAINT task_run_event_generation_check CHECK ((lease_generation >= 0)),
    CONSTRAINT task_run_event_dedupe_tag_len CHECK ((length(dedupe_tag) <= 200))
);

-- 幂等键：同一 Run 内「事件类型 + 执行权世代 + 去重标签」至多一条。
-- dedupe_tag 语义：run_stage_changed=stage 名；command_*=命令 id；run_claimed /
-- run_started / run_finished 用空串（同 generation 内天然至多一次）。
CREATE UNIQUE INDEX uq_task_run_event
    ON delivery.task_run_event (run_id, event_type, lease_generation, dedupe_tag);

-- 监控查询主路径：项目内按 Run 聚合的时间线。
CREATE INDEX idx_task_run_event_project
    ON delivery.task_run_event (tenant_id, project_id, run_id, occurred_at DESC);

-- 事件不可改写（追加式账本）；DELETE 保留给保留期清理（见头注 ④）。
CREATE FUNCTION delivery.guard_task_run_event() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'delivery.task_run_event is append-only (update forbidden)';
    END IF;
    RETURN COALESCE(NEW, OLD);
END;
$$;

CREATE TRIGGER trg_task_run_event_guard
    BEFORE UPDATE ON delivery.task_run_event
    FOR EACH ROW EXECUTE FUNCTION delivery.guard_task_run_event();

COMMENT ON TABLE delivery.task_run_event IS
    'S05 执行监控最小事件账（roadmap §9）：Run 生命周期与 Worker 命令时刻的幂等追加账。interaction/continuation/delivery 有专表不重复落账；不做大型事件溯源。';
