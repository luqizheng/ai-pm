// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";
import Select from "@/components/ui/select/Select.vue";
import type {
  Conversation,
  ConversationMessage as ConversationMessageRecord
} from "@/api/conversations";

import AssistantView from "./AssistantView.vue";

// 会话接线批（2026-11-08）：页面数据一律来自 @/api/conversations（D-4 REST），
// 这里只 mock IO 函数；三件套组件为真实实现——发送链路与诚实空态是被测对象。
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

async function mountPage() {
  const mounted = mountShell(AssistantView);
  await mounted.router.push("/project/assistant?project_id=p1");
  await flushPromises();
  return mounted;
}

// Select 走 Teleport 浮层，用例间清场防 DOM 残留互染。
afterEach(() => {
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
        ? [record({}), record({ id: "m0", created_at: "2026-11-08T09:50:00Z" })]
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
    expect(listMessages).toHaveBeenCalledWith("p1", "c1", { limit: 500 });

    // 切换器：updated_at DESC 首项默认选中，title 为空回落未命名
    const select = wrapper.findComponent(Select);
    expect(select.exists()).toBe(true);
    expect(select.props("modelValue")).toBe("c1");
    const labels = (select.props("options") as { label: string }[]).map(
      option => option.label
    );
    expect(labels).toEqual(["Iteration risks", t("assistant.untitled")]);

    // 时间线：c1 的两条 user 消息（无任何 AI 消息被伪造）
    const bubbles = wrapper.findAll('[data-role="user"]');
    expect(bubbles).toHaveLength(2);
    expect(wrapper.text()).toContain("latest risks");
    expect(wrapper.findAll('[data-role="ai"]')).toHaveLength(0);
    // assistant 通道未开通：诚实提示行常驻
    expect(wrapper.text()).toContain(t("assistant.recordedNotice"));
  });

  it("发送：Composer → POST(201) 后本地追加，仅落 user 消息，不伪造 AI 回复", async () => {
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

    // 时间线仅追加该 user 消息；无任何 assistant/system 内容被伪造
    const bubbles = wrapper.findAll('[data-role="user"]');
    expect(bubbles).toHaveLength(3);
    expect(wrapper.text()).toContain("check forest render risk");
    expect(wrapper.findAll('[data-role="ai"]')).toHaveLength(0);
    expect(wrapper.text()).toContain(t("assistant.recordedNotice"));
    // 发送行同步到列表首位（updated_at DESC）
    const select = wrapper.findComponent(Select);
    expect((select.props("options") as { value: string }[])[0].value).toBe(
      "c1"
    );
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

  it("切换会话：加载目标会话消息并替换时间线", async () => {
    const { wrapper } = await mountPage();

    wrapper.findComponent(Select).vm.$emit("update:modelValue", "c2");
    await flushPromises();

    expect(listMessages).toHaveBeenCalledWith("p1", "c2", { limit: 500 });
    // c2 无消息：时间线为空，仍无伪造内容
    expect(wrapper.findAll(".ai-conversation-message")).toHaveLength(0);
    expect(wrapper.text()).toContain(t("assistant.recordedNotice"));
  });

  it("无项目上下文：空态引导，不发起请求", async () => {
    const mounted = mountShell(AssistantView);
    await flushPromises();

    expect(mounted.wrapper.text()).toContain(t("assistant.noProjectTitle"));
    expect(listConversations).not.toHaveBeenCalled();
    expect(mounted.wrapper.findComponent(Select).exists()).toBe(false);
  });
});
