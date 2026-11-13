-- 0014_task_knowledge_pin.sql
--
-- S05 · 执行上下文注入闭环（批 3 · S05-02 执行上下文注入）。
-- 现状对照 `docs/scenarios/s05/S05-00-code-facts-and-gap-analysis-v1.md` 缺口 S05-02；
-- 裁决记录 `docs/architecture/knowledge-services/kp-s05-knowledge-injection-contract-deviations-v1.md`
-- （用户裁决 2026-11-10：知识来源 = **显式锚点 + 人工确认**）。
--
-- 核心立场：
--   ① **不新建知识表**：`delivery.task_knowledge_ref` 早已存在（任务 → 项目
--      文档身份表的锚点表），本迁移只补「钉版」一列。
--   ② **钉版可选**：`pinned_revision_no IS NULL` = 取该文档最新 published
--      修订（默认，不落后）；非空 = 人工确认时钉定的版本（可复现）。
--      两个语义都属「正确的执行依据」：默认取当前权威版，人工可按需固定。
--   ③ 沿用仓库模式：无数据库级 FK 新增、text/int + CHECK、幂等写入。
--   ④ ADR-06a：文档身份表/修订表是冻结的共享容器——本迁移**不触碰**它们
--      （唯一合法落点是基线迁移 0001_baseline.sql）。

ALTER TABLE delivery.task_knowledge_ref
    ADD COLUMN pinned_revision_no integer;

ALTER TABLE delivery.task_knowledge_ref
    ADD CONSTRAINT task_knowledge_ref_pinned_revision_check
    CHECK (pinned_revision_no IS NULL OR pinned_revision_no >= 1);

COMMENT ON COLUMN delivery.task_knowledge_ref.pinned_revision_no IS
    '知识锚点的钉版修订号：NULL=取该文档最新 published 修订（默认，不落后）；非空=人工确认时钉定的修订号（可复现，Agent 读到的依据固定为该版）。';
