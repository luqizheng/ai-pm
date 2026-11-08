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
});
