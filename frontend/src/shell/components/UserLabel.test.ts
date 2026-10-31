// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { i18n } from "../i18n";
import UserLabel from "./UserLabel.vue";

const { listPrincipals } = vi.hoisted(() => ({ listPrincipals: vi.fn() }));

// 目录数据源已切到 ai-pm principals（/v1/pm/principals）。
vi.mock("../../api/pm", () => ({ listPrincipals }));

function page(
  items: { id: string; display_name: string }[],
  total: number,
  pageNo = 1
) {
  return { items, total, page: pageNo, size: 200 };
}

function mountLabel(props: { userId?: string | null }, pinia = createPinia()) {
  return mount(UserLabel, { props, global: { plugins: [pinia, i18n] } });
}

describe("UserLabel", () => {
  beforeEach(() => {
    listPrincipals.mockReset();
  });

  it("按 userId 解析出昵称", async () => {
    listPrincipals.mockResolvedValue(
      page([{ id: "dev__1", display_name: "张三" }], 1)
    );
    const w = mountLabel({ userId: "dev__1" });
    await flushPromises();
    expect(w.text()).toBe("张三");
  });

  it("同一 userId 只向服务器请求一次", async () => {
    listPrincipals.mockResolvedValue(
      page([{ id: "dev__1", display_name: "张三" }], 1)
    );
    const pinia = createPinia();
    const a = mountLabel({ userId: "dev__1" }, pinia);
    const b = mountLabel({ userId: "dev__1" }, pinia);
    await flushPromises();
    expect(a.text()).toBe("张三");
    expect(b.text()).toBe("张三");
    expect(listPrincipals).toHaveBeenCalledTimes(1);
  });

  it("回退文案剥离 dev__ 环境前缀", async () => {
    listPrincipals.mockResolvedValue(page([], 0));
    const w = mountLabel({ userId: "dev__404" });
    await flushPromises();
    expect(w.text()).toBe("用户 #404");
  });

  it("回退文案保留无前缀 ID 原样", async () => {
    listPrincipals.mockResolvedValue(page([], 0));
    const w = mountLabel({ userId: "ABC123" });
    await flushPromises();
    expect(w.text()).toBe("用户 #ABC123");
  });

  it("无 userId 时显示占位且不请求", async () => {
    listPrincipals.mockResolvedValue(page([], 0));
    const w = mountLabel({ userId: null });
    await flushPromises();
    expect(w.text()).toBe("—");
    expect(listPrincipals).not.toHaveBeenCalled();
  });

  it("目录分页时合并所有页", async () => {
    listPrincipals
      .mockResolvedValueOnce(page([{ id: "u1", display_name: "甲" }], 201, 1))
      .mockResolvedValueOnce(page([{ id: "u2", display_name: "乙" }], 201, 2));
    const w = mountLabel({ userId: "u2" });
    await flushPromises();
    await flushPromises();
    expect(w.text()).toBe("乙");
    expect(listPrincipals).toHaveBeenCalledTimes(2);
  });
});
