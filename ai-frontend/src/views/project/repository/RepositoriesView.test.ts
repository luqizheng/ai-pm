// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import RepositoriesView from "./RepositoriesView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { RepositoryV2 } from "@/api/initialization";

const listRepositoriesV2 = vi.fn();
const createRepositoryV2 = vi.fn();
const confirmRepositoryVersion = vi.fn();
const disconnectRepository = vi.fn();
const updateRepositoryV2 = vi.fn();
const verifyRepositoryV2 = vi.fn();

vi.mock("@/api/initialization", () => ({
  listRepositoriesV2: (...args: unknown[]) => listRepositoriesV2(...args),
  createRepositoryV2: (...args: unknown[]) => createRepositoryV2(...args),
  confirmRepositoryVersion: (...args: unknown[]) =>
    confirmRepositoryVersion(...args),
  disconnectRepository: (...args: unknown[]) => disconnectRepository(...args),
  updateRepositoryV2: (...args: unknown[]) => updateRepositoryV2(...args),
  verifyRepositoryV2: (...args: unknown[]) => verifyRepositoryV2(...args)
}));

/** 后端真实行形状（RepositoryRow 契约）的最小夹具。 */
function repo(overrides: Partial<RepositoryV2>): RepositoryV2 {
  return {
    id: "repo-1",
    project_id: "prj-1",
    name: "payment-service",
    url: "git@git.example.com:acme/payment-service.git",
    role: "main",
    default_branch: "main",
    description: null,
    ssh_key_id: null,
    status: "verified",
    head_commit_sha: "abc1234",
    selected_branch: "main",
    selected_commit: "abc1234",
    selected_confirmed_at: "2026-11-05T00:00:00Z",
    selected_confirmed_by: null,
    last_verified_at: "2026-11-05T00:00:00Z",
    last_error_code: null,
    last_error_detail: null,
    revision: 3,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-05T00:00:00Z",
    ...overrides
  };
}

async function mountView() {
  // 项目作用域经 route.query.project_id 注入（与 BaselinePage 同款取值链）
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/repository/overview?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(RepositoriesView, {}, router);
  await flushPromises();
  return mounted;
}

function findButtonByText(
  wrapper: { findAll: (sel: string) => { text: () => string }[] },
  text: string
) {
  const target = wrapper.findAll("button").find(b => b.text().includes(text));
  expect(target, `button "${text}" should exist`).toBeTruthy();
  return target!;
}

describe("RepositoriesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listRepositoriesV2.mockResolvedValue({ items: [], total: 0 });
  });

  it("零仓库时空态明确传达项目仍可工作（V1.2 §6.3）", async () => {
    const { wrapper } = await mountView();
    const empty = wrapper.find('[data-testid="repo-empty"]');
    expect(empty.exists()).toBe(true);
    // 空态文案必须传达「未接入 ≠ 不可用」——字面量断言防 i18n MISS 恒真
    expect(empty.text()).toContain("尚未关联仓库");
    expect(empty.text()).toContain("项目已可以开展工作");
  });

  it("Primary 由 role=main 唯一性派生：恰好一个显示建议标记，两个都不显示（G-03 前端派生）", async () => {
    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({ id: "r-main", role: "main" }),
        repo({ id: "r-docs", role: "docs", name: "docs" })
      ],
      total: 2
    });
    const { wrapper } = await mountView();
    expect(wrapper.findAll('[data-testid="primary-badge"]')).toHaveLength(1);

    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({ id: "r-main-1", role: "main", name: "a" }),
        repo({ id: "r-main-2", role: "main", name: "b" })
      ],
      total: 2
    });
    const again = await mountView();
    expect(again.wrapper.findAll('[data-testid="primary-badge"]')).toHaveLength(
      0
    );
  });

  it("HEAD 与确认基准不一致时只提示 + 人工确认更新采用 Observed HEAD（R-03-04，不自动更新）", async () => {
    listRepositoriesV2.mockResolvedValue({
      items: [
        repo({
          id: "r-ahead",
          head_commit_sha: "def5678",
          selected_commit: "abc1234"
        })
      ],
      total: 1
    });
    confirmRepositoryVersion.mockResolvedValue(repo({}));
    const { wrapper } = await mountView();

    // 详情面板随行选中渲染：先点列表行
    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");
    const change = wrapper.find('[data-testid="head-change"]');
    expect(change.exists()).toBe(true);
    expect(change.text()).toContain("确认基准未被改动");

    await findButtonByText(wrapper, "确认更新").trigger("click");
    await flushPromises();
    // 人类确认：采用 Observed HEAD，而不是静默改动
    expect(confirmRepositoryVersion).toHaveBeenCalledWith("r-ahead", {
      selected_branch: "main",
      selected_commit: "def5678",
      reason: "confirm observed head"
    });
  });

  it("接入表单提交走真实创建契约（含可选初始基准）", async () => {
    createRepositoryV2.mockResolvedValue(repo({}));
    const { wrapper } = await mountView();
    await findButtonByText(wrapper, "接入仓库").trigger("click");

    await wrapper.find('[data-testid="connect-name"]').setValue("docs");
    await wrapper
      .find('[data-testid="connect-url"]')
      .setValue("git@git.example.com:acme/docs.git");
    // 用 testid 精确命中提交按钮（头部「接入仓库」按钮同样包含「接入」字样）
    await wrapper.find('[data-testid="connect-submit"]').trigger("click");
    await flushPromises();

    expect(createRepositoryV2).toHaveBeenCalledWith("prj-1", {
      name: "docs",
      url: "git@git.example.com:acme/docs.git",
      role: "main",
      default_branch: null,
      selected_branch: null,
      selected_commit: null,
      description: null
    });
  });
});
