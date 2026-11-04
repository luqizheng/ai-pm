// @vitest-environment happy-dom
/**
 * S01-04 认知页行为测试。
 *
 * 断言 UX 硬语义：
 *   - 无仓库不阻断基础认知
 *   - 批量确认不包含有冲突的条目
 *   - Run 完成不等于全部认知已确认
 *   - 证据类型（代码事实 / AI 推断）分别标注
 */
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import { setLocale } from "@/shell/i18n";
import CognitionView from "./CognitionView.vue";
import type {
  CognitionItem,
  CognitionPreparation,
  CognitionRun,
  ProjectKnowledge
} from "@/api/types/initialization";

const mocks = vi.hoisted(() => ({
  decide: vi.fn(),
  startRun: vi.fn(),
  setCore: vi.fn()
}));

const preparation: CognitionPreparation = {
  project_info_complete: true,
  missing_project_fields: [],
  repositories: [],
  collected_count: 12,
  missing_materials: [],
  analysis_scope: ["project_identity", "repository_understanding"]
};

const runs: CognitionRun[] = [
  {
    id: "run-1",
    project_id: "p1",
    status: "completed",
    scope: ["project_identity"],
    repository_baselines: [],
    inputs: [],
    capability_version: null,
    item_count: 4,
    confirmed_count: 1,
    error_message: null,
    started_at: "2026-11-04T10:00:00Z",
    completed_at: "2026-11-04T10:05:00Z",
    created_at: "2026-11-04T10:00:00Z"
  }
];

function item(overrides: Partial<CognitionItem> = {}): CognitionItem {
  return {
    id: "item-1",
    run_id: "run-1",
    domain: "project_identity",
    content: "项目以任务编排为核心",
    evidence: [
      {
        source_type: "code_fact",
        label: "task.rs",
        detail: null,
        locator: "crates/pm-domain/src/task.rs:1"
      }
    ],
    evidence_type: "code_fact",
    confidence: 0.82,
    has_conflict: false,
    status: "proposed",
    change_type: "add",
    related_knowledge_id: null,
    ai_rationale: "从模块结构推断",
    decided_by: null,
    decided_at: null,
    decision_note: null,
    created_at: "2026-11-04T10:00:00Z",
    ...overrides
  };
}

const items: CognitionItem[] = [
  item(),
  item({ id: "item-2", has_conflict: true, content: "存在冲突的认知" })
];

const knowledge: ProjectKnowledge[] = [
  {
    id: "k1",
    project_id: "p1",
    category: "architecture",
    content: "任务编排为核心模块",
    source_references: [],
    confirmed_by: "u1",
    confirmed_by_name: "张伟",
    confirmed_at: "2026-11-04T11:00:00Z",
    version: 1,
    status: "active",
    supersedes: null,
    is_core: true,
    created_at: "2026-11-04T11:00:00Z"
  }
];

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: { projectId: "p1" } }),
  useRouter: () => ({ push: vi.fn() })
}));

vi.mock("@/api/initialization", () => ({
  getCognitionPreparation: vi.fn(async () => preparation),
  listCognitionRuns: vi.fn(async () => ({
    items: runs,
    total: 1,
    page: 1,
    size: 50
  })),
  listCognitionItems: vi.fn(async () => ({
    items,
    total: 2,
    page: 1,
    size: 100
  })),
  listProjectKnowledge: vi.fn(async () => ({
    items: knowledge,
    total: 1,
    page: 1,
    size: 100
  })),
  getAgentContext: vi.fn(async () => ({
    base_context: { knowledge },
    task_context: { knowledge: [] },
    repository_context: {
      repository_id: null,
      branch: null,
      commit: null,
      knowledge: []
    },
    execution_context: { run_id: null, task_id: null, state: "idle" },
    generated_at: "2026-11-04T12:00:00Z"
  })),
  decideCognitionItem: mocks.decide,
  startCognitionRun: mocks.startRun,
  setCoreKnowledge: mocks.setCore
}));

vi.mock("@/api/format", () => ({ formatDateTime: (value: string) => value }));

async function mountView() {
  // i18n 是全局单例：其他测试文件可能切到 en，这里固定回 zh-CN
  setLocale("zh-CN");
  const wrapper = mountShell(CognitionView);
  await nextTick();
  await nextTick();
  await nextTick();
  return wrapper;
}

describe("CognitionView", () => {
  it("没有仓库时仍可进行基础认知分析，不阻断", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("没有仓库也可以进行基础认知分析");
    expect(wrapper.text()).toContain("启动正式分析");
  });

  it("Run 完成不等于全部认知已确认——分别展示条目数与确认进度", async () => {
    const wrapper = await mountView();
    // 切到「执行」页签
    const tabs = wrapper.findAll("button");
    const runsTab = tabs.find(button => button.text() === "认知执行");
    await runsTab?.trigger("mousedown");
    await nextTick();
    expect(wrapper.text()).toContain("1 / 4");
  });

  it("证据类型分别标注：代码事实与 AI 推断不混排", async () => {
    const wrapper = await mountView();
    const reviewTab = wrapper
      .findAll("button")
      .find(button => button.text() === "逐条审核");
    await reviewTab?.trigger("mousedown");
    await nextTick();
    expect(wrapper.text()).toContain("代码事实");
  });

  it("存在冲突的条目不提供批量勾选", async () => {
    const wrapper = await mountView();
    const reviewTab = wrapper
      .findAll("button")
      .find(button => button.text() === "逐条审核");
    await reviewTab?.trigger("mousedown");
    await nextTick();
    // 两条 proposed 中只有无冲突的一条带勾选框
    expect(wrapper.findAll('input[type="checkbox"]').length).toBe(1);
  });
});
