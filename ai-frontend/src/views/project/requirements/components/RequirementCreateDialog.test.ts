// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";

import { i18n } from "@/lib/i18n";

import RequirementCreateDialog from "./RequirementCreateDialog.vue";

const t = (key: string) => i18n.global.t(key) as string;

// Dialog 经 reka Portal 渲染进 document.body，用例间必须清场，
// 否则上一用例的弹窗残留会污染 DOM 查询。
afterEach(() => {
  document.body.innerHTML = "";
});

async function mountDialog(open = true) {
  const wrapper = mount(RequirementCreateDialog, {
    props: { open, nextCode: "REQ-003" },
    global: { plugins: [i18n] }
  });
  await flushPromises();
  return wrapper;
}

describe("RequirementCreateDialog", () => {
  it("编码只读预生成；标题为空时创建按钮不可用", async () => {
    await mountDialog();

    // Dialog 标题（textContent 可见）
    expect(document.body.textContent).toContain(
      t("pm.requirements.createTitle")
    );
    // 编码输入框：值=REQ-003 且只读（input value 不进 textContent，须查元素）
    const codeInput = [...document.body.querySelectorAll("input")].find(
      input =>
        (input as HTMLInputElement).value === "REQ-003" &&
        input.hasAttribute("disabled")
    );
    expect(codeInput).toBeDefined();

    const createButton = [...document.body.querySelectorAll("button")].find(
      node => node.textContent?.includes(t("pm.requirements.createSubmit"))
    );
    expect(createButton?.hasAttribute("disabled")).toBe(true);
  });

  it("填写标题后创建：payload 携带编码/标题/类型/优先级，正文可空", async () => {
    const wrapper = await mountDialog();

    const titleInput = [...document.body.querySelectorAll("input")].find(
      input =>
        input.hasAttribute("disabled") === false &&
        (input as HTMLInputElement).type !== "checkbox"
    );
    expect(titleInput).toBeDefined();
    titleInput.value = "支持组织级 SSO";
    titleInput.dispatchEvent(new Event("input", { bubbles: true }));
    await flushPromises();

    const createButton = [...document.body.querySelectorAll("button")].find(
      node => node.textContent?.includes(t("pm.requirements.createSubmit"))
    );
    expect(createButton?.hasAttribute("disabled")).toBe(false);
    createButton?.click();
    await flushPromises();

    const emitted = wrapper.emitted("create");
    expect(emitted).toHaveLength(1);
    expect(emitted?.[0]?.[0]).toEqual({
      code: "REQ-003",
      title: "支持组织级 SSO",
      requirement_type: "functional",
      priority: "normal",
      markdown: undefined
    });
  });
});
