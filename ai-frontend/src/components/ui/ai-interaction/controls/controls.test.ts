// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Component } from "vue";
import { nextTick } from "vue";
import type { ComponentMountingOptions, VueWrapper } from "@vue/test-utils";

/**
 * Vditor is a browser-only editor; the rich-text control test only needs the
 * wrapper contract (value in / update out), so the real editor is stubbed.
 */
const fake = vi.hoisted(() => {
  interface Recorded {
    options: Record<string, unknown>;
    value: string;
  }
  return { instances: [] as Recorded[] };
});

vi.mock("vditor", () => {
  class FakeVditor {
    options: Record<string, unknown>;
    value: string;

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

import { mountShell } from "@/lib/test-utils";
import SingleSelectControl from "./SingleSelectControl.vue";
import MultiSelectControl from "./MultiSelectControl.vue";
import CheckboxGroupControl from "./CheckboxGroupControl.vue";
import RadioGroupControl from "./RadioGroupControl.vue";
import SelectDropdownControl from "./SelectDropdownControl.vue";
import TextFeedbackControl from "./TextFeedbackControl.vue";
import ReasonInputControl from "./ReasonInputControl.vue";
import RatingControl from "./RatingControl.vue";
import HelpfulControl from "./HelpfulControl.vue";
import InlineEditor from "./InlineEditor.vue";
import RichTextEditorControl from "./RichTextEditorControl.vue";
import DiffViewer from "./DiffViewer.vue";
import EditAcceptControl from "./EditAcceptControl.vue";
import AcceptRejectControl from "./AcceptRejectControl.vue";
import DecisionOptionSelector from "./DecisionOptionSelector.vue";
import FormalDecisionSubmit from "./FormalDecisionSubmit.vue";
import DeferControl from "./DeferControl.vue";
import SaveDraftControl from "./SaveDraftControl.vue";
import RetryControl from "./RetryControl.vue";
import SubmissionStatus from "./SubmissionStatus.vue";
import { decisionFixture, proposalDiffFixture } from "../fixtures";

const wrappers: VueWrapper[] = [];

function mountControl<T extends Component>(
  component: T,
  props: Record<string, unknown>
) {
  const wrapper = mountShell(component, {
    props
  } as unknown as ComponentMountingOptions<T>);
  wrappers.push(wrapper as unknown as VueWrapper);
  return wrapper;
}

afterEach(() => {
  while (wrappers.length) wrappers.pop()?.unmount();
  fake.instances.length = 0;
  document.body.innerHTML = "";
});

const choiceOptions = [
  { value: "full", label: "完整分布算法", description: "包含密度与边缘衰减" },
  { value: "basic", label: "基础分布", description: "仅密度与随机种子" },
  { value: "defer", label: "本轮不做" }
];

describe("SingleSelectControl", () => {
  it("渲染选项描述并在选择后 emit 值", async () => {
    const wrapper = mountControl(SingleSelectControl, {
      modelValue: "",
      options: choiceOptions,
      label: "首发范围"
    });

    expect(wrapper.text()).toContain("包含密度与边缘衰减");
    expect(wrapper.find('[role="radiogroup"]').exists()).toBe(true);

    await wrapper.findAll(".ai-single-select__native")[1].trigger("change");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["basic"]);
  });

  it("选中的选项行标记 is-selected", () => {
    const wrapper = mountControl(SingleSelectControl, {
      modelValue: "full",
      options: choiceOptions
    });

    const rows = wrapper.findAll(".ai-single-select__option");
    expect(rows[0].classes()).toContain("is-selected");
    expect(rows[1].classes()).not.toContain("is-selected");
  });
});

describe("MultiSelectControl", () => {
  it("多选切换后 emit 新数组", async () => {
    const wrapper = mountControl(MultiSelectControl, {
      modelValue: ["full"],
      options: choiceOptions
    });

    await wrapper.findAll('input[type="checkbox"]')[1].trigger("change");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual([
      ["full", "basic"]
    ]);
  });

  it("达到 max 后未选项不可选", async () => {
    const wrapper = mountControl(MultiSelectControl, {
      modelValue: ["full", "basic"],
      options: choiceOptions,
      max: 2
    });

    const inputs = wrapper.findAll('input[type="checkbox"]');
    expect(inputs[2].attributes("disabled")).toBeDefined();

    await inputs[2].trigger("change");
    expect(wrapper.emitted("update:modelValue")).toBeUndefined();
  });

  it("低于 min 时暴露校验错误", async () => {
    const wrapper = mountControl(MultiSelectControl, {
      modelValue: [],
      options: choiceOptions,
      min: 2
    });

    expect(wrapper.find(".ai-control__error").exists()).toBe(false);

    await wrapper.findAll('input[type="checkbox"]')[0].trigger("change");

    expect(wrapper.find(".ai-control__error").exists()).toBe(true);
  });
});

describe("CheckboxGroupControl / RadioGroupControl", () => {
  it("复选组点击后 emit 合并后的值", async () => {
    const wrapper = mountControl(CheckboxGroupControl, {
      modelValue: ["full"],
      options: choiceOptions
    });

    await wrapper.findAll('[role="checkbox"]')[1].trigger("click");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual([
      ["full", "basic"]
    ]);
  });

  it("单选组选择后 emit 单个值", async () => {
    const wrapper = mountControl(RadioGroupControl, {
      modelValue: "full",
      options: choiceOptions
    });

    await wrapper.findAll('input[type="radio"]')[2].trigger("change");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["defer"]);
  });
});

describe("SelectDropdownControl", () => {
  it("label 通过 for/id 关联到下拉触发器", () => {
    const wrapper = mountControl(SelectDropdownControl, {
      modelValue: "full",
      options: choiceOptions,
      label: "首发范围"
    });

    const label = wrapper.find("label");
    const trigger = wrapper.find(".ui-select__trigger");
    expect(trigger.exists()).toBe(true);
    expect(label.attributes("for")).toBe(trigger.attributes("id"));
  });
});

describe("TextFeedbackControl", () => {
  it("Ctrl+Enter 触发 submit，超出 maxLength 截断", async () => {
    const wrapper = mountControl(TextFeedbackControl, {
      modelValue: "",
      maxLength: 4
    });

    const textarea = wrapper.find("textarea");
    await textarea.setValue("abcdef");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["abcd"]);

    await textarea.trigger("keydown", { key: "Enter", ctrlKey: true });
    expect(wrapper.emitted("submit")).toHaveLength(1);
  });
});

describe("ReasonInputControl", () => {
  it("required 且为空时暴露 invalid 与错误文案", () => {
    const wrapper = mountControl(ReasonInputControl, {
      modelValue: "",
      required: true
    });

    expect(wrapper.find(".ui-form-field__error").exists()).toBe(true);
    expect(wrapper.find("textarea").attributes("aria-invalid")).toBe("true");
  });

  it("有内容后校验错误消失", () => {
    const wrapper = mountControl(ReasonInputControl, {
      modelValue: "风险未评估",
      required: true
    });

    expect(wrapper.find(".ui-form-field__error").exists()).toBe(false);
  });
});

describe("RatingControl", () => {
  it("星标为 radiogroup 语义，点击后 emit 分值", async () => {
    const wrapper = mountControl(RatingControl, { modelValue: 2 });

    expect(wrapper.find('[role="radiogroup"]').exists()).toBe(true);
    expect(wrapper.findAll(".ai-rating__star.is-active")).toHaveLength(2);

    await wrapper.findAll('input[type="radio"]')[3].trigger("change");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual([4]);
  });
});

describe("HelpfulControl", () => {
  it("点击切换有用 / 无用，再次点击清空", async () => {
    const wrapper = mountControl(HelpfulControl, { modelValue: null });

    const buttons = wrapper.findAll("button");
    await buttons[0].trigger("click");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["helpful"]);

    await wrapper.setProps({ modelValue: "helpful" });
    await buttons[0].trigger("click");
    expect(wrapper.emitted("update:modelValue")?.[1]).toEqual([null]);
  });
});

describe("InlineEditor", () => {
  it("只读态进入编辑态，Ctrl+Enter 保存", async () => {
    const wrapper = mountControl(InlineEditor, { modelValue: "旧文案" });

    expect(wrapper.find(".ai-inline-editor__text").text()).toBe("旧文案");
    await wrapper.find(".ai-inline-editor__edit").trigger("click");

    const textarea = wrapper.find("textarea");
    expect(textarea.exists()).toBe(true);
    await textarea.setValue("新文案");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["新文案"]);

    await textarea.trigger("keydown", { key: "Enter", ctrlKey: true });
    expect(wrapper.emitted("save")).toHaveLength(1);
  });

  it("Esc 取消编辑并 emit cancel", async () => {
    const wrapper = mountControl(InlineEditor, {
      modelValue: "旧文案",
      editing: true
    });

    await wrapper.find("textarea").trigger("keydown", { key: "Escape" });
    expect(wrapper.emitted("cancel")).toHaveLength(1);
  });
});

describe("RichTextEditorControl", () => {
  it("包装 MarkdownEditor：label 渲染且正文变更向上抛", async () => {
    const wrapper = mountControl(RichTextEditorControl, {
      modelValue: "# 标题",
      label: "正文"
    });

    await nextTick();
    expect(wrapper.find(".ai-rich-text__label").text()).toBe("正文");

    const instance = fake.instances[0];
    expect(instance).toBeDefined();
    (instance.options.input as (value: string) => void)("# 新正文");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["# 新正文"]);
  });

  it("注入附件端口：编辑器拿到上传处理器（上传链路可达）", () => {
    const wrapper = mountControl(RichTextEditorControl, {
      modelValue: ""
    });

    void wrapper;
    const instance = fake.instances[0];
    const upload = instance.options.upload as { handler?: unknown };
    expect(upload.handler).toBeTypeOf("function");
  });
});

describe("DiffViewer", () => {
  it("渲染增删行、行号与路径", () => {
    const wrapper = mountControl(DiffViewer, { diff: proposalDiffFixture });

    expect(wrapper.findAll(".ai-diff__line.is-add")).toHaveLength(3);
    expect(wrapper.findAll(".ai-diff__line.is-remove")).toHaveLength(2);
    expect(wrapper.text()).toContain("terrain-render-pipeline.md");
    expect(wrapper.find(".ai-diff__line.is-add").text()).toContain(
      "TerrainSnapshot"
    );
    expect(wrapper.find(".ai-diff__gutter").text()).toBe("12");
  });

  it("可折叠隐藏差异正文", async () => {
    const wrapper = mountControl(DiffViewer, { diff: proposalDiffFixture });

    expect(wrapper.find(".ai-diff__body").exists()).toBe(true);
    await wrapper.find(".ai-diff__toggle").trigger("click");
    expect(wrapper.find(".ai-diff__body").exists()).toBe(false);
  });
});

describe("EditAcceptControl", () => {
  it("按 state 切换按钮可用性；saved 展示勾", async () => {
    const wrapper = mountControl(EditAcceptControl, { state: "dirty" });

    const buttons = wrapper.findAll("button");
    expect(buttons[0].attributes("disabled")).toBeUndefined();
    expect(buttons[1].attributes("disabled")).toBeUndefined();

    await wrapper.setProps({ state: "saved" });
    expect(wrapper.find(".is-saved").exists()).toBe(true);
    expect(wrapper.findAll("button")[1].attributes("disabled")).toBeDefined();
  });

  it("editing 态不可提交，revert 可撤销", async () => {
    const wrapper = mountControl(EditAcceptControl, { state: "editing" });

    const buttons = wrapper.findAll("button");
    expect(buttons[1].attributes("disabled")).toBeDefined();

    await buttons[0].trigger("click");
    expect(wrapper.emitted("revert")).toHaveLength(1);
  });
});

describe("AcceptRejectControl", () => {
  it("点击拒绝展开原因输入，确认后 emit reject(reason)", async () => {
    const wrapper = mountControl(AcceptRejectControl, {});

    await wrapper
      .findAll(".ai-accept-reject__actions button")[1]
      .trigger("click");
    expect(wrapper.find("textarea").exists()).toBe(true);

    const confirm = wrapper.find(".ai-accept-reject__reason-actions button");
    expect(confirm.attributes("disabled")).toBeDefined();

    await wrapper.find("textarea").setValue("风险未评估，需补充说明");
    expect(
      wrapper
        .find(".ai-accept-reject__reason-actions button")
        .attributes("disabled")
    ).toBeUndefined();

    await wrapper
      .find(".ai-accept-reject__reason-actions button")
      .trigger("click");
    expect(wrapper.emitted("reject")?.[0]).toEqual(["风险未评估，需补充说明"]);
  });

  it("接收直接 emit accept", async () => {
    const wrapper = mountControl(AcceptRejectControl, {});

    await wrapper
      .findAll(".ai-accept-reject__actions button")[0]
      .trigger("click");
    expect(wrapper.emitted("accept")).toHaveLength(1);
  });
});

describe("DecisionOptionSelector", () => {
  it("展示推荐徽标，选择后 emit 选项 id", async () => {
    const wrapper = mountControl(DecisionOptionSelector, {
      modelValue: "",
      options: decisionFixture.options
    });

    expect(wrapper.findAll(".ai-decision-option-selector__badge")).toHaveLength(
      1
    );
    expect(wrapper.text()).toContain("移动端需额外回退实现");

    await wrapper.findAll('input[type="radio"]')[1].trigger("change");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["opt-cpu"]);
  });
});

describe("FormalDecisionSubmit", () => {
  it("requireConfirm 时勾选前不可提交，勾选后可提交", async () => {
    const wrapper = mountControl(FormalDecisionSubmit, {
      selectedOption: "opt-gpu",
      requireConfirm: true
    });

    const submit = wrapper.find(".ai-formal-decision__submit");
    expect(submit.attributes("disabled")).toBeDefined();

    await wrapper.find('[role="checkbox"]').trigger("click");
    expect(
      wrapper.find(".ai-formal-decision__submit").attributes("disabled")
    ).toBeUndefined();

    await wrapper.find(".ai-formal-decision__submit").trigger("click");
    expect(wrapper.emitted("submit")).toHaveLength(1);
  });

  it("意见输入 emit update:comment", async () => {
    const wrapper = mountControl(FormalDecisionSubmit, {
      selectedOption: "opt-gpu"
    });

    await wrapper.find("textarea").setValue("补一句风险");
    expect(wrapper.emitted("update:comment")?.[0]).toEqual(["补一句风险"]);
  });
});

describe("DeferControl", () => {
  it("点击 emit defer", async () => {
    const wrapper = mountControl(DeferControl, {});

    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("defer")).toHaveLength(1);
  });
});

describe("SaveDraftControl", () => {
  it("无改动时禁用，dirty 后可保存", async () => {
    const wrapper = mountControl(SaveDraftControl, { dirty: false });

    expect(wrapper.find("button").attributes("disabled")).toBeDefined();

    await wrapper.setProps({ dirty: true });
    expect(wrapper.find("button").attributes("disabled")).toBeUndefined();

    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("save")).toHaveLength(1);
  });
});

describe("RetryControl", () => {
  it("emit retry 并展示尝试次数；到达上限后禁用", async () => {
    const wrapper = mountControl(RetryControl, {
      attempts: 1,
      maxAttempts: 2
    });

    expect(wrapper.text()).toContain("1");

    await wrapper.find("button").trigger("click");
    expect(wrapper.emitted("retry")).toHaveLength(1);

    await wrapper.setProps({ attempts: 2 });
    expect(wrapper.find("button").attributes("disabled")).toBeDefined();
  });

  it("retrying 时禁用并透传 loading", () => {
    const wrapper = mountControl(RetryControl, { retrying: true });

    expect(wrapper.find("button").attributes("aria-busy")).toBe("true");
    expect(wrapper.find("button").attributes("disabled")).toBeDefined();
  });
});

describe("SubmissionStatus", () => {
  it("透传失败态与重试事件", async () => {
    const wrapper = mountControl(SubmissionStatus, {
      phase: "error",
      errorMessage: "网络异常"
    });

    expect(wrapper.text()).toContain("网络异常");

    await wrapper.find(".ai-submission__retry").trigger("click");
    expect(wrapper.emitted("retry")).toHaveLength(1);
  });
});
