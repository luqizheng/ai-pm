// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";

import { SIDEBAR_STORAGE_KEY, useShellStore } from "./shell";

/**
 * 壳状态 store 契约。
 */
describe("shell store", () => {
  beforeEach(() => {
    localStorage.clear();
    setActivePinia(createPinia());
  });

  it("默认展开：左导航展开、右栏打开、指令栏收起", () => {
    const shell = useShellStore();

    expect(shell.sidebarCollapsed).toBe(false);
    expect(shell.sidebarState).toBe("expanded");
    expect(shell.contextOpen).toBe(true);
    expect(shell.contextState).toBe("open");
    expect(shell.commandbarExpanded).toBe(false);
  });

  it("切换左导航折叠态并持久化", () => {
    const shell = useShellStore();

    shell.toggleSidebar();

    expect(shell.sidebarCollapsed).toBe(true);
    expect(shell.sidebarState).toBe("collapsed");
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("1");
  });

  it("重复设置相同折叠态不重复写入", () => {
    const shell = useShellStore();

    shell.setSidebarCollapsed(false);
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBeNull();

    shell.setSidebarCollapsed(true);
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("1");

    shell.setSidebarCollapsed(true);
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("1");
  });

  it("从持久化值恢复折叠态", () => {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, "1");
    setActivePinia(createPinia());

    expect(useShellStore().sidebarCollapsed).toBe(true);
  });

  it("切换右上下文栏与指令栏展开态", () => {
    const shell = useShellStore();

    shell.toggleContext();
    expect(shell.contextOpen).toBe(false);
    expect(shell.contextState).toBe("closed");

    shell.toggleCommandbar();
    expect(shell.commandbarExpanded).toBe(true);

    shell.setCommandbarExpanded(false);
    expect(shell.commandbarExpanded).toBe(false);
  });
});
