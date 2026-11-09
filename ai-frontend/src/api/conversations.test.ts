import { beforeEach, describe, expect, it, vi } from "vitest";

// 会话 REST 接线批（2026-11-08）：http 层整体 mock，只测
// 「方法 + URL + body/query 透传」的 api 契约（D-4 后端 0060）。
const httpGet = vi.fn();
const httpPost = vi.fn();

vi.mock("@/api/base", () => ({
  http: {
    get: (...args: unknown[]) => httpGet(...args),
    post: (...args: unknown[]) => httpPost(...args)
  }
}));

import {
  appendMessage,
  createConversation,
  listConversations,
  listMessages
} from "./conversations";

describe("conversations api（D-4 会话 REST）", () => {
  beforeEach(() => {
    httpGet.mockReset();
    httpPost.mockReset();
    httpGet.mockResolvedValue([]);
    httpPost.mockResolvedValue({});
  });

  it("createConversation: POST /v1/pm/projects/{id}/conversations，body 透传", async () => {
    await createConversation("p1", { title: "demo" });
    expect(httpPost).toHaveBeenCalledTimes(1);
    expect(httpPost).toHaveBeenCalledWith("/v1/pm/projects/p1/conversations", {
      title: "demo"
    });
  });

  it("createConversation: body 可整体缺省（后端 title 可空）", async () => {
    await createConversation("p1");
    expect(httpPost).toHaveBeenCalledWith(
      "/v1/pm/projects/p1/conversations",
      {}
    );
  });

  it("createConversation: context 锚点随 body 透传（Batch 2）", async () => {
    await createConversation("p1", {
      title: "带上下文",
      context: {
        project_id: "p1",
        project_name: "Atlas Platform",
        repository_name: "atlas-platform",
        default_branch: "main",
        repository_role: "main"
      }
    });
    expect(httpPost).toHaveBeenCalledWith(
      "/v1/pm/projects/p1/conversations",
      {
        title: "带上下文",
        context: {
          project_id: "p1",
          project_name: "Atlas Platform",
          repository_name: "atlas-platform",
          default_branch: "main",
          repository_role: "main"
        }
      }
    );
  });

  it("listConversations: GET 本人会话列表（无 query）", async () => {
    await listConversations("p1");
    expect(httpGet).toHaveBeenCalledTimes(1);
    expect(httpGet).toHaveBeenCalledWith("/v1/pm/projects/p1/conversations");
  });

  it("appendMessage: POST messages，body {content, content_type?} 透传（role 由服务端恒置 user）", async () => {
    await appendMessage("p1", "c1", { content: "hello" });
    expect(httpPost).toHaveBeenCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      { content: "hello" }
    );

    await appendMessage("p1", "c1", {
      content: "plain",
      content_type: "text"
    });
    expect(httpPost).toHaveBeenLastCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      { content: "plain", content_type: "text" }
    );
  });

  it("listMessages: before/limit 透传为 query（keyset 游标 + clamp [1,500] 由服务端执行）", async () => {
    await listMessages("p1", "c1", {
      before: "2026-11-08T00:00:00Z",
      limit: 200
    });
    expect(httpGet).toHaveBeenCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      { before: "2026-11-08T00:00:00Z", limit: 200 }
    );

    await listMessages("p1", "c1");
    expect(httpGet).toHaveBeenLastCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      {}
    );
  });

  it("listMessages: after/after_id 增量游标透传为 query（Batch G 轮询契约）", async () => {
    await listMessages("p1", "c1", {
      after: "2026-11-08T10:00:00Z",
      after_id: "m1",
      limit: 500
    });
    expect(httpGet).toHaveBeenCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      { after: "2026-11-08T10:00:00Z", after_id: "m1", limit: 500 }
    );
  });

  it("listMessages: latest=true 首屏最新窗口透传为 query（Batch H，服务端 DESC 取 limit 再反转 ASC）", async () => {
    await listMessages("p1", "c1", { latest: true, limit: 200 });
    expect(httpGet).toHaveBeenCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      { latest: true, limit: 200 }
    );

    // 缺省不携带（服务端 latest=false 旧语义）。
    await listMessages("p1", "c1", { limit: 500 });
    expect(httpGet).toHaveBeenLastCalledWith(
      "/v1/pm/projects/p1/conversations/c1/messages",
      { limit: 500 }
    );
  });
});
