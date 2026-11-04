// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import DatePicker from "./DatePicker.vue";

describe("DatePicker", () => {
  it("默认渲染 label / 值 / 帮助文案", () => {
    const w = mount(DatePicker, { props: {} });
    expect(w.text()).toContain("Due date");
    expect(w.text()).toContain("Choose a date");
    expect(w.text()).toContain("Date format: MMM DD, YYYY");
  });

  it("open=false 不渲染日历；open=true 渲染 6×7 网格与页脚", () => {
    const closed = mount(DatePicker, { props: {} });
    expect(closed.find(".ui-datepicker__calendar").exists()).toBe(false);

    const open = mount(DatePicker, { props: { open: true } });
    expect(open.text()).toContain("November 2026");
    expect(open.text()).toContain("Today");
    expect(open.text()).toContain("Clear");
    expect(open.findAll(".ui-datepicker__week").length).toBe(7);
  });

  it("点击日期发出 select 并回填值、收起日历", async () => {
    const w = mount(DatePicker, { props: { open: true, selectedDay: "18" } });
    const dayButtons = w.findAll("button").filter(b => b.text() === "19");
    await dayButtons[0]!.trigger("click");
    expect(w.emitted("select")?.[0]).toEqual(["19"]);
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["19"]);
    expect(w.emitted("update:open")?.[0]).toEqual([false]);
  });

  it("error 覆盖帮助文案并切换 danger 色", () => {
    const w = mount(DatePicker, {
      props: { error: "Choose a date after today." }
    });
    expect(w.text()).toContain("Choose a date after today.");
    expect(w.html()).toContain("is-error");
  });

  it("disabled 时字段禁用", () => {
    const w = mount(DatePicker, { props: { disabled: true } });
    expect(
      w.find("button.ui-datepicker__field").attributes("disabled")
    ).toBeDefined();
  });
});
