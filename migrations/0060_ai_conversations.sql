-- 0060_ai_conversations: V2 D-4=A（裁决 2026-11-08）—— Platform 内最小对话存储。
-- 裁决：docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md §〇
-- （D-4=A，2026-11-08）：V0.5 口径先行——结构化交互（task_interaction 链）不动，
-- 本批只建 Platform 内最小对话存储，承载 AI PM 对话记忆；稿 §8 Work Session 挂账。
-- 语义边界：这是**最小对话记忆**，不是 Work Session；无 outbox / 无审计——消息
-- 只存储，assistant 回复的生产者是后续 AI 链批次（REST 侧只允许 role='user'）。
-- 租户化遵循 0019 模式（tenant_id NOT NULL + (tenant_id,id) 唯一 + 组合外键）。

CREATE TABLE ai.conversation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    -- 会话属主（用户）：列表与读写一律按属主隔离，跨属主访问按不存在处理。
    principal_id UUID NOT NULL REFERENCES core.principal(id),
    title TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','archived')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_message_at TIMESTAMPTZ,
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_conversation_project_principal
ON ai.conversation (project_id, principal_id, updated_at DESC);

CREATE TABLE ai.conversation_message (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai.conversation(id) ON DELETE CASCADE,
    -- user = REST 用户输入；assistant / system 留给内部 AI 链（后续批次）。
    role TEXT NOT NULL CHECK (role IN ('user','assistant','system')),
    content TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'markdown',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_conversation_message_conversation
ON ai.conversation_message (conversation_id, created_at);
