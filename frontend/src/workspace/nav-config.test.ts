import { describe, expect, it } from "vitest";
import { NAV_GROUPS, navDefaultSlug, navSlugs } from "./nav-config";

describe("workspace 导航模式配置", () => {
  it("传统模式 9 项：工作空间组 4 + 研发模块组 5", () => {
    expect(navSlugs("classic")).toEqual([
      "console",
      "team",
      "goals",
      "activity",
      "requirements",
      "decisions",
      "knowledge",
      "artifacts",
      "models"
    ]);
    expect(NAV_GROUPS.classic.map(group => group.key)).toEqual([
      "workspace",
      "modules"
    ]);
  });

  it("AI 模式（V1.2）5 项：单工作空间组", () => {
    expect(navSlugs("ai")).toEqual([
      "goals",
      "execution",
      "decisions",
      "knowledge",
      "artifacts"
    ]);
    expect(NAV_GROUPS.ai).toHaveLength(1);
    expect(NAV_GROUPS.ai[0].key).toBe("workspace");
  });

  it("默认落点：传统=console，AI=goals", () => {
    expect(navDefaultSlug("classic")).toBe("console");
    expect(navDefaultSlug("ai")).toBe("goals");
  });
});
