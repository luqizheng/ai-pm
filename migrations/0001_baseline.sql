-- ============================================================================
-- 0001_baseline.sql —— 全量 schema 基线（squash 产物，非手工编写，勿手改）
--
-- 来源：2026-11-09 将旧链 0001~0065（64 文件，0029 历史空号）在冻结点合并。
-- 生成方式：完整旧链建库终态 → pg_dump --schema-only + --data-only --inserts
--   提取，剥除 psql 元命令（\restrict）；对象名全限定，函数体运行时语义与
--   旧链一致。等价性验证：schema/data pg_dump 逐行 diff 为零。
-- 台账：docs/reference/ai-pm-migration-ledger.md；后续新迁移自 0002 起编号。
-- ============================================================================
-- 序头：pg_dump 原生序头的 session 级 SET（pg_dump 依赖序不含 PL/pgSQL 函数
-- 体内引用，必须关掉建函数时的语法层校验，否则体内引用先于表创建即 42P01）。
-- 脚注处全部复位——本迁移在 sqlx 连接池连接上执行，session 状态不得外泄。
-- ============================================================================
SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: ai; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA ai;


--
-- Name: core; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA core;


--
-- Name: delivery; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA delivery;


--
-- Name: devenv; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA devenv;


--
-- Name: discovery; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA discovery;


--
-- Name: governance; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA governance;


--
-- Name: integration; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA integration;


--
-- Name: kb; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA kb;


--
-- Name: pc; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA pc;


--
-- Name: public; Type: SCHEMA; Schema: -; Owner: -
--

-- *not* creating schema, since initdb creates it


--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON SCHEMA public IS '';


--
-- Name: source; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA source;


--
-- Name: pgcrypto; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;


--
-- Name: EXTENSION pgcrypto; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';


--
-- Name: forbid_review_approval_mutation(); Type: FUNCTION; Schema: delivery; Owner: -
--

CREATE FUNCTION delivery.forbid_review_approval_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (OLD.task_id IS DISTINCT FROM NEW.task_id)
       OR (OLD.review_run_id IS DISTINCT FROM NEW.review_run_id)
       OR (OLD.delivery_run_id IS DISTINCT FROM NEW.delivery_run_id)
       OR (OLD.delivery_manifest_hash IS DISTINCT FROM NEW.delivery_manifest_hash)
       OR (OLD.kind IS DISTINCT FROM NEW.kind)
       OR (OLD.required_role IS DISTINCT FROM NEW.required_role)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'review approval % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'pending' THEN
        IF NEW.status NOT IN ('approved', 'rejected') THEN
            RAISE EXCEPTION 'review approval % may only move from pending to a terminal status', OLD.id;
        END IF;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'review approval % is already decided and cannot change status', OLD.id;
    END IF;
    RETURN NEW;
END $$;


--
-- Name: forbid_review_report_update(); Type: FUNCTION; Schema: delivery; Owner: -
--

CREATE FUNCTION delivery.forbid_review_report_update() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.run_kind = 'review' THEN
        IF (NEW.run_kind IS DISTINCT FROM OLD.run_kind)
           OR (OLD.role_id IS DISTINCT FROM NEW.role_id)
           OR (OLD.input_manifest_hash IS DISTINCT FROM NEW.input_manifest_hash)
           OR (OLD.reviewed_manifest_hash IS DISTINCT FROM NEW.reviewed_manifest_hash)
           OR (OLD.review_policy_version IS DISTINCT FROM NEW.review_policy_version)
           OR (OLD.review_verdict IS NOT NULL
               AND OLD.review_verdict IS DISTINCT FROM NEW.review_verdict) THEN
            RAISE EXCEPTION 'review report columns of run % are immutable', OLD.id;
        END IF;
    END IF;
    RETURN NEW;
END $$;


--
-- Name: forbid_row_update(); Type: FUNCTION; Schema: delivery; Owner: -
--

CREATE FUNCTION delivery.forbid_row_update() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'row % in %.% is immutable',
        OLD.id, TG_TABLE_SCHEMA, TG_TABLE_NAME;
END $$;


--
-- Name: reject_mutation_of_failure_record(); Type: FUNCTION; Schema: delivery; Owner: -
--

CREATE FUNCTION delivery.reject_mutation_of_failure_record() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'delivery.task_run_failure is append-only';
END
$$;


--
-- Name: notify_audit_insert(); Type: FUNCTION; Schema: governance; Owner: -
--

CREATE FUNCTION governance.notify_audit_insert() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    PERFORM pg_notify(
        'pm_audit',
        json_build_object(
            'id', NEW.id::text,
            'project_id', NEW.project_id::text,
            'actor_id', NEW.actor_id::text,
            'action', NEW.action,
            'target_type', NEW.target_type,
            'target_id', NEW.target_id::text,
            'created_at', to_char(NEW.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')
        )::text
    );
    RETURN NEW;
END;
$$;


--
-- Name: guard_memory_revision(); Type: FUNCTION; Schema: kb; Owner: -
--

CREATE FUNCTION kb.guard_memory_revision() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'kb.memory_revision is append-only (L1 versions are never physically deleted)';
    END IF;
    IF TG_OP = 'INSERT' THEN
        -- 审核门入口：一切内容变化必须先进待审核区。
        IF NEW.review_status <> 'PENDING_REVIEW' OR NEW.version_status IS NOT NULL
           OR NEW.version_no IS NOT NULL THEN
            RAISE EXCEPTION 'kb.memory_revision must be inserted as PENDING_REVIEW without version state (L1 review gate)';
        END IF;
        RETURN NEW;
    END IF;
    -- UPDATE：内容列冻结。
    IF NEW.id <> OLD.id OR NEW.tenant_id <> OLD.tenant_id OR NEW.memory_id <> OLD.memory_id
       OR NEW.revision_no <> OLD.revision_no OR NEW.change_action <> OLD.change_action
       OR NEW.base_revision_id IS DISTINCT FROM OLD.base_revision_id
       OR NEW.memory_type <> OLD.memory_type OR NEW.title <> OLD.title
       OR NEW.summary <> OLD.summary OR NEW.body IS DISTINCT FROM OLD.body
       OR NEW.change_reason <> OLD.change_reason OR NEW.content_hash <> OLD.content_hash
       OR NEW.proposed_by <> OLD.proposed_by OR NEW.created_at <> OLD.created_at THEN
        RAISE EXCEPTION 'kb.memory_revision content is immutable';
    END IF;
    -- 审核结论单向一次。
    IF OLD.review_status <> 'PENDING_REVIEW' AND (
           NEW.review_status <> OLD.review_status
        OR NEW.reviewed_by IS DISTINCT FROM OLD.reviewed_by
        OR NEW.reviewed_at IS DISTINCT FROM OLD.reviewed_at
        OR NEW.review_note IS DISTINCT FROM OLD.review_note) THEN
        RAISE EXCEPTION 'kb.memory_revision review decision is final';
    END IF;
    -- 版本号一经分配不可改。
    IF OLD.version_no IS NOT NULL AND NEW.version_no IS DISTINCT FROM OLD.version_no THEN
        RAISE EXCEPTION 'kb.memory_revision version_no is immutable once assigned';
    END IF;
    -- 版本状态迁移白名单。
    IF NEW.version_status IS DISTINCT FROM OLD.version_status THEN
        IF NOT (
               (OLD.version_status IS NULL
                AND NEW.version_status IN ('CURRENT','WITHDRAWN')
                AND OLD.review_status = 'PENDING_REVIEW'
                AND NEW.review_status = 'APPROVED')
            OR (OLD.version_status = 'CURRENT' AND NEW.version_status = 'SUPERSEDED')
        ) THEN
            RAISE EXCEPTION 'kb.memory_revision illegal version_status transition % -> %',
                coalesce(OLD.version_status, 'NULL'), coalesce(NEW.version_status, 'NULL');
        END IF;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: reject_immutable_mutation(); Type: FUNCTION; Schema: kb; Owner: -
--

CREATE FUNCTION kb.reject_immutable_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION '%.% is immutable (append-only; drop trigger to migrate)',
        TG_TABLE_SCHEMA, TG_TABLE_NAME;
END;
$$;


--
-- Name: reject_revision_mutation(); Type: FUNCTION; Schema: kb; Owner: -
--

CREATE FUNCTION kb.reject_revision_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'kb.document_revision is append-only (drop trigger to migrate)';
END;
$$;


--
-- Name: enforce_member_endpoint_closure(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.enforce_member_endpoint_closure() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
    broken INT;
    v_tenant UUID; v_project UUID; v_version UUID;
BEGIN
    v_tenant  := COALESCE(NEW.tenant_id,  OLD.tenant_id);
    v_project := COALESCE(NEW.project_id, OLD.project_id);
    v_version := COALESCE(NEW.version_id, OLD.version_id);

    SELECT count(*) INTO broken
    FROM pc.version_member vm
    JOIN pc.relationship_revision rr
      ON (rr.tenant_id, rr.project_id, rr.relationship_id, rr.id)
       = (vm.tenant_id, vm.project_id, vm.relationship_id, vm.relationship_revision_id)
    WHERE (vm.tenant_id, vm.project_id, vm.version_id) = (v_tenant, v_project, v_version)
      AND ((rr.source_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM pc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.source_object_id
                  AND m2.object_revision_id = rr.source_object_revision_id))
        OR (rr.target_object_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM pc.version_member m2
                WHERE (m2.tenant_id, m2.project_id, m2.version_id)
                    = (v_tenant, v_project, v_version)
                  AND m2.object_id = rr.target_object_id
                  AND m2.object_revision_id = rr.target_object_revision_id)));

    IF broken > 0 THEN
        RAISE EXCEPTION 'version % has % relationship member(s) with endpoints missing from the version',
            v_version, broken;
    END IF;
    RETURN NULL;
END;
$$;


--
-- Name: enforce_object_lifecycle(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.enforce_object_lifecycle() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'pc.cognition_object % is terminal (%)', OLD.id, OLD.lifecycle_status;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded object % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: enforce_proposal_member_anchored(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.enforce_proposal_member_anchored() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE p_status TEXT;
BEGIN
    SELECT status INTO p_status FROM pc.proposal
     WHERE tenant_id = COALESCE(NEW.tenant_id, OLD.tenant_id)
       AND id = COALESCE(NEW.proposal_id, OLD.proposal_id);
    IF p_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'proposal % is %; members are anchored by proposal_content_hash',
            COALESCE(NEW.proposal_id, OLD.proposal_id), p_status;
    END IF;
    RETURN NULL;
END;
$$;


--
-- Name: enforce_rel_revision_endpoint_match(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.enforce_rel_revision_endpoint_match() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE r_row pc.relationship%ROWTYPE;
BEGIN
    SELECT * INTO r_row FROM pc.relationship
     WHERE tenant_id = NEW.tenant_id AND project_id = NEW.project_id
       AND id = NEW.relationship_id;
    IF (r_row.relationship_type_key IS DISTINCT FROM NEW.relationship_type_key)
       OR (r_row.source_object_id IS DISTINCT FROM NEW.source_object_id)
       OR (r_row.target_object_id IS DISTINCT FROM NEW.target_object_id)
       OR (r_row.source_ext_domain IS DISTINCT FROM NEW.source_ext_domain)
       OR (r_row.source_ext_type IS DISTINCT FROM NEW.source_ext_type)
       OR (r_row.source_ext_id IS DISTINCT FROM NEW.source_ext_id)
       OR (r_row.target_ext_domain IS DISTINCT FROM NEW.target_ext_domain)
       OR (r_row.target_ext_type IS DISTINCT FROM NEW.target_ext_type)
       OR (r_row.target_ext_id IS DISTINCT FROM NEW.target_ext_id) THEN
        RAISE EXCEPTION 'relationship_revision % endpoint identity does not match relationship %',
            NEW.id, NEW.relationship_id;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: enforce_relationship_anchors(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.enforce_relationship_anchors() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (OLD.relationship_type_key IS DISTINCT FROM NEW.relationship_type_key)
       OR (OLD.source_object_id IS DISTINCT FROM NEW.source_object_id)
       OR (OLD.target_object_id IS DISTINCT FROM NEW.target_object_id)
       OR (OLD.source_ext_domain IS DISTINCT FROM NEW.source_ext_domain)
       OR (OLD.source_ext_type IS DISTINCT FROM NEW.source_ext_type)
       OR (OLD.source_ext_id IS DISTINCT FROM NEW.source_ext_id)
       OR (OLD.target_ext_domain IS DISTINCT FROM NEW.target_ext_domain)
       OR (OLD.target_ext_type IS DISTINCT FROM NEW.target_ext_type)
       OR (OLD.target_ext_id IS DISTINCT FROM NEW.target_ext_id) THEN
        RAISE EXCEPTION 'pc.relationship % endpoint anchors are immutable', OLD.id;
    END IF;
    IF OLD.lifecycle_status IN ('deprecated','superseded')
       AND NEW.lifecycle_status IS DISTINCT FROM OLD.lifecycle_status THEN
        RAISE EXCEPTION 'pc.relationship % is terminal', OLD.id;
    END IF;
    IF NEW.lifecycle_status = 'superseded' AND NEW.superseded_by_id IS NULL THEN
        RAISE EXCEPTION 'superseded relationship % must reference its successor', NEW.id;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: enforce_write_op_state(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.enforce_write_op_state() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status IN ('completed','abandoned') AND NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'pc.write_operation % is terminal (%)', OLD.id, OLD.status;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: forbid_approval_mutation(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.forbid_approval_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (OLD.proposal_id IS DISTINCT FROM NEW.proposal_id)
       OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
       OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'pc.approval % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'pending' THEN
        IF NEW.status NOT IN ('approved','rejected') THEN
            RAISE EXCEPTION 'pc.approval % may only move from pending to a terminal status', OLD.id;
        END IF;
        IF NEW.decided_by IS NULL OR NEW.decided_at IS NULL THEN
            RAISE EXCEPTION 'pc.approval % terminal status requires decider and time', OLD.id;
        END IF;
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        RAISE EXCEPTION 'pc.approval % is already decided and cannot change status', OLD.id;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: forbid_identity_delete(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.forbid_identity_delete() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.lifecycle_status <> 'identity' THEN
        RAISE EXCEPTION 'pc.% % is not identity; deletion is forbidden', TG_TABLE_NAME, OLD.id;
    END IF;
    RETURN OLD;
END;
$$;


--
-- Name: forbid_proposal_content_mutation(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.forbid_proposal_content_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF OLD.status <> 'draft' THEN
        IF (OLD.title IS DISTINCT FROM NEW.title)
           OR (OLD.rationale IS DISTINCT FROM NEW.rationale)
           OR (OLD.origin_kind IS DISTINCT FROM NEW.origin_kind)
           OR (OLD.governance_level IS DISTINCT FROM NEW.governance_level)
           OR (OLD.expected_version_no IS DISTINCT FROM NEW.expected_version_no)
           OR (OLD.proposal_content_hash IS DISTINCT FROM NEW.proposal_content_hash)
           OR (OLD.source_id IS DISTINCT FROM NEW.source_id) THEN
            RAISE EXCEPTION 'pc.proposal % is %; content anchor columns are immutable', OLD.id, OLD.status;
        END IF;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: forbid_row_update(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.forbid_row_update() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    RAISE EXCEPTION 'row % in %.% is immutable', OLD.id, TG_TABLE_SCHEMA, TG_TABLE_NAME;
END;
$$;


--
-- Name: forbid_version_mutation(); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.forbid_version_mutation() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (OLD.version_no IS DISTINCT FROM NEW.version_no)
       OR (OLD.confirmed_by IS DISTINCT FROM NEW.confirmed_by)
       OR (OLD.confirmed_at IS DISTINCT FROM NEW.confirmed_at)
       OR (OLD.source_proposal_id IS DISTINCT FROM NEW.source_proposal_id)
       OR (OLD.snapshot_hash IS DISTINCT FROM NEW.snapshot_hash)
       OR (OLD.predecessor_version_id IS DISTINCT FROM NEW.predecessor_version_id)
       OR (OLD.created_at IS DISTINCT FROM NEW.created_at) THEN
        RAISE EXCEPTION 'pc.cognition_version % anchor columns are immutable', OLD.id;
    END IF;
    IF OLD.status = 'published' AND NEW.status NOT IN ('published','archived') THEN
        RAISE EXCEPTION 'pc.cognition_version % status may only move published -> archived', OLD.id;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: object_type_singleton(text); Type: FUNCTION; Schema: pc; Owner: -
--

CREATE FUNCTION pc.object_type_singleton(key text) RETURNS boolean
    LANGUAGE sql IMMUTABLE
    AS $$ SELECT key IN ('product_profile','product_vision') $$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: agent_definition; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.agent_definition (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    code text NOT NULL,
    display_name text NOT NULL,
    role_kind text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT agent_definition_role_kind_check CHECK ((role_kind = ANY (ARRAY['architect'::text, 'developer'::text, 'tester'::text])))
);


--
-- Name: agent_feedback; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.agent_feedback (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    recipient_id uuid NOT NULL,
    related_event_id uuid NOT NULL,
    feedback_type text NOT NULL,
    content jsonb NOT NULL,
    status text DEFAULT 'available'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: ai_executor_capability; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.ai_executor_capability (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    capability text NOT NULL,
    source text DEFAULT 'agent_definition'::text NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ai_executor_capability_source_check CHECK ((source = ANY (ARRAY['agent_definition'::text, 'manual'::text])))
);


--
-- Name: ai_role_template; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.ai_role_template (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    role_key text NOT NULL,
    name text NOT NULL,
    category text NOT NULL,
    is_system boolean DEFAULT true NOT NULL,
    default_definition jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ai_role_template_category_check CHECK ((category = ANY (ARRAY['management'::text, 'product'::text, 'knowledge'::text, 'architecture'::text, 'design'::text, 'engineering'::text, 'quality'::text, 'governance'::text])))
);


--
-- Name: ai_role_version; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.ai_role_version (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_role_id uuid NOT NULL,
    version text NOT NULL,
    definition jsonb NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: analysis_run; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.analysis_run (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    event_id uuid NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    model_name text NOT NULL,
    policy_version integer NOT NULL,
    config_snapshot jsonb NOT NULL,
    context_snapshot jsonb,
    self_review jsonb,
    result jsonb,
    failure_code text,
    attempt_count integer DEFAULT 0 NOT NULL,
    started_at timestamp with time zone,
    finished_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: baseline_item; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.baseline_item (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    category text NOT NULL,
    content text NOT NULL,
    evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    state text DEFAULT 'proposed'::text NOT NULL,
    decided_by uuid,
    decided_at timestamp with time zone,
    decision_note text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT baseline_item_category_check CHECK ((category = ANY (ARRAY['identity'::text, 'scope'::text, 'technical'::text, 'architecture'::text, 'repository'::text, 'standards'::text, 'constraints'::text]))),
    CONSTRAINT baseline_item_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT baseline_item_state_check CHECK ((state = ANY (ARRAY['proposed'::text, 'confirmed'::text, 'rejected'::text, 'investigation'::text, 'deferred'::text])))
);


--
-- Name: baseline_proposal; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.baseline_proposal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    source_run_id uuid,
    confirmed_by uuid,
    confirmed_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT baseline_proposal_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'confirmed'::text, 'superseded'::text])))
);


--
-- Name: change_proposal; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.change_proposal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    analysis_run_id uuid NOT NULL,
    proposal_seq integer NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    expected_revision bigint,
    command_type text NOT NULL,
    proposed_data jsonb NOT NULL,
    rationale text NOT NULL,
    evidence jsonb DEFAULT '[]'::jsonb NOT NULL,
    risk_level text NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    status text DEFAULT 'proposed'::text NOT NULL,
    executed_at timestamp with time zone,
    executed_audit_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT change_proposal_confidence_check CHECK ((confidence = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text]))),
    CONSTRAINT change_proposal_status_check CHECK ((status = ANY (ARRAY['proposed'::text, 'executed'::text, 'redline_pending'::text, 'escalated'::text, 'vetoed'::text, 'expired'::text, 'failed'::text])))
);


--
-- Name: cognition_item; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.cognition_item (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    run_id uuid NOT NULL,
    domain text NOT NULL,
    content text NOT NULL,
    evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    has_conflict boolean DEFAULT false NOT NULL,
    state text DEFAULT 'proposed'::text NOT NULL,
    decided_by uuid,
    decided_at timestamp with time zone,
    decision_note text,
    knowledge_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cognition_item_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT cognition_item_domain_check CHECK ((domain = ANY (ARRAY['identity'::text, 'repository'::text, 'domain'::text, 'architecture'::text, 'convention'::text, 'evolution'::text]))),
    CONSTRAINT cognition_item_state_check CHECK ((state = ANY (ARRAY['proposed'::text, 'confirmed'::text, 'rejected'::text, 'investigation'::text, 'deferred'::text, 'superseded'::text])))
);


--
-- Name: cognition_run; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.cognition_run (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    scope jsonb DEFAULT '{}'::jsonb NOT NULL,
    repo_versions jsonb DEFAULT '[]'::jsonb NOT NULL,
    item_count integer DEFAULT 0 NOT NULL,
    confirmed_count integer DEFAULT 0 NOT NULL,
    started_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    CONSTRAINT cognition_run_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'collecting'::text, 'analyzing'::text, 'completed'::text, 'partially_completed'::text, 'failed'::text, 'cancelled'::text])))
);


--
-- Name: conversation; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.conversation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    title text DEFAULT ''::text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    last_message_at timestamp with time zone,
    CONSTRAINT conversation_status_check CHECK ((status = ANY (ARRAY['active'::text, 'archived'::text])))
);


--
-- Name: conversation_message; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.conversation_message (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conversation_id uuid NOT NULL,
    role text NOT NULL,
    content text NOT NULL,
    content_type text DEFAULT 'markdown'::text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conversation_message_role_check CHECK ((role = ANY (ARRAY['user'::text, 'assistant'::text, 'system'::text])))
);


--
-- Name: document_index_state; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.document_index_state (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    provider text NOT NULL,
    external_document_id text,
    indexed_content_hash text,
    index_version text,
    status text DEFAULT 'pending'::text NOT NULL,
    last_error text,
    indexed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT document_index_state_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'indexed'::text, 'stale'::text, 'failed'::text])))
);


--
-- Name: document_proposal; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.document_proposal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid,
    doc_key text NOT NULL,
    doc_type text NOT NULL,
    canonical_path text NOT NULL,
    base_revision_id uuid,
    base_content_hash text,
    proposed_markdown text NOT NULL,
    proposed_content_hash text NOT NULL,
    change_summary text,
    source_event_id uuid,
    analysis_run_id uuid,
    status text DEFAULT 'draft'::text NOT NULL,
    quality_gate_result jsonb DEFAULT '{}'::jsonb NOT NULL,
    redline_kind text,
    redline_confirmed_by uuid,
    redline_confirmed_at timestamp with time zone,
    resulting_revision_id uuid,
    rejection_reason text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT document_proposal_doc_key_check CHECK ((doc_key ~ '^[A-Za-z0-9._-]+$'::text)),
    CONSTRAINT document_proposal_doc_type_check CHECK ((doc_type = ANY (ARRAY['requirement'::text, 'decision'::text, 'architecture'::text, 'knowledge'::text]))),
    CONSTRAINT document_proposal_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'quality_gates_passed'::text, 'redline_pending'::text, 'confirmed'::text, 'commit_pending'::text, 'committed'::text, 'rejected'::text, 'rolled_back'::text])))
);


--
-- Name: document_revision; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.document_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    revision_no integer NOT NULL,
    git_commit_sha text NOT NULL,
    content_hash text NOT NULL,
    status text DEFAULT 'draft'::text NOT NULL,
    change_summary text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    source_event_id uuid,
    analysis_run_id uuid,
    redline_confirmed_by uuid,
    published_at timestamp with time zone,
    tenant_id uuid NOT NULL,
    CONSTRAINT document_revision_revision_no_check CHECK ((revision_no > 0)),
    CONSTRAINT document_revision_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'published'::text, 'rejected'::text, 'rolled_back'::text])))
);


--
-- Name: document_section_index; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.document_section_index (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    provider text NOT NULL,
    section_no integer NOT NULL,
    level smallint NOT NULL,
    title text NOT NULL,
    anchor text NOT NULL,
    content text NOT NULL,
    start_line integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: knowledge_reference; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.knowledge_reference (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    section_anchor text,
    section_title text,
    start_line integer,
    reference_kind text DEFAULT 'evidence'::text NOT NULL,
    source_kind text NOT NULL,
    source_id uuid,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT knowledge_reference_reference_kind_check CHECK ((reference_kind = ANY (ARRAY['evidence'::text, 'decision'::text, 'constraint'::text]))),
    CONSTRAINT knowledge_reference_source_kind_check CHECK ((source_kind = ANY (ARRAY['analysis_run'::text, 'event'::text, 'manual'::text])))
);


--
-- Name: knowledge_write_operation; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.knowledge_write_operation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    operation_id text NOT NULL,
    command text NOT NULL,
    proposal_id uuid,
    revision_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT knowledge_write_operation_command_check CHECK ((command = ANY (ARRAY['create_document'::text, 'submit_proposal'::text, 'confirm_redline'::text, 'veto_revision'::text, 'bind_requirement_projection'::text])))
);


--
-- Name: llm_call_record; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.llm_call_record (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid,
    provider_id uuid NOT NULL,
    model text NOT NULL,
    purpose text,
    status text NOT NULL,
    error_kind text,
    error_message text,
    prompt_tokens bigint,
    completion_tokens bigint,
    latency_ms bigint,
    analysis_run_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT llm_call_record_status_check CHECK ((status = ANY (ARRAY['ok'::text, 'failed'::text])))
);


--
-- Name: project_ai_config; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_ai_config (
    project_id uuid NOT NULL,
    ai_principal_id uuid NOT NULL,
    model_provider text NOT NULL,
    model_name text NOT NULL,
    model_config jsonb DEFAULT '{}'::jsonb NOT NULL,
    strategy text DEFAULT 'single_strong_model'::text NOT NULL,
    automation_mode text DEFAULT 'ai_lead'::text NOT NULL,
    policy_version integer DEFAULT 1 NOT NULL,
    config_revision bigint DEFAULT 1 NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT project_ai_config_automation_mode_check CHECK ((automation_mode = ANY (ARRAY['ai_lead'::text, 'controlled'::text]))),
    CONSTRAINT project_ai_config_strategy_check CHECK ((strategy = 'single_strong_model'::text))
);


--
-- Name: project_ai_model_policy; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_ai_model_policy (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    updated_by uuid NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: project_ai_role; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_ai_role (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    role_id uuid NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    current_version_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: project_baseline; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_baseline (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version integer NOT NULL,
    proposal_id uuid NOT NULL,
    confirmed_by uuid NOT NULL,
    confirmed_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: project_document; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_document (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    doc_key text NOT NULL,
    doc_type text NOT NULL,
    canonical_path text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT project_document_doc_key_check CHECK ((doc_key ~ '^[A-Za-z0-9._-]+$'::text)),
    CONSTRAINT project_document_doc_type_check CHECK ((doc_type = ANY (ARRAY['requirement'::text, 'decision'::text, 'architecture'::text, 'knowledge'::text])))
);


--
-- Name: project_knowledge; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_knowledge (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    domain text NOT NULL,
    content text NOT NULL,
    source_item_id uuid,
    version integer DEFAULT 1 NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    supersedes_id uuid,
    confirmed_by uuid NOT NULL,
    confirmed_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT project_knowledge_domain_check CHECK ((domain = ANY (ARRAY['identity'::text, 'repository'::text, 'domain'::text, 'architecture'::text, 'convention'::text, 'evolution'::text]))),
    CONSTRAINT project_knowledge_status_check CHECK ((status = ANY (ARRAY['active'::text, 'superseded'::text, 'retired'::text])))
);


--
-- Name: project_skill_binding; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.project_skill_binding (
    project_id uuid NOT NULL,
    skill_id text NOT NULL,
    skill_version text NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    settings jsonb DEFAULT '{}'::jsonb NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: skill_definition; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.skill_definition (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    current_version text NOT NULL
);


--
-- Name: team_llm_credential; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.team_llm_credential (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    provider_id uuid NOT NULL,
    api_key_ciphertext bytea NOT NULL,
    api_key_hint text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: team_llm_provider; Type: TABLE; Schema: ai; Owner: -
--

CREATE TABLE ai.team_llm_provider (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    name text NOT NULL,
    provider_type text NOT NULL,
    endpoint text NOT NULL,
    configuration jsonb DEFAULT '{}'::jsonb NOT NULL,
    status text DEFAULT 'not_configured'::text NOT NULL,
    credential_id uuid,
    timeout_secs integer DEFAULT 120 NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    last_tested_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT team_llm_provider_status_check CHECK ((status = ANY (ARRAY['not_configured'::text, 'configured'::text, 'connected'::text, 'failed'::text, 'enabled'::text, 'disabled'::text, 'archived'::text]))),
    CONSTRAINT team_llm_provider_timeout_secs_check CHECK (((timeout_secs >= 1) AND (timeout_secs <= 600)))
);


--
-- Name: approver_grant; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.approver_grant (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    granted_by uuid NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL,
    revoked_at timestamp with time zone,
    revoked_by uuid,
    tenant_id uuid NOT NULL
);


--
-- Name: customer; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.customer (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    code text,
    short_name text,
    industry text,
    notes text,
    parent_id uuid,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: principal; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.principal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    kind text NOT NULL,
    display_name text NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT principal_kind_check CHECK ((kind = ANY (ARRAY['human'::text, 'dev_agent'::text, 'ai_pm'::text, 'service'::text])))
);


--
-- Name: principal_identity; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.principal_identity (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider text NOT NULL,
    subject text NOT NULL,
    principal_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_seen_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: project; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.project (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    customer_id uuid NOT NULL,
    code text NOT NULL,
    name text NOT NULL,
    description text,
    status text DEFAULT 'planning'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: project_initialization_operation; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.project_initialization_operation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    import_id uuid,
    generation bigint NOT NULL,
    status text NOT NULL,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    released_at timestamp with time zone,
    CONSTRAINT project_initialization_operation_status_check CHECK ((status = ANY (ARRAY['active'::text, 'released'::text, 'completed'::text, 'cancelled'::text])))
);


--
-- Name: project_member; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.project_member (
    project_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    role text NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT ck_project_member_role CHECK ((role = ANY (ARRAY['owner'::text, 'member'::text, 'ai_pm'::text, 'architect'::text, 'developer'::text, 'tester'::text])))
);


--
-- Name: tenant; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.tenant (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider text NOT NULL,
    external_key text NOT NULL,
    display_name text NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    slug text,
    description text,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: tenant_agent; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.tenant_agent (
    tenant_id uuid NOT NULL,
    agent_definition_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    is_enabled boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: tenant_invitation; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.tenant_invitation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    email text NOT NULL,
    email_normalized text NOT NULL,
    role text NOT NULL,
    token_hash text NOT NULL,
    status text DEFAULT 'PENDING'::text NOT NULL,
    invited_by uuid NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    accepted_principal_id uuid,
    accepted_at timestamp with time zone,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tenant_invitation_check CHECK ((expires_at > created_at)),
    CONSTRAINT tenant_invitation_role_check CHECK ((role = ANY (ARRAY['admin'::text, 'member'::text]))),
    CONSTRAINT tenant_invitation_status_check CHECK ((status = ANY (ARRAY['PENDING'::text, 'ACCEPTED'::text, 'EXPIRED'::text, 'REVOKED'::text])))
);


--
-- Name: tenant_invitation_link; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.tenant_invitation_link (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    token_hash text NOT NULL,
    default_role text NOT NULL,
    status text DEFAULT 'ACTIVE'::text NOT NULL,
    max_uses integer,
    used_count integer DEFAULT 0 NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    revoked_at timestamp with time zone,
    CONSTRAINT tenant_invitation_link_check CHECK (((max_uses IS NULL) OR (used_count <= max_uses))),
    CONSTRAINT tenant_invitation_link_check1 CHECK ((expires_at > created_at)),
    CONSTRAINT tenant_invitation_link_default_role_check CHECK ((default_role = 'member'::text)),
    CONSTRAINT tenant_invitation_link_max_uses_check CHECK (((max_uses IS NULL) OR (max_uses > 0))),
    CONSTRAINT tenant_invitation_link_status_check CHECK ((status = ANY (ARRAY['ACTIVE'::text, 'EXHAUSTED'::text, 'EXPIRED'::text, 'REVOKED'::text]))),
    CONSTRAINT tenant_invitation_link_used_count_check CHECK ((used_count >= 0))
);


--
-- Name: tenant_invitation_link_redemption; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.tenant_invitation_link_redemption (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    link_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    redeemed_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: tenant_member; Type: TABLE; Schema: core; Owner: -
--

CREATE TABLE core.tenant_member (
    tenant_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    role text NOT NULL,
    granted_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT tenant_member_role_check CHECK ((role = ANY (ARRAY['owner'::text, 'admin'::text, 'member'::text])))
);


--
-- Name: acceptance_criterion_verdict; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.acceptance_criterion_verdict (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    criterion_id uuid NOT NULL,
    review_run_id uuid NOT NULL,
    verdict text NOT NULL,
    evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    decided_by uuid,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT acceptance_criterion_verdict_evidence_check CHECK ((jsonb_typeof(evidence) = 'object'::text)),
    CONSTRAINT acceptance_criterion_verdict_verdict_check CHECK ((verdict = ANY (ARRAY['passed'::text, 'failed'::text, 'not_evaluable'::text, 'awaiting_manual'::text])))
);


--
-- Name: continuation; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.continuation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    origin_interaction_id uuid NOT NULL,
    generation integer NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    frozen_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    started_at timestamp with time zone,
    closed_at timestamp with time zone,
    CONSTRAINT continuation_status_check CHECK ((status = ANY (ARRAY['open'::text, 'started'::text, 'consumed'::text, 'closed'::text, 'frozen'::text])))
);


--
-- Name: interaction_response; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.interaction_response (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    interaction_id uuid NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    responded_by uuid NOT NULL,
    response_payload jsonb NOT NULL,
    idempotency_key text NOT NULL,
    authorization_context jsonb NOT NULL,
    continue_command_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: interaction_response_delivery; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.interaction_response_delivery (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    interaction_id uuid NOT NULL,
    response_id uuid NOT NULL,
    continue_command_id uuid,
    run_id uuid NOT NULL,
    continuation_id uuid,
    continuation_generation integer DEFAULT 1 NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempt_count integer DEFAULT 0 NOT NULL,
    last_attempt_at timestamp with time zone,
    delivered_at timestamp with time zone,
    consumed_at timestamp with time zone,
    last_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT interaction_response_delivery_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'delivering'::text, 'delivered'::text, 'consumed'::text, 'failed'::text])))
);


--
-- Name: milestone; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.milestone (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    name text NOT NULL,
    planned_at timestamp with time zone,
    status text DEFAULT 'planned'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: policy_scope; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.policy_scope (
    scope text NOT NULL,
    scope_key text NOT NULL,
    require_human_approval boolean NOT NULL,
    updated_by uuid NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    auto_accept boolean,
    auto_merge boolean,
    auto_publish_knowledge boolean,
    additional_checks jsonb DEFAULT '[]'::jsonb NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT policy_scope_additional_checks_array CHECK ((jsonb_typeof(additional_checks) = 'array'::text)),
    CONSTRAINT policy_scope_scope_check CHECK ((scope = ANY (ARRAY['organization'::text, 'project'::text, 'task_type'::text])))
);


--
-- Name: requirement; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.requirement (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    code text NOT NULL,
    title text NOT NULL,
    lifecycle_status text DEFAULT 'active'::text NOT NULL,
    current_revision_id uuid,
    row_revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    requirement_type text DEFAULT 'functional'::text NOT NULL,
    priority text DEFAULT 'normal'::text NOT NULL,
    created_by uuid,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_by uuid,
    CONSTRAINT chk_requirement_lifecycle CHECK ((lifecycle_status = ANY (ARRAY['active'::text, 'withdrawn'::text, 'archived'::text]))),
    CONSTRAINT chk_requirement_priority CHECK ((priority = ANY (ARRAY['low'::text, 'normal'::text, 'high'::text]))),
    CONSTRAINT chk_requirement_type CHECK ((requirement_type = ANY (ARRAY['functional'::text, 'non_functional'::text])))
);


--
-- Name: requirement_acceptance_criterion; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.requirement_acceptance_criterion (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    requirement_id uuid NOT NULL,
    "position" integer DEFAULT 0 NOT NULL,
    statement text NOT NULL,
    verification_kind text DEFAULT 'manual'::text NOT NULL,
    params jsonb DEFAULT '{}'::jsonb NOT NULL,
    established_in_revision_id uuid,
    verdict text DEFAULT 'pending'::text NOT NULL,
    verdict_note text,
    verdict_by uuid,
    verdict_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT requirement_acceptance_criterion_statement_check CHECK ((length(btrim(statement)) > 0)),
    CONSTRAINT requirement_acceptance_criterion_verdict_check CHECK ((verdict = ANY (ARRAY['pending'::text, 'passed'::text, 'failed'::text]))),
    CONSTRAINT requirement_acceptance_criterion_verification_kind_check CHECK ((verification_kind = ANY (ARRAY['manual'::text, 'automated_test'::text, 'artifact_exists'::text, 'review'::text, 'performance'::text, 'security'::text])))
);


--
-- Name: requirement_confirmation; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.requirement_confirmation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    requirement_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    markdown_version_ref text,
    content_hash text,
    human_principal_id uuid NOT NULL,
    identity_id uuid,
    action text NOT NULL,
    reason text,
    source text DEFAULT 'rest'::text NOT NULL,
    audit_event_id uuid,
    confirmed_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT requirement_confirmation_action_check CHECK ((action = ANY (ARRAY['confirm'::text, 'reject'::text])))
);


--
-- Name: requirement_finding; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.requirement_finding (
    requirement_id uuid NOT NULL,
    finding_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    relation text DEFAULT 'supports'::text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT requirement_finding_relation_check CHECK ((relation = 'supports'::text))
);


--
-- Name: requirement_revision; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.requirement_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    requirement_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version integer NOT NULL,
    content jsonb NOT NULL,
    state text NOT NULL,
    content_hash text NOT NULL,
    created_by uuid NOT NULL,
    approved_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    base_current_revision_id uuid,
    base_row_revision bigint,
    markdown_version_ref text,
    submitted_at timestamp with time zone,
    CONSTRAINT requirement_revision_state_check CHECK ((state = ANY (ARRAY['draft'::text, 'pending_confirmation'::text, 'approved'::text, 'rejected'::text, 'superseded'::text]))),
    CONSTRAINT requirement_revision_version_check CHECK ((version > 0))
);


--
-- Name: requirement_source_input; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.requirement_source_input (
    requirement_id uuid NOT NULL,
    input_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    relation text DEFAULT 'source'::text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: review_finding; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.review_finding (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    review_run_id uuid NOT NULL,
    criterion_id uuid,
    finding_kind text NOT NULL,
    severity text NOT NULL,
    code_location text,
    message text NOT NULL,
    evidence_ref text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT review_finding_severity_check CHECK ((severity = ANY (ARRAY['info'::text, 'warning'::text, 'blocker'::text])))
);


--
-- Name: task; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    milestone_id uuid,
    title text NOT NULL,
    description text,
    status text DEFAULT 'draft'::text NOT NULL,
    priority text DEFAULT 'normal'::text NOT NULL,
    assignee_id uuid,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    superseded_by_task_id uuid,
    attempt_count integer DEFAULT 0 NOT NULL,
    consecutive_failure_count integer DEFAULT 0 NOT NULL,
    last_failure_category text,
    retry_exhausted boolean DEFAULT false NOT NULL,
    task_type text,
    goal text,
    constraints jsonb DEFAULT '{}'::jsonb NOT NULL,
    deliverables jsonb DEFAULT '[]'::jsonb NOT NULL,
    git_branch text,
    tenant_id uuid NOT NULL,
    ai_role_version_id uuid,
    CONSTRAINT task_failure_category_valid CHECK (((last_failure_category IS NULL) OR (last_failure_category = ANY (ARRAY['infrastructure'::text, 'execution'::text, 'validation'::text, 'permission'::text, 'policy_violation'::text, 'missing_dependency'::text, 'unknown'::text])))),
    CONSTRAINT task_status_valid CHECK ((status = ANY (ARRAY['draft'::text, 'ready'::text, 'claimed'::text, 'running'::text, 'awaiting_decision'::text, 'blocked'::text, 'submitted'::text, 'reviewing'::text, 'completed'::text, 'changes_requested'::text, 'failed'::text, 'cancelled'::text, 'superseded'::text])))
);


--
-- Name: task_acceptance_criterion; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_acceptance_criterion (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    ordinal integer NOT NULL,
    text text NOT NULL,
    verification_kind text DEFAULT 'manual'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT task_acceptance_criterion_ordinal_check CHECK ((ordinal >= 1)),
    CONSTRAINT task_acceptance_criterion_text_check CHECK ((length(btrim(text)) > 0)),
    CONSTRAINT task_acceptance_criterion_verification_kind_check CHECK ((verification_kind = ANY (ARRAY['manual'::text, 'automated_test'::text, 'artifact_exists'::text, 'review'::text])))
);


--
-- Name: task_checkpoint; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_checkpoint (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    interaction_id uuid,
    file_path text NOT NULL,
    content_hash text NOT NULL,
    file_size bigint NOT NULL,
    session_id text,
    registered_via text DEFAULT 'executor_report'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_checkpoint_registered_via_check CHECK ((registered_via = ANY (ARRAY['executor_report'::text, 'worker_receipt'::text])))
);


--
-- Name: task_dependency; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_dependency (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    depends_on_task_id uuid NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_dependency_check CHECK ((task_id <> depends_on_task_id))
);


--
-- Name: task_interaction; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_interaction (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    session_id text,
    continuation_generation integer DEFAULT 0 NOT NULL,
    event_id text NOT NULL,
    reason text NOT NULL,
    title text NOT NULL,
    question text NOT NULL,
    options jsonb,
    required_response boolean DEFAULT true NOT NULL,
    checkpoint_path text,
    status text DEFAULT 'pending'::text NOT NULL,
    chain_open boolean DEFAULT true NOT NULL,
    deadline_at timestamp with time zone NOT NULL,
    responded_at timestamp with time zone,
    consumed_at timestamp with time zone,
    timed_out_at timestamp with time zone,
    cancelled_at timestamp with time zone,
    retry_of uuid,
    continuation_start_frozen_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    previous_chain_id uuid,
    CONSTRAINT task_interaction_reason_check CHECK ((reason = ANY (ARRAY['needs_info'::text, 'needs_decision'::text, 'needs_approval'::text, 'external_dependency'::text]))),
    CONSTRAINT task_interaction_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'responded'::text, 'consumed'::text, 'timed_out'::text, 'cancelled'::text])))
);


--
-- Name: task_knowledge_ref; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_knowledge_ref (
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    document_id uuid NOT NULL,
    is_required boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: task_policy_override; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_policy_override (
    task_id uuid NOT NULL,
    project_id uuid NOT NULL,
    auto_accept boolean,
    auto_merge boolean,
    auto_publish_knowledge boolean,
    require_human_approval boolean,
    additional_checks jsonb DEFAULT '[]'::jsonb NOT NULL,
    updated_by uuid NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: task_repository; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_repository (
    task_id uuid NOT NULL,
    project_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    write_access boolean DEFAULT true NOT NULL,
    base_ref text DEFAULT 'main'::text NOT NULL,
    base_commit text,
    path_scope text[],
    role text DEFAULT 'primary'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT task_repository_role_check CHECK ((role = ANY (ARRAY['primary'::text, 'secondary'::text, 'knowledge'::text])))
);


--
-- Name: task_repository_commit; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_repository_commit (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_run_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    commit_sha text NOT NULL,
    branch text,
    changed_files integer DEFAULT 0 NOT NULL,
    test_evidence jsonb DEFAULT '{}'::jsonb NOT NULL,
    commit_author text,
    commit_authored_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_repository_commit_branch_check CHECK (((branch IS NULL) OR (length(branch) <= 255))),
    CONSTRAINT task_repository_commit_changed_files_check CHECK ((changed_files >= 0)),
    CONSTRAINT task_repository_commit_commit_author_check CHECK (((commit_author IS NULL) OR (length(commit_author) <= 255))),
    CONSTRAINT task_repository_commit_commit_sha_check CHECK (((length(btrim(commit_sha)) > 0) AND (length(commit_sha) <= 128))),
    CONSTRAINT task_repository_commit_test_evidence_check CHECK (((jsonb_typeof(test_evidence) = 'object'::text) AND ((NOT (test_evidence ? 'tests'::text)) OR (jsonb_typeof((test_evidence -> 'tests'::text)) = 'array'::text))))
);


--
-- Name: task_requirement; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_requirement (
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    requirement_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: task_run; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_run (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    attempt_no integer NOT NULL,
    stage text,
    status text DEFAULT 'queued'::text NOT NULL,
    outcome text,
    timeout_seconds integer,
    policy_snapshot jsonb,
    policy_version integer,
    override_version bigint,
    policy_hash text,
    evaluated_at timestamp with time zone,
    repository_policy_snapshots jsonb,
    started_at timestamp with time zone,
    finished_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    claimed_at timestamp with time zone,
    lease_owner text,
    lease_expires_at timestamp with time zone,
    heartbeat_at timestamp with time zone,
    dispatch_attempts integer DEFAULT 0 NOT NULL,
    next_dispatch_at timestamp with time zone DEFAULT now() NOT NULL,
    release_reason text,
    executor_kind text,
    executor_session text,
    supersedes_run_id uuid,
    terminated_at timestamp with time zone,
    termination_reason text,
    infrastructure_retry_count integer DEFAULT 0 NOT NULL,
    source_event_id uuid,
    lease_token_hash text,
    client_name text,
    client_version text,
    tenant_id uuid NOT NULL,
    agent_id uuid,
    triggered_by_user_id uuid,
    executor_id uuid,
    integration_status text DEFAULT 'not_run'::text NOT NULL,
    request_fingerprint text,
    lease_generation integer DEFAULT 0 NOT NULL,
    deadline_at timestamp with time zone,
    last_agent_heartbeat_at timestamp with time zone,
    last_progress_at timestamp with time zone,
    run_kind text DEFAULT 'delivery'::text NOT NULL,
    role_id text,
    input_manifest_hash text,
    reviewed_manifest_hash text,
    review_policy_version text,
    review_verdict text,
    execution_mode text,
    policy_snapshot_hash text,
    scheduling_shield text DEFAULT 'none'::text NOT NULL,
    lease_client_id uuid,
    CONSTRAINT chk_delivery_executor_kind CHECK (((run_kind <> 'delivery'::text) OR (executor_kind IS NULL) OR (executor_kind <> 'platform'::text))),
    CONSTRAINT chk_review_platform_isolation CHECK (((run_kind <> 'review'::text) OR ((executor_kind = 'platform'::text) AND (executor_session IS NULL) AND (lease_token_hash IS NULL) AND (lease_generation = 0) AND (deadline_at IS NULL) AND (last_agent_heartbeat_at IS NULL) AND (stage IS NULL)))),
    CONSTRAINT chk_review_role_present CHECK (((run_kind <> 'review'::text) OR ((role_id IS NOT NULL) AND (review_policy_version IS NOT NULL)))),
    CONSTRAINT task_run_attempt_no_check CHECK ((attempt_no >= 1)),
    CONSTRAINT task_run_execution_mode_check CHECK ((execution_mode = ANY (ARRAY['human'::text, 'spawn'::text]))),
    CONSTRAINT task_run_integration_status_check CHECK ((integration_status = ANY (ARRAY['passed'::text, 'failed'::text, 'not_run'::text]))),
    CONSTRAINT task_run_outcome_check CHECK ((outcome = ANY (ARRAY['succeeded'::text, 'failed'::text, 'timed_out'::text, 'cancelled'::text]))),
    CONSTRAINT task_run_review_verdict_check CHECK ((review_verdict = ANY (ARRAY['approved'::text, 'changes_requested'::text, 'inconclusive'::text]))),
    CONSTRAINT task_run_run_kind_check CHECK ((run_kind = ANY (ARRAY['delivery'::text, 'review'::text]))),
    CONSTRAINT task_run_scheduling_shield_check CHECK ((scheduling_shield = ANY (ARRAY['none'::text, 'human_wait'::text, 'start_wait'::text, 'timed_out'::text]))),
    CONSTRAINT task_run_status_check CHECK ((status = ANY (ARRAY['queued'::text, 'claimed'::text, 'running'::text, 'succeeded'::text, 'failed'::text, 'timed_out'::text, 'cancelled'::text])))
);


--
-- Name: COLUMN task_run.lease_generation; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.lease_generation IS '执行权世代（T2 终裁）：新行 0，每次成功获取执行权（领取/重领）+1——首次有效领取即 1，重领再 +1；心跳续租与领取失败不递增；MCP 必须同时校验 token 与 generation，旧 generation 永不重新授权。执行权世代不等于普通凭据校验。';


--
-- Name: COLUMN task_run.deadline_at; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.deadline_at IS 'Run 级绝对执行截止（T3 终裁）：首次成功 claim 时按 task.timeout_seconds 与环境默认上限固化；重领与心跳续租不得重置或延长；绝对截止优先于租约续期。';


--
-- Name: COLUMN task_run.last_agent_heartbeat_at; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.last_agent_heartbeat_at IS '执行端 Agent 心跳（T4 终裁）：仅经 MCP 通道（renew/progress）刷新；与进程存活、执行进展、绝对截止四类信号分立；心跳不能自动视为执行进展。';


--
-- Name: COLUMN task_run.last_progress_at; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.last_progress_at IS '执行进展时刻（T4 终裁）：执行阶段（stage）变化时刷新；与心跳独立阈值独立判定。';


--
-- Name: COLUMN task_run.run_kind; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.run_kind IS 'Run 种类：delivery=D1/D3 执行 Run；review=D5 平台内确定性 Reviewer Run。两条恢复链各自恒定。';


--
-- Name: COLUMN task_run.review_verdict; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.review_verdict IS 'Review 结论三值（approved/changes_requested/inconclusive）。四值中的「审查运行本身失败」由 run status=failed 表达，不重复存。';


--
-- Name: COLUMN task_run.scheduling_shield; Type: COMMENT; Schema: delivery; Owner: -
--

COMMENT ON COLUMN delivery.task_run.scheduling_shield IS 'N-1 人工等待调度保护：none/human_wait/start_wait/timed_out；poll_active_runs 四信号与 B3-2 重领对非 none 一律跳过（ACP-427/428）；由 InteractionService 事务维护，非 Run 状态机（设计稿 V1.1 §4）';


--
-- Name: task_run_cleanup; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_run_cleanup (
    run_id uuid NOT NULL,
    session text NOT NULL,
    executor_kind text,
    identity_hint text,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 8 NOT NULL,
    next_retry_at timestamp with time zone DEFAULT now() NOT NULL,
    last_error text,
    completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT task_run_cleanup_check CHECK (((completed_at IS NULL) OR (status = 'completed'::text))),
    CONSTRAINT task_run_cleanup_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'cleaning'::text, 'completed'::text, 'needs_human'::text])))
);


--
-- Name: task_run_execution_authorization; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_run_execution_authorization (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    run_id uuid NOT NULL,
    interaction_id uuid,
    authorized_by uuid NOT NULL,
    reason text NOT NULL,
    new_deadline_at timestamp with time zone NOT NULL,
    granted_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: task_run_failure; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_run_failure (
    run_id uuid NOT NULL,
    project_id uuid NOT NULL,
    category text NOT NULL,
    retryable boolean NOT NULL,
    reason_code text NOT NULL,
    exit_code integer,
    timeout_seconds integer,
    evidence_ref text,
    recorded_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT task_run_failure_category_check CHECK ((category = ANY (ARRAY['infrastructure'::text, 'execution'::text, 'validation'::text, 'permission'::text, 'policy_violation'::text, 'missing_dependency'::text, 'unknown'::text])))
);


--
-- Name: task_run_repository; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_run_repository (
    run_id uuid NOT NULL,
    project_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    base_ref text NOT NULL,
    base_commit text NOT NULL,
    write_access boolean NOT NULL,
    workspace_path text NOT NULL,
    path_scope text[],
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: task_supersession; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_supersession (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    old_task_id uuid NOT NULL,
    new_task_id uuid NOT NULL,
    reason text NOT NULL,
    created_by uuid NOT NULL,
    approval_request_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT task_supersession_check CHECK ((old_task_id <> new_task_id)),
    CONSTRAINT task_supersession_reason_check CHECK ((length(btrim(reason)) > 0))
);


--
-- Name: task_waiting; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.task_waiting (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    reason text NOT NULL,
    questions jsonb DEFAULT '[]'::jsonb NOT NULL,
    requested_from uuid,
    since timestamp with time zone DEFAULT now() NOT NULL,
    deadline timestamp with time zone,
    ended_at timestamp with time zone,
    ended_by uuid,
    resolution_note text,
    tenant_id uuid NOT NULL,
    CONSTRAINT task_waiting_reason_check CHECK ((reason = ANY (ARRAY['needs_info'::text, 'needs_decision'::text, 'needs_approval'::text, 'external_dependency'::text, 'no_executor_available'::text])))
);


--
-- Name: project_repository; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.project_repository (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    name text NOT NULL,
    url text NOT NULL,
    role text DEFAULT 'main'::text NOT NULL,
    default_branch text,
    description text,
    revision bigint DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    ssh_key_id uuid,
    status text DEFAULT 'pending'::text NOT NULL,
    head_commit_sha text,
    last_verified_at timestamp with time zone,
    last_error_code text,
    last_error_detail text,
    tenant_id uuid NOT NULL,
    selected_branch text,
    selected_commit text,
    selected_confirmed_at timestamp with time zone,
    selected_confirmed_by uuid,
    CONSTRAINT project_repository_role_check CHECK ((role = ANY (ARRAY['main'::text, 'docs'::text, 'service'::text, 'infra'::text]))),
    CONSTRAINT project_repository_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'verified'::text, 'unreachable'::text, 'archived'::text])))
);


--
-- Name: v_task_delivery_manifest; Type: VIEW; Schema: delivery; Owner: -
--

CREATE VIEW delivery.v_task_delivery_manifest AS
 WITH contract AS (
         SELECT tr.task_id,
            tr.project_id,
            tr.repository_id,
            tr.write_access,
            pr.name AS repository_name
           FROM (delivery.task_repository tr
             JOIN integration.project_repository pr ON (((pr.id = tr.repository_id) AND (pr.project_id = tr.project_id))))
        ), latest_run AS (
         SELECT DISTINCT ON (r.task_id) r.task_id,
            r.id AS run_id,
            r.attempt_no,
            r.integration_status
           FROM delivery.task_run r
          WHERE (r.run_kind = 'delivery'::text)
          ORDER BY r.task_id, r.attempt_no DESC, r.created_at DESC
        ), latest_submitted_run AS (
         SELECT DISTINCT ON (r.task_id) r.task_id,
            c_1.task_run_id AS run_id
           FROM (delivery.task_repository_commit c_1
             JOIN delivery.task_run r ON ((r.id = c_1.task_run_id)))
          ORDER BY r.task_id, r.attempt_no DESC, c_1.created_at DESC, c_1.id DESC
        ), counts AS (
         SELECT c_1.task_id,
            (count(*) FILTER (WHERE c_1.write_access))::integer AS expected_delivery_count,
            (count(*) FILTER (WHERE (c_1.write_access AND (lr_1.run_id IS NOT NULL) AND (EXISTS ( SELECT 1
                   FROM delivery.task_repository_commit cr_1
                  WHERE ((cr_1.task_run_id = lr_1.run_id) AND (cr_1.repository_id = c_1.repository_id)))))))::integer AS actual_delivery_count
           FROM (contract c_1
             LEFT JOIN latest_run lr_1 ON ((lr_1.task_id = c_1.task_id)))
          GROUP BY c_1.task_id
        )
 SELECT c.task_id,
    c.project_id,
    c.repository_id,
    c.repository_name,
    c.write_access,
    c.write_access AS delivery_required,
    lr.run_id AS latest_run_id,
    lr.attempt_no AS latest_attempt_no,
    lr.integration_status,
    lsr.run_id AS latest_submitted_run_id,
    trr.base_commit,
    cr.commit_sha,
    cr.changed_files,
    cr.test_evidence,
    cr.branch,
    cr.commit_author,
    cr.commit_authored_at,
    cnt.expected_delivery_count,
    cnt.actual_delivery_count,
        CASE
            WHEN (cnt.expected_delivery_count = 0) THEN 'delivered'::text
            WHEN (cnt.actual_delivery_count = 0) THEN 'never_submitted'::text
            WHEN (cnt.actual_delivery_count = cnt.expected_delivery_count) THEN 'delivered'::text
            ELSE 'partial'::text
        END AS delivery_state
   FROM (((((contract c
     LEFT JOIN latest_run lr ON ((lr.task_id = c.task_id)))
     LEFT JOIN latest_submitted_run lsr ON ((lsr.task_id = c.task_id)))
     LEFT JOIN delivery.task_run_repository trr ON (((trr.run_id = lr.run_id) AND (trr.repository_id = c.repository_id))))
     LEFT JOIN delivery.task_repository_commit cr ON (((cr.task_run_id = lr.run_id) AND (cr.repository_id = c.repository_id))))
     JOIN counts cnt ON ((cnt.task_id = c.task_id)));


--
-- Name: worker_command; Type: TABLE; Schema: delivery; Owner: -
--

CREATE TABLE delivery.worker_command (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    command_type text NOT NULL,
    worker_id text NOT NULL,
    task_id uuid NOT NULL,
    run_id uuid NOT NULL,
    interaction_id uuid,
    continuation_id uuid,
    lease_generation integer DEFAULT 0 NOT NULL,
    payload jsonb NOT NULL,
    status text DEFAULT 'issued'::text NOT NULL,
    claim_count integer DEFAULT 0 NOT NULL,
    claimed_by text,
    claimed_at timestamp with time zone,
    claim_expires_at timestamp with time zone,
    issued_at timestamp with time zone DEFAULT now() NOT NULL,
    delivered_at timestamp with time zone,
    acked_at timestamp with time zone,
    failed_at timestamp with time zone,
    last_error text,
    reacquired_generation integer,
    CONSTRAINT worker_command_check CHECK (((status <> 'delivered'::text) OR ((claimed_by IS NOT NULL) AND (claim_expires_at IS NOT NULL)))),
    CONSTRAINT worker_command_command_type_check CHECK ((command_type = ANY (ARRAY['agent.process.terminate'::text, 'agent.task.continue'::text]))),
    CONSTRAINT worker_command_status_check CHECK ((status = ANY (ARRAY['issued'::text, 'delivered'::text, 'acked'::text, 'failed'::text])))
);


--
-- Name: device; Type: TABLE; Schema: devenv; Owner: -
--

CREATE TABLE devenv.device (
    id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    owner_principal_id uuid NOT NULL,
    name text NOT NULL,
    platform text NOT NULL,
    os_version text,
    executor_kind text DEFAULT 'codex'::text NOT NULL,
    executor_version text,
    status text DEFAULT 'active'::text NOT NULL,
    readiness text,
    last_check_at timestamp with time zone,
    last_check jsonb,
    revoked_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    credential_key_hash text,
    credential_issued_at timestamp with time zone,
    credential_revoked_at timestamp with time zone,
    granted_permissions text[] DEFAULT ARRAY['PM_MCP_READ'::text, 'PM_MCP_AGENT_REPORT'::text, 'PM_MCP_COMMAND'::text] NOT NULL,
    is_runner boolean DEFAULT false NOT NULL,
    last_heartbeat_at timestamp with time zone,
    CONSTRAINT device_readiness_check CHECK ((readiness = ANY (ARRAY['installed'::text, 'connected'::text, 'authenticated'::text, 'ready'::text, 'degraded'::text, 'failed'::text]))),
    CONSTRAINT device_status_check CHECK ((status = ANY (ARRAY['active'::text, 'revoked'::text])))
);


--
-- Name: COLUMN device.is_runner; Type: COMMENT; Schema: devenv; Owner: -
--

COMMENT ON COLUMN devenv.device.is_runner IS '领取型 Runner 载体（D-2=c Q-D2C-1）：其绑定项目的 delivery run 由 Runner 显式领取
     （Model A）；worker 无人值守派发（Model B）在候选发现阶段静默排除。';


--
-- Name: COLUMN device.last_heartbeat_at; Type: COMMENT; Schema: devenv; Owner: -
--

COMMENT ON COLUMN devenv.device.last_heartbeat_at IS '设备级心跳（可用性真源，Q-D2C-2）：已认证设备活动（领取/列表/上报/续领/发现/
     命令轮询）服务端归因刷新；新鲜度语义由 AI_PM_RUNNER_AVAILABILITY_STALE_SECS 定义。';


--
-- Name: device_project_binding; Type: TABLE; Schema: devenv; Owner: -
--

CREATE TABLE devenv.device_project_binding (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    device_id uuid NOT NULL,
    project_id uuid NOT NULL,
    declared_repository_ids jsonb DEFAULT '[]'::jsonb NOT NULL,
    verified_repositories jsonb DEFAULT '[]'::jsonb NOT NULL,
    bound_by uuid NOT NULL,
    bound_at timestamp with time zone DEFAULT now() NOT NULL,
    unbound_at timestamp with time zone
);


--
-- Name: discovery_answer; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_answer (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    question_id uuid NOT NULL,
    answer text NOT NULL,
    answered_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: discovery_finding; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_finding (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL,
    kind text NOT NULL,
    content text NOT NULL,
    confidence double precision,
    verification_status text DEFAULT 'PENDING'::text NOT NULL,
    actor_role text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    project_id uuid NOT NULL,
    CONSTRAINT discovery_finding_confidence_check CHECK (((confidence IS NULL) OR ((confidence >= (0)::double precision) AND (confidence <= (1)::double precision)))),
    CONSTRAINT discovery_finding_kind_check CHECK ((kind = ANY (ARRAY['observed'::text, 'user_confirmed'::text, 'speculation'::text, 'contradicted'::text]))),
    CONSTRAINT discovery_finding_verification_status_check CHECK ((verification_status = ANY (ARRAY['PENDING'::text, 'CONFIRMED'::text, 'REJECTED'::text, 'CONFLICT'::text])))
);


--
-- Name: discovery_finding_evidence; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_finding_evidence (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    finding_id uuid NOT NULL,
    evidence_type text NOT NULL,
    description text,
    file_path text,
    symbol text,
    code_range text,
    import_id uuid,
    repository_id uuid,
    commit_sha text,
    attempt_no integer,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT discovery_finding_evidence_check CHECK ((((evidence_type = ANY (ARRAY['code'::text, 'test_result'::text, 'commit'::text])) AND (import_id IS NOT NULL) AND (repository_id IS NOT NULL) AND (commit_sha IS NOT NULL) AND (attempt_no IS NOT NULL)) OR ((evidence_type = ANY (ARRAY['user_statement'::text, 'system_observation'::text])) AND (import_id IS NULL) AND (repository_id IS NULL) AND (commit_sha IS NULL) AND (attempt_no IS NULL)))),
    CONSTRAINT discovery_finding_evidence_evidence_type_check CHECK ((evidence_type = ANY (ARRAY['code'::text, 'test_result'::text, 'commit'::text, 'user_statement'::text, 'system_observation'::text])))
);


--
-- Name: discovery_finding_repository; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_finding_repository (
    tenant_id uuid NOT NULL,
    finding_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    project_id uuid NOT NULL
);


--
-- Name: discovery_message; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_message (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL,
    speaker text NOT NULL,
    source text NOT NULL,
    content text NOT NULL,
    seq bigint NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT discovery_message_source_check CHECK ((source = ANY (ARRAY['WEB'::text, 'CODEX'::text, 'SYSTEM'::text]))),
    CONSTRAINT discovery_message_speaker_check CHECK ((speaker = ANY (ARRAY['ai'::text, 'user'::text])))
);


--
-- Name: discovery_proposal; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_proposal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL,
    action text NOT NULL,
    target_document text NOT NULL,
    document_id uuid,
    base_revision_id uuid,
    proposed_content text NOT NULL,
    evidence jsonb DEFAULT '[]'::jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    document_proposal_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    project_id uuid NOT NULL,
    CONSTRAINT discovery_proposal_action_check CHECK ((action = ANY (ARRAY['add'::text, 'modify'::text]))),
    CONSTRAINT discovery_proposal_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text, 'conflict'::text])))
);


--
-- Name: discovery_question; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_question (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    session_id uuid NOT NULL,
    finding_id uuid,
    question text NOT NULL,
    why_blocking text,
    status text DEFAULT 'open'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    project_id uuid NOT NULL,
    CONSTRAINT discovery_question_status_check CHECK ((status = ANY (ARRAY['open'::text, 'answered'::text, 'deferred'::text])))
);


--
-- Name: discovery_session; Type: TABLE; Schema: discovery; Owner: -
--

CREATE TABLE discovery.discovery_session (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    source text NOT NULL,
    kind text NOT NULL,
    status text DEFAULT 'IN_PROGRESS'::text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT discovery_session_kind_check CHECK ((kind = ANY (ARRAY['INITIAL_ONBOARDING'::text, 'UPDATE'::text, 'TASK_DISCOVERY'::text]))),
    CONSTRAINT discovery_session_source_check CHECK ((source = ANY (ARRAY['WEB'::text, 'CODEX'::text, 'SYSTEM'::text]))),
    CONSTRAINT discovery_session_status_check CHECK ((status = ANY (ARRAY['IN_PROGRESS'::text, 'NEEDS_USER_INPUT'::text, 'REVIEW_REQUIRED'::text, 'COMPLETED'::text, 'PAUSED'::text])))
);


--
-- Name: approval_request; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.approval_request (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    required_role text NOT NULL,
    decided_by uuid,
    decision_reason text,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: audit_log; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.audit_log (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    actor_id uuid NOT NULL,
    action text NOT NULL,
    target_type text NOT NULL,
    target_id uuid,
    before_data jsonb,
    after_data jsonb,
    compensation_command jsonb,
    compensated_command_id uuid,
    source_event_id uuid,
    analysis_run_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    source_kind text,
    source_ref uuid,
    actor_client text,
    actor_auth_mode text,
    actor_identity_id uuid,
    tenant_id uuid NOT NULL,
    claimed_device_id uuid
);


--
-- Name: human_intervention; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.human_intervention (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    intervention_type text NOT NULL,
    target_proposal_id uuid,
    target_command_id uuid,
    actor_id uuid NOT NULL,
    reason text NOT NULL,
    executed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL,
    CONSTRAINT human_intervention_intervention_type_check CHECK ((intervention_type = ANY (ARRAY['veto'::text, 'rollback'::text, 'freeze'::text, 'unfreeze'::text, 'mode_switch'::text, 'policy_change'::text, 'rollback_failed'::text])))
);


--
-- Name: review_approval; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.review_approval (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    task_id uuid NOT NULL,
    review_run_id uuid NOT NULL,
    delivery_run_id uuid NOT NULL,
    delivery_manifest_hash text NOT NULL,
    kind text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    required_role text,
    decided_by uuid,
    decision_reason text,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT review_approval_kind_check CHECK ((kind = ANY (ARRAY['auto_accept_gate'::text, 'manual_review'::text, 'review_degraded'::text]))),
    CONSTRAINT review_approval_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text])))
);


--
-- Name: tenant_audit_log; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.tenant_audit_log (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    actor_id uuid NOT NULL,
    actor_identity_id uuid,
    actor_client text,
    actor_auth_mode text,
    action text NOT NULL,
    target_principal_id uuid,
    before_data jsonb,
    after_data jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    claimed_device_id uuid,
    target_type text,
    target_id uuid
);


--
-- Name: inbound_event; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.inbound_event (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    actor_id uuid NOT NULL,
    source text NOT NULL,
    event_type text NOT NULL,
    idempotency_key text NOT NULL,
    payload jsonb NOT NULL,
    payload_hash text NOT NULL,
    status text DEFAULT 'received'::text NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: intake_record; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.intake_record (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    intake_type text NOT NULL,
    channel text NOT NULL,
    submitted_by uuid NOT NULL,
    source_ref jsonb NOT NULL,
    content_hash text,
    title text,
    intake_status text DEFAULT 'RECEIVED'::text NOT NULL,
    idempotency_key text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT intake_record_channel_check CHECK ((channel = ANY (ARRAY['web'::text, 'mcp'::text, 'agent_api'::text, 'system'::text]))),
    CONSTRAINT intake_record_intake_status_check CHECK ((intake_status = ANY (ARRAY['RECEIVED'::text, 'PERSISTED'::text, 'ROUTED'::text, 'CLOSED'::text, 'FAILED'::text, 'CANCELLED'::text]))),
    CONSTRAINT intake_record_intake_type_check CHECK ((intake_type = ANY (ARRAY['document_upload'::text, 'meeting_notes'::text, 'product_fact'::text, 'product_decision'::text, 'engineering_artifact'::text, 'engineering_result'::text, 'external_change'::text, 'analysis_request'::text])))
);


--
-- Name: intake_task; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.intake_task (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    intake_id uuid NOT NULL,
    target_service text NOT NULL,
    task_type text NOT NULL,
    phase text DEFAULT 'required'::text NOT NULL,
    task_status text DEFAULT 'PENDING'::text NOT NULL,
    attempt integer DEFAULT 0 NOT NULL,
    idempotency_key text NOT NULL,
    signal_ref jsonb,
    result_ref jsonb,
    last_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT intake_task_phase_check CHECK ((phase = ANY (ARRAY['required'::text, 'optional'::text]))),
    CONSTRAINT intake_task_target_service_check CHECK ((target_service = ANY (ARRAY['kb'::text, 'pc'::text]))),
    CONSTRAINT intake_task_task_status_check CHECK ((task_status = ANY (ARRAY['PENDING'::text, 'DISPATCHED'::text, 'PROCESSING'::text, 'COMPLETED'::text, 'FAILED'::text, 'NEEDS_HUMAN_REVIEW'::text, 'SKIPPED'::text, 'CANCELLED'::text]))),
    CONSTRAINT intake_task_task_type_check CHECK ((task_type = ANY (ARRAY['kb_analyze'::text, 'pc_analyze'::text, 'pc_decide'::text, 'kb_archive'::text, 'kb_supply'::text])))
);


--
-- Name: job; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.job (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    event_id uuid NOT NULL,
    job_type text NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 5 NOT NULL,
    available_at timestamp with time zone DEFAULT now() NOT NULL,
    lease_owner text,
    lease_until timestamp with time zone,
    last_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: notification; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.notification (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    source_proposal_id uuid,
    kind text NOT NULL,
    payload jsonb NOT NULL,
    veto_window_until timestamp with time zone,
    read_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    tenant_id uuid NOT NULL
);


--
-- Name: outbox_event; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.outbox_event (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    event_type text NOT NULL,
    aggregate_type text NOT NULL,
    aggregate_id text NOT NULL,
    payload jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 5 NOT NULL,
    next_attempt_at timestamp with time zone DEFAULT now() NOT NULL,
    locked_until timestamp with time zone,
    lease_owner text,
    last_error text,
    idempotency_key text NOT NULL,
    schema_version integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    delivered_at timestamp with time zone,
    tenant_id uuid NOT NULL,
    CONSTRAINT outbox_event_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'processing'::text, 'delivered'::text, 'dead_letter'::text])))
);


--
-- Name: project_ssh_key; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.project_ssh_key (
    project_id uuid NOT NULL,
    label text DEFAULT 'default'::text NOT NULL,
    key_type text DEFAULT 'ed25519'::text NOT NULL,
    public_key text NOT NULL,
    private_key_ciphertext bytea NOT NULL,
    fingerprint text NOT NULL,
    cipher_suite text DEFAULT 'aes256gcm-v1'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    created_by uuid,
    revoked_at timestamp with time zone,
    tenant_id uuid NOT NULL,
    CONSTRAINT project_ssh_key_key_type_check CHECK ((key_type = 'ed25519'::text)),
    CONSTRAINT project_ssh_key_status_check CHECK ((status = ANY (ARRAY['active'::text, 'revoked'::text])))
);


--
-- Name: repo_import; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.repo_import (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    project_id uuid NOT NULL,
    requested_by uuid NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    stage_detail text,
    progress integer DEFAULT 0 NOT NULL,
    model_name text DEFAULT 'fake'::text NOT NULL,
    draft_revision bigint DEFAULT 0 NOT NULL,
    dropped_items integer DEFAULT 0 NOT NULL,
    error_code text,
    error_detail text,
    analysis_run_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    proposed_at timestamp with time zone,
    applied_at timestamp with time zone,
    tenant_id uuid NOT NULL,
    discovery_session_id uuid,
    CONSTRAINT repo_import_progress_check CHECK (((progress >= 0) AND (progress <= 100))),
    CONSTRAINT repo_import_status_check CHECK ((status = ANY (ARRAY['queued'::text, 'cloning'::text, 'reading'::text, 'analyzing'::text, 'draft_ready'::text, 'proposed'::text, 'applied'::text, 'rejected'::text, 'failed'::text, 'cancelled'::text])))
);


--
-- Name: repo_import_draft_version; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.repo_import_draft_version (
    tenant_id uuid NOT NULL,
    import_id uuid NOT NULL,
    revision bigint NOT NULL,
    draft jsonb NOT NULL,
    source_attempts jsonb DEFAULT '[]'::jsonb NOT NULL,
    dropped_items integer DEFAULT 0 NOT NULL,
    generation_status text NOT NULL,
    generated_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT repo_import_draft_version_generation_status_check CHECK ((generation_status = ANY (ARRAY['generating'::text, 'succeeded'::text, 'failed'::text])))
);


--
-- Name: repo_import_source; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.repo_import_source (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    import_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    head_commit_sha text,
    current_attempt_no integer,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: repo_import_source_attempt; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.repo_import_source_attempt (
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    import_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    attempt_no integer NOT NULL,
    status text NOT NULL,
    commit_sha text,
    facts jsonb,
    commit_count integer,
    doc_count integer,
    size_bytes bigint,
    truncated boolean,
    error_code text,
    error_detail text,
    job_id uuid,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    finished_at timestamp with time zone,
    CONSTRAINT repo_import_source_attempt_check CHECK (((finished_at IS NULL) OR (finished_at >= started_at))),
    CONSTRAINT repo_import_source_attempt_status_check CHECK ((status = ANY (ARRAY['running'::text, 'succeeded'::text, 'failed'::text, 'cancelled'::text])))
);


--
-- Name: repository_version_confirmation; Type: TABLE; Schema: integration; Owner: -
--

CREATE TABLE integration.repository_version_confirmation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    repository_id uuid NOT NULL,
    previous_branch text,
    previous_commit text,
    selected_branch text NOT NULL,
    selected_commit text NOT NULL,
    reason text,
    confirmed_by uuid NOT NULL,
    confirmed_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: document; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.document (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    doc_key text NOT NULL,
    doc_type text NOT NULL,
    canonical_path text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_doc_key_check CHECK ((doc_key ~ '^[A-Za-z0-9._-]+$'::text)),
    CONSTRAINT document_doc_type_check CHECK ((doc_type = ANY (ARRAY['requirement'::text, 'decision'::text, 'architecture'::text, 'knowledge'::text])))
);


--
-- Name: document_revision; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.document_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    document_id uuid NOT NULL,
    revision_no integer NOT NULL,
    source_commit_sha text,
    content_hash text NOT NULL,
    status text DEFAULT 'draft'::text NOT NULL,
    content text NOT NULL,
    change_summary text,
    created_by uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    analysis_mode text DEFAULT 'derived'::text NOT NULL,
    CONSTRAINT document_revision_analysis_mode_check CHECK ((analysis_mode = ANY (ARRAY['derived'::text, 'derived_fallback'::text, 'llm'::text]))),
    CONSTRAINT document_revision_revision_no_check CHECK ((revision_no >= 1)),
    CONSTRAINT document_revision_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'published'::text, 'rejected'::text])))
);


--
-- Name: memory; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.memory (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: memory_ref; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.memory_ref (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    ref_role text NOT NULL,
    target_domain text NOT NULL,
    target_type text NOT NULL,
    target_id text NOT NULL,
    target_revision_id text,
    revision_missing_reason text,
    locator jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT memory_ref_check CHECK (((target_revision_id IS NULL) = (revision_missing_reason IS NOT NULL))),
    CONSTRAINT memory_ref_ref_role_check CHECK ((ref_role = ANY (ARRAY['evidence'::text, 'subject'::text])))
);


--
-- Name: memory_revision; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.memory_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    memory_id uuid NOT NULL,
    revision_no integer NOT NULL,
    change_action text NOT NULL,
    base_revision_id uuid,
    memory_type text NOT NULL,
    title text NOT NULL,
    summary text NOT NULL,
    body text,
    change_reason text NOT NULL,
    content_hash text NOT NULL,
    proposed_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    review_status text DEFAULT 'PENDING_REVIEW'::text NOT NULL,
    reviewed_by uuid,
    reviewed_at timestamp with time zone,
    review_note text,
    version_no integer,
    version_status text,
    CONSTRAINT memory_revision_change_action_check CHECK ((change_action = ANY (ARRAY['CREATE'::text, 'UPDATE'::text, 'WITHDRAW'::text]))),
    CONSTRAINT memory_revision_change_reason_check CHECK (((length(btrim(change_reason)) >= 1) AND (length(btrim(change_reason)) <= 1000))),
    CONSTRAINT memory_revision_check CHECK (((review_status = 'PENDING_REVIEW'::text) = (reviewed_by IS NULL))),
    CONSTRAINT memory_revision_check1 CHECK (((review_status = 'PENDING_REVIEW'::text) = (reviewed_at IS NULL))),
    CONSTRAINT memory_revision_check2 CHECK (((review_status = 'APPROVED'::text) = (version_status IS NOT NULL))),
    CONSTRAINT memory_revision_check3 CHECK (((version_no IS NULL) = (version_status IS NULL))),
    CONSTRAINT memory_revision_check4 CHECK (((change_action = 'CREATE'::text) = (base_revision_id IS NULL))),
    CONSTRAINT memory_revision_check5 CHECK (((version_status IS DISTINCT FROM 'WITHDRAWN'::text) OR (change_action = 'WITHDRAW'::text))),
    CONSTRAINT memory_revision_check6 CHECK (((change_action <> 'WITHDRAW'::text) OR (version_status IS NULL) OR (version_status = 'WITHDRAWN'::text))),
    CONSTRAINT memory_revision_content_hash_check CHECK ((content_hash ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT memory_revision_memory_type_check CHECK ((memory_type = ANY (ARRAY['discussion'::text, 'decision_background'::text, 'rejected_option'::text, 'open_question'::text, 'business_constraint'::text, 'controversy'::text]))),
    CONSTRAINT memory_revision_review_note_check CHECK (((review_note IS NULL) OR (length(review_note) <= 1000))),
    CONSTRAINT memory_revision_review_status_check CHECK ((review_status = ANY (ARRAY['PENDING_REVIEW'::text, 'APPROVED'::text, 'REJECTED'::text, 'REVISION_REQUESTED'::text]))),
    CONSTRAINT memory_revision_revision_no_check CHECK ((revision_no >= 1)),
    CONSTRAINT memory_revision_summary_check CHECK (((length(btrim(summary)) >= 1) AND (length(btrim(summary)) <= 2000))),
    CONSTRAINT memory_revision_title_check CHECK (((length(btrim(title)) >= 1) AND (length(btrim(title)) <= 200))),
    CONSTRAINT memory_revision_version_no_check CHECK (((version_no IS NULL) OR (version_no >= 1))),
    CONSTRAINT memory_revision_version_status_check CHECK (((version_status IS NULL) OR (version_status = ANY (ARRAY['CURRENT'::text, 'SUPERSEDED'::text, 'WITHDRAWN'::text, 'MERGED'::text]))))
);


--
-- Name: source; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.source (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    source_kind text NOT NULL,
    title text NOT NULL,
    uri text,
    inline_content text,
    mime text,
    size_bytes bigint,
    content_hash text NOT NULL,
    registered_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT source_check CHECK (((uri IS NULL) <> (inline_content IS NULL))),
    CONSTRAINT source_content_hash_check CHECK ((content_hash ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT source_inline_content_check CHECK (((inline_content IS NULL) OR (length(inline_content) > 0))),
    CONSTRAINT source_size_bytes_check CHECK (((size_bytes IS NULL) OR (size_bytes >= 0))),
    CONSTRAINT source_source_kind_check CHECK ((source_kind = ANY (ARRAY['markdown'::text, 'docx'::text, 'pdf'::text, 'meeting_notes'::text, 'email'::text, 'im_log'::text, 'design'::text, 'recording'::text, 'other'::text]))),
    CONSTRAINT source_title_check CHECK (((length(btrim(title)) >= 1) AND (length(btrim(title)) <= 200))),
    CONSTRAINT source_uri_check CHECK (((uri IS NULL) OR (length(btrim(uri)) > 0)))
);


--
-- Name: source_project; Type: TABLE; Schema: kb; Owner: -
--

CREATE TABLE kb.source_project (
    tenant_id uuid NOT NULL,
    source_id uuid NOT NULL,
    project_id uuid NOT NULL,
    linked_by uuid NOT NULL,
    linked_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: approval; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.approval (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_content_hash text NOT NULL,
    expected_version_no bigint NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    decided_by uuid,
    decision_reason text,
    decided_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT approval_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text])))
);


--
-- Name: cognition_object; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.cognition_object (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    object_type text NOT NULL,
    object_key text,
    display_name text NOT NULL,
    lifecycle_status text DEFAULT 'draft'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    current_revision_id uuid,
    superseded_by_id uuid,
    created_by uuid NOT NULL,
    updated_by uuid,
    deprecated_at timestamp with time zone,
    CONSTRAINT cognition_object_lifecycle_status_check CHECK ((lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text, 'deprecated'::text, 'superseded'::text])))
);


--
-- Name: cognition_revision; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.cognition_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    object_id uuid NOT NULL,
    revision_no integer NOT NULL,
    change_summary text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    analysis_mode text DEFAULT 'derived'::text,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    document_revision_id uuid NOT NULL,
    content_hash text NOT NULL,
    source_context text,
    CONSTRAINT cognition_revision_analysis_mode_check CHECK ((analysis_mode = ANY (ARRAY['derived'::text, 'derived_fallback'::text, 'llm'::text]))),
    CONSTRAINT cognition_revision_revision_no_check CHECK ((revision_no >= 1))
);


--
-- Name: cognition_version; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.cognition_version (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version_no bigint NOT NULL,
    status text DEFAULT 'published'::text NOT NULL,
    confirmed_by uuid NOT NULL,
    confirmed_at timestamp with time zone DEFAULT now() NOT NULL,
    source_proposal_id uuid NOT NULL,
    snapshot_hash text NOT NULL,
    predecessor_version_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT cognition_version_status_check CHECK ((status = ANY (ARRAY['published'::text, 'archived'::text]))),
    CONSTRAINT cognition_version_version_no_check CHECK ((version_no >= 1))
);


--
-- Name: conflict; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.conflict (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conflict_type text NOT NULL,
    severity text NOT NULL,
    description text NOT NULL,
    detection_source text,
    status text DEFAULT 'detected'::text NOT NULL,
    resolution_note text,
    resolved_by uuid,
    resolved_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT conflict_check CHECK (((status <> ALL (ARRAY['resolved'::text, 'dismissed'::text])) OR ((resolved_by IS NOT NULL) AND (resolved_at IS NOT NULL)))),
    CONSTRAINT conflict_conflict_type_check CHECK ((conflict_type = ANY (ARRAY['content_contradiction'::text, 'rule_contradiction'::text, 'scope_conflict'::text, 'relationship_conflict'::text, 'source_conflict'::text, 'revision_conflict'::text, 'reality_mismatch'::text, 'cross_domain_authority'::text]))),
    CONSTRAINT conflict_severity_check CHECK ((severity = ANY (ARRAY['critical'::text, 'major'::text, 'minor'::text]))),
    CONSTRAINT conflict_status_check CHECK ((status = ANY (ARRAY['detected'::text, 'under_review'::text, 'awaiting_human'::text, 'resolved'::text, 'dismissed'::text, 'reopened'::text])))
);


--
-- Name: conflict_object; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.conflict_object (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    conflict_id uuid NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    object_id uuid,
    relationship_id uuid,
    CONSTRAINT conflict_object_check CHECK (((object_id IS NOT NULL) <> (relationship_id IS NOT NULL)))
);


--
-- Name: document; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.document (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    doc_key text NOT NULL,
    canonical_path text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: document_revision; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.document_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    document_id uuid NOT NULL,
    revision_no integer NOT NULL,
    git_commit_sha text,
    content_hash text NOT NULL,
    content text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT document_revision_revision_no_check CHECK ((revision_no >= 1))
);


--
-- Name: evidence; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.evidence (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    source_id uuid,
    evidence_type text NOT NULL,
    evidence_hash text NOT NULL,
    verification_status text DEFAULT 'unverified'::text NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    object_revision_id uuid NOT NULL,
    relationship_revision_id uuid,
    target_domain text,
    target_type text,
    target_id text,
    target_revision_id text,
    file_path text,
    symbol text,
    code_range text,
    commit_sha text,
    section_anchor text,
    start_line integer,
    end_line integer,
    excerpt text,
    description text,
    created_by uuid NOT NULL,
    CONSTRAINT evidence_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT evidence_subject_xor_check CHECK (((object_revision_id IS NOT NULL) <> (relationship_revision_id IS NOT NULL))),
    CONSTRAINT evidence_type_check CHECK ((evidence_type = ANY (ARRAY['code'::text, 'test_result'::text, 'commit'::text, 'document_section'::text, 'meeting'::text, 'user_statement'::text, 'system_observation'::text]))),
    CONSTRAINT evidence_type_locator_check CHECK ((((evidence_type = ANY (ARRAY['code'::text, 'test_result'::text, 'commit'::text])) AND (file_path IS NOT NULL) AND (commit_sha IS NOT NULL) AND (target_domain IS NULL)) OR ((evidence_type = 'document_section'::text) AND (target_domain = 'knowledge_base'::text) AND (target_revision_id IS NOT NULL) AND (file_path IS NULL) AND (commit_sha IS NULL)) OR ((evidence_type = ANY (ARRAY['meeting'::text, 'user_statement'::text, 'system_observation'::text])) AND ((excerpt IS NOT NULL) OR (description IS NOT NULL)) AND (target_domain IS NULL)))),
    CONSTRAINT evidence_verification_status_check CHECK ((verification_status = ANY (ARRAY['unverified'::text, 'verified'::text, 'refuted'::text, 'conflicted'::text])))
);


--
-- Name: object_type; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.object_type (
    type_key text NOT NULL,
    label text NOT NULL,
    description text,
    layer text DEFAULT 'snapshot'::text NOT NULL,
    governance_level text DEFAULT 'L1'::text NOT NULL,
    singleton boolean DEFAULT false NOT NULL,
    doc_template_key text,
    status text DEFAULT 'active'::text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT object_type_governance_level_check CHECK ((governance_level = ANY (ARRAY['L1'::text, 'L2'::text]))),
    CONSTRAINT object_type_layer_check CHECK ((layer = ANY (ARRAY['L1_identity'::text, 'L2_structure'::text, 'L3_usage'::text, 'L4_semantic'::text, 'L5_decision'::text, 'snapshot'::text]))),
    CONSTRAINT object_type_status_check CHECK ((status = ANY (ARRAY['active'::text, 'deprecated'::text])))
);


--
-- Name: proposal; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.proposal (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    rationale text NOT NULL,
    origin_kind text NOT NULL,
    source_id uuid,
    governance_level text NOT NULL,
    risk_level text DEFAULT 'medium'::text NOT NULL,
    confidence text DEFAULT 'medium'::text NOT NULL,
    expected_version_no bigint NOT NULL,
    proposal_content_hash text NOT NULL,
    status text DEFAULT 'draft'::text NOT NULL,
    submitted_at timestamp with time zone,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT proposal_confidence_check CHECK ((confidence = ANY (ARRAY['high'::text, 'medium'::text, 'low'::text]))),
    CONSTRAINT proposal_expected_version_no_check CHECK ((expected_version_no >= 0)),
    CONSTRAINT proposal_governance_level_check CHECK ((governance_level = ANY (ARRAY['L1'::text, 'L2'::text]))),
    CONSTRAINT proposal_origin_kind_check CHECK ((origin_kind = ANY (ARRAY['ai_agent'::text, 'kb_memory'::text, 'manual_input'::text, 'requirement_change'::text, 'conflict_resolution'::text]))),
    CONSTRAINT proposal_risk_level_check CHECK ((risk_level = ANY (ARRAY['low'::text, 'medium'::text, 'high'::text, 'critical'::text]))),
    CONSTRAINT proposal_status_check CHECK ((status = ANY (ARRAY['draft'::text, 'submitted'::text, 'approved'::text, 'rejected'::text, 'withdrawn'::text, 'expired'::text, 'applied'::text, 'failed'::text])))
);


--
-- Name: proposal_member; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.proposal_member (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    change_kind text NOT NULL,
    object_id uuid,
    object_revision_id uuid,
    relationship_id uuid,
    relationship_revision_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT proposal_member_change_kind_check CHECK ((change_kind = ANY (ARRAY['create_object'::text, 'update_object'::text, 'create_relationship'::text, 'update_relationship'::text, 'deprecate_object'::text, 'deprecate_relationship'::text]))),
    CONSTRAINT proposal_member_check CHECK ((((object_id IS NOT NULL) AND (object_revision_id IS NOT NULL) AND (relationship_id IS NULL) AND (relationship_revision_id IS NULL)) OR ((relationship_id IS NOT NULL) AND (relationship_revision_id IS NOT NULL) AND (object_id IS NULL) AND (object_revision_id IS NULL)))),
    CONSTRAINT proposal_member_check1 CHECK (((change_kind <> ALL (ARRAY['update_object'::text, 'deprecate_object'::text])) OR (object_id IS NOT NULL))),
    CONSTRAINT proposal_member_check2 CHECK (((change_kind <> ALL (ARRAY['update_relationship'::text, 'deprecate_relationship'::text])) OR (relationship_id IS NOT NULL)))
);


--
-- Name: relationship; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.relationship (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    relationship_type_key text NOT NULL,
    lifecycle_status text DEFAULT 'identity'::text NOT NULL,
    source_object_id uuid,
    target_object_id uuid,
    source_ext_domain text,
    source_ext_type text,
    source_ext_id text,
    source_ext_revision_id text,
    target_ext_domain text,
    target_ext_type text,
    target_ext_id text,
    target_ext_revision_id text,
    superseded_by_id uuid,
    current_revision_id uuid,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT relationship_check CHECK (((source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL))),
    CONSTRAINT relationship_check1 CHECK (((target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL))),
    CONSTRAINT relationship_check2 CHECK (((source_ext_id IS NULL) OR ((source_ext_domain IS NOT NULL) AND (source_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_check3 CHECK (((target_ext_id IS NULL) OR ((target_ext_domain IS NOT NULL) AND (target_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_check4 CHECK (((lifecycle_status <> 'superseded'::text) OR (superseded_by_id IS NOT NULL))),
    CONSTRAINT relationship_lifecycle_status_check CHECK ((lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text, 'deprecated'::text, 'superseded'::text])))
);


--
-- Name: relationship_revision; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.relationship_revision (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    relationship_id uuid NOT NULL,
    revision_no integer NOT NULL,
    relationship_type_key text NOT NULL,
    directionality text NOT NULL,
    source_object_id uuid,
    source_object_revision_id uuid,
    target_object_id uuid,
    target_object_revision_id uuid,
    source_ext_domain text,
    source_ext_type text,
    source_ext_id text,
    source_ext_revision_id text,
    target_ext_domain text,
    target_ext_type text,
    target_ext_id text,
    target_ext_revision_id text,
    properties jsonb DEFAULT '{}'::jsonb NOT NULL,
    change_summary text,
    document_id uuid,
    document_revision_id uuid,
    content_hash text NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT relationship_revision_check CHECK (((source_object_id IS NOT NULL) <> (source_ext_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check1 CHECK (((target_object_id IS NOT NULL) <> (target_ext_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check2 CHECK (((source_object_id IS NULL) OR (source_object_revision_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check3 CHECK (((target_object_id IS NULL) OR (target_object_revision_id IS NOT NULL))),
    CONSTRAINT relationship_revision_check4 CHECK (((source_ext_id IS NULL) OR ((source_ext_domain IS NOT NULL) AND (source_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_revision_check5 CHECK (((target_ext_id IS NULL) OR ((target_ext_domain IS NOT NULL) AND (target_ext_type IS NOT NULL)))),
    CONSTRAINT relationship_revision_directionality_check CHECK ((directionality = ANY (ARRAY['directed'::text, 'undirected'::text]))),
    CONSTRAINT relationship_revision_revision_no_check CHECK ((revision_no >= 1))
);


--
-- Name: relationship_type; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.relationship_type (
    type_key text NOT NULL,
    label text NOT NULL,
    directionality text NOT NULL,
    source_object_types text[] NOT NULL,
    target_object_types text[] NOT NULL,
    allow_self_reference boolean DEFAULT false NOT NULL,
    allow_duplicate boolean DEFAULT false NOT NULL,
    governance_level text NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT relationship_type_directionality_check CHECK ((directionality = ANY (ARRAY['directed'::text, 'undirected'::text]))),
    CONSTRAINT relationship_type_governance_level_check CHECK ((governance_level = ANY (ARRAY['L1'::text, 'L2'::text]))),
    CONSTRAINT relationship_type_status_check CHECK ((status = ANY (ARRAY['active'::text, 'deprecated'::text])))
);


--
-- Name: source; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.source (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    target_domain text NOT NULL,
    target_type text NOT NULL,
    target_id text NOT NULL,
    target_revision_id text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    source_kind text NOT NULL,
    revision_missing_reason text,
    section_anchor text,
    start_line integer,
    description text,
    source_content_hash text,
    collected_by uuid NOT NULL,
    locator text,
    CONSTRAINT source_kb_document_channel_check CHECK (((source_kind <> 'kb_document'::text) OR ((target_domain = 'knowledge_base'::text) AND (target_revision_id IS NOT NULL)))),
    CONSTRAINT source_kind_check CHECK ((source_kind = ANY (ARRAY['kb_document'::text, 'kb_memory'::text, 'manual_input'::text, 'requirement'::text, 'requirement_revision'::text, 'discovery_finding'::text, 'project_cognition'::text, 'external_reference'::text]))),
    CONSTRAINT source_revision_missing_check CHECK (((target_revision_id IS NOT NULL) OR (revision_missing_reason IS NOT NULL)))
);


--
-- Name: version_member; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.version_member (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    version_id uuid NOT NULL,
    object_id uuid,
    object_revision_id uuid,
    relationship_id uuid,
    relationship_revision_id uuid,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT version_member_check CHECK ((((object_id IS NOT NULL) AND (object_revision_id IS NOT NULL) AND (relationship_id IS NULL) AND (relationship_revision_id IS NULL)) OR ((relationship_id IS NOT NULL) AND (relationship_revision_id IS NOT NULL) AND (object_id IS NULL) AND (object_revision_id IS NULL))))
);


--
-- Name: v_revision_versioned; Type: VIEW; Schema: pc; Owner: -
--

CREATE VIEW pc.v_revision_versioned AS
 SELECT r.tenant_id,
    r.project_id,
    r.object_id,
    r.id AS object_revision_id,
    (vm.object_revision_id IS NOT NULL) AS in_published_version
   FROM (pc.cognition_revision r
     LEFT JOIN LATERAL ( SELECT v.object_revision_id
           FROM pc.version_member v
          WHERE ((v.tenant_id = r.tenant_id) AND (v.project_id = r.project_id) AND (v.object_revision_id = r.id))
         LIMIT 1) vm ON (true));


--
-- Name: write_operation; Type: TABLE; Schema: pc; Owner: -
--

CREATE TABLE pc.write_operation (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    operation_id text NOT NULL,
    command text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    expected_content_hash text NOT NULL,
    git_commit_sha text,
    object_revision_id uuid,
    relationship_revision_id uuid,
    version_id uuid,
    last_error text,
    retry_count integer DEFAULT 0 NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT write_operation_command_check CHECK ((command = ANY (ARRAY['pc_commit_object_revision'::text, 'pc_commit_relationship_revision'::text, 'pc_publish_version'::text]))),
    CONSTRAINT write_operation_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'git_committed'::text, 'completed'::text, 'failed'::text, 'abandoned'::text])))
);


--
-- Name: source_input; Type: TABLE; Schema: source; Owner: -
--

CREATE TABLE source.source_input (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    tenant_id uuid NOT NULL,
    project_id uuid NOT NULL,
    input_kind text NOT NULL,
    title text NOT NULL,
    content text,
    source_ref text,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT source_input_input_kind_check CHECK ((input_kind = ANY (ARRAY['human_input'::text, 'discovery'::text, 'external_feedback'::text, 'intake'::text, 'import'::text])))
);


--
-- Name: agent_definition agent_definition_code_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.agent_definition
    ADD CONSTRAINT agent_definition_code_key UNIQUE (code);


--
-- Name: agent_definition agent_definition_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.agent_definition
    ADD CONSTRAINT agent_definition_pkey PRIMARY KEY (id);


--
-- Name: agent_feedback agent_feedback_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.agent_feedback
    ADD CONSTRAINT agent_feedback_pkey PRIMARY KEY (id);


--
-- Name: ai_executor_capability ai_executor_capability_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_executor_capability
    ADD CONSTRAINT ai_executor_capability_pkey PRIMARY KEY (id);


--
-- Name: ai_executor_capability ai_executor_capability_tenant_id_principal_id_capability_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_executor_capability
    ADD CONSTRAINT ai_executor_capability_tenant_id_principal_id_capability_key UNIQUE (tenant_id, principal_id, capability);


--
-- Name: ai_role_template ai_role_template_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_role_template
    ADD CONSTRAINT ai_role_template_pkey PRIMARY KEY (id);


--
-- Name: ai_role_template ai_role_template_role_key_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_role_template
    ADD CONSTRAINT ai_role_template_role_key_key UNIQUE (role_key);


--
-- Name: ai_role_version ai_role_version_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_role_version
    ADD CONSTRAINT ai_role_version_pkey PRIMARY KEY (id);


--
-- Name: ai_role_version ai_role_version_project_role_id_version_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_role_version
    ADD CONSTRAINT ai_role_version_project_role_id_version_key UNIQUE (project_role_id, version);


--
-- Name: analysis_run analysis_run_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.analysis_run
    ADD CONSTRAINT analysis_run_pkey PRIMARY KEY (id);


--
-- Name: analysis_run analysis_run_project_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.analysis_run
    ADD CONSTRAINT analysis_run_project_id_id_key UNIQUE (project_id, id);


--
-- Name: baseline_item baseline_item_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_item
    ADD CONSTRAINT baseline_item_pkey PRIMARY KEY (id);


--
-- Name: baseline_item baseline_item_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_item
    ADD CONSTRAINT baseline_item_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: baseline_proposal baseline_proposal_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_pkey PRIMARY KEY (id);


--
-- Name: baseline_proposal baseline_proposal_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: change_proposal change_proposal_analysis_run_id_proposal_seq_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.change_proposal
    ADD CONSTRAINT change_proposal_analysis_run_id_proposal_seq_key UNIQUE (analysis_run_id, proposal_seq);


--
-- Name: change_proposal change_proposal_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.change_proposal
    ADD CONSTRAINT change_proposal_pkey PRIMARY KEY (id);


--
-- Name: cognition_item cognition_item_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_item
    ADD CONSTRAINT cognition_item_pkey PRIMARY KEY (id);


--
-- Name: cognition_item cognition_item_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_item
    ADD CONSTRAINT cognition_item_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: cognition_run cognition_run_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_run
    ADD CONSTRAINT cognition_run_pkey PRIMARY KEY (id);


--
-- Name: cognition_run cognition_run_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_run
    ADD CONSTRAINT cognition_run_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: conversation_message conversation_message_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation_message
    ADD CONSTRAINT conversation_message_pkey PRIMARY KEY (id);


--
-- Name: conversation conversation_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation
    ADD CONSTRAINT conversation_pkey PRIMARY KEY (id);


--
-- Name: conversation conversation_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation
    ADD CONSTRAINT conversation_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: document_index_state document_index_state_document_revision_id_provider_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_index_state
    ADD CONSTRAINT document_index_state_document_revision_id_provider_key UNIQUE (document_revision_id, provider);


--
-- Name: document_index_state document_index_state_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_index_state
    ADD CONSTRAINT document_index_state_pkey PRIMARY KEY (id);


--
-- Name: document_proposal document_proposal_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT document_proposal_pkey PRIMARY KEY (id);


--
-- Name: document_revision document_revision_document_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT document_revision_document_id_id_key UNIQUE (document_id, id);


--
-- Name: document_revision document_revision_document_id_revision_no_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT document_revision_document_id_revision_no_key UNIQUE (document_id, revision_no);


--
-- Name: document_revision document_revision_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT document_revision_pkey PRIMARY KEY (id);


--
-- Name: document_section_index document_section_index_document_revision_id_provider_sectio_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_section_index
    ADD CONSTRAINT document_section_index_document_revision_id_provider_sectio_key UNIQUE (document_revision_id, provider, section_no);


--
-- Name: document_section_index document_section_index_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_section_index
    ADD CONSTRAINT document_section_index_pkey PRIMARY KEY (id);


--
-- Name: knowledge_reference knowledge_reference_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_reference
    ADD CONSTRAINT knowledge_reference_pkey PRIMARY KEY (id);


--
-- Name: knowledge_write_operation knowledge_write_operation_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_write_operation
    ADD CONSTRAINT knowledge_write_operation_pkey PRIMARY KEY (id);


--
-- Name: knowledge_write_operation knowledge_write_operation_project_id_operation_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_write_operation
    ADD CONSTRAINT knowledge_write_operation_project_id_operation_id_key UNIQUE (project_id, operation_id);


--
-- Name: llm_call_record llm_call_record_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.llm_call_record
    ADD CONSTRAINT llm_call_record_pkey PRIMARY KEY (id);


--
-- Name: project_ai_config project_ai_config_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_config
    ADD CONSTRAINT project_ai_config_pkey PRIMARY KEY (project_id);


--
-- Name: project_ai_model_policy project_ai_model_policy_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_model_policy
    ADD CONSTRAINT project_ai_model_policy_pkey PRIMARY KEY (tenant_id, project_id);


--
-- Name: project_ai_role project_ai_role_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT project_ai_role_pkey PRIMARY KEY (id);


--
-- Name: project_ai_role project_ai_role_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT project_ai_role_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: project_ai_role project_ai_role_tenant_id_project_id_role_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT project_ai_role_tenant_id_project_id_role_id_key UNIQUE (tenant_id, project_id, role_id);


--
-- Name: project_baseline project_baseline_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_pkey PRIMARY KEY (id);


--
-- Name: project_baseline project_baseline_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: project_baseline project_baseline_tenant_id_project_id_version_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_tenant_id_project_id_version_key UNIQUE (tenant_id, project_id, version);


--
-- Name: project_document project_document_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_document
    ADD CONSTRAINT project_document_pkey PRIMARY KEY (id);


--
-- Name: project_document project_document_project_id_canonical_path_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_document
    ADD CONSTRAINT project_document_project_id_canonical_path_key UNIQUE (project_id, canonical_path);


--
-- Name: project_document project_document_project_id_doc_key_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_document
    ADD CONSTRAINT project_document_project_id_doc_key_key UNIQUE (project_id, doc_key);


--
-- Name: project_document project_document_project_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_document
    ADD CONSTRAINT project_document_project_id_id_key UNIQUE (project_id, id);


--
-- Name: project_knowledge project_knowledge_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_pkey PRIMARY KEY (id);


--
-- Name: project_knowledge project_knowledge_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: project_skill_binding project_skill_binding_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_skill_binding
    ADD CONSTRAINT project_skill_binding_pkey PRIMARY KEY (project_id, skill_id);


--
-- Name: skill_definition skill_definition_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.skill_definition
    ADD CONSTRAINT skill_definition_pkey PRIMARY KEY (id);


--
-- Name: team_llm_credential team_llm_credential_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_credential
    ADD CONSTRAINT team_llm_credential_pkey PRIMARY KEY (id);


--
-- Name: team_llm_credential team_llm_credential_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_credential
    ADD CONSTRAINT team_llm_credential_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: team_llm_provider team_llm_provider_pkey; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_pkey PRIMARY KEY (id);


--
-- Name: team_llm_provider team_llm_provider_tenant_id_id_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: team_llm_provider team_llm_provider_tenant_id_name_key; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_tenant_id_name_key UNIQUE (tenant_id, name);


--
-- Name: analysis_run uq_analysis_run_tenant_id; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.analysis_run
    ADD CONSTRAINT uq_analysis_run_tenant_id UNIQUE (tenant_id, id);


--
-- Name: change_proposal uq_change_proposal_tenant_id; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.change_proposal
    ADD CONSTRAINT uq_change_proposal_tenant_id UNIQUE (tenant_id, id);


--
-- Name: document_proposal uq_document_proposal_tenant_id; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT uq_document_proposal_tenant_id UNIQUE (tenant_id, id);


--
-- Name: document_revision uq_document_revision_tenant_id; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT uq_document_revision_tenant_id UNIQUE (tenant_id, id);


--
-- Name: project_document uq_project_document_tenant_id; Type: CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_document
    ADD CONSTRAINT uq_project_document_tenant_id UNIQUE (tenant_id, id);


--
-- Name: approver_grant approver_grant_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_pkey PRIMARY KEY (id);


--
-- Name: approver_grant approver_grant_project_id_principal_id_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_project_id_principal_id_key UNIQUE (project_id, principal_id);


--
-- Name: customer customer_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.customer
    ADD CONSTRAINT customer_pkey PRIMARY KEY (id);


--
-- Name: principal_identity principal_identity_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.principal_identity
    ADD CONSTRAINT principal_identity_pkey PRIMARY KEY (id);


--
-- Name: principal principal_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.principal
    ADD CONSTRAINT principal_pkey PRIMARY KEY (id);


--
-- Name: project project_id_customer_id_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project
    ADD CONSTRAINT project_id_customer_id_key UNIQUE (id, customer_id);


--
-- Name: project_initialization_operation project_initialization_operat_tenant_id_project_id_generati_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_initialization_operation
    ADD CONSTRAINT project_initialization_operat_tenant_id_project_id_generati_key UNIQUE (tenant_id, project_id, generation);


--
-- Name: project_initialization_operation project_initialization_operation_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_initialization_operation
    ADD CONSTRAINT project_initialization_operation_pkey PRIMARY KEY (id);


--
-- Name: project_member project_member_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_member
    ADD CONSTRAINT project_member_pkey PRIMARY KEY (project_id, principal_id);


--
-- Name: project project_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project
    ADD CONSTRAINT project_pkey PRIMARY KEY (id);


--
-- Name: tenant_agent tenant_agent_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_agent
    ADD CONSTRAINT tenant_agent_pkey PRIMARY KEY (tenant_id, agent_definition_id);


--
-- Name: tenant_agent tenant_agent_principal_id_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_agent
    ADD CONSTRAINT tenant_agent_principal_id_key UNIQUE (principal_id);


--
-- Name: tenant_invitation_link tenant_invitation_link_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link
    ADD CONSTRAINT tenant_invitation_link_pkey PRIMARY KEY (id);


--
-- Name: tenant_invitation_link_redemption tenant_invitation_link_redemption_link_id_principal_id_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link_redemption
    ADD CONSTRAINT tenant_invitation_link_redemption_link_id_principal_id_key UNIQUE (link_id, principal_id);


--
-- Name: tenant_invitation_link_redemption tenant_invitation_link_redemption_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link_redemption
    ADD CONSTRAINT tenant_invitation_link_redemption_pkey PRIMARY KEY (id);


--
-- Name: tenant_invitation_link tenant_invitation_link_tenant_id_id_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link
    ADD CONSTRAINT tenant_invitation_link_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: tenant_invitation tenant_invitation_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation
    ADD CONSTRAINT tenant_invitation_pkey PRIMARY KEY (id);


--
-- Name: tenant_invitation tenant_invitation_tenant_id_id_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation
    ADD CONSTRAINT tenant_invitation_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: tenant_member tenant_member_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_member
    ADD CONSTRAINT tenant_member_pkey PRIMARY KEY (tenant_id, principal_id);


--
-- Name: tenant tenant_pkey; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant
    ADD CONSTRAINT tenant_pkey PRIMARY KEY (id);


--
-- Name: tenant tenant_provider_external_key_key; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant
    ADD CONSTRAINT tenant_provider_external_key_key UNIQUE (provider, external_key);


--
-- Name: customer uq_customer_tenant_id; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.customer
    ADD CONSTRAINT uq_customer_tenant_id UNIQUE (tenant_id, id);


--
-- Name: principal_identity uq_principal_identity; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.principal_identity
    ADD CONSTRAINT uq_principal_identity UNIQUE (provider, tenant_id, subject);


--
-- Name: project uq_project_tenant_id; Type: CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project
    ADD CONSTRAINT uq_project_tenant_id UNIQUE (tenant_id, id);


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_criterion_id_review_run_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_criterion_id_review_run_id_key UNIQUE (criterion_id, review_run_id);


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_pkey PRIMARY KEY (id);


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_tenant_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: continuation continuation_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_pkey PRIMARY KEY (id);


--
-- Name: continuation continuation_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_project_id_id_key UNIQUE (project_id, id);


--
-- Name: continuation continuation_run_id_generation_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_run_id_generation_key UNIQUE (run_id, generation);


--
-- Name: interaction_response_delivery interaction_response_delivery_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_pkey PRIMARY KEY (id);


--
-- Name: interaction_response_delivery interaction_response_delivery_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_project_id_id_key UNIQUE (project_id, id);


--
-- Name: interaction_response interaction_response_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_pkey PRIMARY KEY (id);


--
-- Name: interaction_response interaction_response_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_project_id_id_key UNIQUE (project_id, id);


--
-- Name: milestone milestone_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.milestone
    ADD CONSTRAINT milestone_pkey PRIMARY KEY (id);


--
-- Name: milestone milestone_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.milestone
    ADD CONSTRAINT milestone_project_id_id_key UNIQUE (project_id, id);


--
-- Name: policy_scope policy_scope_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.policy_scope
    ADD CONSTRAINT policy_scope_pkey PRIMARY KEY (tenant_id, scope, scope_key);


--
-- Name: requirement_acceptance_criterion requirement_acceptance_criterion_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_acceptance_criterion
    ADD CONSTRAINT requirement_acceptance_criterion_pkey PRIMARY KEY (id);


--
-- Name: requirement_acceptance_criterion requirement_acceptance_criterion_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_acceptance_criterion
    ADD CONSTRAINT requirement_acceptance_criterion_project_id_id_key UNIQUE (project_id, id);


--
-- Name: requirement_confirmation requirement_confirmation_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_confirmation
    ADD CONSTRAINT requirement_confirmation_pkey PRIMARY KEY (id);


--
-- Name: requirement_finding requirement_finding_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_finding
    ADD CONSTRAINT requirement_finding_pkey PRIMARY KEY (requirement_id, finding_id);


--
-- Name: requirement requirement_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement
    ADD CONSTRAINT requirement_pkey PRIMARY KEY (id);


--
-- Name: requirement requirement_project_id_code_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement
    ADD CONSTRAINT requirement_project_id_code_key UNIQUE (project_id, code);


--
-- Name: requirement requirement_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement
    ADD CONSTRAINT requirement_project_id_id_key UNIQUE (project_id, id);


--
-- Name: requirement_revision requirement_revision_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_pkey PRIMARY KEY (id);


--
-- Name: requirement_revision requirement_revision_requirement_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_requirement_id_id_key UNIQUE (requirement_id, id);


--
-- Name: requirement_revision requirement_revision_requirement_id_version_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_requirement_id_version_key UNIQUE (requirement_id, version);


--
-- Name: requirement_source_input requirement_source_input_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_source_input
    ADD CONSTRAINT requirement_source_input_pkey PRIMARY KEY (requirement_id, input_id);


--
-- Name: review_finding review_finding_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.review_finding
    ADD CONSTRAINT review_finding_pkey PRIMARY KEY (id);


--
-- Name: review_finding review_finding_tenant_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.review_finding
    ADD CONSTRAINT review_finding_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: task_acceptance_criterion task_acceptance_criterion_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_acceptance_criterion
    ADD CONSTRAINT task_acceptance_criterion_pkey PRIMARY KEY (id);


--
-- Name: task_acceptance_criterion task_acceptance_criterion_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_acceptance_criterion
    ADD CONSTRAINT task_acceptance_criterion_project_id_id_key UNIQUE (project_id, id);


--
-- Name: task_acceptance_criterion task_acceptance_criterion_task_id_ordinal_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_acceptance_criterion
    ADD CONSTRAINT task_acceptance_criterion_task_id_ordinal_key UNIQUE (task_id, ordinal);


--
-- Name: task_checkpoint task_checkpoint_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_pkey PRIMARY KEY (id);


--
-- Name: task_checkpoint task_checkpoint_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_project_id_id_key UNIQUE (project_id, id);


--
-- Name: task_checkpoint task_checkpoint_run_id_content_hash_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_run_id_content_hash_key UNIQUE (run_id, content_hash);


--
-- Name: task_dependency task_dependency_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_pkey PRIMARY KEY (id);


--
-- Name: task_dependency task_dependency_task_id_depends_on_task_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_task_id_depends_on_task_id_key UNIQUE (task_id, depends_on_task_id);


--
-- Name: task_dependency task_dependency_tenant_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: task_interaction task_interaction_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_pkey PRIMARY KEY (id);


--
-- Name: task_interaction task_interaction_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_project_id_id_key UNIQUE (project_id, id);


--
-- Name: task_interaction task_interaction_run_id_event_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_run_id_event_id_key UNIQUE (run_id, event_id);


--
-- Name: task_knowledge_ref task_knowledge_ref_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_knowledge_ref
    ADD CONSTRAINT task_knowledge_ref_pkey PRIMARY KEY (task_id, document_id);


--
-- Name: task task_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT task_pkey PRIMARY KEY (id);


--
-- Name: task_policy_override task_policy_override_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_policy_override
    ADD CONSTRAINT task_policy_override_pkey PRIMARY KEY (task_id);


--
-- Name: task task_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT task_project_id_id_key UNIQUE (project_id, id);


--
-- Name: task_repository_commit task_repository_commit_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository_commit
    ADD CONSTRAINT task_repository_commit_pkey PRIMARY KEY (id);


--
-- Name: task_repository_commit task_repository_commit_task_run_id_repository_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository_commit
    ADD CONSTRAINT task_repository_commit_task_run_id_repository_id_key UNIQUE (task_run_id, repository_id);


--
-- Name: task_repository_commit task_repository_commit_tenant_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository_commit
    ADD CONSTRAINT task_repository_commit_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: task_repository task_repository_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository
    ADD CONSTRAINT task_repository_pkey PRIMARY KEY (task_id, repository_id);


--
-- Name: task_requirement task_requirement_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_requirement
    ADD CONSTRAINT task_requirement_pkey PRIMARY KEY (task_id, revision_id);


--
-- Name: task_run_cleanup task_run_cleanup_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_cleanup
    ADD CONSTRAINT task_run_cleanup_pkey PRIMARY KEY (run_id);


--
-- Name: task_run_execution_authorization task_run_execution_authorization_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_pkey PRIMARY KEY (id);


--
-- Name: task_run_execution_authorization task_run_execution_authorization_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_project_id_id_key UNIQUE (project_id, id);


--
-- Name: task_run_failure task_run_failure_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_failure
    ADD CONSTRAINT task_run_failure_pkey PRIMARY KEY (run_id);


--
-- Name: task_run task_run_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT task_run_pkey PRIMARY KEY (id);


--
-- Name: task_run_repository task_run_repository_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_repository
    ADD CONSTRAINT task_run_repository_pkey PRIMARY KEY (run_id, repository_id);


--
-- Name: task_supersession task_supersession_old_task_id_new_task_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT task_supersession_old_task_id_new_task_id_key UNIQUE (old_task_id, new_task_id);


--
-- Name: task_supersession task_supersession_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT task_supersession_pkey PRIMARY KEY (id);


--
-- Name: task_waiting task_waiting_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_waiting
    ADD CONSTRAINT task_waiting_pkey PRIMARY KEY (id);


--
-- Name: milestone uq_milestone_tenant_id; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.milestone
    ADD CONSTRAINT uq_milestone_tenant_id UNIQUE (tenant_id, id);


--
-- Name: requirement_confirmation uq_requirement_confirmation_revision_action; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_confirmation
    ADD CONSTRAINT uq_requirement_confirmation_revision_action UNIQUE (revision_id, action);


--
-- Name: requirement_revision uq_requirement_revision_tenant_id; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT uq_requirement_revision_tenant_id UNIQUE (tenant_id, id);


--
-- Name: requirement uq_requirement_tenant_id; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement
    ADD CONSTRAINT uq_requirement_tenant_id UNIQUE (tenant_id, id);


--
-- Name: task_run uq_task_run_project_id; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT uq_task_run_project_id UNIQUE (project_id, id);


--
-- Name: task_run uq_task_run_tenant_id; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT uq_task_run_tenant_id UNIQUE (tenant_id, id);


--
-- Name: task uq_task_tenant_id; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT uq_task_tenant_id UNIQUE (tenant_id, id);


--
-- Name: worker_command worker_command_pkey; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_pkey PRIMARY KEY (id);


--
-- Name: worker_command worker_command_project_id_id_key; Type: CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_project_id_id_key UNIQUE (project_id, id);


--
-- Name: device device_pkey; Type: CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device
    ADD CONSTRAINT device_pkey PRIMARY KEY (id);


--
-- Name: device_project_binding device_project_binding_device_id_project_id_key; Type: CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device_project_binding
    ADD CONSTRAINT device_project_binding_device_id_project_id_key UNIQUE (device_id, project_id);


--
-- Name: device_project_binding device_project_binding_pkey; Type: CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device_project_binding
    ADD CONSTRAINT device_project_binding_pkey PRIMARY KEY (id);


--
-- Name: device_project_binding device_project_binding_tenant_id_id_key; Type: CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device_project_binding
    ADD CONSTRAINT device_project_binding_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: device device_tenant_id_id_key; Type: CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device
    ADD CONSTRAINT device_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: device uq_device_tenant_id; Type: CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device
    ADD CONSTRAINT uq_device_tenant_id UNIQUE (tenant_id, id);


--
-- Name: discovery_answer discovery_answer_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_answer
    ADD CONSTRAINT discovery_answer_pkey PRIMARY KEY (id);


--
-- Name: discovery_answer discovery_answer_tenant_id_id_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_answer
    ADD CONSTRAINT discovery_answer_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: discovery_finding_evidence discovery_finding_evidence_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_evidence
    ADD CONSTRAINT discovery_finding_evidence_pkey PRIMARY KEY (id);


--
-- Name: discovery_finding discovery_finding_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding
    ADD CONSTRAINT discovery_finding_pkey PRIMARY KEY (id);


--
-- Name: discovery_finding_repository discovery_finding_repository_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_repository
    ADD CONSTRAINT discovery_finding_repository_pkey PRIMARY KEY (tenant_id, finding_id, repository_id);


--
-- Name: discovery_finding discovery_finding_tenant_id_id_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding
    ADD CONSTRAINT discovery_finding_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: discovery_message discovery_message_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_message
    ADD CONSTRAINT discovery_message_pkey PRIMARY KEY (id);


--
-- Name: discovery_message discovery_message_tenant_id_id_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_message
    ADD CONSTRAINT discovery_message_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: discovery_message discovery_message_tenant_id_session_id_seq_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_message
    ADD CONSTRAINT discovery_message_tenant_id_session_id_seq_key UNIQUE (tenant_id, session_id, seq);


--
-- Name: discovery_proposal discovery_proposal_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_proposal
    ADD CONSTRAINT discovery_proposal_pkey PRIMARY KEY (id);


--
-- Name: discovery_proposal discovery_proposal_tenant_id_id_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_proposal
    ADD CONSTRAINT discovery_proposal_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: discovery_question discovery_question_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_question
    ADD CONSTRAINT discovery_question_pkey PRIMARY KEY (id);


--
-- Name: discovery_question discovery_question_tenant_id_id_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_question
    ADD CONSTRAINT discovery_question_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: discovery_session discovery_session_pkey; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_session
    ADD CONSTRAINT discovery_session_pkey PRIMARY KEY (id);


--
-- Name: discovery_session discovery_session_tenant_id_id_key; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_session
    ADD CONSTRAINT discovery_session_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: discovery_finding uq_discovery_finding_tenant_project_id; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding
    ADD CONSTRAINT uq_discovery_finding_tenant_project_id UNIQUE (tenant_id, project_id, id);


--
-- Name: discovery_session uq_discovery_session_tenant_project_id; Type: CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_session
    ADD CONSTRAINT uq_discovery_session_tenant_project_id UNIQUE (tenant_id, project_id, id);


--
-- Name: approval_request approval_request_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.approval_request
    ADD CONSTRAINT approval_request_pkey PRIMARY KEY (id);


--
-- Name: approval_request approval_request_proposal_id_key; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.approval_request
    ADD CONSTRAINT approval_request_proposal_id_key UNIQUE (proposal_id);


--
-- Name: audit_log audit_log_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.audit_log
    ADD CONSTRAINT audit_log_pkey PRIMARY KEY (id);


--
-- Name: human_intervention human_intervention_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.human_intervention
    ADD CONSTRAINT human_intervention_pkey PRIMARY KEY (id);


--
-- Name: review_approval review_approval_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_pkey PRIMARY KEY (id);


--
-- Name: review_approval review_approval_tenant_id_id_key; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: tenant_audit_log tenant_audit_log_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.tenant_audit_log
    ADD CONSTRAINT tenant_audit_log_pkey PRIMARY KEY (id);


--
-- Name: approval_request uq_approval_request_tenant_id; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.approval_request
    ADD CONSTRAINT uq_approval_request_tenant_id UNIQUE (tenant_id, id);


--
-- Name: inbound_event inbound_event_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.inbound_event
    ADD CONSTRAINT inbound_event_pkey PRIMARY KEY (id);


--
-- Name: inbound_event inbound_event_project_id_id_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.inbound_event
    ADD CONSTRAINT inbound_event_project_id_id_key UNIQUE (project_id, id);


--
-- Name: inbound_event inbound_event_project_id_source_idempotency_key_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.inbound_event
    ADD CONSTRAINT inbound_event_project_id_source_idempotency_key_key UNIQUE (project_id, source, idempotency_key);


--
-- Name: intake_record intake_record_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_record
    ADD CONSTRAINT intake_record_pkey PRIMARY KEY (id);


--
-- Name: intake_record intake_record_tenant_id_id_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_record
    ADD CONSTRAINT intake_record_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: intake_record intake_record_tenant_id_project_id_idempotency_key_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_record
    ADD CONSTRAINT intake_record_tenant_id_project_id_idempotency_key_key UNIQUE (tenant_id, project_id, idempotency_key);


--
-- Name: intake_task intake_task_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_task
    ADD CONSTRAINT intake_task_pkey PRIMARY KEY (id);


--
-- Name: intake_task intake_task_tenant_id_project_id_idempotency_key_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_task
    ADD CONSTRAINT intake_task_tenant_id_project_id_idempotency_key_key UNIQUE (tenant_id, project_id, idempotency_key);


--
-- Name: job job_event_id_job_type_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.job
    ADD CONSTRAINT job_event_id_job_type_key UNIQUE (event_id, job_type);


--
-- Name: job job_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.job
    ADD CONSTRAINT job_pkey PRIMARY KEY (id);


--
-- Name: notification notification_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.notification
    ADD CONSTRAINT notification_pkey PRIMARY KEY (id);


--
-- Name: notification notification_source_proposal_id_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.notification
    ADD CONSTRAINT notification_source_proposal_id_key UNIQUE (source_proposal_id);


--
-- Name: outbox_event outbox_event_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.outbox_event
    ADD CONSTRAINT outbox_event_pkey PRIMARY KEY (id);


--
-- Name: outbox_event outbox_event_tenant_idempotency_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.outbox_event
    ADD CONSTRAINT outbox_event_tenant_idempotency_key UNIQUE (tenant_id, idempotency_key);


--
-- Name: project_repository project_repository_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_repository
    ADD CONSTRAINT project_repository_pkey PRIMARY KEY (id);


--
-- Name: project_repository project_repository_project_id_id_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_repository
    ADD CONSTRAINT project_repository_project_id_id_key UNIQUE (project_id, id);


--
-- Name: project_repository project_repository_project_id_url_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_repository
    ADD CONSTRAINT project_repository_project_id_url_key UNIQUE (project_id, url);


--
-- Name: project_ssh_key project_ssh_key_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_ssh_key
    ADD CONSTRAINT project_ssh_key_pkey PRIMARY KEY (project_id);


--
-- Name: repo_import_draft_version repo_import_draft_version_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_draft_version
    ADD CONSTRAINT repo_import_draft_version_pkey PRIMARY KEY (tenant_id, import_id, revision);


--
-- Name: repo_import repo_import_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT repo_import_pkey PRIMARY KEY (id);


--
-- Name: repo_import repo_import_project_id_id_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT repo_import_project_id_id_key UNIQUE (project_id, id);


--
-- Name: repo_import_source_attempt repo_import_source_attempt_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source_attempt
    ADD CONSTRAINT repo_import_source_attempt_pkey PRIMARY KEY (tenant_id, import_id, repository_id, attempt_no);


--
-- Name: repo_import_source_attempt repo_import_source_attempt_tenant_id_project_id_import_id_r_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source_attempt
    ADD CONSTRAINT repo_import_source_attempt_tenant_id_project_id_import_id_r_key UNIQUE (tenant_id, project_id, import_id, repository_id, attempt_no);


--
-- Name: repo_import_source repo_import_source_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source
    ADD CONSTRAINT repo_import_source_pkey PRIMARY KEY (tenant_id, import_id, repository_id);


--
-- Name: repo_import_source repo_import_source_tenant_id_project_id_import_id_repositor_key; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source
    ADD CONSTRAINT repo_import_source_tenant_id_project_id_import_id_repositor_key UNIQUE (tenant_id, project_id, import_id, repository_id);


--
-- Name: repository_version_confirmation repository_version_confirmation_pkey; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repository_version_confirmation
    ADD CONSTRAINT repository_version_confirmation_pkey PRIMARY KEY (id);


--
-- Name: inbound_event uq_inbound_event_tenant_id; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.inbound_event
    ADD CONSTRAINT uq_inbound_event_tenant_id UNIQUE (tenant_id, id);


--
-- Name: project_repository uq_project_repository_tenant_id; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_repository
    ADD CONSTRAINT uq_project_repository_tenant_id UNIQUE (tenant_id, id);


--
-- Name: project_repository uq_project_repository_tenant_project_id; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_repository
    ADD CONSTRAINT uq_project_repository_tenant_project_id UNIQUE (tenant_id, project_id, id);


--
-- Name: repo_import uq_repo_import_tenant_id; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT uq_repo_import_tenant_id UNIQUE (tenant_id, id);


--
-- Name: repo_import uq_repo_import_tenant_project_id; Type: CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT uq_repo_import_tenant_project_id UNIQUE (tenant_id, project_id, id);


--
-- Name: document document_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document
    ADD CONSTRAINT document_pkey PRIMARY KEY (id);


--
-- Name: document_revision document_revision_document_id_revision_no_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document_revision
    ADD CONSTRAINT document_revision_document_id_revision_no_key UNIQUE (document_id, revision_no);


--
-- Name: document_revision document_revision_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document_revision
    ADD CONSTRAINT document_revision_pkey PRIMARY KEY (id);


--
-- Name: document_revision document_revision_tenant_id_id_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document_revision
    ADD CONSTRAINT document_revision_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: document document_tenant_id_project_id_doc_key_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document
    ADD CONSTRAINT document_tenant_id_project_id_doc_key_key UNIQUE (tenant_id, project_id, doc_key);


--
-- Name: memory memory_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory
    ADD CONSTRAINT memory_pkey PRIMARY KEY (id);


--
-- Name: memory_ref memory_ref_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_ref
    ADD CONSTRAINT memory_ref_pkey PRIMARY KEY (id);


--
-- Name: memory_ref memory_ref_revision_id_ref_role_target_domain_target_type_t_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_ref
    ADD CONSTRAINT memory_ref_revision_id_ref_role_target_domain_target_type_t_key UNIQUE (revision_id, ref_role, target_domain, target_type, target_id);


--
-- Name: memory_revision memory_revision_memory_id_revision_no_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_memory_id_revision_no_key UNIQUE (memory_id, revision_no);


--
-- Name: memory_revision memory_revision_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_pkey PRIMARY KEY (id);


--
-- Name: memory_revision memory_revision_tenant_id_id_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: memory memory_tenant_id_id_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory
    ADD CONSTRAINT memory_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: source source_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source
    ADD CONSTRAINT source_pkey PRIMARY KEY (id);


--
-- Name: source_project source_project_pkey; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source_project
    ADD CONSTRAINT source_project_pkey PRIMARY KEY (source_id, project_id);


--
-- Name: source source_tenant_id_content_hash_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source
    ADD CONSTRAINT source_tenant_id_content_hash_key UNIQUE (tenant_id, content_hash);


--
-- Name: source source_tenant_id_id_key; Type: CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source
    ADD CONSTRAINT source_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: approval approval_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_pkey PRIMARY KEY (id);


--
-- Name: approval approval_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: approval approval_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: cognition_object cognition_object_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_pkey PRIMARY KEY (id);


--
-- Name: cognition_object cognition_object_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: cognition_object cognition_object_tenant_id_project_id_object_key_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_id_project_id_object_key_key UNIQUE (tenant_id, project_id, object_key);


--
-- Name: cognition_object cognition_object_tenant_project_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_project_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: cognition_revision cognition_revision_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_pkey PRIMARY KEY (id);


--
-- Name: cognition_revision cognition_revision_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: cognition_revision cognition_revision_tenant_project_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_tenant_project_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: cognition_revision cognition_revision_tenant_project_object_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_tenant_project_object_id_key UNIQUE (tenant_id, project_id, object_id, id);


--
-- Name: cognition_revision cognition_revision_tenant_project_object_no_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_tenant_project_object_no_key UNIQUE (tenant_id, project_id, object_id, revision_no);


--
-- Name: cognition_version cognition_version_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_pkey PRIMARY KEY (id);


--
-- Name: cognition_version cognition_version_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: cognition_version cognition_version_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: cognition_version cognition_version_tenant_id_project_id_version_no_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_version_no_key UNIQUE (tenant_id, project_id, version_no);


--
-- Name: conflict_object conflict_object_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict_object
    ADD CONSTRAINT conflict_object_pkey PRIMARY KEY (id);


--
-- Name: conflict conflict_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_pkey PRIMARY KEY (id);


--
-- Name: conflict conflict_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: conflict conflict_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: document document_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document
    ADD CONSTRAINT document_pkey PRIMARY KEY (id);


--
-- Name: document_revision document_revision_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_pkey PRIMARY KEY (id);


--
-- Name: document_revision document_revision_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: document_revision document_revision_tenant_id_project_id_document_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_document_id_id_key UNIQUE (tenant_id, project_id, document_id, id);


--
-- Name: document_revision document_revision_tenant_id_project_id_document_id_revision_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_document_id_revision_key UNIQUE (tenant_id, project_id, document_id, revision_no);


--
-- Name: document_revision document_revision_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: document document_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document
    ADD CONSTRAINT document_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: document document_tenant_id_project_id_doc_key_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document
    ADD CONSTRAINT document_tenant_id_project_id_doc_key_key UNIQUE (tenant_id, project_id, doc_key);


--
-- Name: document document_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document
    ADD CONSTRAINT document_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: evidence evidence_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_pkey PRIMARY KEY (id);


--
-- Name: evidence evidence_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: object_type object_type_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.object_type
    ADD CONSTRAINT object_type_pkey PRIMARY KEY (type_key);


--
-- Name: proposal_member proposal_member_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_pkey PRIMARY KEY (id);


--
-- Name: proposal_member proposal_member_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: proposal proposal_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_pkey PRIMARY KEY (id);


--
-- Name: proposal proposal_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: proposal proposal_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: relationship relationship_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_pkey PRIMARY KEY (id);


--
-- Name: relationship_revision relationship_revision_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_pkey PRIMARY KEY (id);


--
-- Name: relationship_revision relationship_revision_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_relationship_id__key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_relationship_id__key UNIQUE (tenant_id, project_id, relationship_id, revision_no);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_relationship_id_key1; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_relationship_id_key1 UNIQUE (tenant_id, project_id, relationship_id, id);


--
-- Name: relationship relationship_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: relationship relationship_tenant_id_project_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: relationship_type relationship_type_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_type
    ADD CONSTRAINT relationship_type_pkey PRIMARY KEY (type_key);


--
-- Name: source source_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.source
    ADD CONSTRAINT source_pkey PRIMARY KEY (id);


--
-- Name: source source_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.source
    ADD CONSTRAINT source_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: source source_tenant_project_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.source
    ADD CONSTRAINT source_tenant_project_id_key UNIQUE (tenant_id, project_id, id);


--
-- Name: version_member version_member_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_pkey PRIMARY KEY (id);


--
-- Name: version_member version_member_tenant_id_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_id_key UNIQUE (tenant_id, id);


--
-- Name: write_operation write_operation_pkey; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.write_operation
    ADD CONSTRAINT write_operation_pkey PRIMARY KEY (id);


--
-- Name: write_operation write_operation_tenant_id_project_id_operation_id_key; Type: CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.write_operation
    ADD CONSTRAINT write_operation_tenant_id_project_id_operation_id_key UNIQUE (tenant_id, project_id, operation_id);


--
-- Name: source_input source_input_pkey; Type: CONSTRAINT; Schema: source; Owner: -
--

ALTER TABLE ONLY source.source_input
    ADD CONSTRAINT source_input_pkey PRIMARY KEY (id);


--
-- Name: source_input source_input_project_id_id_key; Type: CONSTRAINT; Schema: source; Owner: -
--

ALTER TABLE ONLY source.source_input
    ADD CONSTRAINT source_input_project_id_id_key UNIQUE (project_id, id);


--
-- Name: idx_ai_executor_capability_active; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_ai_executor_capability_active ON ai.ai_executor_capability USING btree (tenant_id, capability) WHERE (revoked_at IS NULL);


--
-- Name: idx_analysis_status_time; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_analysis_status_time ON ai.analysis_run USING btree (status, created_at);


--
-- Name: idx_baseline_item_proposal; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_baseline_item_proposal ON ai.baseline_item USING btree (proposal_id, state);


--
-- Name: idx_cognition_item_run; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_cognition_item_run ON ai.cognition_item USING btree (run_id, state);


--
-- Name: idx_cognition_run_project; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_cognition_run_project ON ai.cognition_run USING btree (project_id, created_at DESC);


--
-- Name: idx_conversation_message_conversation; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_conversation_message_conversation ON ai.conversation_message USING btree (conversation_id, created_at);


--
-- Name: idx_conversation_project_principal; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_conversation_project_principal ON ai.conversation USING btree (project_id, principal_id, updated_at DESC);


--
-- Name: idx_document_index_state_project; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_index_state_project ON ai.document_index_state USING btree (project_id, status);


--
-- Name: idx_document_proposal_document; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_proposal_document ON ai.document_proposal USING btree (document_id, created_at DESC) WHERE (document_id IS NOT NULL);


--
-- Name: idx_document_proposal_project_status; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_proposal_project_status ON ai.document_proposal USING btree (project_id, status, created_at DESC);


--
-- Name: idx_document_revision_document; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_revision_document ON ai.document_revision USING btree (document_id, revision_no DESC);


--
-- Name: idx_document_revision_source_event; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_revision_source_event ON ai.document_revision USING btree (source_event_id) WHERE (source_event_id IS NOT NULL);


--
-- Name: idx_document_section_project; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_section_project ON ai.document_section_index USING btree (tenant_id, project_id, provider);


--
-- Name: idx_document_section_revision; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_document_section_revision ON ai.document_section_index USING btree (document_revision_id, provider);


--
-- Name: idx_knowledge_reference_document; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_knowledge_reference_document ON ai.knowledge_reference USING btree (document_id, created_at DESC);


--
-- Name: idx_knowledge_write_op_revision; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_knowledge_write_op_revision ON ai.knowledge_write_operation USING btree (revision_id) WHERE (revision_id IS NOT NULL);


--
-- Name: idx_project_knowledge_active; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_project_knowledge_active ON ai.project_knowledge USING btree (project_id, status, confirmed_at DESC);


--
-- Name: idx_proposal_executed_time; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_proposal_executed_time ON ai.change_proposal USING btree (project_id, executed_at DESC) WHERE (executed_at IS NOT NULL);


--
-- Name: idx_proposal_project_status; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX idx_proposal_project_status ON ai.change_proposal USING btree (project_id, status);


--
-- Name: llm_call_record_provider_idx; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX llm_call_record_provider_idx ON ai.llm_call_record USING btree (provider_id);


--
-- Name: llm_call_record_tenant_idx; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX llm_call_record_tenant_idx ON ai.llm_call_record USING btree (tenant_id, created_at DESC);


--
-- Name: project_ai_model_policy_provider_idx; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX project_ai_model_policy_provider_idx ON ai.project_ai_model_policy USING btree (provider_id);


--
-- Name: team_llm_provider_tenant_idx; Type: INDEX; Schema: ai; Owner: -
--

CREATE INDEX team_llm_provider_tenant_idx ON ai.team_llm_provider USING btree (tenant_id, status);


--
-- Name: uq_baseline_proposal_pending; Type: INDEX; Schema: ai; Owner: -
--

CREATE UNIQUE INDEX uq_baseline_proposal_pending ON ai.baseline_proposal USING btree (tenant_id, project_id) WHERE (status = 'pending'::text);


--
-- Name: uq_knowledge_reference_identity; Type: INDEX; Schema: ai; Owner: -
--

CREATE UNIQUE INDEX uq_knowledge_reference_identity ON ai.knowledge_reference USING btree (document_revision_id, COALESCE(section_anchor, ''::text), source_kind, COALESCE(source_id, '00000000-0000-0000-0000-000000000000'::uuid));


--
-- Name: approver_grant_active_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX approver_grant_active_idx ON core.approver_grant USING btree (tenant_id, project_id) WHERE (revoked_at IS NULL);


--
-- Name: customer_code_uniq; Type: INDEX; Schema: core; Owner: -
--

CREATE UNIQUE INDEX customer_code_uniq ON core.customer USING btree (tenant_id, code) WHERE (code IS NOT NULL);


--
-- Name: customer_parent_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX customer_parent_idx ON core.customer USING btree (parent_id);


--
-- Name: customer_tenant_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX customer_tenant_idx ON core.customer USING btree (tenant_id, created_at DESC);


--
-- Name: idx_principal_identity_principal; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX idx_principal_identity_principal ON core.principal_identity USING btree (principal_id);


--
-- Name: idx_project_member_tenant_principal; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX idx_project_member_tenant_principal ON core.project_member USING btree (tenant_id, principal_id);


--
-- Name: project_code_uniq; Type: INDEX; Schema: core; Owner: -
--

CREATE UNIQUE INDEX project_code_uniq ON core.project USING btree (tenant_id, code);


--
-- Name: project_customer_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX project_customer_idx ON core.project USING btree (customer_id);


--
-- Name: project_tenant_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX project_tenant_idx ON core.project USING btree (tenant_id, created_at DESC);


--
-- Name: tenant_invitation_email_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX tenant_invitation_email_idx ON core.tenant_invitation USING btree (email_normalized);


--
-- Name: tenant_invitation_link_tenant_status_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX tenant_invitation_link_tenant_status_idx ON core.tenant_invitation_link USING btree (tenant_id, status);


--
-- Name: tenant_invitation_tenant_status_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX tenant_invitation_tenant_status_idx ON core.tenant_invitation USING btree (tenant_id, status);


--
-- Name: tenant_member_principal_idx; Type: INDEX; Schema: core; Owner: -
--

CREATE INDEX tenant_member_principal_idx ON core.tenant_member USING btree (principal_id);


--
-- Name: uq_project_initialization_operation_active; Type: INDEX; Schema: core; Owner: -
--

CREATE UNIQUE INDEX uq_project_initialization_operation_active ON core.project_initialization_operation USING btree (project_id) WHERE (status = 'active'::text);


--
-- Name: uq_tenant_invitation_link_token_hash; Type: INDEX; Schema: core; Owner: -
--

CREATE UNIQUE INDEX uq_tenant_invitation_link_token_hash ON core.tenant_invitation_link USING btree (token_hash);


--
-- Name: uq_tenant_invitation_pending_email; Type: INDEX; Schema: core; Owner: -
--

CREATE UNIQUE INDEX uq_tenant_invitation_pending_email ON core.tenant_invitation USING btree (tenant_id, email_normalized) WHERE (status = 'PENDING'::text);


--
-- Name: uq_tenant_slug; Type: INDEX; Schema: core; Owner: -
--

CREATE UNIQUE INDEX uq_tenant_slug ON core.tenant USING btree (slug) WHERE (slug IS NOT NULL);


--
-- Name: idx_continuation_origin; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_continuation_origin ON delivery.continuation USING btree (origin_interaction_id);


--
-- Name: idx_continuation_run; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_continuation_run ON delivery.continuation USING btree (run_id, status);


--
-- Name: idx_criterion_verdict_criterion; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_criterion_verdict_criterion ON delivery.acceptance_criterion_verdict USING btree (criterion_id);


--
-- Name: idx_req_acceptance_requirement; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_req_acceptance_requirement ON delivery.requirement_acceptance_criterion USING btree (requirement_id, "position", id);


--
-- Name: idx_req_finding_finding; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_req_finding_finding ON delivery.requirement_finding USING btree (finding_id);


--
-- Name: idx_req_finding_req; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_req_finding_req ON delivery.requirement_finding USING btree (requirement_id);


--
-- Name: idx_req_source_input_req; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_req_source_input_req ON delivery.requirement_source_input USING btree (requirement_id);


--
-- Name: idx_requirement_project; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_requirement_project ON delivery.requirement USING btree (project_id, lifecycle_status);


--
-- Name: idx_response_delivery_interaction; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_response_delivery_interaction ON delivery.interaction_response_delivery USING btree (interaction_id);


--
-- Name: idx_response_delivery_unconsumed; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_response_delivery_unconsumed ON delivery.interaction_response_delivery USING btree (project_id, status) WHERE (status = ANY (ARRAY['delivering'::text, 'delivered'::text]));


--
-- Name: idx_review_finding_criterion; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_review_finding_criterion ON delivery.review_finding USING btree (criterion_id) WHERE (criterion_id IS NOT NULL);


--
-- Name: idx_review_finding_run; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_review_finding_run ON delivery.review_finding USING btree (review_run_id);


--
-- Name: idx_run_authorization_active; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_run_authorization_active ON delivery.task_run_execution_authorization USING btree (run_id) WHERE (revoked_at IS NULL);


--
-- Name: idx_task_checkpoint_run; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_checkpoint_run ON delivery.task_checkpoint USING btree (run_id, created_at DESC);


--
-- Name: idx_task_dependency_upstream; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_dependency_upstream ON delivery.task_dependency USING btree (project_id, depends_on_task_id);


--
-- Name: idx_task_interaction_prev_chain; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_interaction_prev_chain ON delivery.task_interaction USING btree (previous_chain_id) WHERE (previous_chain_id IS NOT NULL);


--
-- Name: idx_task_project_status; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_project_status ON delivery.task USING btree (project_id, status);


--
-- Name: idx_task_repository_commit_run; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_repository_commit_run ON delivery.task_repository_commit USING btree (task_run_id);


--
-- Name: idx_task_repository_repo; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_repository_repo ON delivery.task_repository USING btree (repository_id, created_at DESC);


--
-- Name: idx_task_run_agent_heartbeat; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_agent_heartbeat ON delivery.task_run USING btree (last_agent_heartbeat_at) WHERE ((status = ANY (ARRAY['claimed'::text, 'running'::text])) AND (last_agent_heartbeat_at IS NOT NULL));


--
-- Name: idx_task_run_cleanup_due; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_cleanup_due ON delivery.task_run_cleanup USING btree (status, next_retry_at) WHERE (status = ANY (ARRAY['pending'::text, 'cleaning'::text]));


--
-- Name: idx_task_run_cleanup_human; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_cleanup_human ON delivery.task_run_cleanup USING btree (status, created_at) WHERE (status = 'needs_human'::text);


--
-- Name: idx_task_run_client; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_client ON delivery.task_run USING btree (lease_client_id) WHERE (lease_client_id IS NOT NULL);


--
-- Name: idx_task_run_deadline; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_deadline ON delivery.task_run USING btree (deadline_at) WHERE ((status = ANY (ARRAY['claimed'::text, 'running'::text])) AND (deadline_at IS NOT NULL));


--
-- Name: idx_task_run_dispatch; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_dispatch ON delivery.task_run USING btree (next_dispatch_at, created_at) WHERE (status = ANY (ARRAY['queued'::text, 'claimed'::text]));


--
-- Name: idx_task_run_execution_mode; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_execution_mode ON delivery.task_run USING btree (execution_mode) WHERE (execution_mode IS NOT NULL);


--
-- Name: idx_task_run_lease_expiry; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_lease_expiry ON delivery.task_run USING btree (lease_expires_at) WHERE ((status = 'claimed'::text) AND (lease_expires_at IS NOT NULL));


--
-- Name: idx_task_run_shielded; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_shielded ON delivery.task_run USING btree (scheduling_shield) WHERE (scheduling_shield <> 'none'::text);


--
-- Name: idx_task_run_task_status; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_run_task_status ON delivery.task_run USING btree (task_id, status);


--
-- Name: idx_task_supersession_new; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX idx_task_supersession_new ON delivery.task_supersession USING btree (new_task_id);


--
-- Name: interaction_response_task_idx; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX interaction_response_task_idx ON delivery.interaction_response USING btree (task_id, created_at);


--
-- Name: requirement_confirmation_tenant_idx; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX requirement_confirmation_tenant_idx ON delivery.requirement_confirmation USING btree (tenant_id, requirement_id, confirmed_at);


--
-- Name: task_interaction_run_status_idx; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX task_interaction_run_status_idx ON delivery.task_interaction USING btree (run_id, status);


--
-- Name: task_interaction_task_status_idx; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX task_interaction_task_status_idx ON delivery.task_interaction USING btree (task_id, status);


--
-- Name: uq_interaction_response_idem; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_interaction_response_idem ON delivery.interaction_response USING btree (interaction_id, idempotency_key);


--
-- Name: uq_interaction_response_single; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_interaction_response_single ON delivery.interaction_response USING btree (interaction_id);


--
-- Name: uq_response_delivery_one; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_response_delivery_one ON delivery.interaction_response_delivery USING btree (response_id);


--
-- Name: uq_task_interaction_open_chain; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_interaction_open_chain ON delivery.task_interaction USING btree (task_id) WHERE chain_open;


--
-- Name: uq_task_review_active; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_review_active ON delivery.task_run USING btree (task_id) WHERE ((run_kind = 'review'::text) AND (status = ANY (ARRAY['queued'::text, 'running'::text])));


--
-- Name: uq_task_run_active; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_run_active ON delivery.task_run USING btree (task_id) WHERE ((run_kind = 'delivery'::text) AND (status = ANY (ARRAY['queued'::text, 'claimed'::text, 'running'::text])));


--
-- Name: uq_task_run_attempt_delivery; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_run_attempt_delivery ON delivery.task_run USING btree (task_id, attempt_no) WHERE (run_kind = 'delivery'::text);


--
-- Name: uq_task_run_attempt_review; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_run_attempt_review ON delivery.task_run USING btree (task_id, attempt_no) WHERE (run_kind = 'review'::text);


--
-- Name: uq_task_run_source_event; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_run_source_event ON delivery.task_run USING btree (source_event_id) WHERE (source_event_id IS NOT NULL);


--
-- Name: uq_task_waiting_active; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_task_waiting_active ON delivery.task_waiting USING btree (task_id) WHERE (ended_at IS NULL);


--
-- Name: uq_worker_command_active; Type: INDEX; Schema: delivery; Owner: -
--

CREATE UNIQUE INDEX uq_worker_command_active ON delivery.worker_command USING btree (run_id, command_type, interaction_id) WHERE (status <> 'failed'::text);


--
-- Name: worker_command_poll_idx; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX worker_command_poll_idx ON delivery.worker_command USING btree (worker_id, status, claim_expires_at);


--
-- Name: worker_command_run_idx; Type: INDEX; Schema: delivery; Owner: -
--

CREATE INDEX worker_command_run_idx ON delivery.worker_command USING btree (run_id, status);


--
-- Name: idx_binding_device; Type: INDEX; Schema: devenv; Owner: -
--

CREATE INDEX idx_binding_device ON devenv.device_project_binding USING btree (device_id);


--
-- Name: idx_binding_project; Type: INDEX; Schema: devenv; Owner: -
--

CREATE INDEX idx_binding_project ON devenv.device_project_binding USING btree (project_id);


--
-- Name: idx_device_credential_hash; Type: INDEX; Schema: devenv; Owner: -
--

CREATE INDEX idx_device_credential_hash ON devenv.device USING btree (credential_key_hash) WHERE (credential_key_hash IS NOT NULL);


--
-- Name: idx_device_owner; Type: INDEX; Schema: devenv; Owner: -
--

CREATE INDEX idx_device_owner ON devenv.device USING btree (tenant_id, owner_principal_id);


--
-- Name: idx_device_runner_heartbeat; Type: INDEX; Schema: devenv; Owner: -
--

CREATE INDEX idx_device_runner_heartbeat ON devenv.device USING btree (last_heartbeat_at) WHERE is_runner;


--
-- Name: idx_device_tenant; Type: INDEX; Schema: devenv; Owner: -
--

CREATE INDEX idx_device_tenant ON devenv.device USING btree (tenant_id, created_at DESC);


--
-- Name: uq_device_tenant_active_name; Type: INDEX; Schema: devenv; Owner: -
--

CREATE UNIQUE INDEX uq_device_tenant_active_name ON devenv.device USING btree (tenant_id, name) WHERE (status = 'active'::text);


--
-- Name: idx_discovery_evidence_finding; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_evidence_finding ON discovery.discovery_finding_evidence USING btree (tenant_id, project_id, finding_id);


--
-- Name: idx_discovery_finding_project; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_finding_project ON discovery.discovery_finding USING btree (tenant_id, project_id, verification_status, created_at DESC);


--
-- Name: idx_discovery_finding_session; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_finding_session ON discovery.discovery_finding USING btree (tenant_id, session_id, kind);


--
-- Name: idx_discovery_proposal_session; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_proposal_session ON discovery.discovery_proposal USING btree (tenant_id, session_id, status);


--
-- Name: idx_discovery_question_session; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_question_session ON discovery.discovery_question USING btree (tenant_id, session_id, status);


--
-- Name: idx_discovery_session_project; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_session_project ON discovery.discovery_session USING btree (tenant_id, project_id, created_at DESC);


--
-- Name: idx_discovery_session_status; Type: INDEX; Schema: discovery; Owner: -
--

CREATE INDEX idx_discovery_session_status ON discovery.discovery_session USING btree (tenant_id, status);


--
-- Name: idx_audit_claimed_device; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX idx_audit_claimed_device ON governance.audit_log USING btree (claimed_device_id) WHERE (claimed_device_id IS NOT NULL);


--
-- Name: idx_audit_log_project_time; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX idx_audit_log_project_time ON governance.audit_log USING btree (tenant_id, project_id, created_at DESC, id DESC);


--
-- Name: idx_intervention_project_time; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX idx_intervention_project_time ON governance.human_intervention USING btree (project_id, created_at DESC);


--
-- Name: idx_tenant_audit_claimed_device; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX idx_tenant_audit_claimed_device ON governance.tenant_audit_log USING btree (claimed_device_id) WHERE (claimed_device_id IS NOT NULL);


--
-- Name: tenant_audit_log_tenant_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX tenant_audit_log_tenant_idx ON governance.tenant_audit_log USING btree (tenant_id, created_at DESC);


--
-- Name: uq_review_approval_pending; Type: INDEX; Schema: governance; Owner: -
--

CREATE UNIQUE INDEX uq_review_approval_pending ON governance.review_approval USING btree (task_id) WHERE (status = 'pending'::text);


--
-- Name: idx_event_project_time; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_event_project_time ON integration.inbound_event USING btree (project_id, received_at DESC);


--
-- Name: idx_intake_record_project; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_intake_record_project ON integration.intake_record USING btree (tenant_id, project_id, created_at DESC);


--
-- Name: idx_intake_task_intake; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_intake_task_intake ON integration.intake_task USING btree (intake_id, created_at);


--
-- Name: idx_intake_task_status; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_intake_task_status ON integration.intake_task USING btree (task_status, updated_at) WHERE (task_status = ANY (ARRAY['PENDING'::text, 'DISPATCHED'::text, 'PROCESSING'::text]));


--
-- Name: idx_job_claim; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_job_claim ON integration.job USING btree (status, available_at) WHERE (status = 'queued'::text);


--
-- Name: idx_notification_project_time; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_notification_project_time ON integration.notification USING btree (project_id, created_at DESC);


--
-- Name: idx_outbox_claim; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_outbox_claim ON integration.outbox_event USING btree (status, next_attempt_at) WHERE (status = ANY (ARRAY['pending'::text, 'processing'::text]));


--
-- Name: idx_project_repository_project; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_project_repository_project ON integration.project_repository USING btree (project_id, created_at DESC);


--
-- Name: idx_project_repository_status; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_project_repository_status ON integration.project_repository USING btree (project_id, status);


--
-- Name: idx_repo_import_discovery_session; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_repo_import_discovery_session ON integration.repo_import USING btree (tenant_id, discovery_session_id);


--
-- Name: idx_repo_import_project; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_repo_import_project ON integration.repo_import USING btree (project_id, created_at DESC);


--
-- Name: idx_repo_version_confirmation_repo; Type: INDEX; Schema: integration; Owner: -
--

CREATE INDEX idx_repo_version_confirmation_repo ON integration.repository_version_confirmation USING btree (repository_id, confirmed_at DESC);


--
-- Name: repo_import_one_active_per_project; Type: INDEX; Schema: integration; Owner: -
--

CREATE UNIQUE INDEX repo_import_one_active_per_project ON integration.repo_import USING btree (tenant_id, project_id) WHERE (status = ANY (ARRAY['queued'::text, 'cloning'::text, 'reading'::text, 'analyzing'::text]));


--
-- Name: uq_intake_task_active; Type: INDEX; Schema: integration; Owner: -
--

CREATE UNIQUE INDEX uq_intake_task_active ON integration.intake_task USING btree (intake_id, task_type) WHERE (task_status = ANY (ARRAY['PENDING'::text, 'DISPATCHED'::text, 'PROCESSING'::text]));


--
-- Name: idx_kb_document_revision_document; Type: INDEX; Schema: kb; Owner: -
--

CREATE INDEX idx_kb_document_revision_document ON kb.document_revision USING btree (document_id, revision_no DESC);


--
-- Name: idx_kb_memory_project; Type: INDEX; Schema: kb; Owner: -
--

CREATE INDEX idx_kb_memory_project ON kb.memory USING btree (project_id, created_at DESC);


--
-- Name: idx_kb_memory_ref_revision; Type: INDEX; Schema: kb; Owner: -
--

CREATE INDEX idx_kb_memory_ref_revision ON kb.memory_ref USING btree (revision_id);


--
-- Name: idx_kb_memory_ref_subject; Type: INDEX; Schema: kb; Owner: -
--

CREATE INDEX idx_kb_memory_ref_subject ON kb.memory_ref USING btree (project_id, target_domain, target_type, target_id) WHERE (ref_role = 'subject'::text);


--
-- Name: idx_kb_memory_revision_review_queue; Type: INDEX; Schema: kb; Owner: -
--

CREATE INDEX idx_kb_memory_revision_review_queue ON kb.memory_revision USING btree (tenant_id, created_at) WHERE (review_status = 'PENDING_REVIEW'::text);


--
-- Name: idx_kb_source_project_project; Type: INDEX; Schema: kb; Owner: -
--

CREATE INDEX idx_kb_source_project_project ON kb.source_project USING btree (project_id, linked_at DESC);


--
-- Name: uq_kb_memory_revision_current; Type: INDEX; Schema: kb; Owner: -
--

CREATE UNIQUE INDEX uq_kb_memory_revision_current ON kb.memory_revision USING btree (memory_id) WHERE (version_status = 'CURRENT'::text);


--
-- Name: uq_kb_memory_revision_pending; Type: INDEX; Schema: kb; Owner: -
--

CREATE UNIQUE INDEX uq_kb_memory_revision_pending ON kb.memory_revision USING btree (memory_id) WHERE (review_status = 'PENDING_REVIEW'::text);


--
-- Name: uq_kb_memory_revision_version_no; Type: INDEX; Schema: kb; Owner: -
--

CREATE UNIQUE INDEX uq_kb_memory_revision_version_no ON kb.memory_revision USING btree (memory_id, version_no) WHERE (version_no IS NOT NULL);


--
-- Name: idx_pc_approval_proposal; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_approval_proposal ON pc.approval USING btree (tenant_id, project_id, proposal_id, created_at DESC);


--
-- Name: idx_pc_cognition_revision_object; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_cognition_revision_object ON pc.cognition_revision USING btree (object_id, revision_no DESC);


--
-- Name: idx_pc_conflict_object; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_conflict_object ON pc.conflict_object USING btree (tenant_id, project_id, object_id);


--
-- Name: idx_pc_conflict_rel; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_conflict_rel ON pc.conflict_object USING btree (tenant_id, project_id, relationship_id);


--
-- Name: idx_pc_conflict_status; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_conflict_status ON pc.conflict USING btree (tenant_id, project_id, status);


--
-- Name: idx_pc_evidence_subject; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_evidence_subject ON pc.evidence USING btree (tenant_id, project_id, object_revision_id);


--
-- Name: idx_pc_evidence_subject_rel; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_evidence_subject_rel ON pc.evidence USING btree (tenant_id, project_id, relationship_revision_id);


--
-- Name: idx_pc_proposal_status; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_proposal_status ON pc.proposal USING btree (tenant_id, project_id, status, created_at DESC);


--
-- Name: idx_pc_rel_revision_endpoint_src; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_rel_revision_endpoint_src ON pc.relationship_revision USING btree (tenant_id, project_id, source_object_id, source_object_revision_id);


--
-- Name: idx_pc_rel_revision_endpoint_tgt; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_rel_revision_endpoint_tgt ON pc.relationship_revision USING btree (tenant_id, project_id, target_object_id, target_object_revision_id);


--
-- Name: idx_pc_rel_revision_rel; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_rel_revision_rel ON pc.relationship_revision USING btree (tenant_id, project_id, relationship_id, revision_no DESC);


--
-- Name: idx_pc_relationship_source; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_relationship_source ON pc.relationship USING btree (tenant_id, project_id, source_object_id, lifecycle_status);


--
-- Name: idx_pc_relationship_target; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_relationship_target ON pc.relationship USING btree (tenant_id, project_id, target_object_id, lifecycle_status);


--
-- Name: idx_pc_revision_document; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_revision_document ON pc.cognition_revision USING btree (tenant_id, project_id, document_revision_id);


--
-- Name: idx_pc_source_kind; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_source_kind ON pc.source USING btree (tenant_id, project_id, source_kind);


--
-- Name: idx_pc_source_target; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_source_target ON pc.source USING btree (tenant_id, project_id, target_domain, target_type);


--
-- Name: idx_pc_vm_object; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_vm_object ON pc.version_member USING btree (tenant_id, project_id, object_id, object_revision_id);


--
-- Name: idx_pc_vm_rel; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_vm_rel ON pc.version_member USING btree (tenant_id, project_id, relationship_id, relationship_revision_id);


--
-- Name: idx_pc_vm_version; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_vm_version ON pc.version_member USING btree (tenant_id, project_id, version_id);


--
-- Name: idx_pc_write_op_pending; Type: INDEX; Schema: pc; Owner: -
--

CREATE INDEX idx_pc_write_op_pending ON pc.write_operation USING btree (tenant_id, project_id, status) WHERE (status = ANY (ARRAY['pending'::text, 'git_committed'::text, 'failed'::text]));


--
-- Name: uq_pc_approval_pending; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_approval_pending ON pc.approval USING btree (proposal_id) WHERE (status = 'pending'::text);


--
-- Name: uq_pc_conflict_object; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_conflict_object ON pc.conflict_object USING btree (tenant_id, project_id, conflict_id, COALESCE((object_id)::text, ''::text), COALESCE((relationship_id)::text, ''::text));


--
-- Name: uq_pc_evidence_identity; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_evidence_identity ON pc.evidence USING btree (tenant_id, project_id, source_id, COALESCE((object_revision_id)::text, (relationship_revision_id)::text), evidence_type, COALESCE(file_path, ''::text), COALESCE(section_anchor, ''::text), COALESCE(code_range, ''::text), COALESCE(commit_sha, ''::text));


--
-- Name: uq_pc_object_singleton; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_object_singleton ON pc.cognition_object USING btree (tenant_id, project_id, object_type) WHERE ((pc.object_type_singleton(object_type) = true) AND (lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text])));


--
-- Name: uq_pc_pm_target; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_pm_target ON pc.proposal_member USING btree (tenant_id, project_id, proposal_id, COALESCE((object_id)::text, ''::text), COALESCE((relationship_id)::text, ''::text)) WHERE (change_kind = ANY (ARRAY['update_object'::text, 'deprecate_object'::text, 'update_relationship'::text, 'deprecate_relationship'::text]));


--
-- Name: uq_pc_relationship_identity; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_relationship_identity ON pc.relationship USING btree (tenant_id, project_id, relationship_type_key, COALESCE((source_object_id)::text, ((((source_ext_domain || ':'::text) || source_ext_type) || ':'::text) || source_ext_id)), COALESCE((target_object_id)::text, ((((target_ext_domain || ':'::text) || target_ext_type) || ':'::text) || target_ext_id))) WHERE (lifecycle_status = ANY (ARRAY['identity'::text, 'draft'::text, 'active'::text]));


--
-- Name: uq_pc_vm_object; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_vm_object ON pc.version_member USING btree (tenant_id, project_id, version_id, object_id, object_revision_id) WHERE (object_id IS NOT NULL);


--
-- Name: uq_pc_vm_relationship; Type: INDEX; Schema: pc; Owner: -
--

CREATE UNIQUE INDEX uq_pc_vm_relationship ON pc.version_member USING btree (tenant_id, project_id, version_id, relationship_id, relationship_revision_id) WHERE (relationship_id IS NOT NULL);


--
-- Name: idx_source_input_project; Type: INDEX; Schema: source; Owner: -
--

CREATE INDEX idx_source_input_project ON source.source_input USING btree (tenant_id, project_id, created_at DESC);


--
-- Name: task_run_failure task_run_failure_immutable; Type: TRIGGER; Schema: delivery; Owner: -
--

CREATE TRIGGER task_run_failure_immutable BEFORE DELETE OR UPDATE ON delivery.task_run_failure FOR EACH ROW EXECUTE FUNCTION delivery.reject_mutation_of_failure_record();


--
-- Name: acceptance_criterion_verdict trg_criterion_verdict_immutable; Type: TRIGGER; Schema: delivery; Owner: -
--

CREATE TRIGGER trg_criterion_verdict_immutable BEFORE UPDATE ON delivery.acceptance_criterion_verdict FOR EACH ROW EXECUTE FUNCTION delivery.forbid_row_update();


--
-- Name: review_finding trg_review_finding_immutable; Type: TRIGGER; Schema: delivery; Owner: -
--

CREATE TRIGGER trg_review_finding_immutable BEFORE UPDATE ON delivery.review_finding FOR EACH ROW EXECUTE FUNCTION delivery.forbid_row_update();


--
-- Name: task_run trg_review_report_immutable; Type: TRIGGER; Schema: delivery; Owner: -
--

CREATE TRIGGER trg_review_report_immutable BEFORE UPDATE ON delivery.task_run FOR EACH ROW EXECUTE FUNCTION delivery.forbid_review_report_update();


--
-- Name: audit_log audit_log_notify; Type: TRIGGER; Schema: governance; Owner: -
--

CREATE TRIGGER audit_log_notify AFTER INSERT ON governance.audit_log FOR EACH ROW EXECUTE FUNCTION governance.notify_audit_insert();


--
-- Name: review_approval trg_review_approval_immutable; Type: TRIGGER; Schema: governance; Owner: -
--

CREATE TRIGGER trg_review_approval_immutable BEFORE UPDATE ON governance.review_approval FOR EACH ROW EXECUTE FUNCTION delivery.forbid_review_approval_mutation();


--
-- Name: memory_ref trg_kb_memory_ref_immutable; Type: TRIGGER; Schema: kb; Owner: -
--

CREATE TRIGGER trg_kb_memory_ref_immutable BEFORE DELETE OR UPDATE ON kb.memory_ref FOR EACH ROW EXECUTE FUNCTION kb.reject_immutable_mutation();


--
-- Name: memory_revision trg_kb_memory_revision_guard; Type: TRIGGER; Schema: kb; Owner: -
--

CREATE TRIGGER trg_kb_memory_revision_guard BEFORE INSERT OR DELETE OR UPDATE ON kb.memory_revision FOR EACH ROW EXECUTE FUNCTION kb.guard_memory_revision();


--
-- Name: document_revision trg_kb_revision_append_only; Type: TRIGGER; Schema: kb; Owner: -
--

CREATE TRIGGER trg_kb_revision_append_only BEFORE DELETE OR UPDATE ON kb.document_revision FOR EACH ROW EXECUTE FUNCTION kb.reject_revision_mutation();


--
-- Name: source trg_kb_source_immutable; Type: TRIGGER; Schema: kb; Owner: -
--

CREATE TRIGGER trg_kb_source_immutable BEFORE DELETE OR UPDATE ON kb.source FOR EACH ROW EXECUTE FUNCTION kb.reject_immutable_mutation();


--
-- Name: approval trg_pc_approval_immutable; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_approval_immutable BEFORE UPDATE ON pc.approval FOR EACH ROW EXECUTE FUNCTION pc.forbid_approval_mutation();


--
-- Name: document_revision trg_pc_document_revision_immutable; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_document_revision_immutable BEFORE DELETE OR UPDATE ON pc.document_revision FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();


--
-- Name: cognition_object trg_pc_object_delete; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_object_delete BEFORE DELETE ON pc.cognition_object FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete();


--
-- Name: cognition_object trg_pc_object_lifecycle; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_object_lifecycle BEFORE UPDATE ON pc.cognition_object FOR EACH ROW EXECUTE FUNCTION pc.enforce_object_lifecycle();


--
-- Name: proposal_member trg_pc_pm_anchored; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored AFTER DELETE OR UPDATE ON pc.proposal_member DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION pc.enforce_proposal_member_anchored();


--
-- Name: proposal_member trg_pc_pm_anchored_ins; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE CONSTRAINT TRIGGER trg_pc_pm_anchored_ins AFTER INSERT ON pc.proposal_member DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION pc.enforce_proposal_member_anchored();


--
-- Name: proposal trg_pc_proposal_anchors; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_proposal_anchors BEFORE UPDATE ON pc.proposal FOR EACH ROW EXECUTE FUNCTION pc.forbid_proposal_content_mutation();


--
-- Name: relationship_revision trg_pc_rel_revision_endpoint_match; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_rel_revision_endpoint_match BEFORE INSERT ON pc.relationship_revision FOR EACH ROW EXECUTE FUNCTION pc.enforce_rel_revision_endpoint_match();


--
-- Name: relationship_revision trg_pc_rel_revision_immutable; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_rel_revision_immutable BEFORE DELETE OR UPDATE ON pc.relationship_revision FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();


--
-- Name: relationship trg_pc_relationship_anchors; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_relationship_anchors BEFORE UPDATE ON pc.relationship FOR EACH ROW EXECUTE FUNCTION pc.enforce_relationship_anchors();


--
-- Name: relationship trg_pc_relationship_delete; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_relationship_delete BEFORE DELETE ON pc.relationship FOR EACH ROW EXECUTE FUNCTION pc.forbid_identity_delete();


--
-- Name: cognition_revision trg_pc_revision_immutable; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_revision_immutable BEFORE DELETE OR UPDATE ON pc.cognition_revision FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();


--
-- Name: cognition_version trg_pc_version_delete; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_version_delete BEFORE DELETE ON pc.cognition_version FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();


--
-- Name: cognition_version trg_pc_version_immutable; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_version_immutable BEFORE UPDATE ON pc.cognition_version FOR EACH ROW EXECUTE FUNCTION pc.forbid_version_mutation();


--
-- Name: version_member trg_pc_vm_closure; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE CONSTRAINT TRIGGER trg_pc_vm_closure AFTER INSERT ON pc.version_member DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION pc.enforce_member_endpoint_closure();


--
-- Name: version_member trg_pc_vm_immutable; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_vm_immutable BEFORE DELETE OR UPDATE ON pc.version_member FOR EACH ROW EXECUTE FUNCTION pc.forbid_row_update();


--
-- Name: write_operation trg_pc_write_op_state; Type: TRIGGER; Schema: pc; Owner: -
--

CREATE TRIGGER trg_pc_write_op_state BEFORE UPDATE ON pc.write_operation FOR EACH ROW EXECUTE FUNCTION pc.enforce_write_op_state();


--
-- Name: agent_feedback agent_feedback_recipient_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.agent_feedback
    ADD CONSTRAINT agent_feedback_recipient_id_fkey FOREIGN KEY (recipient_id) REFERENCES core.principal(id);


--
-- Name: agent_feedback agent_feedback_related_event_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.agent_feedback
    ADD CONSTRAINT agent_feedback_related_event_id_fkey FOREIGN KEY (related_event_id) REFERENCES integration.inbound_event(id);


--
-- Name: ai_executor_capability ai_executor_capability_principal_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_executor_capability
    ADD CONSTRAINT ai_executor_capability_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: ai_executor_capability ai_executor_capability_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_executor_capability
    ADD CONSTRAINT ai_executor_capability_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: analysis_run analysis_run_project_id_event_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.analysis_run
    ADD CONSTRAINT analysis_run_project_id_event_id_fkey FOREIGN KEY (project_id, event_id) REFERENCES integration.inbound_event(project_id, id);


--
-- Name: baseline_item baseline_item_decided_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_item
    ADD CONSTRAINT baseline_item_decided_by_fkey FOREIGN KEY (decided_by) REFERENCES core.principal(id);


--
-- Name: baseline_item baseline_item_proposal_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_item
    ADD CONSTRAINT baseline_item_proposal_id_fkey FOREIGN KEY (proposal_id) REFERENCES ai.baseline_proposal(id);


--
-- Name: baseline_item baseline_item_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_item
    ADD CONSTRAINT baseline_item_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: baseline_item baseline_item_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_item
    ADD CONSTRAINT baseline_item_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: baseline_proposal baseline_proposal_confirmed_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_confirmed_by_fkey FOREIGN KEY (confirmed_by) REFERENCES core.principal(id);


--
-- Name: baseline_proposal baseline_proposal_created_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: baseline_proposal baseline_proposal_source_run_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_source_run_id_fkey FOREIGN KEY (source_run_id) REFERENCES ai.cognition_run(id);


--
-- Name: baseline_proposal baseline_proposal_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: baseline_proposal baseline_proposal_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.baseline_proposal
    ADD CONSTRAINT baseline_proposal_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: change_proposal change_proposal_project_id_analysis_run_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.change_proposal
    ADD CONSTRAINT change_proposal_project_id_analysis_run_id_fkey FOREIGN KEY (project_id, analysis_run_id) REFERENCES ai.analysis_run(project_id, id);


--
-- Name: cognition_item cognition_item_decided_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_item
    ADD CONSTRAINT cognition_item_decided_by_fkey FOREIGN KEY (decided_by) REFERENCES core.principal(id);


--
-- Name: cognition_item cognition_item_run_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_item
    ADD CONSTRAINT cognition_item_run_id_fkey FOREIGN KEY (run_id) REFERENCES ai.cognition_run(id);


--
-- Name: cognition_item cognition_item_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_item
    ADD CONSTRAINT cognition_item_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: cognition_item cognition_item_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_item
    ADD CONSTRAINT cognition_item_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: cognition_run cognition_run_started_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_run
    ADD CONSTRAINT cognition_run_started_by_fkey FOREIGN KEY (started_by) REFERENCES core.principal(id);


--
-- Name: cognition_run cognition_run_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_run
    ADD CONSTRAINT cognition_run_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: cognition_run cognition_run_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.cognition_run
    ADD CONSTRAINT cognition_run_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: conversation_message conversation_message_conversation_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation_message
    ADD CONSTRAINT conversation_message_conversation_id_fkey FOREIGN KEY (conversation_id) REFERENCES ai.conversation(id) ON DELETE CASCADE;


--
-- Name: conversation conversation_principal_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation
    ADD CONSTRAINT conversation_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: conversation conversation_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation
    ADD CONSTRAINT conversation_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: conversation conversation_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.conversation
    ADD CONSTRAINT conversation_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_proposal document_proposal_created_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT document_proposal_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: document_proposal document_proposal_redline_confirmed_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT document_proposal_redline_confirmed_by_fkey FOREIGN KEY (redline_confirmed_by) REFERENCES core.principal(id);


--
-- Name: document_revision document_revision_created_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT document_revision_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: document_revision document_revision_project_id_document_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT document_revision_project_id_document_id_fkey FOREIGN KEY (project_id, document_id) REFERENCES ai.project_document(project_id, id);


--
-- Name: document_revision document_revision_redline_confirmed_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT document_revision_redline_confirmed_by_fkey FOREIGN KEY (redline_confirmed_by) REFERENCES core.principal(id);


--
-- Name: agent_feedback fk_agent_feedback_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.agent_feedback
    ADD CONSTRAINT fk_agent_feedback_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: ai_role_version fk_ai_role_version_project_role; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.ai_role_version
    ADD CONSTRAINT fk_ai_role_version_project_role FOREIGN KEY (project_role_id) REFERENCES ai.project_ai_role(id) ON DELETE RESTRICT;


--
-- Name: analysis_run fk_analysis_run_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.analysis_run
    ADD CONSTRAINT fk_analysis_run_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: change_proposal fk_change_proposal_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.change_proposal
    ADD CONSTRAINT fk_change_proposal_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_index_state fk_document_index_state_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_index_state
    ADD CONSTRAINT fk_document_index_state_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_index_state fk_document_index_state_tenant_revision; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_index_state
    ADD CONSTRAINT fk_document_index_state_tenant_revision FOREIGN KEY (tenant_id, document_revision_id) REFERENCES ai.document_revision(tenant_id, id) ON DELETE CASCADE;


--
-- Name: document_proposal fk_document_proposal_tenant_base_revision; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT fk_document_proposal_tenant_base_revision FOREIGN KEY (tenant_id, base_revision_id) REFERENCES ai.document_revision(tenant_id, id);


--
-- Name: document_proposal fk_document_proposal_tenant_document; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT fk_document_proposal_tenant_document FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document(tenant_id, id);


--
-- Name: document_proposal fk_document_proposal_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT fk_document_proposal_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_proposal fk_document_proposal_tenant_result_revision; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_proposal
    ADD CONSTRAINT fk_document_proposal_tenant_result_revision FOREIGN KEY (tenant_id, resulting_revision_id) REFERENCES ai.document_revision(tenant_id, id);


--
-- Name: document_revision fk_document_revision_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_revision
    ADD CONSTRAINT fk_document_revision_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_section_index fk_document_section_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_section_index
    ADD CONSTRAINT fk_document_section_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_section_index fk_document_section_tenant_revision; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.document_section_index
    ADD CONSTRAINT fk_document_section_tenant_revision FOREIGN KEY (tenant_id, document_revision_id) REFERENCES ai.document_revision(tenant_id, id) ON DELETE CASCADE;


--
-- Name: knowledge_reference fk_knowledge_reference_tenant_document; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_reference
    ADD CONSTRAINT fk_knowledge_reference_tenant_document FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document(tenant_id, id) ON DELETE CASCADE;


--
-- Name: knowledge_reference fk_knowledge_reference_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_reference
    ADD CONSTRAINT fk_knowledge_reference_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: knowledge_reference fk_knowledge_reference_tenant_revision; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_reference
    ADD CONSTRAINT fk_knowledge_reference_tenant_revision FOREIGN KEY (tenant_id, document_revision_id) REFERENCES ai.document_revision(tenant_id, id) ON DELETE CASCADE;


--
-- Name: knowledge_write_operation fk_knowledge_write_op_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_write_operation
    ADD CONSTRAINT fk_knowledge_write_op_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_ai_config fk_project_ai_config_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_config
    ADD CONSTRAINT fk_project_ai_config_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_ai_role fk_project_ai_role_current_version; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT fk_project_ai_role_current_version FOREIGN KEY (current_version_id) REFERENCES ai.ai_role_version(id);


--
-- Name: project_document fk_project_document_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_document
    ADD CONSTRAINT fk_project_document_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_skill_binding fk_project_skill_binding_tenant_project; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_skill_binding
    ADD CONSTRAINT fk_project_skill_binding_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: knowledge_reference knowledge_reference_created_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.knowledge_reference
    ADD CONSTRAINT knowledge_reference_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: llm_call_record llm_call_record_analysis_run_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.llm_call_record
    ADD CONSTRAINT llm_call_record_analysis_run_id_fkey FOREIGN KEY (analysis_run_id) REFERENCES ai.analysis_run(id);


--
-- Name: llm_call_record llm_call_record_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.llm_call_record
    ADD CONSTRAINT llm_call_record_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: llm_call_record llm_call_record_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.llm_call_record
    ADD CONSTRAINT llm_call_record_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: llm_call_record llm_call_record_tenant_id_provider_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.llm_call_record
    ADD CONSTRAINT llm_call_record_tenant_id_provider_id_fkey FOREIGN KEY (tenant_id, provider_id) REFERENCES ai.team_llm_provider(tenant_id, id);


--
-- Name: project_ai_config project_ai_config_ai_principal_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_config
    ADD CONSTRAINT project_ai_config_ai_principal_id_fkey FOREIGN KEY (ai_principal_id) REFERENCES core.principal(id);


--
-- Name: project_ai_model_policy project_ai_model_policy_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_model_policy
    ADD CONSTRAINT project_ai_model_policy_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: project_ai_model_policy project_ai_model_policy_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_model_policy
    ADD CONSTRAINT project_ai_model_policy_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE CASCADE;


--
-- Name: project_ai_model_policy project_ai_model_policy_tenant_id_provider_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_model_policy
    ADD CONSTRAINT project_ai_model_policy_tenant_id_provider_id_fkey FOREIGN KEY (tenant_id, provider_id) REFERENCES ai.team_llm_provider(tenant_id, id);


--
-- Name: project_ai_model_policy project_ai_model_policy_updated_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_model_policy
    ADD CONSTRAINT project_ai_model_policy_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES core.principal(id);


--
-- Name: project_ai_role project_ai_role_role_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT project_ai_role_role_id_fkey FOREIGN KEY (role_id) REFERENCES ai.ai_role_template(id);


--
-- Name: project_ai_role project_ai_role_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT project_ai_role_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: project_ai_role project_ai_role_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_ai_role
    ADD CONSTRAINT project_ai_role_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_baseline project_baseline_confirmed_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_confirmed_by_fkey FOREIGN KEY (confirmed_by) REFERENCES core.principal(id);


--
-- Name: project_baseline project_baseline_proposal_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_proposal_id_fkey FOREIGN KEY (proposal_id) REFERENCES ai.baseline_proposal(id);


--
-- Name: project_baseline project_baseline_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: project_baseline project_baseline_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_baseline
    ADD CONSTRAINT project_baseline_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_knowledge project_knowledge_confirmed_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_confirmed_by_fkey FOREIGN KEY (confirmed_by) REFERENCES core.principal(id);


--
-- Name: project_knowledge project_knowledge_source_item_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_source_item_id_fkey FOREIGN KEY (source_item_id) REFERENCES ai.cognition_item(id);


--
-- Name: project_knowledge project_knowledge_supersedes_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_supersedes_id_fkey FOREIGN KEY (supersedes_id) REFERENCES ai.project_knowledge(id);


--
-- Name: project_knowledge project_knowledge_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: project_knowledge project_knowledge_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_knowledge
    ADD CONSTRAINT project_knowledge_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_skill_binding project_skill_binding_project_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_skill_binding
    ADD CONSTRAINT project_skill_binding_project_id_fkey FOREIGN KEY (project_id) REFERENCES ai.project_ai_config(project_id);


--
-- Name: project_skill_binding project_skill_binding_skill_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.project_skill_binding
    ADD CONSTRAINT project_skill_binding_skill_id_fkey FOREIGN KEY (skill_id) REFERENCES ai.skill_definition(id);


--
-- Name: team_llm_credential team_llm_credential_created_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_credential
    ADD CONSTRAINT team_llm_credential_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: team_llm_credential team_llm_credential_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_credential
    ADD CONSTRAINT team_llm_credential_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: team_llm_credential team_llm_credential_tenant_id_provider_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_credential
    ADD CONSTRAINT team_llm_credential_tenant_id_provider_id_fkey FOREIGN KEY (tenant_id, provider_id) REFERENCES ai.team_llm_provider(tenant_id, id) ON DELETE CASCADE;


--
-- Name: team_llm_provider team_llm_provider_created_by_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: team_llm_provider team_llm_provider_credential_fk; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_credential_fk FOREIGN KEY (credential_id) REFERENCES ai.team_llm_credential(id) ON DELETE SET NULL;


--
-- Name: team_llm_provider team_llm_provider_tenant_id_fkey; Type: FK CONSTRAINT; Schema: ai; Owner: -
--

ALTER TABLE ONLY ai.team_llm_provider
    ADD CONSTRAINT team_llm_provider_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: approver_grant approver_grant_granted_by_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_granted_by_fkey FOREIGN KEY (granted_by) REFERENCES core.principal(id);


--
-- Name: approver_grant approver_grant_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: approver_grant approver_grant_project_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: approver_grant approver_grant_revoked_by_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_revoked_by_fkey FOREIGN KEY (revoked_by) REFERENCES core.principal(id);


--
-- Name: approver_grant approver_grant_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.approver_grant
    ADD CONSTRAINT approver_grant_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: customer customer_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.customer
    ADD CONSTRAINT customer_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: customer fk_customer_parent; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.customer
    ADD CONSTRAINT fk_customer_parent FOREIGN KEY (tenant_id, parent_id) REFERENCES core.customer(tenant_id, id);


--
-- Name: project fk_project_customer; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project
    ADD CONSTRAINT fk_project_customer FOREIGN KEY (tenant_id, customer_id) REFERENCES core.customer(tenant_id, id);


--
-- Name: project_member fk_project_member_tenant_project; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_member
    ADD CONSTRAINT fk_project_member_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: principal_identity principal_identity_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.principal_identity
    ADD CONSTRAINT principal_identity_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: principal_identity principal_identity_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.principal_identity
    ADD CONSTRAINT principal_identity_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: project_initialization_operation project_initialization_operation_created_by_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_initialization_operation
    ADD CONSTRAINT project_initialization_operation_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: project_initialization_operation project_initialization_operation_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_initialization_operation
    ADD CONSTRAINT project_initialization_operation_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE CASCADE;


--
-- Name: project_member project_member_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project_member
    ADD CONSTRAINT project_member_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: project project_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.project
    ADD CONSTRAINT project_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: tenant_agent tenant_agent_agent_definition_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_agent
    ADD CONSTRAINT tenant_agent_agent_definition_id_fkey FOREIGN KEY (agent_definition_id) REFERENCES ai.agent_definition(id);


--
-- Name: tenant_agent tenant_agent_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_agent
    ADD CONSTRAINT tenant_agent_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: tenant_agent tenant_agent_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_agent
    ADD CONSTRAINT tenant_agent_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id) ON DELETE CASCADE;


--
-- Name: tenant_invitation tenant_invitation_accepted_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation
    ADD CONSTRAINT tenant_invitation_accepted_principal_id_fkey FOREIGN KEY (accepted_principal_id) REFERENCES core.principal(id);


--
-- Name: tenant_invitation tenant_invitation_invited_by_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation
    ADD CONSTRAINT tenant_invitation_invited_by_fkey FOREIGN KEY (invited_by) REFERENCES core.principal(id);


--
-- Name: tenant_invitation_link tenant_invitation_link_created_by_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link
    ADD CONSTRAINT tenant_invitation_link_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: tenant_invitation_link_redemption tenant_invitation_link_redemption_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link_redemption
    ADD CONSTRAINT tenant_invitation_link_redemption_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: tenant_invitation_link_redemption tenant_invitation_link_redemption_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link_redemption
    ADD CONSTRAINT tenant_invitation_link_redemption_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: tenant_invitation_link_redemption tenant_invitation_link_redemption_tenant_id_link_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link_redemption
    ADD CONSTRAINT tenant_invitation_link_redemption_tenant_id_link_id_fkey FOREIGN KEY (tenant_id, link_id) REFERENCES core.tenant_invitation_link(tenant_id, id);


--
-- Name: tenant_invitation_link tenant_invitation_link_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation_link
    ADD CONSTRAINT tenant_invitation_link_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: tenant_invitation tenant_invitation_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_invitation
    ADD CONSTRAINT tenant_invitation_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: tenant_member tenant_member_granted_by_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_member
    ADD CONSTRAINT tenant_member_granted_by_fkey FOREIGN KEY (granted_by) REFERENCES core.principal(id);


--
-- Name: tenant_member tenant_member_principal_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_member
    ADD CONSTRAINT tenant_member_principal_id_fkey FOREIGN KEY (principal_id) REFERENCES core.principal(id);


--
-- Name: tenant_member tenant_member_tenant_id_fkey; Type: FK CONSTRAINT; Schema: core; Owner: -
--

ALTER TABLE ONLY core.tenant_member
    ADD CONSTRAINT tenant_member_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id) ON DELETE CASCADE;


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_decided_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_decided_by_fkey FOREIGN KEY (decided_by) REFERENCES core.principal(id);


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_project_id_criterion_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_project_id_criterion_id_fkey FOREIGN KEY (project_id, criterion_id) REFERENCES delivery.task_acceptance_criterion(project_id, id);


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_project_id_review_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_project_id_review_run_id_fkey FOREIGN KEY (project_id, review_run_id) REFERENCES delivery.task_run(project_id, id) ON DELETE CASCADE;


--
-- Name: acceptance_criterion_verdict acceptance_criterion_verdict_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.acceptance_criterion_verdict
    ADD CONSTRAINT acceptance_criterion_verdict_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: continuation continuation_origin_interaction_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_origin_interaction_id_fkey FOREIGN KEY (origin_interaction_id) REFERENCES delivery.task_interaction(id);


--
-- Name: continuation continuation_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: continuation continuation_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: continuation continuation_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_task_id_fkey FOREIGN KEY (task_id) REFERENCES delivery.task(id);


--
-- Name: continuation continuation_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.continuation
    ADD CONSTRAINT continuation_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: requirement fk_current_requirement_revision; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement
    ADD CONSTRAINT fk_current_requirement_revision FOREIGN KEY (id, current_revision_id) REFERENCES delivery.requirement_revision(requirement_id, id);


--
-- Name: milestone fk_milestone_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.milestone
    ADD CONSTRAINT fk_milestone_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: requirement_revision fk_requirement_revision_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT fk_requirement_revision_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: requirement fk_requirement_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement
    ADD CONSTRAINT fk_requirement_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_acceptance_criterion fk_task_acceptance_criterion_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_acceptance_criterion
    ADD CONSTRAINT fk_task_acceptance_criterion_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_knowledge_ref fk_task_knowledge_ref_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_knowledge_ref
    ADD CONSTRAINT fk_task_knowledge_ref_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_policy_override fk_task_policy_override_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_policy_override
    ADD CONSTRAINT fk_task_policy_override_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_repository fk_task_repository_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository
    ADD CONSTRAINT fk_task_repository_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_repository fk_task_repository_tenant_project_repository; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository
    ADD CONSTRAINT fk_task_repository_tenant_project_repository FOREIGN KEY (tenant_id, project_id, repository_id) REFERENCES integration.project_repository(tenant_id, project_id, id) ON DELETE RESTRICT;


--
-- Name: task_requirement fk_task_requirement_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_requirement
    ADD CONSTRAINT fk_task_requirement_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_run fk_task_run_client_tenant; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT fk_task_run_client_tenant FOREIGN KEY (tenant_id, lease_client_id) REFERENCES devenv.device(tenant_id, id);


--
-- Name: task_run_failure fk_task_run_failure_tenant_run; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_failure
    ADD CONSTRAINT fk_task_run_failure_tenant_run FOREIGN KEY (tenant_id, run_id) REFERENCES delivery.task_run(tenant_id, id);


--
-- Name: task_run_repository fk_task_run_repository_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_repository
    ADD CONSTRAINT fk_task_run_repository_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_run fk_task_run_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT fk_task_run_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task fk_task_superseded_by; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT fk_task_superseded_by FOREIGN KEY (project_id, superseded_by_task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_supersession fk_task_supersession_tenant_approval; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT fk_task_supersession_tenant_approval FOREIGN KEY (tenant_id, approval_request_id) REFERENCES governance.approval_request(tenant_id, id);


--
-- Name: task_supersession fk_task_supersession_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT fk_task_supersession_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task fk_task_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT fk_task_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_waiting fk_task_waiting_tenant_project; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_waiting
    ADD CONSTRAINT fk_task_waiting_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: interaction_response_delivery interaction_response_delivery_interaction_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_interaction_id_fkey FOREIGN KEY (interaction_id) REFERENCES delivery.task_interaction(id);


--
-- Name: interaction_response_delivery interaction_response_delivery_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: interaction_response_delivery interaction_response_delivery_response_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_response_id_fkey FOREIGN KEY (response_id) REFERENCES delivery.interaction_response(id);


--
-- Name: interaction_response_delivery interaction_response_delivery_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: interaction_response_delivery interaction_response_delivery_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response_delivery
    ADD CONSTRAINT interaction_response_delivery_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: interaction_response interaction_response_interaction_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_interaction_id_fkey FOREIGN KEY (interaction_id) REFERENCES delivery.task_interaction(id);


--
-- Name: interaction_response interaction_response_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: interaction_response interaction_response_responded_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_responded_by_fkey FOREIGN KEY (responded_by) REFERENCES core.principal(id);


--
-- Name: interaction_response interaction_response_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: interaction_response interaction_response_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_task_id_fkey FOREIGN KEY (task_id) REFERENCES delivery.task(id);


--
-- Name: interaction_response interaction_response_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.interaction_response
    ADD CONSTRAINT interaction_response_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: policy_scope policy_scope_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.policy_scope
    ADD CONSTRAINT policy_scope_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: policy_scope policy_scope_updated_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.policy_scope
    ADD CONSTRAINT policy_scope_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES core.principal(id);


--
-- Name: requirement_acceptance_criterion requirement_acceptance_criterion_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_acceptance_criterion
    ADD CONSTRAINT requirement_acceptance_criterion_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: requirement_confirmation requirement_confirmation_project_id_requirement_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_confirmation
    ADD CONSTRAINT requirement_confirmation_project_id_requirement_id_fkey FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id);


--
-- Name: requirement_confirmation requirement_confirmation_requirement_id_revision_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_confirmation
    ADD CONSTRAINT requirement_confirmation_requirement_id_revision_id_fkey FOREIGN KEY (requirement_id, revision_id) REFERENCES delivery.requirement_revision(requirement_id, id);


--
-- Name: requirement_finding requirement_finding_project_id_requirement_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_finding
    ADD CONSTRAINT requirement_finding_project_id_requirement_id_fkey FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id);


--
-- Name: requirement_finding requirement_finding_tenant_id_project_id_finding_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_finding
    ADD CONSTRAINT requirement_finding_tenant_id_project_id_finding_id_fkey FOREIGN KEY (tenant_id, project_id, finding_id) REFERENCES discovery.discovery_finding(tenant_id, project_id, id);


--
-- Name: requirement_finding requirement_finding_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_finding
    ADD CONSTRAINT requirement_finding_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: requirement_revision requirement_revision_approved_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_approved_by_fkey FOREIGN KEY (approved_by) REFERENCES core.principal(id);


--
-- Name: requirement_revision requirement_revision_created_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: requirement_revision requirement_revision_project_id_requirement_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_revision
    ADD CONSTRAINT requirement_revision_project_id_requirement_id_fkey FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id);


--
-- Name: requirement_source_input requirement_source_input_project_id_input_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_source_input
    ADD CONSTRAINT requirement_source_input_project_id_input_id_fkey FOREIGN KEY (project_id, input_id) REFERENCES source.source_input(project_id, id);


--
-- Name: requirement_source_input requirement_source_input_project_id_requirement_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_source_input
    ADD CONSTRAINT requirement_source_input_project_id_requirement_id_fkey FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id);


--
-- Name: requirement_source_input requirement_source_input_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.requirement_source_input
    ADD CONSTRAINT requirement_source_input_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: review_finding review_finding_project_id_criterion_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.review_finding
    ADD CONSTRAINT review_finding_project_id_criterion_id_fkey FOREIGN KEY (project_id, criterion_id) REFERENCES delivery.task_acceptance_criterion(project_id, id);


--
-- Name: review_finding review_finding_project_id_review_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.review_finding
    ADD CONSTRAINT review_finding_project_id_review_run_id_fkey FOREIGN KEY (project_id, review_run_id) REFERENCES delivery.task_run(project_id, id) ON DELETE CASCADE;


--
-- Name: review_finding review_finding_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.review_finding
    ADD CONSTRAINT review_finding_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_acceptance_criterion task_acceptance_criterion_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_acceptance_criterion
    ADD CONSTRAINT task_acceptance_criterion_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE;


--
-- Name: task task_ai_role_version_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT task_ai_role_version_id_fkey FOREIGN KEY (ai_role_version_id) REFERENCES ai.ai_role_version(id);


--
-- Name: task task_assignee_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT task_assignee_id_fkey FOREIGN KEY (assignee_id) REFERENCES core.principal(id);


--
-- Name: task_checkpoint task_checkpoint_interaction_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_interaction_id_fkey FOREIGN KEY (interaction_id) REFERENCES delivery.task_interaction(id);


--
-- Name: task_checkpoint task_checkpoint_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: task_checkpoint task_checkpoint_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: task_checkpoint task_checkpoint_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_task_id_fkey FOREIGN KEY (task_id) REFERENCES delivery.task(id);


--
-- Name: task_checkpoint task_checkpoint_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_checkpoint
    ADD CONSTRAINT task_checkpoint_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: task_dependency task_dependency_created_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: task_dependency task_dependency_project_id_depends_on_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_project_id_depends_on_task_id_fkey FOREIGN KEY (project_id, depends_on_task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE;


--
-- Name: task_dependency task_dependency_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE;


--
-- Name: task_dependency task_dependency_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_dependency
    ADD CONSTRAINT task_dependency_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_interaction task_interaction_previous_chain_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_previous_chain_id_fkey FOREIGN KEY (previous_chain_id) REFERENCES delivery.task_interaction(id);


--
-- Name: task_interaction task_interaction_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: task_interaction task_interaction_retry_of_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_retry_of_fkey FOREIGN KEY (retry_of) REFERENCES delivery.task_interaction(id);


--
-- Name: task_interaction task_interaction_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: task_interaction task_interaction_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_task_id_fkey FOREIGN KEY (task_id) REFERENCES delivery.task(id);


--
-- Name: task_interaction task_interaction_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_interaction
    ADD CONSTRAINT task_interaction_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: task_knowledge_ref task_knowledge_ref_project_id_document_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_knowledge_ref
    ADD CONSTRAINT task_knowledge_ref_project_id_document_id_fkey FOREIGN KEY (project_id, document_id) REFERENCES ai.project_document(project_id, id);


--
-- Name: task_knowledge_ref task_knowledge_ref_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_knowledge_ref
    ADD CONSTRAINT task_knowledge_ref_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE;


--
-- Name: task_policy_override task_policy_override_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_policy_override
    ADD CONSTRAINT task_policy_override_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_policy_override task_policy_override_updated_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_policy_override
    ADD CONSTRAINT task_policy_override_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES core.principal(id);


--
-- Name: task task_project_id_milestone_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task
    ADD CONSTRAINT task_project_id_milestone_id_fkey FOREIGN KEY (project_id, milestone_id) REFERENCES delivery.milestone(project_id, id);


--
-- Name: task_repository_commit task_repository_commit_project_id_repository_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository_commit
    ADD CONSTRAINT task_repository_commit_project_id_repository_id_fkey FOREIGN KEY (project_id, repository_id) REFERENCES integration.project_repository(project_id, id);


--
-- Name: task_repository_commit task_repository_commit_project_id_task_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository_commit
    ADD CONSTRAINT task_repository_commit_project_id_task_run_id_fkey FOREIGN KEY (project_id, task_run_id) REFERENCES delivery.task_run(project_id, id) ON DELETE RESTRICT;


--
-- Name: task_repository_commit task_repository_commit_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository_commit
    ADD CONSTRAINT task_repository_commit_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: task_repository task_repository_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_repository
    ADD CONSTRAINT task_repository_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id) ON DELETE CASCADE;


--
-- Name: task_requirement task_requirement_project_id_requirement_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_requirement
    ADD CONSTRAINT task_requirement_project_id_requirement_id_fkey FOREIGN KEY (project_id, requirement_id) REFERENCES delivery.requirement(project_id, id);


--
-- Name: task_requirement task_requirement_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_requirement
    ADD CONSTRAINT task_requirement_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_requirement task_requirement_requirement_id_revision_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_requirement
    ADD CONSTRAINT task_requirement_requirement_id_revision_id_fkey FOREIGN KEY (requirement_id, revision_id) REFERENCES delivery.requirement_revision(requirement_id, id);


--
-- Name: task_run task_run_agent_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT task_run_agent_id_fkey FOREIGN KEY (agent_id) REFERENCES core.principal(id);


--
-- Name: task_run_cleanup task_run_cleanup_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_cleanup
    ADD CONSTRAINT task_run_cleanup_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id) ON DELETE CASCADE;


--
-- Name: task_run_execution_authorization task_run_execution_authorization_authorized_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_authorized_by_fkey FOREIGN KEY (authorized_by) REFERENCES core.principal(id);


--
-- Name: task_run_execution_authorization task_run_execution_authorization_interaction_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_interaction_id_fkey FOREIGN KEY (interaction_id) REFERENCES delivery.task_interaction(id);


--
-- Name: task_run_execution_authorization task_run_execution_authorization_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: task_run_execution_authorization task_run_execution_authorization_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: task_run_execution_authorization task_run_execution_authorization_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_execution_authorization
    ADD CONSTRAINT task_run_execution_authorization_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: task_run task_run_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT task_run_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_run_repository task_run_repository_project_id_repository_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_repository
    ADD CONSTRAINT task_run_repository_project_id_repository_id_fkey FOREIGN KEY (project_id, repository_id) REFERENCES integration.project_repository(project_id, id);


--
-- Name: task_run_repository task_run_repository_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run_repository
    ADD CONSTRAINT task_run_repository_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id) ON DELETE CASCADE;


--
-- Name: task_run task_run_supersedes_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT task_run_supersedes_run_id_fkey FOREIGN KEY (supersedes_run_id) REFERENCES delivery.task_run(id);


--
-- Name: task_run task_run_triggered_by_user_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_run
    ADD CONSTRAINT task_run_triggered_by_user_id_fkey FOREIGN KEY (triggered_by_user_id) REFERENCES core.principal(id);


--
-- Name: task_supersession task_supersession_created_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT task_supersession_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: task_supersession task_supersession_project_id_new_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT task_supersession_project_id_new_task_id_fkey FOREIGN KEY (project_id, new_task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_supersession task_supersession_project_id_old_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_supersession
    ADD CONSTRAINT task_supersession_project_id_old_task_id_fkey FOREIGN KEY (project_id, old_task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_waiting task_waiting_ended_by_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_waiting
    ADD CONSTRAINT task_waiting_ended_by_fkey FOREIGN KEY (ended_by) REFERENCES core.principal(id);


--
-- Name: task_waiting task_waiting_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_waiting
    ADD CONSTRAINT task_waiting_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: task_waiting task_waiting_requested_from_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.task_waiting
    ADD CONSTRAINT task_waiting_requested_from_fkey FOREIGN KEY (requested_from) REFERENCES core.principal(id);


--
-- Name: worker_command worker_command_interaction_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_interaction_id_fkey FOREIGN KEY (interaction_id) REFERENCES delivery.task_interaction(id);


--
-- Name: worker_command worker_command_project_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_project_id_fkey FOREIGN KEY (project_id) REFERENCES core.project(id);


--
-- Name: worker_command worker_command_run_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_run_id_fkey FOREIGN KEY (run_id) REFERENCES delivery.task_run(id);


--
-- Name: worker_command worker_command_task_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_task_id_fkey FOREIGN KEY (task_id) REFERENCES delivery.task(id);


--
-- Name: worker_command worker_command_tenant_id_fkey; Type: FK CONSTRAINT; Schema: delivery; Owner: -
--

ALTER TABLE ONLY delivery.worker_command
    ADD CONSTRAINT worker_command_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: device_project_binding device_project_binding_tenant_id_device_id_fkey; Type: FK CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device_project_binding
    ADD CONSTRAINT device_project_binding_tenant_id_device_id_fkey FOREIGN KEY (tenant_id, device_id) REFERENCES devenv.device(tenant_id, id);


--
-- Name: device_project_binding device_project_binding_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device_project_binding
    ADD CONSTRAINT device_project_binding_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: device device_tenant_id_fkey; Type: FK CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device
    ADD CONSTRAINT device_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: device_project_binding fk_binding_tenant_bounder; Type: FK CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device_project_binding
    ADD CONSTRAINT fk_binding_tenant_bounder FOREIGN KEY (tenant_id, bound_by) REFERENCES core.tenant_member(tenant_id, principal_id);


--
-- Name: device fk_device_tenant_owner; Type: FK CONSTRAINT; Schema: devenv; Owner: -
--

ALTER TABLE ONLY devenv.device
    ADD CONSTRAINT fk_device_tenant_owner FOREIGN KEY (tenant_id, owner_principal_id) REFERENCES core.tenant_member(tenant_id, principal_id);


--
-- Name: discovery_answer discovery_answer_answered_by_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_answer
    ADD CONSTRAINT discovery_answer_answered_by_fkey FOREIGN KEY (answered_by) REFERENCES core.principal(id);


--
-- Name: discovery_answer discovery_answer_tenant_id_question_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_answer
    ADD CONSTRAINT discovery_answer_tenant_id_question_id_fkey FOREIGN KEY (tenant_id, question_id) REFERENCES discovery.discovery_question(tenant_id, id) ON DELETE CASCADE;


--
-- Name: discovery_finding_evidence discovery_finding_evidence_created_by_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_evidence
    ADD CONSTRAINT discovery_finding_evidence_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: discovery_finding_evidence discovery_finding_evidence_tenant_id_project_id_finding_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_evidence
    ADD CONSTRAINT discovery_finding_evidence_tenant_id_project_id_finding_id_fkey FOREIGN KEY (tenant_id, project_id, finding_id) REFERENCES discovery.discovery_finding(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: discovery_finding_evidence discovery_finding_evidence_tenant_id_project_id_import_id__fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_evidence
    ADD CONSTRAINT discovery_finding_evidence_tenant_id_project_id_import_id__fkey FOREIGN KEY (tenant_id, project_id, import_id, repository_id) REFERENCES integration.repo_import_source(tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE;


--
-- Name: discovery_finding_evidence discovery_finding_evidence_tenant_id_project_id_import_id_fkey1; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_evidence
    ADD CONSTRAINT discovery_finding_evidence_tenant_id_project_id_import_id_fkey1 FOREIGN KEY (tenant_id, project_id, import_id, repository_id, attempt_no) REFERENCES integration.repo_import_source_attempt(tenant_id, project_id, import_id, repository_id, attempt_no) ON DELETE CASCADE;


--
-- Name: discovery_message discovery_message_tenant_id_session_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_message
    ADD CONSTRAINT discovery_message_tenant_id_session_id_fkey FOREIGN KEY (tenant_id, session_id) REFERENCES discovery.discovery_session(tenant_id, id) ON DELETE CASCADE;


--
-- Name: discovery_proposal discovery_proposal_tenant_id_base_revision_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_proposal
    ADD CONSTRAINT discovery_proposal_tenant_id_base_revision_id_fkey FOREIGN KEY (tenant_id, base_revision_id) REFERENCES ai.document_revision(tenant_id, id);


--
-- Name: discovery_proposal discovery_proposal_tenant_id_document_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_proposal
    ADD CONSTRAINT discovery_proposal_tenant_id_document_id_fkey FOREIGN KEY (tenant_id, document_id) REFERENCES ai.project_document(tenant_id, id);


--
-- Name: discovery_proposal discovery_proposal_tenant_id_document_proposal_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_proposal
    ADD CONSTRAINT discovery_proposal_tenant_id_document_proposal_id_fkey FOREIGN KEY (tenant_id, document_proposal_id) REFERENCES ai.document_proposal(tenant_id, id);


--
-- Name: discovery_session discovery_session_created_by_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_session
    ADD CONSTRAINT discovery_session_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: discovery_session discovery_session_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_session
    ADD CONSTRAINT discovery_session_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE CASCADE;


--
-- Name: discovery_finding fk_discovery_finding_tenant_project_session; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding
    ADD CONSTRAINT fk_discovery_finding_tenant_project_session FOREIGN KEY (tenant_id, project_id, session_id) REFERENCES discovery.discovery_session(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: discovery_proposal fk_discovery_proposal_tenant_project_session; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_proposal
    ADD CONSTRAINT fk_discovery_proposal_tenant_project_session FOREIGN KEY (tenant_id, project_id, session_id) REFERENCES discovery.discovery_session(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: discovery_question fk_discovery_question_tenant_project_finding; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_question
    ADD CONSTRAINT fk_discovery_question_tenant_project_finding FOREIGN KEY (tenant_id, project_id, finding_id) REFERENCES discovery.discovery_finding(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: discovery_question fk_discovery_question_tenant_project_session; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_question
    ADD CONSTRAINT fk_discovery_question_tenant_project_session FOREIGN KEY (tenant_id, project_id, session_id) REFERENCES discovery.discovery_session(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: discovery_finding_repository fk_finding_repository_tenant_project_finding; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_repository
    ADD CONSTRAINT fk_finding_repository_tenant_project_finding FOREIGN KEY (tenant_id, project_id, finding_id) REFERENCES discovery.discovery_finding(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: discovery_finding_repository fk_finding_repository_tenant_project_repository; Type: FK CONSTRAINT; Schema: discovery; Owner: -
--

ALTER TABLE ONLY discovery.discovery_finding_repository
    ADD CONSTRAINT fk_finding_repository_tenant_project_repository FOREIGN KEY (tenant_id, project_id, repository_id) REFERENCES integration.project_repository(tenant_id, project_id, id) ON DELETE RESTRICT;


--
-- Name: approval_request approval_request_decided_by_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.approval_request
    ADD CONSTRAINT approval_request_decided_by_fkey FOREIGN KEY (decided_by) REFERENCES core.principal(id);


--
-- Name: audit_log audit_log_actor_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.audit_log
    ADD CONSTRAINT audit_log_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES core.principal(id);


--
-- Name: audit_log audit_log_actor_identity_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.audit_log
    ADD CONSTRAINT audit_log_actor_identity_id_fkey FOREIGN KEY (actor_identity_id) REFERENCES core.principal_identity(id);


--
-- Name: audit_log audit_log_claimed_device_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.audit_log
    ADD CONSTRAINT audit_log_claimed_device_id_fkey FOREIGN KEY (claimed_device_id) REFERENCES devenv.device(id);


--
-- Name: approval_request fk_approval_request_tenant_proposal; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.approval_request
    ADD CONSTRAINT fk_approval_request_tenant_proposal FOREIGN KEY (tenant_id, proposal_id) REFERENCES ai.change_proposal(tenant_id, id);


--
-- Name: audit_log fk_audit_log_tenant_project; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.audit_log
    ADD CONSTRAINT fk_audit_log_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: human_intervention fk_human_intervention_tenant_project; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.human_intervention
    ADD CONSTRAINT fk_human_intervention_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: human_intervention fk_human_intervention_tenant_proposal; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.human_intervention
    ADD CONSTRAINT fk_human_intervention_tenant_proposal FOREIGN KEY (tenant_id, target_proposal_id) REFERENCES ai.change_proposal(tenant_id, id);


--
-- Name: human_intervention human_intervention_actor_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.human_intervention
    ADD CONSTRAINT human_intervention_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES core.principal(id);


--
-- Name: review_approval review_approval_decided_by_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_decided_by_fkey FOREIGN KEY (decided_by) REFERENCES core.principal(id);


--
-- Name: review_approval review_approval_project_id_delivery_run_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_project_id_delivery_run_id_fkey FOREIGN KEY (project_id, delivery_run_id) REFERENCES delivery.task_run(project_id, id);


--
-- Name: review_approval review_approval_project_id_review_run_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_project_id_review_run_id_fkey FOREIGN KEY (project_id, review_run_id) REFERENCES delivery.task_run(project_id, id) ON DELETE CASCADE;


--
-- Name: review_approval review_approval_project_id_task_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_project_id_task_id_fkey FOREIGN KEY (project_id, task_id) REFERENCES delivery.task(project_id, id);


--
-- Name: review_approval review_approval_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.review_approval
    ADD CONSTRAINT review_approval_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: tenant_audit_log tenant_audit_log_actor_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.tenant_audit_log
    ADD CONSTRAINT tenant_audit_log_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES core.principal(id);


--
-- Name: tenant_audit_log tenant_audit_log_actor_identity_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.tenant_audit_log
    ADD CONSTRAINT tenant_audit_log_actor_identity_id_fkey FOREIGN KEY (actor_identity_id) REFERENCES core.principal_identity(id);


--
-- Name: tenant_audit_log tenant_audit_log_claimed_device_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.tenant_audit_log
    ADD CONSTRAINT tenant_audit_log_claimed_device_id_fkey FOREIGN KEY (claimed_device_id) REFERENCES devenv.device(id);


--
-- Name: tenant_audit_log tenant_audit_log_target_principal_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.tenant_audit_log
    ADD CONSTRAINT tenant_audit_log_target_principal_id_fkey FOREIGN KEY (target_principal_id) REFERENCES core.principal(id);


--
-- Name: tenant_audit_log tenant_audit_log_tenant_id_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.tenant_audit_log
    ADD CONSTRAINT tenant_audit_log_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: inbound_event fk_inbound_event_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.inbound_event
    ADD CONSTRAINT fk_inbound_event_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: job fk_job_tenant_event; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.job
    ADD CONSTRAINT fk_job_tenant_event FOREIGN KEY (tenant_id, event_id) REFERENCES integration.inbound_event(tenant_id, id);


--
-- Name: job fk_job_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.job
    ADD CONSTRAINT fk_job_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: notification fk_notification_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.notification
    ADD CONSTRAINT fk_notification_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: outbox_event fk_outbox_event_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.outbox_event
    ADD CONSTRAINT fk_outbox_event_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_repository fk_project_repository_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_repository
    ADD CONSTRAINT fk_project_repository_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE CASCADE;


--
-- Name: project_ssh_key fk_project_ssh_key_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_ssh_key
    ADD CONSTRAINT fk_project_ssh_key_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE CASCADE;


--
-- Name: repo_import fk_repo_import_tenant_analysis; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT fk_repo_import_tenant_analysis FOREIGN KEY (tenant_id, analysis_run_id) REFERENCES ai.analysis_run(tenant_id, id);


--
-- Name: repo_import fk_repo_import_tenant_project; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT fk_repo_import_tenant_project FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id) ON DELETE CASCADE;


--
-- Name: repo_import fk_repo_import_tenant_project_discovery_session; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT fk_repo_import_tenant_project_discovery_session FOREIGN KEY (tenant_id, project_id, discovery_session_id) REFERENCES discovery.discovery_session(tenant_id, project_id, id);


--
-- Name: inbound_event inbound_event_actor_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.inbound_event
    ADD CONSTRAINT inbound_event_actor_id_fkey FOREIGN KEY (actor_id) REFERENCES core.principal(id);


--
-- Name: intake_record intake_record_submitted_by_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_record
    ADD CONSTRAINT intake_record_submitted_by_fkey FOREIGN KEY (submitted_by) REFERENCES core.principal(id);


--
-- Name: intake_record intake_record_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_record
    ADD CONSTRAINT intake_record_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: intake_task intake_task_tenant_id_intake_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_task
    ADD CONSTRAINT intake_task_tenant_id_intake_id_fkey FOREIGN KEY (tenant_id, intake_id) REFERENCES integration.intake_record(tenant_id, id);


--
-- Name: intake_task intake_task_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.intake_task
    ADD CONSTRAINT intake_task_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: project_ssh_key project_ssh_key_created_by_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.project_ssh_key
    ADD CONSTRAINT project_ssh_key_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: repo_import_draft_version repo_import_draft_version_tenant_id_import_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_draft_version
    ADD CONSTRAINT repo_import_draft_version_tenant_id_import_id_fkey FOREIGN KEY (tenant_id, import_id) REFERENCES integration.repo_import(tenant_id, id) ON DELETE CASCADE;


--
-- Name: repo_import repo_import_requested_by_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import
    ADD CONSTRAINT repo_import_requested_by_fkey FOREIGN KEY (requested_by) REFERENCES core.principal(id);


--
-- Name: repo_import_source_attempt repo_import_source_attempt_tenant_id_project_id_import_id__fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source_attempt
    ADD CONSTRAINT repo_import_source_attempt_tenant_id_project_id_import_id__fkey FOREIGN KEY (tenant_id, project_id, import_id, repository_id) REFERENCES integration.repo_import_source(tenant_id, project_id, import_id, repository_id) ON DELETE CASCADE;


--
-- Name: repo_import_source repo_import_source_tenant_id_project_id_import_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source
    ADD CONSTRAINT repo_import_source_tenant_id_project_id_import_id_fkey FOREIGN KEY (tenant_id, project_id, import_id) REFERENCES integration.repo_import(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: repo_import_source repo_import_source_tenant_id_project_id_repository_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repo_import_source
    ADD CONSTRAINT repo_import_source_tenant_id_project_id_repository_id_fkey FOREIGN KEY (tenant_id, project_id, repository_id) REFERENCES integration.project_repository(tenant_id, project_id, id) ON DELETE RESTRICT;


--
-- Name: repository_version_confirmation repository_version_confirmation_repository_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repository_version_confirmation
    ADD CONSTRAINT repository_version_confirmation_repository_id_fkey FOREIGN KEY (repository_id) REFERENCES integration.project_repository(id) ON DELETE CASCADE;


--
-- Name: repository_version_confirmation repository_version_confirmation_tenant_id_fkey; Type: FK CONSTRAINT; Schema: integration; Owner: -
--

ALTER TABLE ONLY integration.repository_version_confirmation
    ADD CONSTRAINT repository_version_confirmation_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: document_revision document_revision_document_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document_revision
    ADD CONSTRAINT document_revision_document_id_fkey FOREIGN KEY (document_id) REFERENCES kb.document(id);


--
-- Name: document document_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.document
    ADD CONSTRAINT document_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: memory memory_created_by_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory
    ADD CONSTRAINT memory_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: memory_ref memory_ref_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_ref
    ADD CONSTRAINT memory_ref_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: memory_ref memory_ref_tenant_id_revision_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_ref
    ADD CONSTRAINT memory_ref_tenant_id_revision_id_fkey FOREIGN KEY (tenant_id, revision_id) REFERENCES kb.memory_revision(tenant_id, id);


--
-- Name: memory_revision memory_revision_proposed_by_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_proposed_by_fkey FOREIGN KEY (proposed_by) REFERENCES core.principal(id);


--
-- Name: memory_revision memory_revision_reviewed_by_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_reviewed_by_fkey FOREIGN KEY (reviewed_by) REFERENCES core.principal(id);


--
-- Name: memory_revision memory_revision_tenant_id_base_revision_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_tenant_id_base_revision_id_fkey FOREIGN KEY (tenant_id, base_revision_id) REFERENCES kb.memory_revision(tenant_id, id);


--
-- Name: memory_revision memory_revision_tenant_id_memory_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory_revision
    ADD CONSTRAINT memory_revision_tenant_id_memory_id_fkey FOREIGN KEY (tenant_id, memory_id) REFERENCES kb.memory(tenant_id, id);


--
-- Name: memory memory_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.memory
    ADD CONSTRAINT memory_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: source_project source_project_linked_by_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source_project
    ADD CONSTRAINT source_project_linked_by_fkey FOREIGN KEY (linked_by) REFERENCES core.principal(id);


--
-- Name: source_project source_project_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source_project
    ADD CONSTRAINT source_project_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: source_project source_project_tenant_id_source_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source_project
    ADD CONSTRAINT source_project_tenant_id_source_id_fkey FOREIGN KEY (tenant_id, source_id) REFERENCES kb.source(tenant_id, id);


--
-- Name: source source_registered_by_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source
    ADD CONSTRAINT source_registered_by_fkey FOREIGN KEY (registered_by) REFERENCES core.principal(id);


--
-- Name: source source_tenant_id_fkey; Type: FK CONSTRAINT; Schema: kb; Owner: -
--

ALTER TABLE ONLY kb.source
    ADD CONSTRAINT source_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: approval approval_decided_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_decided_by_fkey FOREIGN KEY (decided_by) REFERENCES core.principal(id);


--
-- Name: approval approval_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: approval approval_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: approval approval_tenant_id_project_id_proposal_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.approval
    ADD CONSTRAINT approval_tenant_id_project_id_proposal_id_fkey FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES pc.proposal(tenant_id, project_id, id);


--
-- Name: cognition_object cognition_object_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: cognition_object cognition_object_current_revision_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_current_revision_fkey FOREIGN KEY (tenant_id, project_id, id, current_revision_id) REFERENCES pc.cognition_revision(tenant_id, project_id, object_id, id) DEFERRABLE;


--
-- Name: cognition_object cognition_object_object_type_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_object_type_fkey FOREIGN KEY (object_type) REFERENCES pc.object_type(type_key);


--
-- Name: cognition_object cognition_object_superseded_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_superseded_by_fkey FOREIGN KEY (tenant_id, project_id, superseded_by_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: cognition_object cognition_object_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: cognition_object cognition_object_updated_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_object
    ADD CONSTRAINT cognition_object_updated_by_fkey FOREIGN KEY (updated_by) REFERENCES core.principal(id);


--
-- Name: cognition_revision cognition_revision_document_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_document_fkey FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES pc.document(tenant_id, project_id, id);


--
-- Name: cognition_revision cognition_revision_document_revision_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_document_revision_fkey FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id) REFERENCES pc.document_revision(tenant_id, project_id, document_id, id);


--
-- Name: cognition_revision cognition_revision_object_composite_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_object_composite_fkey FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: cognition_revision cognition_revision_project_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_revision
    ADD CONSTRAINT cognition_revision_project_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: cognition_version cognition_version_confirmed_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_confirmed_by_fkey FOREIGN KEY (confirmed_by) REFERENCES core.principal(id);


--
-- Name: cognition_version cognition_version_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: cognition_version cognition_version_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: cognition_version cognition_version_tenant_id_project_id_source_proposal_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.cognition_version
    ADD CONSTRAINT cognition_version_tenant_id_project_id_source_proposal_id_fkey FOREIGN KEY (tenant_id, project_id, source_proposal_id) REFERENCES pc.proposal(tenant_id, project_id, id);


--
-- Name: conflict conflict_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: conflict_object conflict_object_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: conflict_object conflict_object_tenant_id_project_id_conflict_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_project_id_conflict_id_fkey FOREIGN KEY (tenant_id, project_id, conflict_id) REFERENCES pc.conflict(tenant_id, project_id, id) ON DELETE CASCADE;


--
-- Name: conflict_object conflict_object_tenant_id_project_id_object_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_project_id_object_id_fkey FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: conflict_object conflict_object_tenant_id_project_id_relationship_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict_object
    ADD CONSTRAINT conflict_object_tenant_id_project_id_relationship_id_fkey FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES pc.relationship(tenant_id, project_id, id);


--
-- Name: conflict conflict_resolved_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_resolved_by_fkey FOREIGN KEY (resolved_by) REFERENCES core.principal(id);


--
-- Name: conflict conflict_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: conflict conflict_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.conflict
    ADD CONSTRAINT conflict_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document_revision document_revision_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: document_revision document_revision_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: document_revision document_revision_tenant_id_project_id_document_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_document_id_fkey FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES pc.document(tenant_id, project_id, id);


--
-- Name: document_revision document_revision_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document_revision
    ADD CONSTRAINT document_revision_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: document document_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document
    ADD CONSTRAINT document_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: document document_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.document
    ADD CONSTRAINT document_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: evidence evidence_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: evidence evidence_object_revision_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_object_revision_fkey FOREIGN KEY (tenant_id, project_id, object_revision_id) REFERENCES pc.cognition_revision(tenant_id, project_id, id);


--
-- Name: evidence evidence_relationship_revision_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_relationship_revision_fkey FOREIGN KEY (tenant_id, project_id, relationship_revision_id) REFERENCES pc.relationship_revision(tenant_id, project_id, id);


--
-- Name: evidence evidence_source_composite_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_source_composite_fkey FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES pc.source(tenant_id, project_id, id);


--
-- Name: evidence evidence_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.evidence
    ADD CONSTRAINT evidence_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: proposal proposal_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: proposal_member proposal_member_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: proposal_member proposal_member_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: proposal_member proposal_member_tenant_id_project_id_object_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_object_id_fkey FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: proposal_member proposal_member_tenant_id_project_id_object_id_object_revi_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_object_id_object_revi_fkey FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id) REFERENCES pc.cognition_revision(tenant_id, project_id, object_id, id);


--
-- Name: proposal_member proposal_member_tenant_id_project_id_proposal_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_proposal_id_fkey FOREIGN KEY (tenant_id, project_id, proposal_id) REFERENCES pc.proposal(tenant_id, project_id, id);


--
-- Name: proposal_member proposal_member_tenant_id_project_id_relationship_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_relationship_id_fkey FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES pc.relationship(tenant_id, project_id, id);


--
-- Name: proposal_member proposal_member_tenant_id_project_id_relationship_id_relat_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal_member
    ADD CONSTRAINT proposal_member_tenant_id_project_id_relationship_id_relat_fkey FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id) REFERENCES pc.relationship_revision(tenant_id, project_id, relationship_id, id);


--
-- Name: proposal proposal_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: proposal proposal_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: proposal proposal_tenant_id_project_id_source_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.proposal
    ADD CONSTRAINT proposal_tenant_id_project_id_source_id_fkey FOREIGN KEY (tenant_id, project_id, source_id) REFERENCES pc.source(tenant_id, project_id, id);


--
-- Name: relationship relationship_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: relationship relationship_current_revision_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_current_revision_fkey FOREIGN KEY (tenant_id, project_id, id, current_revision_id) REFERENCES pc.relationship_revision(tenant_id, project_id, relationship_id, id) DEFERRABLE;


--
-- Name: relationship relationship_relationship_type_key_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_relationship_type_key_fkey FOREIGN KEY (relationship_type_key) REFERENCES pc.relationship_type(type_key);


--
-- Name: relationship_revision relationship_revision_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: relationship_revision relationship_revision_relationship_type_key_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_relationship_type_key_fkey FOREIGN KEY (relationship_type_key) REFERENCES pc.relationship_type(type_key);


--
-- Name: relationship_revision relationship_revision_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_document_id_doc_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_document_id_doc_fkey FOREIGN KEY (tenant_id, project_id, document_id, document_revision_id) REFERENCES pc.document_revision(tenant_id, project_id, document_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_document_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_document_id_fkey FOREIGN KEY (tenant_id, project_id, document_id) REFERENCES pc.document(tenant_id, project_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_relationship_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_relationship_id_fkey FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES pc.relationship(tenant_id, project_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_source_object__fkey1; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_source_object__fkey1 FOREIGN KEY (tenant_id, project_id, source_object_id, source_object_revision_id) REFERENCES pc.cognition_revision(tenant_id, project_id, object_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_source_object_i_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_source_object_i_fkey FOREIGN KEY (tenant_id, project_id, source_object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_target_object__fkey1; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_target_object__fkey1 FOREIGN KEY (tenant_id, project_id, target_object_id, target_object_revision_id) REFERENCES pc.cognition_revision(tenant_id, project_id, object_id, id);


--
-- Name: relationship_revision relationship_revision_tenant_id_project_id_target_object_i_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship_revision
    ADD CONSTRAINT relationship_revision_tenant_id_project_id_target_object_i_fkey FOREIGN KEY (tenant_id, project_id, target_object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: relationship relationship_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: relationship relationship_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: relationship relationship_tenant_id_project_id_source_object_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_source_object_id_fkey FOREIGN KEY (tenant_id, project_id, source_object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: relationship relationship_tenant_id_project_id_superseded_by_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_superseded_by_id_fkey FOREIGN KEY (tenant_id, project_id, superseded_by_id) REFERENCES pc.relationship(tenant_id, project_id, id);


--
-- Name: relationship relationship_tenant_id_project_id_target_object_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.relationship
    ADD CONSTRAINT relationship_tenant_id_project_id_target_object_id_fkey FOREIGN KEY (tenant_id, project_id, target_object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: source source_collected_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.source
    ADD CONSTRAINT source_collected_by_fkey FOREIGN KEY (collected_by) REFERENCES core.principal(id);


--
-- Name: source source_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.source
    ADD CONSTRAINT source_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: version_member version_member_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: version_member version_member_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: version_member version_member_tenant_id_project_id_object_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_object_id_fkey FOREIGN KEY (tenant_id, project_id, object_id) REFERENCES pc.cognition_object(tenant_id, project_id, id);


--
-- Name: version_member version_member_tenant_id_project_id_object_id_object_revis_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_object_id_object_revis_fkey FOREIGN KEY (tenant_id, project_id, object_id, object_revision_id) REFERENCES pc.cognition_revision(tenant_id, project_id, object_id, id);


--
-- Name: version_member version_member_tenant_id_project_id_relationship_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_relationship_id_fkey FOREIGN KEY (tenant_id, project_id, relationship_id) REFERENCES pc.relationship(tenant_id, project_id, id);


--
-- Name: version_member version_member_tenant_id_project_id_relationship_id_relati_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_relationship_id_relati_fkey FOREIGN KEY (tenant_id, project_id, relationship_id, relationship_revision_id) REFERENCES pc.relationship_revision(tenant_id, project_id, relationship_id, id);


--
-- Name: version_member version_member_tenant_id_project_id_version_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.version_member
    ADD CONSTRAINT version_member_tenant_id_project_id_version_id_fkey FOREIGN KEY (tenant_id, project_id, version_id) REFERENCES pc.cognition_version(tenant_id, project_id, id);


--
-- Name: write_operation write_operation_created_by_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.write_operation
    ADD CONSTRAINT write_operation_created_by_fkey FOREIGN KEY (created_by) REFERENCES core.principal(id);


--
-- Name: write_operation write_operation_tenant_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.write_operation
    ADD CONSTRAINT write_operation_tenant_id_fkey FOREIGN KEY (tenant_id) REFERENCES core.tenant(id);


--
-- Name: write_operation write_operation_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: pc; Owner: -
--

ALTER TABLE ONLY pc.write_operation
    ADD CONSTRAINT write_operation_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- Name: source_input source_input_tenant_id_project_id_fkey; Type: FK CONSTRAINT; Schema: source; Owner: -
--

ALTER TABLE ONLY source.source_input
    ADD CONSTRAINT source_input_tenant_id_project_id_fkey FOREIGN KEY (tenant_id, project_id) REFERENCES core.project(tenant_id, id);


--
-- PostgreSQL database dump complete
--


-- Data for Name: agent_definition; Type: TABLE DATA; Schema: ai; Owner: -
INSERT INTO ai.agent_definition VALUES ('b72a4c50-deea-4e02-8303-0cda10ea0dad', 'architect', '架构 Agent', 'architect', '负责技术方案与架构一致性', '2026-11-08 16:56:47.615899+00');
INSERT INTO ai.agent_definition VALUES ('77db9503-cc7b-43f7-9d8d-e2f1b7625316', 'developer', '开发 Agent', 'developer', '负责实现与交付', '2026-11-08 16:56:47.615899+00');
INSERT INTO ai.agent_definition VALUES ('1b561697-3ff3-4f7b-bb49-ed17b784b514', 'tester', '测试 Agent', 'tester', '负责验证与失败判定', '2026-11-08 16:56:47.615899+00');
-- Data for Name: tenant; Type: TABLE DATA; Schema: core; Owner: -
INSERT INTO core.tenant VALUES ('00000000-0000-0000-0000-000000000010', 'system', 'default', 'system', true, '2026-11-08 16:56:47.421539+00', NULL, NULL, '2026-11-08 16:56:48.674307+00');
-- Data for Name: customer; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: principal; Type: TABLE DATA; Schema: core; Owner: -
INSERT INTO core.principal VALUES ('00000000-0000-0000-0000-000000000001', 'service', 'ai-pm-dispatch', true, '2026-11-08 16:56:47.320479+00');
-- Data for Name: project; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: inbound_event; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: agent_feedback; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: ai_executor_capability; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: ai_role_template; Type: TABLE DATA; Schema: ai; Owner: -
INSERT INTO ai.ai_role_template VALUES ('9dd50e11-26af-49a3-9797-e6aa69140c20', 'project_manager', 'AI Project Manager', 'management', true, '{"contract": {}, "identity": {"version": "1.0", "role_key": "project_manager"}, "execution": {"acceptance_criteria": ["项目计划与里程碑已建立并可查询", "关键风险均有升级记录"]}, "governance": {"notes": "AI Project Manager 是管理协调角色，不允许项目禁用（AT-D12/稿§3.3）", "risk_level": "medium"}, "capabilities": {"capabilities": ["task_scheduling", "progress_tracking", "risk_escalation"]}, "responsibilities": {"constraints": ["不做具体编码与测试执行", "关键决策必须升级给人类确认"], "responsibilities": ["统筹项目计划与里程碑，跟踪交付进度", "协调需求、架构、开发、测试四方 AI 角色", "向人类 owner 升级关键决策与风险"]}}', '2026-11-08 16:56:49.368344+00', '2026-11-08 16:56:49.368344+00');
INSERT INTO ai.ai_role_template VALUES ('28df6dd6-0264-456e-bf4a-9355d779aa45', 'requirement_analyst', 'Requirement Analyst', 'product', true, '{"contract": {}, "identity": {"version": "1.0", "role_key": "requirement_analyst"}, "execution": {"acceptance_criteria": ["每条需求都有明确验收标准", "需求条目可追溯到来源"]}, "governance": {"risk_level": "medium"}, "capabilities": {"capabilities": ["requirement_extraction", "acceptance_definition"]}, "responsibilities": {"constraints": ["不擅自扩大需求范围", "模糊需求必须标记为待澄清"], "responsibilities": ["从访谈与材料中提炼结构化需求", "维护需求条目与验收标准的一致性"]}}', '2026-11-08 16:56:49.368344+00', '2026-11-08 16:56:49.368344+00');
INSERT INTO ai.ai_role_template VALUES ('42b6b5b7-d758-40a8-ba8d-700875a221ca', 'architect', 'Solution Architect', 'architecture', true, '{"contract": {}, "identity": {"version": "1.0", "role_key": "architect"}, "execution": {"acceptance_criteria": ["关键架构决策均有记录", "方案与既有架构不冲突"]}, "governance": {"risk_level": "medium"}, "capabilities": {"capabilities": ["architecture_design", "tech_decision"]}, "responsibilities": {"constraints": ["不直接实现业务代码", "架构变更需留下决策记录"], "responsibilities": ["产出技术方案与架构决策", "保证实现与架构一致性"]}}', '2026-11-08 16:56:49.368344+00', '2026-11-08 16:56:49.368344+00');
INSERT INTO ai.ai_role_template VALUES ('2f5e09b7-d81f-4f36-b9ce-543aedf4afde', 'developer', 'Developer', 'engineering', true, '{"contract": {}, "identity": {"version": "1.0", "role_key": "developer"}, "execution": {"acceptance_criteria": ["任务范围内的验收标准全部通过", "交付物有对应测试"]}, "governance": {"risk_level": "medium"}, "capabilities": {"capabilities": ["code_implementation", "debugging"]}, "responsibilities": {"constraints": ["不越过任务范围修改无关代码", "不绕过基线直接改主分支"], "responsibilities": ["按任务规格实现代码", "为交付物补充测试"]}}', '2026-11-08 16:56:49.368344+00', '2026-11-08 16:56:49.368344+00');
INSERT INTO ai.ai_role_template VALUES ('fa5b22f2-13cf-4c28-b8b2-55411d85674c', 'tester', 'QA Engineer', 'quality', true, '{"contract": {}, "identity": {"version": "1.0", "role_key": "tester"}, "execution": {"acceptance_criteria": ["每个交付物都有验证结论", "失败判定附可复现证据"]}, "governance": {"risk_level": "medium"}, "capabilities": {"capabilities": ["test_design", "failure_judgement"]}, "responsibilities": {"constraints": ["不直接修改业务代码", "判定结论必须附证据"], "responsibilities": ["设计并执行验证方案", "对失败交付给出可复现证据"]}}', '2026-11-08 16:56:49.368344+00', '2026-11-08 16:56:49.368344+00');
-- Data for Name: ai_role_version; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: analysis_run; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: cognition_run; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: baseline_proposal; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: baseline_item; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: change_proposal; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: cognition_item; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: conversation; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: conversation_message; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: project_document; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: document_revision; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: document_index_state; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: document_proposal; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: document_section_index; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: knowledge_reference; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: knowledge_write_operation; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: team_llm_provider; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: llm_call_record; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: project_ai_config; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: project_ai_model_policy; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: project_ai_role; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: project_baseline; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: project_knowledge; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: skill_definition; Type: TABLE DATA; Schema: ai; Owner: -
INSERT INTO ai.skill_definition VALUES ('requirement_analysis', 'Requirement Analysis', 'Analyze raw customer needs and requirement revisions for ambiguity, gaps and conflicts.', 'v1');
INSERT INTO ai.skill_definition VALUES ('task_analysis', 'Task Analysis', 'Decompose approved requirements into tasks with estimates and dependencies.', 'v1');
INSERT INTO ai.skill_definition VALUES ('risk_detection', 'Risk Detection', 'Detect schedule, scope and quality risks across requirements and tasks.', 'v1');
INSERT INTO ai.skill_definition VALUES ('architecture_consistency', 'Architecture Consistency', 'Check changes against documented architecture decisions and constraints.', 'v1');
INSERT INTO ai.skill_definition VALUES ('scheduling_orchestration', 'Scheduling Orchestration', 'Orchestrate milestones and task scheduling within capacity constraints.', 'v1');
-- Data for Name: project_skill_binding; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: team_llm_credential; Type: TABLE DATA; Schema: ai; Owner: -
-- Data for Name: approver_grant; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: principal_identity; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: project_initialization_operation; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: project_member; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: tenant_agent; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: tenant_invitation; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: tenant_invitation_link; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: tenant_invitation_link_redemption; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: tenant_member; Type: TABLE DATA; Schema: core; Owner: -
-- Data for Name: milestone; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_acceptance_criterion; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: device; Type: TABLE DATA; Schema: devenv; Owner: -
-- Data for Name: task_run; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: acceptance_criterion_verdict; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_interaction; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: continuation; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: interaction_response; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: interaction_response_delivery; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: policy_scope; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: requirement; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: requirement_acceptance_criterion; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: requirement_revision; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: requirement_confirmation; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: discovery_session; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: discovery_finding; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: requirement_finding; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: source_input; Type: TABLE DATA; Schema: source; Owner: -
-- Data for Name: requirement_source_input; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: review_finding; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_checkpoint; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_dependency; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_knowledge_ref; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_policy_override; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: project_repository; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: task_repository; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_repository_commit; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_requirement; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_run_cleanup; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_run_execution_authorization; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_run_failure; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_run_repository; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: approval_request; Type: TABLE DATA; Schema: governance; Owner: -
-- Data for Name: task_supersession; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: task_waiting; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: worker_command; Type: TABLE DATA; Schema: delivery; Owner: -
-- Data for Name: device_project_binding; Type: TABLE DATA; Schema: devenv; Owner: -
-- Data for Name: discovery_question; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: discovery_answer; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: repo_import; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: repo_import_source; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: repo_import_source_attempt; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: discovery_finding_evidence; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: discovery_finding_repository; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: discovery_message; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: discovery_proposal; Type: TABLE DATA; Schema: discovery; Owner: -
-- Data for Name: audit_log; Type: TABLE DATA; Schema: governance; Owner: -
-- Data for Name: human_intervention; Type: TABLE DATA; Schema: governance; Owner: -
-- Data for Name: review_approval; Type: TABLE DATA; Schema: governance; Owner: -
-- Data for Name: tenant_audit_log; Type: TABLE DATA; Schema: governance; Owner: -
-- Data for Name: intake_record; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: intake_task; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: job; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: notification; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: outbox_event; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: project_ssh_key; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: repo_import_draft_version; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: repository_version_confirmation; Type: TABLE DATA; Schema: integration; Owner: -
-- Data for Name: document; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: document_revision; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: memory; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: memory_revision; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: memory_ref; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: source; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: source_project; Type: TABLE DATA; Schema: kb; Owner: -
-- Data for Name: source; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: proposal; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: approval; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: object_type; Type: TABLE DATA; Schema: pc; Owner: -
INSERT INTO pc.object_type VALUES ('product_profile', '产品档案', '产品身份与基本信息（单例；描述修改 L1）', 'L1_identity', 'L1', true, 'product-profile', 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('product_vision', '产品愿景', '产品方向与价值主张（单例；全变更 L2）', 'L1_identity', 'L2', true, 'product-vision', 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('business_domain', '业务域', '业务域划分', 'L2_structure', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('business_capability', '业务能力', '业务能力（废弃/合并 L2）', 'L2_structure', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('feature_module', '功能模块', '功能模块', 'L2_structure', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('feature', '功能', '功能点（按关联与治理需要决定是否独立建模）', 'L2_structure', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('actor', '参与者', '使用角色', 'L3_usage', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('scenario', '场景', '使用场景', 'L3_usage', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('business_process', '业务流程', '业务流程（流程及状态转换 L2）', 'L3_usage', 'L2', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('page', '页面', '页面', 'L3_usage', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('interaction', '交互', '页面交互', 'L3_usage', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('domain_object', '领域对象', '业务语义对象', 'L4_semantic', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('business_rule', '业务规则', '业务规则（全生命周期 L2）', 'L4_semantic', 'L2', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('product_decision', '产品决策', '正式产品决策', 'L5_decision', 'L2', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('repository', '代码仓库', '仓库快照身份对象（checkpoint-1 派生）', 'snapshot', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('module', '模块/热点', '热点文件映射的模块对象（checkpoint-1 派生）', 'snapshot', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('tech_stack', '技术栈', '语言/扩展名构成（checkpoint-1 派生）', 'snapshot', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.object_type VALUES ('delivery_activity', '交付动态', '近期 commit 流摘要（checkpoint-1 派生）', 'snapshot', 'L1', false, NULL, 'active', '2026-11-08 16:56:49.522458+00');
-- Data for Name: cognition_object; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: document; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: document_revision; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: cognition_revision; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: cognition_version; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: conflict; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: relationship_type; Type: TABLE DATA; Schema: pc; Owner: -
INSERT INTO pc.relationship_type VALUES ('supports', '支撑', 'directed', '{product_vision,interaction}', '{project_goal.project_goal,scenario}', false, false, 'L1', 'active', 'Vision 支撑 Project Goal（跨域）/ Interaction 支撑 Scenario（同键双语义，§15.5 注记）', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('covers', '覆盖', 'directed', '{product_vision}', '{business_domain}', false, false, 'L1', 'active', '愿景覆盖业务域', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('contains', '包含', 'directed', '{business_domain,feature_module,page}', '{business_capability,feature,interaction}', false, false, 'L1', 'active', 'Domain→Capability / Module→Feature / Page→Interaction（同键三语义，§15.5 注记）', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('realized_by', '实现于', 'directed', '{business_capability,scenario}', '{feature_module,business_process}', false, false, 'L1', 'active', '能力/场景由模块或流程实现', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('specified_by', '规格于', 'directed', '{feature}', '{requirement.requirement_revision}', false, false, 'L1', 'active', '功能由需求修订规格化（Q-PC-03 Revision 级引用）', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('participates_in', '参与', 'directed', '{actor}', '{scenario}', false, false, 'L1', 'active', '参与者参与场景', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('uses', '使用', 'directed', '{scenario}', '{feature}', false, false, 'L1', 'active', '场景使用功能', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('governed_by', '受约束于', 'directed', '{business_process,domain_object}', '{business_rule}', false, false, 'L2', 'active', '流程/对象受业务规则约束（固定 L2）', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('implements', '实现', 'directed', '{page}', '{feature}', false, false, 'L1', 'active', '页面实现功能', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('constrains', '约束', 'directed', '{business_rule}', '{requirement.requirement_revision}', false, false, 'L2', 'active', '业务规则约束需求修订（固定 L2）', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('affects', '影响', 'directed', '{product_decision}', '{product_profile,product_vision,business_domain,business_capability,feature_module,feature,actor,scenario,business_process,page,interaction,domain_object,business_rule,product_decision}', false, false, 'L2', 'active', '决策影响面（全部 14 类对象；固定 L2）', '2026-11-08 16:56:49.522458+00');
INSERT INTO pc.relationship_type VALUES ('informs', '告知', 'directed', '{product_decision}', '{requirement.requirement_revision}', false, false, 'L2', 'active', '决策告知需求修订（固定 L2）', '2026-11-08 16:56:49.522458+00');
-- Data for Name: relationship; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: conflict_object; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: relationship_revision; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: evidence; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: proposal_member; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: version_member; Type: TABLE DATA; Schema: pc; Owner: -
-- Data for Name: write_operation; Type: TABLE DATA; Schema: pc; Owner: -

--
-- 尾注：会话状态复位（迁移在 sqlx 连接池连接上执行，session 级 SET 不外泄）
--
SET row_security = on;
SET client_min_messages = notice;
SET check_function_bodies = on;
SELECT pg_catalog.set_config('search_path', 'public', false);
