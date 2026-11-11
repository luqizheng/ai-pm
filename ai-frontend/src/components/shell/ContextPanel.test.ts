// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";

import ContextPanel from "./ContextPanel.vue";
import { useShellStore } from "@/stores/shell";
import { demoProject, demoProjects } from "./demo";
import { mountShell } from "./test-utils";

/**
 * 右上下文栏行为（Application Shell §2D）。
 */

/**
 * 切到 Sources Tab（DR-C3：注入源归 Sources，不在 Context Tab）。
 *
 * 注入源/作用域串的断言意图不变，只是先切 Tab 再断言。
 */
async function openSourcesTab(wrapper: { find: (s: string) => unknown }) {
  const tabs = wrapper.findAll(".shell-context__tab") as Array<{
    text: () => string;
    trigger: (e: string) => Promise<void>;
  }>;
  const sources = tabs.find(tab => tab.text() === "来源");
  expect(sources).toBeTruthy();
  await sources!.trigger("click");
  await nextTick();
}

describe("ContextPanel", () => {
  it("渲染当前项目与仓库元数据", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    shell.setProjects(demoProjects);
    shell.setCurrentProject(demoProject.id);
    await nextTick();

    expect(wrapper.find(".shell-context__project-name").text()).toBe(
      "Atlas Platform"
    );

    // mock 样本按需动态加载（等待 onMounted 中的 import 完成）
    await vi.waitFor(() => {
      const labels = wrapper.findAll(".shell-context__meta-label");
      expect(labels.map(el => el.text())).toEqual([
        "可见性",
        "默认分支",
        "我的角色",
        "归属团队"
      ]);
    });
  });

  it("渲染相关信息链接与助手上下文提示", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();

    await vi.waitFor(() => {
      expect(wrapper.findAll(".shell-context__link").length).toBe(2);
    });
    // 注入源归 Sources Tab（DR-C3）
    await openSourcesTab(wrapper);
    expect(wrapper.find(".shell-context__ai").exists()).toBe(true);
  });

  it("点击关闭按钮收起右上下文栏（翻转 store 的 contextOpen）", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    expect(shell.contextOpen).toBe(true);

    const close = wrapper
      .findAll(".shell-context__icon-button")
      .find(el => el.attributes("aria-label") === "关闭上下文栏");
    expect(close).toBeTruthy();

    await close!.trigger("click");
    await nextTick();

    expect(shell.contextOpen).toBe(false);
  });

  // ───────────────────────────────── 单一真源（整合计划 Batch 1）

  it("仓库元数据取自 assistantContext 单一真源", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "release/2026",
      role: "admin",
      status: "verified"
    };
    await nextTick();

    // 非 mock 预览态：不渲染相关文档段，只渲染真实仓库元数据
    const rows = wrapper.findAll(".shell-context__meta-row");
    const pairs = rows.map(row => ({
      label: row.find(".shell-context__meta-label").text(),
      value: row.find(".shell-context__meta-value").text()
    }));
    expect(pairs).toEqual([
      { label: "默认分支", value: "release/2026" },
      { label: "我的角色", value: "admin" }
    ]);
  });

  it("上下文卡列真实注入源（Batch 4 诚实化）：与 Batch 3A 两节对应", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };
    await nextTick();
    await openSourcesTab(wrapper);

    const sources = wrapper.find(".shell-context__ai-sources").text();
    expect(sources).toContain("注入源");
    // text() 拼接含模板边界空白，逐段断言（空白不敏感）
    expect(sources).toContain("项目画像");
    expect(sources).toContain("Atlas Platform");
    expect(sources).toContain("仓库事实");
    expect(sources.replace(/\s+/g, "")).toContain(
      "仓库事实·atlas-platform/main"
    );
  });

  it("仓库未锚定时如实显示「未锚定」，不冒充注入面", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    await nextTick();
    await openSourcesTab(wrapper);

    const sources = wrapper.find(".shell-context__ai-sources").text();
    // 未锚定如实显示「未锚定」，且不出现任何仓库名/分支（无 "/仓库/分支" 面）
    expect(sources).toContain("未锚定");
    expect(sources.replace(/\s+/g, "")).not.toContain("/");
  });

  it("助手上下文作用域串与 store 的 assistantContext 一致", async () => {
    const { wrapper } = mountShell(ContextPanel);
    const shell = useShellStore();
    shell.setProjects([{ id: "p1", name: "Atlas Platform" }]);
    shell.setCurrentProject("p1");
    shell.currentRepository = {
      name: "atlas-platform",
      defaultBranch: "main",
      role: "main",
      status: "verified"
    };
    await nextTick();
    await openSourcesTab(wrapper);

    expect(wrapper.find(".shell-context__ai-text").text()).toContain(
      `${shell.assistantContext.projectName} / Repository / ${shell.assistantContext.defaultBranch}`
    );
  });
});

// ───────────────────────────────── 四一级 Tab + History Drawer（DR-C3）

describe("ContextPanel（四 Tab + History Drawer）", () => {
  it("四个一级 Tab 齐备，切换不重排壳", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();

    const tabs = wrapper.findAll(".shell-context__tab");
    expect(tabs.map(tab => tab.text())).toEqual([
      "上下文",
      "来源",
      "结果",
      "操作"
    ]);
    // 默认在 Context Tab
    expect(wrapper.attributes("aria-label")).toBe("上下文");
    expect(wrapper.find(".shell-context__project-name").exists()).toBe(true);
  });

  it("Sources Tab 无注入记账时如实显示「未注入」，不冒充注入面", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();
    await openSourcesTab(wrapper);

    expect(wrapper.text()).toContain("尚未收到 AI 回复");
  });

  it("Results / Actions Tab 无数据时给空态，不填充演示数据", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();

    const tabs = wrapper.findAll(".shell-context__tab");
    await tabs[2].trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("当前会话还没有 AI 结果");

    await tabs[3].trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("当前会话还没有待授权操作");
  });

  it("History Drawer：二级入口打开、五类筛选、关闭后回到原 Tab", async () => {
    const { wrapper } = mountShell(ContextPanel);
    await nextTick();

    // History 不是第五个主 Tab —— 走二级入口
    expect(wrapper.findAll(".shell-context__tab").length).toBe(4);

    const history = wrapper
      .findAll(".shell-context__icon-button")
      .find(el => el.attributes("aria-label") === "历史记录");
    expect(history).toBeTruthy();

    await history!.trigger("click");
    await nextTick();

    // Drawer 覆盖本栏，五个类别可筛选
    const chips = wrapper.findAll(".shell-context__chip");
    expect(chips.map(chip => chip.text())).toEqual([
      "全部",
      "会话",
      "AI 动作",
      "上下文变更",
      "审批",
      "执行"
    ]);

    const close = wrapper
      .findAll(".shell-context__icon-button")
      .find(el => el.attributes("aria-label") === "关闭历史记录");
    expect(close).toBeTruthy();
    await close!.trigger("click");
    await nextTick();

    // 关闭后回到原先的 Tab（Context）
    expect(wrapper.find(".shell-context__drawer").exists()).toBe(false);
    expect(wrapper.find(".shell-context__project-name").exists()).toBe(true);
  });
});
