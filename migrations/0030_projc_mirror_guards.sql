-- 0030_projc_mirror_guards.sql
--
-- P3-D04-02 Step5 Batch1 GAP-37-A/B：ProjC 镜像保护面补齐（D-4 裁决）。
-- 与 pc 真源逐字同款（0001:7043 trg_pc_revision_immutable /
--   0001:7071 trg_pc_vm_immutable 同构，域内改前缀）。
-- 函数 projc.forbid_row_update 已在位（0025:329-334，无条件 RAISE→
--   BEFORE DELETE OR UPDATE 下 UPDATE/DELETE 双面拒绝）；本迁移零新函数。
-- 实施设计核实记录：函数实际行为/触发时机/异常语义 2026-11-17 只读核实
--   （《Step5 批次规划 V1.0》§五.1）；实测探针为 §六 验收项。
-- AGENTS.md 规则 1：纯增量，无数据迁移。

CREATE TRIGGER trg_projc_revision_immutable
    BEFORE DELETE OR UPDATE ON projc.cognition_revision
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();

CREATE TRIGGER trg_projc_vm_immutable
    BEFORE DELETE OR UPDATE ON projc.version_member
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();
