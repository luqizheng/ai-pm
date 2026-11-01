// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ProjectImportWizard from "./ProjectImportWizard.vue";
import { mountShell } from "../../test-utils";

const createProject = vi.fn();
const createSshKey = vi.fn();
const listRepositories = vi.fn();
const createRepository = vi.fn();
const verifyRepository = vi.fn();
const startImport = vi.fn();
const getImport = vi.fn();
const cancelImport = vi.fn();
const applyImport = vi.fn();
const listCustomers = vi.fn();

vi.mock("@/api/projects", () => ({
  createProject: (...args: unknown[]) => createProject(...args)
}));

vi.mock("@/api/customers", () => ({
  listCustomers: (...args: unknown[]) => listCustomers(...args)
}));

vi.mock("@/api/repositories", () => ({
  createRepository: (...args: unknown[]) => createRepository(...args),
  createSshKey: (...args: unknown[]) => createSshKey(...args),
  listRepositories: (...args: unknown[]) => listRepositories(...args),
  verifyRepository: (...args: unknown[]) => verifyRepository(...args)
}));

vi.mock("@/api/imports", () => ({
  startImport: (...args: unknown[]) => startImport(...args),
  getImport: (...args: unknown[]) => getImport(...args),
  cancelImport: (...args: unknown[]) => cancelImport(...args),
  applyImport: (...args: unknown[]) => applyImport(...args)
}));

vi.mock("vue-router", () => ({
  useRouter: () => ({ push: vi.fn() }),
  useRoute: () => ({ query: {}, params: {} })
}));

vi.mock("@/components/ui/toast", () => ({
  useToast: () => ({ toast: vi.fn() })
}));

const verifiedRepo = {
  id: "repo-1",
  project_id: "project-1",
  name: "主仓库",
  url: "git@gitlab.local:team/world-map.git",
  role: "main",
  default_branch: "main",
  description: null,
  ssh_key_id: "project-1",
  status: "verified",
  head_commit_sha: "a".repeat(40),
  last_verified_at: "2026-10-31T00:00:00Z",
  last_error_code: null,
  last_error_detail: null,
  revision: 1,
  created_at: "2026-10-31T00:00:00Z",
  updated_at: "2026-10-31T00:00:00Z"
};

beforeEach(() => {
  vi.clearAllMocks();
  createProject.mockResolvedValue({
    id: "project-1",
    customer_id: "customer-1",
    code: "IMP-1",
    name: "导入项目",
    description: null,
    status: "planning",
    revision: 1,
    created_at: "2026-10-31T00:00:00Z",
    updated_at: "2026-10-31T00:00:00Z"
  });
  createSshKey.mockResolvedValue({
    project_id: "project-1",
    label: "default",
    public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI-test",
    fingerprint: "SHA256:abcdef",
    status: "active",
    created_at: "2026-10-31T00:00:00Z"
  });
  listRepositories.mockResolvedValue({ items: [], total: 0 });
  listCustomers.mockResolvedValue({ items: [], total: 0 });
});

describe("冷启动导入向导", () => {
  it("创建项目后进入连接仓库步骤并展示 SSH 公钥", async () => {
    const wrapper = mountShell(ProjectImportWizard);
    await flushPromises();

    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("IMP-1");
    await inputs[1].setValue("导入项目");

    const createButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("创建项目"));
    expect(createButton, "创建按钮应可用").toBeTruthy();
    await createButton!.trigger("click");
    await flushPromises();

    expect(createProject).toHaveBeenCalledWith({
      code: "IMP-1",
      name: "导入项目",
      description: ""
    });
    expect(wrapper.text()).toContain("连接仓库");
    expect(wrapper.text()).toContain(
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI-test"
    );
    expect(wrapper.text()).toContain("SHA256:abcdef");
  });

  it("没有已连通仓库时禁用开始分析", async () => {
    listRepositories.mockResolvedValueOnce({ items: [], total: 0 });
    const wrapper = mountShell(ProjectImportWizard);
    await flushPromises();

    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("IMP-2");
    await inputs[1].setValue("导入项目二");
    const createButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("创建项目"));
    await createButton!.trigger("click");
    await flushPromises();

    const startButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("开始分析"));
    expect(startButton, "开始分析按钮应存在").toBeTruthy();
    expect(startButton!.attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toContain("至少需要一个已连通的仓库");
  });

  it("仓库校验后允许开始分析", async () => {
    const pendingRepo = { ...verifiedRepo, status: "pending" };
    listRepositories
      .mockResolvedValueOnce({ items: [pendingRepo], total: 1 })
      .mockResolvedValue({ items: [verifiedRepo], total: 1 });
    verifyRepository.mockResolvedValue({
      repository_id: "repo-1",
      status: "verified",
      default_branch: "main",
      head_commit_sha: "a".repeat(40),
      error_code: null,
      error_detail: null
    });
    const wrapper = mountShell(ProjectImportWizard);
    await flushPromises();

    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("IMP-3");
    await inputs[1].setValue("导入项目三");
    await wrapper
      .findAll("button")
      .find(button => button.text().includes("创建项目"))!
      .trigger("click");
    await flushPromises();

    const startBefore = wrapper
      .findAll("button")
      .find(button => button.text().includes("开始分析"));
    expect(startBefore!.attributes("disabled")).toBeDefined();

    const verify = wrapper
      .findAll("button")
      .find(button => button.text() === "校验");
    await verify!.trigger("click");
    await flushPromises();

    expect(verifyRepository).toHaveBeenCalledWith("repo-1");
    const startButton = wrapper
      .findAll("button")
      .find(button => button.text().includes("开始分析"));
    expect(startButton!.attributes("disabled")).toBeUndefined();
  });
});