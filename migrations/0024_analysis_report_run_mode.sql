-- 0024_analysis_report_run_mode.sql
--
-- D-2 · 增量/全量分析（docs/scenarios/s01/docs/s01-reanalysis-and-product-cognition-closure-v1.md，
-- 2026-11-15 用户裁决）：报告行记录本次运行模式与增量起点。
--
-- run_mode：incremental（默认——自 since_commit 起区间采集 + 既有认知 merge 重析）
--           / full（显式整库重析）。
-- since_commit：增量起点（=关闭上一份报告时固化的 project_repository.last_analyzed_commit_sha）；
--           全量为 NULL。不用 stage_detail（会被进度文案覆写）。
--
-- 迁移号：0024 = 当前 migrations/ 目录最大号 0023 + 1（台账 §三 纪律）。

ALTER TABLE integration.repo_analysis_report
    ADD COLUMN run_mode text NOT NULL DEFAULT 'full'
        CHECK (run_mode IN ('incremental', 'full')),
    ADD COLUMN since_commit text;
