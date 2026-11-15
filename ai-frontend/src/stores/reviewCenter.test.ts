// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const {
  reviewApi,
  discoveryApi,
  knowledgeApi,
  acceptanceApi,
  reviewDecisionApi
} = vi.hoisted(() => ({
  reviewApi: { loadHumanActionBlock: vi.fn() },
  discoveryApi: {
    confirmDiscoveryFinding: vi.fn(),
    getDiscoverySession: vi.fn()
  },
  knowledgeApi: { decidePcProposal: vi.fn(), getPcProposal: vi.fn() },
  acceptanceApi: { listTaskDelivery: vi.fn() },
  reviewDecisionApi: {
    decideReviewApproval: vi.fn(),
    listTaskReviewApprovals: vi.fn(),
    listCriterionVerdicts: vi.fn()
  }
}));

vi.mock("@/api/reviewCenter", async importOriginal => ({
  ...(await importOriginal<object>()),
  loadHumanActionBlock: reviewApi.loadHumanActionBlock
}));
vi.mock("@/api/discovery", () => discoveryApi);
vi.mock("@/api/knowledge", () => knowledgeApi);
vi.mock("@/api/acceptance", () => acceptanceApi);
vi.mock("@/api/review", () => reviewDecisionApi);

import { HttpError } from "@digua/http";

import type { ActionItem } from "@/api/command-center";
import {
  useReviewCenterStore,
  type DeliveryContextData
} from "@/stores/reviewCenter";
import type { PcProposalDetail } from "@/api/knowledge";
import type { DiscoveryFinding } from "@/api/discovery";

function findingItem(overrides: Partial<ActionItem> = {}): ActionItem {
  return {
    kind: "finding_pending",
    domain: "discovery",
    id: "f-1",
    project_id: "p-1",
    title: "发现待处理（architecture）",
    detail: "发现内容摘要",
    source_state: "PENDING",
    updated_at: "2026-11-15T10:00:00Z",
    action_hint: "确认或拒绝该发现",
    link: {
      kind: "discovery_finding",
      params: { finding_id: "f-1", session_id: "s-1" }
    },
    ...overrides
  };
}

function proposalItem(): ActionItem {
  return {
    kind: "proposal_pending",
    domain: "knowledge",
    id: "pr-1",
    project_id: "p-1",
    title: "文档提案待确认（goal）",
    detail: "提案摘要",
    source_state: "submitted",
    updated_at: "2026-11-15T09:00:00Z",
    action_hint: "确认或否决该提案",
    link: { kind: "document_proposal", params: { proposal_id: "pr-1" } }
  };
}

function deliveryItem(): ActionItem {
  return {
    kind: "delivery_pending",
    domain: "delivery",
    id: "t-1",
    project_id: "p-1",
    title: "交付待验收（实现登录）",
    detail: undefined,
    source_state: "submitted",
    updated_at: "2026-11-15T08:00:00Z",
    action_hint: "验收交付证据",
    link: { kind: "task_delivery", params: { task_id: "t-1" } }
  };
}

function aggregate(items: ActionItem[]) {
  return {
    status: "ok" as const,
    asOf: "2026-11-15T10:05:00Z",
    total: items.length,
    items
  };
}

const proposalDetail = (): PcProposalDetail => ({
  proposal_id: "pr-1",
  status: "submitted",
  governance_level: "L2",
  members: [],
  content_hash: "abc",
  title: "目标修订提案",
  rationale: "调整目标表述",
  origin_kind: "analysis",
  risk_level: "medium",
  confidence: "high",
  expected_version_no: 3,
  submitted_at: "2026-11-15T09:00:00Z",
  created_at: "2026-11-15T08:30:00Z",
  updated_at: "2026-11-15T09:00:00Z",
  approvals: [],
  published_version_no: null
});

const findingDetail = (): DiscoveryFinding => ({
  id: "f-1",
  session_id: "s-1",
  kind: "architecture",
  content: "缺少鉴权层",
  confidence: 0.8,
  verification_status: "PENDING",
  evidence: [],
  actor_role: null,
  repository_ids: [],
  created_at: "2026-11-15T10:00:00Z"
});

const deliveryData = (): DeliveryContextData => ({
  rows: [],
  pendingApproval: {
    id: "ra-1",
    task_id: "t-1",
    review_run_id: "rr-1",
    delivery_run_id: "dr-1",
    kind: "delivery",
    status: "pending",
    decided_by: null,
    decision_reason: null,
    decided_at: null,
    created_at: "2026-11-15T08:00:00Z",
    task_title: "实现登录"
  },
  verdicts: null
});

/** 审核中心 store：聚合筛选 / 三类决策闭环（R05–R08）/ 409 语义。 */
describe("reviewCenter store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    reviewApi.loadHumanActionBlock.mockResolvedValue(
      aggregate([findingItem(), proposalItem(), deliveryItem()])
    );
    discoveryApi.getDiscoverySession.mockResolvedValue({
      session: {},
      messages: [],
      findings: [findingDetail()]
    });
    knowledgeApi.getPcProposal.mockResolvedValue(proposalDetail());
    acceptanceApi.listTaskDelivery.mockResolvedValue({
      task_id: "t-1",
      project_id: "p-1",
      items: [],
      total: 0
    });
    reviewDecisionApi.listTaskReviewApprovals.mockResolvedValue({
      items: [deliveryData().pendingApproval],
      total: 1
    });
    reviewDecisionApi.listCriterionVerdicts.mockResolvedValue({ items: [] });
  });

  it("load → ready；筛选（页签/搜索/排序）作用于混排队列", async () => {
    const store = useReviewCenterStore();
    await store.load("p-1");
    expect(store.status).toBe("ready");
    expect(store.totalCount).toBe(3);
    expect(store.filteredItems.map(item => item.id)).toEqual([
      "f-1",
      "pr-1",
      "t-1"
    ]);

    store.kindTab = "finding";
    expect(store.filteredItems.map(item => item.id)).toEqual(["f-1"]);

    store.kindTab = "all";
    store.search = "提案";
    expect(store.filteredItems.map(item => item.id)).toEqual(["pr-1"]);
    store.search = "";

    store.sortBy = "updated_asc";
    expect(store.filteredItems.map(item => item.id)).toEqual([
      "t-1",
      "pr-1",
      "f-1"
    ]);
  });

  it("块不可用 → error 且 totalCount null（徽标不伪造 0）", async () => {
    reviewApi.loadHumanActionBlock.mockResolvedValue({
      status: "unavailable",
      asOf: null,
      total: null,
      items: []
    });
    const store = useReviewCenterStore();
    await store.load("p-1");
    expect(store.status).toBe("error");
    expect(store.totalCount).toBeNull();
  });

  it("Finding 决策：answer 直传决策端点（R05/R06），成功后刷新聚合", async () => {
    const store = useReviewCenterStore();
    await store.load("p-1");
    const item = store.items[0];
    discoveryApi.confirmDiscoveryFinding.mockResolvedValue({});

    const result = await store.submitFindingDecision(
      "p-1",
      item,
      "CONFIRMED",
      "依据充分"
    );
    expect(result.ok).toBe(true);
    expect(discoveryApi.confirmDiscoveryFinding).toHaveBeenCalledWith(
      "p-1",
      "s-1",
      "f-1",
      { status: "CONFIRMED", answer: "依据充分" }
    );
    expect(store.lastFeedback).toBe("review.detail.doneConfirmed");
    expect(reviewApi.loadHumanActionBlock).toHaveBeenCalledTimes(2);
  });

  it("Finding 决策：link 缺 session_id（R06 前的旧数据）拒绝提交", async () => {
    const store = useReviewCenterStore();
    await store.load("p-1");
    const legacy = {
      ...store.items[0],
      link: { kind: "discovery_finding", params: { finding_id: "f-1" } }
    };
    const result = await store.submitFindingDecision(
      "p-1",
      legacy,
      "CONFIRMED",
      "x"
    );
    expect(result.ok).toBe(false);
    expect(discoveryApi.confirmDiscoveryFinding).not.toHaveBeenCalled();
  });

  it("Proposal 决策：CAS expected_version_no 回传；409 置 stale 并重读（R07）", async () => {
    const store = useReviewCenterStore();
    await store.load("p-1");
    const item = store.items[1];
    await store.loadProposalContext("p-1", item);
    knowledgeApi.decidePcProposal.mockRejectedValue(
      new HttpError("conflict", { status: 409 })
    );

    const result = await store.submitProposalDecision(
      "p-1",
      item,
      "approve",
      ""
    );
    expect(result.ok).toBe(false);
    expect(result.conflict).toBe(true);
    expect(knowledgeApi.decidePcProposal).toHaveBeenCalledWith("pr-1", "p-1", {
      action: "approve",
      expected_version_no: 3
    });
    expect(store.proposalStale["proposal_pending:pr-1"]).toBe(true);
    expect(knowledgeApi.getPcProposal).toHaveBeenCalledTimes(2);
  });

  it("Delivery 验收：无 pending 审批不提交；有 pending 走 D5 decision", async () => {
    const store = useReviewCenterStore();
    await store.load("p-1");
    const item = store.items[2];

    const noPending = await store.submitDeliveryDecision(
      "p-1",
      item,
      "approved",
      ""
    );
    expect(noPending.ok).toBe(false);
    expect(reviewDecisionApi.decideReviewApproval).not.toHaveBeenCalled();

    await store.loadDeliveryContext("p-1", item);
    reviewDecisionApi.decideReviewApproval.mockResolvedValue({
      outcome: "completed"
    });
    const result = await store.submitDeliveryDecision(
      "p-1",
      item,
      "approved",
      "证据齐备"
    );
    expect(result.ok).toBe(true);
    expect(reviewDecisionApi.decideReviewApproval).toHaveBeenCalledWith(
      "ra-1",
      { project_id: "p-1", decision: "approved", reason: "证据齐备" }
    );
  });
});
