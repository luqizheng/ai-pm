-- 0035_s01_run_cleanup: S01 · I1-E 持久化进程清理账本（设计稿 §9.2）。
--
-- 依据：docs/ui/smark-pm-s01-i1-archive-lifecycle-v1.md §9（Executor 进程清理）：
--   「Run 终态与进程资源清理是两个独立生命周期」「必须存在持久化清理记录，
--    或者证明现有结构可以完整承担同等职责」。现有 `cleanup_finished_processes`
--    的 1 小时窗口 + LIMIT 64 **不能作为完整清理保障**（§9.4）——超过 1 小时的
--    残留进程将永远丢失。本迁移把清理状态落库：
--
--   pending → cleaning → completed；失败可重试；达到重试阈值 → needs_human
--   （持续可查询、可告警，I1-040）。
--
-- 无回填（AGENTS.md 规则 1）：新项目无存量残留进程；账本行由取消/收割路径
-- 在撤凭据时写入（cancel_run / harvest），旧行为的「尽力而为 terminate」保留
-- 作为兜底。

CREATE TABLE delivery.task_run_cleanup (
    run_id          UUID NOT NULL,
    session         TEXT NOT NULL,
    executor_kind   TEXT,
    -- 进程身份锚点（identity_hint）：裸 PID 不足以防复用误杀（§9.3），
    -- 锚点由执行器写入 pid 文件，本表只存其引用事实。
    identity_hint   TEXT,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'cleaning', 'completed', 'needs_human')),
    attempts        INT  NOT NULL DEFAULT 0,
    max_attempts    INT  NOT NULL DEFAULT 8,
    next_retry_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error      TEXT,
    completed_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id),
    FOREIGN KEY (run_id) REFERENCES delivery.task_run (id) ON DELETE CASCADE,
    CHECK (completed_at IS NULL OR status = 'completed')
);

-- 清理 Worker 的领取索引：只扫到期的待清理行。
CREATE INDEX idx_task_run_cleanup_due
    ON delivery.task_run_cleanup (status, next_retry_at)
    WHERE status IN ('pending', 'cleaning');
-- 人工处理队列的查询索引（I1-040：可查询、告警）。
CREATE INDEX idx_task_run_cleanup_human
    ON delivery.task_run_cleanup (status, created_at)
    WHERE status = 'needs_human';
