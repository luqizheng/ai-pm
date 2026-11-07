// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";

import CommandCenterView from "./CommandCenterView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { i18n } from "@/lib/i18n";

const t = (key: string, params?: Record<string, unknown>) =>
  i18n.global.t(key, params ?? {}) as string;

describe("CommandCenterView", () => {
  it("注册路由后渲染六大区块与设计稿样本数据", async () => {
    const mounted = mountShell(CommandCenterView);
    await mounted.router.push("/project/command-center");
    await flushPromises();

    const text = mounted.wrapper.text();

    // 页头（标题 + 状态/可见性 chip + 元信息）
    expect(text).toContain("Atlas Platform");
    expect(text).toContain(t("s01.command-center.statusNormal"));
    expect(text).toContain(t("s01.command-center.visibilityOrgOnly"));
    // 健康度 + 需关注
    expect(text).toContain(t("s01.command-center.healthTitle"));
    expect(text).toContain(t("s01.command-center.attentionTitle"));
    expect(text).toContain("REQ-118");
    // 任务/Agents 双栏
    expect(text).toContain(t("s01.command-center.taskTitle"));
    expect(text).toContain("Planner-01");
    // 运行/交付物双栏
    expect(text).toContain("RUN-8842");
    expect(text).toContain(t("s01.command-center.delivFootnote"));
    // 动态 + 右栏
    expect(text).toContain(t("s01.command-center.activityTitle"));
    expect(text).toContain(t("s01.command-center.knowledgeTitle"));
    expect(text).toContain("林悦");
  });

  it("页头「项目设置」跳转 Project Setting（V1.0 稿统一入口）", async () => {
    const mounted = mountShell(CommandCenterView);
    await mounted.router.push("/project/command-center");
    await flushPromises();

    const header = mounted.wrapper.findComponent({
      name: "CommandCenterHeader"
    });
    expect(header.exists()).toBe(true);
    header.vm.$emit("settings");
    await flushPromises();
    expect(mounted.router.currentRoute.value.path).toBe(
      "/project/settings/general"
    );
  });
});
