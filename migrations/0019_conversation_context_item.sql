-- 0019：会话级对象上下文条目（AI Collaboration V1.0 · DR-C7 / 后端契约 CX1）
--
-- 用途：AI Collaboration Workspace V1.0 的 **AI-02 对象级 Context** —— 会话可用的
-- 工作上下文条目（Requirement / Design / ADR / Knowledge / Document / Task）。
--
-- 设计口径（架构评审 2026-11-11 裁定）：
--   1. **独立表**——不扩展 `ai.conversation.context_anchor` JSONB。锚点职责是
--      Project / Repository **线程作用域定位**；对象级 Context 有独立生命周期、
--      启停操作、版本引用与历史审计，两者生命周期不同，不混同一 JSONB。
--   2. **只存定位与版本引用，不存正文**——正文与状态一律服务端重读
--      （SQL-07：注入时重新读取被引用修订并复核版本与 content_hash）。
--   3. **双轴分列**：`attachment`（attached / detached）与
--      `lifecycle`（active / inactive / pending / unavailable）分列；
--      `read_only` 是**可写性属性**，独立于生命周期，不随状态流转切换。
--      **Available 不是存储态**——它是候选对象查询（context-candidates）的结果。
--   4. **软移除**（SQL-04）：移除 = 从当前 Conversation 移除（attachment='detached'），
--      不删原始业务对象、不丢版本引用与操作记录；移除事件进 History。
--   5. **归属字段服从既有属主隔离模式**：照 `ai.conversation_interaction` 的做法——
--      保留 tenant_id / project_id 列，FK 只钉 conversation_id，**不建** (tenant,project,id)
--      三元组唯一键；「item 与其 conversation 同租户同项目」由**服务层 join 校验**
--      （SQL-06），跨属主访问按既有先例 **404 同形**。
--   6. **版本引用按类型解释**（SQL-01）：不同对象域的版本载体不同
--      （requirement/design/adr 有不可变修订；knowledge 有 version；document 有
--      version_number + checksum；**task 只有乐观锁行版本**）。没有正式内容版本的
--      对象**不得伪造内容版本**——故版本列可空，另设 `row_version` 专记乐观锁。
--
-- 迁移号：0019（0018 已被 team_llm_catalog 占用）。

CREATE TABLE ai.conversation_context_item (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    -- ── 对象定位（多态；**不存正文**）──────────────────────────────
    object_type text NOT NULL,
    object_id text NOT NULL,
    object_code text,
    -- ── 版本引用（**按类型解释**，见 SQL-01；无内容版本的对象留空）────
    revision_id uuid,             -- requirement_revision.id / design_revision.id / …
    version integer,              -- 该修订的版本号（不得为乐观锁伪造）
    content_hash text,             -- 引用时内容哈希（注入时服务端复核，SQL-07）
    row_version bigint,            -- 仅乐观锁类对象（如 task）记此，**不参与版本语义**
    -- ── 归属轴（**只有两态**；Available 是候选查询结果，不入库）────────
    attachment text NOT NULL DEFAULT 'attached',
    -- ── 生命周期轴 ────────────────────────────────────────────────
    lifecycle text NOT NULL DEFAULT 'active',
    -- ── 可写性（**独立于 lifecycle**，不随状态流转切换）──────────────
    read_only boolean NOT NULL DEFAULT false,
    -- ── 来源与不可用原因 ─────────────────────────────────────────
    source text NOT NULL,
    unavailable_reason text,
    added_by uuid NOT NULL,
    added_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_context_item_pk PRIMARY KEY (id),
    CONSTRAINT conversation_context_item_object_type_check CHECK (
        object_type = ANY (ARRAY['requirement'::text, 'design'::text, 'adr'::text,
                            'knowledge'::text, 'document'::text, 'task'::text])
    ),
    CONSTRAINT conversation_context_item_attachment_check CHECK (
        attachment = ANY (ARRAY['attached'::text, 'detached'::text])
    ),
    CONSTRAINT conversation_context_item_lifecycle_check CHECK (
        lifecycle = ANY (ARRAY['active'::text, 'inactive'::text, 'pending'::text,
                            'unavailable'::text])
    ),
    CONSTRAINT conversation_context_item_source_check CHECK (
        source = ANY (ARRAY['page'::text, 'manual'::text, 'discovery'::text,
                           'ai_suggestion'::text])
    ),
    -- 离开 unavailable 时原因须清空（不得留误导性当前原因）
    CONSTRAINT conversation_context_item_unavailable_reason_check CHECK (
        (lifecycle = 'unavailable') = (unavailable_reason IS NOT NULL)
    ),
    -- 内容版本号必须为正（乐观锁行版本另计）
    CONSTRAINT conversation_context_item_version_check CHECK (
        (version IS NULL) OR (version > 0)
    ),
    -- 同一会话不重复附加同一对象；revision_id 可空 → NULLS NOT DISTINCT（PG16），
    -- 否则普通 UNIQUE 会放行「同对象多行 NULL 组合」。
    CONSTRAINT conversation_context_item_uniq
        UNIQUE NULLS NOT DISTINCT (conversation_id, object_type, object_id, revision_id),
    CONSTRAINT conversation_context_item_fk_conversation
        FOREIGN KEY (conversation_id) REFERENCES ai.conversation (id)
);

CREATE INDEX idx_conversation_context_item_conversation
    ON ai.conversation_context_item (conversation_id, attachment, lifecycle);
