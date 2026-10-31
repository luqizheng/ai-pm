// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import GoalsPage from "./GoalsPage.vue";
import ExecutionPage from "./ExecutionPage.vue";
import TeamPage from "./TeamPage.vue";
import ActivityPage from "./ActivityPage.vue";
import RequirementsPage from "./RequirementsPage.vue";
import DecisionsPage from "./DecisionsPage.vue";
import KnowledgePage from "./KnowledgePage.vue";
import ArtifactsPage from "./ArtifactsPage.vue";
import { mountShell } from "../../shell/test-utils";
import {
  agentRun,
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

describe("GoalsPage（目标计划 V1.2）", () => {
  it("渲染标题与全部目标卡（编码 + 进度依据 + 里程碑 + 任务树）", () => {
    const w = mountShell(GoalsPage);
    expect(w.text()).toContain("目标计划");
    expect(w.findAll("[data-test='goal-card']")).toHaveLength(goals.length);
    const text = w.text();
    expect(text).toContain("GOAL-001");
    expect(text).toContain("基于计划权重");
    expect(text).toContain("架构评审通过");
    expect(text).toContain("目标帧率与视距范围定义");
    expect(
      w.findAll("[data-test='milestone-item']").length
    ).toBeGreaterThanOrEqual(7);
    expect(w.findAll("[data-test='task-item']").length).toBeGreaterThanOrEqual(
      8
    );
  });

  it("三视图切换：目标树 / 依赖图 / 时间线（V1.2）", async () => {
    const w = mountShell(GoalsPage);
    // 默认目标树：依赖块可见
    expect(w.find("[data-test='goal-dependencies']").exists()).toBe(true);
    // 依赖图视图
    await w.find("[data-test='view-deps']").trigger("click");
    expect(w.findAll("[data-test='dep-chain']")).toHaveLength(goals.length);
    expect(w.text()).toContain("系统集成测试");
    // 时间线视图
    await w.find("[data-test='view-timeline']").trigger("click");
    expect(
      w.findAll("[data-test='timeline-item']").length
    ).toBeGreaterThanOrEqual(8);
  });

  it("依赖块提供「查看阻塞的架构决策」入口（V1.2 原型）", () => {
    const w = mountShell(GoalsPage);
    const cta = w.find("[data-test='view-blocked-decision']");
    expect(cta.exists()).toBe(true);
    expect(cta.text()).toContain("查看阻塞的架构决策");
  });

  it("AI 提议的计划（草案）与已批准计划分离（V1.2）", () => {
    const w = mountShell(GoalsPage);
    expect(w.find("[data-test='proposed-tasks']").exists()).toBe(true);
    expect(w.text()).toContain("AI 提议的计划（草案，未批准不执行）");
    expect(w.text()).toContain("新增：视距三档性能采样任务");
  });
});

describe("ExecutionPage（AI 执行中心 V1.2）", () => {
  it("阶段状态 + 检查点 + 剩余工作（不用估算百分比）", () => {
    const w = mountShell(ExecutionPage);
    expect(w.text()).toContain("AI 执行中心");
    expect(w.text()).toContain(agentRun.phase);
    expect(w.text()).toContain("已完成检查点");
    expect(w.text()).toContain("剩余工作");
    expect(w.text()).toContain("性能回归测试（gt-6）");
  });

  it("双栏：左步骤区分四类事件，右详情含工具调用/日志/代码变化", async () => {
    const w = mountShell(ExecutionPage);
    const steps = w.findAll("[data-test='run-step']");
    expect(steps).toHaveLength(agentRun.steps.length);
    const text = w.text();
    // 四类事件徽标（V1.2：不混在聊天气泡里）
    expect(text).toContain("AI 思考摘要");
    expect(text).toContain("工具执行");
    expect(text).toContain("业务状态变化");
    expect(text).toContain("人工审批请求");
    // 右侧详情默认显示选中步骤
    expect(w.find("[data-test='step-detail']").text()).toContain(
      agentRun.steps[0].title
    );
    // 点工具步骤 → 工具调用与日志
    const toolStep = steps.find(el => el.text().includes("读取生成配置模板"));
    await toolStep!.trigger("click");
    expect(w.find("[data-test='step-tool']").text()).toContain("read_file");
    // 日志在 run_tests 步骤（step-4）
    const runStep = steps.find(el => el.text().includes("运行平原用例"));
    await runStep!.trigger("click");
    expect(w.find("[data-test='step-logs']").exists()).toBe(true);
    // 点业务状态变化步骤 → 代码变化
    const stateStep = steps.find(el =>
      el.text().includes("分布逻辑主实现合入工作区")
    );
    await stateStep!.trigger("click");
    expect(w.find("[data-test='step-codechange']").text()).toContain(
      "gen/forest.rs"
    );
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

describe("DecisionsPage（决策中心 V1.2）", () => {
  it("渲染完整决策记录：背景/维度/推荐/风险/决策人/验证", () => {
    const w = mountShell(DecisionsPage);
    expect(w.text()).toContain("决策中心");
    expect(w.findAll("[data-test='decision-card']")).toHaveLength(
      decisions.length
    );
    const text = w.text();
    expect(text).toContain("问题背景");
    expect(text).toContain("比较维度");
    expect(text).toContain("AI 推荐理由");
    expect(text).toContain("风险");
    expect(text).toContain("决策人");
    expect(text).toContain("批准时间");
    expect(text).toContain("后续验证结果");
    expect(text).toContain("采用分块 tile 版本");
  });

  it("维度对照表 + 重新评估提示（V1.2 核心竞争力）", () => {
    const w = mountShell(DecisionsPage);
    expect(w.findAll("[data-test='decision-dimensions']")).toHaveLength(
      decisions.length
    );
    // 系统主动提示重新评估（不让 Agent 按旧决策执行）
    const alerts = w.findAll("[data-test='decision-reassess']");
    expect(alerts.length).toBe(1);
    expect(alerts[0].text()).toContain("重新评估");
    expect(alerts[0].text()).toContain("REQ-105");
  });
});

describe("KnowledgePage（项目认知 V1.2 三层）", () => {
  it("知识树 | 阅读器 | 证据与引用关系 三层布局", async () => {
    const w = mountShell(KnowledgePage);
    expect(w.text()).toContain("项目知识");
    expect(w.find("[data-test='doc-evidence']").exists()).toBe(true);
    expect(w.find("[data-test='doc-reader']").text()).toContain(
      knowledgeDocs[0].title
    );
    // 默认文档：审核状态 + 人工批准事实 + AI 摘要 + 引用
    const reader = w.find("[data-test='doc-reader']");
    expect(reader.text()).toContain("人工批准的正式事实");
    expect(reader.text()).toContain("AI 维护摘要");
    expect(
      w.findAll("[data-test='doc-reference']").length
    ).toBeGreaterThanOrEqual(2);
    // 点击树切换
    const nodes = w.findAll("[data-test='doc-node']");
    const target = nodes.find(n => n.text() === "迭代 2.0 需求文档.md");
    await target!.trigger("click");
    expect(w.find("[data-test='doc-reader']").text()).toContain(
      "迭代 2.0 需求文档"
    );
    expect(w.find("[data-test='doc-review']").text()).toContain("已批准");
  });
});

describe("ArtifactsPage（产物验收 V1.2 主从视图）", () => {
  it("产物清单可选，详情含判定横幅/Diff/评论/测试报告/验收标准", async () => {
    const w = mountShell(ArtifactsPage);
    expect(w.text()).toContain("产物验收");
    const items = w.findAll("[data-test='artifact-item']");
    expect(items).toHaveLength(artifacts.length);
    // 默认选中首个：高保数据模块
    expect(w.find("[data-test='artifact-verdict']").text()).toContain(
      "部分检查通过"
    );
    expect(w.find("[data-test='artifact-diff']").text()).toContain(
      "check_boundary"
    );
    expect(w.findAll("[data-test='artifact-comments']").length).toBeGreaterThan(
      0
    );
    expect(w.find("[data-test='artifact-tests']").text()).toContain(
      "性能验证 · 大场景导入"
    );
    expect(w.findAll("[data-test='artifact-criteria'] li").length).toBe(3);
    // 切换到已驳回的森林生成原型 → 未达验收标准
    const forest = items.find(el => el.text().includes("森林生成原型"));
    await forest!.trigger("click");
    expect(w.find("[data-test='artifact-verdict']").text()).toContain(
      "未达验收标准"
    );
    // 切换到 Alpha → 符合验收标准
    const alpha = items.find(el => el.text().includes("世界地图编辑器 Alpha"));
    await alpha!.trigger("click");
    expect(w.find("[data-test='artifact-verdict']").text()).toContain(
      "符合验收标准"
    );
  });
});
