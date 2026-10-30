import { describe, expect, it } from "vitest";
import {
  agents,
  artifacts,
  attentionItems,
  briefing,
  contextAdvice,
  contextPlaceholders,
  decisions,
  demoProject,
  demoProjectId,
  dynamics,
  goals,
  knowledgeDocs,
  docTree,
  metricCards,
  requirements
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
      expect(g.description.trim()).not.toBe("");
      expect(g.progress).toBeGreaterThanOrEqual(0);
      expect(g.progress).toBeLessThanOrEqual(100);
      // 里程碑与任务树（用户目标：里程碑、项目、产品等 mock 数据）
      expect(g.milestones.length).toBeGreaterThanOrEqual(2);
      for (const m of g.milestones) {
        expect(["done", "active", "pending"]).toContain(m.status);
        expect(m.name.trim()).not.toBe("");
        expect(m.due.trim()).not.toBe("");
      }
      expect(g.tasks.length).toBeGreaterThanOrEqual(2);
      for (const t of g.tasks) {
        expect(["done", "doing", "todo"]).toContain(t.status);
        expect(t.owner.trim()).not.toBe("");
      }
    }
    expect(dynamics.length).toBeGreaterThanOrEqual(3);
    for (const d of dynamics) {
      expect(["change", "failure", "decision"]).toContain(d.kind);
      expect(d.text.trim()).not.toBe("");
      expect(d.time.trim()).not.toBe("");
    }
  });

  it("AI 团队：4 名 Agent 与总控台指标一致（1 等待审批）", () => {
    expect(agents).toHaveLength(4);
    expect(agents.filter(a => a.status === "waiting-approval")).toHaveLength(1);
    for (const a of agents) {
      expect(["working", "waiting-approval", "idle"]).toContain(a.status);
      expect(a.name.trim()).not.toBe("");
      expect(a.role.trim()).not.toBe("");
      expect(a.currentTask.trim()).not.toBe("");
      expect(a.completed).toBeGreaterThanOrEqual(0);
      expect(a.lastActive.trim()).not.toBe("");
    }
  });

  it("需求：编号/状态/优先级/验收标准齐备", () => {
    expect(requirements.length).toBeGreaterThanOrEqual(5);
    const ids = requirements.map(r => r.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const r of requirements) {
      expect([
        "draft",
        "reviewing",
        "confirmed",
        "implementing",
        "accepted"
      ]).toContain(r.status);
      expect(["P0", "P1", "P2"]).toContain(r.priority);
      expect(r.acceptance.length).toBeGreaterThanOrEqual(2);
      expect(r.source.trim()).not.toBe("");
    }
  });

  it("决策：双方案对比 + 证据；含待决策与已决策", () => {
    expect(decisions.length).toBeGreaterThanOrEqual(2);
    expect(decisions.some(d => d.status === "pending")).toBe(true);
    expect(decisions.some(d => d.status === "decided")).toBe(true);
    for (const d of decisions) {
      expect(d.options.length).toBeGreaterThanOrEqual(2);
      for (const o of d.options) {
        expect(o.pros.length).toBeGreaterThanOrEqual(1);
        expect(o.cons.length).toBeGreaterThanOrEqual(1);
      }
      expect(d.evidence.length).toBeGreaterThanOrEqual(1);
      expect(d.impact.trim()).not.toBe("");
    }
    // 地形渲染评审与总控台待处理项同一叙事
    expect(decisions.some(d => d.title.includes("地形渲染"))).toBe(true);
  });

  it("项目知识：文档树 + 至少两篇可读文档（版本/来源/正文）", () => {
    expect(docTree.length).toBeGreaterThanOrEqual(3);
    expect(knowledgeDocs.length).toBeGreaterThanOrEqual(2);
    const docIds = new Set(knowledgeDocs.map(d => d.id));
    expect(docIds.size).toBe(knowledgeDocs.length);
    for (const doc of knowledgeDocs) {
      expect(doc.version.trim()).not.toBe("");
      expect(doc.updatedAt.trim()).not.toBe("");
      expect(doc.source.trim()).not.toBe("");
      expect(doc.body.length).toBeGreaterThanOrEqual(2);
    }
  });

  it("产物（产品/交付物）：测试与审批状态齐备，含高保数据模块（与待处理一致）", () => {
    expect(artifacts.length).toBeGreaterThanOrEqual(4);
    for (const a of artifacts) {
      expect(["passed", "failed", "running"]).toContain(a.testStatus);
      expect(["pending", "approved", "rejected"]).toContain(a.approval);
      expect(a.version.trim()).not.toBe("");
      expect(a.owner.trim()).not.toBe("");
    }
    const gaobao = artifacts.find(a => a.name.includes("高保数据模块"));
    expect(gaobao).toBeTruthy();
    expect(gaobao!.testStatus).toBe("passed");
    expect(gaobao!.approval).toBe("pending");
  });
});
