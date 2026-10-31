-- 0007_llm_provider: 全局 LLM 供应商配置（OpenAI 兼容面）。
-- Per docs/ai-pm-architecture-principles.md §密钥管理: API Key 明文永不落库，
-- 只存 AEAD 密文（密钥由 AI_PM_SECRET_KEY 派生）；api_key_hint 仅保留末 4 位
-- 供界面识别。五家供应商（deepseek/openai/glm/minimax/ollama）都实现
-- /chat/completions 兼容面，故共用 base_url + model + timeout 三个字段。

CREATE TABLE ai.llm_provider (
    provider TEXT PRIMARY KEY CHECK (
        provider IN ('deepseek', 'openai', 'glm', 'minimax', 'ollama')
    ),
    base_url TEXT NOT NULL,
    model TEXT NOT NULL DEFAULT '',
    api_key_ciphertext BYTEA,
    api_key_hint TEXT,
    enabled BOOLEAN NOT NULL DEFAULT false,
    timeout_secs INT NOT NULL DEFAULT 120 CHECK (timeout_secs BETWEEN 1 AND 600),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO ai.llm_provider (provider, base_url, model) VALUES
    ('deepseek', 'https://api.deepseek.com/v1', 'deepseek-chat'),
    ('openai', 'https://api.openai.com/v1', 'gpt-4o-mini'),
    ('glm', 'https://open.bigmodel.cn/api/paas/v4', 'glm-4-plus'),
    ('minimax', 'https://api.minimax.chat/v1', 'MiniMax-Text-01'),
    ('ollama', 'http://127.0.0.1:11434/v1', 'qwen2.5:7b');