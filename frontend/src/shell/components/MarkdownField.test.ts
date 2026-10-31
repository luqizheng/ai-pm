// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";

/**
 * 公共字段组件的行为契约。
 *
 * 这里不 mock 子组件：要证明的正是「调用方只给 v-model，附件链路就已经通了」，
 * 所以断言落在 Vditor 的构造参数与实际调用结果上。
 */
const fake = vi.hoisted(() => ({
  instances: [] as Array<{
    options: Record<string, unknown>;
    value: string;
    getValue: () => string;
  }>
}));

const ports = vi.hoisted(() => ({
  uploadAttachment: vi.fn(async () => ({ id: "f-1", name: "截图.png" })),
  resolveAttachmentUrl: vi.fn(async () => "https://signed/f-1")
}));

const md2html = vi.hoisted(() => vi.fn(async () => "<p>渲染结果</p>"));

vi.mock("@/shell/api/attachments", () => ports);

vi.mock("vditor", () => {
  class FakeVditor {
    static md2html = md2html;

    options: Record<string, unknown>;
    value: string;

    constructor(_host: HTMLElement, options: Record<string, unknown> = {}) {
      this.options = options;
      this.value = (options.value as string) ?? "";
      fake.instances.push(this as unknown as (typeof fake.instances)[number]);
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

    destroy(): void {}
  }

  return { default: FakeVditor };
});

import MarkdownField from "./MarkdownField.vue";

const wrappers: Array<{ unmount: () => void }> = [];

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  fake.instances.length = 0;
  md2html.mockClear();
  ports.uploadAttachment.mockClear();
  ports.resolveAttachmentUrl.mockClear();
  document.body.innerHTML = "";
});

function mountField(props: Record<string, unknown> = {}) {
  const wrapper = mountShell(MarkdownField, { props });
  wrappers.push(wrapper);
  return wrapper;
}

function uploadHandlerOf(index = 0) {
  const upload = fake.instances[index].options.upload as {
    handler?: (files: File[]) => Promise<unknown>;
  };
  return upload.handler;
}

describe("MarkdownField", () => {
  it("默认接入 file-services：粘贴的文件直接走后台上传并插入引用", async () => {
    mountField({ modelValue: "" });
    await nextTick();

    const handler = uploadHandlerOf();
    expect(handler).toBeTypeOf("function");

    const file = new File(["png"], "截图.png", { type: "image/png" });
    await handler?.([file]);

    expect(ports.uploadAttachment).toHaveBeenCalledWith(file);
    // 正文只落 `/v1/file/{id}` 引用，不落会过期的临时地址
    expect(fake.instances[0].value).toContain("/v1/file/f-1");
  });

  it("默认上限取自文件服务 multipart 的 100MiB", async () => {
    mountField();
    await nextTick();

    const file = new File(["x"], "big.bin", {
      type: "application/octet-stream"
    });
    Object.defineProperty(file, "size", { value: 100 * 1024 * 1024 + 1 });

    await uploadHandlerOf()?.([file]);

    expect(ports.uploadAttachment).not.toHaveBeenCalled();
    expect(fake.instances[0].value).toBe("");
  });

  it("view 模式改为静态渲染，不再创建编辑器实例", async () => {
    const wrapper = mountField({ mode: "view", modelValue: "正文内容" });
    await nextTick();
    await nextTick();

    expect(fake.instances).toHaveLength(0);
    expect(md2html).toHaveBeenCalledWith("正文内容", expect.anything());
    expect(wrapper.html()).toContain("渲染结果");
  });

  it("view 模式空正文显示空态文案而不是空白", async () => {
    const wrapper = mountField({ mode: "view", modelValue: "" });
    await nextTick();
    await nextTick();

    expect(wrapper.text()).toContain("暂无内容");
  });

  it("error 优先于 description 展示", async () => {
    const wrapper = mountField({
      mode: "view",
      description: "说明文案",
      error: "正文不能为空"
    });
    await nextTick();
    await nextTick();

    expect(wrapper.text()).toContain("正文不能为空");
    expect(wrapper.text()).not.toContain("说明文案");
  });

  it("disabled 透传为编辑器的只读态", async () => {
    const wrapper = mountField({ modelValue: "内容", disabled: true });
    await nextTick();

    const instance = fake.instances[0] as unknown as {
      options: Record<string, unknown>;
    };
    // MarkdownEditor 在 readonly 变化时会调用 disabled()，这里断言状态被传下去
    expect(instance.options.mode).toBe("wysiwyg");
    expect(wrapper.find(".md-editor--readonly").exists()).toBe(true);
  });

  it("编辑器输入向上冒泡成 update:modelValue", async () => {
    const wrapper = mountField({ modelValue: "" });
    await nextTick();

    const input = fake.instances[0].options.input as (value: string) => void;
    input("新增的文字");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["新增的文字"]);
  });
});
