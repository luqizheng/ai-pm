// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { THEME_STORAGE_KEY, createThemeController } from "./useTheme";

describe("useTheme", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("无任何记忆时默认深色", () => {
    const t = createThemeController();
    expect(t.theme.value).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("setTheme 写入 dataset 与 localStorage", () => {
    const t = createThemeController();
    t.setTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
    expect(t.theme.value).toBe("light");
  });

  it("toggle 深浅往返", () => {
    const t = createThemeController();
    t.toggle();
    expect(t.theme.value).toBe("light");
    t.toggle();
    expect(t.theme.value).toBe("dark");
  });

  it("data-theme 已有值时初始化采用之（预挂载脚本先于应用执行）", () => {
    document.documentElement.dataset.theme = "light";
    const t = createThemeController();
    expect(t.theme.value).toBe("light");
  });

  it("无 dataset 时从 localStorage 恢复记忆", () => {
    localStorage.setItem(THEME_STORAGE_KEY, "light");
    const t = createThemeController();
    expect(t.theme.value).toBe("light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
