-- 0050 · PA-05 统一事项基础模型与 Activity Timeline（2026-11-21；裁决=注册表形态）
--
-- PA-05-01：统一基础模型 + 独立领域对象——core.work_item 为注册表
-- （统一事项 ID = 主键；基础描述/创建信息/关联/活动历史挂载点），
-- Requirement/Task/Design(ADR) 等领域对象保留独立表与状态机。
-- PA-05-02：统一状态语义 = 跨领域可理解的过滤/待办维度（open/in_progress/
-- blocked/in_review/done/cancelled/failed/archived），映射在各领域显式
-- 声明（同名状态不默认同义）。
-- PA-05-03/04：统一关联多对多 + 关系类型；来源 human|agent + 理由；
-- 关联不改变对象归属；Agent 不得覆盖人类明确修改的关系（V1 无 agent
-- 删除通道，删除一律记 relation_changed 活动）。
-- PA-05-05/06/07：Activity Timeline 追加式（无 UPDATE 路径，纠错=追加
-- correction），区分 human|agent 来源，支持 事项/Product/Project/时间
-- 四维查看；Execution Logs（工具调用/命令执行等）归 delivery.task_run_event，
-- 不混入 Timeline；正式知识正文权威 = Markdown + Git，本表仅存版本引用。
-- PA-05-09：已读状态 per-user（Read/Notified 两列；Processed/Approved
-- 在领域对象自身状态，不混此表）。

CREATE TABLE core.work_item (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    target_type text NOT NULL,
    target_id uuid NOT NULL,
    project_id uuid,
    title text NOT NULL,
    status text NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_work_item_target CHECK (
        target_type = ANY (ARRAY['task'::text, 'requirement'::text, 'adr'::text])
    ),
    CONSTRAINT ck_work_item_status CHECK (
        status = ANY (ARRAY['open'::text, 'in_progress'::text, 'blocked'::text,
                            'in_review'::text, 'done'::text, 'cancelled'::text,
                            'failed'::text, 'archived'::text])
    ),
    CONSTRAINT fk_work_item_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.work_item ADD CONSTRAINT work_item_pkey PRIMARY KEY (id);

CREATE UNIQUE INDEX uq_work_item_target ON core.work_item USING btree (tenant_id, target_type, target_id);
CREATE INDEX ix_work_item_project ON core.work_item USING btree (tenant_id, project_id);

CREATE TABLE core.work_item_relation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    source_item_id uuid NOT NULL,
    target_item_id uuid NOT NULL,
    relation_type text NOT NULL,
    origin text NOT NULL DEFAULT 'human'::text,
    reason text,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_work_item_relation_type CHECK (
        relation_type = ANY (ARRAY['implements'::text, 'supports'::text, 'affects'::text,
                                   'blocks'::text, 'derived_from'::text, 'references'::text])
    ),
    CONSTRAINT ck_work_item_relation_origin CHECK (
        origin = ANY (ARRAY['human'::text, 'agent'::text])
    ),
    CONSTRAINT ck_work_item_relation_no_self CHECK (source_item_id <> target_item_id),
    CONSTRAINT fk_work_item_relation_source FOREIGN KEY (source_item_id) REFERENCES core.work_item(id),
    CONSTRAINT fk_work_item_relation_target FOREIGN KEY (target_item_id) REFERENCES core.work_item(id),
    CONSTRAINT fk_work_item_relation_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.work_item_relation ADD CONSTRAINT work_item_relation_pkey PRIMARY KEY (id);

CREATE UNIQUE INDEX uq_work_item_relation
    ON core.work_item_relation USING btree (source_item_id, target_item_id, relation_type);

CREATE TABLE core.work_item_activity (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    item_id uuid NOT NULL,
    project_id uuid,
    product_id uuid,
    activity_type text NOT NULL,
    actor_type text NOT NULL,
    actor_id uuid,
    summary text NOT NULL,
    detail jsonb,
    revision_ref jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ck_work_item_activity_actor CHECK (
        actor_type = ANY (ARRAY['human'::text, 'agent'::text])
    ),
    CONSTRAINT fk_work_item_activity_item FOREIGN KEY (item_id) REFERENCES core.work_item(id),
    CONSTRAINT fk_work_item_activity_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.work_item_activity ADD CONSTRAINT work_item_activity_pkey PRIMARY KEY (id);

-- PA-05-05 四维查看（事项 / Product / Project / 时间）
CREATE INDEX ix_work_item_activity_item
    ON core.work_item_activity USING btree (tenant_id, item_id, created_at DESC);
CREATE INDEX ix_work_item_activity_project
    ON core.work_item_activity USING btree (tenant_id, project_id, created_at DESC);
CREATE INDEX ix_work_item_activity_product
    ON core.work_item_activity USING btree (tenant_id, product_id, created_at DESC);

CREATE TABLE core.work_item_read_state (
    tenant_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    item_id uuid NOT NULL,
    read_at timestamp with time zone,
    notified_at timestamp with time zone,
    CONSTRAINT fk_work_item_read_state_item FOREIGN KEY (item_id) REFERENCES core.work_item(id),
    CONSTRAINT fk_work_item_read_state_tenant FOREIGN KEY (tenant_id) REFERENCES core.tenant(id)
);

ALTER TABLE core.work_item_read_state ADD CONSTRAINT work_item_read_state_pkey PRIMARY KEY (principal_id, item_id);
