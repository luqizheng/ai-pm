import { describe, expect, it } from "vitest";

import {
  aggregateStatus,
  deriveCurrentAction,
  deriveHandoverOutcome,
  projectStages,
  type StageStatus
} from "./initialization-overview";
import type {
  HumanActionSummary,
  InitializationProgress,
  InitItem,
  RiskItem
} from "./command-center";

/**
 * S01-01 投影层行为：状态聚合、阶段投影、Current Action 决策阶梯、交接判定。
 *
 * 这些规则是设计稿的硬约束，必须可复算：
 *   - 阻塞 > 等待人类 > 进行中 > 尚无法判断 > 未开始 > 完成/不适用；
 *   - 零仓库（不适用）不判失败；
 *   - 完成 + 不适用 视为完成；
 *   - 当前动作唯一，且阻塞优先于等待人类、等待人类优先于执行中。
 */

const EMPTY_ACTIONS: HumanActionSummary = { total: 0, items: [] };
const EMPTY_RISKS: RiskItem[] = [];

function progress(
  overrides: Partial<InitItem>[] = [],
  completion = "not_determinable"
): InitializationProgress {
  const base: InitItem[] = [
    {
      key: "project_basics",
      title: "项目基本信息",
      status: "completed",
      required: true
    },
    {
      key: "repository_access",
      title: "仓库接入",
      status: "not_started",
      required: false
    },
    {
      key: "code_analysis",
      title: "代码分析",
      status: "not_applicable",
      required: false
    },
    {
      key: "project_cognition",
      title: "项目认知",
      status: "not_started",
      required: false
    },
    {
      key: "dev_environment",
      title: "开发环境",
      status: "not_started",
      required: false
    },
    {
      key: "human_actions",
      title: "人工处理",
      status: "completed",
      required: true
    }
  ];

  const items = base.map(item => {
    const override = overrides.find(entry => entry.key === item.key);
    return override ? { ...item, ...override } : item;
  });

  return {
    items,
    required_items: ["project_basics", "human_actions"],
    completion,
    expanded: true
  };
}

const PENDING_ACTION: HumanActionSummary = {
  total: 1,
  items: [
    {
      kind: "finding_pending",
      domain: "discovery",
      id: "fnd-1",
      project_id: "p1",
      title: "待确认",
      source_state: "PENDING",
      action_hint: "确认",
      link: { kind: "discovery_finding", params: {} }
    }
  ]
};

const CONFLICT_RISK: RiskItem[] = [
  {
    kind: "conflict",
    domain: "discovery",
    source_id: "fnd-2",
    project_id: "p1",
    title: "冲突",
    source_state: "CONFLICT",
    severity: "high",
    suggestion: "裁决",
    link: { kind: "discovery_finding", params: {} }
  }
];

describe("aggregateStatus", () => {
  it("阻塞优先于其他一切状态", () => {
    expect(
      aggregateStatus(["in_progress", "blocked", "completed"] as StageStatus[])
    ).toBe("blocked");
  });

  it("等待人类优先于进行中", () => {
    expect(
      aggregateStatus(["in_progress", "waiting_for_human"] as StageStatus[])
    ).toBe("waiting_for_human");
  });

  it("完成与不适用混合时视为完成", () => {
    expect(
      aggregateStatus(["completed", "not_applicable"] as StageStatus[])
    ).toBe("completed");
  });

  it("全部不适用时不判失败", () => {
    expect(aggregateStatus(["not_applicable"] as StageStatus[])).toBe(
      "not_applicable"
    );
  });

  it("无输入时为尚无法判断", () => {
    expect(aggregateStatus([])).toBe("undetermined");
  });
});

describe("projectStages", () => {
  it("投影出六个阶段并保持阶段顺序", () => {
    const stages = projectStages(progress(), EMPTY_ACTIONS, EMPTY_RISKS);
    expect(stages).toHaveLength(6);
    expect(stages.map(stage => stage.order)).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("零仓库项目不判失败——仓库阶段为未开始", () => {
    const stages = projectStages(progress(), EMPTY_ACTIONS, EMPTY_RISKS);
    const repository = stages.find(stage => stage.id === "repository");
    expect(repository?.status).toBe("not_started");
  });

  it("基线阶段标注为前端合成", () => {
    const stages = projectStages(progress(), EMPTY_ACTIONS, EMPTY_RISKS);
    const baseline = stages.find(stage => stage.id === "baseline");
    expect(baseline?.source).toBe("derived");
  });

  it("有待确认事项时基线阶段为等待人类", () => {
    const stages = projectStages(
      progress([{ key: "project_cognition", status: "completed" }]),
      PENDING_ACTION,
      EMPTY_RISKS
    );
    const baseline = stages.find(stage => stage.id === "baseline");
    expect(baseline?.status).toBe("waiting_for_human");
  });

  it("存在冲突项时基线阶段为阻塞", () => {
    const stages = projectStages(
      progress([{ key: "project_cognition", status: "completed" }]),
      EMPTY_ACTIONS,
      CONFLICT_RISK
    );
    const baseline = stages.find(stage => stage.id === "baseline");
    expect(baseline?.status).toBe("blocked");
  });
});

describe("deriveCurrentAction", () => {
  it("阻塞事项优先于等待人类", () => {
    const current = progress([
      { key: "dev_environment", status: "blocked" },
      { key: "project_cognition", status: "completed" }
    ]);
    const stages = projectStages(current, PENDING_ACTION, EMPTY_RISKS);
    const action = deriveCurrentAction(stages, current, PENDING_ACTION);
    expect(action.status).toBe("blocked");
    expect(action.stageId).toBe("environment");
    expect(action.needsUser).toBe(true);
  });

  it("执行中的仓库分析归给 AI 且不要求用户操作", () => {
    const current = progress([
      { key: "repository_access", status: "in_progress" },
      { key: "code_analysis", status: "in_progress" }
    ]);
    const stages = projectStages(current, EMPTY_ACTIONS, EMPTY_RISKS);
    const action = deriveCurrentAction(stages, current, EMPTY_ACTIONS);
    expect(action.stageId).toBe("repository");
    expect(action.owner).toBe("ai");
    expect(action.needsUser).toBe(false);
  });

  it("无待办时交接到交接阶段", () => {
    const current = progress([
      { key: "repository_access", status: "completed" },
      { key: "code_analysis", status: "completed" },
      { key: "project_cognition", status: "completed" },
      { key: "dev_environment", status: "completed" }
    ]);
    const stages = projectStages(current, EMPTY_ACTIONS, EMPTY_RISKS);
    const action = deriveCurrentAction(stages, current, EMPTY_ACTIONS);
    expect(action.stageId).toBe("handover");
    expect(action.needsUser).toBe(false);
  });
});

describe("deriveHandoverOutcome", () => {
  it("后端未给出完成判定时不得宣称完成", () => {
    const stages = projectStages(progress(), EMPTY_ACTIONS, EMPTY_RISKS);
    expect(deriveHandoverOutcome(stages, "not_determinable")).toBe(
      "undetermined"
    );
  });

  it("存在阻塞时交接态为阻塞", () => {
    const current = progress([{ key: "dev_environment", status: "blocked" }]);
    const stages = projectStages(current, EMPTY_ACTIONS, EMPTY_RISKS);
    expect(deriveHandoverOutcome(stages, "not_determinable")).toBe("blocked");
  });
});
