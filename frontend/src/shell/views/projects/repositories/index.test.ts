// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import RepositoriesPage from "./index.vue";
import { mountShell } from "../../../test-utils";

const listRepositories = vi.fn();
const createRepository = vi.fn();
const updateRepository = vi.fn();
const deleteRepository = vi.fn();
const getSshKey = vi.fn();
const generateSshKey = vi.fn();
const deleteSshKey = vi.fn();

vi.mock("@/api/repositories", () => ({
  listRepositories: (...args: unknown[]) => listRepositories(...args),
  createRepository: (...args: unknown[]) => createRepository(...args),
  updateRepository: (...args: unknown[]) => updateRepository(...args),
  deleteRepository: (...args: unknown[]) => deleteRepository(...args),
  getSshKey: (...args: unknown[]) => getSshKey(...args),
  generateSshKey: (...args: unknown[]) => generateSshKey(...args),
  deleteSshKey: (...args: unknown[]) => deleteSshKey(...args)
}));

vi.mock("vue-router", () => ({
  useRoute: () => ({ params: { projectId: "project-1" } }),
  useRouter: () => ({ push: vi.fn() })
}));

vi.mock("@/api/scope", () => ({
  SCOPE_ALL: "__all__",
  useScopeStore: () => ({
    projects: [],
    currentProjectId: "__all__",
    projectIdParam: undefined,
    ensureLoaded: async () => undefined
  })
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

const repositories = [
  {
    id: "repo-1",
    project_id: "project-1",
    name: "主仓库",
    url: "git@gitlab.local:team/world-map.git",
    role: "main",
    default_branch: "main",
    description: "游戏客户端",
    revision: 1,
    created_at: "2026-10-31T00:00:00Z",
    updated_at: "2026-10-31T00:00:00Z"
  },
  {
    id: "repo-2",
    project_id: "project-1",
    name: "文档仓库",
    url: "ssh://git@gitlab.local:2222/team/docs.git",
    role: "docs",
    default_branch: null,
    description: null,
    revision: 1,
    created_at: "2026-10-31T00:00:00Z",
    updated_at: "2026-10-31T00:00:00Z"
  }
];

beforeEach(() => {
  vi.clearAllMocks();
  listRepositories.mockResolvedValue({
    items: repositories,
    total: repositories.length
  });
  getSshKey.mockRejectedValue(new Error("404 not found"));
});

describe("项目代码仓库页", () => {
  it("渲染已绑定仓库与 SSH 密钥空态", async () => {
    const wrapper = mountShell(RepositoriesPage);
    await flushPromises();

    expect(listRepositories).toHaveBeenCalledWith("project-1");
    const text = wrapper.text();
    expect(text).toContain("主仓库");
    expect(text).toContain("git@gitlab.local:team/world-map.git");
    expect(text).toContain("文档仓库");
    expect(text).toContain("主仓库");
    expect(text).toContain("还没有生成密钥");
  });

  it("无仓库时展示空态文案", async () => {
    listRepositories.mockResolvedValue({ items: [], total: 0 });
    const wrapper = mountShell(RepositoriesPage);
    await flushPromises();
    expect(wrapper.text()).toContain("还没有关联仓库");
  });

  it("生成密钥后展示公钥与指纹", async () => {
    generateSshKey.mockResolvedValue({
      project_id: "project-1",
      label: "default",
      public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI-test ai-pm:PROJ",
      fingerprint: "SHA256:abcdef",
      created_at: "2026-10-31T00:00:00Z"
    });
    const wrapper = mountShell(RepositoriesPage);
    await flushPromises();

    const generate = wrapper
      .findAll("button")
      .find(button => button.text() === "生成密钥");
    expect(generate, "生成密钥按钮应存在").toBeTruthy();
    await generate!.trigger("click");
    await flushPromises();

    expect(generateSshKey).toHaveBeenCalledWith("project-1");
    expect(wrapper.text()).toContain(
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI-test"
    );
    expect(wrapper.text()).toContain("SHA256:abcdef");
  });
});
