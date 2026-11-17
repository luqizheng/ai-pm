-- 0034_proposal_transition_guard.sql
--
-- P3-D04-02 Step5 Batch5 GAP-06：proposal 状态转换 DB 兜底触发器（两域镜像）。
-- 依据=设计方案 V1.0.7 §D1.2 合法转换表（T-01~T-10 allowlist）：纯拒绝型
-- （无推进逻辑，矩阵 GAP-06 行）；同态（OLD=NEW）放行（T-09 幂等语义+
-- 非状态列更新不受影响）。合法 8 转换：
--   draft→{submitted,withdrawn}；submitted→{approved,rejected,withdrawn,expired}；
--   approved→{applied,failed}；rejected/withdrawn/expired/applied/failed=终态。
-- AGENTS.md 规则 1：纯增量无数据迁移。

CREATE FUNCTION pc.guard_proposal_transition() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NEW.status = OLD.status THEN
        RETURN NEW;
    END IF;
    IF (OLD.status = 'draft' AND NEW.status IN ('submitted', 'withdrawn'))
       OR (OLD.status = 'submitted' AND NEW.status IN ('approved', 'rejected', 'withdrawn', 'expired'))
       OR (OLD.status = 'approved' AND NEW.status IN ('applied', 'failed'))
    THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'illegal pc.proposal status transition % -> %', OLD.status, NEW.status;
END;
$$;

CREATE TRIGGER trg_pc_proposal_transition
    BEFORE UPDATE ON pc.proposal
    FOR EACH ROW EXECUTE FUNCTION pc.guard_proposal_transition();

CREATE FUNCTION projc.guard_proposal_transition() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF NEW.status = OLD.status THEN
        RETURN NEW;
    END IF;
    IF (OLD.status = 'draft' AND NEW.status IN ('submitted', 'withdrawn'))
       OR (OLD.status = 'submitted' AND NEW.status IN ('approved', 'rejected', 'withdrawn', 'expired'))
       OR (OLD.status = 'approved' AND NEW.status IN ('applied', 'failed'))
    THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'illegal projc.proposal status transition % -> %', OLD.status, NEW.status;
END;
$$;

CREATE TRIGGER trg_projc_proposal_transition
    BEFORE UPDATE ON projc.proposal
    FOR EACH ROW EXECUTE FUNCTION projc.guard_proposal_transition();
