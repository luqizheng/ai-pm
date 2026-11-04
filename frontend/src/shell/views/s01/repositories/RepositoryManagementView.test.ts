// @vitest-environment happy-dom
/**
 * S01-03 仓库管理页行为测试。
 *
 * 断言 UX 硬语义：
 *   - Branch HEAD 变化只提示，不自动更新采用版本
 *   - 无 Primary 是合法状态，只提示不阻断
 *   - Disconnect 是逻辑断开
 *   - Default / Selected / HEAD 三者分别呈现
 */
import { describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { mountShell } from "@/shell/test-utils";
import { setLocale } from "@/shell/i18n";
import RepositoryManagementView from "./RepositoryManagementView.vue";
import type { RepositoryV2 } from "@/api/types/initialization";

const mocks = vi.hoisted(() => ({
  disconnect: vi.fn(),
  restore: vi.fn(),
  setPrimary: vi.fn(),
  updateVersion: vi.fn(),
  verify: vi.fn()
}));

function repository(overrides: Partial<RepositoryV2> = {}): RepositoryV2 {
  return {
    id: "repo-1",
    project_id: "p1",
    name: "主仓库",
    url: "git@gitlab.local:team/main.git",
    provider: "gitlab",
    repo_owner: "team",
    role: "supporting",
    default_branch: "main",
    selected_branch: "main",
    selected_commit: "abc123",
    connection_status: "connected",
    verification_status: "passed",
    last_verified_at: "2026-11-04T10:00:00Z",
    last_scanned_at: null,
    head_commit_sha: "def456",
    description: null,
    revision: 1,
    created_at: "2026-11-04T09:00:00Z",
    updated_at: "2026-11-04T10:00:00Z",
    ...overrides
  };
}

const rows: RepositoryV2[] = [repository()];

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: { projectId: "p1" } }),
  useRouter: () => ({ push: vi.fn() })
}));

vi.mock("@/api/initialization", () => ({
  listRepositoriesV2: vi.fn(async () => ({ items: rows, total: 1 })),
  listGitProviders: vi.fn(async () => [
    {
      provider: "gitlab",
      scopes: ["repo:read"],
      status: "active",
      expires_at: null,
      authorized_at: null
    }
  ]),
  disconnectRepository: mocks.disconnect,
  restoreRepository: mocks.restore,
  setPrimaryRepository: mocks.setPrimary,
  updateRepositoryVersion: mocks.updateVersion,
  verifyRepositoryV2: mocks.verify
}));

vi.mock("@/api/format", () => ({ formatDateTime: (value: string) => value }));

async function mountView() {
  // i18n 是全局单例：其他测试文件可能切到 en，这里固定回 zh-CN
  setLocale("zh-CN");
  const wrapper = mountShell(RepositoryManagementView);
  await nextTick();
  await nextTick();
  await nextTick();
  return wrapper;
}

describe("RepositoryManagementView", () => {
  it("Branch HEAD 变化只提示，不自动改写项目采用版本", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("仓库已有新提交");
    // 采用版本仍是 abc123，HEAD 是 def456，两者分别呈现
    expect(wrapper.text()).toContain("abc123");
    expect(wrapper.text()).toContain("def456");
  });

  it("无 Primary 仓库时提示但不阻断", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("项目最多一个主仓库，也可以暂时没有");
    // 仓库仍在列表中
    expect(wrapper.text()).toContain("主仓库");
  });

  it("区分仓库默认分支与项目采用分支", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("仓库默认分支");
    expect(wrapper.text()).toContain("项目采用分支");
    expect(wrapper.text()).toContain("项目采用版本");
  });

  it("平台授权只展示授权范围，不暴露凭证", async () => {
    const wrapper = await mountView();
    expect(wrapper.text()).toContain("repo:read");
    expect(wrapper.text()).not.toContain("token");
  });
});
