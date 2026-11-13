// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";

import SettingsProjectRepositoriesView from "./SettingsProjectRepositoriesView.vue";
import { mountShell } from "@/components/shell/test-utils";
import { buildRoutes } from "@/router";
import type { RepositoryV2 } from "@/api/initialization";
import type { SshKey } from "@/api/types";

const listRepositoriesV2 = vi.fn();
const createRepositoryV2 = vi.fn();
const confirmRepositoryVersion = vi.fn();
const disconnectRepository = vi.fn();
const updateRepositoryV2 = vi.fn();
const verifyRepositoryV2 = vi.fn();
const listSshKeys = vi.fn();
const importSshKey = vi.fn();
const revokeSshKey = vi.fn();
const bindRepositoryKey = vi.fn();

vi.mock("@/api/initialization", () => ({
  listRepositoriesV2: (...args: unknown[]) => listRepositoriesV2(...args),
  createRepositoryV2: (...args: unknown[]) => createRepositoryV2(...args),
  confirmRepositoryVersion: (...args: unknown[]) =>
    confirmRepositoryVersion(...args),
  disconnectRepository: (...args: unknown[]) => disconnectRepository(...args),
  updateRepositoryV2: (...args: unknown[]) => updateRepositoryV2(...args),
  verifyRepositoryV2: (...args: unknown[]) => verifyRepositoryV2(...args)
}));

vi.mock("@/api/repositories", () => ({
  listSshKeys: (...args: unknown[]) => listSshKeys(...args),
  importSshKey: (...args: unknown[]) => importSshKey(...args),
  revokeSshKey: (...args: unknown[]) => revokeSshKey(...args),
  bindRepositoryKey: (...args: unknown[]) => bindRepositoryKey(...args)
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

/** SshKey 契约（迁移 0022 多把：公钥 + 指纹 + is_default，无私钥字段）的最小夹具。 */
function sshKey(overrides: Partial<SshKey>): SshKey {
  return {
    id: "key-1",
    project_id: "prj-1",
    label: "github",
    public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI... ai-pm:demo",
    fingerprint: "SHA256:AbCdEf123456",
    status: "active",
    is_default: true,
    created_at: "2026-11-13T00:00:00Z",
    ...overrides
  };
}

async function mountView() {
  // 项目作用域经 route.query.project_id 注入（与 BaselinePage 同款取值链）
  const router = createRouter({
    history: createMemoryHistory(),
    routes: buildRoutes()
  });
  router.push("/project/settings/repositories?project_id=prj-1");
  await router.isReady();
  const mounted = mountShell(SettingsProjectRepositoriesView, {}, router);
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

describe("SettingsProjectRepositoriesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listRepositoriesV2.mockResolvedValue({ items: [], total: 0 });
    // 默认未配置密钥（空列表）
    listSshKeys.mockResolvedValue({ items: [], total: 0 });
  });

  it("零仓库时空态明确传达项目仍可工作（V1.2 §6.3）", async () => {
    const { wrapper } = await mountView();
    const empty = wrapper.find('[data-testid="repo-empty"]');
    expect(empty.exists()).toBe(true);
    // 空态文案必须传达「未接入 ≠ 不可用」——字面量断言防 i18n MISS 恒真
    expect(empty.text()).toContain("尚未关联仓库");
    expect(empty.text()).toContain("项目已可以开展工作");
  });

  it("统计卡渲染标签（compact + icon；迁入前 stacked 无 icon 只剩裸数字）", async () => {
    const { wrapper } = await mountView();
    const text = wrapper.text();
    for (const label of [
      "已关联仓库",
      "可达性已验证",
      "需要关注",
      "已确认基准"
    ]) {
      expect(text).toContain(label);
    }
  });

  it("无项目上下文时不发请求（scope 哨兵 __all__ 不得当作项目 id）", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: buildRoutes()
    });
    router.push("/project/settings/repositories");
    await router.isReady();
    mountShell(SettingsProjectRepositoriesView, {}, router);
    await flushPromises();
    expect(listRepositoriesV2).not.toHaveBeenCalled();
  });

  it("侧栏条目携带项目上下文，且当前节保持激活", async () => {
    const { wrapper } = await mountView();
    const runners = wrapper
      .findAll("a")
      .find(a => a.attributes("href")?.startsWith("/project/settings/runners"));
    expect(runners?.attributes("href")).toBe(
      "/project/settings/runners?project_id=prj-1"
    );
    const active = wrapper.find(".settings-nav-item.is-active");
    expect(active.attributes("href")).toBe(
      "/project/settings/repositories?project_id=prj-1"
    );
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

  it("SSH 密钥未配置态：空列表提示，名称+粘贴保存走导入契约（trim 后提交）", async () => {
    importSshKey.mockResolvedValue(sshKey({}));
    const { wrapper } = await mountView();
    const panel = wrapper.find('[data-testid="ssh-key-panel"]');
    expect(panel.exists()).toBe(true);
    expect(panel.find('[data-testid="ssh-key-status"]').text()).toContain(
      "未配置"
    );
    expect(panel.find('[data-testid="ssh-key-empty"]').text()).toContain(
      "还没有密钥"
    );

    await wrapper
      .find('[data-testid="ssh-key-name"]')
      .setValue(" github-main ");
    await wrapper
      .find('[data-testid="ssh-key-input"]')
      .setValue(
        "  -----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----\n  "
      );
    await wrapper.find('[data-testid="ssh-key-submit"]').trigger("click");
    await flushPromises();

    expect(importSshKey).toHaveBeenCalledWith(
      "prj-1",
      "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----",
      "github-main"
    );
    // 空名称 / 空输入不得提交
    await wrapper.find('[data-testid="ssh-key-name"]').setValue("  ");
    await wrapper
      .find('[data-testid="ssh-key-input"]')
      .setValue("-----BEGIN OPENSSH PRIVATE KEY-----");
    await wrapper.find('[data-testid="ssh-key-submit"]').trigger("click");
    await flushPromises();
    expect(importSshKey).toHaveBeenCalledTimes(1);
  });

  it("SSH 密钥列表：展示名称/指纹/默认标记，吊销两段式按 key 执行", async () => {
    listSshKeys.mockResolvedValue({
      items: [
        sshKey({}),
        sshKey({ id: "key-2", label: "gitlab", is_default: false })
      ],
      total: 2
    });
    revokeSshKey.mockResolvedValue({ id: "key-2" });
    const { wrapper } = await mountView();
    const panel = wrapper.find('[data-testid="ssh-key-panel"]');
    expect(panel.find('[data-testid="ssh-key-status"]').text()).toContain(
      "已配置"
    );
    expect(panel.findAll('[data-testid="ssh-key-item"]')).toHaveLength(2);
    expect(panel.text()).toContain("github");
    expect(panel.text()).toContain("gitlab");
    expect(panel.text()).toContain("SHA256:");
    expect(panel.text()).toContain("默认");

    // 两段式吊销：第一击武装（文案变为确认），第二击按 key id 执行
    const items = wrapper.findAll('[data-testid="ssh-key-item"]');
    const gitlabRevoke = items[1]!
      .findAll("button")
      .find(b => b.text().includes("吊销"))!;
    await gitlabRevoke.trigger("click");
    expect(gitlabRevoke.text()).toContain("确认吊销");
    await gitlabRevoke.trigger("click");
    await flushPromises();
    expect(revokeSshKey).toHaveBeenCalledWith("key-2");
  });

  it("仓库详情可绑定/解绑 SSH 密钥（迁移 0022；空串 = 跟随项目默认）", async () => {
    listSshKeys.mockResolvedValue({
      items: [
        sshKey({}),
        sshKey({ id: "key-2", label: "gitlab", is_default: false })
      ],
      total: 2
    });
    listRepositoriesV2.mockResolvedValue({
      items: [repo({ id: "r-bind" })],
      total: 1
    });
    bindRepositoryKey.mockResolvedValue(
      repo({ id: "r-bind", ssh_key_id: "key-2" })
    );
    const { wrapper } = await mountView();

    await wrapper.find('[data-testid="repo-list"] > div').trigger("click");
    const select = wrapper.find('[data-testid="bind-key-select"]');
    expect(select.exists()).toBe(true);

    await select.setValue("key-2");
    await flushPromises();
    expect(bindRepositoryKey).toHaveBeenCalledWith("r-bind", "key-2");

    await wrapper.find('[data-testid="bind-key-select"]').setValue("");
    await flushPromises();
    expect(bindRepositoryKey).toHaveBeenCalledWith("r-bind", null);
  });
});
