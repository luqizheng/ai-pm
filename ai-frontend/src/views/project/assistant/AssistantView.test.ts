// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, type VueWrapper } from "@vue/test-utils";

import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import Select from "@/components/ui/select/Select.vue";
import type {
  Conversation,
  ConversationMessage as ConversationMessageRecord
} from "@/api/conversations";

import AssistantView from "./AssistantView.vue";

// 会话接线批（2026-11-08）+ Batch G 轮询批 + Batch H 首屏最新窗口：
// 页面数据一律来自 @/api/conversations（D-4 REST），这里只 mock IO 函数；
// 三件套组件为真实实现——发送链路、增量轮询、首屏 latest 参数与 assistant
// 渲染是被测对象。
const listConversations = vi.fn();
const createConversation = vi.fn();
const appendMessage = vi.fn();
const listMessages = vi.fn();

vi.mock("@/api/conversations", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/conversations")>();
  return {
    ...actual,
    listConversations: (...args: unknown[]) => listConversations(...args),
    createConversation: (...args: unknown[]) => createConversation(...args),
    appendMessage: (...args: unknown[]) => appendMessage(...args),
    listMessages: (...args: unknown[]) => listMessages(...args)
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
      expect(wrapper.text()).toContain("AI 平台模型的回复");
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
      expect(wrapper.text()).toContain("迟到的回复");
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
});
