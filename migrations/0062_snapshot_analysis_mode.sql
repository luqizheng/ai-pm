-- 0062_snapshot_analysis_mode：仓库快照分析引入 LLM 执行体（P2-13 裁决 D-1=A，
-- PC-I6 起笔切片 D-2=A，降级语义 D-3=A；2026-11-08 裁决包
-- docs/project-ai-team-v2/ai-pm-p2-13-pc-i6-llm-analysis-adjudication-pack-v1.md）。
--
-- 修订行持久化分析模式，供治理面对账与降级可观测（产物正文尾部另有可读标记，
-- pc.evidence.locator 另记 llm_evidence_refs）：
-- * derived          —— 结构化事实派生（无 LLM；既有行为，默认值）
-- * derived_fallback —— 已配置 LLM 但解析/调用失败，回退结构化派生（D-3=A）
-- * llm              —— LLM 增强分析成功（产物仍 draft/PROPOSED，确认语义不变）
-- kb.document_revision / pc.cognition_revision 均 append-only（0053 触发器），
-- 本批仅 ADD COLUMN（默认值回填存量行为 derived，语义准确）。
ALTER TABLE kb.document_revision
    ADD COLUMN analysis_mode TEXT NOT NULL DEFAULT 'derived'
    CHECK (analysis_mode IN ('derived', 'derived_fallback', 'llm'));

ALTER TABLE pc.cognition_revision
    ADD COLUMN analysis_mode TEXT NOT NULL DEFAULT 'derived'
    CHECK (analysis_mode IN ('derived', 'derived_fallback', 'llm'));
