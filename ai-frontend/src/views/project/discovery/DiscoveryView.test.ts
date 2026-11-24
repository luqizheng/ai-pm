// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import DiscoveryView from "./DiscoveryView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import { useShellStore } from "@/stores/shell";
import type { DiscoverySession, DiscoverySessionDetail } from "@/api/discovery";

/**
 * 代码发现（S01-I2，接真）行为测试：
 * 治理红线——**AI 只产发现，人类负责确认/否决**。只有 PENDING / CONFLICT
 * 的发现给人工动作；已定案（CONFIRMED / REJECTED）不再给按钮。
 */

const listDiscoverySessions = vi.fn();
const getDiscoverySession = vi.fn();
const confirmDiscoveryFinding = vi.fn();
const answerDiscoveryQuestion = vi.fn();
const deferDiscoveryQuestion = vi.fn();
const startDiscoverySession = vi.fn();
const transitionDiscoverySession = vi.fn();

vi.mock("@/api/discovery", () => ({
  listDiscoverySessions: (...args: unknown[]) => listDiscoverySessions(...args),
  getDiscoverySession: (...args: unknown[]) => getDiscoverySession(...args),
  confirmDiscoveryFinding: (...args: unknown[]) =>
    confirmDiscoveryFinding(...args),
  answerDiscoveryQuestion: (...args: unknown[]) =>
    answerDiscoveryQuestion(...args),
  deferDiscoveryQuestion: (...args: unknown[]) =>
    deferDiscoveryQuestion(...args),
  startDiscoverySession: (...args: unknown[]) => startDiscoverySession(...args),
  transitionDiscoverySession: (...args: unknown[]) =>
    transitionDiscoverySession(...args)
}));

function session(overrides: Partial<DiscoverySession> = {}): DiscoverySession {
  return {
    id: "sess-1",
    project_id: "proj-1",
    source: "WEB",
    kind: "initial",
    status: "active",
    created_by: "p-1",
    created_at: "2026-11-10T08:00:00Z",
    updated_at: "2026-11-10T09:00:00Z",
    last_round_error: null,
    last_round_error_at: null,
    ...overrides
  };
}

function detail(sessionOverrides: Partial<DiscoverySession> = {}): DiscoverySessionDetail {
  return {
    session: session(sessionOverrides),
    messages: [],
    findings: [
      {
        id: "f-pending",
        session_id: "sess-1",
        kind: "architecture",
        content: "认证服务使用自签令牌",
        confidence: 0.8,
        verification_status: "PENDING",
        evidence: [],
        actor_role: "architect",
        repository_ids: [],
        created_at: "2026-11-10T08:10:00Z"
      },
      {
        id: "f-confirmed",
        session_id: "sess-1",
        kind: "architecture",
        content: "已确认的发现",
        confidence: 0.9,
        verification_status: "CONFIRMED",
        evidence: [],
        actor_role: "architect",
        repository_ids: [],
        created_at: "2026-11-10T08:20:00Z"
      }
    ],
    questions: [
      {
        id: "q-1",
        session_id: "sess-1",
        finding_id: null,
        question: "令牌是否由网关统一签发？",
        why_blocking: "影响认证边界判定",
        status: "open",
        created_at: "2026-11-10T08:30:00Z",
        answers: []
      }
    ],
    proposals: []
  };
}

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/project/discovery");
  await router.isReady();
  const { wrapper } = mountShell(
    DiscoveryView,
    { attachTo: document.body },
    router
  );
  const shell = useShellStore();
  shell.setProjects([{ id: "proj-1", name: "评测项目" }]);
  shell.setCurrentProject("proj-1");
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  listDiscoverySessions.mockReset();
  getDiscoverySession.mockReset();
  confirmDiscoveryFinding.mockReset();
  transitionDiscoverySession.mockReset();
  listDiscoverySessions.mockResolvedValue({ items: [session()], total: 1 });
  getDiscoverySession.mockResolvedValue(detail());
  confirmDiscoveryFinding.mockResolvedValue({});
  transitionDiscoverySession.mockResolvedValue(session());
});

describe("DiscoveryView", () => {
  it("读真实会话并渲染发现与阻塞问题", async () => {
    const { wrapper } = await mountView();

    expect(listDiscoverySessions).toHaveBeenCalledWith("proj-1");
    expect(getDiscoverySession).toHaveBeenCalledWith("proj-1", "sess-1");
    const text = wrapper.text();
    expect(text).toContain("认证服务使用自签令牌");
    expect(text).toContain("令牌是否由网关统一签发？");
    expect(text).toContain("影响认证边界判定");
  });

  it("只有未定案发现给人工动作，已确认的不再给按钮", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.find('[data-testid="dc-confirm-f-pending"]').exists()).toBe(
      true
    );
    expect(wrapper.find('[data-testid="dc-reject-f-pending"]').exists()).toBe(
      true
    );
    // 已 CONFIRMED → 无动作按钮
    expect(
      wrapper.find('[data-testid="dc-confirm-f-confirmed"]').exists()
    ).toBe(false);
  });

  it("确认发现调真实命令并用大写状态词表", async () => {
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="dc-confirm-f-pending"]').trigger("click");
    await flushPromises();

    expect(confirmDiscoveryFinding).toHaveBeenCalledWith(
      "proj-1",
      "sess-1",
      "f-pending",
      expect.objectContaining({ status: "CONFIRMED" })
    );
  });

  // ── B03-01（R1）：等待复核/输入的会话必须有处理入口（否则消息死信） ──

  it("REVIEW_REQUIRED 会话给出复核提示与退回/完结入口", async () => {
    getDiscoverySession.mockResolvedValue(
      detail({ status: "REVIEW_REQUIRED" })
    );
    const { wrapper } = await mountView();
    await flushPromises();

    expect(
      wrapper.find('[data-testid="dc-session-actions"]').exists()
    ).toBe(true);
    expect(
      wrapper.find('[data-testid="dc-resume-session"]').exists()
    ).toBe(true);
    expect(
      wrapper.find('[data-testid="dc-complete-session"]').exists()
    ).toBe(true);
  });

  it("退回/完结调真实跃迁命令", async () => {
    getDiscoverySession.mockResolvedValue(
      detail({ status: "REVIEW_REQUIRED" })
    );
    const { wrapper } = await mountView();
    await flushPromises();

    await wrapper.find('[data-testid="dc-resume-session"]').trigger("click");
    await flushPromises();
    expect(transitionDiscoverySession).toHaveBeenCalledWith(
      "proj-1",
      "sess-1",
      "IN_PROGRESS"
    );

    await wrapper.find('[data-testid="dc-complete-session"]').trigger("click");
    await flushPromises();
    expect(transitionDiscoverySession).toHaveBeenCalledWith(
      "proj-1",
      "sess-1",
      "COMPLETED"
    );
  });

  it("NEEDS_USER_INPUT 会话只给完结入口；失败标记可见", async () => {
    getDiscoverySession.mockResolvedValue(
      detail({
        status: "NEEDS_USER_INPUT",
        last_round_error: "model_error: timeout",
        last_round_error_at: "2026-11-24T08:00:00Z"
      })
    );
    const { wrapper } = await mountView();
    await flushPromises();

    expect(
      wrapper.find('[data-testid="dc-resume-session"]').exists()
    ).toBe(false);
    expect(
      wrapper.find('[data-testid="dc-complete-session"]').exists()
    ).toBe(true);
    expect(wrapper.find('[data-testid="dc-round-error"]').text()).toContain(
      "model_error: timeout"
    );
  });

  it("跃迁失败（409 竞态）不打断，重拉详情对齐服务端", async () => {
    getDiscoverySession.mockResolvedValue(
      detail({ status: "REVIEW_REQUIRED" })
    );
    transitionDiscoverySession.mockRejectedValue(new Error("409"));
    const { wrapper } = await mountView();
    await flushPromises();

    await wrapper.find('[data-testid="dc-resume-session"]').trigger("click");
    await flushPromises();

    expect(transitionDiscoverySession).toHaveBeenCalledTimes(1);
    // 失败后重拉了详情（服务端状态是唯一真源）。
    expect(getDiscoverySession.mock.calls.length).toBeGreaterThan(1);
  });
});
