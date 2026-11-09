-- 0005_conversation_interaction: AI 对话 × 上下文整合 Batch 3C——
-- 交互写回生命周期表（接受/拒绝/提交/失败四反馈的权威落库）。
--
-- 设计：docs/product/ai-pm-assistant-context-integration-plan-v1.md §三
-- Batch 3C；裁决 D-IC-7=A（独立交互记录表，记录号/终态/原因/决策人为列，
-- 信封 status 作回读投影）。
--
-- 取号：0004 已被 DK-F 占用（llm_call_record_metadata，其库已应用）——
-- 按台账纪律顺移 0005（2026-11-09）。
--
-- 语义：
--   - **一线程卡一生命周期**：UNIQUE (message_id)——一张卡只有一个交互
--     生命周期，终态（accepted/rejected/deferred/resolved/submitted）即封盘；
--     重复提交同一 action → 幂等返回既有记录（200）；换 action → 409。
--   - **权威与投影分离**：本表是交互终态的**权威**；写回成功后同事务把
--     消息信封 status 终态化（回读投影，前端刷新/重进/他设备看到同一终态，
--     不回退 presented——A-21）。
--   - **真人落列**：decided_by = 提交交互的会话属主（AI 冒充在端点层被
--     属主校验挡住——AI 不冒充人类决策人，Figma 委托书 §4.5）。
--   - reference：服务端生成的人类可读记录号（INT-XXXXXXXX）。
--   - 无冗余钉列：conversation FK 引主键（id），message FK 引主键——
--     跨线程/跨项目/跨属主伪造由服务层 join 校验挡住（ai_conversations
--     属主隔离先例：404 同形）；conversation 表无 (tenant,project,id)
--     三元组唯一键（0002 的三元组钉的是 discovery_session 侧），不为本表
--     改既有表形状。

CREATE TABLE ai.conversation_interaction (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    message_id uuid NOT NULL,
    -- 触发动作（请求侧词表）
    action text NOT NULL,
    -- 终态（落库即终态；无 in-progress 中间态——中间态是前端表现态）
    status text NOT NULL,
    reason text,
    option_id text,
    answer jsonb,
    decided_by uuid NOT NULL,
    reference text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_interaction_pk PRIMARY KEY (id),
    CONSTRAINT conversation_interaction_action_check CHECK (
        action = ANY (ARRAY['accept'::text, 'reject'::text, 'submit'::text,
                            'defer'::text, 'decision'::text])
    ),
    CONSTRAINT conversation_interaction_status_check CHECK (
        status = ANY (ARRAY['accepted'::text, 'rejected'::text, 'deferred'::text,
                            'submitted'::text, 'resolved'::text])
    ),
    CONSTRAINT conversation_interaction_reason_check CHECK (
        (action <> 'reject') OR (reason IS NOT NULL AND length(btrim(reason)) > 0)
    ),
    CONSTRAINT conversation_interaction_option_check CHECK (
        (action <> 'decision') OR (option_id IS NOT NULL AND length(btrim(option_id)) > 0)
    ),
    CONSTRAINT conversation_interaction_message_uniq UNIQUE (message_id),
    CONSTRAINT conversation_interaction_reference_uniq UNIQUE (reference)
);

ALTER TABLE ai.conversation_interaction
    ADD CONSTRAINT fk_conversation_interaction_conversation
    FOREIGN KEY (conversation_id)
    REFERENCES ai.conversation (id);

ALTER TABLE ai.conversation_interaction
    ADD CONSTRAINT fk_conversation_interaction_message
    FOREIGN KEY (message_id)
    REFERENCES ai.conversation_message (id);

CREATE INDEX idx_conversation_interaction_conversation
    ON ai.conversation_interaction (conversation_id);
