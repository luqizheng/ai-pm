// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import Vditor from "vditor";

/**
 * 不 mock，真实导入。
 *
 * 验证 Vite 能把这个 UMD 包解析成 ESM 且默认导出完整 —— 这是接入第三方编辑器时
 * 最先出问题的环节（CJS 互操作、具名导出丢失、默认导出为 undefined）。
 * 一旦这里红，说明打包链路有问题，而不是组件用法有问题。
 */
describe("vditor 模块形态", () => {
  it("默认导出可用，且带静态渲染方法", () => {
    expect(Vditor).toBeDefined();
    expect(typeof Vditor.md2html).toBe("function");
    expect(typeof Vditor.preview).toBe("function");
  });
});
