// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConsolePage from "./ConsolePage.vue";
import { mountShell } from "../../shell/test-utils";
import { useWorkspaceStore } from "../store";
import {
  attentionItems,
  briefing,
  demoProject,
  dynamics,
  goals,
  metricCards
} from "../fixtures";

const push = vi.fn();
let mockProjectId = "proj-worldmap";

vi.mock("vue-router", () => ({
  useRoute: () => ({
    path: `/workspace/${mockProjectId}/console`,
    params: { projectId: mockProjectId },
    query: {}
  }),
  useRouter: () => ({ push })
}));

describe("ConsolePage（总控台 V1.1）", () => {
  beforeEach(() => {
    push.mockClear();
    mockProjectId = "proj-worldmap";
    localStorage.clear();
    document.documentElement.dataset.theme = "dark";
  });

  it("页头：标题、运行中徽标与演示数据副题", () => {
    const w = mountShell(ConsolePage);
    const text = w.text();
    expect(text).toContain("项目总控");
    expect(text).toContain("项目运行中");
    expect(text).toContain(demoProject.name);
    expect(text).toContain(demoProject.iteration);
    expect(text).toContain("演示数据");
  });

  it("AI 负责人简报：署名与建议为供稿原文", () => {
    const w = mountShell(ConsolePage);
    const text = w.text();
    expect(text).toContain(briefing.author);
    expect(text).toContain(briefing.body);
    expect(text).toContain(briefing.suggestion);
  });

  it("指标卡三张值 12 / 3 / 4", () => {
    const w = mountShell(ConsolePage);
    const values = w
      .findAll("[data-test='metric-value']")
      .map(el => Number(el.text()));
    expect(values).toEqual(metricCards.map(c => c.value));
    expect(values).toEqual([12, 3, 4]);
  });

  it("需要你处理：3 条带类型徽标（决策/验收/阻塞）", () => {
    const w = mountShell(ConsolePage);
    const items = w.findAll("[data-test='attention-item']");
    expect(items).toHaveLength(3);
    const text = w.text();
    expect(text).toContain("决策");
    expect(text).toContain("验收");
    expect(text).toContain("阻塞");
    for (const [i, item] of attentionItems.entries()) {
      expect(items[i].text()).toContain(item.title);
    }
  });

  it("点击待处理项 → store 选中 + 选中描边", async () => {
    const w = mountShell(ConsolePage);
    const store = useWorkspaceStore();
    const items = w.findAll("[data-test='attention-item']");
    await items[1].trigger("click");
    expect(store.selectedAttentionId).toBe(attentionItems[1].id);
    expect(items[1].classes()).toContain("ws-selected-ring");
    expect(items[0].classes()).not.toContain("ws-selected-ring");
  });

  it("默认选中首条（与 V1.1 原型一致）", () => {
    const w = mountShell(ConsolePage);
    const items = w.findAll("[data-test='attention-item']");
    expect(items[0].classes()).toContain("ws-selected-ring");
  });

  it("目标进展与项目动态按 fixtures 渲染", () => {
    const w = mountShell(ConsolePage);
    expect(w.findAll("[data-test='goal-item']")).toHaveLength(goals.length);
    expect(w.findAll("[data-test='dynamic-item']")).toHaveLength(
      dynamics.length
    );
  });

  it("未知 projectId 渲染降级视图，不渲染演示数据", async () => {
    mockProjectId = "proj-other";
    const w = mountShell(ConsolePage);
    const text = w.text();
    expect(text).toContain("页面不存在或无权访问");
    expect(text).not.toContain(briefing.author);
    const back = w.find("[data-test='unavailable-back']");
    expect(back.exists()).toBe(true);
    await back.trigger("click");
    expect(push).toHaveBeenCalledWith("/dashboard");
  });
});
