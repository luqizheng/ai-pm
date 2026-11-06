// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";

import { buildAcceptUrl, invitationErrorKey } from "./team";

/**
 * api/team 纯逻辑契约：稳定错误 message → 结果态 key（Screen 07 七态 +
 * invalid 兜底）、accept_path → 完整 URL（V1 复制分享）。
 */
describe("invitationErrorKey", () => {
  it("maps stable backend messages to result states", () => {
    expect(invitationErrorKey("invitation_expired")).toBe("expired");
    expect(invitationErrorKey("invitation_revoked")).toBe("revoked");
    expect(invitationErrorKey("invitation_already_accepted")).toBe(
      "alreadyAccepted"
    );
    expect(invitationErrorKey("invitation_link_exhausted")).toBe("exhausted");
    expect(invitationErrorKey("email_mismatch")).toBe("emailMismatch");
    expect(invitationErrorKey("invitation_tenant_mismatch")).toBe(
      "tenantMismatch"
    );
    expect(invitationErrorKey("team_suspended")).toBe("teamSuspended");
  });

  it("falls back to invalid for unknown messages", () => {
    expect(invitationErrorKey("invalid email")).toBe("invalid");
  });
});

describe("buildAcceptUrl", () => {
  it("resolves the accept path against the current origin", () => {
    const url = buildAcceptUrl("/invitation/accept?token=abc&type=invite");
    expect(url).toContain("/invitation/accept?token=abc&type=invite");
    expect(url.startsWith("http")).toBe(true);
  });
});
