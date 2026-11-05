-- 0037_s01_baseline: S01-05 Project Baseline V1 —— 提案 / 条目 / 已确认版本头
-- 设计稿：S01-05 高保真 UX 规格（AI 提案 → 人类逐项确认 → 版本不可变）。
-- 管线：提案条目从 S01-04 已确认知识（ai.project_knowledge）+ 仓库快照派生；
-- 与 S02 需求基线（集合视图，S02-04 V1.2）互不相干——本表是项目级上下文基线。
-- 注意：0036 已被 S01-04 占用，本迁移顺延 0037；S02 起号顺移 0038。

CREATE TABLE ai.baseline_proposal (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    version INT NOT NULL DEFAULT 1,
    -- pending：待逐项确认；confirmed：已成为正式基线（条目随即不可变）；
    -- superseded：被更新提案取代。
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','confirmed','superseded')),
    source_run_id UUID REFERENCES ai.cognition_run(id),
    -- 同一项目同时只允许一个待确认提案（变更走新提案，不原地改）。
    confirmed_by UUID REFERENCES core.principal(id),
    confirmed_at TIMESTAMPTZ,
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE UNIQUE INDEX uq_baseline_proposal_pending
ON ai.baseline_proposal (tenant_id, project_id) WHERE status = 'pending';

CREATE TABLE ai.baseline_item (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    proposal_id UUID NOT NULL REFERENCES ai.baseline_proposal(id),
    -- 六基线类别 + 约束（§5 左栏分节）。
    category TEXT NOT NULL
        CHECK (category IN ('identity','scope','technical','architecture',
                            'repository','standards','constraints')),
    content TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    confidence TEXT NOT NULL DEFAULT 'medium'
        CHECK (confidence IN ('high','medium','low')),
    state TEXT NOT NULL DEFAULT 'proposed'
        CHECK (state IN ('proposed','confirmed','rejected','investigation','deferred')),
    decided_by UUID REFERENCES core.principal(id),
    decided_at TIMESTAMPTZ,
    decision_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

CREATE INDEX idx_baseline_item_proposal ON ai.baseline_item (proposal_id, state);

-- 已确认基线版本头：不可变；历史可追溯（确认人/时间/来源提案）。
CREATE TABLE ai.project_baseline (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES core.tenant(id),
    project_id UUID NOT NULL,
    version INT NOT NULL,
    proposal_id UUID NOT NULL REFERENCES ai.baseline_proposal(id),
    confirmed_by UUID NOT NULL REFERENCES core.principal(id),
    confirmed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, version),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);
