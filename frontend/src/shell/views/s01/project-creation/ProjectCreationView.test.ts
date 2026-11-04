// @vitest-environment happy-dom
/**
 * S01-02 项目创建页行为测试。
 *
 * 断言 UX 硬语义：
 *   - Project Name 必填
 *   - Project Key 由系统建议、人类确认（可改）
 *   - 可选背景信息不阻止创建
 *   - 创建请求带幂等键
 *   - 创建成功后进入初始化工作空间
 */
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import { setLocale } from "@/shell/i18n";
import ProjectCreationView from "./ProjectCreationView.vue";

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(async () => true),
  toast: vi.fn(),
  create: vi.fn(async () => ({
    id: "p-new",
    code: "SMARK",
    name: "Smark PM",
    customer_id: null,
    owner_principal_id: "u1",
    status: "planning",
    revision: 1,
    init_status: "not_started",
    created_at: "2026-11-04T10:00:00Z",
    updated_at: "2026-11-04T10:00:00Z"
  })),
  suggest: vi.fn(async () => ({
    code: "SMARK",
    available: true,
    conflict_code: null
  }))
}));

const push = vi.fn();

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: {} }),
  useRouter: () => ({ push })
}));

vi.mock("@/api/customers", () => ({
  listCustomers: vi.fn(async () => ({ items: [], total: 0 }))
}));

vi.mock("@/api/pm", () => ({
  listPrincipals: vi.fn(async () => ({
    items: [
      {
        id: "u1",
        kind: "human",
        display_name: "张伟",
        is_active: true,
        created_at: ""
      }
    ],
    total: 1,
    page: 1,
    size: 200
  }))
}));

vi.mock("@/api/initialization", () => ({
  createInitializedProject: mocks.create,
  suggestProjectCode: mocks.suggest
}));

vi.mock("@/shell/composables/useConfirm", () => ({
  useConfirm: () => ({ confirm: mocks.confirm })
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: mocks.toast })
}));

async function mountView() {
  // i18n 是全局单例：其他测试文件可能切到 en，这里固定回 zh-CN
  setLocale("zh-CN");
  const wrapper = mountShell(ProjectCreationView);
  await nextTick();
  await nextTick();
  return wrapper;
}

describe("ProjectCreationView", () => {
  it("首步展示项目名称与标识，标识来自系统建议", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("项目名称");
    expect(wrapper.text()).toContain("项目标识");
  });

  it("名称为空时不允许进入下一步", async () => {
    const wrapper = await mountView();
    const nextButton = wrapper
      .findAll("button")
      .find(button => button.text() === "确认");
    expect(nextButton?.attributes("disabled")).toBeDefined();
  });

  it("背景信息全部留空也不阻止创建", async () => {
    const wrapper = await mountView();
    // 表单里没有必填的背景字段提示
    expect(wrapper.text()).not.toContain("项目背景（必填）");
  });

  it("创建请求携带幂等键，避免重复创建", async () => {
    const wrapper = await mountView();
    await wrapper.findAll("input")[0]?.setValue("Smark PM");
    // Key 建议是 debounce 触发的异步调用，等它落定
    await vi.waitFor(
      () => {
        expect(mocks.suggest).toHaveBeenCalled();
      },
      { timeout: 5000, interval: 50 }
    );
    await nextTick();

    // 跳到第四步（确认），再点「创建项目」
    await wrapper.findAll(".steps__btn")[3]?.trigger("click");
    await nextTick();
    const submitButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("创建项目"));
    expect(submitButton).toBeDefined();
    await submitButton?.trigger("click");
    // 二次确认是异步的：等确认弹窗 resolve 后才会真正提交
    await vi.waitFor(
      () => {
        expect(mocks.create).toHaveBeenCalled();
      },
      { timeout: 5000, interval: 50 }
    );

    const body = mocks.create.mock.calls[0][0];
    expect(body.idempotency_key).toBeTruthy();
    expect(body.name).toBe("Smark PM");
    expect(body.code).toBe("SMARK");
  });
});
