-- 0052_recovery_foundation: Recovery Foundation 批（联合实施基线 V1.0 批 3）
-- （ACP V1.2 §3.6 同主体续领 R-ACP-16 已裁 = 方案 a；R-11 续领与 Session 重启
--   分离（ACP-C4）；专家第 2 块 2.6：链间弱关联 previous_chain_id。）
--
-- 取号：tentative 0052（0049 Client Registry / 0050 ACP-04 已落；0051 暂定预留
-- 给 ACP-05——落地前重核台账）。

-- 1) 命令行 generation 集合比对（R-2/R-11：§3.3 ③ 修订）——续领写回字段
ALTER TABLE delivery.worker_command
    ADD COLUMN reacquired_generation INT;
-- 部分索引支撑续领后的路由/比对查询（可选：查询均走 run_id 前缀索引，不强制）

-- 2) 链间弱关联（专家第 2 块裁决 2.6）：只引用同 Task 已关闭前驱链的最新
--    Interaction；禁自引用/环（应用服务创建时校验）；不改变 retry_of 链内语义；
--    只服务历史追溯/审计/UI 展示。
ALTER TABLE delivery.task_interaction
    ADD COLUMN previous_chain_id UUID REFERENCES delivery.task_interaction(id);
CREATE INDEX idx_task_interaction_prev_chain
    ON delivery.task_interaction (previous_chain_id)
    WHERE previous_chain_id IS NOT NULL;
