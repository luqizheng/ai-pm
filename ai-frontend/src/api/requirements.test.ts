/**
 * S02-I1 前端适配器单测：后端读模型 → requirements/requirement-entry.ts UI 读模型。
 * 重点是状态/优先级映射（S02D-04/S02D-05 词表对齐）与「诚实缺位」空占位。
 */

import { describe, expect, it } from "vitest";
import {
  toCriteria,
  toEntryFromListItem,
  toRequirementEntry,
  toSourceItems,
  type RequirementCoreConfirmation,
  type RequirementCoreDetail,
  type RequirementCoreEntry,
  type RequirementCoreRevision,
  type RequirementCoreSourceInput
} from "./requirements";

function entry(over: Partial<RequirementCoreEntry> = {}): RequirementCoreEntry {
  return {
    id: "req-1",
    project_id: "p1",
    code: "REQ-001",
    title: "User authentication",
    requirement_type: "functional",
    priority: "normal",
    lifecycle_status: "active",
    created_at: "2026-11-05T10:00:00Z",
    updated_at: "2026-11-05T11:00:00Z",
    row_revision: 2,
    current_revision_id: null,
    current_version: null,
    current_content_hash: null,
    latest_revision_id: "rev-1",
    latest_version: 1,
    latest_state: "draft",
    latest_submitted_at: null,
    ...over
  };
}

function revision(
  over: Partial<RequirementCoreRevision> = {}
): RequirementCoreRevision {
  return {
    id: "rev-1",
    requirement_id: "req-1",
    version: 1,
    state: "draft",
    markdown: "# body",
    summary: "初始拆解",
    content_hash: "hash-1",
    markdown_version_ref: null,
    base_current_revision_id: null,
    base_row_revision: null,
    submitted_at: null,
    created_by: "principal-1",
    created_by_name: "林悦",
    approved_by: null,
    approved_by_name: null,
    created_at: "2026-11-05T10:00:00Z",
    ...over
  };
}

function confirmation(
  over: Partial<RequirementCoreConfirmation> = {}
): RequirementCoreConfirmation {
  return {
    id: "conf-1",
    revision_id: "rev-1",
    action: "confirm",
    reason: null,
    human_principal_id: "principal-2",
    human_display_name: "Approver 甲",
    source: "rest",
    content_hash: "hash-1",
    markdown_version_ref: null,
    confirmed_at: "2026-11-05T12:00:00Z",
    ...over
  };
}

function detail(
  e: RequirementCoreEntry,
  revisions: RequirementCoreRevision[],
  confirmations: RequirementCoreConfirmation[] = []
): RequirementCoreDetail {
  return { requirement: e, revisions, confirmations };
}

describe("api/requirements 适配器", () => {
  it("状态映射：approved→confirmed / pending_confirmation→pending / 其余→draft（S02D-04 词表）", () => {
    expect(
      toEntryFromListItem(entry({ latest_state: "approved" })).status
    ).toBe("confirmed");
    expect(
      toEntryFromListItem(entry({ latest_state: "pending_confirmation" }))
        .status
    ).toBe("pending_confirmation");
    expect(toEntryFromListItem(entry({ latest_state: "draft" })).status).toBe(
      "draft"
    );
    expect(toEntryFromListItem(entry({ latest_state: null })).status).toBe(
      "draft"
    );
    // rejected/superseded 的最新修订不产生 UI 误报，回落 draft
    expect(
      toEntryFromListItem(entry({ latest_state: "rejected" })).status
    ).toBe("draft");
  });

  it("优先级映射：后端 normal → UI medium（S02D-05）", () => {
    expect(toEntryFromListItem(entry({ priority: "normal" })).priority).toBe(
      "medium"
    );
    expect(toEntryFromListItem(entry({ priority: "high" })).priority).toBe(
      "high"
    );
    expect(toEntryFromListItem(entry({ priority: "low" })).priority).toBe(
      "low"
    );
  });

  it("详情适配：修订按 version 倒序、决策映射显示名、描述取 v1 summary", () => {
    const d = detail(
      entry({
        current_revision_id: "rev-2",
        current_version: 2,
        latest_revision_id: "rev-2",
        latest_version: 2,
        latest_state: "approved"
      }),
      [
        revision({ id: "rev-1", version: 1, state: "superseded" }),
        revision({
          id: "rev-2",
          version: 2,
          state: "approved",
          summary: "补充锁定策略"
        })
      ],
      [
        confirmation({ revision_id: "rev-1", action: "confirm" }),
        confirmation({ revision_id: "rev-2", action: "confirm" })
      ]
    );
    const ui = toRequirementEntry(d);

    expect(ui.revisions.map(r => r.version)).toEqual([2, 1]);
    expect(ui.status).toBe("confirmed");
    expect(ui.description).toBe("初始拆解"); // v1 summary（创建时 description 落投影）
    expect(ui.revisions[0].decision).toEqual({
      kind: "confirmed",
      actor: "Approver 甲",
      acted_at: "2026-11-05T12:00:00Z"
    });
    expect(ui.revisions[0].author).toBe("林悦");
    expect(ui.revisions[1].decision?.kind).toBe("confirmed");
  });

  it("决策缺显示名时回退 principal_id；reject 映射 rejected", () => {
    const d = detail(
      entry(),
      [revision()],
      [
        confirmation({
          action: "reject",
          reason: "不可测",
          human_display_name: null,
          human_principal_id: "principal-9"
        })
      ]
    );
    const ui = toRequirementEntry(d);
    expect(ui.revisions[0].decision).toEqual({
      kind: "rejected",
      actor: "principal-9",
      acted_at: "2026-11-05T12:00:00Z"
    });
  });

  it("诚实缺位：I3/I5 域与 AI 编排域为空数组占位，origin 缺省", () => {
    const ui = toRequirementEntry(detail(entry(), [revision()]));
    expect(ui.specification).toEqual({
      business_rationale: "",
      expected_outcome: "",
      scope: "",
      explicit_exclusions: ""
    });
    expect(ui.acceptance_criteria).toEqual([]);
    // sources 由页根单独调 listRequirementSources + toSourceItems 补齐（S02-I3）
    expect(ui.sources).toEqual([]);
    expect(ui.related).toEqual([]);
    expect(ui.analysis).toEqual([]);
    expect(ui.ai_suggestions).toEqual([]);
    expect(ui.revisions[0].origin).toBeUndefined();
  });

  it("列表行适配：修订链为空，status/优先级同口径", () => {
    const ui = toEntryFromListItem(
      entry({ latest_state: "pending_confirmation", priority: "high" })
    );
    expect(ui.revisions).toEqual([]);
    expect(ui.status).toBe("pending_confirmation");
    expect(ui.priority).toBe("high");
  });
});

function sourceInput(
  over: Partial<RequirementCoreSourceInput>
): RequirementCoreSourceInput {
  return {
    id: "src-1",
    project_id: "p1",
    input_kind: "human_input",
    title: "用户访谈纪要",
    content: "用户希望支持邮箱登录",
    source_ref: "MIN-004",
    created_by: "principal-1",
    created_at: "2026-11-07T09:00:00Z",
    ...over
  };
}

describe("api/requirements 来源适配（S02-I3，2026-11-08 接线）", () => {
  it("human_input 映射为 UI user_request；content/ref 诚实映射（空串/缺省）", () => {
    const items = toSourceItems([sourceInput({})]);
    expect(items[0]).toEqual({
      id: "src-1",
      kind: "user_request",
      title: "用户访谈纪要",
      detail: "用户希望支持邮箱登录",
      ref: "MIN-004"
    });
  });

  it("I3 后端词表透传不吞语义；content 缺省落空串、ref 缺省 undefined", () => {
    const items = toSourceItems([
      sourceInput({
        id: "s2",
        input_kind: "intake",
        content: null,
        source_ref: null
      }),
      sourceInput({ id: "s3", input_kind: "external_feedback" }),
      sourceInput({ id: "s4", input_kind: "import" }),
      sourceInput({ id: "s5", input_kind: "discovery" })
    ]);
    expect(items.map(item => item.kind)).toEqual([
      "intake",
      "external_feedback",
      "import",
      "discovery"
    ]);
    expect(items[0].detail).toBe("");
    expect(items[0].ref).toBeUndefined();
  });

  it("正文映射：正文取当前正式版本，无正式版本回落最新修订", () => {
    const approved = toRequirementEntry(
      detail(
        entry({
          current_revision_id: "rev-2",
          current_version: 2,
          latest_revision_id: "rev-2",
          latest_version: 2,
          latest_state: "approved"
        }),
        [
          revision({ id: "rev-1", version: 1, state: "superseded" }),
          revision({ id: "rev-2", version: 2, state: "approved" })
        ]
      )
    );
    expect(approved.body_markdown).toBe("# body");

    const draftOnly = toRequirementEntry(detail(entry(), [revision()]));
    expect(draftOnly.body_markdown).toBe("# body");

    const empty = toEntryFromListItem(entry());
    expect(empty.body_markdown).toBeUndefined();
  });
});

describe("api/requirements 验收标准适配（S02-I5，2026-11-08 接线）", () => {
  it("字段直传不吞语义；verdict_note/at 缺省 undefined；verdict 透传", () => {
    const items = toCriteria([
      {
        id: "ac-1",
        requirement_id: "r1",
        position: 1,
        statement: "P95 ≤ 300ms",
        verification_kind: "performance",
        params: { threshold: 300 },
        established_in_revision_id: null,
        verdict: "passed",
        verdict_note: "压测报告 PR-9",
        verdict_by: "p2",
        verdict_at: "2026-11-08T10:00:00Z",
        created_at: "2026-11-08T09:00:00Z",
        updated_at: "2026-11-08T10:00:00Z"
      },
      {
        id: "ac-2",
        requirement_id: "r1",
        position: 2,
        statement: "越权访问零容忍",
        verification_kind: "security",
        params: {},
        established_in_revision_id: null,
        verdict: "pending",
        verdict_note: null,
        verdict_by: null,
        verdict_at: null,
        created_at: "2026-11-08T09:00:00Z",
        updated_at: "2026-11-08T09:00:00Z"
      }
    ]);
    expect(items[0]).toEqual({
      id: "ac-1",
      statement: "P95 ≤ 300ms",
      verification_kind: "performance",
      params: { threshold: 300 },
      verdict: "passed",
      verdict_note: "压测报告 PR-9",
      verdict_at: "2026-11-08T10:00:00Z",
      position: 1
    });
    expect(items[1]).toEqual({
      id: "ac-2",
      statement: "越权访问零容忍",
      verification_kind: "security",
      params: {},
      verdict: "pending",
      verdict_note: undefined,
      verdict_at: undefined,
      position: 2
    });
  });
});
