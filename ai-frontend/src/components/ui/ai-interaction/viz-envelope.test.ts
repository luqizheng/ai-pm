// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import { toInteractionEnvelope } from "@/composables/useAssistantConversation";
import { PAYLOAD_REQUIRED_FIELDS, RESPONSE_TYPES } from "./types";

/**
 * 第 9 类信封 `viz_artifact` 的识别断言。
 *
 * 关键：**必须断言「被识别为 viz-artifact 类型」，而不是「没报错」**——
 * 白名单未登记时信封会静默退化为纯文本（fail-closed），不抛错，
 * 因此「不报错」不能证明集成正确（16 号设计 §7 的验收要求）。
 */
function envelope(payload: Record<string, unknown>): unknown {
  return {
    envelope: {
      id: "env-1",
      type: "viz-artifact",
      status: "presented",
      identity: { agentName: "AI", agentRole: "ai_pm" },
      createdAt: "2026-11-12T00:00:00Z",
      payload
    }
  };
}

const validPayload = {
  artifactId: "artifact-1",
  slotKey: "req-trace:R-042",
  previewUrl: "blob:mock"
};

describe("viz-artifact 信封", () => {
  it("类型已登记进运行时白名单", () => {
    expect(RESPONSE_TYPES).toContain("viz-artifact");
  });

  it("必填字段已登记（漏登记即破 fail-closed）", () => {
    expect(PAYLOAD_REQUIRED_FIELDS["viz-artifact"]).toEqual([
      "artifactId",
      "slotKey",
      "previewUrl"
    ]);
  });

  it("合法信封被识别为 viz-artifact（而不是静默退化为纯文本）", () => {
    const parsed = toInteractionEnvelope(envelope(validPayload));
    expect(parsed).toBeDefined();
    expect(parsed?.type).toBe("viz-artifact");
    expect(parsed?.payload.artifactId).toBe("artifact-1");
  });

  it("缺必填字段时整信封拒收（fail-closed）", () => {
    for (const field of ["artifactId", "slotKey", "previewUrl"]) {
      const payload = { ...validPayload };
      delete (payload as Record<string, unknown>)[field];
      expect(toInteractionEnvelope(envelope(payload))).toBeUndefined();
    }
  });

  it("未登记类型被拒收", () => {
    const raw = envelope(validPayload) as {
      envelope: Record<string, unknown>;
    };
    raw.envelope.type = "viz-unknown";
    expect(toInteractionEnvelope(raw)).toBeUndefined();
  });
});
