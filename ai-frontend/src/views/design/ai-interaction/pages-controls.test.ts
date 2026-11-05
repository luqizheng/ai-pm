// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { nextTick } from "vue";

import { mountShell } from "@/lib/test-utils";
import { buildRoutes } from "@/router";

/** Vditor 替身：RichTextEditorControl 挂载真实 MarkdownEditor 时不触网。 */
vi.mock("vditor", () => {
  class FakeVditor {
    value: string;
    destroyed = false;
    constructor(_host: HTMLElement, options: Record<string, unknown> = {}) {
      this.value = (options.value as string) ?? "";
      setTimeout(() => {
        (options.after as (() => void) | undefined)?.();
      }, 0);
    }
    getValue(): string {
      return this.value;
    }
    setValue(markdown: string): void {
      this.value = markdown;
    }
    insertValue(markdown: string): void {
      this.value += markdown;
    }
    focus(): void {}
    blur(): void {}
    disabled(): void {}
    enable(): void {}
    isUploading(): boolean {
      return false;
    }
    destroy(): void {
      this.destroyed = true;
    }
  }
  return { default: FakeVditor };
});

import AiInteractionControlsView from "./AiInteractionControlsView.vue";

async function mountPage() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  await router.push("/design");
  await router.isReady();

  const wrapper = mountShell(AiInteractionControlsView, {
    global: { plugins: [router] }
  });
  await flushPromises();
  await nextTick();
  return wrapper;
}

describe("AiInteractionControlsView", () => {
  it("渲染五个控件族画框与全部控件样板", async () => {
    const wrapper = await mountPage();
    const text = wrapper.text();

    expect(text).toContain("Selection — 5 controls");
    expect(text).toContain("Feedback — 4 controls");
    expect(text).toContain("Content editing — 4 controls");
    expect(text).toContain("Decision — 3 controls");
    expect(text).toContain("Workflow — 4 controls");

    expect(wrapper.findAll(".design-specimen").length).toBeGreaterThanOrEqual(
      18
    );
  });

  it("提交成功态样板展示记录号与「不代替审批」提示", async () => {
    const wrapper = await mountPage();
    const text = wrapper.text();

    expect(text).toContain("INT-2041");
    expect(text).toContain("正式业务变更以审批结果为准");
  });

  it("单选控件可切换选中值（v-model 生效）", async () => {
    const wrapper = await mountPage();
    const radios = wrapper.findAll('input[type="radio"]') as unknown as Array<{
      element: HTMLInputElement;
      trigger: (event: string) => Promise<void>;
      setValue: (value: boolean) => Promise<void>;
    }>;

    expect(radios.length).toBeGreaterThan(0);
    await radios[radios.length - 1].setValue(true);
    await flushPromises();

    expect(
      (radios[radios.length - 1].element as HTMLInputElement).checked
    ).toBe(true);
  });
});
