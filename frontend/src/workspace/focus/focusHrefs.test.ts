import { describe, expect, it } from "vitest";
import { createFocusHref, returnHref } from "./focusHrefs";

describe("focus hrefs", () => {
  it("createFocusHref 带 from 与 anchor", () => {
    const href = createFocusHref("proj-1", "requirement", "R-101", {
      from: "/workspace/proj-1/requirements?status=active",
      anchor: "#rev-3"
    });
    expect(href).toContain("/workspace/proj-1/focus/requirement/R-101");
    expect(href).toContain("from=");
    expect(href).toContain("anchor=");
  });

  it("returnHref 回到同项目来源页", () => {
    expect(
      returnHref("/workspace/proj-1/requirements?status=active", "proj-1")
    ).toBe("/workspace/proj-1/requirements?status=active");
  });

  it("跨项目或缺失来源回落到总控台", () => {
    expect(returnHref("/workspace/other/requirements", "proj-1")).toBe(
      "/workspace/proj-1/console"
    );
    expect(returnHref(undefined, "proj-1")).toBe("/workspace/proj-1/console");
    // 来源本身是 focus 时不回环
    expect(returnHref("/workspace/proj-1/focus/task/T-1", "proj-1")).toBe(
      "/workspace/proj-1/console"
    );
  });
});