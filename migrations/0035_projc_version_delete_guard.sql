-- 0035_projc_version_delete_guard.sql
--
-- P3-D04-02 Step6 前置 GAP-37-C：projc.cognition_version 补 DELETE immutable 守卫
-- （负责人裁决批准设立 2026-11-18）。pc 真源=trg_pc_version_delete
-- （0001:7050）逐字同款，域内改前缀；函数 projc.forbid_row_update 复用
-- （0025:329 在位，零新函数）。镜像保护面最后一条同族不对称消除。
-- AGENTS.md 规则 1：纯增量无数据迁移。

CREATE TRIGGER trg_projc_version_delete
    BEFORE DELETE ON projc.cognition_version
    FOR EACH ROW EXECUTE FUNCTION projc.forbid_row_update();
