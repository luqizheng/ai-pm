// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, type ComponentMountingOptions } from "@vue/test-utils";
import type { Component } from "vue";
import { nextTick } from "vue";
import { createMemoryHistory, createRouter, type Router } from "vue-router";

/** MarkdownViewer 依赖 Vditor：按 responses-a.test.ts 的做法整体替换 */
const md2html = vi.hoisted(() => vi.fn());

vi.mock("vditor", () => ({
  default: {
    md2html,
    preview: vi.fn(),
    version: "test"
  }
}));

import AiConversationPanel from "@/components/ui/ai-interaction/conversation/AiConversationPanel.vue";
import { conversationFixture } from "@/components/ui/ai-interaction/fixtures";
import { i18n } from "@/lib/i18n";
import { mountShell } from "@/lib/test-utils";
import AiInteractionConversationView from "./AiInteractionConversationView.vue";

/**
 * 会话集成（§7 面板 + 原型 A–H）行为用例：
 *   - 会话面板按消息角色区分用户 / AI，并内联渲染交互卡；
 *   - composer 空值禁用，输入后发送 emit send；
 *   - 目录页含 1440×900 壳画框与 A–H 原型帧；
 *   - 原型 G 的高后果确认弹层可触发。
 */

const mountedWrappers: Array<{ unmount: () => void }> = [];

function createTestRouter(): Router {
  return createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/:pathMatch(.*)*", component: { template: "<div />" } }]
  });
}

function mountWithShell<T extends Component>(
  component: T,
  options: ComponentMountingOptions<T> = {} as ComponentMountingOptions<T>
) {
  const router = createTestRouter();
  const wrapper = mountShell(component, {
    ...options,
    global: {
      ...options.global,
      plugins: [router]
    }
  } as ComponentMountingOptions<T>);
  mountedWrappers.push(wrapper);
  return wrapper;
}

async function flush(): Promise<void> {
  await flushPromises();
  for (let index = 0; index < 3; index += 1) await nextTick();
  await flushPromises();
}

beforeEach(() => {
  md2html.mockResolvedValue("<p>body</p>");
});

afterEach(() => {
  while (mountedWrappers.length) mountedWrappers.pop()?.unmount();
  md2html.mockReset();
  document.body.innerHTML = "";
});

describe("AiConversationPanel", () => {
  it("渲染 conversationFixture：用户消息、AI 普通消息与内联交互卡", async () => {
    const wrapper = mountWithShell(AiConversationPanel, {
      props: { messages: conversationFixture }
    });
    await flush();

    expect(wrapper.findAll(".ai-conversation-message.is-user").length).toBe(3);
    expect(wrapper.findAll(".ai-conversation-message.is-ai").length).toBe(4);

    const userMessage = conversationFixture[0];
    const aiMessage = conversationFixture[1];
    // 用户消息原文直出；AI 文本经 MarkdownViewer 渲染（md2html mock 返回
    // "<p>body</p>"），原文送渲染、渲染结果上屏。
    expect(wrapper.text()).toContain(userMessage.text);
    expect(md2html).toHaveBeenCalledWith(aiMessage.text, expect.anything());
    expect(wrapper.text()).toContain("body");

    // 轻量 AI 消息带头像章；结构化交互内联为响应卡
    expect(
      wrapper.findAll(".ai-conversation-message__identity").length
    ).toBeGreaterThan(0);
    const cards = wrapper.findAll(".ai-card");
    expect(cards.length).toBeGreaterThanOrEqual(1);
    expect(cards[0].attributes("data-type")).toBe("recommendation");
  });

  it("composer 空值禁用，输入后发送 emit send 并清空草稿", async () => {
    const wrapper = mountWithShell(AiConversationPanel, {
      props: { messages: conversationFixture }
    });
    await flush();

    const send = () => wrapper.find(".ai-conversation-composer__send");
    expect(send().attributes("disabled")).toBeDefined();

    await wrapper
      .find(".ai-conversation-composer textarea")
      .setValue("  继续检查地形渲染  ");
    await flush();
    expect(send().attributes("disabled")).toBeUndefined();

    await send().trigger("click");
    await flush();

    expect(wrapper.emitted("send")).toBeTruthy();
    expect(wrapper.emitted("send")?.[0]).toEqual(["继续检查地形渲染"]);
    expect(
      (
        wrapper.find(".ai-conversation-composer textarea")
          .element as HTMLTextAreaElement
      ).value
    ).toBe("");
  });
});

describe("AiInteractionConversationView", () => {
  it("挂载成功：1440×900 壳画框与 A–H 原型帧都在", async () => {
    const wrapper = mountWithShell(AiInteractionConversationView);
    await flush();

    expect(wrapper.find(".conversation-frame__grid").exists()).toBe(true);
    expect(wrapper.find(".workspace-topbar").exists()).toBe(true);
    expect(wrapper.find(".workspace-leftnav").exists()).toBe(true);
    expect(wrapper.find(".workspace-main").exists()).toBe(true);
    expect(wrapper.find(".workspace-context").exists()).toBe(true);
    expect(wrapper.find(".workspace-commandbar").exists()).toBe(true);
    expect(wrapper.find(".ai-conversation").exists()).toBe(true);

    const labels = wrapper
      .findAll(".design-frame__label")
      .map(node => node.text());
    for (const letter of ["A", "B", "C", "D", "E", "F", "G", "H"]) {
      expect(
        labels.some(label => label.startsWith(`${letter} —`)),
        `missing prototype frame ${letter}`
      ).toBe(true);
    }
  });

  it("原型 G：点击确认后弹出高后果操作确认层", async () => {
    const wrapper = mountWithShell(AiInteractionConversationView);
    await flush();

    const frame = wrapper.find('[data-prototype="G"]');
    expect(frame.exists()).toBe(true);

    const confirmLabel = i18n.global.t("ai.actionRequest.actions.confirm");
    const confirmButton = frame
      .findAll(".ai-card__footer button")
      .find(button => button.text().trim() === confirmLabel);
    expect(confirmButton).toBeTruthy();

    await confirmButton!.trigger("click");
    await nextTick();

    expect(document.body.querySelector(".ui-confirm")).not.toBeNull();
  });
});
