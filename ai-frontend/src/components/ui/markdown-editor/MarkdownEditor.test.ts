// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/lib/test-utils";

/** 夹具：记录构造参数与生命周期调用，替代真实 Vditor */
const fake = vi.hoisted(() => {
  interface Recorded {
    options: Record<string, unknown>;
    value: string;
    destroyed: boolean;
    setValueCalls: string[];
  }
  return { instances: [] as Recorded[] };
});

vi.mock("vditor", () => {
  class FakeVditor {
    options: Record<string, unknown>;
    value: string;
    destroyed = false;
    setValueCalls: string[] = [];

    constructor(_host: HTMLElement, options: Record<string, unknown> = {}) {
      this.options = options;
      this.value = (options.value as string) ?? "";
      fake.instances.push(this);
      setTimeout(() => {
        (options.after as (() => void) | undefined)?.();
      }, 0);
    }

    getValue(): string {
      return this.value;
    }

    setValue(markdown: string): void {
      this.value = markdown;
      this.setValueCalls.push(markdown);
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

import MarkdownEditor from "./MarkdownEditor.vue";

const wrappers: Array<{ unmount: () => void }> = [];

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  fake.instances.length = 0;
  document.body.innerHTML = "";
});

function mountEditor(props: Record<string, unknown> = {}) {
  const wrapper = mountShell(MarkdownEditor, { props });
  wrappers.push(wrapper);
  return wrapper;
}

describe("MarkdownEditor", () => {
  it("以富文本模式挂载并透传初始内容与禁用缓存", async () => {
    mountEditor({ modelValue: "# 标题" });
    await nextTick();

    const instance = fake.instances[0];
    expect(instance).toBeDefined();
    expect(instance.options.mode).toBe("wysiwyg");
    expect(instance.options.value).toBe("# 标题");
    // 缓存必须关闭，否则 localStorage 草稿会和外部 v-model 抢真源
    expect(instance.options.cache).toEqual({ enable: false });
  });

  it("挂载完成后抛出 ready 事件", async () => {
    const wrapper = mountEditor({ modelValue: "内容" });
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(wrapper.emitted("ready")).toHaveLength(1);
  });

  it("外部改写 modelValue 时同步进编辑器", async () => {
    const wrapper = mountEditor({ modelValue: "旧内容" });
    await nextTick();

    await wrapper.setProps({ modelValue: "新内容" });

    const instance = fake.instances[0];
    expect(instance.setValueCalls).toEqual(["新内容"]);
  });

  it("值未变化时不打断输入", async () => {
    const wrapper = mountEditor({ modelValue: "同一份内容" });
    await nextTick();

    await wrapper.setProps({ modelValue: "同一份内容" });

    expect(fake.instances[0].setValueCalls).toEqual([]);
  });

  it("卸载时销毁编辑器实例", async () => {
    const wrapper = mountEditor({ modelValue: "" });
    await nextTick();

    const instance = fake.instances[0];
    wrapper.unmount();

    expect(instance.destroyed).toBe(true);
  });

  it("未注入上传端口时不配置上传回调", async () => {
    mountEditor();
    await nextTick();

    const upload = fake.instances[0].options.upload as {
      handler?: unknown;
    };
    expect(upload.handler).toBeUndefined();
  });

  it("注入上传端口后把 handler 交给 Vditor", async () => {
    mountEditor({ uploader: async () => ({ id: "i1", name: "a.png" }) });
    await nextTick();

    const upload = fake.instances[0].options.upload as {
      handler?: unknown;
    };
    expect(typeof upload.handler).toBe("function");
  });
});
