-- 0058: P0 外部 Commit 采纳闭环（External Commit Adoption Loop）。
--
-- 目标：开发者在外部工具（Codex / Claude Code / Trae / 原生 git）提交 Commit 后，
-- AI-PM 能够发现 → AI 分析 → 澄清 → 人类采纳 → 关闭，全程可追踪。
--
-- 硬约束（任务书 §三）：
-- * 发现 ≠ 采纳：状态机显式分离，不存在任何把 discovered 视为 adopted 的路径。
-- * AI 不自动批准正式变更：采纳决定（approve/no_update/rejected）一律人类主体；
--   知识更新走既有 document_proposal 管线（质量门 + 红线人类确认）。
-- * 不静默覆盖 Markdown+Git 权威正文：采纳建议携带拟稿正文，人类批准前可见；
--   实际写入只经 knowledge_write_coordinator（submit_proposal）。
-- * PostgreSQL 管状态/审批/事件/审计；Git 只管知识正文。
--
-- 状态机 status（CHECK 强制，迁移不可达状态）：
--   discovered        已发现，待 AI 分析
--   analyzed          分析完成（clarification_state ∈ pending）；not_needed 时直接推进
--   awaiting_adoption 澄清闭环已达成（无问题或全部已答复），待人类决定
--   adopted           人类已批准，知识更新提案已提交既有管线
--   closed            终态：提案落定 / 显式无需更新 / 人类拒绝
--   failed            分析失败（attempts/last_error），可重试
--
-- clarification_state 独立维度：
--   not_needed（分析产出零问题——显式记录，不允许「跳过」）
--   pending（存在 open 问题）/ resolved（全部已答复）
--
-- 无存量数据：直接建表（AGENTS.md 规则 1）。

CREATE TABLE integration.external_commit (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    commit_sha text NOT NULL,
    branch text NOT NULL,
    author_name text NOT NULL,
    committed_at timestamp with time zone NOT NULL,
    subject text NOT NULL,
    changed_files jsonb DEFAULT '[]'::jsonb NOT NULL,
    -- changed_files 元素：{"path": "...", "change": "A|M|D|R"}
    status text NOT NULL DEFAULT 'discovered',
    analysis jsonb,
    -- AI 分析结果（恒 draft/unverified 语义，采纳需人类批准）：
    -- {summary, analysis_mode, related_objects[], deviation, knowledge_updates[]}
    analysis_mode text,
    analyzed_at timestamp with time zone,
    clarification_state text NOT NULL DEFAULT 'not_needed',
    adoption_decision text,
    -- approved / no_update / rejected（人类决定）
    adoption_note text,
    adopted_by uuid,
    adopted_at timestamp with time zone,
    knowledge_proposals jsonb DEFAULT '[]'::jsonb NOT NULL,
    -- 采纳后提交的 document_proposal 记录：
    -- [{proposal_id, revision_id?, doc_key, outcome}]
    adopted_revision_ids jsonb DEFAULT '[]'::jsonb NOT NULL,
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    discovered_at timestamp with time zone NOT NULL DEFAULT now(),
    discovered_by uuid,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT uq_external_commit_repo_sha UNIQUE (repository_id, commit_sha),
    CONSTRAINT ck_ec_status CHECK (status = ANY (ARRAY[
        'discovered'::text, 'analyzed'::text, 'awaiting_adoption'::text,
        'adopted'::text, 'closed'::text, 'failed'::text])),
    CONSTRAINT ck_ec_clarification CHECK (clarification_state = ANY (ARRAY[
        'not_needed'::text, 'pending'::text, 'resolved'::text])),
    CONSTRAINT ck_ec_adoption_decision CHECK (adoption_decision IS NULL OR adoption_decision = ANY (ARRAY[
        'approved'::text, 'no_update'::text, 'rejected'::text])),
    CONSTRAINT ck_ec_analysis_mode CHECK (analysis_mode IS NULL OR analysis_mode = ANY (ARRAY[
        'llm'::text, 'derived'::text])),
    -- 决定四元组完整性：有决定必须有人和时间；无决定必须都为空。
    CONSTRAINT ck_ec_decision_complete CHECK (
        (adoption_decision IS NULL) = (adopted_by IS NULL AND adopted_at IS NULL)),
    -- 采纳必须带决定；closed 必须带决定（closed = no_update/rejected 落定，
    -- 或 adopted 后提案全部终态）。
    CONSTRAINT ck_ec_adopted_needs_decision CHECK (
        status NOT IN ('adopted', 'closed') OR adoption_decision IS NOT NULL),
    -- failed 必须带错误与尝试次数。
    CONSTRAINT ck_ec_failed_has_error CHECK (
        status <> 'failed' OR (last_error IS NOT NULL AND attempts > 0)),
    -- 发现 ≠ 采纳：任何时刻 adopted/closed 都要求 analysis 已落（结构保证
    -- 「未经分析不得采纳」）。
    CONSTRAINT ck_ec_analyzed_before_adopt CHECK (
        status NOT IN ('awaiting_adoption', 'adopted', 'closed')
        OR (analysis IS NOT NULL AND analyzed_at IS NOT NULL)),
    CONSTRAINT fk_ec_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE integration.external_commit
    ADD CONSTRAINT external_commit_pkey PRIMARY KEY (id);
CREATE INDEX idx_ec_project ON integration.external_commit (project_id, status, committed_at DESC);
CREATE INDEX idx_ec_repository ON integration.external_commit (repository_id, committed_at DESC);

COMMENT ON CONSTRAINT uq_external_commit_repo_sha ON integration.external_commit IS
    '防止重复发现：同仓库同 commit 唯一；重复扫描 ON CONFLICT DO NOTHING。';
COMMENT ON CONSTRAINT ck_ec_analyzed_before_adopt ON integration.external_commit IS
    '发现 ≠ 采纳：进入澄清/采纳/关闭态之前必须已有 AI 分析结果。';

CREATE TABLE integration.external_commit_clarification (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    external_commit_id uuid NOT NULL,
    project_id uuid NOT NULL,
    question text NOT NULL,
    answer text,
    answered_by uuid,
    answered_at timestamp with time zone,
    status text NOT NULL DEFAULT 'open',
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_ecc_status CHECK (status = ANY (ARRAY['open'::text, 'answered'::text])),
    CONSTRAINT ck_ecc_answered_complete CHECK (
        status <> 'answered'
        OR (answer IS NOT NULL AND answered_by IS NOT NULL AND answered_at IS NOT NULL)),
    CONSTRAINT fk_ecc_commit FOREIGN KEY (external_commit_id)
        REFERENCES integration.external_commit(id) ON DELETE CASCADE
);

ALTER TABLE integration.external_commit_clarification
    ADD CONSTRAINT external_commit_clarification_pkey PRIMARY KEY (id);
CREATE INDEX idx_ecc_commit ON integration.external_commit_clarification (external_commit_id, status);

COMMENT ON TABLE integration.external_commit_clarification IS
    '外部 Commit 澄清问答：AI 分析产出的不确定事项逐条落行；人类逐条答复；
     全部 answered 时主行 clarification_state=resolved。零问题走
     clarification_state=not_needed 显式记录，不允许跳过记录。';

-- 发现锚点：每次扫描成功的远端 HEAD。首次扫描无锚点时以 Confirmed 基准
-- （selected_commit）为起点；两者皆无 → 首扫只建锚不发现（避免把既有历史
-- 全量灌进采纳队列）。
ALTER TABLE integration.project_repository
    ADD COLUMN last_external_scan_commit_sha text,
    ADD COLUMN last_external_scan_at timestamp with time zone;

COMMENT ON COLUMN integration.project_repository.last_external_scan_commit_sha IS
    '外部 Commit 发现扫描锚点（上次成功扫描的远端 HEAD）；发现区间 = 锚..HEAD。';
