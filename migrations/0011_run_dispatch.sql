-- 0011_run_dispatch: Run 领取 / 回收所需的租约、心跳、执行阶段归因与多仓基线快照。
-- 依据：docs/ai-pm-slice-d-execution-pipeline-v1.md §5.3 / §十一（B2 B3 Q9 R2 R3 S6）

-- 1) 状态：新增 claimed（已签发租约、执行端尚未启动）。
--    R2：不依赖隐式约束名——按 pg_constraint 元数据识别 status 的 CHECK 并 DROP；
--        一个都没找到即显式失败（结构不符合预期，不得静默跳过）。
DO $$
DECLARE c RECORD; dropped INT := 0;
BEGIN
    FOR c IN
        SELECT conname
          FROM pg_constraint
         WHERE conrelid = 'delivery.task_run'::regclass
           AND contype = 'c'
           AND pg_get_constraintdef(oid) LIKE '%status%'
    LOOP
        EXECUTE format('ALTER TABLE delivery.task_run DROP CONSTRAINT %I', c.conname);
        dropped := dropped + 1;
    END LOOP;
    IF dropped = 0 THEN
        RAISE EXCEPTION
            '0011: no status CHECK constraint found on delivery.task_run; schema drift, aborting';
    END IF;
END $$;

ALTER TABLE delivery.task_run
    ADD CONSTRAINT task_run_status_check CHECK (status IN (
        'queued', 'claimed', 'running', 'succeeded', 'failed', 'timed_out', 'cancelled'));

-- 2) 租约、执行端身份、执行阶段与基础设施独立计数（Q9 / B3）
ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS claimed_at                 TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS lease_owner                TEXT,
    ADD COLUMN IF NOT EXISTS lease_expires_at           TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS heartbeat_at               TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS dispatch_attempts          INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS next_dispatch_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    ADD COLUMN IF NOT EXISTS release_reason             TEXT,
    ADD COLUMN IF NOT EXISTS executor_kind              TEXT,
    ADD COLUMN IF NOT EXISTS executor_session           TEXT,
    -- B3：已启动执行器的 Run 被替代前必须记录旧实例终止结果（隔离 / 围栏证据）
    ADD COLUMN IF NOT EXISTS supersedes_run_id          UUID REFERENCES delivery.task_run(id),
    ADD COLUMN IF NOT EXISTS terminated_at              TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS termination_reason         TEXT,
    -- B3-4 / §5.5.2：基础设施类失败的独立重试计数（独立上限，不并入执行连续失败）
    ADD COLUMN IF NOT EXISTS infrastructure_retry_count INT NOT NULL DEFAULT 0;

-- 3) 领取队列索引：只覆盖可领取集合，避免全表扫描
CREATE INDEX IF NOT EXISTS idx_task_run_dispatch
    ON delivery.task_run(next_dispatch_at, created_at)
    WHERE status IN ('queued', 'claimed');

-- 4) 租约到期扫描（健康检查持续运行，独立于调度开关，见 R4 / Q8）
CREATE INDEX IF NOT EXISTS idx_task_run_lease_expiry
    ON delivery.task_run(lease_expires_at)
    WHERE status = 'claimed' AND lease_expires_at IS NOT NULL;

-- 5) R3：删除语义不一致且无实际用途的旧索引。0010 的 idx_task_run_active
--    谓词为 queued/running（漏 claimed），且读路径与修正后的 handler 计数均不
--    依赖它；改由 uq_task_run_active（同样的部分谓词、但唯一）承担读写两侧。
DROP INDEX IF EXISTS delivery.idx_task_run_active;

-- 6) S6：把 I1 从"应用层约定"升级为"数据库事实"。存量冲突必须显式失败，且
--    输出可定位的 task_id / run_id / status / attempt_no，不得自动删除冲突数据。
DO $$
DECLARE r RECORD;
BEGIN
    FOR r IN
        SELECT tr.task_id, tr.id AS run_id, tr.status, tr.attempt_no,
               count(*) OVER (PARTITION BY tr.task_id) AS active_count
          FROM delivery.task_run tr
         WHERE tr.status IN ('queued', 'claimed', 'running')
    LOOP
        IF r.active_count > 1 THEN
            RAISE EXCEPTION
                'I1 violated before 0011: task_id=% run_id=% status=% attempt_no=% (active=%)',
                r.task_id, r.run_id, r.status, r.attempt_no, r.active_count;
        END IF;
    END LOOP;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS uq_task_run_active
    ON delivery.task_run(task_id) WHERE status IN ('queued', 'claimed', 'running');

-- 7) 任务 ↔ 仓库（声明；一个任务可关联多个仓库）
CREATE TABLE IF NOT EXISTS delivery.task_repository (
    task_id       UUID NOT NULL,
    project_id    UUID NOT NULL,
    repository_id UUID NOT NULL,
    write_access  BOOLEAN NOT NULL DEFAULT true,
    base_ref      TEXT NOT NULL DEFAULT 'main',
    base_commit   TEXT,
    path_scope    TEXT[],
    role          TEXT NOT NULL DEFAULT 'primary'
                  CHECK (role IN ('primary', 'secondary', 'knowledge')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, repository_id),
    FOREIGN KEY (project_id, task_id)
        REFERENCES delivery.task(project_id, id) ON DELETE CASCADE,
    -- 组合外键：仓库必须属于同一项目，杜绝跨项目串仓
    FOREIGN KEY (project_id, repository_id)
        REFERENCES integration.project_repository(project_id, id)
);
CREATE INDEX IF NOT EXISTS idx_task_repository_repo
    ON delivery.task_repository(repository_id, created_at DESC);

-- 8) Run ↔ 仓库（事实：本次领取锁定的基线快照，D2 契约从此生成）
CREATE TABLE IF NOT EXISTS delivery.task_run_repository (
    run_id         UUID NOT NULL REFERENCES delivery.task_run(id) ON DELETE CASCADE,
    project_id     UUID NOT NULL,
    repository_id  UUID NOT NULL,
    base_ref       TEXT NOT NULL,
    base_commit    TEXT NOT NULL,
    write_access   BOOLEAN NOT NULL,
    workspace_path TEXT NOT NULL,
    path_scope     TEXT[],
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, repository_id),
    FOREIGN KEY (project_id, repository_id)
        REFERENCES integration.project_repository(project_id, id)
);