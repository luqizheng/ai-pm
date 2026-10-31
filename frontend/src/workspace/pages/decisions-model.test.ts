import { describe, expect, it } from "vitest";
import type { DecisionRecord } from "@/api/workspace-view";
import { evidenceText, principalNameMap, toDecision } from "./decisions-model";

const texts = {
  approvalTitle: "approval",
  noRationale: "no-rationale",
  requiredRole: (role: string) => `role:${role}`,
  riskLabel: (level: string) => `risk:${level}`,
  formatDecidedAt: (value: string) => `at:${value.slice(0, 10)}`
};

function record(overrides: Partial<DecisionRecord> = {}): DecisionRecord {
  return {
    id: "a1",
    project_id: "p1",
    status: "pending",
    required_role: "project_owner",
    decided_by: null,
    decision_reason: null,
    decided_at: null,
    created_at: "2026-10-31T00:00:00Z",
    proposal_id: "prop1",
    proposal_status: "redline_pending",
    target_type: "task",
    command_type: "update_task",
    rationale: "split task to reduce risk",
    risk_level: "high",
    evidence: ["doc:REQ-1", { kind: "commit", title: "abc123" }, { summary: "perf" }],
    proposed_data: {},
    analysis_run_id: "run1",
    ...overrides
  };
}

describe("toDecision（真模式决策卡）", () => {
  it("审批记录 + 变更提案映射为决策卡，不含候选方案与推荐理由", () => {
    const decision = toDecision(record(), { u9: "Carol" }, texts);
    expect(decision.title).toBe("update_task · task");
    expect(decision.question).toBe("split task to reduce risk");
    expect(decision.status).toBe("pending");
    expect(decision.evidence).toEqual(["doc:REQ-1", "abc123", "perf"]);
    expect(decision.risks).toEqual(["risk:high"]);
    expect(decision.impact).toBe("role:project_owner");
    expect(decision.options).toEqual([]);
    expect(decision.dimensions).toEqual([]);
    expect(decision.recommendation).toBe("");
    expect(decision.record?.id).toBe("a1");
  });

  it("已决策记录展示决策人、时间与结论", () => {
    const decision = toDecision(
      record({
        status: "approved",
        decided_by: "u9",
        decision_reason: "ok",
        decided_at: "2026-10-31T08:30:00Z"
      }),
      { u9: "Carol" },
      texts
    );
    expect(decision.status).toBe("decided");
    expect(decision.decidedBy).toBe("Carol");
    expect(decision.approvedAt).toBe("at:2026-10-31");
    expect(decision.conclusion).toBe("ok");
  });

  it("提案缺失（左连接为空）时回落默认标题与空证据", () => {
    const decision = toDecision(
      record({
        proposal_id: null,
        proposal_status: null,
        target_type: null,
        command_type: null,
        rationale: null,
        risk_level: null,
        evidence: null
      }),
      {},
      texts
    );
    expect(decision.title).toBe("approval");
    expect(decision.question).toBe("no-rationale");
    expect(decision.background).toBe("");
    expect(decision.evidence).toEqual([]);
    expect(decision.risks).toEqual([]);
    expect(decision.impact).toBe("role:project_owner");
  });
});

describe("evidenceText / principalNameMap", () => {
  it("证据支持字符串与对象形态，无法识别的对象给空串", () => {
    expect(evidenceText("plain")).toBe("plain");
    expect(evidenceText({ title: "T" })).toBe("T");
    expect(evidenceText({ kind: "commit" })).toBe("commit");
    expect(evidenceText({ foo: 1 })).toBe("");
    expect(evidenceText(null)).toBe("");
  });

  it("principal 列表折成 id → display_name", () => {
    expect(
      principalNameMap([
        {
          id: "u9",
          kind: "human",
          display_name: "Carol",
          is_active: true,
          created_at: "2026-10-31T00:00:00Z"
        }
      ])
    ).toEqual({ u9: "Carol" });
  });
});