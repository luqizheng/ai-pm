// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, type VueWrapper } from "@vue/test-utils";

import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import { i18n } from "@/lib/i18n";
import Select from "@/components/ui/select/Select.vue";
import type {
  Conversation,
  ConversationMessage as ConversationMessageRecord
} from "@/api/conversations";

import AssistantView from "./AssistantView.vue";
import type { DiscoverySessionDetail } from "@/api/discovery";

// AI 文本消息走 MarkdownViewer（Vditor md2html）：happy-dom 禁脚本加载会抛
// 未处理 rejection 并污染并行 worker 的其他套件——按 responses-a/pages-b 的
// 做法整体替换 vditor。
const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: {
    md2html,
    preview: vi.fn(),
    version: "test"
  }
}));

// 会话接线批（2026-11-08）+ Batch G 轮询批 + Batch H 首屏最新窗口：
// 页面数据一律来自 @/api/conversations（D-4 REST），这里只 mock IO 函数；
// 三件套组件为真实实现——发送链路、增量轮询、首屏 latest 参数与 assistant
// 渲染是被测对象。
const listConversations = vi.fn();
const createConversation = vi.fn();
const appendMessage = vi.fn();
const listMessages = vi.fn();
const submitInteraction = vi.fn();

vi.mock("@/api/conversations", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/conversations")>();
  return {
    ...actual,
    listConversations: (...args: unknown[]) => listConversations(...args),
    createConversation: (...args: unknown[]) => createConversation(...args),
    appendMessage: (...args: unknown[]) => appendMessage(...args),
    listMessages: (...args: unknown[]) => listMessages(...args),
    submitInteraction: (...args: unknown[]) => submitInteraction(...args)
  };
});

// DK-F 访谈段接线：页面经 @/api/discovery 读 discovery 段（详情），
// 这里 mock IO，断言访谈分隔条/访谈时间线/待办块的呈现。
const getDiscoverySession = vi.fn();

vi.mock("@/api/discovery", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/discovery")>();
  return {
    ...actual,
    getDiscoverySession: (...args: unknown[]) => getDiscoverySession(...args)
  };
});

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

/** 与实现一致（4s 定时增量轮询）；fake timers 按此推进。 */
const POLL_INTERVAL_MS = 4000;

function conversation(over: Partial<Conversation>): Conversation {
  return {
    id: "c1",
    tenant_id: "t1",
    project_id: "p1",
    principal_id: "prc-1",
    title: "Iteration risks",
    status: "active",
    created_at: "2026-11-08T09:00:00Z",
    updated_at: "2026-11-08T10:00:00Z",
    last_message_at: "2026-11-08T10:00:00Z",
    discovery_session_id: null,
    ...over
  };
}

function record(
  over: Partial<ConversationMessageRecord>
): ConversationMessageRecord {
  return {
    id: "m1",
    conversation_id: "c1",
    role: "user",
    content: "latest risks",
    content_type: "markdown",
    metadata: {},
    created_at: "2026-11-08T09:59:00Z",
    ...over
  };
}

/** DK-F 访谈段夹具：discovery 会话详情（messages/findings/questions）。 */
function discoveryDetail(
  over: Partial<DiscoverySessionDetail>
): DiscoverySessionDetail {
  return {
    session: {
      id: "s1",
      project_id: "p1",
      source: "WEB",
      kind: "TASK_DISCOVERY",
      status: "NEEDS_USER_INPUT",
      created_by: "prc-1",
      created_at: "2026-11-08T10:05:00Z",
      updated_at: "2026-11-08T10:06:00Z"
    },
    messages: [
      {
        id: "d1",
        session_id: "s1",
        speaker: "user",
        source: "WEB",
        content: "本项目需要手机登录",
        seq: 1,
        created_at: "2026-11-08T10:05:00Z"
      },
      {
        id: "d2",
        session_id: "s1",
        speaker: "ai",
        source: "SYSTEM",
        content: "收到，先澄清验收口径。",
        seq: 2,
        created_at: "2026-11-08T10:05:30Z"
      }
    ],
    findings: [
      {
        id: "f1",
        session_id: "s1",
        kind: "user_confirmed",
        content: "需要手机号登录",
        confidence: 0.9,
        verification_status: "CONFIRMED",
        evidence: [],
        actor_role: "slot:core_behavior",
        repository_ids: [],
        created_at: "2026-11-08T10:05:30Z"
      }
    ],
    questions: [
      {
        id: "q1",
        session_id: "s1",
        finding_id: "f1",
        question: "哪些端需要手机登录？",
        why_blocking: "不确认会导致验收范围缺失",
        status: "open",
        created_at: "2026-11-08T10:05:30Z",
        answers: []
      },
      {
        id: "q2",
        session_id: "s1",
        finding_id: null,
        question:
          "[更正待知识应用] 项目知识（KB digest / PC 对象）：认证方式由 JWT 更正为 session cookie。",
        why_blocking: null,
        status: "deferred",
        created_at: "2026-11-08T10:06:00Z",
        answers: []
      }
    ],
    proposals: [],
    ...over
  };
}

// 卸载统一收口：onUnmounted 会停轮询链，避免残留真实定时器跨用例开火。
const mountedWrappers: VueWrapper[] = [];

async function mountPage() {
  const mounted = mountShell(AssistantView);
  mountedWrappers.push(mounted.wrapper);
  await mounted.router.push("/project/assistant?project_id=p1");
  await flushPromises();
  return mounted;
}

// Select 走 Teleport 浮层，用例间清场防 DOM 残留互染。
afterEach(() => {
  for (const wrapper of mountedWrappers.splice(0)) {
    try {
      wrapper.unmount();
    } catch {
      // 用例内已显式 unmount 的包装器忽略二次卸载。
    }
  }
  document.body.innerHTML = "";
});

describe("AssistantView（会话接线）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // AI 文本 Markdown 渲染：给确定性产物（原文仍在 md2html 入参里断言）
    md2html.mockReset();
    md2html.mockResolvedValue("<p>md-rendered</p>");
    listConversations.mockResolvedValue([
      conversation({}),
      conversation({ id: "c2", title: "", updated_at: "2026-11-08T09:00:00Z" })
    ]);
    listMessages.mockImplementation(async (_p: string, cid: string) =>
      cid === "c1"
        ? [
            // 服务端契约：(created_at, id) ASC —— 时间线末条即最新消息。
            record({ id: "m0", created_at: "2026-11-08T09:50:00Z" }),
            record({})
          ]
        : []
    );
    createConversation.mockResolvedValue(
      conversation({ id: "nc1", title: "first message" })
    );
    getDiscoverySession.mockResolvedValue(discoveryDetail({}));
    appendMessage.mockImplementation(
      async (_p: string, cid: string, body: { content: string }) =>
        record({
          id: `m-${body.content.slice(0, 6)}`,
          conversation_id: cid,
          content: body.content,
          created_at: "2026-11-08T10:05:00Z"
        })
    );
  });

  it("进入页面：拉会话列表、自动选中最近会话并按 ASC 渲染消息", async () => {
    const { wrapper } = await mountPage();

    expect(listConversations).toHaveBeenCalledWith("p1");
    // Batch H 首屏：最新窗口（latest=true, limit=200），不再是旧 500 条首屏。
    expect(listMessages).toHaveBeenCalledWith("p1", "c1", {
      latest: true,
      limit: 200
    });

    // 切换器：updated_at DESC 首项默认选中，title 为空回落未命名
    const select = wrapper.findComponent(Select);
    expect(select.exists()).toBe(true);
    expect(select.props("modelValue")).toBe("c1");
    const labels = (select.props("options") as { label: string }[]).map(
      option => option.label
    );
    expect(labels).toEqual(["Iteration risks", t("assistant.untitled")]);

    // 时间线：c1 的两条 user 消息（mock 未含 assistant，AI 侧为 0）
    const bubbles = wrapper.findAll('[data-role="user"]');
    expect(bubbles).toHaveLength(2);
    expect(wrapper.text()).toContain("latest risks");
    expect(wrapper.findAll('[data-role="ai"]')).toHaveLength(0);
    // 常驻提示：真实通道语义（不再是「通道尚未开通」）
    expect(wrapper.text()).toContain(t("assistant.recordedNotice"));
    expect(wrapper.text()).not.toContain("尚未开通");
  });

  it("发送：Composer → POST(201) 后本地追加 user 消息，并立即增量轮询", async () => {
    const { wrapper } = await mountPage();

    const textarea = wrapper.find("textarea");
    await textarea.setValue("check forest render risk");
    const sendButton = wrapper
      .findAll("button")
      .find(
        node =>
          node.attributes("aria-label") === t("ai.conversation.composer.send")
      );
    await sendButton?.trigger("click");
    await flushPromises();

    expect(appendMessage).toHaveBeenCalledTimes(1);
    expect(appendMessage).toHaveBeenCalledWith("p1", "c1", {
      content: "check forest render risk"
    });

    // 发送成功立即触发一次增量轮询：游标 = 刚追加的 user 消息
    //（mock id = `m-${content.slice(0, 6)}` = "m-check "，带尾随空格）
    expect(listMessages).toHaveBeenLastCalledWith("p1", "c1", {
      after: "2026-11-08T10:05:00Z",
      after_id: "m-check ",
      limit: 500
    });

    // 时间线追加该 user 消息；本轮增量与已有记录去重，AI 侧仍为 0
    const bubbles = wrapper.findAll('[data-role="user"]');
    expect(bubbles).toHaveLength(3);
    expect(wrapper.text()).toContain("check forest render risk");
    expect(wrapper.findAll('[data-role="ai"]')).toHaveLength(0);
    // 发送行同步到列表首位（updated_at DESC）
    const select = wrapper.findComponent(Select);
    expect((select.props("options") as { value: string }[])[0].value).toBe(
      "c1"
    );
  });

  it("轮询：4s 定时以 after/after_id 拉增量，assistant 消息渲染到 AI 侧", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      const { wrapper } = await mountPage();
      expect(listMessages).toHaveBeenCalledWith("p1", "c1", {
        latest: true,
        limit: 200
      });

      // 下一轮增量：worker 已把 assistant 回复落库
      listMessages.mockResolvedValue([
        record({
          id: "a1",
          role: "assistant",
          content: "AI 平台模型的回复",
          created_at: "2026-11-08T10:06:00Z"
        })
      ]);
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
      expect(listMessages).toHaveBeenLastCalledWith("p1", "c1", {
        after: "2026-11-08T09:59:00Z",
        after_id: "m1",
        limit: 500
      });
      await flushPromises();

      expect(wrapper.findAll('[data-role="ai"]')).toHaveLength(1);
      // AI 文本经 MarkdownViewer 渲染：原文送 md2html，渲染产物上屏
      expect(md2html).toHaveBeenCalledWith(
        "AI 平台模型的回复",
        expect.anything()
      );
      expect(wrapper.text()).toContain("md-rendered");
    } finally {
      vi.useRealTimers();
    }
  });

  it("轮询失败（429/网络）：静默保留旧态，下一轮自愈继续", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      const { wrapper } = await mountPage();

      listMessages.mockRejectedValue(new Error("429 too many requests"));
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
      await flushPromises();

      // 旧时间线原样保留；失败不进任何错误 UI
      expect(wrapper.findAll('[data-role="user"]')).toHaveLength(2);
      expect(wrapper.text()).not.toContain("429");
      expect(wrapper.text()).not.toContain(t("assistant.loadError"));

      // 下一轮恢复：增量照常追加
      listMessages.mockResolvedValue([
        record({
          id: "a2",
          role: "assistant",
          content: "迟到的回复",
          created_at: "2026-11-08T10:07:00Z"
        })
      ]);
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
      await flushPromises();
      expect(md2html).toHaveBeenCalledWith("迟到的回复", expect.anything());
      expect(wrapper.text()).toContain("md-rendered");
    } finally {
      vi.useRealTimers();
    }
  });

  it("卸载：轮询定时器被清理，不再发起请求", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      const { wrapper } = await mountPage();
      const calls = listMessages.mock.calls.length;

      wrapper.unmount();
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
      expect(listMessages.mock.calls.length).toBe(calls);
    } finally {
      vi.useRealTimers();
    }
  });

  it("空会话引导：首次发送自动建会话（title 取首条消息截断）再落消息", async () => {
    listConversations.mockResolvedValue([]);
    const { wrapper } = await mountPage();

    expect(wrapper.text()).toContain(t("assistant.emptyGuide"));
    expect(wrapper.findComponent(Select).exists()).toBe(false);

    const textarea = wrapper.find("textarea");
    await textarea.setValue("first message");
    const sendButton = wrapper
      .findAll("button")
      .find(
        node =>
          node.attributes("aria-label") === t("ai.conversation.composer.send")
      );
    await sendButton?.trigger("click");
    await flushPromises();

    expect(createConversation).toHaveBeenCalledTimes(1);
    expect(createConversation).toHaveBeenCalledWith("p1", {
      title: "first message"
    });
    expect(appendMessage).toHaveBeenCalledWith("p1", "nc1", {
      content: "first message"
    });
    // 建会话后切换器出现，引导条消失
    const select = wrapper.findComponent(Select);
    expect(select.exists()).toBe(true);
    expect(
      (select.props("options") as { label: string }[]).map(o => o.label)
    ).toContain("first message");
    expect(wrapper.text()).not.toContain(t("assistant.emptyGuide"));
    expect(wrapper.text()).toContain("first message");
  });

  it("发送失败：错误态 + 保留原文可重发，不静默丢字", async () => {
    appendMessage.mockRejectedValueOnce(new Error("network down"));
    const { wrapper } = await mountPage();

    const textarea = wrapper.find("textarea");
    await textarea.setValue("resend me");
    const sendButton = wrapper
      .findAll("button")
      .find(
        node =>
          node.attributes("aria-label") === t("ai.conversation.composer.send")
      );
    await sendButton?.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain(t("assistant.sendError"));
    expect(wrapper.text()).toContain("network down");

    // 重发：原文原样再投递一次，成功后错误态消失、消息落时间线
    const resendButton = wrapper
      .findAll("button")
      .find(node => node.text().includes(t("assistant.resend")));
    await resendButton?.trigger("click");
    await flushPromises();

    expect(appendMessage).toHaveBeenCalledTimes(2);
    expect(appendMessage).toHaveBeenLastCalledWith("p1", "c1", {
      content: "resend me"
    });
    expect(wrapper.text()).not.toContain(t("assistant.sendError"));
    expect(wrapper.text()).toContain("resend me");
  });

  it("切换会话：加载目标会话消息并替换时间线，轮询游标随新时间线重置", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      const { wrapper } = await mountPage();

      wrapper.findComponent(Select).vm.$emit("update:modelValue", "c2");
      await flushPromises();

      expect(listMessages).toHaveBeenCalledWith("p1", "c2", {
        latest: true,
        limit: 200
      });
      // c2 无消息：时间线为空，无伪造内容
      expect(wrapper.findAll(".ai-conversation-message")).toHaveLength(0);
      expect(wrapper.text()).toContain(t("assistant.recordedNotice"));

      // 新时间线的第一轮增量：c2 时间线为空 → 退化为与首屏一致的最新窗口
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
      expect(listMessages).toHaveBeenLastCalledWith("p1", "c2", {
        latest: true,
        limit: 200
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it("无项目上下文：空态引导，不发起请求", async () => {
    const mounted = mountShell(AssistantView);
    mountedWrappers.push(mounted.wrapper);
    await flushPromises();

    expect(mounted.wrapper.text()).toContain(t("assistant.noProjectTitle"));
    expect(listConversations).not.toHaveBeenCalled();
    expect(mounted.wrapper.findComponent(Select).exists()).toBe(false);
  });
  it("指令栏交接：待发提问发到当前会话；消费即清空，同文案可重复触发", async () => {
    const { wrapper } = await mountPage();
    const shell = useShellStore();

    shell.queueAssistantPrompt("梳理当前迭代风险");
    await flushPromises();

    // 已有会话（自动选中 c1）：直接追加，不新建
    expect(createConversation).not.toHaveBeenCalled();
    expect(appendMessage).toHaveBeenCalledWith("p1", "c1", {
      content: "梳理当前迭代风险"
    });
    expect(wrapper.text()).toContain("梳理当前迭代风险");
    // 消费即清空，不重复发送
    expect(shell.pendingAssistantPrompt).toBeNull();
    expect(appendMessage).toHaveBeenCalledTimes(1);

    // 槽为对象：相同文案的第二次提交仍会触发发送
    shell.queueAssistantPrompt("梳理当前迭代风险");
    await flushPromises();
    expect(appendMessage).toHaveBeenCalledTimes(2);
  });

  it("交接快照：指令栏提交时的项目决定发送落点（提交后切项目不漂移）", async () => {
    const { wrapper } = await mountPage();
    const shell = useShellStore();
    shell.setProjects([
      { id: "p1", name: "Atlas Platform" },
      { id: "p2", name: "Research Sandbox" }
    ]);
    shell.setCurrentProject("p1");

    // 提交时刻定格快照：项目 p1
    shell.queueAssistantPrompt("梳理当前迭代风险");
    // 交接后立刻切到 p2——快照不应漂移
    await shell.switchProject("p2");
    await flushPromises();

    expect(appendMessage).toHaveBeenCalledWith("p1", "c1", {
      content: "梳理当前迭代风险"
    });
    expect(wrapper.text()).toContain("梳理当前迭代风险");
  });

  it("交接快照：无项目上下文时不落到哨兵项目", async () => {
    await mountPage();
    const shell = useShellStore();

    shell.queueAssistantPrompt("没有项目上下文");
    await flushPromises();

    // 快照 projectId 为 null → 回落到路由上下文 p1，不落哨兵
    expect(appendMessage).toHaveBeenCalledWith("p1", "c1", {
      content: "没有项目上下文"
    });
  });

  it("Batch 2：新建会话时随 body 带上上下文锚点", async () => {
    // 无既有会话 → 首条消息经自动建会话（createConversation 带锚点）。
    // mock 须在挂载前设定：mountPage 内即拉会话列表。
    listConversations.mockResolvedValue([]);

    const { wrapper } = await mountPage();
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };
    await flushPromises();

    shell.queueAssistantPrompt("梳理当前迭代风险");
    await flushPromises();

    expect(createConversation).toHaveBeenCalledWith("p1", {
      title: "梳理当前迭代风险",
      context: {
        project_id: "p1",
        project_name: "Atlas Platform",
        repository_name: "atlas-platform",
        default_branch: "main",
        repository_role: "main"
      }
    });
    expect(wrapper.text()).toContain("梳理当前迭代风险");
  });

  it("新建会话按钮：进入草稿态不落库，首条消息经自动建会话落新会话", async () => {
    const { wrapper } = await mountPage();

    // 新建按钮带专属 class：Select 触发器也是 <button> 且占位文案同为
    // 「新建会话」，按文案匹配会串（故不能按文案定位）。
    const newButton = wrapper.find("button.assistant-new");
    expect(newButton.exists()).toBe(true);
    await newButton.trigger("click");

    // 草稿态：时间线清空、选择器归空（placeholder=新建会话）、按钮隐没
    expect(wrapper.findAll(".ai-conversation-message")).toHaveLength(0);
    const select = wrapper.findComponent(Select);
    expect(select.props("modelValue")).toBe("");
    expect(select.props("placeholder")).toBe(t("assistant.newPlaceholder"));
    expect(wrapper.find("button.assistant-new").exists()).toBe(false);

    // 首条消息：自动建会话（title 取首条截断）再落库，草稿态解除
    const textarea = wrapper.find("textarea");
    await textarea.setValue("brand new thread");
    const sendButton = wrapper
      .findAll("button")
      .find(
        node =>
          node.attributes("aria-label") === t("ai.conversation.composer.send")
      );
    await sendButton?.trigger("click");
    await flushPromises();

    expect(createConversation).toHaveBeenCalledTimes(1);
    expect(createConversation).toHaveBeenCalledWith("p1", {
      title: "brand new thread"
    });
    expect(appendMessage).toHaveBeenCalledWith("p1", "nc1", {
      content: "brand new thread"
    });
    expect(
      (wrapper.findComponent(Select).props("options") as { value: string }[])[0]
        .value
    ).toBe("nc1");
  });

  // ────────────────────────────── DK-F 访谈段接线（§2.5 体例）──────────────

  it("DK-F 落点：landed=interview 时消息不进 chat 时间线，访谈段分隔条与待办出现", async () => {
    const { wrapper } = await mountPage();

    // 发送落点 = 升级为访谈（服务端回包带 landed/discovery_session_id/handoff）。
    appendMessage.mockResolvedValueOnce({
      ...record({ id: "m-int", content: "本项目需要手机登录" }),
      landed: "interview",
      discovery_session_id: "s1",
      handoff: {
        boundary_message_id: "bm1",
        discovery_session_id: "s1",
        trigger: "implicit_intent",
        intent: "requirement_intake",
        created_session: true,
        notice: "已切换到需求澄清",
        undo_until: "2026-11-08T10:35:00Z"
      }
    });

    const textarea = wrapper.find("textarea");
    await textarea.setValue("本项目需要手机登录");
    const sendButton = wrapper
      .findAll("button")
      .find(
        node =>
          node.attributes("aria-label") === t("ai.conversation.composer.send")
      );
    await sendButton?.trigger("click");
    await flushPromises();

    // chat 面板时间线不含该消息（落 discovery 段；访谈段里的同文本是
    // discovery_message 的合法呈现，作用域断言分开）。
    const panel = wrapper.find(".ai-conversation");
    expect(panel.text()).not.toContain("本项目需要手机登录");
    // 边界分隔条 = 服务端提示（handoff.notice），非 AI 气泡
    expect(wrapper.text()).toContain("已切换到需求澄清");
    // 访谈段时间线（discovery_message 映射）：user 消息为原样气泡，
    // AI 消息经 MarkdownViewer 渲染（原文进 md2html，产物上屏）。
    expect(getDiscoverySession).toHaveBeenCalledWith("p1", "s1");
    expect(md2html).toHaveBeenCalledWith("收到，先澄清验收口径。", expect.anything());
    expect(wrapper.text()).toContain("md-rendered");
    // 待办块：待答问题（含 why_blocking 提示）
    expect(wrapper.text()).toContain(t("assistant.todoTitle"));
    expect(wrapper.text()).toContain("哪些端需要手机登录？");
    expect(wrapper.text()).toContain("不确认会导致验收范围缺失");
  });

  it("DK-F 边界消息：system 行渲染为分隔条上下文，不渲染为 AI 气泡", async () => {
    // 会话行已带链接（升级过的线程重进）：首屏含一条 system 边界消息。
    listMessages.mockImplementation(async (_p: string, cid: string) =>
      cid === "c1"
        ? [
            record({ id: "m0", created_at: "2026-11-08T09:50:00Z" }),
            record({
              id: "bm1",
              role: "system",
              content: "已切换到需求澄清",
              created_at: "2026-11-08T09:55:00Z",
              metadata: {
                handoff: { discovery_session_id: "s1", trigger: "implicit_intent" }
              }
            })
          ]
        : []
    );
    listConversations.mockResolvedValue([
      conversation({ discovery_session_id: "s1" })
    ]);
    const { wrapper } = await mountPage();

    // system 行不进 chat 面板（作用域=面板）；访谈段里 discovery 的
    // user/ai 消息各自渲染，面板内只见 m0 一条用户消息。
    const panel = wrapper.find(".ai-conversation");
    expect(panel.findAll('[data-role="user"]')).toHaveLength(1);
    expect(panel.findAll('[data-role="ai"]')).toHaveLength(0);
    // 链接存在 → 分隔条 + 访谈段 + 待办块可见
    expect(wrapper.text()).toContain(t("assistant.interviewNotice"));
    expect(wrapper.text()).toContain("[更正待知识应用]");
    expect(wrapper.text()).toContain(t("assistant.pendingKnowledge"));
    // 已确认事实入待办
    expect(wrapper.text()).toContain(t("assistant.confirmedFacts"));
    expect(wrapper.text()).toContain("需要手机号登录");
  });

  it("DK-F 未链接线程：不拉访谈段，时间线与既有行为完全一致", async () => {
    const { wrapper } = await mountPage();

    expect(getDiscoverySession).not.toHaveBeenCalled();
    expect(wrapper.find(".assistant-interview").exists()).toBe(false);
    expect(wrapper.text()).not.toContain(t("assistant.interviewNotice"));
    // 既有时间线不受影响
    expect(wrapper.findAll('[data-role="user"]')).toHaveLength(2);
  });
});

// ── Batch 3B：响应卡信封消费（metadata.envelope → interaction）─────────

describe("AssistantView（信封消费）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    md2html.mockReset();
    md2html.mockResolvedValue("<p>md-rendered</p>");
    listConversations.mockResolvedValue([conversation({})]);
    createConversation.mockResolvedValue(
      conversation({ id: "nc1", title: "first message" })
    );
    appendMessage.mockImplementation(
      async (_p: string, cid: string, body: { content: string }) =>
        record({
          id: `m-${body.content.slice(0, 6)}`,
          conversation_id: cid,
          content: body.content,
          created_at: "2026-11-08T10:05:00Z"
        })
    );
  });

  /** 合法信封 = payload 满足前端 RecommendationPayload 必填全集（与后端同源）。 */
  function envelopeFixture(over: Record<string, unknown> = {}) {
    return {
      id: "env-1",
      type: "recommendation",
      status: "presented",
      identity: { name: "AI-PM", role: "研发负责人" },
      createdAt: "2026-11-08T09:59:30Z",
      title: "建议拆分任务",
      payload: {
        summary: "当前迭代风险集中，建议拆分",
        explanationMarkdown: "依据风险分布……",
        benefits: [{ label: "风险可见", detail: "逐项暴露" }],
        risks: [{ label: "迁移成本" }]
      },
      ...over
    };
  }

  it("合法信封：AI 消息内联渲染响应卡（type/status/payload 透传）", async () => {
    listMessages.mockResolvedValue([
      record({
        id: "m-card",
        role: "assistant",
        content: "卡片消息",
        metadata: { envelope: envelopeFixture() }
      })
    ]);
    const { wrapper } = await mountPage();
    await flushPromises();

    // class 透传：__card 与 ai-card 同落一个根元素（复合选择器，非后代）。
    const card = wrapper.find(".ai-conversation-message__card.ai-card");
    expect(card.exists()).toBe(true);
    expect(card.attributes("data-type")).toBe("recommendation");
  });

  it("白名单外类型：fail-closed 退化为纯文本，不伪造卡片", async () => {
    listMessages.mockResolvedValue([
      record({
        id: "m-bad-type",
        role: "assistant",
        content: "坏类型消息",
        metadata: { envelope: envelopeFixture({ type: "chat-message" }) }
      })
    ]);
    const { wrapper } = await mountPage();
    await flushPromises();

    expect(wrapper.find(".ai-conversation-message__card").exists()).toBe(false);
    // 内容仍以纯文本（Markdown 链）渲染——md2html 收到原文即证明未丢字。
    expect(md2html).toHaveBeenCalledWith("坏类型消息", expect.anything());
  });

  it("payload 缺必填字段：退化为纯文本（fail-closed）", async () => {
    listMessages.mockResolvedValue([
      record({
        id: "m-bad-payload",
        role: "assistant",
        content: "坏载荷消息",
        metadata: { envelope: envelopeFixture({ payload: {} }) }
      })
    ]);
    const { wrapper } = await mountPage();
    await flushPromises();

    expect(wrapper.find(".ai-conversation-message__card").exists()).toBe(false);
    expect(md2html).toHaveBeenCalledWith("坏载荷消息", expect.anything());
  });

  it("无信封 metadata：旧行为不变（纯 Markdown，零卡片）", async () => {
    listMessages.mockResolvedValue([record({})]);
    const { wrapper } = await mountPage();
    await flushPromises();

    expect(wrapper.find(".ai-conversation-message__card").exists()).toBe(false);
    expect(wrapper.text()).toContain("latest risks");
  });
});

// ── Batch 3C：卡片真提交接线（卡动作 → 写回端点 + 本地终态）──────────

describe("AssistantView（交互写回接线）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    md2html.mockReset();
    md2html.mockResolvedValue("<p>md-rendered</p>");
    listConversations.mockResolvedValue([conversation({})]);
    createConversation.mockResolvedValue(
      conversation({ id: "nc1", title: "first message" })
    );
    appendMessage.mockImplementation(
      async (_p: string, cid: string, body: { content: string }) =>
        record({
          id: `m-${body.content.slice(0, 6)}`,
          conversation_id: cid,
          content: body.content,
          created_at: "2026-11-08T10:05:00Z"
        })
    );
  });

  function cardEnvelope() {
    return {
      id: "env-1",
      type: "recommendation",
      status: "presented",
      identity: { name: "AI-PM", role: "研发负责人" },
      createdAt: "2026-11-08T09:59:30Z",
      title: "建议拆分任务",
      payload: {
        summary: "当前迭代风险集中，建议拆分",
        explanationMarkdown: "依据风险分布……",
        benefits: [{ label: "风险可见", detail: "逐项暴露" }],
        risks: [{ label: "迁移成本" }]
      }
    };
  }

  it("点卡「接收」→ 写回端点收 accept + 记录号回传 + 信封本地终态", async () => {
    listMessages.mockResolvedValue([
      record({
        id: "m-card",
        role: "assistant",
        content: "卡片消息",
        metadata: { envelope: cardEnvelope() }
      })
    ]);
    submitInteraction.mockResolvedValue({
      id: "it-1",
      message_id: "m-card",
      conversation_id: "c1",
      action: "accept",
      status: "accepted",
      decided_by: "prc",
      reference: "INT-AB12CD34",
      created_at: "2026-11-08T10:06:00Z",
      replayed: false
    });
    const { wrapper } = await mountPage();
    await flushPromises();

    const accept = wrapper
      .findAll("button")
      .find(node => node.text().includes("接收"));
    expect(accept).toBeTruthy();
    await accept!.trigger("click");
    await flushPromises();

    expect(submitInteraction).toHaveBeenCalledWith("p1", "c1", "m-card", {
      action: "accept"
    });
    // 卡面记录号（真端点回传，非 mock INT-*）
    expect(wrapper.text()).toContain("INT-AB12CD34");
    // 信封本地终态化（时间线内同步，不等轮询）
    const card = wrapper.find(".ai-conversation-message__card.ai-card");
    expect(card.classes()).toContain("ai-card");
  });

  it("端点 4xx：错误上卡（错误相），不伪造成功", async () => {
    listMessages.mockResolvedValue([
      record({
        id: "m-card",
        role: "assistant",
        content: "卡片消息",
        metadata: { envelope: cardEnvelope() }
      })
    ]);
    submitInteraction.mockRejectedValue(
      new Error("interaction target not found")
    );
    const { wrapper } = await mountPage();
    await flushPromises();

    const accept = wrapper
      .findAll("button")
      .find(node => node.text().includes("接收"));
    await accept!.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("interaction target not found");
    expect(wrapper.text()).not.toContain("INT-");
  });
});
