-- 0002_conversation_discovery_link: DK-A 对话编排底座——线程 → 访谈会话链接列。
-- 设计：docs/product/ai-pm-dialog-orchestration-design-v1.md §2.3(4) / §9.2-①；
-- 裁决 D-DK-1=a+（线程目录唯一 + 段落归属 + 单向升级）、D-DK-2=加列（§13）。
-- 取号：设计稿预留 0063；实施时并行批已依次取走 0063（runner_availability）、
-- 0064（pc_formal_model）、0065（kb_three_layer_minimal，均已进 master）→ 顺移
-- 0066；2026-11-09 冻结点基线合并（0001~0065 → 0001_baseline.sql）后按台账
-- §〇 改取 0002。
--
-- 语义：NULL = 纯 chat 线程（存量全部 NULL，无回填）；非 NULL = 线程已升级，
-- 指向最近一次升级的 discovery_session。只加列，不加状态、不加枚举值。
-- 外键用三元组 (tenant_id, project_id, discovery_session_id)，比 (tenant_id, id)
-- 更严：同时钉住「访谈会话与线程同项目」（0031 已建该唯一键）。
-- MATCH SIMPLE：discovery_session_id 为 NULL 时外键不检查。

ALTER TABLE ai.conversation
    ADD COLUMN discovery_session_id UUID NULL;

ALTER TABLE ai.conversation
    ADD CONSTRAINT fk_conversation_discovery_session
    FOREIGN KEY (tenant_id, project_id, discovery_session_id)
    REFERENCES discovery.discovery_session (tenant_id, project_id, id);

CREATE INDEX idx_conversation_discovery_session
    ON ai.conversation (discovery_session_id)
    WHERE discovery_session_id IS NOT NULL;
