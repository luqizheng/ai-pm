import { computed, ref } from "vue";
import { defineStore } from "pinia";

import { HttpError } from "@digua/http";

import type { ActionItem } from "@/api/command-center";
import {
  loadHumanActionBlock,
  reviewItemKey,
  reviewTabOf,
  type HumanActionBlock
} from "@/api/reviewCenter";
import {
  confirmDiscoveryFinding,
  getDiscoverySession,
  type DiscoveryFinding
} from "@/api/discovery";
import {
  decidePcProposal,
  getPcProposal,
  type PcProposalDetail
} from "@/api/knowledge";
import { listTaskDelivery, type TaskDeliveryRow } from "@/api/acceptance";
import {
  decideReviewApproval,
  listTaskReviewApprovals,
  listCriterionVerdicts,
  type CriterionVerdict,
  type ReviewApproval
} from "@/api/review";

/**
 * 人工审核中心页面状态（R01–R12，docs/ui/review-center）。
 *
 * 职责边界（D5/D7，专家 V1.1 §十二）：
 * - 只管队列、筛选、选中与提交中状态；**领域决策规则不在这里**——
 *   Finding→discovery、Proposal→pc、Delivery→D5 review-approvals，各自直调；
 * - 领域状态是业务事实，成功后重新拉聚合（D7），前端不维护第二状态权威；
 * - 409（CAS 失配 / anchor_drift / already_decided）按独立业务异常处理，
 *   不自动重试（R07）。
 */

export const REVIEW_CENTER_PATH = "/project/review-center";

export type ReviewTab = "all" | "finding" | "report" | "proposal" | "delivery";
export type ReviewSort = "updated_desc" | "updated_asc";

export type FindingDecisionStatus = "CONFIRMED" | "REJECTED";
export type ProposalDecisionAction = "approve" | "reject";
export type DeliveryDecision = "approved" | "rejected";

export interface DecisionResult {
  ok: boolean;
  /** 409 家族：版本/锚点已变或已被处理，不自动重试。 */
  conflict?: boolean;
  noPermission?: boolean;
  /** 领域回执语义（如 D5 changes_requested）。 */
  outcome?: string;
  message?: string;
}

export interface DeliveryContextData {
  rows: TaskDeliveryRow[];
  pendingApproval: ReviewApproval | null;
  verdicts: CriterionVerdict[] | null;
}

interface DomainCtx<T> {
  loading: boolean;
  failed: boolean;
  data: T | null;
}

export const useReviewCenterStore = defineStore("reviewCenter", () => {
  const projectId = ref<string | null>(null);
  const block = ref<HumanActionBlock | null>(null);
  const status = ref<"idle" | "loading" | "ready" | "error">("idle");
  const errorMessage = ref<string | null>(null);

  // 队列筛选 / 排序（R03：474px 队列只承担识别与排序）
  const kindTab = ref<ReviewTab>("all");
  const domainFilter = ref<string | null>(null);
  const search = ref("");
  const sortBy = ref<ReviewSort>("updated_desc");

  // 选中（kind:id 复合键）
  const selectedKey = ref<string | null>(null);
  const submittingKey = ref<string | null>(null);
  const lastFeedback = ref<string | null>(null);

  // 领域上下文缓存（key = reviewItemKey）
  const findingCtx = ref<Record<string, DomainCtx<DiscoveryFinding>>>({});
  const proposalCtx = ref<Record<string, DomainCtx<PcProposalDetail>>>({});
  /** Proposal CAS 失配后置位：展示 stale 提示，直到成功决策或换选。 */
  const proposalStale = ref<Record<string, boolean>>({});
  const deliveryCtx = ref<Record<string, DomainCtx<DeliveryContextData>>>({});

  // 徽标缓存：同一项目不重复拉（SideNav 全局渲染调 ensureLoaded）
  let loadedProjectId: string | null = null;
  let inflight: Promise<void> | null = null;

  const items = computed<ActionItem[]>(() => block.value?.items ?? []);
  /** `null` = 聚合不可用（徽标不得伪造 0，V1.1 §6.3）。 */
  const totalCount = computed<number | null>(() =>
    block.value && block.value.status === "ok" ? block.value.total : null
  );

  const filteredItems = computed<ActionItem[]>(() => {
    const kw = search.value.trim().toLowerCase();
    const list = items.value.filter(item => {
      if (kindTab.value !== "all" && reviewTabOf(item) !== kindTab.value) {
        return false;
      }
      if (domainFilter.value && item.domain !== domainFilter.value) {
        return false;
      }
      if (
        kw &&
        !`${item.title}\n${item.detail ?? ""}`.toLowerCase().includes(kw)
      ) {
        return false;
      }
      return true;
    });
    return [...list].sort((a, b) => {
      const ta = Date.parse(a.updated_at ?? "") || 0;
      const tb = Date.parse(b.updated_at ?? "") || 0;
      return sortBy.value === "updated_desc" ? tb - ta : ta - tb;
    });
  });

  const selectedItem = computed<ActionItem | null>(() => {
    if (!selectedKey.value) return null;
    return (
      items.value.find(item => reviewItemKey(item) === selectedKey.value) ??
      null
    );
  });

  /** 聚合加载（页面主路径）。 */
  async function load(pid: string): Promise<void> {
    projectId.value = pid;
    status.value = "loading";
    errorMessage.value = null;
    try {
      block.value = await loadHumanActionBlock(pid);
      loadedProjectId = pid;
      if (block.value.status === "unavailable") {
        status.value = "error";
        errorMessage.value =
          block.value.errorMessage ?? block.value.errorCode ?? "unavailable";
      } else {
        status.value = "ready";
        // 选中项可能在刷新后消失（已被处理）：保留键，selectedItem 自然置空
      }
    } catch (err) {
      status.value = "error";
      errorMessage.value = err instanceof Error ? err.message : String(err);
    }
  }

  /** 徽标路径：同项目已就绪则跳过；进行中合并请求。 */
  function ensureLoaded(pid: string): Promise<void> {
    if (loadedProjectId === pid && status.value === "ready")
      return Promise.resolve();
    if (!inflight) {
      inflight = load(pid).finally(() => {
        inflight = null;
      });
    }
    return inflight;
  }

  function select(key: string | null): void {
    selectedKey.value = key;
    if (key === null) return;
    // 换选清上一事项的临时反馈；409 stale 在重新成功决策时清除
    lastFeedback.value = null;
  }

  // ── 领域上下文（展示数据直读领域读端点；不塞进聚合契约）──────────

  async function loadFindingContext(pid: string, item: ActionItem) {
    const key = reviewItemKey(item);
    const sessionId = item.link.params.session_id;
    const findingId = item.link.params.finding_id;
    if (!sessionId || !findingId) {
      findingCtx.value[key] = { loading: false, failed: true, data: null };
      return;
    }
    if (findingCtx.value[key]?.data) return;
    findingCtx.value[key] = { loading: true, failed: false, data: null };
    try {
      const detail = await getDiscoverySession(pid, sessionId);
      const finding = detail.findings.find(f => f.id === findingId) ?? null;
      findingCtx.value[key] = {
        loading: false,
        failed: finding === null,
        data: finding
      };
    } catch {
      findingCtx.value[key] = { loading: false, failed: true, data: null };
    }
  }

  async function loadProposalContext(
    pid: string,
    item: ActionItem,
    force = false
  ) {
    const key = reviewItemKey(item);
    const proposalId = item.link.params.proposal_id;
    if (!proposalId) {
      proposalCtx.value[key] = { loading: false, failed: true, data: null };
      return;
    }
    if (!force && proposalCtx.value[key]?.data) return;
    proposalCtx.value[key] = { loading: true, failed: false, data: null };
    try {
      const detail = await getPcProposal(proposalId, pid);
      proposalCtx.value[key] = { loading: false, failed: false, data: detail };
    } catch {
      proposalCtx.value[key] = { loading: false, failed: true, data: null };
    }
  }

  async function loadDeliveryContext(pid: string, item: ActionItem) {
    const key = reviewItemKey(item);
    const taskId = item.link.params.task_id;
    if (!taskId) {
      deliveryCtx.value[key] = { loading: false, failed: true, data: null };
      return;
    }
    if (deliveryCtx.value[key]?.data) return;
    deliveryCtx.value[key] = { loading: true, failed: false, data: null };
    try {
      const [delivery, approvals, verdicts] = await Promise.all([
        listTaskDelivery(taskId),
        listTaskReviewApprovals(taskId),
        listCriterionVerdicts(taskId).catch(() => null)
      ]);
      deliveryCtx.value[key] = {
        loading: false,
        failed: false,
        data: {
          rows: delivery.items,
          pendingApproval:
            approvals.items.find(a => a.status === "pending") ?? null,
          verdicts: verdicts?.items ?? null
        }
      };
    } catch {
      deliveryCtx.value[key] = { loading: false, failed: true, data: null };
    }
  }

  // ── 决策（R05/R07：理由与 CAS 规则按领域配置）────────────────────

  function errorResult(err: unknown): DecisionResult {
    if (err instanceof HttpError) {
      if (err.status === 403) {
        return { ok: false, noPermission: true, message: err.message };
      }
      if (err.status === 409) {
        return { ok: false, conflict: true, message: err.message };
      }
    }
    return {
      ok: false,
      message: err instanceof Error ? err.message : String(err)
    };
  }

  /** Finding 决策（R06：link 携带 session_id；R05：answer 必填）。 */
  async function submitFindingDecision(
    pid: string,
    item: ActionItem,
    decision: FindingDecisionStatus,
    answer: string
  ): Promise<DecisionResult> {
    const sessionId = item.link.params.session_id;
    const findingId = item.link.params.finding_id;
    if (!sessionId || !findingId) {
      return { ok: false, message: "missing link params" };
    }
    const key = reviewItemKey(item);
    submittingKey.value = key;
    try {
      await confirmDiscoveryFinding(pid, sessionId, findingId, {
        status: decision,
        answer
      });
      lastFeedback.value =
        decision === "CONFIRMED"
          ? "review.detail.doneConfirmed"
          : "review.detail.doneRejected";
      await load(pid);
      return { ok: true };
    } catch (err) {
      const result = errorResult(err);
      // 409：事项状态已变（可能已被处理）——刷新聚合但不重试决策
      if (result.conflict) await load(pid);
      return result;
    } finally {
      submittingKey.value = null;
    }
  }

  /** Proposal 决策（R07：CAS expected_version_no；409 不自动重试）。 */
  async function submitProposalDecision(
    pid: string,
    item: ActionItem,
    action: ProposalDecisionAction,
    note: string
  ): Promise<DecisionResult> {
    const proposalId = item.link.params.proposal_id;
    if (!proposalId) return { ok: false, message: "missing link params" };
    const key = reviewItemKey(item);
    const detail = proposalCtx.value[key]?.data;
    if (!detail) return { ok: false, message: "proposal not loaded" };
    submittingKey.value = key;
    try {
      await decidePcProposal(proposalId, pid, {
        action,
        ...(note.trim() ? { note: note.trim() } : {}),
        expected_version_no: detail.expected_version_no
      });
      proposalStale.value[key] = false;
      lastFeedback.value =
        action === "approve"
          ? "review.detail.doneApproved"
          : "review.detail.doneRejectedProposal";
      await load(pid);
      return { ok: true };
    } catch (err) {
      const result = errorResult(err);
      if (result.conflict) {
        // CAS 失配：置 stale 提示 + 重新加载最新版本（不自动重试决策）
        proposalStale.value[key] = true;
        await loadProposalContext(pid, item, true);
        await load(pid);
      }
      return result;
    } finally {
      submittingKey.value = null;
    }
  }

  /** Delivery 验收（D5 decision；anchor_drift 等 409 透传，不静默重试）。 */
  async function submitDeliveryDecision(
    pid: string,
    item: ActionItem,
    decision: DeliveryDecision,
    reason: string
  ): Promise<DecisionResult> {
    const key = reviewItemKey(item);
    const pending = deliveryCtx.value[key]?.data?.pendingApproval;
    if (!pending) return { ok: false, message: "no pending approval" };
    submittingKey.value = key;
    try {
      const outcome = await decideReviewApproval(pending.id, {
        project_id: pid,
        decision,
        ...(reason.trim() ? { reason: reason.trim() } : {})
      });
      lastFeedback.value =
        outcome.outcome === "changes_requested"
          ? "review.delivery.outcomeChangesRequested"
          : decision === "approved"
            ? "review.detail.doneApproved"
            : "review.detail.doneRejectedProposal";
      await load(pid);
      return { ok: true, outcome: outcome.outcome };
    } catch (err) {
      const result = errorResult(err);
      if (result.conflict) await loadDeliveryContext(pid, item);
      return result;
    } finally {
      submittingKey.value = null;
    }
  }

  return {
    projectId,
    block,
    status,
    errorMessage,
    kindTab,
    domainFilter,
    search,
    sortBy,
    selectedKey,
    submittingKey,
    lastFeedback,
    findingCtx,
    proposalCtx,
    proposalStale,
    deliveryCtx,
    items,
    totalCount,
    filteredItems,
    selectedItem,
    load,
    ensureLoaded,
    select,
    loadFindingContext,
    loadProposalContext,
    loadDeliveryContext,
    submitFindingDecision,
    submitProposalDecision,
    submitDeliveryDecision
  };
});
