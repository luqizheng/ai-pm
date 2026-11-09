-- 0003_conversation_context_anchor: AI 对话 × 上下文整合 Batch 2——
-- 线程级上下文锚点（建会话时定格，消息级不收）。
--
-- 设计：docs/product/ai-pm-assistant-context-integration-plan-v1.md §三 Batch 2；
-- 裁决 D-IC-1=线程级 / D-IC-3=shell 为真源 / D-IC-2=最小两节注入。
--
-- 语义：
--   NULL = 无锚点（存量全部 NULL，零回填——沿 0002 链接列先例）；
--   非 NULL = 建会话时声明的上下文快照（项目 + 仓库 + 分支）。
--
-- 粒度理由（D-IC-1）：锚点挂在**线程**而非消息上——一线程一上下文，
-- 消息级收锚点会让人为每条消息伪造上下文、且每条可漂移。
--
-- 结构理由：单列 JSONB（不拆 repository_name / branch 两列）——
-- 锚点是整体快照，字段会随注入源演进（Batch 3A 的 S1/S2 两节），
-- 单列演进不改 schema；查询侧不需要按锚点字段检索（只按线程取回）。
--
-- 约束：
--   - 必须是 JSON 对象（防存标量/数组导致后续解析 panic）；
--   - 只允许白名单键（防注入任意结构，防后续被当作指令源）；
--   - tenant_id/project_id 冗余进锚点，与列本身同项目一致（服务端校验）。
--
-- 取号：0002 已被 DK-A 链接列占用，本批取 0003。

ALTER TABLE ai.conversation
    ADD COLUMN context_anchor jsonb NULL;

ALTER TABLE ai.conversation
    ADD CONSTRAINT conversation_context_anchor_object_check
    CHECK (
        context_anchor IS NULL
        OR jsonb_typeof(context_anchor) = 'object'
    );

-- 白名单键：project_id / project_name / repository_name / default_branch /
-- repository_role（与前端 AssistantContextSnapshot 同构）。
--
-- 用 JSONB 减法算子表达「键集 ⊆ 白名单」：把白名单键逐个减去后必须为空
-- 对象（多任何一个键即违反）。比 jsonb_build_object 更纯粹且 immutable
-- （jsonb_build_object 的参数须键值成对，易写错）。
ALTER TABLE ai.conversation
    ADD CONSTRAINT conversation_context_anchor_keys_check
    CHECK (
        context_anchor IS NULL
        OR (
            context_anchor
            - 'project_id'
            - 'project_name'
            - 'repository_name'
            - 'default_branch'
            - 'repository_role'
        ) = '{}'::jsonb
    );

-- 锚点内项目 id 必须与列上的 project_id 一致（防跨项目锚点注入）。
ALTER TABLE ai.conversation
    ADD CONSTRAINT conversation_context_anchor_project_match
    CHECK (
        context_anchor IS NULL
        OR NOT (context_anchor ? 'project_id')
        OR context_anchor ->> 'project_id' = project_id::text
    );
