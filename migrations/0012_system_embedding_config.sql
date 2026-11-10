-- 0012_system_embedding_config.sql
--
-- 系统级 embedding 供给配置（D-1 裁决落地物；检索任务书 G-1/G-3）。
--
-- D-1 已裁（2026-11-10）：embedding = bge-m3 @ Ollama 本地（1024 维，公司文档
-- 不出域）。本迁移建立**单行系统级配置**，供管理面（/admin/settings）读写与
-- 连通性测试；Vector 通道本体仍按 G-3 门控（enabled=true 表示意图，消费点在
-- Vector 通道启用时读取本表——届时有效性要素④ embedding_model_version 一并
-- 落列）。
--
-- 门语义：本表为**系统级**单例（与租户无关）；写门 = 调用方 is_admin（应用层
-- 校验，pm-api handler）。当前部署形态（公司内部单栈、空库首管理员注册）下
-- tenant admin 即系统管理员；若未来转多租户 SaaS，需引入平台级角色再收紧
-- （登记于 handler 文档注释）。

CREATE SCHEMA IF NOT EXISTS platform;

CREATE TABLE platform.embedding_config (
    id smallint PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    endpoint text NOT NULL DEFAULT '',
    model text NOT NULL DEFAULT '',
    enabled boolean NOT NULL DEFAULT false,
    updated_by uuid,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    -- 单行约束由 id CHECK 保证；endpoint 形态校验在应用层（http/https 前缀）。
    CONSTRAINT embedding_config_endpoint_check
        CHECK (endpoint = '' OR endpoint LIKE 'http://%'),
    CONSTRAINT embedding_config_model_check
        CHECK (model = '' OR length(btrim(model)) > 0)
);

-- 种子 = .10 已部署实例（2026-11-10 部署验证：1024 维中文向量实测通过）；
-- enabled=false：Vector 通道 G-3 门控中，配置先行、消费后置。
INSERT INTO platform.embedding_config (id, endpoint, model, enabled)
VALUES (1, 'http://192.168.2.10:11434/v1', 'bge-m3', false);
