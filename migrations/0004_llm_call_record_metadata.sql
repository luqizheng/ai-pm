-- 0004_llm_call_record_metadata: DK-F 真栈联调——LLM 调用记账加结构化元数据列。
-- 设计：docs/product/ai-pm-dialog-orchestration-design-v1.md §5.3（裁剪必须可观测）
-- / §8.3-D2（「超预算整节裁剪，且 llm_call_record.metadata 记录 sections_dropped」）；
-- 裁决 D-DK-7（§13：裁剪可观测是硬验收）。缺口来源：DK-C/DK-B 验收记录挂账
-- 「0041 无 metadata 列，sections_dropped 无处可落」，DK-F 第 0 步登记 G-2。
--
-- 语义：可空 JSONB，NULL = 该调用无结构化元数据（存量全部 NULL，无回填）。
-- 当前唯一写入方 = 编排轮次（purpose='discovery_orchestration'）：
-- {sections_included, sections_dropped, sections_absent, chars_total}。
-- 不加约束、不加索引（观测面，非查询热路径）。

ALTER TABLE ai.llm_call_record
    ADD COLUMN metadata JSONB NULL;

COMMENT ON COLUMN ai.llm_call_record.metadata IS
    'DK-F/D2: 结构化调用元数据（编排轮次的 sections_dropped 等）；NULL = 无。';
