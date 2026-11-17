-- 0031_proposal_expire_columns.sql
--
-- P3-D04-02 Step5 Batch3 GAP-36：proposal 增 expired_at / expired_reason 列
-- （T-10 submitted→expired 人工终态承载；Q-12 模式 A：仅人工触发、reason 必填、
--   无自动到期）。expired_at=决策事实时间（R38，GAP-33 同批语义），非流程元数据。
-- 两域镜像（pc/projc）；纯增量无数据迁移；AGENTS.md 规则 1。

ALTER TABLE pc.proposal
    ADD COLUMN expired_at timestamp with time zone;
ALTER TABLE pc.proposal
    ADD COLUMN expired_reason text;

ALTER TABLE projc.proposal
    ADD COLUMN expired_at timestamp with time zone;
ALTER TABLE projc.proposal
    ADD COLUMN expired_reason text;
