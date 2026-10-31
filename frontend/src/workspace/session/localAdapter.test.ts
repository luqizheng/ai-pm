// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import type { WorkspaceSpec } from "../spec/types";
import { localSessionAdapter } from "./localAdapter";

function spec(id: string): WorkspaceSpec {
  return {
    schemaVersion: "1.0",
    workspaceId: id,
    projectId: "proj-1",
    layout: "overview",
    panels: [{ component: "BriefingCard" }]
  };
}

describe("local session adapter", () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
  });

  it("save 后 load 返回同一 spec", () => {
    localSessionAdapter.save("proj-1", spec("ws-1"));
    expect(localSessionAdapter.load("proj-1")?.workspaceId).toBe("ws-1");
  });

  it("history 上限 10 且最新在前", () => {
    for (let i = 0; i < 12; i += 1) {
      localSessionAdapter.save("proj-1", spec(`ws-${i}`));
    }
    const history = localSessionAdapter.history("proj-1");
    expect(history).toHaveLength(10);
    expect(history[0].workspaceId).toBe("ws-11");
  });

  it("两个标签（不同 session id）互不覆盖", () => {
    localSessionAdapter.save("proj-1", spec("tab-a"));
    sessionStorage.clear(); // 模拟新标签页
    localSessionAdapter.save("proj-1", spec("tab-b"));

    expect(localSessionAdapter.load("proj-1")?.workspaceId).toBe("tab-b");
    const firstTab = sessionStorage.getItem("ai-pm:workspace:session");
    expect(firstTab).toBeTruthy();
  });

  it("损坏的快照返回 undefined 而不是抛错", () => {
    const sessionId = "sess-test";
    sessionStorage.setItem("ai-pm:workspace:session", sessionId);
    localStorage.setItem(
      `ai-pm:workspace:snapshot:proj-1:${sessionId}`,
      "{not json"
    );
    expect(localSessionAdapter.load("proj-1")).toBeUndefined();
  });
});