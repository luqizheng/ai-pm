-- 0058_project_ai_role: D-1=A AI Role 实体 + 不可变 Role Version 全链
-- （裁决 docs/project-ai-team-v2/ai-pm-ai-team-kickoff-adjudication-pack-v1.md §〇/§二）。
--
-- 三表结构（Role 模板 → 项目启用行 → 内容快照）：
--   1) ai.ai_role_template  —— 全局预置模板（role_key 唯一；照全局字典
--      ai.agent_definition 0020:14-22 模式；V1 不开放自定义，is_system 恒真）；
--   2) ai.ai_role_version   —— 不可变版本快照（append-only；UNIQUE(project_role_id,
--      version)；含稿 §4 六组字段的裁剪版 default/actual definition）；
--   3) ai.project_ai_role   —— 项目启用行（一项目一角色至多一行；enabled 启停；
--      current_version_id 指向当前版本）。
-- project 引用照 0019 组合 FK 模式（0036 ai.cognition_run 样板：tenant_id NOT NULL
-- + (tenant_id, id) 唯一 + 组合外键锁同租户）。
-- 环引用（version ↔ project_role / current_version）在两表建好后 ALTER 补 FK：
--   - version.project_role_id ON DELETE RESTRICT（版本 append-only，启用行有
--     历史版本时不得删）；
--   - project_role.current_version_id 可空 FK（项目行创建时由应用层同事务
--     建版本 1.0 并指向它）。
-- 不变量③（稿 §17.2「Task 保留创建时版本引用」）由库保证：delivery.task 新增
-- 可空 ai_role_version_id FK；版本 append-only + RESTRICT ⇒ 引用永不被改写或删除。
-- 种子纪律（0020:50-52）：迁移只种**全局**模板 5 行；项目启用行不在迁移种——
-- 由 bootstrap 建项时播（ensure_project_ai_roles），存量项目由启用接口懒生效。
-- 取号：初取时 0057 已被同批 D-3 迁移占用，按后实施者纪律取当时最大号+1 = 0058。

-- 1) 全局 Role 模板
CREATE TABLE ai.ai_role_template (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role_key           TEXT NOT NULL UNIQUE,
    name               TEXT NOT NULL,
    category           TEXT NOT NULL CHECK (category IN ('management','product','knowledge',
                                                          'architecture','design','engineering',
                                                          'quality','governance')),
    is_system          BOOLEAN NOT NULL DEFAULT TRUE,
    default_definition JSONB NOT NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 2) 不可变 Role Version（append-only；内容快照 = 稿 §4 六组字段裁剪版）
CREATE TABLE ai.ai_role_version (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_role_id UUID NOT NULL,        -- FK 稍后 ALTER 加入（环）
    version         TEXT NOT NULL,
    definition      JSONB NOT NULL,
    created_by      UUID,                 -- 可空 principal
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_role_id, version)
);

-- 3) 项目启用行
CREATE TABLE ai.project_ai_role (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id          UUID NOT NULL REFERENCES core.tenant(id),
    project_id         UUID NOT NULL,
    role_id            UUID NOT NULL REFERENCES ai.ai_role_template(id),
    enabled            BOOLEAN NOT NULL DEFAULT TRUE,
    current_version_id UUID,              -- FK 稍后 ALTER 加入（环）
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, project_id, role_id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES core.project (tenant_id, id)
);

ALTER TABLE ai.ai_role_version
    ADD CONSTRAINT fk_ai_role_version_project_role
    FOREIGN KEY (project_role_id) REFERENCES ai.project_ai_role (id) ON DELETE RESTRICT;

ALTER TABLE ai.project_ai_role
    ADD CONSTRAINT fk_project_ai_role_current_version
    FOREIGN KEY (current_version_id) REFERENCES ai.ai_role_version (id);

-- 4) Task 创建时的版本引用（可空；历史 Task 允许无 Role 锚）
ALTER TABLE delivery.task
    ADD COLUMN ai_role_version_id UUID
        REFERENCES ai.ai_role_version (id);

-- 5) Catalog 首批 5 角色（与现有域对齐；default_definition 为六组裁剪版）
INSERT INTO ai.ai_role_template (role_key, name, category, is_system, default_definition)
VALUES
    ('project_manager', 'AI Project Manager', 'management', TRUE,
     '{"identity":{"role_key":"project_manager","version":"1.0"},"responsibilities":{"responsibilities":["统筹项目计划与里程碑，跟踪交付进度","协调需求、架构、开发、测试四方 AI 角色","向人类 owner 升级关键决策与风险"],"constraints":["不做具体编码与测试执行","关键决策必须升级给人类确认"]},"capabilities":{"capabilities":["task_scheduling","progress_tracking","risk_escalation"]},"contract":{},"governance":{"risk_level":"medium","notes":"AI Project Manager 是管理协调角色，不允许项目禁用（AT-D12/稿§3.3）"},"execution":{"acceptance_criteria":["项目计划与里程碑已建立并可查询","关键风险均有升级记录"]}}'::jsonb),
    ('requirement_analyst', 'Requirement Analyst', 'product', TRUE,
     '{"identity":{"role_key":"requirement_analyst","version":"1.0"},"responsibilities":{"responsibilities":["从访谈与材料中提炼结构化需求","维护需求条目与验收标准的一致性"],"constraints":["不擅自扩大需求范围","模糊需求必须标记为待澄清"]},"capabilities":{"capabilities":["requirement_extraction","acceptance_definition"]},"contract":{},"governance":{"risk_level":"medium"},"execution":{"acceptance_criteria":["每条需求都有明确验收标准","需求条目可追溯到来源"]}}'::jsonb),
    ('architect', 'Solution Architect', 'architecture', TRUE,
     '{"identity":{"role_key":"architect","version":"1.0"},"responsibilities":{"responsibilities":["产出技术方案与架构决策","保证实现与架构一致性"],"constraints":["不直接实现业务代码","架构变更需留下决策记录"]},"capabilities":{"capabilities":["architecture_design","tech_decision"]},"contract":{},"governance":{"risk_level":"medium"},"execution":{"acceptance_criteria":["关键架构决策均有记录","方案与既有架构不冲突"]}}'::jsonb),
    ('developer', 'Developer', 'engineering', TRUE,
     '{"identity":{"role_key":"developer","version":"1.0"},"responsibilities":{"responsibilities":["按任务规格实现代码","为交付物补充测试"],"constraints":["不越过任务范围修改无关代码","不绕过基线直接改主分支"]},"capabilities":{"capabilities":["code_implementation","debugging"]},"contract":{},"governance":{"risk_level":"medium"},"execution":{"acceptance_criteria":["任务范围内的验收标准全部通过","交付物有对应测试"]}}'::jsonb),
    ('tester', 'QA Engineer', 'quality', TRUE,
     '{"identity":{"role_key":"tester","version":"1.0"},"responsibilities":{"responsibilities":["设计并执行验证方案","对失败交付给出可复现证据"],"constraints":["不直接修改业务代码","判定结论必须附证据"]},"capabilities":{"capabilities":["test_design","failure_judgement"]},"contract":{},"governance":{"risk_level":"medium"},"execution":{"acceptance_criteria":["每个交付物都有验证结论","失败判定附可复现证据"]}}'::jsonb)
ON CONFLICT (role_key) DO NOTHING;
