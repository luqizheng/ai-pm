-- scripts/reconciliation/write_operation_consistency.sql
--
-- P3-D04-02 Step5 Batch5 GAP-21 对账巡检（只读运维差分；零写路径）。
-- 差分口径（责任矩阵 GAP-21 行）：completed 发布账本行 × 正式版本存在性——
-- ①completed 发布账本行指向不存在的 version（孤儿账本）；
-- ②正式版本无对应 completed 发布账本行（无账本产物）。
-- 运行：psql -f 本文件（或容器 docker exec -i psql < 本文件）；期望输出 0 行
-- （同事务模型 D6.7 下不可能产生差分；非 0 = 违反同事务不变量，须人工核查）。
-- 另含起草段账本自检（Batch4a 扩展面）：completed 起草行应有对应活提案。

-- ① 孤儿账本：completed 发布行 → version 缺失
SELECT 'orphan_completed_publish_row' AS divergence_kind, 'pc' AS domain,
       wo.operation_id, wo.command, wo.status
FROM pc.write_operation wo
WHERE wo.status = 'completed' AND wo.command = 'pc_publish_version'
  AND NOT EXISTS (SELECT 1 FROM pc.cognition_version v WHERE v.id = wo.version_id)
UNION ALL
SELECT 'orphan_completed_publish_row', 'projc', wo.operation_id, wo.command, wo.status
FROM projc.write_operation wo
WHERE wo.status = 'completed' AND wo.command = 'projc_publish_version'
  AND NOT EXISTS (SELECT 1 FROM projc.cognition_version v WHERE v.id = wo.version_id);

-- ② 无账本产物：正式版本 → 无对应 completed 发布行
SELECT 'version_without_completed_ledger_row' AS divergence_kind, 'pc' AS domain,
       v.id::text AS operation_id, 'pc_publish_version' AS command, 'missing' AS status
FROM pc.cognition_version v
WHERE NOT EXISTS (
    SELECT 1 FROM pc.write_operation wo
     WHERE wo.status = 'completed' AND wo.command = 'pc_publish_version'
       AND wo.version_id = v.id)
UNION ALL
SELECT 'version_without_completed_ledger_row', 'projc', v.id::text,
       'projc_publish_version', 'missing'
FROM projc.cognition_version v
WHERE NOT EXISTS (
    SELECT 1 FROM projc.write_operation wo
     WHERE wo.status = 'completed' AND wo.command = 'projc_publish_version'
       AND wo.version_id = v.id);

-- ③ 起草段自检：completed 起草账本行 → 活提案缺失（同事务应无孤儿）
SELECT 'orphan_completed_draft_row' AS divergence_kind, 'pc' AS domain,
       wo.operation_id, wo.command, wo.status
FROM pc.write_operation wo
WHERE wo.status = 'completed' AND wo.command = 'pc_propose_from_analysis'
  AND NOT EXISTS (
      SELECT 1 FROM pc.proposal p
       WHERE p.source_id IS NOT NULL AND p.origin_kind = 'ai_agent'
         AND p.status NOT IN ('failed','expired')
         AND p.project_id = wo.project_id)
UNION ALL
SELECT 'orphan_completed_draft_row', 'projc', wo.operation_id, wo.command, wo.status
FROM projc.write_operation wo
WHERE wo.status = 'completed' AND wo.command = 'projc_propose_from_analysis'
  AND NOT EXISTS (
      SELECT 1 FROM projc.proposal p
       WHERE p.source_id IS NOT NULL AND p.origin_kind = 'repo_analysis'
         AND p.status NOT IN ('failed','expired')
         AND p.project_id = wo.project_id);
