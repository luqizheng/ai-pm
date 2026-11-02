import { describe, expect, it } from "vitest";
import {
  acceptanceDetails,
  acceptanceOverview,
  agentRun,
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
      // V1.2：GOAL 编码 + 进度依据（不得是无依据的 AI 估算百分比）
      expect(g.code).toMatch(/^GOAL-\d{3}$/);
      expect(g.progressBasis.trim()).not.toBe("");
      // 依赖链（V1.2：依赖图）
      expect(g.dependencies.length).toBeGreaterThanOrEqual(2);
      // 里程碑与任务树（用户目标：里程碑、项目、产品等 mock 数据）
      expect(g.milestones.length).toBeGreaterThanOrEqual(2);
      for (const m of g.milestones) {
        expect(["done", "active", "pending"]).toContain(m.status);
        expect(m.name.trim()).not.toBe("");
        expect(m.due.trim()).not.toBe("");
      }
      expect(g.tasks.length).toBeGreaterThanOrEqual(2);
      for (const t of g.tasks) {
        // V1.2 扩展态：blocked / awaiting-acceptance
        expect([
          "done",
          "doing",
          "todo",
          "blocked",
          "awaiting-acceptance"
        ]).toContain(t.status);
        expect(t.owner.trim()).not.toBe("");
      }
      // AI 提议的计划（草案）与已批准计划分离
      for (const p of g.proposedTasks) {
        expect(p.status).toBe("proposed");
        expect(p.owner.trim()).not.toBe("");
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
      // V1.2：正式决策的完整字段
      expect(d.background.trim()).not.toBe("");
      expect(d.recommendation.trim()).not.toBe("");
      expect(d.risks.length).toBeGreaterThanOrEqual(1);
      expect(d.dimensions.length).toBeGreaterThanOrEqual(2);
      expect(d.options.length).toBeGreaterThanOrEqual(2);
      for (const o of d.options) {
        expect(o.pros.length).toBeGreaterThanOrEqual(1);
        expect(o.cons.length).toBeGreaterThanOrEqual(1);
      }
      expect(d.evidence.length).toBeGreaterThanOrEqual(1);
      expect(d.impact.trim()).not.toBe("");
      if (d.status === "decided") {
        expect(d.decidedBy!.trim()).not.toBe("");
        expect(d.approvedAt!.trim()).not.toBe("");
        expect(d.verification!.length).toBeGreaterThanOrEqual(1);
      }
    }
    // V1.2：需求变化触发重新评估提示（系统主动提示，不让 Agent 按旧决策执行）
    expect(decisions.some(d => d.reassess)).toBe(true);
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
      // V1.2：人工批准的正式事实与 AI 维护摘要分离 + 引用关系
      expect(["approved", "ai-draft"]).toContain(doc.review);
      expect(doc.approvedFacts!.length).toBeGreaterThanOrEqual(1);
      expect(doc.aiSummary!.trim()).not.toBe("");
      expect(doc.references.length).toBeGreaterThanOrEqual(1);
      for (const ref of doc.references) {
        expect(["decision", "requirement", "artifact"]).toContain(ref.kind);
        expect(ref.title.trim()).not.toBe("");
      }
    }
  });

  it("产物（产品/交付物）：测试与审批状态齐备，含高保数据模块（与待处理一致）", () => {
    expect(artifacts.length).toBeGreaterThanOrEqual(4);
    for (const a of artifacts) {
      expect(["passed", "failed", "running"]).toContain(a.testStatus);
      expect(["pending", "approved", "rejected"]).toContain(a.approval);
      expect(a.version.trim()).not.toBe("");
      expect(a.owner.trim()).not.toBe("");
      // V1.2：文件 Diff + 行级评论 + 测试报告 + 验收标准 + 整体判定
      expect(a.files.length).toBeGreaterThanOrEqual(1);
      for (const f of a.files) {
        expect(f.diff.length).toBeGreaterThanOrEqual(1);
      }
      expect(a.testReport.length).toBeGreaterThanOrEqual(1);
      expect(a.acceptanceCriteria.length).toBeGreaterThanOrEqual(1);
      for (const c of a.acceptanceCriteria) {
        expect(typeof c.passed).toBe("boolean");
      }
      // 「部分检查通过」与「整体符合验收标准」必须可区分
      expect(["partial", "meets", "not-met"]).toContain(a.verdict);
      expect(a.linkedTask.trim()).not.toBe("");
    }
    const gaobao = artifacts.find(a => a.name.includes("高保数据模块"));
    expect(gaobao).toBeTruthy();
    expect(gaobao!.testStatus).toBe("passed");
    expect(gaobao!.approval).toBe("pending");
    // 功能测试通过但性能验证未完成 → 部分检查通过（V1.2 明确要求可区分）
    expect(gaobao!.verdict).toBe("partial");
  });

  it("AI 执行中心：执行步骤区分四类事件，进度用检查点而非百分比（V1.2）", () => {
    const types = new Set(agentRun.steps.map(s => s.type));
    for (const type of ["thought", "tool", "state", "approval"]) {
      expect(types.has(type as never), `缺少事件类型 ${type}`).toBe(true);
    }
    expect(agentRun.steps.length).toBeGreaterThanOrEqual(6);
    for (const step of agentRun.steps) {
      expect(["done", "running", "waiting"]).toContain(step.status);
      expect(step.title.trim()).not.toBe("");
      expect(step.detail.trim()).not.toBe("");
      if (step.type === "tool") {
        expect(step.tool).toBeTruthy();
        expect(step.tool!.name.trim()).not.toBe("");
      }
    }
    // 可验证的阶段状态、已完成检查点、剩余工作——不是 AI 估算百分比
    expect(agentRun.phase.trim()).not.toBe("");
    expect(agentRun.checkpoints.length).toBeGreaterThanOrEqual(1);
    expect(agentRun.remaining.length).toBeGreaterThanOrEqual(1);
    // 存在等待人工审批的步骤（与 Agent 等待审批一致）
    expect(
      agentRun.steps.some(s => s.type === "approval" && s.status === "waiting")
    ).toBe(true);
  });
});

describe("交付验收 fixtures（T-070）：与后端契约同构、徽标互斥、证据表齐全", () => {
  it("project_id 等于 demoProjectId，三指标由任务徽标推得", () => {
    expect(acceptanceOverview.project_id).toBe(demoProjectId);
    expect(acceptanceOverview.total).toBe(acceptanceOverview.tasks.length);
    const count = (badge: string) =>
      acceptanceOverview.tasks.filter(t => t.badge === badge).length;
    expect(acceptanceOverview.counts).toEqual({
      pending: acceptanceOverview.tasks.length,
      eligible: count("eligible"),
      blocked: count("blocked")
    });
  });

  it("每个任务恰好一个合法徽标；pending 审批仅在 awaiting_decision 时存在", () => {
    for (const task of acceptanceOverview.tasks) {
      expect(["eligible", "blocked", "missing"]).toContain(task.badge);
      if (task.review_status === "awaiting_decision") {
        expect(task.pending_approval_id).not.toBeNull();
      } else {
        expect(task.pending_approval_id).toBeNull();
      }
    }
    // Q1：存在零交付（待补充）的任务，证明总表不以交付状态筛任务。
    expect(
      acceptanceOverview.tasks.some(
        t => t.badge === "missing" && t.delivery_summary.expected_repos === 0
      )
    ).toBe(true);
  });

  it("每个任务都有证据表，且 pending 审批 id 能在审批历史里找到", () => {
    for (const task of acceptanceOverview.tasks) {
      const detail = acceptanceDetails[task.task_id];
      expect(detail, task.task_id).toBeDefined();
      if (task.pending_approval_id) {
        expect(
          detail.approvals.some(
            a => a.id === task.pending_approval_id && a.status === "pending"
          )
        ).toBe(true);
      }
      for (const run of detail.runs) {
        expect(run.task_id).toBe(task.task_id);
      }
    }
  });
});
