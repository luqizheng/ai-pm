import { defineComponent } from "vue";
import { beforeEach, describe, expect, it } from "vitest";
import {
  getWorkspaceComponent,
  registerWorkspaceComponent,
  resetWorkspaceRegistry,
  validateProps
} from "./registry";

const Stub = defineComponent({ render: () => null });

describe("workspace component registry", () => {
  beforeEach(() => resetWorkspaceRegistry());

  it("注册后可取回", () => {
    registerWorkspaceComponent({ id: "GoalStrip", component: Stub });
    expect(getWorkspaceComponent("GoalStrip")?.component).toBe(Stub);
  });

  it("未注册 id 返回 undefined", () => {
    expect(getWorkspaceComponent("Nope")).toBeUndefined();
  });

  it("带 validateConfig 的组件在非法 config 时返回 errors", () => {
    registerWorkspaceComponent({
      id: "SplitReview",
      component: Stub,
      validateConfig: config =>
        config && typeof config.title === "string" ? [] : ["title is required"]
    });
    const def = getWorkspaceComponent("SplitReview")!;
    expect(validateProps(def, { title: "ok" })).toEqual([]);
    expect(validateProps(def, {})).toEqual(["title is required"]);
  });

  it("未声明 validateConfig 时视为合法", () => {
    registerWorkspaceComponent({ id: "Plain", component: Stub });
    expect(validateProps(getWorkspaceComponent("Plain")!, { any: 1 })).toEqual([]);
  });
});