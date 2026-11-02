// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import DiscoveryPanel from "./DiscoveryPanel.vue";
import { mountShell } from "../../test-utils";

const listDiscoverySessions = vi.fn();
const startDiscoverySession = vi.fn();
const getDiscoverySession = vi.fn();
const transitionDiscoverySession = vi.fn();
const confirmDiscoveryFinding = vi.fn();
const answerDiscoveryQuestion = vi.fn();
const deferDiscoveryQuestion = vi.fn();

vi.mock("@/api/discovery", () => ({
  listDiscoverySessions: (...args: unknown[]) => listDiscoverySessions(...args),
  startDiscoverySession: (...args: unknown[]) => startDiscoverySession(...args),
  getDiscoverySession: (...args: unknown[]) => getDiscoverySession(...args),
  transitionDiscoverySession: (...args: unknown[]) => transitionDiscoverySession(...args),
  confirmDiscoveryFinding: (...args: unknown[]) => confirmDiscoveryFinding(...args),
  answerDiscoveryQuestion: (...args: unknown[]) => answerDiscoveryQuestion(...args),
  deferDiscoveryQuestion: (...args: unknown[]) => deferDiscoveryQuestion(...args)
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

const session = {
  id: "session-1",
  project_id: "project-1",
  source: "WEB",
  kind: "INITIAL_ONBOARDING",
  status: "IN_PROGRESS",
  created_by: "human-1",
  created_at: "2026-11-02T00:00:00Z",
  updated_at: "2026-11-02T00:00:00Z"
};

function detail(overrides: Record<string, unknown> = {}) {
  return {
    session,
    messages: [],
    findings: [],
    questions: [],
    proposals: [],
    ...overrides
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  listDiscoverySessions.mockResolvedValue({ items: [], total: 0 });
});

describe("DiscoveryPanel", () => {
  it("无会话时展示开始按钮，点击创建会话（带 import_id）并加载详情", async () => {
    startDiscoverySession.mockResolvedValue({
      session,
      questions_added: 1,
      linked_import_id: "import-1"
    });
    getDiscoverySession.mockResolvedValue(
      detail({
        questions: [
          {
            id: "q-1",
            session_id: session.id,
            finding_id: null,
            question: "支付模块由谁实现？",
            why_blocking: "阻塞架构认知",
            status: "open",
            created_at: "2026-11-02T00:00:00Z",
            answers: []
          }
        ]
      })
    );
    const wrapper = mountShell(DiscoveryPanel, {
      props: { projectId: "project-1", importId: "import-1" }
    });
    await flushPromises();

    const startButton = wrapper
      .findAll("button")
      .find(node => node.text().includes("开始认知访谈"));
    expect(startButton).toBeDefined();
    await startButton!.trigger("click");
    await flushPromises();

    expect(startDiscoverySession).toHaveBeenCalledWith("project-1", "import-1");
    expect(getDiscoverySession).toHaveBeenCalledWith("project-1", session.id);
    expect(wrapper.text()).toContain("支付模块由谁实现？");
    expect(wrapper.text()).toContain("阻塞架构认知");
  });

  it("待确认发现卡片展示 kind 徽标与分析证据，确认后写入裁决", async () => {
    listDiscoverySessions.mockResolvedValue({
      items: [{ id: session.id }],
      total: 1
    });
    const finding = {
      id: "f-1",
      session_id: session.id,
      kind: "speculation",
      content: "支付模块可能尚未完成",
      confidence: 0.78,
      verification_status: "PENDING",
      evidence: [
        {
          repository: "backend",
          file: "src/payment/mod.rs",
          description: "存在未实现的支付接口"
        }
      ],
      actor_role: null,
      repository_ids: [],
      created_at: "2026-11-02T00:00:00Z"
    };
    getDiscoverySession.mockResolvedValue(detail({ findings: [finding] }));
    confirmDiscoveryFinding.mockResolvedValue({
      finding: { ...finding, verification_status: "CONFIRMED" },
      question: null,
      answer: null,
      confirmed_finding: null
    });
    const wrapper = mountShell(DiscoveryPanel, {
      props: { projectId: "project-1" }
    });
    await flushPromises();

    expect(wrapper.text()).toContain("支付模块可能尚未完成");
    expect(wrapper.text()).toContain("src/payment/mod.rs");
    expect(wrapper.text()).toContain("存在未实现的支付接口");

    const radio = wrapper.findAll('input[type="radio"]').at(0)!;
    await radio.setValue();
    const confirmButton = wrapper
      .findAll("button")
      .find(node => node.text().includes("确认") && !node.text().includes("不符"));
    await confirmButton!.trigger("click");
    await flushPromises();

    expect(confirmDiscoveryFinding).toHaveBeenCalledTimes(1);
    const [projectId, sessionId, findingId, payload] =
      confirmDiscoveryFinding.mock.calls[0];
    expect(projectId).toBe("project-1");
    expect(sessionId).toBe(session.id);
    expect(findingId).toBe("f-1");
    expect(payload.status).toBe("CONFIRMED");
    expect(payload.answer).toContain("确认属实");
  });

  it("回答待澄清问题后提交 answerDiscoveryQuestion", async () => {
    listDiscoverySessions.mockResolvedValue({
      items: [{ id: session.id }],
      total: 1
    });
    getDiscoverySession.mockResolvedValue(
      detail({
        questions: [
          {
            id: "q-1",
            session_id: session.id,
            finding_id: null,
            question: "部署形态是单机还是集群？",
            why_blocking: null,
            status: "open",
            created_at: "2026-11-02T00:00:00Z",
            answers: []
          }
        ]
      })
    );
    answerDiscoveryQuestion.mockResolvedValue({
      id: "a-1",
      question_id: "q-1",
      answer: "单机",
      answered_by: "human-1",
      created_at: "2026-11-02T00:00:00Z"
    });
    const wrapper = mountShell(DiscoveryPanel, {
      props: { projectId: "project-1" }
    });
    await flushPromises();

    const input = wrapper.find("input:not([type='radio'])");
    await input.setValue("单机");
    const submitButton = wrapper
      .findAll("button")
      .find(node => node.text().includes("提交回答"));
    await submitButton!.trigger("click");
    await flushPromises();

    expect(answerDiscoveryQuestion).toHaveBeenCalledWith(
      "project-1",
      session.id,
      "q-1",
      "单机"
    );
  });

  it("问题与发现全部裁决后出现标记完成按钮并推进状态", async () => {
    listDiscoverySessions.mockResolvedValue({
      items: [{ id: session.id }],
      total: 1
    });
    getDiscoverySession.mockResolvedValue(
      detail({
        findings: [
          {
            id: "f-1",
            session_id: session.id,
            kind: "user_confirmed",
            content: "支付由外部服务实现",
            confidence: null,
            verification_status: "CONFIRMED",
            evidence: [],
            actor_role: null,
            repository_ids: [],
            created_at: "2026-11-02T00:00:00Z"
          }
        ]
      })
    );
    transitionDiscoverySession.mockResolvedValue({
      ...session,
      status: "COMPLETED"
    });
    const wrapper = mountShell(DiscoveryPanel, {
      props: { projectId: "project-1" }
    });
    await flushPromises();

    expect(wrapper.text()).toContain("已确认");
    const completeButton = wrapper
      .findAll("button")
      .find(node => node.text().includes("标记访谈完成"));
    expect(completeButton).toBeDefined();
    await completeButton!.trigger("click");
    await flushPromises();

    expect(transitionDiscoverySession).toHaveBeenCalledWith(
      "project-1",
      session.id,
      "COMPLETED"
    );
  });
});