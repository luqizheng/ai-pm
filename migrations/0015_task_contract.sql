-- 0015_task_contract: 执行契约字段（D2）
-- 依据：docs/ai-pm-slice-d2-mcp-context-v1.md §四 §五（v1.1，含评审修订 R-F8/F9/F10/G-7）
--
-- 内容：
--   1) delivery.task 契约四列：goal / constraints / deliverables / git_branch
--   2) delivery.task_acceptance_criterion：逐条验收（独立表，D5 逐条裁决并存证据）
--   3) delivery.task_knowledge_ref：任务 → 知识文档引用（C2：以 document_id 为准）
--   4) delivery.task_run 执行端身份与租约凭据三列
--
-- 原则：forward-only；全部可空或有默认值；不改既有列语义。

-- 1) 任务契约（目标 / 约束 / 产物 / 平台生成的分支名）
--    deliverables 为 JSONB 对象数组（[{id, path}]）：D4 交付对账需按条目 id 锚定
--    证据，TEXT[] 无法引用单个条目（评审 R-F10）。
ALTER TABLE delivery.task
    ADD COLUMN IF NOT EXISTS goal         TEXT,
    ADD COLUMN IF NOT EXISTS constraints  JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS deliverables JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS git_branch   TEXT;

-- 2) 逐条验收（C 裁决：独立表，D5 逐条裁决并存证据）
--    ordinal 允许间隙（仅作排序，不要求连续；中间插入用 max+10，禁止全量重排——
--    评审 R-F8）。updated_at 记录条目修订时间；若 D5 需完整版本化留痕，
--    forward-only 追加迁移补。
CREATE TABLE IF NOT EXISTS delivery.task_acceptance_criterion (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id       UUID NOT NULL,
    task_id          UUID NOT NULL,
    ordinal          INT  NOT NULL CHECK (ordinal >= 1),
    text             TEXT NOT NULL CHECK (length(btrim(text)) > 0),
    verification_kind TEXT NOT NULL DEFAULT 'manual'
        CHECK (verification_kind IN ('manual', 'automated_test', 'artifact_exists', 'review')),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (task_id, ordinal),
    -- 预留 D5 证据表按 (project_id, id) 组合外键锚定单条验收项（与 0006/0011 同款）。
    UNIQUE (project_id, id),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE
);

-- 3) 知识引用（C2：以 document_id 为准，路径仅作展示）
CREATE TABLE IF NOT EXISTS delivery.task_knowledge_ref (
    project_id   UUID NOT NULL,
    task_id      UUID NOT NULL,
    document_id  UUID NOT NULL,
    is_required  BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (task_id, document_id),
    FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, document_id) REFERENCES ai.project_document(project_id, id)
);

-- 4) run 级凭据（D 裁决）：只存哈希，明文在 claim 时一次性返回。
--    编码钉死（评审 R-G7）：明文 token = 32 字节随机数的小写 hex（64 字符）；
--    lease_token_hash = sha256(明文 token 字符串) 的小写 hex（64 字符），
--    严格匹配（大写变体拒绝）。校验一律按 run_id 主键取行后比对 hash
--    （不存在 token 反查场景），故无需 token 索引（评审 R-F9）。
ALTER TABLE delivery.task_run
    ADD COLUMN IF NOT EXISTS lease_token_hash TEXT,
    ADD COLUMN IF NOT EXISTS client_name      TEXT,
    ADD COLUMN IF NOT EXISTS client_version   TEXT;

-- 平台专用 service principal（评审 R-F7）：审计回退链的最终落点——项目无
-- human 成员时，claim/run 写审计不再 RowNotFound；after_data 强制携带
-- client_name / lease_owner 保留归因。幂等种子（重跑不重复插入）。
INSERT INTO core.principal (id, kind, display_name)
VALUES ('00000000-0000-0000-0000-000000000001', 'service', 'ai-pm-dispatch')
ON CONFLICT (id) DO NOTHING;
