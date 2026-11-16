-- 0027_pc_snapshot_dictionary_deprecated.sql
--
-- P3-11 T-1/T-2/T-3 阶段二（切换批）· W7 pc 收敛：pc.object_type snapshot 四类
-- 字典废弃（范围确认点①=A）。
--
-- 依据：方案 V1.1 W7（「迁移内 pc.object_type snapshot 四类（repository/
-- tech_stack/delivery_activity/module）UPDATE status='deprecated'」；阶段一
-- 明确不含，随切换批落地）。语义：
--   * load_object_type（pc/projc 服务层同款）仅取 status='active' → 废弃即拒——
--     任何残留 pc 路径调用显式报错而非静默（fail-closed，错误域草稿源头不可达）；
--   * identity 链 product_* 类型不受影响（layer=L1_identity/L2_structure 等）；
--   * 既有 pc.cognition_object 存量行（迁移红利窗口内 ≈ 0 行）不删除（R-10：
--     Legacy Transitional 留原位，注册表附录标记承载）。
--
-- AGENTS.md 规则 1：无数据迁移/回填/兼容分支；字典层禁写 = 目标态直接生效。

UPDATE pc.object_type
    SET status = 'deprecated'
  WHERE type_key IN ('repository', 'tech_stack', 'delivery_activity', 'module')
    AND layer = 'snapshot';
