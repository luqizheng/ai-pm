// @vitest-environment happy-dom
/**
 * S01-01 基线确认页行为测试。
 *
 * 断言的是 UX 硬语义，不是类名：
 *   - 当前生效基线始终可见
 *   - 未裁决 P0 时确认入口禁用并说明原因
 *   - AI 分析状态与人类裁决分别呈现
 *   - 差异视图默认只呈现变化内容
 */
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import { setLocale } from "@/shell/i18n";
import BaselineReviewView from "./BaselineReviewView.vue";
import type {
  BaselineOverview,
  BaselineProposal
} from "@/api/types/initialization";

const mocks = vi.hoisted(() => ({
  propose: vi.fn(),
  decideRisk: vi.fn(),
  revise: vi.fn(),
  reject: vi.fn()
}));

const overview: BaselineOverview = {
  current: {
    id: "baseline-1",
    project_id: "p1",
    version: 2,
    content: [],
    repositories: [],
    proposal_id: null,
    proposal_version: null,
    confirmed_by: "u1",
    confirmed_by_name: "张伟",
    confirmed_at: "2026-11-01T10:00:00Z",
    superseded_at: null,
    created_at: "2026-11-01T10:00:00Z"
  },
  history: [],
  pending_proposals: 1
};

function proposal(overrides: Partial<BaselineProposal> = {}): BaselineProposal {
  return {
    id: "prop-1",
    project_id: "p1",
    proposal_version: 3,
    status: "pending_review",
    generation_reason: "仓库已有新提交，重新生成项目理解",
    analysis_scope: ["project_identity"],
    sources: [
      {
        source_type: "code_fact",
        label: "core/project.rs",
        detail: null,
        locator: "crates/pm-domain/src/lib.rs:1"
      },
      {
        source_type: "ai_inference",
        label: "推断：该项目以任务编排为核心",
        detail: null,
        locator: null
      }
    ],
    repositories: [],
    content: [{ key: "identity", title: "项目身份", body: "Smark PM" }],
    diff: [
      {
        diff_type: "added",
        section: "模块",
        field: null,
        before: null,
        after: "新增交付校验模块",
        reason: null,
        locator: null
      },
      {
        diff_type: "unchanged",
        section: "业务领域",
        field: null,
        before: "研发管理",
        after: "研发管理",
        reason: null,
        locator: null
      }
    ],
    risks: [],
    repository_baselines: [],
    created_at: "2026-11-04T10:00:00Z",
    updated_at: "2026-11-04T10:00:00Z",
    confirmed_at: null,
    rejection_reason: null,
    failure_reason: null,
    ...overrides
  };
}

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: { projectId: "p1" } }),
  useRouter: () => ({ push: vi.fn() })
}));

vi.mock("@/api/initialization", () => ({
  getBaselineOverview: vi.fn(async () => overview),
  listBaselineProposals: vi.fn(async () => ({
    items: [proposal()],
    total: 1,
    page: 1,
    size: 50
  })),
  getBaselineProposal: vi.fn(async () => proposal()),
  confirmBaseline: mocks.propose,
  decideBaselineRisk: mocks.decideRisk,
  reviseBaselineProposal: mocks.revise,
  rejectBaselineProposal: mocks.reject
}));

vi.mock("@/api/format", () => ({
  formatDateTime: (value: string) => value
}));

async function mountView() {
  // i18n 是全局单例：其他测试文件可能切到 en，这里固定回 zh-CN
  setLocale("zh-CN");
  const wrapper = mountShell(BaselineReviewView);
  await nextTick();
  await nextTick();
  await nextTick();
  return wrapper;
}

describe("BaselineReviewView", () => {
  it("当前生效基线始终可见，审核新提案时不被隐藏", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("当前生效版本");
    expect(wrapper.text()).toContain("V2");
  });

  it("差异视图默认只呈现变化内容，未变化条目可筛出", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("新增交付校验模块");
    expect(wrapper.text()).not.toContain("研发管理");
  });

  it("存在未裁决 P0 风险时确认入口禁用并说明原因", async () => {
    vi.mocked(
      (await import("@/api/initialization")).getBaselineProposal
    ).mockResolvedValueOnce(
      proposal({
        risks: [
          {
            risk_id: "r1",
            risk_level: "P0",
            risk_title: "缺少鉴权设计",
            risk_description: "提案未覆盖权限边界",
            ai_status: "resolved",
            human_decision: null,
            evidence_ref: "docs/a.md",
            decided_by: null,
            decided_at: null,
            decision_note: null
          }
        ]
      })
    );
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("存在未裁决的 P0 风险");
    const confirmButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("确认生效"));
    expect(confirmButton?.attributes("disabled")).toBeDefined();
  });

  it("AI 标记已解决与人类已裁决分别呈现，不混为一谈", async () => {
    vi.mocked(
      (await import("@/api/initialization")).getBaselineProposal
    ).mockResolvedValueOnce(
      proposal({
        risks: [
          {
            risk_id: "r1",
            risk_level: "P1",
            risk_title: "术语不统一",
            risk_description: "同一概念两种叫法",
            ai_status: "resolved",
            human_decision: null,
            evidence_ref: null,
            decided_by: null,
            decided_at: null,
            decision_note: null
          }
        ]
      })
    );
    const wrapper = await mountView();
    // Badge 只标「AI 标记已解决」，人类裁决列留空（—）——两者不混为一谈。
    expect(wrapper.text()).toContain("AI 标记已解决");
    expect(wrapper.text()).not.toContain("人类已裁决");
    expect(wrapper.text()).toContain("人类裁决");
  });

  it("无未裁决 P0 时确认入口可用", async () => {
    const wrapper = await mountView();
    const confirmButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("确认生效"));
    expect(confirmButton?.attributes("disabled")).toBeUndefined();
  });
});
