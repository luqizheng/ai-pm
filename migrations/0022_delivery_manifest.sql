-- 0022_delivery_manifest: 交付清单事实（Slice D4 / T-041）
-- 依据：docs/ai-pm-slice-d4-delivery-v1.md §2 §3
-- 上位依据：docs/ai-pm-multi-repo-model-v1.md §3.2/§3.3/§4.4/§4.5、
--           docs/ai-pm-slice-d-execution-pipeline-v1.md §三 D4 行 + Q2/Q5 + §15.5。
--
-- 内容：
--   1) delivery.task_run 补 UNIQUE (project_id, id) —— 组合外键的父键（Q5 裁定）。
--   2) delivery.task_run 补 integration_status —— Run 级集成事实的单一事实来源（Q-D4-6）。
--   3) delivery.task_run 补 request_fingerprint —— 提交幂等指纹（§3 四情况分派）。
--   4) delivery.task_repository_commit —— 逐仓「结果 commit → 证据」事实。
--   5) delivery.v_task_delivery_manifest —— 按 task × 契约仓的对账视图。
--
-- 原则：新项目、无存量数据（AGENTS.md 规则 1）——直接改契约，不写回填脚本。

-- ═══════════════════════════════════════════════════════════════════
-- 1) 组合外键的父键
-- ═══════════════════════════════════════════════════════════════════
-- 交付记录要指向「某个 Run」且不跨项目，组合外键 (project_id, task_run_id) →
-- task_run (project_id, id) 需要父表有该唯一键。0019 只补了 (tenant_id, id)。

ALTER TABLE delivery.task_run
    ADD CONSTRAINT uq_task_run_project_id UNIQUE (project_id, id);

-- ═══════════════════════════════════════════════════════════════════
-- 2) integration_status：Run 级集成事实（单一事实来源）
-- ═══════════════════════════════════════════════════════════════════
-- Q-D4-6：仓级 test_evidence **不内嵌**同名字段；partial 整单状态由视图计算。
-- `not_run` 是事实（Run 未执行集成测试），D4 **不自动推断**为 `failed`。

ALTER TABLE delivery.task_run
    ADD COLUMN integration_status TEXT NOT NULL DEFAULT 'not_run'
        CHECK (integration_status IN ('passed', 'failed', 'not_run'));

-- ═══════════════════════════════════════════════════════════════════
-- 3) request_fingerprint：提交幂等指纹
-- ═══════════════════════════════════════════════════════════════════
-- §3：`UNIQUE(task_run_id, repository_id)` 只能证明「库里已有该仓提交记录」，
-- 不能证明「第二次请求与第一次是同一个提交」。指纹对规范化后的完整提交清单
-- 计算摘要：integration_status + 每仓 repository_id / base_commit / commit_sha /
-- changed_files / tests。平台侧计算，契约不增字段。

ALTER TABLE delivery.task_run
    ADD COLUMN request_fingerprint TEXT;

-- ═══════════════════════════════════════════════════════════════════
-- 4) delivery.task_repository_commit：逐仓交付事实
-- ═══════════════════════════════════════════════════════════════════
-- 基线事实**不在本表重复保存**：基线 = task_run_repository.base_commit（D3 领取
-- 快照），按 (task_run_id, repository_id) 关联取得，避免双基线字段漂移（§2.1）。

CREATE TABLE delivery.task_repository_commit (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL,
    project_id         UUID NOT NULL,
    task_run_id        UUID NOT NULL,
    repository_id      UUID NOT NULL,
    commit_sha         TEXT NOT NULL
                       CHECK (length(btrim(commit_sha)) > 0 AND length(commit_sha) <= 128),
    -- 平台从 Run 工作区 git HEAD 读取，不取执行端声明（§3.4）。
    branch             TEXT CHECK (branch IS NULL OR length(branch) <= 255),
    -- Git 现算为准（Q-D4-3）；执行端上报计数只做一致性比对，不一致即拒绝。
    changed_files      INT  NOT NULL DEFAULT 0
                       CHECK (changed_files >= 0),
    -- {submitted_at, tests:[{name,passed,evidence_ref}]}；integration_status
    -- 不内嵌（Q-D4-4 / Q-D4-6）。
    test_evidence      JSONB NOT NULL DEFAULT '{}'::jsonb
                       CHECK (jsonb_typeof(test_evidence) = 'object'
                              AND (NOT test_evidence ? 'tests'
                                   OR jsonb_typeof(test_evidence -> 'tests') = 'array')),
    commit_author      TEXT CHECK (commit_author IS NULL OR length(commit_author) <= 255),
    commit_authored_at TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_run_id, repository_id),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id),
    -- Q2 裁定：Run 历史是交付与审计的重要关联事实，不因 Run 删除级联丢失交付记录。
    FOREIGN KEY (project_id, task_run_id)
        REFERENCES delivery.task_run (project_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (project_id, repository_id)
        REFERENCES integration.project_repository (project_id, id)
);

-- 交付清单按 Run × 仓读取。
CREATE INDEX idx_task_repository_commit_run
    ON delivery.task_repository_commit (task_run_id);

-- 评审收编 3.1 的评估项结论：`task_repository_commit → task_repository` 组合外键
-- **不成立**——`task_repository` 的主键是 (task_id, repository_id)，而本表没有
-- task_id 列（Task 身份经 task_run 传递）。为此外键新增 task_id 列会复制 Run
-- 已有的关联，破坏「单一事实源」，故不改既有模型（评审收编 3.1 的「不得为此外键
-- 破坏既有模型」）。仓归属由上面两条组合外键保证：仓库属本项目、Run 属本项目。

-- ═══════════════════════════════════════════════════════════════════
-- 5) delivery.v_task_delivery_manifest：交付对账视图
-- ═══════════════════════════════════════════════════════════════════
-- 视图只陈述事实，**不改状态机**（delivery_state 永不是任务状态字段）。
--
-- 分母只计可写仓（§2.3）：expected = COUNT(task_repository WHERE write_access)。
-- latest_run / latest_submitted_run 分离（评审收编 P1-2）：
--   latest_run          = attempt_no 最大的 Run，**不考虑是否提交**；
--   latest_submitted_run = 最近一次产生有效交付记录的 Run。
-- 主 Manifest 展示 latest_run 的现状——**不得因最新 Run 未提交而以旧 Run 的
-- 交付顶替最新交付**。

CREATE VIEW delivery.v_task_delivery_manifest AS
WITH contract AS (
    SELECT tr.task_id,
           tr.project_id,
           tr.repository_id,
           tr.write_access,
           pr.name AS repository_name
      FROM delivery.task_repository tr
      JOIN integration.project_repository pr
        ON pr.id = tr.repository_id AND pr.project_id = tr.project_id
),
latest_run AS (
    SELECT DISTINCT ON (r.task_id)
           r.task_id, r.id AS run_id, r.attempt_no, r.integration_status
      FROM delivery.task_run r
     ORDER BY r.task_id, r.attempt_no DESC, r.created_at DESC
),
latest_submitted_run AS (
    SELECT DISTINCT ON (r.task_id)
           r.task_id, c.task_run_id AS run_id
      FROM delivery.task_repository_commit c
      JOIN delivery.task_run r ON r.id = c.task_run_id
     ORDER BY r.task_id, r.attempt_no DESC, c.created_at DESC, c.id DESC
),
counts AS (
    SELECT c.task_id,
           count(*) FILTER (WHERE c.write_access)::int AS expected_delivery_count,
           count(*) FILTER (
               WHERE c.write_access
                 AND lr.run_id IS NOT NULL
                 AND EXISTS (SELECT 1 FROM delivery.task_repository_commit cr
                             WHERE cr.task_run_id = lr.run_id
                               AND cr.repository_id = c.repository_id)
           )::int AS actual_delivery_count
      FROM contract c
      LEFT JOIN latest_run lr ON lr.task_id = c.task_id
     GROUP BY c.task_id
)
SELECT c.task_id,
       c.project_id,
       c.repository_id,
       c.repository_name,
       c.write_access,
       -- 只读仓出现在 Manifest 里，但不是预期 Commit 的交付对象。
       c.write_access AS delivery_required,
       lr.run_id                     AS latest_run_id,
       lr.attempt_no                 AS latest_attempt_no,
       lr.integration_status,
       lsr.run_id                    AS latest_submitted_run_id,
       trr.base_commit,
       cr.commit_sha,
       cr.changed_files,
       cr.test_evidence,
       cr.branch,
       cr.commit_author,
       cr.commit_authored_at,
       cnt.expected_delivery_count,
       cnt.actual_delivery_count,
       -- 四态判定（Q1 裁决）：
       --   expected = 0            → delivered（空集即完备，含全只读仓任务）
       --   actual  = expected      → delivered
       --   0 < actual < expected   → partial
       --   actual  = 0             → never_submitted（latest_run 的现状）
       CASE
           WHEN cnt.expected_delivery_count = 0 THEN 'delivered'
           WHEN cnt.actual_delivery_count = 0 THEN 'never_submitted'
           WHEN cnt.actual_delivery_count = cnt.expected_delivery_count THEN 'delivered'
           ELSE 'partial'
       END AS delivery_state
  FROM contract c
  LEFT JOIN latest_run lr ON lr.task_id = c.task_id
  LEFT JOIN latest_submitted_run lsr ON lsr.task_id = c.task_id
  LEFT JOIN delivery.task_run_repository trr
         ON trr.run_id = lr.run_id AND trr.repository_id = c.repository_id
  LEFT JOIN delivery.task_repository_commit cr
         ON cr.task_run_id = lr.run_id AND cr.repository_id = c.repository_id
  JOIN counts cnt ON cnt.task_id = c.task_id;
