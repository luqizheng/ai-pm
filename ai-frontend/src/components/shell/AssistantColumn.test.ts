// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, type VueWrapper } from "@vue/test-utils";

import { mountShell } from "@/components/shell/test-utils";
import { useShellStore } from "@/stores/shell";
import type {
  Conversation,
  ConversationMessage as ConversationMessageRecord
} from "@/api/conversations";

import AssistantColumn from "./AssistantColumn.vue";

/**
 * AI Chat 常驻列（DR-C8 第 ② 步）：与会话页共用会话机制（composable），
 * 列侧只负责窄列形态的组装 + 就地发送（不再入队 + 跳转会话页）。
 */

const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: { md2html, preview: vi.fn(), version: "test" }
}));

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

const mountedWrappers: VueWrapper[] = [];

async function mountColumn() {
  const mounted = mountShell(AssistantColumn);
  mountedWrappers.push(mounted.wrapper);
  await flushPromises();
  return mounted;
}

describe("AssistantColumn（AI Chat 常驻列）", () => {
  beforeEach(() => {
    localStorage.clear();
    listConversations.mockReset();
    createConversation.mockReset();
    appendMessage.mockReset();
    listMessages.mockReset();
    submitInteraction.mockReset();
    listConversations.mockResolvedValue([]);
    listMessages.mockResolvedValue([]);
    appendMessage.mockResolvedValue(record({ id: "m-sent" }));
  });

  afterEach(() => {
    mountedWrappers.splice(0).forEach(wrapper => wrapper.unmount());
  });

  /** 挂载后设置当前项目（列的项目落点 = shell 活态项目）。 */
  function bindProject(projectId: string | null) {
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Smark Platform" }]);
    shell.setCurrentProject(projectId);
  }

  it("按当前项目拉会话列表并渲染时间线", async () => {
    listConversations.mockResolvedValue([conversation({})]);
    listMessages.mockResolvedValue([
      record({ id: "m1", role: "user", content: "先看风险" }),
      record({
        id: "m2",
        role: "assistant",
        content: "风险有三处",
        created_at: "2026-11-08T10:00:00Z"
      })
    ]);

    const { wrapper } = await mountColumn();
    bindProject("p1");
    await flushPromises();

    expect(listConversations).toHaveBeenCalledWith("p1");
    const panel = wrapper.find(".ai-conversation");
    expect(panel.exists()).toBe(true);
    expect(panel.text()).toContain("先看风险");
    expect(panel.text()).toContain("风险有三处");
  });

  it("Composer 就地发送：不入队、不跳转会话页", async () => {
    listConversations.mockResolvedValue([conversation({})]);

    const { wrapper, router } = await mountColumn();
    bindProject("p1");
    await flushPromises();
    expect(listConversations).toHaveBeenCalledWith("p1");
    const pathBefore = router.currentRoute.value.path;

    const input = wrapper.find(".shell-commandbar__field");
    await input.setValue("这个需求还缺验收条件");
    const send = wrapper.find(".shell-commandbar__send");
    expect(send.exists()).toBe(true);
    await send.trigger("click");
    await flushPromises();

    // 发送落到会话机制（appendMessage），而不是「入队 + 跳会话页」
    expect(appendMessage).toHaveBeenCalledWith("p1", "c1", {
      content: "这个需求还缺验收条件"
    });
    expect(router.currentRoute.value.path).toBe(pathBefore);
  });

  it("无项目上下文时不发请求，给空态", async () => {
    const { wrapper } = await mountColumn();
    bindProject(null);
    await flushPromises();

    expect(listConversations).not.toHaveBeenCalled();
    expect(wrapper.find(".ui-empty").exists()).toBe(true);
  });
});
