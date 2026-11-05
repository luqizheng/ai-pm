-- 0036_s01_cognition: S01-04 AI-PM Cognition V1 —— 认知 Run / 认知条目 / 项目知识
-- 设计稿：docs/ui/s01/s01-04-ai-pm-cognition-ux-v1-1.md（五裁决 Q1~Q5 已收编）。
-- V1 边界：分析推导用既有结构化事实派生（项目记录 / 仓库元数据），
-- 真模型编排走后续 ai-pm-worker 切片；本迁移只固化三实体与状态机。
-- 租户化遵循 0019 模式（tenant_id NOT NULL + (tenant_id,id) 唯一 + 组合外键）。

CREATE TABLE ai.cognition_run (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    -- Run 状态机（§5.1）。V1 只落 pending / completed / partially_completed /
    -- failed / cancelled；collecting / analyzing 属执行器阶段，编排切片启用。
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','collecting','analyzing','completed',
                          'partially_completed','failed','cancelled')),
    -- 分析范围与输入资料（§2.4）：来源类型与仓储基准版本。
    scope JSONB NOT NULL DEFAULT '{}'::jsonb,
    repo_versions JSONB NOT NULL DEFAULT '[]'::jsonb,
    item_count INT NOT NULL DEFAULT 0,
    confirmed_count INT NOT NULL DEFAULT 0,
    -- Q2-C：正式认知由人类启动，AI 不得自行启动（§11）。
    started_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ,
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_cognition_run_project
ON ai.cognition_run (project_id, created_at DESC);

CREATE TABLE ai.cognition_item (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    run_id UUID NOT NULL REFERENCES ai.cognition_run(id),
    -- 六认知领域（§3）。
    domain TEXT NOT NULL
        CHECK (domain IN ('identity','repository','domain','architecture',
                          'convention','evolution')),
    content TEXT NOT NULL,
    -- 证据来源 + 类型（§4.5）：{type, source, ...}；V1 type ∈
    -- project_record / repository_metadata。
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    confidence TEXT NOT NULL DEFAULT 'medium'
        CHECK (confidence IN ('high','medium','low')),
    has_conflict BOOLEAN NOT NULL DEFAULT false,
    -- Item 状态机（§5.2）。investigation = Request Investigation。
    state TEXT NOT NULL DEFAULT 'proposed'
        CHECK (state IN ('proposed','confirmed','rejected','investigation',
                         'deferred','superseded')),
    -- 确认 / 拒绝记录（§6.3/6.4）：操作人、时间、备注（Edit&Confirm 的修订
    -- 正文落 content 更新，原 AI 提案保存在 decision_note 前缀）。
    decided_by UUID REFERENCES core.principal(id),
    decided_at TIMESTAMPTZ,
    decision_note TEXT,
    knowledge_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_cognition_item_run ON ai.cognition_item (run_id, state);

-- 已确认认知 = 项目知识（§2.2）。版本链：重复确认同领域同源走 supersede。
CREATE TABLE ai.project_knowledge (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    domain TEXT NOT NULL
        CHECK (domain IN ('identity','repository','domain','architecture',
                          'convention','evolution')),
    content TEXT NOT NULL,
    source_item_id UUID REFERENCES ai.cognition_item(id),
    version INT NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','superseded','retired')),
    supersedes_id UUID REFERENCES ai.project_knowledge(id),
    -- 确认记录：确认人 + 时间（§2.2 Confirmation Record）。
    confirmed_by UUID NOT NULL REFERENCES core.principal(id),
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_project_knowledge_active
ON ai.project_knowledge (project_id, status, confirmed_at DESC);
