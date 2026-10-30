import { describe, expect, it } from "vitest";
import {
  attentionItems,
  briefing,
  contextAdvice,
  contextPlaceholders,
  demoProject,
  demoProjectId,
  dynamics,
  goals,
  metricCards
} from "./fixtures";

describe("workspace fixtures（V1.1 演示数据）", () => {
  it("演示项目与迭代副题", () => {
    expect(demoProjectId).toBe("proj-worldmap");
    expect(demoProject.id).toBe(demoProjectId);
    expect(demoProject.name).toBe("世界地图编辑器");
    expect(demoProject.iteration).toBe("Iteration 2.0");
  });

  it("待处理恰 3 条，类型依次为 决策/验收/阻塞", () => {
    expect(attentionItems).toHaveLength(3);
    expect(attentionItems.map(i => i.kind)).toEqual([
      "decision",
      "acceptance",
      "blocker"
    ]);
    expect(attentionItems[0].title).toBe("地形渲染架构评审");
    expect(attentionItems[1].title).toBe("高保数据模块验收");
    expect(attentionItems[2].title).toContain("森林生成性能");
    for (const item of attentionItems) {
      expect(item.description.trim()).not.toBe("");
      expect(item.advice.trim()).not.toBe("");
    }
  });

  it("AI 负责人简报署名与建议为供稿原文", () => {
    expect(briefing.author).toBe("元构");
    expect(briefing.suggestion).toBe("优先确认地形渲染方案，再推进相关实现。");
    expect(briefing.body.trim()).not.toBe("");
  });

  it("指标卡 3 张：12 迭代任务 / 03 需要介入 / 04 活跃 Agent", () => {
    expect(metricCards.map(c => c.value)).toEqual([12, 3, 4]);
    expect(metricCards[0].caption).toContain("迭代任务");
    expect(metricCards[0].sub).toContain("5 项执行中");
    expect(metricCards[1].caption).toContain("需要介入");
    expect(metricCards[1].sub).toContain("1 项最优先");
    expect(metricCards[2].caption).toContain("活跃 Agent");
    expect(metricCards[2].sub).toContain("1 等待审批");
  });

  it("右栏建议与占位区为供稿原文", () => {
    expect(contextAdvice).toBe("先验证 Shader 原型性能，再做正式架构选择");
    expect(contextPlaceholders).toEqual(["批量原型验证", "补充证据"]);
  });

  it("目标进展与项目动态非空且字段完整", () => {
    expect(goals.length).toBeGreaterThanOrEqual(2);
    for (const g of goals) {
      expect(g.name.trim()).not.toBe("");
      expect(g.milestone.trim()).not.toBe("");
      expect(g.progress).toBeGreaterThanOrEqual(0);
      expect(g.progress).toBeLessThanOrEqual(100);
    }
    expect(dynamics.length).toBeGreaterThanOrEqual(3);
    for (const d of dynamics) {
      expect(["change", "failure", "decision"]).toContain(d.kind);
      expect(d.text.trim()).not.toBe("");
      expect(d.time.trim()).not.toBe("");
    }
  });
});
