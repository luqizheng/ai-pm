// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

import ProjectCreateView from "./ProjectCreateView.vue";
import { mountShell } from "@/components/shell/test-utils";

const createProject = vi.fn();
vi.mock("@/api/projects", () => ({
  createProject: (...args: unknown[]) => createProject(...args)
}));

async function mountView() {
  const mounted = mountShell(ProjectCreateView);
  await flushPromises();
  return mounted;
}

function findButtonByText(
  wrapper: {
    findAll: (
      sel: string
    ) => { text: () => string; trigger: (e: string) => Promise<void> }[];
  },
  text: string
) {
  const target = wrapper
    .findAll("button")
    .find(button => button.text().includes(text));
  expect(target, `button "${text}" should exist`).toBeTruthy();
  return target!;
}

describe("ProjectCreateView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("名称自动建议 Key；人工编辑后停止跟随，可用「使用建议」还原", async () => {
    const { wrapper } = await mountView();
    const inputs = wrapper.findAll("input");
    const nameInput = inputs[0];
    const keyInput = inputs[1];

    // i18n 真实命中（键 MISS 时 t() 会原样吐键名，字面量断言防恒真）
    expect(wrapper.text()).toContain("项目标识（Key）");

    await nameInput.setValue("Smark PM");
    expect((keyInput.element as HTMLInputElement).value).toBe("SMARK");

    // 人工编辑后不再跟随名称
    await keyInput.setValue("CUSTOM");
    await nameInput.setValue("Smark PM Labs");
    expect((keyInput.element as HTMLInputElement).value).toBe("CUSTOM");

    // 还原建议
    await findButtonByText(wrapper, "使用建议").trigger("click");
    expect((keyInput.element as HTMLInputElement).value).toBe("SMARK");
  });

  it("空表单提交触发必填校验且不调用 API", async () => {
    const { wrapper } = await mountView();
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain("请填写项目名称。");
    expect(wrapper.text()).toContain("请填写项目标识。");
    expect(createProject).not.toHaveBeenCalled();
  });

  it("提交成功调用 createProject 并直接进入项目总控台（携带 project_id）", async () => {
    createProject.mockResolvedValue({
      id: "prj-1",
      name: "Atlas Platform"
    });
    const { wrapper, router } = await mountView();

    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("Atlas Platform");
    await wrapper
      .find("textarea")
      .setValue("R&D delivery workbench for platform teams");
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(createProject).toHaveBeenCalledWith({
      code: "ATLAS",
      name: "Atlas Platform",
      description: "R&D delivery workbench for platform teams"
    });

    // 创建即可工作：无成功停留页，直接进入总控台（不再经过初始化总览）
    expect(router.currentRoute.value.path).toBe("/project/command-center");
    expect(router.currentRoute.value.query.project_id).toBe("prj-1");
    wrapper.unmount();
  });

  it("服务端 409 冲突落在 Key 字段，表单数据保留", async () => {
    createProject.mockRejectedValue(
      Object.assign(new Error("conflict"), { status: 409 })
    );
    const { wrapper } = await mountView();

    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("Atlas Platform");
    await wrapper.find("form").trigger("submit");
    await flushPromises();

    expect(wrapper.text()).toContain("该标识已被其他项目占用，请更换一个。");
    expect((inputs[0].element as HTMLInputElement).value).toBe(
      "Atlas Platform"
    );
    expect(createProject).toHaveBeenCalledTimes(1);
  });
});
