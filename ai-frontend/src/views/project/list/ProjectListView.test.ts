// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { nextTick } from "vue";

import ConfirmationDialog from "@/components/ui/confirmation-dialog/ConfirmationDialog.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

import ProjectListView from "./ProjectListView.vue";
import { loadProjectEntries } from "./fixtures";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

/** reka-ui 浮层（菜单/确认弹窗）为异步渲染：留出宏任务时间 */
async function flushMenu() {
  await nextTick();
  await new Promise(resolve => setTimeout(resolve, 10));
  await nextTick();
}

async function mountList(loader?: () => Promise<unknown>) {
  const mounted = mountShell(ProjectListView, {
    props: loader ? { loader } : {}
  });
  await mounted.router.push("/projects");
  await flushPromises();
  return mounted;
}

describe("ProjectListView", () => {
  it("渲染项目列表：页头、四指标、样本项目、工具栏与仓库摘要", async () => {
    const mounted = await mountList();
    const text = mounted.wrapper.text();

    // 页头（设计稿 §3）
    expect(text).toContain(t("pm.projects.pageTitle"));
    expect(text).toContain(t("pm.projects.new"));
    // 概览指标（§4）
    expect(text).toContain(t("pm.projects.metricTotal"));
    expect(text).toContain(t("pm.projects.metricRecent"));
    expect(text).toContain(t("pm.projects.metricAttention"));
    expect(text).toContain(t("pm.projects.metricArchived"));
    // 工具栏（§5）与五种形态样本（§6）：多仓库 / 零仓库 / 无主仓库 / 不可达 / 归档
    expect(text).toContain(t("pm.projects.filterAll"));
    expect(text).toContain(t("pm.projects.sortUpdated"));
    expect(text).toContain("Atlas Platform");
    expect(text).toContain("Docs Portal");
    expect(text).toContain("Mobile Companion");
    expect(text).toContain("Billing Core");
    expect(text).toContain("Legacy Reporter");
    expect(text).toContain(t("pm.projects.repoNone"));
    expect(text).toContain(t("pm.projects.repoNoPrimary"));
    expect(text).toContain(t("pm.projects.repoUnreachable"));
    expect(text).toContain(t("pm.projects.statusArchived"));
  });

  it("指标点击联动过滤：归档指标只剩归档项目，关注指标只剩存在不可达仓库的项目", async () => {
    const mounted = await mountList();

    const archivedMetric = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.projects.metricArchived")));
    await archivedMetric?.trigger("click");
    await flushPromises();

    let text = mounted.wrapper.text();
    expect(text).toContain("Legacy Reporter");
    expect(text).not.toContain("Atlas Platform");

    // 回到全部，再点「需要关注」：Billing Core 有 unreachable 仓库
    const totalMetric = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.projects.metricTotal")));
    await totalMetric?.trigger("click");
    const attentionMetric = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.projects.metricAttention")));
    await attentionMetric?.trigger("click");
    await flushPromises();

    text = mounted.wrapper.text();
    expect(text).toContain("Billing Core");
    expect(text).not.toContain("Atlas Platform");
    // 关注过滤 chip 出现且可移除
    expect(text).toContain(t("pm.projects.attentionChip"));
  });

  it("搜索无结果呈现空态，清除过滤后恢复", async () => {
    const mounted = await mountList();

    const input = mounted.wrapper.find("input");
    await input.setValue("no-such-project-zzz");
    await flushPromises();

    expect(mounted.wrapper.text()).toContain(t("pm.projects.noResultsTitle"));

    const clearButton = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.projects.noResultsCta")));
    await clearButton?.trigger("click");
    await flushPromises();

    expect(mounted.wrapper.text()).toContain("Atlas Platform");
  });

  it("删除是显式操作：必须经确认弹窗，确认后项目才被移除", async () => {
    const mounted = await mountList();

    // 打开第一行（Atlas Platform）的操作菜单 → Delete
    const trigger = mounted.wrapper.find('[aria-haspopup="menu"]');
    await trigger.trigger("click");
    await flushMenu();

    const deleteItem = Array.from(
      document.querySelectorAll<HTMLElement>('[role="menuitem"]')
    ).find(node => node.textContent?.includes(t("pm.projects.menuDelete")));
    expect(deleteItem).toBeTruthy();
    deleteItem?.click();
    await flushMenu();

    // 确认弹窗出现（危险 variant），项目尚未被移除（红线：未确认不生效）
    expect(document.body.textContent).toContain(t("pm.projects.deleteTitle"));
    expect(mounted.wrapper.text()).toContain("Atlas Platform");

    const dialog = mounted.wrapper.findComponent(ConfirmationDialog);
    expect(dialog.exists()).toBe(true);
    dialog.vm.$emit("confirm");
    await flushPromises();

    expect(mounted.wrapper.text()).not.toContain("Atlas Platform");
  });

  it("加载失败呈现错误态，重试成功后恢复列表", async () => {
    let shouldFail = true;
    const loader = () =>
      shouldFail
        ? Promise.reject(new Error("network down"))
        : loadProjectEntries();

    const mounted = await mountList(loader);

    expect(mounted.wrapper.text()).toContain(t("pm.projects.errorTitle"));

    const retryButton = mounted.wrapper
      .findAll("button")
      .find(node => node.text().includes(t("pm.projects.errorCta")));
    shouldFail = false;
    await retryButton?.trigger("click");
    await flushPromises();

    expect(mounted.wrapper.text()).toContain("Atlas Platform");
    expect(mounted.wrapper.text()).not.toContain(t("pm.projects.errorTitle"));
  });

  it("点击项目行打开项目总控台，并携带项目上下文 query", async () => {
    const mounted = await mountList();

    const row = mounted.wrapper.find(".proj-row");
    await row.trigger("click");
    await flushPromises();

    // 总控台以 ?project_id= 为数据真源（CommandCenterView 只认 URL query），
    // 不带 query 会命中「请先在顶栏选择一个项目」空态
    expect(mounted.router.currentRoute.value.path).toBe(
      "/project/command-center"
    );
    expect(mounted.router.currentRoute.value.query.project_id).toBe("proj-001");
  });
});
