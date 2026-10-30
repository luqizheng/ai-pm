// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import GoalsPage from "./GoalsPage.vue";
import TeamPage from "./TeamPage.vue";
import ActivityPage from "./ActivityPage.vue";
import RequirementsPage from "./RequirementsPage.vue";
import DecisionsPage from "./DecisionsPage.vue";
import KnowledgePage from "./KnowledgePage.vue";
import ArtifactsPage from "./ArtifactsPage.vue";
import { mountShell } from "../../shell/test-utils";
import {
  agents,
  artifacts,
  decisions,
  dynamics,
  goals,
  knowledgeDocs,
  requirements
} from "../fixtures";

vi.mock("vue-router", () => ({
  useRoute: () => ({
    path: "/workspace/proj-worldmap/goals",
    params: {},
    query: {}
  }),
  useRouter: () => ({ push: vi.fn() })
}));

beforeEach(() => {
  localStorage.clear();
  document.documentElement.dataset.theme = "dark";
});

describe("GoalsPage（目标计划）", () => {
  it("渲染标题与全部目标卡（里程碑 + 任务树）", () => {
    const w = mountShell(GoalsPage);
    expect(w.text()).toContain("目标计划");
    expect(w.findAll("[data-test='goal-card']")).toHaveLength(goals.length);
    const text = w.text();
    expect(text).toContain("架构评审通过");
    expect(text).toContain("目标帧率与视距范围定义");
    expect(
      w.findAll("[data-test='milestone-item']").length
    ).toBeGreaterThanOrEqual(7);
    expect(w.findAll("[data-test='task-item']").length).toBeGreaterThanOrEqual(
      8
    );
  });

  it("里程碑与任务带状态徽标", () => {
    const w = mountShell(GoalsPage);
    const text = w.text();
    expect(text).toContain("已达成");
    expect(text).toContain("进行中");
    expect(text).toContain("未开始");
  });
});

describe("TeamPage（AI 团队）", () => {
  it("渲染 4 名 Agent 与状态徽标（1 等待审批）", () => {
    const w = mountShell(TeamPage);
    expect(w.text()).toContain("AI 团队");
    const cards = w.findAll("[data-test='agent-card']");
    expect(cards).toHaveLength(agents.length);
    expect(cards).toHaveLength(4);
    const text = w.text();
    expect(text).toContain("元构");
    expect(text).toContain("等待审批");
    expect(text).toContain("执行中");
    expect(text).toContain("CPU 预计算原型性能基准");
  });
});

describe("ActivityPage（项目动态）", () => {
  it("渲染全部动态条目与类型徽标", () => {
    const w = mountShell(ActivityPage);
    expect(w.text()).toContain("项目动态");
    expect(w.findAll("[data-test='activity-item']")).toHaveLength(
      dynamics.length
    );
    const text = w.text();
    expect(text).toContain("高保数据模块全部测试通过");
    expect(text).toContain("变更");
    expect(text).toContain("失败");
  });
});

describe("RequirementsPage（需求管理）", () => {
  it("渲染需求清单（编号/优先级/状态/验收标准）", () => {
    const w = mountShell(RequirementsPage);
    expect(w.text()).toContain("需求管理");
    expect(w.findAll("[data-test='req-item']")).toHaveLength(
      requirements.length
    );
    const text = w.text();
    expect(text).toContain("REQ-101");
    expect(text).toContain("P0");
    expect(text).toContain("目标帧率 ≥ 60 FPS（中端 GPU）");
    expect(text).toContain("评审中");
  });
});

describe("DecisionsPage（决策中心）", () => {
  it("渲染双方案对比、证据与结论", () => {
    const w = mountShell(DecisionsPage);
    expect(w.text()).toContain("决策中心");
    expect(w.findAll("[data-test='decision-card']")).toHaveLength(
      decisions.length
    );
    const text = w.text();
    expect(text).toContain("GPU Shader");
    expect(text).toContain("CPU 预计算");
    expect(text).toContain("优势");
    expect(text).toContain("劣势");
    expect(text).toContain("证据");
    expect(text).toContain("采用分块 tile 版本");
  });
});

describe("KnowledgePage（项目知识）", () => {
  it("文档树 + 阅读器：默认选中首篇，点击切换", async () => {
    const w = mountShell(KnowledgePage);
    expect(w.text()).toContain("项目知识");
    expect(w.find("[data-test='doc-reader']").text()).toContain(
      knowledgeDocs[0].title
    );
    const nodes = w.findAll("[data-test='doc-node']");
    const target = nodes.find(n => n.text() === "迭代 2.0 需求文档.md");
    expect(target).toBeTruthy();
    await target!.trigger("click");
    expect(w.find("[data-test='doc-reader']").text()).toContain(
      "迭代 2.0 需求文档"
    );
    expect(w.find("[data-test='doc-reader']").text()).toContain("v1.4");
  });
});

describe("ArtifactsPage（产物验收）", () => {
  it("渲染产物清单与测试/审批徽标", () => {
    const w = mountShell(ArtifactsPage);
    expect(w.text()).toContain("产物验收");
    expect(w.findAll("[data-test='artifact-item']")).toHaveLength(
      artifacts.length
    );
    const text = w.text();
    expect(text).toContain("高保数据模块");
    expect(text).toContain("v1.0.0");
    expect(text).toContain("测试通过");
    expect(text).toContain("待审批");
    expect(text).toContain("已驳回");
  });
});
