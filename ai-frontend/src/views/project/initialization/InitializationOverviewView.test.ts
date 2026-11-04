// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";

import InitializationOverviewView from "./InitializationOverviewView.vue";
import { i18n } from "@/lib/i18n";
import { commandCenter } from "@/api/mock/command-center";

/**
 * S01-01 页面行为。
 *
 * 断言行为而不是类名：
 *   - 六个阶段全部呈现，且状态来自后端派生事项；
 *   - Current Action 唯一且带责任主体；
 *   - 待人工处理只展示后端派生出的待确认事项；
 *   - AI 上下文栏可见且标注只读；
 *   - **不出现百分比、不宣称「初始化完成」**（后端 completion 恒为 not_determinable）。
 *
 * 本用例自建挂载（不依赖 @/router 的导航树）：页面只用到 useRoute / useRouter，
 * 自建路由可让用例在导航树变动时保持稳定。
 */

vi.mock("@/api/command-center", async importOriginal => {
  const actual = await importOriginal<typeof import("@/api/command-center")>();
  return {
    ...actual,
    getCommandCenter: vi.fn(async () => commandCenter)
  };
});

async function mountView() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/project/initialization", component: InitializationOverviewView }
    ]
  });
  await router.push("/project/initialization?project_id=prj-shield");
  await router.isReady();

  const wrapper = mount(InitializationOverviewView, {
    global: { plugins: [createPinia(), i18n, router] }
  });
  await flushPromises();
  return { wrapper, router };
}

describe("InitializationOverviewView", () => {
  it("呈现六个初始化阶段并沿用后端派生状态", async () => {
    const { wrapper } = await mountView();

    const stages = wrapper.findAll("[data-stage]");
    expect(stages).toHaveLength(6);

    // 后端：repository_access 与 code_analysis 都是 in_progress → 聚合为进行中
    const repository = wrapper.find('[data-stage="repository"]');
    expect(repository.attributes("data-status")).toBe("in_progress");

    // 后端：会话在开 + 有待确认发现 → 等待你确认（G-01，非 in_progress）
    const cognition = wrapper.find('[data-stage="cognition"]');
    expect(cognition.attributes("data-status")).toBe("waiting_for_human");

    // 后端：dev_environment 从未自检 → 尚无法判断
    const environment = wrapper.find('[data-stage="environment"]');
    expect(environment.attributes("data-status")).toBe("undetermined");
  });

  it("G-01 七态：等待你确认与进行中/阻塞可区分地渲染", async () => {
    const { wrapper } = await mountView();

    // 夹具：human_action_summary 有 3 条待确认事项 → 基线阶段合成出 waiting_for_human。
    const baseline = wrapper.find('[data-stage="baseline"]');
    expect(baseline.attributes("data-status")).toBe("waiting_for_human");
    expect(baseline.text()).toContain("等待你确认");
    // 等待人类的实况指向真实待办数量，而不是笼统的「进行中」。
    expect(baseline.text()).toContain("3 项认知结果待人工确认");

    // 进行中（仓库）与等待（基线）同时在场且状态不同——三态不混淆。
    expect(
      wrapper.find('[data-stage="repository"]').attributes("data-status")
    ).toBe("in_progress");
  });

  it("当前动作唯一并标注责任主体", async () => {
    const { wrapper } = await mountView();

    const current = wrapper.find('[data-testid="init-current-action"]');
    expect(current.exists()).toBe(true);

    // 阶梯：waiting 优先于 in_progress（人工待办不得被进行中覆盖）——
    // 认知与基线同时在等确认，按阶段序取最小（认知=3）→ 当前动作归人类。
    expect(current.text()).toContain("需要你处理");
    expect(current.text()).toContain("AI-PM 认知正在等待你确认");
    expect(current.text()).toContain("需要你操作");
  });

  it("待人工处理只展示后端派生出的待确认事项", async () => {
    const { wrapper } = await mountView();

    const actions = wrapper.find('[data-testid="init-human-actions"]');
    expect(actions.exists()).toBe(true);
    expect(actions.findAll("li")).toHaveLength(3);
    expect(actions.text()).toContain("待确认发现");
  });

  it("AI 上下文栏可见且标注只读", async () => {
    const { wrapper } = await mountView();

    const rail = wrapper.find('[data-testid="init-ai-context"]');
    expect(rail.exists()).toBe(true);
    expect(rail.text()).toContain("只读");
  });

  it("不展示百分比，也不宣称初始化已完成", async () => {
    const { wrapper } = await mountView();

    expect(wrapper.text()).not.toContain("%");
    expect(wrapper.text()).not.toContain("初始化完成");
    // completion = not_determinable → 交接态是「项目接入概况」
    expect(wrapper.find(".init-handover").attributes("data-outcome")).toBe(
      "undetermined"
    );
  });
});
